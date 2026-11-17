//! Opérations sur les pages (P-01 à P-03) : insérer une page blanche ou
//! les pages d'autres PDF, supprimer, dupliquer, déplacer, pivoter.
//!
//! Chacune est une seule entrée de l'historique, et les éléments ajoutés
//! suivent leur page : déplacés, copiés, retirés ou tournés avec elle.

use std::collections::{BTreeSet, HashMap};

use crate::document::{PageIndex, PageSize};

use super::element::ElementId;
use super::history::{Change, Edits, Operation};
use super::import::ImportId;
use super::pages::{MAX_PAGES, PagePlan, PlannedPage, check_blank_size};
use super::rotate::rotated;

/// Page du nouveau plan, décrite à partir du plan actuel.
enum Slot {
    /// Page actuelle (déplacée ou non) : ses éléments la suivent.
    Keep(PageIndex),
    /// Copie d'une page actuelle, éléments compris.
    Copy(PageIndex),
    /// Nouvelle page (blanche).
    New(PlannedPage),
}

impl Edits {
    /// Insère une page blanche à la place `at` (`at` = nombre de pages : à la fin).
    pub fn insert_blank(&mut self, at: PageIndex, size: PageSize) -> Result<Change, String> {
        check_blank_size(size.width, size.height)?;
        let count = self.plan.len();
        if at > count {
            return Err(missing(at));
        }
        let mut slots: Vec<Slot> = (0..count).map(Slot::Keep).collect();
        slots.insert(at, Slot::New(PlannedPage::blank(size)));
        self.restructure(slots)
    }

    /// Insère, à la place `at`, des pages de PDF déjà enregistrés par
    /// `register_import`, dans l'ordre donné (fusionner : à la fin).
    pub fn insert_imported(
        &mut self,
        at: PageIndex,
        pages: &[(ImportId, PageIndex)],
    ) -> Result<Change, String> {
        let count = self.plan.len();
        if at > count {
            return Err(missing(at));
        }
        if pages.is_empty() {
            return Err("aucune page à insérer".into());
        }
        if let Some((_, index)) = pages
            .iter()
            .find(|(file, index)| self.sources.imported_page(*file, *index).is_none())
        {
            return Err(format!("la page {} du PDF inséré n'existe pas", index + 1));
        }
        let mut slots: Vec<Slot> = (0..count).map(Slot::Keep).collect();
        let new = pages
            .iter()
            .map(|&(file, index)| Slot::New(PlannedPage::imported(file, index)));
        slots.splice(at..at, new);
        self.restructure(slots)
    }

    /// Supprime des pages, avec ce qui y a été ajouté.
    pub fn remove_pages(&mut self, pages: &[PageIndex]) -> Result<Change, String> {
        let selected = self.selection(pages)?;
        if selected.len() == self.plan.len() {
            return Err("un document garde au moins une page".into());
        }
        let slots = (0..self.plan.len())
            .filter(|p| !selected.contains(p))
            .map(Slot::Keep)
            .collect();
        self.restructure(slots)
    }

    /// Copie chaque page choisie juste après elle ; renvoie la place des copies.
    pub fn duplicate_pages(
        &mut self,
        pages: &[PageIndex],
    ) -> Result<(Vec<PageIndex>, Change), String> {
        let selected = self.selection(pages)?;
        let mut slots = Vec::with_capacity(self.plan.len() + selected.len());
        let mut copies = Vec::with_capacity(selected.len());
        for page in 0..self.plan.len() {
            slots.push(Slot::Keep(page));
            if selected.contains(&page) {
                copies.push(slots.len());
                slots.push(Slot::Copy(page));
            }
        }
        Ok((copies, self.restructure(slots)?))
    }

    /// Déplace les pages choisies, dans leur ordre, devant la page `to`
    /// (`to` = nombre de pages : à la fin) ; renvoie leur nouvelle place.
    pub fn move_pages(
        &mut self,
        pages: &[PageIndex],
        to: PageIndex,
    ) -> Result<(Vec<PageIndex>, Change), String> {
        let selected = self.selection(pages)?;
        let count = self.plan.len();
        if to > count {
            return Err(missing(to));
        }
        let mut slots: Vec<Slot> = (0..count)
            .filter(|p| !selected.contains(p))
            .map(Slot::Keep)
            .collect();
        let at = (0..to).filter(|p| !selected.contains(p)).count();
        slots.splice(at..at, selected.iter().copied().map(Slot::Keep));
        Ok((
            (at..at + selected.len()).collect(),
            self.restructure(slots)?,
        ))
    }

