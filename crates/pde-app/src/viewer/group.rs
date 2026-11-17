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
