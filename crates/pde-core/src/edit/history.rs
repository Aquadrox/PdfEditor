//! Éléments ajoutés, plan des pages et historique des opérations
//! (annuler, rétablir), avec le suivi « modifié depuis le dernier
//! enregistrement ». Les opérations sur les pages sont dans `page_ops.rs`.

use std::collections::BTreeMap;

use crate::document::{PageIndex, PageSize};

use super::element::{Element, ElementId};
use super::form::{FieldValue, Metadata};
use super::import::{ImportId, PageSources};
use super::pages::PagePlan;

/// Opération sur les éléments ; chacune connaît son inverse.
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Add {
        id: ElementId,
        element: Element,
    },
    Update {
        id: ElementId,
        before: Element,
        after: Element,
    },
    Remove {
        id: ElementId,
        element: Element,
    },
    /// Plusieurs opérations faites ensemble (sélection multiple) : une
    /// seule entrée de l'historique, annulée d'un coup.
    Batch(Vec<Operation>),
    /// Nouveau plan des pages (ordre, pages ajoutées, rotations).
    Pages {
        before: PagePlan,
        after: PagePlan,
    },
    /// Valeur d'un champ du formulaire (`None` : celle de l'original).
    Field {
        name: String,
        before: Option<FieldValue>,
        after: Option<FieldValue>,
    },
    /// Propriétés du document (`None` : celles de l'original).
    Metadata {
        before: Option<Metadata>,
        after: Option<Metadata>,
    },
    /// Formulaire aplati (champs fondus dans les pages).
    Flatten {
        before: bool,
        after: bool,
    },
}

impl Operation {
    fn inverse(&self) -> Operation {
        match self.clone() {
            Operation::Add { id, element } => Operation::Remove { id, element },
            Operation::Remove { id, element } => Operation::Add { id, element },
            Operation::Update { id, before, after } => Operation::Update {
                id,
                before: after,
                after: before,
            },
            // Défaites dans l'ordre inverse.
            Operation::Batch(operations) => {
                Operation::Batch(operations.iter().rev().map(Operation::inverse).collect())
            }
            Operation::Pages { before, after } => Operation::Pages {
                before: after,
                after: before,
            },
            Operation::Field {
                name,
                before,
                after,
            } => Operation::Field {
                name,
                before: after,
                after: before,
            },
            Operation::Metadata { before, after } => Operation::Metadata {
                before: after,
                after: before,
            },
            Operation::Flatten { before, after } => Operation::Flatten {
                before: after,
                after: before,
            },
        }
    }

    /// L'opération touche le document entier (formulaire, propriétés) :
    /// toutes les pages sont à redessiner.
    fn touches_document(&self) -> bool {
        match self {
            Operation::Field { .. } | Operation::Metadata { .. } | Operation::Flatten { .. } => {
                true
            }
            Operation::Batch(operations) => operations.iter().any(Operation::touches_document),
            _ => false,
        }
    }

    /// L'opération change le plan des pages.
    fn changes_layout(&self) -> bool {
        match self {
            Operation::Pages { .. } => true,
            Operation::Batch(operations) => operations.iter().any(Operation::changes_layout),
            _ => false,
        }
    }

    /// Pages dont l'affichage change (deux pour un élément changé de page).
    fn pages(&self) -> Vec<PageIndex> {
        let mut pages = match self {
            Operation::Add { element, .. } | Operation::Remove { element, .. } => {
                vec![element.page()]
            }
            Operation::Update { before, after, .. } => vec![before.page(), after.page()],
            Operation::Batch(operations) => operations.iter().flat_map(Operation::pages).collect(),
            // Toutes les pages changent : voir `Change::layout` et `Change::all_pages`.
            Operation::Pages { .. }
            | Operation::Field { .. }
            | Operation::Metadata { .. }
            | Operation::Flatten { .. } => Vec::new(),
        };
        pages.sort_unstable();
        pages.dedup();
        pages
    }
}

/// Effet d'une opération : pages à redessiner.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Change {
    pub pages: Vec<PageIndex>,
    /// Le plan des pages a changé (nombre, ordre, tailles) : tout le
    /// document est à redessiner.
    pub layout: bool,
    /// Formulaire ou propriétés changés : toutes les pages sont à
    /// redessiner, leur nombre et leur taille ne changent pas.
    pub all_pages: bool,
}

impl Change {
    /// L'opération n'a rien changé.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty() && !self.layout && !self.all_pages
    }
}