    /// Fait pivoter des pages de `turns` quarts de tour (sens horaire),
    /// leurs ajouts avec elles.
    pub fn rotate_pages(&mut self, pages: &[PageIndex], turns: u8) -> Result<Change, String> {
        let selected = self.selection(pages)?;
        let turns = turns % 4;
        if turns == 0 {
            return Ok(Change::default());
        }
        let sizes = self.page_sizes();
        let mut planned = self.plan.pages().to_vec();
        for &page in &selected {
            planned[page].rotation = (planned[page].rotation + turns) % 4;
        }
        let mut operations = vec![Operation::Pages {
            before: self.plan.clone(),
            after: PagePlan::new(planned),
        }];
        for (&id, element) in &self.elements {
            let page = element.page();
            if !selected.contains(&page) {
                continue;
            }
            let size = sizes.get(page).copied().ok_or_else(|| missing(page))?;
            let after = rotated(element, size, turns).map_err(|_| {
                format!(
                    "la page {} porte du texte ajouté, souligné ou barré, qui ne pivote pas \
                     encore avec elle : faites pivoter la page avant de l'ajouter",
                    page + 1
                )
            })?;
            operations.push(Operation::Update {
                id,
                before: element.clone(),
                after,
            });
        }
        Ok(self.record_batch(operations))
    }

    /// Plan relu d'une sauvegarde automatique, appliqué avant ses éléments.
    pub fn restore_plan(&mut self, plan: PagePlan) -> Result<Change, String> {
        plan.validate(&self.sources)?;
        if plan == self.plan {
            return Ok(Change::default());
        }
        let before = self.plan.clone();
        Ok(self.record_batch(vec![Operation::Pages {
            before,
            after: plan,
        }]))
    }

    /// Pages choisies, sans doublon ; toutes doivent exister.
    fn selection(&self, pages: &[PageIndex]) -> Result<BTreeSet<PageIndex>, String> {
        if pages.is_empty() {
            return Err("aucune page choisie".into());
        }
        let count = self.plan.len();
        match pages.iter().find(|&&p| p >= count) {
            Some(&page) => Err(missing(page)),
            None => Ok(pages.iter().copied().collect()),
        }
    }

    /// Nouveau plan et éléments qui suivent leur page, en une opération ;
    /// rien n'est gardé si l'ordre ne change pas.
    fn restructure(&mut self, slots: Vec<Slot>) -> Result<Change, String> {
        if slots.len() > MAX_PAGES {
            return Err(format!("plus de {MAX_PAGES} pages"));
        }
        let unchanged = slots.len() == self.plan.len()
            && slots
                .iter()
                .enumerate()
                .all(|(at, slot)| matches!(slot, Slot::Keep(page) if *page == at));
        if unchanged {
            return Ok(Change::default());
        }
        let before = self.plan.clone();
        let after = PagePlan::new(
            slots
                .iter()
                .map(|slot| match slot {
                    Slot::Keep(page) | Slot::Copy(page) => before.pages()[*page],
                    Slot::New(page) => *page,
                })
                .collect(),
        );
        let place: HashMap<PageIndex, PageIndex> = slots
            .iter()
            .enumerate()
            .filter_map(|(at, slot)| match slot {
                Slot::Keep(page) => Some((*page, at)),
                _ => None,
            })
            .collect();

        let mut operations = vec![Operation::Pages { before, after }];
        for (&id, element) in &self.elements {
            let page = element.page();
            match place.get(&page) {
                Some(&at) if at == page => {}
                Some(&at) => operations.push(Operation::Update {
                    id,
                    before: element.clone(),
                    after: element.clone().with_page(at),
                }),
                None => operations.push(Operation::Remove {
                    id,
                    element: element.clone(),
                }),
            }
        }
        let copies: Vec<_> = slots
            .iter()
            .enumerate()
            .filter_map(|(at, slot)| match slot {
                Slot::Copy(page) => Some((*page, at)),
                _ => None,
            })
            .flat_map(|(page, at)| {
                self.elements
                    .values()
                    .filter(move |e| e.page() == page)
                    .map(move |e| e.clone().with_page(at))
            })
            .collect();
        for element in copies {
            self.next_id += 1;
            operations.push(Operation::Add {
                id: ElementId(self.next_id),
                element,
            });
        }
        Ok(self.record_batch(operations))
    }
}

