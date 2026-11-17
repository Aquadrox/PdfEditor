//! Notes (fin de E-04) : de vraies annotations PDF « Texte », avec leur
//! commentaire et leur auteur, lisibles dans tous les lecteurs (icône sur
//! la page, commentaire dans le panneau des annotations du lecteur).
//!
//! Coordonnées : page affichée, points, origine en bas à gauche.

use serde::{Deserialize, Serialize};

use super::element::Color;
use crate::document::PageIndex;
use crate::geometry::{Point, Rect};

/// Côté de l'icône d'une note (points), comme dans les autres lecteurs.
pub const NOTE_SIZE: f64 = 20.0;
pub const MAX_NOTE_CHARS: usize = 20_000;
pub const MAX_AUTHOR_CHARS: usize = 200;

fn yellow() -> Color {
    Color {
        r: 0xff,
        g: 0xd5,
        b: 0x4a,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteBox {
    pub page: PageIndex,
    /// Coin haut gauche de l'icône.
    pub top_left: Point,
    /// Commentaire (peut rester vide le temps de l'écrire).
    #[serde(default)]
    pub text: String,
    /// Auteur affiché par les lecteurs (réglage Édition).
    #[serde(default)]
    pub author: String,
    #[serde(default = "yellow")]
    pub color: Color,
}

impl NoteBox {
    pub fn bounds(&self) -> Rect {
        Rect::from_corners(
            Point::new(self.top_left.x, self.top_left.y - NOTE_SIZE),
            Point::new(self.top_left.x + NOTE_SIZE, self.top_left.y),
        )
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !self.top_left.x.is_finite() || !self.top_left.y.is_finite() {
            return Err("position invalide".into());
        }
        if self.text.chars().count() > MAX_NOTE_CHARS {
            return Err(format!(
                "note trop longue (plus de {MAX_NOTE_CHARS} caractères)"
            ));
        }
        if self.author.chars().count() > MAX_AUTHOR_CHARS {
            return Err(format!(
                "nom d'auteur trop long (plus de {MAX_AUTHOR_CHARS} caractères)"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note() -> NoteBox {
        NoteBox {
            page: 0,
            top_left: Point::new(100.0, 500.0),
            text: "À vérifier".into(),
            author: "Camille".into(),
            color: yellow(),
        }
    }

    #[test]
    fn icon_bounds_and_validation() {
        let b = note().bounds();
        assert_eq!(
            (b.min(), b.max()),
            (Point::new(100.0, 480.0), Point::new(120.0, 500.0))
        );
        assert_eq!(note().validate(), Ok(()));
        let cases: [fn(&mut NoteBox); 3] = [
            |n| n.top_left.y = f64::NAN,
            |n| n.text = "x".repeat(MAX_NOTE_CHARS + 1),
            |n| n.author = "x".repeat(MAX_AUTHOR_CHARS + 1),
        ];
        for change in cases {
            let mut n = note();
            change(&mut n);
            assert!(n.validate().is_err());
        }
    }

    #[test]
    fn json_defaults() {
        let n: NoteBox = serde_json::from_str(r#"{"page":2,"top_left":{"x":1,"y":2}}"#).unwrap();
        assert_eq!(
            (n.text.as_str(), n.author.as_str(), n.color),
            ("", "", yellow())
        );
    }
}
