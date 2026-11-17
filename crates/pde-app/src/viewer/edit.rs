//! Édition : éléments ajoutés, annuler / rétablir, enregistrement.
//!
//! À chaque opération, le moteur repart du document d'origine et y pose
//! tous les éléments ; les pages touchées changent de version et leurs
//! images et textes en cache sont oubliés.

use std::path::{Path, PathBuf};

use pde_core::document::DocumentId;
use pde_core::edit::{Change, Element, ElementId};
use pde_core::engine::Edition;

use super::{OpenDocument, Viewer, lock, unknown_document};
use crate::edit_view::{EditView, ElementView, SavedView, flip};
use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;

pub(super) fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(ErrorKind::InvalidRequest, message)
}

impl Viewer {
    /// Exécute `f` sur le document ouvert `id`, documents verrouillés.
    pub(super) fn with_document<T>(
        &self,
        id: u64,
        f: impl FnOnce(&mut OpenDocument, DocumentId) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let doc = DocumentId(id);
        let mut documents = lock(&self.documents);
        let open = documents.get_mut(&doc).ok_or_else(unknown_document)?;
        f(open, doc)
    }

    /// Un document ouvert a des modifications non enregistrées.
    pub fn any_dirty(&self) -> bool {
        lock(&self.documents).values().any(|d| d.edits.is_dirty())
    }

    pub fn edit_view(&self, id: u64) -> Result<EditView, AppError> {
        self.with_document(id, |open, _| Ok(view(open)))
    }

    /// Ajoute un élément (coordonnées avec l'origine en haut) ; renvoie son
    /// identifiant et l'état de l'édition.
    pub fn add_element(&self, id: u64, element: Element) -> Result<(u64, EditView), AppError> {
        self.with_document(id, |open, doc| {
            let element = self.checked(open, &element)?;
            let (element_id, change) = open.edits.add(element);
            self.commit(open, doc, change)?;
            Ok((element_id.0, view(open)))
        })
    }

    pub fn update_element(
        &self,
        id: u64,
        element_id: u64,
        element: Element,
    ) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            let element = self.checked(open, &element)?;
            let change = open
                .edits
                .update(ElementId(element_id), element)
                .map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(view(open))
        })
    }

    pub fn remove_element(&self, id: u64, element_id: u64) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            let change = open.edits.remove(ElementId(element_id)).map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(view(open))
        })
    }

    pub fn undo(&self, id: u64) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            if let Some(change) = open.edits.undo() {
                self.redraw(open, doc, change)?;
            }
            Ok(view(open))
        })
    }

    pub fn redo(&self, id: u64) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            if let Some(change) = open.edits.redo() {
                self.redraw(open, doc, change)?;
            }
            Ok(view(open))
        })
    }

    /// Enregistre le document dans son état actuel, sous `target` ou à sa
    /// place. Le contenu est relu avant d'écrire, puis écrit de façon
    /// atomique : en cas d'échec, le fichier existant reste intact.
    pub fn save(&self, id: u64, target: Option<PathBuf>) -> Result<SavedView, AppError> {
        self.with_document(id, |open, doc| {
            let target = target.unwrap_or_else(|| open.path.clone());
            let bytes = self.engine.save(doc)?;
            self.verify(&bytes, open)?;
            write_atomic(&target, &bytes)?;
            log::info!("document enregistré : {} Ko", bytes.len() / 1024);
            open.edits.mark_saved();
            open.path.clone_from(&target);
            Ok(SavedView {
                file_name: file_name(&target),
                path: target.display().to_string(),
            })
        })
    }

    /// Élément venu de l'interface : vérifié, ramené à l'origine en bas.
    pub(super) fn checked(
        &self,
        open: &OpenDocument,
        element: &Element,
    ) -> Result<Element, AppError> {
        element
            .validate(open.info.page_count(), Some(&open.images))
            .map_err(invalid)?;
        let size = open.info.page_size(element.page())?;
        Ok(flip(element, size))
    }

    /// Applique l'opération au document ; en cas d'échec, elle est oubliée et
    /// le document reste tel qu'avant.
    pub(super) fn commit(
        &self,
        open: &mut OpenDocument,
        doc: DocumentId,
        change: Change,
    ) -> Result<(), AppError> {
        if change.is_empty() {
            return Ok(());
        }
        self.redraw(open, doc, change).inspect_err(|_| {
            open.edits.rollback();
        })
    }

    /// Range les pages, pose tous les éléments et marque les pages touchées
    /// comme changées (toutes si le plan des pages a changé).
    pub(super) fn redraw(
        &self,
        open: &mut OpenDocument,
        doc: DocumentId,
        change: Change,
    ) -> Result<(), AppError> {
        let elements = open
            .edits
            .elements()
            .map(|(id, e)| (id, e.clone()))
            .collect();
        let edition = Edition {
            plan: open.edits.plan().clone(),
            elements,
            images: open.images.clone(),
            imports: open.imports.clone(),
        };
        let placed = self.engine.apply_edits(doc, edition)?;
        open.placed = placed.into_iter().map(|p| (p.id, p.bounds)).collect();
        if change.layout {
            self.relayout(open, doc);
            return Ok(());
        }
        for page in change.pages {
            if let Some(revision) = open.revisions.get_mut(page) {
                *revision += 1;
            }
            lock(&self.cache).remove_page(doc, page);
            lock(&self.texts).remove(&(doc, page));
        }
        Ok(())
    }

    /// Le contenu à écrire se rouvre (avec le même mot de passe) et a le
    /// même nombre de pages.
    fn verify(&self, bytes: &[u8], open: &OpenDocument) -> Result<(), AppError> {
        let check = self.engine.open(bytes.to_vec(), open.password.clone())?;
        self.engine.close(check.id);
        if check.info.page_count() != open.info.page_count() {
            return Err(AppError::new(
                ErrorKind::Engine,
                "le document produit ne correspond pas : enregistrement annulé",
            ));
        }
        Ok(())
    }
}

