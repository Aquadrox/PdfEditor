//! Texte d'une page : chaque caractère avec sa position, base de la
//! sélection, de la recherche et des actions IA.
//!
//! Coordonnées : celles de la page **affichée** (rotation de la page
//! appliquée), en points, origine en bas à gauche, comme [`crate::geometry`].

use crate::geometry::Rect;

/// Caractère et son rectangle ; rectangle vide pour un caractère déduit par
/// le moteur (espace ou fin de ligne absents du contenu de la page).
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

/// Suite de caractères sur une même ligne, sans grand écart : l'unité de
/// la couche de texte sélectionnable de l'interface.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub bounds: Rect,
}

/// Deux rectangles sont sur la même ligne si leurs hauteurs se recouvrent
/// sur au moins la moitié de la plus petite.
pub(crate) fn same_line(a: &Rect, b: &Rect) -> bool {
    let overlap = a.max().y.min(b.max().y) - a.min().y.max(b.min().y);
    overlap >= 0.5 * a.height().min(b.height())
}

impl PageText {
    /// Le texte seul, tel qu'il serait copié.
    pub fn text(&self) -> String {
        self.chars.iter().map(|c| c.ch).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.iter().all(|c| c.ch.is_whitespace())
    }

    /// Regroupe les caractères en segments : un nouveau segment commence à
    /// chaque fin de ligne, changement de ligne ou grand écart horizontal
    /// (colonnes, tableaux).
    pub fn runs(&self) -> Vec<TextRun> {
        let mut runs = Vec::new();
        let mut current: Option<TextRun> = None;
        for c in &self.chars {
            if c.ch == '\n' || c.ch == '\r' {
                close(&mut runs, current.take());
                continue;
            }
            if c.bounds.is_empty() {
                // Espace déduit : gardé entre deux mots d'un même segment.
                if let Some(run) = current.as_mut()
                    && c.ch.is_whitespace()
                    && !run.text.ends_with(' ')
                {
                    run.text.push(' ');
                }
                continue;
            }
            match current.as_mut() {
                Some(run) if continues(run, &c.bounds) => {
                    run.text.push(c.ch);
                    run.bounds = run.bounds.union(&c.bounds);
                }
                _ => {
                    close(&mut runs, current.take());
                    current = Some(TextRun {
                        text: c.ch.to_string(),
                        bounds: c.bounds,
                    });
                }
            }
        }
        close(&mut runs, current);
        runs
    }
}

/// Le caractère prolonge le segment : même ligne, à droite, écart inférieur
/// à la hauteur du texte.
fn continues(run: &TextRun, next: &Rect) -> bool {
    let height = run.bounds.height().max(next.height());
    let gap = next.min().x - run.bounds.max().x;
    same_line(&run.bounds, next) && gap <= height && gap >= -0.5 * height
}

fn close(runs: &mut Vec<TextRun>, run: Option<TextRun>) {
    if let Some(mut run) = run {
        let trimmed = run.text.trim_end().len();
        run.text.truncate(trimmed);
        if !run.text.is_empty() {
            runs.push(run);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    /// Caractère de 5 × 10 pt posé en (x, y).
    fn at(ch: char, x: f64, y: f64) -> TextChar {
        TextChar {
            ch,
            bounds: Rect::from_corners(Point::new(x, y), Point::new(x + 5.0, y + 10.0)),
        }
    }

    fn generated(ch: char) -> TextChar {
        TextChar {
            ch,
            bounds: Rect::default(),
        }
    }

    fn word(text: &str, x: f64, y: f64) -> Vec<TextChar> {
        text.chars()
            .enumerate()
            .map(|(i, ch)| at(ch, x + 5.0 * i as f64, y))
            .collect()
    }

    fn runs_text(page: &PageText) -> Vec<String> {
        page.runs().into_iter().map(|r| r.text).collect()
    }

    #[test]
    fn text_concatenates_chars() {
        let page = PageText {
            chars: word("Oui", 0.0, 0.0),
        };
        assert_eq!(page.text(), "Oui");
        assert!(!page.is_empty());
    }

    #[test]
    fn page_without_text_or_only_spaces_is_empty() {
        assert!(PageText::default().is_empty());
        assert!(
            PageText {
                chars: vec![generated(' '), generated('\n')]
            }
            .is_empty()
        );
    }

    #[test]
    fn words_on_a_line_form_one_run_with_their_spaces() {
        let mut chars = word("Bonjour", 0.0, 100.0);
        chars.push(generated(' '));
        chars.extend(word("monde", 38.0, 100.0));
        let page = PageText { chars };
        let runs = page.runs();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "Bonjour monde");
        assert_eq!(runs[0].bounds.min(), Point::new(0.0, 100.0));
        assert_eq!(runs[0].bounds.max(), Point::new(63.0, 110.0));
    }

    #[test]
    fn line_breaks_and_new_lines_start_new_runs() {
        let mut chars = word("ligne", 0.0, 100.0);
        chars.push(generated('\r'));
        chars.push(generated('\n'));
        chars.extend(word("suite", 0.0, 85.0));
        // Pas de fin de ligne explicite mais une autre hauteur : nouveau segment.
        chars.extend(word("bas", 30.0, 60.0));
        assert_eq!(runs_text(&PageText { chars }), ["ligne", "suite", "bas"]);
    }

    #[test]
    fn large_gap_splits_columns() {
        let mut chars = word("gauche", 0.0, 100.0);
        chars.extend(word("droite", 300.0, 100.0));
        assert_eq!(runs_text(&PageText { chars }), ["gauche", "droite"]);
    }

    #[test]
    fn trailing_and_leading_generated_spaces_are_dropped() {
        let mut chars = vec![generated(' ')];
        chars.extend(word("mot", 0.0, 0.0));
        chars.push(generated(' '));
        assert_eq!(runs_text(&PageText { chars }), ["mot"]);
    }
}
