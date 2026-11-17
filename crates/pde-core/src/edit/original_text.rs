//! Texte d'origine du document modifié (E-05), bloc par bloc. Un bloc est
//! une suite d'objets texte de la page sur une même ligne (même police,
//! même corps, même couleur) ; il est désigné par sa page et le rang de
//! ses objets parmi ceux de la page arrangée, avant tout ajout : le moteur
//! repartant de l'original à chaque pose, ces rangs ne changent pas. Son
//! cadre d'origine est gardé pour vérifier que c'est bien lui.
//!
//! Le moteur garde la police d'origine si elle sait écrire le nouveau
//! texte ; sinon il retire le bloc et écrit le texte au même endroit avec
//! la police standard la plus proche (`style`), ce qui est signalé.

use serde::{Deserialize, Serialize};

use super::annotation::Area;
use super::element::{FONT_SIZE_RANGE, TextStyle};
use crate::document::PageIndex;
use crate::geometry::Rect;

/// Objets texte d'un bloc, au plus.
pub const MAX_BLOCK_OBJECTS: usize = 2_000;
/// Texte d'un bloc (une ligne), au plus.
pub const MAX_BLOCK_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OriginalText {
    pub page: PageIndex,
    /// Rangs des objets texte du bloc parmi les objets de la page, croissants.
    pub objects: Vec<u32>,
    /// Cadre du bloc dans le document d'origine.
    pub original: Area,
    /// Texte d'origine du bloc (affiché pour l'annulation, jamais journalisé).
    pub before: String,
    /// Nouveau texte, sur une ligne ; vide : bloc supprimé.
    pub text: String,
    /// Police de remplacement, corps et couleur du bloc.
    #[serde(default)]
    pub style: TextStyle,
}

impl OriginalText {
    /// Place du bloc d'origine (le moteur donne celle du nouveau texte).
    pub fn bounds(&self) -> Rect {
        self.original.rect()
    }

    pub fn is_removed(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.objects.is_empty() || self.objects.len() > MAX_BLOCK_OBJECTS {
            return Err("bloc de texte d'origine invalide".into());
        }
        if self.objects.windows(2).any(|w| w[0] >= w[1]) {
            return Err("objets du bloc dans le désordre".into());
        }
        let area = &self.original;
        let numbers = [area.top_left.x, area.top_left.y, area.width, area.height];
        if numbers.iter().any(|v| !v.is_finite()) || area.width < 0.0 || area.height < 0.0 {
            return Err("cadre d'origine invalide".into());
        }
        if !self.style.size.is_finite() || !FONT_SIZE_RANGE.contains(&self.style.size) {
            return Err(format!(
                "corps entre {} et {} pt",
                FONT_SIZE_RANGE.start(),
                FONT_SIZE_RANGE.end()
            ));
        }
        if self.text.contains(['\n', '\r']) {
            return Err("le texte d'origine se modifie ligne par ligne".into());
        }
        if self.text.chars().count() > MAX_BLOCK_CHARS {
            return Err(format!(
                "texte trop long (plus de {MAX_BLOCK_CHARS} caractères)"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn block(text: &str) -> OriginalText {
        OriginalText {
            page: 0,
            objects: vec![2, 3],
            original: Area {
                top_left: Point::new(72.0, 770.0),
                width: 120.0,
                height: 14.0,
            },
            before: "Bonjour PdfEditor".into(),
            text: text.into(),
            style: TextStyle {
                size: 14.0,
                ..TextStyle::default()
            },
        }
    }

    #[test]
    fn a_new_line_of_text_is_valid_an_empty_one_removes_the_block() {
        assert_eq!(block("Bonsoir").validate(), Ok(()));
        assert!(!block("Bonsoir").is_removed());
        assert!(block("  ").is_removed());
        assert_eq!(block("").validate(), Ok(()));
        assert_eq!(block("x").bounds().max(), Point::new(192.0, 770.0));
    }

    #[test]
    fn invalid_blocks_are_refused() {
        let cases: [fn(&mut OriginalText); 6] = [
            |b| b.objects.clear(),
            |b| b.objects = vec![3, 2],
            |b| b.original.width = f64::NAN,
            |b| b.style.size = 0.5,
            |b| b.text = "deux\nlignes".into(),
            |b| b.text = "x".repeat(MAX_BLOCK_CHARS + 1),
        ];
        for change in cases {
            let mut b = block("ok");
            change(&mut b);
            assert!(b.validate().is_err(), "{b:?}");
        }
    }

    #[test]
    fn json_for_the_interface() {
        let json = serde_json::to_value(super::super::Element::OriginalText(block("A"))).unwrap();
        assert_eq!(json["kind"], "original_text");
        assert_eq!(json["objects"], serde_json::json!([2, 3]));
        assert_eq!(json["style"]["size"], 14.0);
    }
}
