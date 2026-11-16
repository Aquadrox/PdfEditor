//! Texte des pages et résultats de recherche tels que l'interface les
//! reçoit : rectangles en points de la page affichée, origine en **haut** à
//! gauche (le sens du HTML), pour un placement direct en pourcentage.

use pde_core::document::PageSize;
use pde_core::geometry::Rect;
use pde_core::text::TextRun;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoxView {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl BoxView {
    /// Rectangle de la page (origine en bas) → boîte HTML (origine en haut).
    pub fn new(rect: &Rect, page: PageSize) -> Self {
        Self {
            x: rect.min().x,
            y: page.height - rect.max().y,
            width: rect.width(),
            height: rect.height(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TextRunView {
    pub text: String,
    #[serde(flatten)]
    pub area: BoxView,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageTextView {
    pub runs: Vec<TextRunView>,
}

impl PageTextView {
    pub fn new(runs: Vec<TextRun>, page: PageSize) -> Self {
        Self {
            runs: runs
                .into_iter()
                .map(|run| TextRunView {
                    area: BoxView::new(&run.bounds, page),
                    text: run.text,
                })
                .collect(),
        }
    }
}

/// Occurrence trouvée : sa page et un rectangle par ligne couverte.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchHit {
    pub page: usize,
    pub rects: Vec<BoxView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    /// Vrai si la recherche s'est arrêtée à la limite d'occurrences.
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pde_core::geometry::Point;

    #[test]
    fn origin_moves_to_the_top() {
        let page = PageSize::new(595.0, 842.0);
        let rect = Rect::from_corners(Point::new(72.0, 760.0), Point::new(172.0, 784.0));
        assert_eq!(
            BoxView::new(&rect, page),
            BoxView {
                x: 72.0,
                y: 58.0,
                width: 100.0,
                height: 24.0
            }
        );
    }

    #[test]
    fn run_json_is_flat() {
        let page = PageSize::new(100.0, 100.0);
        let run = TextRun {
            text: "Oui".into(),
            bounds: Rect::from_corners(Point::new(0.0, 90.0), Point::new(15.0, 100.0)),
        };
        let json = serde_json::to_string(&PageTextView::new(vec![run], page)).unwrap();
        assert_eq!(
            json,
            r#"{"runs":[{"text":"Oui","x":0.0,"y":0.0,"width":15.0,"height":10.0}]}"#
        );
    }
}
