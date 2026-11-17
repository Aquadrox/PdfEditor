//! Pages (P-01, P-02) : insérer une page blanche, supprimer, dupliquer,
//! déplacer, pivoter. Chaque opération change le plan des pages ; le
//! document affiché est alors décrit à nouveau (nombre, ordre, taille des
//! pages, signets).

use pde_core::document::{DocumentId, DocumentInfo, OutlineItem, PageIndex, PageSize};
use pde_core::edit::{Change, Edits, PagePlan, PageSources};
use serde::Serialize;

use super::edit::{invalid, view};
use super::{OpenDocument, Viewer, lock};
use crate::edit_view::EditView;
use crate::error::AppError;
use crate::view::DocumentView;

/// Taille d'une page blanche ajoutée à un document vide de référence.
const A4: PageSize = PageSize::new(595.0, 842.0);

/// Résultat d'une opération sur les pages : le document décrit à nouveau,
/// l'état de l'édition, et les pages à sélectionner ensuite.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PagesView {
    pub document: DocumentView,
    pub edit: EditView,
    pub selected: Vec<PageIndex>,
}

impl Viewer {
    /// Description du document tel qu'affiché (plan des pages appliqué).
    pub fn document_view(&self, id: u64) -> Result<DocumentView, AppError> {
        self.with_document(id, |open, doc| Ok(document_view(open, doc)))
    }

    /// Page blanche à la place `at`, de la taille de la page qui la précède
    /// (de la première page si elle est insérée en tête).
    pub fn insert_blank_page(&self, id: u64, at: PageIndex) -> Result<PagesView, AppError> {
        self.page_operation(id, |edits| {
            let sizes = edits.page_sizes();
            let size = sizes
                .get(at.saturating_sub(1))
                .or(sizes.first())
                .copied()
                .unwrap_or(A4);
            Ok((vec![at], edits.insert_blank(at, size)?))
        })
    }

    pub fn remove_pages(&self, id: u64, pages: &[PageIndex]) -> Result<PagesView, AppError> {
        self.page_operation(id, |edits| {
            let change = edits.remove_pages(pages)?;
            // La page qui prend la place de la première supprimée.
            let first = pages.iter().min().copied().unwrap_or(0);
            let next = first.min(edits.plan().len().saturating_sub(1));
            Ok((vec![next], change))
        })
    }

    pub fn duplicate_pages(&self, id: u64, pages: &[PageIndex]) -> Result<PagesView, AppError> {
        self.page_operation(id, |edits| edits.duplicate_pages(pages))
    }

    /// Déplace les pages devant la page `to` (`to` = nombre de pages : à la fin).
    pub fn move_pages(
        &self,
        id: u64,
        pages: &[PageIndex],
        to: PageIndex,
    ) -> Result<PagesView, AppError> {
        self.page_operation(id, |edits| edits.move_pages(pages, to))
    }

    /// Fait pivoter les pages de `turns` quarts de tour, sens horaire
    /// (3 : un quart de tour dans l'autre sens).
    pub fn rotate_pages(
        &self,
        id: u64,
        pages: &[PageIndex],
        turns: u8,
    ) -> Result<PagesView, AppError> {
        self.page_operation(id, |edits| {
            Ok((pages.to_vec(), edits.rotate_pages(pages, turns)?))
        })
    }

    /// Applique une opération sur les pages ; en cas d'échec du moteur,
    /// elle est oubliée et le document reste tel qu'avant.
    fn page_operation(
        &self,
        id: u64,
        operation: impl FnOnce(&mut Edits) -> Result<(Vec<PageIndex>, Change), String>,
    ) -> Result<PagesView, AppError> {
        self.with_document(id, |open, doc| {
            let (selected, change) = operation(&mut open.edits).map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(PagesView {
                document: document_view(open, doc),
                edit: view(open),
                selected,
            })
        })
    }

    /// Le plan des pages a changé : le document est décrit à nouveau et
    /// toutes ses pages sont à redessiner.
    pub(super) fn relayout(&self, open: &mut OpenDocument, doc: DocumentId) {
        open.info = arranged(&open.original, open.edits.plan(), open.edits.sources());
        open.layout += 1;
        open.page_images.clear();
        open.page_text_blocks.clear();
        open.form_fields = None;
        let next = open.revisions.iter().max().map_or(1, |r| r + 1);
        open.revisions = vec![next; open.info.page_count()];
        lock(&self.cache).remove_document(doc);
        lock(&self.texts).retain(|(d, _), _| *d != doc);
    }
}

pub(super) fn document_view(open: &OpenDocument, doc: DocumentId) -> DocumentView {
    let file_name = open
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| open.path.display().to_string());
    DocumentView::new(doc.0, file_name, &open.info)
}

/// Le document d'origine décrit selon le plan : tailles des pages, et
/// signets qui suivent leur page (sans page si elle a été supprimée).
pub(super) fn arranged(
    original: &DocumentInfo,
    plan: &PagePlan,
    sources: &PageSources,
) -> DocumentInfo {
    fn outline(items: &[OutlineItem], plan: &PagePlan) -> Vec<OutlineItem> {
        items
            .iter()
            .map(|item| OutlineItem {
                title: item.title.clone(),
                page: item.page.and_then(|p| plan.position_of(p)),
                children: outline(&item.children, plan),
            })
            .collect()
    }
    DocumentInfo {
        page_sizes: plan.sizes(sources),
        title: original.title.clone(),
        outline: outline(&original.outline, plan),
        metadata: original.metadata.clone(),
    }
}

