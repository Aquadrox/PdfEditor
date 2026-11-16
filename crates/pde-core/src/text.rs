//! Texte d'une page : chaque caractère avec sa position, base de la
//! sélection, de la recherche et des actions IA.

use crate::geometry::Rect;

/// Caractère et son rectangle en coordonnées de page (points, origine en bas à gauche).
#[derive(Debug, Clone, PartialEq)]
pub struct TextChar {
    pub ch: char,
    pub bounds: Rect,
}

/// Texte d'une page dans l'ordre de lecture donné par le moteur.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageText {
    pub chars: Vec<TextChar>,
}

impl PageText {
    /// Le texte seul, tel qu'il serait copié.
    pub fn text(&self) -> String {
        self.chars.iter().map(|c| c.ch).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.iter().all(|c| c.ch.is_whitespace())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn ch(ch: char, x: f64) -> TextChar {
        TextChar {
            ch,
            bounds: Rect::from_corners(Point::new(x, 0.0), Point::new(x + 5.0, 10.0)),
        }
    }

    #[test]
    fn text_concatenates_chars() {
        let page = PageText {
            chars: vec![ch('O', 0.0), ch('u', 5.0), ch('i', 10.0)],
        };
        assert_eq!(page.text(), "Oui");
        assert!(!page.is_empty());
    }

    #[test]
    fn page_without_text_or_only_spaces_is_empty() {
        assert!(PageText::default().is_empty());
        assert!(
            PageText {
                chars: vec![ch(' ', 0.0), ch('\n', 5.0)]
            }
            .is_empty()
        );
    }
}
