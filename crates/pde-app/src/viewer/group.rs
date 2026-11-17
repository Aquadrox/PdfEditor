//! Opérations sur une sélection de plusieurs éléments (E-03) : une seule
//! entrée de l'historique pour tout le groupe (dupliquer, déplacer,
//! aligner, supprimer).

use pde_core::edit::{Element, ElementId};

use super::Viewer;
use super::edit::{invalid, view};
use crate::edit_view::EditView;
use crate::error::AppError;

impl Viewer {
    /// Ajoute plusieurs éléments (origine en haut) ; renvoie leurs identifiants.
    pub fn add_elements(
        &self,
        id: u64,
        elements: Vec<Element>,
    ) -> Result<(Vec<u64>, EditView), AppError> {
        self.with_document(id, |open, doc| {
            let elements = elements
                .iter()
                .map(|e| self.checked(open, e))
                .collect::<Result<Vec<_>, _>>()?;
            let (ids, change) = open.edits.add_many(elements);
            self.commit(open, doc, change)?;
            Ok((ids.into_iter().map(|i| i.0).collect(), view(open)))
        })
    }

    /// Ajoute, remplace et retire des éléments en une opération (remplacer
    /// une image d'origine) ; renvoie les identifiants des éléments ajoutés.
    pub fn add_and_update_elements(
        &self,
        id: u64,
        added: Vec<Element>,
        updated: Vec<(u64, Element)>,
        removed: &[u64],
    ) -> Result<(Vec<u64>, EditView), AppError> {
        self.with_document(id, |open, doc| {
            let added = added
                .iter()
                .map(|e| self.checked(open, e))
                .collect::<Result<Vec<_>, _>>()?;
            let updated = updated
                .iter()
                .map(|(element_id, e)| Ok((ElementId(*element_id), self.checked(open, e)?)))
                .collect::<Result<Vec<_>, AppError>>()?;
            let removed: Vec<ElementId> = removed.iter().map(|&r| ElementId(r)).collect();
            let (ids, change) = open
                .edits
                .add_and_update(added, updated, &removed)
                .map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok((ids.into_iter().map(|i| i.0).collect(), view(open)))
        })
    }

    pub fn update_elements(
        &self,
        id: u64,
        changes: Vec<(u64, Element)>,
    ) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            let changes = changes
                .iter()
                .map(|(element_id, e)| Ok((ElementId(*element_id), self.checked(open, e)?)))
                .collect::<Result<Vec<_>, AppError>>()?;
            let change = open.edits.update_many(changes).map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(view(open))
        })
    }

    pub fn remove_elements(&self, id: u64, element_ids: &[u64]) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            let ids: Vec<ElementId> = element_ids.iter().map(|&i| ElementId(i)).collect();
            let change = open.edits.remove_many(&ids).map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(view(open))
        })
    }
}

#[cfg(test)]
mod tests {
    use pde_core::edit::{TextBox, TextStyle};
    use pde_core::geometry::Point;

    use super::super::testing::Session;
    use super::*;
    use crate::fake::UNWRITABLE;

    fn text(top: f64, content: &str) -> Element {
        Element::Text(TextBox {
            page: 0,
            top_left: Point::new(72.0, top),
            width: 200.0,
            text: content.into(),
            style: TextStyle::default(),
        })
    }

    /// Image d'origine du faux moteur (origine en haut : y de 142 à 242).
    fn existing(frame: Option<pde_core::edit::Area>) -> Element {
        Element::ExistingImage(pde_core::edit::ExistingImage {
            page: 0,
            object: 2,
            original: pde_core::edit::Area {
                top_left: Point::new(100.0, 142.0),
                width: 200.0,
                height: 100.0,
            },
            frame,
        })
    }

    #[test]
    fn an_original_image_is_moved_then_removed_and_replaced_in_one_step() {
        let s = Session::new("image-origine");
        let moved = pde_core::edit::Area {
            top_left: Point::new(50.0, 300.0),
            width: 100.0,
            height: 50.0,
        };
        let (edit_id, view) = s.viewer.add_element(s.id, existing(Some(moved))).unwrap();
        // Coordonnées de l'interface gardées, place = nouveau cadre.
        assert_eq!(view.elements[0].element, existing(Some(moved)));
        assert_eq!(view.elements[0].bounds.y, 300.0);
        // Remplacer : l'image d'origine supprimée et une zone posée, ensemble.
        let (ids, view) = s
            .viewer
            .add_and_update_elements(
                s.id,
                vec![text(300.0, "à la place")],
                vec![(edit_id, existing(None))],
                &[],
            )
            .unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(view.elements.len(), 2);
        let view = s.viewer.undo(s.id).unwrap();
        assert_eq!(view.elements.len(), 1);
        assert_eq!(view.elements[0].element, existing(Some(moved)));
        assert!(
            s.viewer
                .add_and_update_elements(s.id, Vec::new(), vec![(99, existing(None))], &[])
                .is_err()
        );
    }

    #[test]
    fn group_add_move_remove_are_undone_at_once() {
        let s = Session::new("groupe");
        let (ids, view) = s
            .viewer
            .add_elements(s.id, vec![text(100.0, "a"), text(150.0, "b")])
            .unwrap();
        assert_eq!((ids.len(), view.elements.len()), (2, 2));
        let view = s
            .viewer
            .update_elements(
                s.id,
                vec![(ids[0], text(110.0, "a")), (ids[1], text(160.0, "b"))],
            )
            .unwrap();
        // Coordonnées de l'interface (origine en haut) gardées.
        let tops: Vec<f64> = view.elements.iter().map(|e| e.bounds.y).collect();
        assert_eq!(tops, [110.0, 160.0]);
        let view = s.viewer.undo(s.id).unwrap();
        let tops: Vec<f64> = view.elements.iter().map(|e| e.bounds.y).collect();
        assert_eq!(tops, [100.0, 150.0], "une annulation pour le groupe");
        let view = s.viewer.remove_elements(s.id, &ids).unwrap();
        assert!(view.elements.is_empty());
        assert_eq!(s.viewer.undo(s.id).unwrap().elements.len(), 2);
    }

    #[test]
    fn a_failing_group_changes_nothing() {
        let s = Session::new("groupe-echec");
        let (ids, _) = s
            .viewer
            .add_elements(s.id, vec![text(100.0, "a"), text(150.0, "b")])
            .unwrap();
        let error = s.viewer.update_elements(
            s.id,
            vec![
                (ids[0], text(110.0, "a")),
                (ids[1], text(160.0, UNWRITABLE)),
            ],
        );
        assert!(error.is_err());
        let view = s.viewer.edit_view(s.id).unwrap();
        let tops: Vec<f64> = view.elements.iter().map(|e| e.bounds.y).collect();
        assert_eq!(tops, [100.0, 150.0]);
        assert!(
            s.viewer
                .update_elements(s.id, vec![(999, text(1.0, "x"))])
                .is_err()
        );
    }
}
