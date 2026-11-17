//! Sauvegarde automatique vue de la visionneuse (F-02) : instantanés des
//! documents modifiés, et réapplication d'une sauvegarde au même fichier.

use std::sync::Arc;

use pde_core::edit::{Change, Element, ImageId, Images};

use super::edit::{invalid, view};
use super::{Viewer, lock};
use crate::edit_view::EditView;
use crate::error::AppError;
use crate::recovery::{Recovered, Snapshot};

/// Document à sauvegarder, et la version de ses éléments à ce moment.
#[derive(Debug, Clone)]
pub struct Pending {
    pub doc: u64,
    pub revision: u64,
    pub snapshot: Snapshot,
}

/// Travail d'une sauvegarde automatique.
#[derive(Debug, Default)]
pub struct AutosaveWork {
    /// Modifiés depuis leur dernière sauvegarde automatique.
    pub pending: Vec<Pending>,
    /// Revenus à leur état enregistré : leur sauvegarde n'a plus d'objet.
    pub clean: Vec<u64>,
}

impl Viewer {
    /// Ce qu'il faut écrire ou effacer. Jamais un document protégé par mot
    /// de passe : ses ajouts ne doivent pas être écrits en clair sur le disque.
    pub fn autosave_work(&self) -> AutosaveWork {
        let mut work = AutosaveWork::default();
        let mut documents = lock(&self.documents);
        for (id, open) in documents.iter_mut() {
            if open.password.is_some() {
                continue;
            }
            if !open.edits.is_dirty() {
                if open.autosaved.take().is_some() {
                    work.clean.push(id.0);
                }
                continue;
            }
            let revision = open.edits.revision();
            if open.autosaved == Some(revision) {
                continue;
            }
            let elements: Vec<Element> = open.edits.elements().map(|(_, e)| e.clone()).collect();
            let images: Images = elements
                .iter()
                .filter_map(|e| match e {
                    Element::Image(image) => open
                        .images
                        .get(&image.image)
                        .map(|asset| (image.image, Arc::clone(asset))),
                    _ => None,
                })
                .collect();
            let file_name = open
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            work.pending.push(Pending {
                doc: id.0,
                revision,
                snapshot: Snapshot {
                    path: open.path.clone(),
                    file_name,
                    digest: open.digest.clone(),
                    page_count: open.original.page_count(),
                    pages: (!open.edits.plan().is_identity(open.original.page_count()))
                        .then(|| open.edits.plan().clone()),
                    elements,
                    images,
                },
            });
        }
        work
    }

    /// Sauvegarde écrite : rien à refaire tant que les éléments ne changent pas.
    pub fn mark_autosaved(&self, doc: u64, revision: u64) {
        if let Some(open) = lock(&self.documents).get_mut(&pde_core::document::DocumentId(doc)) {
            open.autosaved = Some(revision);
        }
    }

    /// Réapplique une sauvegarde au document `id`, qui doit être le même
    /// fichier (même empreinte, même nombre de pages). Le plan des pages et
    /// chaque élément deviennent des opérations ordinaires, annulables.
    pub fn restore(&self, id: u64, recovered: &Recovered) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            if open.digest != recovered.digest
                || open.original.page_count() != recovered.page_count
            {
                return Err(invalid(format!(
                    "« {} » a changé depuis la sauvegarde automatique : ses modifications ne peuvent pas y être reprises",
                    recovered.file_name
                )));
            }
            // Images reprises sous de nouveaux identifiants.
            let mut renamed = std::collections::HashMap::new();
            for (old, asset) in &recovered.images {
                let new = ImageId(open.images.keys().next_back().map_or(1, |last| last.0 + 1));
                open.images.insert(new, Arc::clone(asset));
                renamed.insert(*old, new);
            }
            let mut added = 0;
            let mut layout = false;
            if let Some(plan) = &recovered.pages {
                let change = open.edits.restore_plan(plan.clone()).map_err(invalid)?;
                layout = change.layout;
                added += usize::from(layout);
            }
            let page_count = open.edits.plan().len();
            let mut pages = Vec::new();
            for element in &recovered.elements {
                let mut element = element.clone();
                if let Element::Image(image) = &mut element {
                    image.image = renamed.get(&image.image).copied().unwrap_or(image.image);
                }
                if let Err(e) = element.validate(page_count, Some(&open.images)) {
                    (0..added).for_each(|_| {
                        open.edits.rollback();
                    });
                    return Err(invalid(e));
                }
                pages.extend(open.edits.add(element).1.pages);
                added += 1;
            }
            pages.sort_unstable();
            pages.dedup();
            if let Err(e) = self.redraw(open, doc, Change { pages, layout }) {
                (0..added).for_each(|_| {
                    open.edits.rollback();
                });
                return Err(e);
            }
            log::info!("sauvegarde automatique reprise : {added} élément(s)");
            Ok(view(open))
        })
    }
}

#[cfg(test)]
mod tests {
    use pde_core::edit::{ImageBox, TextBox, TextStyle};
    use pde_core::geometry::Point;

    use super::super::testing::Session;
    use super::*;
    use crate::error::ErrorKind;

