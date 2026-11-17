//! Rubrique Édition (R-03) : réglages des nouveaux éléments ajoutés.

use serde::{Deserialize, Serialize};

use crate::edit::{FONT_SIZE_RANGE, MAX_AUTHOR_CHARS, StandardFont};

const DEFAULT_TEXT_COLOR: &str = "#1c2430";
const DEFAULT_HIGHLIGHT_COLOR: &str = "#ffd54a";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edit {
    /// Nom inscrit comme auteur des notes (vide : aucun).
    pub author: String,
    /// Police des nouvelles zones de texte.
    pub font: StandardFont,
    /// Corps des nouvelles zones de texte (points).
    pub font_size: f64,
    /// Couleur des nouvelles zones de texte (`#rrggbb`).
    pub text_color: String,
    /// Couleur de surlignage (`#rrggbb`).
    pub highlight_color: String,
}

impl Default for Edit {
    fn default() -> Self {
        Self {
            author: String::new(),
            font: StandardFont::Sans,
            font_size: 11.0,
            text_color: DEFAULT_TEXT_COLOR.into(),
            highlight_color: DEFAULT_HIGHLIGHT_COLOR.into(),
        }
    }
}

/// Couleur `#rrggbb` (casse indifférente), rendue en minuscules.
fn hex_color(value: &str) -> Option<String> {
    let digits = value.strip_prefix('#')?;
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| format!("#{}", digits.to_ascii_lowercase()))
}

impl Edit {
    /// Valeurs ramenées dans leurs limites, chaque correction notée.
    pub(super) fn checked(&self, warnings: &mut Vec<String>) -> Edit {
        let mut e = self.clone();
        let size = if e.font_size.is_finite() {
            e.font_size
        } else {
            11.0
        };
        let clamped = size.clamp(*FONT_SIZE_RANGE.start(), *FONT_SIZE_RANGE.end());
        if clamped != e.font_size {
            warnings.push(format!(
                "edit.font_size = {} hors de {} à {} : ramené à {clamped}",
                e.font_size,
                FONT_SIZE_RANGE.start(),
                FONT_SIZE_RANGE.end()
            ));
            e.font_size = clamped;
        }
        for (name, value, default) in [
            ("edit.text_color", &mut e.text_color, DEFAULT_TEXT_COLOR),
            (
                "edit.highlight_color",
                &mut e.highlight_color,
                DEFAULT_HIGHLIGHT_COLOR,
            ),
        ] {
            match hex_color(value) {
                Some(color) => *value = color,
                None => {
                    warnings.push(format!(
                        "{name} = « {value} » n'est pas une couleur #rrggbb : {default} utilisée"
                    ));
                    *value = default.into();
                }
            }
        }
        let author = e.author.trim().to_owned();
        if author.chars().count() > MAX_AUTHOR_CHARS {
            warnings.push(format!(
                "edit.author : plus de {MAX_AUTHOR_CHARS} caractères, raccourci"
            ));
            e.author = author.chars().take(MAX_AUTHOR_CHARS).collect();
        } else {
            e.author = author;
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let mut warnings = Vec::new();
        assert_eq!(Edit::default().checked(&mut warnings), Edit::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn invalid_values_are_corrected_with_a_warning() {
        let e = Edit {
            author: format!("  {}  ", "a".repeat(MAX_AUTHOR_CHARS + 5)),
            font: StandardFont::Serif,
            font_size: 900.0,
            text_color: "rouge".into(),
            highlight_color: "#ABCDEF".into(),
        };
        let mut warnings = Vec::new();
        let checked = e.checked(&mut warnings);
        assert_eq!(checked.font_size, 200.0);
        assert_eq!(checked.text_color, DEFAULT_TEXT_COLOR);
        assert_eq!(
            checked.highlight_color, "#abcdef",
            "casse seule : pas d'avertissement"
        );
        assert_eq!(checked.author.chars().count(), MAX_AUTHOR_CHARS);
        assert_eq!(checked.font, StandardFont::Serif);
        assert_eq!(warnings.len(), 3, "{warnings:?}");
    }

    #[test]
    fn author_is_trimmed_silently() {
        let mut warnings = Vec::new();
        let e = Edit {
            author: "  Camille Martin ".into(),
            ..Edit::default()
        };
        assert_eq!(e.checked(&mut warnings).author, "Camille Martin");
        assert!(warnings.is_empty());
    }
}
