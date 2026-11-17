//! Formulaire (E-08) et propriétés du document (E-11) : valeurs saisies
//! dans les champs, aplatissement, titre, auteur, sujet et mots-clés.
//!
//! Comme le reste, ce sont des modifications appliquées par le moteur à
//! une copie de l'original : le fichier d'origine ne change pas tant qu'il
//! n'est pas enregistré, et chaque changement s'annule.

use serde::{Deserialize, Serialize};

use crate::document::PageIndex;
use crate::geometry::Rect;

/// Texte d'un champ ou d'une propriété au-delà duquel la saisie est refusée.
pub const MAX_FIELD_CHARS: usize = 10_000;

/// Valeur saisie dans un champ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FieldValue {
    /// Champ texte.
    Text(String),
    /// Case à cocher.
    Checked(bool),
    /// Bouton radio (valeur d'export choisie) ou liste de choix (option).
    Choice(String),
}

impl FieldValue {
    pub fn validate(&self) -> Result<(), String> {
        let text = match self {
            FieldValue::Text(text) | FieldValue::Choice(text) => text,
            FieldValue::Checked(_) => return Ok(()),
        };
        if text.chars().count() > MAX_FIELD_CHARS {
            return Err(format!(
                "texte trop long (plus de {MAX_FIELD_CHARS} caractères)"
            ));
        }
        if text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err("caractère de contrôle refusé".into());
        }
        Ok(())
    }
}

/// Propriétés du document (dictionnaire d'information du PDF). Une valeur
/// vide retire la propriété.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
}

impl Metadata {
    pub fn validate(&self) -> Result<(), String> {
        [&self.title, &self.author, &self.subject, &self.keywords]
            .iter()
            .try_for_each(|v| FieldValue::Text((*v).clone()).validate())
    }
}

/// Genre d'un champ, et ce qu'il faut pour le remplir.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldKind {
    Text {
        multiline: bool,
        max_len: Option<u32>,
    },
    Checkbox,
    /// Boutons radio : valeur d'export de chaque bouton, dans l'ordre.
    Radio {
        options: Vec<String>,
    },
    /// Liste ou liste déroulante : options proposées.
    Choice {
        options: Vec<String>,
        editable: bool,
    },
}

/// Emplacement d'un champ sur une page du document arrangé.
#[derive(Debug, Clone, PartialEq)]
pub struct Widget {
    pub page: PageIndex,
    /// Place sur la page affichée (origine en bas).
    pub bounds: Rect,
    /// Bouton radio : rang de son option dans `FieldKind::Radio`.
    pub option: Option<usize>,
}

/// Champ du formulaire, tel que lu dans le document d'origine.
#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    /// Nom complet (« client.nom »), qui le désigne.
    pub name: String,
    pub kind: FieldKind,
    pub read_only: bool,
    /// Valeur dans le document d'origine.
    pub value: Option<FieldValue>,
    pub widgets: Vec<Widget>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_checked() {
        assert_eq!(FieldValue::Text("Jean\nDupont".into()).validate(), Ok(()));
        assert!(
            FieldValue::Text("x".repeat(MAX_FIELD_CHARS + 1))
                .validate()
                .is_err()
        );
        assert!(FieldValue::Choice("a\u{7}".into()).validate().is_err());
        assert_eq!(FieldValue::Checked(true).validate(), Ok(()));
        let mut meta = Metadata::default();
        assert_eq!(meta.validate(), Ok(()));
        meta.title = "\u{0}".into();
        assert!(meta.validate().is_err());
    }

    #[test]
    fn json_for_the_interface() {
        let json = serde_json::to_value(FieldValue::Checked(true)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "checked", "value": true })
        );
        let back: FieldValue =
            serde_json::from_value(serde_json::json!({ "kind": "text", "value": "a" })).unwrap();
        assert_eq!(back, FieldValue::Text("a".into()));
        let meta: Metadata = serde_json::from_value(serde_json::json!({ "title": "T" })).unwrap();
        assert_eq!(meta.title, "T");
    }
}