pub(super) fn view(open: &OpenDocument) -> EditView {
    let elements = open
        .edits
        .elements()
        .filter_map(|(id, element)| {
            let size = open.info.page_size(element.page()).ok()?;
            let bounds = open.placed.get(&id)?;
            Some(ElementView::new(id, element, bounds, size))
        })
        .collect();
    EditView {
        elements,
        revisions: open.revisions.clone(),
        layout: open.layout,
        can_undo: open.edits.can_undo(),
        can_redo: open.edits.can_redo(),
        dirty: open.edits.is_dirty(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use pde_core::edit::{Element, TextBox, TextStyle};
    use pde_core::geometry::Point;

    use super::super::testing::Session;
    use super::*;
    use crate::fake::{SIMPLE_PDF, UNWRITABLE};

    /// Zone de texte en coordonnées de l'interface (origine en haut).
    fn text(page: usize, top: f64, content: &str) -> Element {
        Element::Text(TextBox {
            page,
            top_left: Point::new(72.0, top),
            width: 200.0,
            text: content.into(),
            style: TextStyle::default(),
        })
    }

    #[test]
    fn added_element_keeps_interface_coordinates() {
        let s = Session::new("coords");
        let (element_id, view) = s.viewer.add_element(s.id, text(0, 100.0, "a")).unwrap();
        assert_eq!(view.elements.len(), 1);
        assert_eq!(view.elements[0].id, element_id);
        assert_eq!(view.elements[0].element, text(0, 100.0, "a"));
        assert_eq!(view.elements[0].bounds.y, 100.0);
        assert_eq!(view.elements[0].bounds.height, 11.0 * 1.25);
        assert!(view.dirty && view.can_undo && !view.can_redo);
    }

    #[test]
    fn only_changed_pages_get_a_new_revision() {
        let s = Session::new("revisions");
        let (element_id, view) = s.viewer.add_element(s.id, text(1, 100.0, "a")).unwrap();
        assert_eq!(view.revisions, [0, 1]);
        let view = s
            .viewer
            .update_element(s.id, element_id, text(0, 100.0, "a"))
            .unwrap();
        assert_eq!(view.revisions, [1, 2]);
    }

    #[test]
    fn changed_page_is_rendered_again() {
        let s = Session::new("render");
        s.viewer.page_png(s.id, 0, 1.0).unwrap();
        s.viewer.page_png(s.id, 1, 1.0).unwrap();
        s.viewer.add_element(s.id, text(0, 100.0, "a")).unwrap();
        s.viewer.page_png(s.id, 0, 1.0).unwrap();
        s.viewer.page_png(s.id, 1, 1.0).unwrap();
        assert_eq!(
            s.engine.render_count(),
            3,
            "page 0 rendue à nouveau, page 1 en cache"
        );
    }

    #[test]
    fn undo_redo_and_dirty_state() {
        let s = Session::new("undo");
        let (element_id, _) = s.viewer.add_element(s.id, text(0, 100.0, "a")).unwrap();
        s.viewer
            .update_element(s.id, element_id, text(0, 100.0, "b"))
            .unwrap();
        let view = s.viewer.undo(s.id).unwrap();
        assert_eq!(view.elements[0].element, text(0, 100.0, "a"));
        assert!(view.can_redo);
        let view = s.viewer.undo(s.id).unwrap();
        assert!(view.elements.is_empty() && !view.dirty);
        let view = s.viewer.redo(s.id).unwrap();
        assert_eq!(view.elements.len(), 1);
        let view = s.viewer.remove_element(s.id, element_id).unwrap();
        assert!(view.elements.is_empty() && view.dirty);
        assert!(s.viewer.any_dirty());
        s.viewer.save(s.id, None).unwrap();
        assert!(!s.viewer.any_dirty());
    }

    #[test]
    fn invalid_element_is_refused_without_trace() {
        let s = Session::new("invalid");
        let error = s.viewer.add_element(s.id, text(5, 100.0, "a")).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidRequest);
        let view = s.viewer.edit_view(s.id).unwrap();
        assert!(!view.can_undo && !view.dirty);
    }

    #[test]
    fn engine_failure_leaves_the_document_unchanged() {
        let s = Session::new("failure");
        let (element_id, _) = s.viewer.add_element(s.id, text(0, 100.0, "a")).unwrap();
        assert!(
            s.viewer
                .update_element(s.id, element_id, text(0, 100.0, UNWRITABLE))
                .is_err()
        );
        let view = s.viewer.edit_view(s.id).unwrap();
        assert_eq!(view.elements[0].element, text(0, 100.0, "a"));
        assert!(!view.can_redo);
        // Une seule opération à annuler : l'ajout.
        assert!(s.viewer.undo(s.id).unwrap().elements.is_empty());
    }

    #[test]
    fn save_and_save_as() {
        let s = Session::new("save");
        s.viewer.add_element(s.id, text(0, 100.0, "a")).unwrap();
        let saved = s.viewer.save(s.id, None).unwrap();
        assert_eq!(saved.file_name, "contrat.pdf");
        assert!(!s.viewer.edit_view(s.id).unwrap().dirty);

        let copy = s.dir.join("copie.pdf");
        let saved = s.viewer.save(s.id, Some(copy.clone())).unwrap();
        assert_eq!(saved.file_name, "copie.pdf");
        assert_eq!(std::fs::read(&copy).unwrap(), SIMPLE_PDF);
        // « Enregistrer » écrit désormais dans la copie.
        assert_eq!(s.viewer.save(s.id, None).unwrap().file_name, "copie.pdf");
    }

    #[test]
    fn unknown_document_or_element() {
        let s = Session::new("unknown");
        assert_eq!(
            s.viewer.edit_view(s.id + 1).unwrap_err().kind,
            ErrorKind::UnknownDocument
        );
        assert_eq!(
            s.viewer.remove_element(s.id, 42).unwrap_err().kind,
            ErrorKind::InvalidRequest
        );
    }
}
