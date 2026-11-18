//! Réglages de la traduction (IA-06 à IA-08) : dernière langue choisie,
//! plus petit corps accepté quand le texte traduit est plus long, et
//! glossaire (termes imposés, ou à ne pas traduire).

use serde::{Deserialize, Serialize};

/// Corps minimal accepté, en points.
pub const MIN_SIZE_RANGE: std::ops::RangeInclusive<f64> = 4.0..=24.0;
/// Glossaire, au plus (caractères).
const MAX_GLOSSARY: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Translation {
    /// Langue proposée par défaut (la dernière choisie).
    pub language: String,
    /// Corps en dessous duquel le texte traduit n'est plus réduit : il
    /// déborde, et c'est signalé.
    pub min_size: f64,
    /// Une ligne par terme : « terme = traduction », ou « terme » seul pour
    /// le garder tel quel.
    pub glossary: String,
}

impl Default for Translation {
    fn default() -> Self {
        Self {
            language: "anglais".into(),
            min_size: 6.0,
            glossary: String::new(),
        }
    }
}

/// Entrée du glossaire : traduction imposée, ou `None` pour garder le terme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossaryEntry {
    pub term: String,
    pub translation: Option<String>,
}

impl Translation {
    pub(super) fn checked(&self, warnings: &mut Vec<String>) -> Translation {
        let mut checked = self.clone();
        if !checked.min_size.is_finite() || !MIN_SIZE_RANGE.contains(&checked.min_size) {
            warnings.push(format!(
                "ai.translation.min_size : entre {} et {} pt ; 6 pt rétabli",
                MIN_SIZE_RANGE.start(),
                MIN_SIZE_RANGE.end()
            ));
            checked.min_size = 6.0;
        }
        checked.language = checked.language.trim().chars().take(40).collect();
        if checked.language.is_empty() {
            checked.language = Translation::default().language;
        }
        if checked.glossary.chars().count() > MAX_GLOSSARY {
            warnings.push(format!(
                "ai.translation.glossary : {MAX_GLOSSARY} caractères au plus"
            ));
            checked.glossary = checked.glossary.chars().take(MAX_GLOSSARY).collect();
        }
        checked
    }

    /// Entrées du glossaire ; lignes vides et commentaires (« # ») ignorés.
    pub fn glossary_entries(&self) -> Vec<GlossaryEntry> {
        self.glossary
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let (term, translation) = match line.split_once('=') {
                    Some((term, translation)) => (term.trim(), Some(translation.trim())),
                    None => (line, None),
                };
                (!term.is_empty()).then(|| GlossaryEntry {
                    term: term.to_owned(),
                    translation: translation.filter(|t| !t.is_empty()).map(str::to_owned),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glossary_lines_are_read() {
        let translation = Translation {
            glossary: "# termes du client\nbail = lease\n\nPdfEditor\n  Société Dupont =  \n= rien"
                .into(),
            ..Translation::default()
        };
        let entries = translation.glossary_entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].translation.as_deref(), Some("lease"));
        assert_eq!(
            entries[1],
            GlossaryEntry {
                term: "PdfEditor".into(),
                translation: None
            }
        );
        assert_eq!(entries[2].term, "Société Dupont");
        assert_eq!(
            entries[2].translation, None,
            "traduction vide : terme gardé"
        );
    }

    #[test]
    fn out_of_range_values_are_corrected() {
        let mut warnings = Vec::new();
        let checked = Translation {
            min_size: 1.0,
            language: "   ".into(),
            ..Translation::default()
        }
        .checked(&mut warnings);
        assert_eq!(checked.min_size, 6.0);
        assert_eq!(checked.language, "anglais");
        assert_eq!(warnings.len(), 1);
    }
}