#[cfg(test)]
mod tests {
    use pde_core::edit::{Element, InkBox, PlannedPage};
    use pde_core::geometry::Point;

    use super::super::testing::Session;
    use super::*;
    use crate::error::ErrorKind;
    use crate::fake::pdf_with_pages;

    fn ink(page: PageIndex) -> Element {
        Element::Ink(InkBox {
            page,
            points: vec![Point::new(10.0, 20.0), Point::new(30.0, 40.0)],
            color: pde_core::edit::Color::INK,
            width: 2.0,
        })
    }

    #[test]
    fn blank_page_takes_the_size_of_the_previous_one() {
        let s = Session::new("pages-blank");
        let view = s.viewer.insert_blank_page(s.id, 2).unwrap();
        assert_eq!(view.document.pages.len(), 3);
        assert_eq!(view.document.pages[2].width, 595.0);
        assert_eq!(view.selected, [2]);
        assert_eq!(view.edit.layout, 1);
        assert!(view.edit.dirty && view.edit.can_undo);
        assert_eq!(
            s.viewer.insert_blank_page(s.id, 9).unwrap_err().kind,
            ErrorKind::InvalidRequest
        );
    }

    #[test]
    fn every_page_is_rendered_again_after_a_change_of_layout() {
        let s = Session::new("pages-render");
        s.viewer.page_png(s.id, 0, 1.0).unwrap();
        let before = s.viewer.edit_view(s.id).unwrap().revisions;
        let view = s.viewer.rotate_pages(s.id, &[1], 1).unwrap();
        assert!(
            view.edit
                .revisions
                .iter()
                .all(|r| before.iter().all(|b| r > b))
        );
        // Page tournée : paysage.
        assert_eq!(view.document.pages[1].width, 842.0);
        s.viewer.page_png(s.id, 0, 1.0).unwrap();
        assert_eq!(s.engine.render_count(), 2, "cache oublié");
    }

    #[test]
    fn elements_follow_their_page_and_undo_restores_everything() {
        let s = Session::new("pages-follow");
        s.viewer.add_element(s.id, ink(1)).unwrap();
        let view = s.viewer.move_pages(s.id, &[1], 0).unwrap();
        assert_eq!(view.selected, [0]);
        assert_eq!(view.edit.elements[0].element.page(), 0);
        let view = s.viewer.remove_pages(s.id, &[0]).unwrap();
        assert!(view.edit.elements.is_empty());
        assert_eq!(view.document.pages.len(), 1);
        assert_eq!(view.selected, [0]);
        // Deux annulations : la suppression, puis le déplacement.
        s.viewer.undo(s.id).unwrap();
        let edit = s.viewer.undo(s.id).unwrap();
        assert_eq!(edit.elements[0].element.page(), 1);
        assert_eq!(s.viewer.document_view(s.id).unwrap().pages.len(), 2);
        assert_eq!(edit.layout, 4);
    }

    #[test]
    fn duplicated_pages_are_saved_and_checked() {
        let s = Session::new("pages-save");
        let view = s.viewer.duplicate_pages(s.id, &[0, 1]).unwrap();
        assert_eq!(view.selected, [1, 3]);
        s.viewer.save(s.id, None).unwrap();
        assert_eq!(
            std::fs::read(s.dir.join("contrat.pdf")).unwrap(),
            pdf_with_pages(4)
        );
        assert!(!s.viewer.edit_view(s.id).unwrap().dirty);
    }

    #[test]
    fn refused_operations_leave_no_trace() {
        let s = Session::new("pages-refused");
        let error = s.viewer.remove_pages(s.id, &[0, 1]).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidRequest);
        assert!(s.viewer.rotate_pages(s.id, &[4], 1).is_err());
        let edit = s.viewer.edit_view(s.id).unwrap();
        assert!(!edit.can_undo && edit.layout == 0);
    }

    #[test]
    fn bookmarks_follow_their_page() {
        let original = DocumentInfo {
            page_sizes: vec![A4, PageSize::new(842.0, 595.0)],
            title: Some("Contrat".into()),
            outline: vec![OutlineItem {
                title: "Annexe".into(),
                page: Some(1),
                children: vec![OutlineItem {
                    title: "Début".into(),
                    page: Some(0),
                    children: Vec::new(),
                }],
            }],
            metadata: Default::default(),
        };
        let plan = PagePlan::new(vec![PlannedPage::original(1)]);
        let sources = PageSources::new(original.page_sizes.clone());
        let info = arranged(&original, &plan, &sources);
        assert_eq!(info.page_sizes, [PageSize::new(842.0, 595.0)]);
        assert_eq!(info.title.as_deref(), Some("Contrat"));
        assert_eq!(info.outline[0].page, Some(0));
        assert_eq!(info.outline[0].children[0].page, None);
    }
}
