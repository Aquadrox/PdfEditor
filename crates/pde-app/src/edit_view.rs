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
        Element::Image(mut image) => {
            image.top_left = Point::new(image.top_left.x, page.height - image.top_left.y);
            Element::Image(image)
        }
        Element::Shape(mut shape) => {
            shape.start = flip_point(shape.start, page);
            shape.end = flip_point(shape.end, page);
            Element::Shape(shape)
        }
        Element::Ink(mut ink) => {
            ink.points
                .iter_mut()
                .for_each(|p| *p = flip_point(*p, page));
            Element::Ink(ink)
        }
        Element::Note(mut note) => {
            note.top_left = flip_point(note.top_left, page);
            Element::Note(note)
        }
        Element::Markup(mut markup) => {
            for area in &mut markup.areas {
                area.top_left = flip_point(area.top_left, page);
            }
            Element::Markup(markup)
        }
        Element::Redact(mut redact) => {
            for area in &mut redact.areas {
                area.top_left = flip_point(area.top_left, page);
            }
            Element::Redact(redact)
        }
        Element::ExistingImage(mut image) => {
            image.original.top_left = flip_point(image.original.top_left, page);
            if let Some(frame) = &mut image.frame {
                frame.top_left = flip_point(frame.top_left, page);
            }
            Element::ExistingImage(image)
        }
    }
}

fn flip_point(p: Point, page: PageSize) -> Point {
    Point::new(p.x, page.height - p.y)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementView {
    pub id: u64,
    /// L'élément, coordonnées avec l'origine en haut.
    pub element: Element,
    /// Place occupée (hauteur du texte comprise), origine en haut.
    pub bounds: BoxView,
    /// Caractères d'une zone de texte qu'aucune police ne sait écrire
    /// (chinois, arabe… : signalés, prévus plus tard) ; vide sinon.
    pub missing_chars: String,
}

impl ElementView {
    pub fn new(id: ElementId, element: &Element, bounds: &Rect, page: PageSize) -> Self {
        let missing_chars = match element {
            Element::Text(text) => pde_fonts::missing_chars(
                &text.text,
                text.style.font,
                text.style.bold,
                text.style.italic,
            )
            .into_iter()
            .collect(),
            _ => String::new(),
        };
        Self {
            id: id.0,
            element: flip(element, page),
            bounds: BoxView::new(bounds, page),
            missing_chars,
        }
    }
}

/// État de l'édition d'un document après chaque opération.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EditView {
    pub elements: Vec<ElementView>,
    /// Version du contenu de chaque page (à mettre dans l'adresse de l'image).
    pub revisions: Vec<u64>,
    /// Version du plan des pages : quand elle change, l'interface relit la
    /// description du document (nombre, ordre et taille des pages).
    pub layout: u64,
    /// Valeurs saisies dans les champs du formulaire (E-08), par nom.
    pub fields: std::collections::BTreeMap<String, pde_core::edit::FieldValue>,
    /// Propriétés du document changées (E-11), sinon celles de l'original.
    pub metadata: Option<pde_core::edit::Metadata>,
    /// Formulaire aplati.
    pub flattened: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Modifié depuis l'ouverture ou le dernier enregistrement.
    pub dirty: bool,
}

/// Image importée, prête à être posée : taille en pixels pour proposer un
/// cadre aux bonnes proportions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportedImage {
    pub image_id: u64,
    pub width: u32,
    pub height: u32,
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
        let Element::Text(text) = &flipped else {
            panic!("zone de texte attendue");
        };
        assert_eq!(text.top_left, Point::new(72.0, 142.0));
        assert_eq!(flip(&flipped, page), element);
    }

    #[test]
    fn annotations_flip_every_point() {
        use pde_core::edit::{Area, Color, InkBox, MarkKind, MarkupBox, ShapeBox, ShapeKind};
        let page = PageSize::new(595.0, 842.0);
        let red = Color { r: 200, g: 0, b: 0 };
        let elements = [
            Element::Shape(ShapeBox {
                page: 0,
                shape: ShapeKind::Arrow,
                start: Point::new(10.0, 20.0),
                end: Point::new(30.0, 40.0),
                color: red,
                width: 2.0,
                filled: false,
            }),
            Element::Ink(InkBox {
                page: 0,
                points: vec![Point::new(1.0, 2.0), Point::new(3.0, 800.0)],
                color: red,
                width: 2.0,
            }),
            Element::Markup(MarkupBox {
                page: 0,
                mark: MarkKind::Highlight,
                areas: vec![Area {
                    top_left: Point::new(50.0, 100.0),
                    width: 80.0,
                    height: 12.0,
                }],
                color: red,
            }),
        ];
        for element in elements {
            let flipped = flip(&element, page);
            assert_ne!(flipped, element);
            assert_eq!(flip(&flipped, page), element);
        }
        let Element::Shape(shape) = flip(
            &Element::Shape(ShapeBox {
                page: 0,
                shape: ShapeKind::Line,
                start: Point::new(10.0, 20.0),
                end: Point::new(30.0, 40.0),
                color: red,
                width: 2.0,
                filled: false,
            }),
            page,
        ) else {
            panic!("forme attendue");
        };
        assert_eq!(
            (shape.start, shape.end),
            (Point::new(10.0, 822.0), Point::new(30.0, 802.0))
        );
    }

    #[test]
    fn unsupported_characters_are_reported() {
        let page = PageSize::new(595.0, 842.0);
        let rect = Rect::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let text = |content: &str| {
            Element::Text(TextBox {
                page: 0,
                top_left: Point::new(72.0, 700.0),
                width: 200.0,
                text: content.into(),
                style: TextStyle::default(),
            })
        };
        let view = ElementView::new(ElementId(1), &text("Contrat 契約, Договор"), &rect, page);
        assert_eq!(view.missing_chars, "契約");
        let view = ElementView::new(ElementId(1), &text("Σύμβαση, œuvre à 18 €"), &rect, page);
        assert_eq!(view.missing_chars, "");
    }
}
