//! Éléments ajoutés tels que l'interface les échange : coordonnées de la
//! page affichée avec l'origine en **haut** à gauche (le sens du HTML).
//! `pde-core` les garde avec l'origine en bas : la conversion est ici.

use pde_core::document::PageSize;
use pde_core::edit::{Element, ElementId};
use pde_core::geometry::{Point, Rect};
use serde::Serialize;

use crate::text_view::BoxView;

/// Retourne l'axe vertical d'un élément (bas ↔ haut) ; la conversion est sa
/// propre réciproque.
pub fn flip(element: &Element, page: PageSize) -> Element {
    match element.clone() {
        Element::Text(mut text) => {
            text.top_left = Point::new(text.top_left.x, page.height - text.top_left.y);
            Element::Text(text)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementView {
    pub id: u64,
    /// L'élément, coordonnées avec l'origine en haut.
    pub element: Element,
    /// Place occupée (hauteur du texte comprise), origine en haut.
    pub bounds: BoxView,
}

impl ElementView {
    pub fn new(id: ElementId, element: &Element, bounds: &Rect, page: PageSize) -> Self {
        Self {
            id: id.0,
            element: flip(element, page),
            bounds: BoxView::new(bounds, page),
        }
    }
}

/// État de l'édition d'un document après chaque opération.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EditView {
    pub elements: Vec<ElementView>,
    /// Version du contenu de chaque page (à mettre dans l'adresse de l'image).
    pub revisions: Vec<u64>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Modifié depuis l'ouverture ou le dernier enregistrement.
    pub dirty: bool,
}

/// Résultat d'un enregistrement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SavedView {
    pub path: String,
    pub file_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pde_core::edit::{TextBox, TextStyle};

    #[test]
    fn flip_is_its_own_inverse() {
        let page = PageSize::new(595.0, 842.0);
        let element = Element::Text(TextBox {
            page: 0,
            top_left: Point::new(72.0, 700.0),
            width: 200.0,
            text: "a".into(),
            style: TextStyle::default(),
        });
        let flipped = flip(&element, page);
        let Element::Text(text) = &flipped;
        assert_eq!(text.top_left, Point::new(72.0, 142.0));
        assert_eq!(flip(&flipped, page), element);
    }
}