/// Éléments du document, plan des pages et historique.
#[derive(Debug, Clone, Default)]
pub struct Edits {
    pub(super) elements: BTreeMap<ElementId, Element>,
    pub(super) next_id: u64,
    /// Taille affichée des pages de l'original et des PDF insérés.
    pub(super) sources: PageSources,
    pub(super) plan: PagePlan,
    /// Valeurs saisies dans les champs, par nom complet.
    pub(super) fields: BTreeMap<String, FieldValue>,
    pub(super) metadata: Option<Metadata>,
    pub(super) flatten: bool,
    /// Opérations faites, chacune avec son numéro (pour l'état enregistré).
    done: Vec<(u64, Operation)>,
    undone: Vec<(u64, Operation)>,
    next_seq: u64,
    /// Numéro de la dernière opération au moment de l'enregistrement.
    saved: u64,
}

impl Edits {
    /// Document sans page connue (tests des éléments seuls).
    pub fn new() -> Self {
        Self::default()
    }

    /// Document dont les pages d'origine ont ces tailles affichées.
    pub fn with_pages(originals: Vec<PageSize>) -> Self {
        Self {
            plan: PagePlan::identity(originals.len()),
            sources: PageSources::new(originals),
            ..Self::default()
        }
    }

    /// PDF inséré : ses pages peuvent entrer dans le plan. Il reste connu
    /// toute la session (annuler, rétablir), comme les images.
    pub fn register_import(&mut self, id: ImportId, page_sizes: Vec<PageSize>) {
        self.sources.imported.insert(id, page_sizes);
    }

    /// Pages que le plan peut désigner.
    pub fn sources(&self) -> &PageSources {
        &self.sources
    }

    pub fn plan(&self) -> &PagePlan {
        &self.plan
    }

    /// Taille affichée de chaque page du document arrangé.
    pub fn page_sizes(&self) -> Vec<PageSize> {
        self.plan.sizes(&self.sources)
    }

    /// Taille d'origine de chaque page de l'original.
    pub fn original_sizes(&self) -> &[PageSize] {
        &self.sources.originals
    }

    pub fn elements(&self) -> impl Iterator<Item = (ElementId, &Element)> {
        self.elements.iter().map(|(id, e)| (*id, e))
    }