fn missing(page: PageIndex) -> String {
    format!("la page {} n'existe pas", page + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{Color, Element, InkBox, PageSource, TextBox, TextStyle};
    use crate::geometry::Point;

    const A4: PageSize = PageSize::new(595.0, 842.0);
    const LANDSCAPE: PageSize = PageSize::new(842.0, 595.0);

    fn text(page: PageIndex, content: &str) -> Element {
        Element::Text(TextBox {
            page,
            top_left: Point::new(72.0, 700.0),
            width: 200.0,
            text: content.into(),
            style: TextStyle::default(),
        })
    }

    fn ink(page: PageIndex) -> Element {
        Element::Ink(InkBox {
            page,
            points: vec![Point::new(10.0, 20.0), Point::new(30.0, 40.0)],
            color: Color::INK,
            width: 2.0,
        })
    }

    /// Document de trois pages (A4, paysage, A4) avec un texte par page.
    fn three_pages() -> Edits {
        let mut edits = Edits::with_pages(vec![A4, LANDSCAPE, A4]);
        for (page, content) in ["a", "b", "c"].iter().enumerate() {
            edits.add(text(page, content));
        }
        edits
    }

    /// Texte et page de chaque élément, dans l'ordre des pages.
    fn contents(edits: &Edits) -> Vec<(PageIndex, String)> {
        let mut contents: Vec<_> = edits
            .elements()
            .filter_map(|(_, e)| match e {
                Element::Text(t) => Some((t.page, t.text.clone())),
                _ => None,
            })
            .collect();
        contents.sort();
        contents
    }

    fn origins(edits: &Edits) -> Vec<Option<PageIndex>> {
        edits
            .plan()
            .pages()
            .iter()
            .map(|p| match p.source {
                PageSource::Original { index } => Some(index),
                PageSource::Blank { .. } | PageSource::Imported { .. } => None,
            })
            .collect()
    }

    fn texts(items: &[(PageIndex, &str)]) -> Vec<(PageIndex, String)> {
        items.iter().map(|(p, t)| (*p, (*t).to_string())).collect()
    }

    #[test]
    fn blank_page_shifts_the_following_ones() {
        let mut edits = three_pages();
        let change = edits.insert_blank(1, A4).unwrap();
        assert!(change.layout);
        assert_eq!(origins(&edits), [Some(0), None, Some(1), Some(2)]);
        assert_eq!(contents(&edits), texts(&[(0, "a"), (2, "b"), (3, "c")]));
        assert_eq!(edits.page_sizes(), [A4, A4, LANDSCAPE, A4]);
        assert!(edits.insert_blank(9, A4).is_err());
        assert!(edits.insert_blank(0, PageSize::new(0.0, 10.0)).is_err());
    }

    #[test]
    fn removed_pages_take_their_elements_and_undo_brings_them_back() {
        let mut edits = three_pages();
        edits.remove_pages(&[0, 2]).unwrap();
        assert_eq!(origins(&edits), [Some(1)]);
        assert_eq!(contents(&edits), texts(&[(0, "b")]));
        assert!(edits.remove_pages(&[0]).is_err(), "dernière page");
        // Une seule opération à annuler.
        let change = edits.undo().unwrap();
        assert!(change.layout);
        assert_eq!(origins(&edits), [Some(0), Some(1), Some(2)]);
        assert_eq!(contents(&edits), texts(&[(0, "a"), (1, "b"), (2, "c")]));
        edits.redo();
        assert_eq!(contents(&edits), texts(&[(0, "b")]));
    }

    #[test]
    fn duplicated_pages_copy_their_elements() {
        let mut edits = three_pages();
        let (copies, _) = edits.duplicate_pages(&[0, 1]).unwrap();
        assert_eq!(copies, [1, 3]);
        assert_eq!(
            origins(&edits),
            [Some(0), Some(0), Some(1), Some(1), Some(2)]
        );
        assert_eq!(
            contents(&edits),
            texts(&[(0, "a"), (1, "a"), (2, "b"), (3, "b"), (4, "c")])
        );
        // Les copies sont de nouveaux éléments.
        assert_eq!(edits.elements().count(), 5);
        edits.undo();
        assert_eq!(edits.elements().count(), 3);
    }

    #[test]
    fn moved_pages_keep_their_order_and_elements() {
        let mut edits = three_pages();
        // Pages 1 et 3 devant la page 2… soit à la place 1.
        let (placed, _) = edits.move_pages(&[2, 0], 1).unwrap();
        assert_eq!(placed, [0, 1]);
        assert_eq!(origins(&edits), [Some(0), Some(2), Some(1)]);
        assert_eq!(contents(&edits), texts(&[(0, "a"), (1, "c"), (2, "b")]));
        // À la fin.
        let (placed, _) = edits.move_pages(&[0], 3).unwrap();
        assert_eq!(placed, [2]);
        assert_eq!(origins(&edits), [Some(2), Some(1), Some(0)]);
        assert!(edits.move_pages(&[0], 4).is_err());
    }

    #[test]
    fn a_move_that_changes_nothing_leaves_no_trace() {
        let mut edits = three_pages();
        let before = edits.revision();
        let (_, change) = edits.move_pages(&[1], 1).unwrap();
        assert!(change.is_empty());
        let (_, change) = edits.move_pages(&[1], 2).unwrap();
        assert!(change.is_empty());
        assert_eq!(edits.revision(), before);
    }

    #[test]
    fn rotation_turns_the_page_and_its_drawings() {
        let mut edits = Edits::with_pages(vec![A4, A4]);
        edits.add(ink(0));
        let change = edits.rotate_pages(&[0], 1).unwrap();
        assert!(change.layout);
        assert_eq!(edits.plan().pages()[0].rotation, 1);
        assert_eq!(edits.page_sizes(), [LANDSCAPE, A4]);
        let (_, Element::Ink(turned)) = edits.elements().next().unwrap() else {
            panic!("tracé attendu");
        };
        assert_eq!(turned.points[0], Point::new(20.0, 585.0));
        // Trois quarts de tour de plus : retour au départ.
        edits.rotate_pages(&[0], 3).unwrap();
        assert_eq!(edits.plan().pages()[0].rotation, 0);
        assert_eq!(edits.elements().next().unwrap().1, &ink(0));
    }

    #[test]
    fn added_text_blocks_the_rotation_of_its_page_only() {
        let mut edits = three_pages();
        let before = edits.revision();
        let error = edits.rotate_pages(&[1], 1).unwrap_err();
        assert!(error.contains("page 2"), "{error}");
        assert_eq!(edits.revision(), before);
        let mut edits = Edits::with_pages(vec![A4, A4]);
        edits.add(text(1, "a"));
        assert!(edits.rotate_pages(&[0], 2).is_ok());
    }

    #[test]
    fn invalid_selections_are_refused() {
        let mut edits = three_pages();
        assert!(edits.remove_pages(&[]).is_err());
        assert!(edits.duplicate_pages(&[3]).is_err());
        assert!(edits.rotate_pages(&[5], 1).is_err());
    }

    #[test]
    fn restored_plan_is_checked() {
        let mut edits = Edits::with_pages(vec![A4, A4]);
        let plan = PagePlan::new(vec![PlannedPage::original(1)]);
        assert!(edits.restore_plan(plan.clone()).unwrap().layout);
        assert_eq!(edits.plan(), &plan);
        let invalid = PagePlan::new(vec![PlannedPage::original(4)]);
        assert!(edits.restore_plan(invalid).is_err());
        assert!(edits.restore_plan(plan).unwrap().is_empty());
    }

    #[test]
    fn pages_of_another_pdf_are_inserted_in_one_operation() {
        let mut edits = three_pages();
        edits.register_import(ImportId(1), vec![LANDSCAPE, A4]);
        // Fusionner : toutes les pages du PDF, à la fin.
        let change = edits
            .insert_imported(3, &[(ImportId(1), 0), (ImportId(1), 1)])
            .unwrap();
        assert!(change.layout);
        assert_eq!(edits.page_sizes(), [A4, LANDSCAPE, A4, LANDSCAPE, A4]);
        assert_eq!(
            edits.plan().pages()[3],
            PlannedPage::imported(ImportId(1), 0)
        );
        // En tête : les éléments des pages suivantes sont décalés.
        edits.insert_imported(0, &[(ImportId(1), 1)]).unwrap();
        assert_eq!(contents(&edits), texts(&[(1, "a"), (2, "b"), (3, "c")]));
        // Une seule annulation par insertion.
        edits.undo();
        edits.undo();
        assert_eq!(edits.plan().len(), 3);
        assert!(edits.insert_imported(0, &[(ImportId(2), 0)]).is_err());
        assert!(edits.insert_imported(0, &[(ImportId(1), 5)]).is_err());
        assert!(edits.insert_imported(9, &[(ImportId(1), 0)]).is_err());
        assert!(edits.insert_imported(0, &[]).is_err());
    }
}