    fn text(content: &str) -> Element {
        Element::Text(TextBox {
            page: 0,
            top_left: Point::new(72.0, 100.0),
            width: 200.0,
            text: content.into(),
            style: TextStyle::default(),
        })
    }

    #[test]
    fn only_changed_documents_are_saved_once() {
        let s = Session::new("autosave");
        assert!(s.viewer.autosave_work().pending.is_empty());
        s.viewer.add_element(s.id, text("a")).unwrap();
        let work = s.viewer.autosave_work();
        assert_eq!(work.pending.len(), 1);
        let pending = &work.pending[0];
        assert_eq!(pending.snapshot.file_name, "contrat.pdf");
        assert_eq!(pending.snapshot.digest.len(), 64);
        s.viewer.mark_autosaved(s.id, pending.revision);
        // Rien de nouveau : rien à écrire.
        assert!(s.viewer.autosave_work().pending.is_empty());
        // Annuler change la liste : nouvelle sauvegarde… puis plus rien à
        // garder une fois revenu à l'état d'origine.
        s.viewer.undo(s.id).unwrap();
        let work = s.viewer.autosave_work();
        assert!(work.pending.is_empty());
        assert_eq!(work.clean, [s.id]);
        assert!(s.viewer.autosave_work().clean.is_empty());
    }

    #[test]
    fn snapshot_restores_on_a_fresh_copy_of_the_same_file() {
        let s = Session::new("reprise");
        let imported = s.viewer.import_image(s.id, png()).unwrap();
        s.viewer.add_element(s.id, text("Annexe B")).unwrap();
        let image = Element::Image(ImageBox {
            page: 1,
            top_left: Point::new(50.0, 60.0),
            width: 40.0,
            height: 30.0,
            image: ImageId(imported.image_id),
            rotation: 0,
            opacity: 1.0,
            keep_ratio: true,
        });
        s.viewer.add_element(s.id, image).unwrap();
        let snapshot = s.viewer.autosave_work().pending.remove(0).snapshot;
        let before = s.viewer.edit_view(s.id).unwrap().elements;

        // Même fichier rouvert après un plantage.
        let path = s.dir.join("contrat.pdf");
        let reopened = s.viewer.open_file(&path, None).unwrap().id;
        let view = s.viewer.restore(reopened, &recovered(snapshot)).unwrap();
        assert!(view.dirty && view.can_undo);
        let elements: Vec<_> = view
            .elements
            .iter()
            .map(|e| (&e.element, &e.bounds))
            .collect();
        let expected: Vec<_> = before.iter().map(|e| (&e.element, &e.bounds)).collect();
        assert_eq!(elements, expected);
    }

    #[test]
    fn page_plan_is_saved_and_restored_before_the_elements() {
        let s = Session::new("reprise-pages");
        // Rien sur les pages : pas de plan dans la sauvegarde.
        s.viewer.add_element(s.id, text("a")).unwrap();
        assert!(s.viewer.autosave_work().pending[0].snapshot.pages.is_none());
        // Page 1 (avec son texte) placée après la page 2.
        s.viewer.move_pages(s.id, &[0], 2).unwrap();
        let snapshot = s.viewer.autosave_work().pending.remove(0).snapshot;
        assert_eq!(snapshot.page_count, 2);
        assert!(snapshot.pages.is_some());

        let reopened = s
            .viewer
            .open_file(&s.dir.join("contrat.pdf"), None)
            .unwrap()
            .id;
        let view = s.viewer.restore(reopened, &recovered(snapshot)).unwrap();
        assert_eq!(view.layout, 1);
        assert_eq!(view.elements[0].element.page(), 1);
        // Annuler défait l'élément, puis le plan.
        s.viewer.undo(reopened).unwrap();
        let view = s.viewer.undo(reopened).unwrap();
        assert!(!view.can_undo && view.layout == 2);
    }

    #[test]
    fn a_changed_file_is_refused() {
        let s = Session::new("reprise-refusee");
        s.viewer.add_element(s.id, text("a")).unwrap();
        let mut snapshot = s.viewer.autosave_work().pending.remove(0).snapshot;
        snapshot.digest = "ff".repeat(32);
        let error = s.viewer.restore(s.id, &recovered(snapshot)).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidRequest);
        assert!(error.message.contains("a changé"), "{}", error.message);
    }

    #[test]
    fn password_protected_documents_are_never_autosaved() {
        let s = Session::new("protege");
        s.viewer.add_element(s.id, text("a")).unwrap();
        lock(&s.viewer.documents)
            .values_mut()
            .for_each(|d| d.password = Some("x".into()));
        assert!(s.viewer.autosave_work().pending.is_empty());
    }

    fn recovered(snapshot: Snapshot) -> Recovered {
        Recovered {
            path: snapshot.path,
            file_name: snapshot.file_name,
            digest: snapshot.digest,
            page_count: snapshot.page_count,
            saved_at: 1,
            pages: snapshot.pages,
            elements: snapshot.elements,
            images: snapshot.images,
        }
    }

    fn png() -> Vec<u8> {
        use image::codecs::png::PngEncoder;
        use image::{ExtendedColorType, ImageEncoder};
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(&[9; 4 * 4], 2, 2, ExtendedColorType::Rgba8)
            .unwrap();
        out
    }
}