    pub fn get(&self, id: ElementId) -> Option<&Element> {
        self.elements.get(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// Le document diffère de son dernier enregistrement (ou de l'ouverture).
    pub fn is_dirty(&self) -> bool {
        self.current_seq() != self.saved
    }

    /// Le document vient d'être enregistré dans son état actuel.
    pub fn mark_saved(&mut self) {
        self.saved = self.current_seq();
    }

    /// Version de la liste d'éléments : change à chaque opération, annuler
    /// et rétablir compris (sauvegarde automatique : « a-t-elle changé ? »).
    pub fn revision(&self) -> u64 {
        self.current_seq()
    }

    fn current_seq(&self) -> u64 {
        self.done.last().map_or(0, |(seq, _)| *seq)
    }

    pub fn add(&mut self, element: Element) -> (ElementId, Change) {
        self.next_id += 1;
        let id = ElementId(self.next_id);
        let change = self.record(Operation::Add { id, element });
        (id, change)
    }

    /// Remplace un élément ; sans effet (ni historique) s'il est identique.
    pub fn update(&mut self, id: ElementId, element: Element) -> Result<Change, String> {
        let before = self.get(id).ok_or_else(|| unknown(id))?.clone();
        if before == element {
            return Ok(Change::default());
        }
        Ok(self.record(Operation::Update {
            id,
            before,
            after: element,
        }))
    }

    /// Ajoute plusieurs éléments en une opération (dupliquer une sélection).
    pub fn add_many(&mut self, elements: Vec<Element>) -> (Vec<ElementId>, Change) {
        let mut ids = Vec::with_capacity(elements.len());
        let operations = elements
            .into_iter()
            .map(|element| {
                self.next_id += 1;
                let id = ElementId(self.next_id);
                ids.push(id);
                Operation::Add { id, element }
            })
            .collect();
        (ids, self.record_batch(operations))
    }

    /// Remplace plusieurs éléments en une opération ; les éléments
    /// inchangés sont ignorés, et rien n'est gardé si aucun ne change.
    pub fn update_many(&mut self, changes: Vec<(ElementId, Element)>) -> Result<Change, String> {
        let mut operations = Vec::new();
        for (id, after) in changes {
            let before = self.get(id).ok_or_else(|| unknown(id))?.clone();
            if before != after {
                operations.push(Operation::Update { id, before, after });
            }
        }
        Ok(self.record_batch(operations))
    }

    /// Ajoute, remplace et retire des éléments en une seule opération
    /// (remplacer une image d'origine : la supprimer et poser la nouvelle ;
    /// supprimer une sélection qui mêle images d'origine et ajouts).
    pub fn add_and_update(
        &mut self,
        added: Vec<Element>,
        updated: Vec<(ElementId, Element)>,
        removed: &[ElementId],
    ) -> Result<(Vec<ElementId>, Change), String> {
        let mut operations = Vec::new();
        for &id in removed {
            let element = self.get(id).ok_or_else(|| unknown(id))?.clone();
            operations.push(Operation::Remove { id, element });
        }
        for (id, after) in updated {
            let before = self.get(id).ok_or_else(|| unknown(id))?.clone();
            if before != after {
                operations.push(Operation::Update { id, before, after });
            }
        }
        let mut ids = Vec::with_capacity(added.len());
        for element in added {
            self.next_id += 1;
            let id = ElementId(self.next_id);
            ids.push(id);
            operations.push(Operation::Add { id, element });
        }
        Ok((ids, self.record_batch(operations)))
    }

    /// Retire plusieurs éléments en une opération.
    pub fn remove_many(&mut self, ids: &[ElementId]) -> Result<Change, String> {
        let operations = ids
            .iter()
            .map(|&id| {
                let element = self.get(id).ok_or_else(|| unknown(id))?.clone();
                Ok(Operation::Remove { id, element })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(self.record_batch(operations))
    }

    /// Une opération seule reste telle quelle ; aucune : rien n'est gardé.
    pub(super) fn record_batch(&mut self, mut operations: Vec<Operation>) -> Change {
        match operations.len() {
            0 => Change::default(),
            1 => self.record(operations.remove(0)),
            _ => self.record(Operation::Batch(operations)),
        }
    }

    pub fn remove(&mut self, id: ElementId) -> Result<Change, String> {
        let element = self.get(id).ok_or_else(|| unknown(id))?.clone();
        Ok(self.record(Operation::Remove { id, element }))
    }

    /// Fait plusieurs opérations comme une seule (actions acceptées de
    /// l'assistant) : une entrée de l'historique, annulée d'un coup. En cas
    /// d'échec de l'une d'elles, toutes sont défaites et l'historique reste
    /// tel qu'avant.
    pub fn grouped<T>(
        &mut self,
        operations: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<(T, Change), String> {
        let mark = self.done.len();
        let undone = self.undone.clone();
        match operations(self) {
            Ok(value) => {
                let done: Vec<Operation> = self.done.drain(mark..).map(|(_, op)| op).collect();
                let operation = match done.len() {
                    0 => return Ok((value, Change::default())),
                    1 => done
                        .into_iter()
                        .next()
                        .unwrap_or(Operation::Batch(Vec::new())),
                    _ => Operation::Batch(done),
                };
                let change = Change {
                    pages: operation.pages(),
                    layout: operation.changes_layout(),
                    all_pages: operation.touches_document(),
                };
                // Déjà appliquées une à une : on ne garde que leur trace commune.
                self.next_seq += 1;
                self.done.push((self.next_seq, operation));
                Ok((value, change))
            }
            Err(error) => {
                while self.done.len() > mark {
                    self.rollback();
                }
                self.undone = undone;
                Err(error)
            }
        }
    }

    /// Annule la dernière opération ; `None` s'il n'y en a pas.
    pub fn undo(&mut self) -> Option<Change> {
        let (seq, operation) = self.done.pop()?;
        let change = self.apply(&operation.inverse());
        self.undone.push((seq, operation));
        Some(change)
    }

    /// Refait la dernière opération annulée ; `None` s'il n'y en a pas.
    pub fn redo(&mut self) -> Option<Change> {
        let (seq, operation) = self.undone.pop()?;
        let change = self.apply(&operation);
        self.done.push((seq, operation));
        Some(change)
    }

    /// Défait la dernière opération et l'oublie (elle n'a pas pu être
    /// appliquée au document) : ni « rétablir », ni trace dans l'historique.
    pub fn rollback(&mut self) -> Option<Change> {
        let (_, operation) = self.done.pop()?;
        Some(self.apply(&operation.inverse()))
    }

    /// Nouvelle opération : appliquée, gardée, et les opérations annulées
    /// ne peuvent plus être refaites.
    fn record(&mut self, operation: Operation) -> Change {
        let change = self.apply(&operation);
        self.next_seq += 1;
        self.done.push((self.next_seq, operation));
        self.undone.clear();
        change
    }

    fn apply(&mut self, operation: &Operation) -> Change {
        match operation.clone() {
            Operation::Add { id, element } => {
                self.elements.insert(id, element);
            }
            Operation::Update { id, after, .. } => {
                self.elements.insert(id, after);
            }
            Operation::Remove { id, .. } => {
                self.elements.remove(&id);
            }
            Operation::Batch(operations) => {
                for operation in &operations {
                    self.apply(operation);
                }
            }
            Operation::Pages { after, .. } => self.plan = after,
            Operation::Field { name, after, .. } => match after {
                Some(value) => {
                    self.fields.insert(name, value);
                }
                None => {
                    self.fields.remove(&name);
                }
            },
            Operation::Metadata { after, .. } => self.metadata = after,
            Operation::Flatten { after, .. } => self.flatten = after,
        }
        Change {
            pages: operation.pages(),
            layout: operation.changes_layout(),
            all_pages: operation.touches_document(),
        }
    }
}

fn unknown(id: ElementId) -> String {
    format!("élément {} introuvable", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{TextBox, TextStyle};
    use crate::geometry::Point;

    fn text(page: PageIndex, content: &str) -> Element {
        Element::Text(TextBox {
            page,
            top_left: Point::new(72.0, 700.0),
            width: 200.0,
            text: content.into(),
            style: TextStyle::default(),
        })
    }

    fn texts(edits: &Edits) -> Vec<String> {
        edits
            .elements()
            .map(|(_, e)| match e {
                Element::Text(t) => t.text.clone(),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn grouped_operations_are_undone_at_once() {
        let mut edits = Edits::with_pages(vec![PageSize::new(595.0, 842.0); 2]);
        edits.add(text(0, "avant"));
        let (ids, change) = edits
            .grouped(|e| {
                let (a, _) = e.add(text(0, "a"));
                let (b, _) = e.add(text(0, "b"));
                e.rotate_pages(&[1], 1)?;
                Ok([a, b])
            })
            .unwrap();
        assert_eq!(ids.len(), 2);
        assert!(change.layout && change.pages.contains(&0), "{change:?}");
        assert_eq!(texts(&edits), ["avant", "a", "b"]);
        assert_eq!(edits.plan().pages()[1].rotation, 1);

        edits.undo().unwrap();
        assert_eq!(texts(&edits), ["avant"]);
        assert_eq!(edits.plan().pages()[1].rotation, 0);
        edits.redo().unwrap();
        assert_eq!(texts(&edits), ["avant", "a", "b"]);
        edits.undo().unwrap();
        edits.undo().unwrap();
        assert!(
            !edits.can_undo(),
            "le groupe ne compte que pour une opération"
        );
    }

    #[test]
    fn failed_group_leaves_nothing_behind() {
        let mut edits = Edits::with_pages(vec![PageSize::new(595.0, 842.0)]);
        edits.add(text(0, "garde"));
        edits.undo();
        let failed = edits.grouped(|e| {
            e.add(text(0, "a"));
            e.rotate_pages(&[5], 1)
        });
        assert!(failed.is_err());
        assert!(texts(&edits).is_empty() && !edits.can_undo());
        assert!(edits.can_redo(), "« rétablir » reste possible");
        assert!(!edits.is_dirty());
    }

    #[test]
    fn add_update_remove_with_pages_to_redraw() {
        let mut edits = Edits::new();
        let (a, change) = edits.add(text(0, "a"));
        assert_eq!(change.pages, [0]);
        // Déplacé sur une autre page : les deux pages changent.
        assert_eq!(edits.update(a, text(2, "a")).unwrap().pages, [0, 2]);
        assert_eq!(edits.remove(a).unwrap().pages, [2]);
        assert!(edits.is_empty());
        assert!(edits.remove(a).is_err());
    }

    #[test]
    fn undo_and_redo_restore_each_state() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "un"));
        edits.update(a, text(0, "deux")).unwrap();
        let (_, _) = edits.add(text(1, "trois"));
        assert_eq!(texts(&edits), ["deux", "trois"]);

        edits.undo();
        assert_eq!(texts(&edits), ["deux"]);
        edits.undo();
        assert_eq!(texts(&edits), ["un"]);
        edits.undo();
        assert!(edits.is_empty());
        assert_eq!(edits.undo(), None);

        edits.redo();
        edits.redo();
        assert_eq!(texts(&edits), ["deux"]);
        edits.redo();
        assert_eq!(texts(&edits), ["deux", "trois"]);
        assert_eq!(edits.redo(), None);
    }

    #[test]
    fn new_operation_clears_redo() {
        let mut edits = Edits::new();
        edits.add(text(0, "a"));
        edits.undo();
        assert!(edits.can_redo());
        edits.add(text(0, "b"));
        assert!(!edits.can_redo());
        assert_eq!(texts(&edits), ["b"]);
    }

    #[test]
    fn identical_update_is_not_recorded() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        assert_eq!(edits.update(a, text(0, "a")).unwrap(), Change::default());
        edits.undo();
        assert!(edits.is_empty(), "un seul « annuler » suffit");
    }

    #[test]
    fn dirty_follows_the_saved_state() {
        let mut edits = Edits::new();
        assert!(!edits.is_dirty());
        let (a, _) = edits.add(text(0, "a"));
        assert!(edits.is_dirty());
        edits.mark_saved();
        assert!(!edits.is_dirty());
        edits.update(a, text(0, "b")).unwrap();
        assert!(edits.is_dirty());
        edits.undo();
        assert!(!edits.is_dirty(), "retour à l'état enregistré");
        // Même nombre d'opérations, mais une autre : modifié.
        edits.update(a, text(0, "c")).unwrap();
        assert!(edits.is_dirty());
    }

    #[test]
    fn rollback_forgets_the_failed_operation() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        edits.update(a, text(0, "b")).unwrap();
        assert_eq!(edits.rollback().unwrap().pages, [0]);
        assert_eq!(texts(&edits), ["a"]);
        assert!(!edits.can_redo());
        edits.undo();
        assert!(edits.is_empty());
    }

    #[test]
    fn ids_are_never_reused() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        edits.undo();
        let (b, _) = edits.add(text(0, "b"));
        assert_ne!(a, b);
    }

    #[test]
    fn added_and_updated_together_is_one_entry() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        let (ids, change) = edits
            .add_and_update(vec![text(1, "b")], vec![(a, text(0, "a2"))], &[])
            .unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(change.pages, [0, 1]);
        assert_eq!(texts(&edits), ["a2", "b"]);
        edits.undo();
        assert_eq!(texts(&edits), ["a"]);
        assert!(
            edits
                .add_and_update(Vec::new(), vec![(ElementId(99), text(0, "x"))], &[])
                .is_err()
        );
        let (c, _) = edits.add(text(2, "c"));
        edits
            .add_and_update(Vec::new(), Vec::new(), &[a, c])
            .unwrap();
        assert!(edits.is_empty());
        edits.undo();
        assert_eq!(texts(&edits), ["a", "c"]);
    }

    #[test]
    fn group_operations_are_one_history_entry() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        let (b, _) = edits.add(text(2, "b"));
        let change = edits
            .update_many(vec![(a, text(0, "A")), (b, text(2, "B"))])
            .unwrap();
        assert_eq!(change.pages, [0, 2]);
        assert_eq!(texts(&edits), ["A", "B"]);
        // Une seule annulation défait tout le groupe.
        edits.undo();
        assert_eq!(texts(&edits), ["a", "b"]);
        edits.redo();
        assert_eq!(texts(&edits), ["A", "B"]);

        let (copies, change) = edits.add_many(vec![text(1, "c"), text(1, "d")]);
        assert_eq!((copies.len(), change.pages.as_slice()), (2, &[1][..]));
        assert_eq!(edits.remove_many(&[a, copies[1]]).unwrap().pages, [0, 1]);
        assert_eq!(texts(&edits), ["B", "c"]);
        edits.undo();
        assert_eq!(texts(&edits), ["A", "B", "c", "d"]);
        // Rollback d'un groupe (pose impossible) : tout est défait.
        edits
            .update_many(vec![(a, text(0, "x")), (b, text(2, "y"))])
            .unwrap();
        edits.rollback();
        assert_eq!(texts(&edits), ["A", "B", "c", "d"]);
    }

    #[test]
    fn unchanged_or_empty_groups_leave_no_trace() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        let revision = edits.revision();
        assert!(
            edits
                .update_many(vec![(a, text(0, "a"))])
                .unwrap()
                .pages
                .is_empty()
        );
        assert!(edits.remove_many(&[]).unwrap().pages.is_empty());
        assert_eq!(edits.revision(), revision);
        // Élément inconnu : refusé, rien n'est changé.
        assert!(
            edits
                .update_many(vec![(a, text(0, "z")), (ElementId(99), text(0, "?"))])
                .is_err()
        );
        assert!(edits.remove_many(&[a, ElementId(99)]).is_err());
        assert_eq!(texts(&edits), ["a"]);
    }
}
