//! Formulaire (E-08) et propriétés du document (E-11) : valeurs saisies
//! dans les champs, aplatissement, titre, auteur, sujet et mots-clés ;
//! genre et réglages de chaque champ (E-12 : formats, calculs, champs
//! obligatoires, apparence).
//!
//! Comme le reste, ce sont des modifications appliquées par le moteur à
//! une copie de l'original : le fichier d'origine ne change pas tant qu'il
//! n'est pas enregistré, et chaque changement s'annule.

use serde::{Deserialize, Serialize};

use super::element::{Align, Color, ElementId};
use crate::document::PageIndex;
use crate::formats::{FieldCalc, FieldFormat};
use crate::geometry::Rect;

/// Texte d'un champ ou d'une propriété au-delà duquel la saisie est refusée.
pub const MAX_FIELD_CHARS: usize = 10_000;
/// Options d'une liste au-delà desquelles la liste est refusée.
pub const MAX_OPTIONS: usize = 500;
/// Corps accepté pour le texte d'un champ (0 : automatique).
pub const FIELD_FONT_RANGE: std::ops::RangeInclusive<f64> = 4.0..=72.0;
/// Infobulle la plus longue acceptée.
pub const MAX_TOOLTIP_CHARS: usize = 500;

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
    /// Liste à choix multiples : options choisies.
    Choices(Vec<String>),
}

pub(crate) fn check_text(text: &str) -> Result<(), String> {
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

impl FieldValue {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            FieldValue::Text(text) | FieldValue::Choice(text) => check_text(text),
            FieldValue::Checked(_) => Ok(()),
            FieldValue::Choices(items) if items.len() > MAX_OPTIONS => {
                Err(format!("{MAX_OPTIONS} options au plus"))
            }
            FieldValue::Choices(items) => items.iter().try_for_each(|i| check_text(i)),
        }
    }

    /// Texte de la valeur (champ texte, choix) ; vide pour une case.
    pub fn text(&self) -> &str {
        match self {
            FieldValue::Text(text) | FieldValue::Choice(text) => text,
            FieldValue::Choices(items) => items.first().map_or("", String::as_str),
            FieldValue::Checked(_) => "",
        }
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
            .try_for_each(|v| check_text(v))
    }
}

/// Genre d'un champ, et ce qu'il faut pour le remplir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldKind {
    Text {
        #[serde(default)]
        multiline: bool,
        #[serde(default)]
        max_len: Option<u32>,
        /// Saisie masquée (mot de passe).
        #[serde(default)]
        password: bool,
        /// Une case par caractère (`max_len` cases).
        #[serde(default)]
        comb: bool,
        /// Date, nombre, masque… (champ « intelligent »).
        #[serde(default)]
        format: Option<FieldFormat>,
        /// Valeur calculée à partir d'autres champs.
        #[serde(default)]
        calc: Option<FieldCalc>,
    },
    Checkbox,
    /// Boutons radio : valeur d'export de chaque bouton, dans l'ordre.
    Radio {
        #[serde(default)]
        options: Vec<String>,
    },
    /// Liste déroulante (`list` faux) ou liste affichée : options proposées.
    Choice {
        #[serde(default)]
        options: Vec<String>,
        /// Liste déroulante où l'on peut aussi écrire.
        #[serde(default)]
        editable: bool,
        /// Liste affichée (plusieurs lignes) plutôt que déroulante.
        #[serde(default)]
        list: bool,
        /// Liste affichée où l'on peut choisir plusieurs options.
        #[serde(default)]
        multi: bool,
    },
    /// Emplacement d'une signature (signé dans le mode Signer).
    Signature,
}

impl FieldKind {
    /// Champ texte simple, sans format.
    pub fn text() -> Self {
        FieldKind::Text {
            multiline: false,
            max_len: None,
            password: false,
            comb: false,
            format: None,
            calc: None,
        }
    }

    /// Même genre (texte, case, radio, liste, signature), réglages à part.
    pub fn same_family(&self, other: &FieldKind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// Format du champ texte, s'il en a un.
    pub fn format(&self) -> Option<&FieldFormat> {
        match self {
            FieldKind::Text { format, .. } => format.as_ref(),
            _ => None,
        }
    }

    /// Calcul du champ texte, s'il en a un.
    pub fn calc(&self) -> Option<&FieldCalc> {
        match self {
            FieldKind::Text { calc, .. } => calc.as_ref(),
            _ => None,
        }
    }

    pub fn validate(&self, own_name: &str) -> Result<(), String> {
        match self {
            FieldKind::Text {
                max_len,
                comb,
                format,
                calc,
                ..
            } => {
                if let Some(max) = max_len
                    && (*max == 0 || *max as usize > MAX_FIELD_CHARS)
                {
                    return Err(format!("longueur entre 1 et {MAX_FIELD_CHARS}"));
                }
                if *comb && max_len.is_none_or(|m| m > 100) {
                    return Err("un champ en cases demande une longueur de 1 à 100".into());
                }
                if let Some(format) = format {
                    format.validate()?;
                }
                if let Some(calc) = calc {
                    calc.validate(own_name)?;
                }
                Ok(())
            }
            FieldKind::Choice { options, .. } | FieldKind::Radio { options } => {
                if options.len() > MAX_OPTIONS {
                    return Err(format!("{MAX_OPTIONS} options au plus"));
                }
                options.iter().try_for_each(|o| check_text(o))
            }
            FieldKind::Checkbox | FieldKind::Signature => Ok(()),
        }
    }

    /// La valeur convient au genre du champ (option existante, longueur).
    pub fn accepts(&self, value: &FieldValue) -> Result<(), String> {
        match (self, value) {
            (FieldKind::Text { max_len, .. }, FieldValue::Text(text)) => match max_len {
                Some(max) if text.chars().count() > *max as usize => {
                    Err(format!("{max} caractères au plus"))
                }
                _ => Ok(()),
            },
            (FieldKind::Checkbox, FieldValue::Checked(_)) => Ok(()),
            (FieldKind::Radio { options }, FieldValue::Choice(choice))
            | (
                FieldKind::Choice {
                    options,
                    editable: false,
                    ..
                },
                FieldValue::Choice(choice),
            ) if options.contains(choice) => Ok(()),
            (FieldKind::Choice { editable: true, .. }, FieldValue::Choice(_)) => Ok(()),
            (
                FieldKind::Choice {
                    options,
                    multi: true,
                    ..
                },
                FieldValue::Choices(items),
            ) if items.iter().all(|i| options.contains(i)) => Ok(()),
            _ => Err("valeur qui ne convient pas à ce champ".into()),
        }
    }
}

/// Apparence d'un champ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldLook {
    /// Corps du texte ; 0 : ajusté à la hauteur du champ.
    pub font_size: f64,
    pub align: Align,
    pub color: Color,
    /// Contour (aucun : `None`).
    pub border: Option<Color>,
    /// Fond (transparent : `None`).
    pub background: Option<Color>,
}

impl Default for FieldLook {
    fn default() -> Self {
        Self {
            font_size: 0.0,
            align: Align::Left,
            color: Color { r: 0, g: 0, b: 0 },
            border: Some(Color {
                r: 0x80,
                g: 0x8a,
                b: 0x96,
            }),
            background: None,
        }
    }
}

impl FieldLook {
    pub fn validate(&self) -> Result<(), String> {
        if !self.font_size.is_finite()
            || (self.font_size != 0.0 && !FIELD_FONT_RANGE.contains(&self.font_size))
        {
            return Err(format!(
                "corps automatique ou entre {} et {} pt",
                FIELD_FONT_RANGE.start(),
                FIELD_FONT_RANGE.end()
            ));
        }
        Ok(())
    }
}

/// Emplacement d'un champ sur une page du document arrangé.
#[derive(Debug, Clone, PartialEq)]
pub struct Widget {
    pub page: PageIndex,
    /// Place sur la page affichée (origine en bas).
    pub bounds: Rect,
    /// Bouton radio : rang de son option dans `FieldKind::Radio`.
    pub option: Option<usize>,
    /// Rang de l'emplacement parmi ceux du champ dans le document
    /// (désigne l'emplacement d'origine que l'on modifie).
    pub index: usize,
    /// Case à cocher ou bouton radio : nom de l'état « coché ».
    pub export: Option<String>,
    /// Élément de conception qui le décrit, s'il a été créé ou modifié.
    pub element: Option<ElementId>,
}

impl Widget {
    /// Emplacement d'origine `index` du champ.
    pub fn new(page: PageIndex, bounds: Rect, index: usize) -> Self {
        Self {
            page,
            bounds,
            option: None,
            index,
            export: None,
            element: None,
        }
    }
}

/// Champ du formulaire, tel que lu dans le document d'origine.
#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    /// Nom complet (« client.nom »), qui le désigne.
    pub name: String,
    pub kind: FieldKind,
    pub read_only: bool,
    pub required: bool,
    /// Infobulle (`/TU`) : le libellé lisible du champ.
    pub tooltip: String,
    pub look: FieldLook,
    /// Valeur dans le document d'origine.
    pub value: Option<FieldValue>,
    pub widgets: Vec<Widget>,
}

impl FormField {
    /// Champ sans emplacement ni réglage particulier.
    pub fn new(name: impl Into<String>, kind: FieldKind) -> Self {
        Self {
            name: name.into(),
            kind,
            read_only: false,
            required: false,
            tooltip: String::new(),
            look: FieldLook::default(),
            value: None,
            widgets: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{CalcOp, Separator};

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
        assert!(
            FieldValue::Choices(vec!["a".into(), "\u{0}".into()])
                .validate()
                .is_err()
        );
        let mut meta = Metadata::default();
        assert_eq!(meta.validate(), Ok(()));
        meta.title = "\u{0}".into();
        assert!(meta.validate().is_err());
    }

    #[test]
    fn values_must_suit_their_field() {
        let list = FieldKind::Choice {
            options: vec!["Paris".into(), "Lyon".into()],
            editable: false,
            list: true,
            multi: true,
        };
        assert!(list.accepts(&FieldValue::Choice("Lyon".into())).is_ok());
        assert!(list.accepts(&FieldValue::Choice("Rome".into())).is_err());
        assert!(
            list.accepts(&FieldValue::Choices(vec!["Paris".into(), "Lyon".into()]))
                .is_ok()
        );
        assert!(
            list.accepts(&FieldValue::Choices(vec!["Rome".into()]))
                .is_err()
        );
        let combo = FieldKind::Choice {
            options: vec!["Paris".into()],
            editable: true,
            list: false,
            multi: false,
        };
        assert!(combo.accepts(&FieldValue::Choice("Rome".into())).is_ok());
        assert!(
            FieldKind::Signature
                .accepts(&FieldValue::Text("x".into()))
                .is_err()
        );
        assert!(FieldKind::text().same_family(&FieldKind::Text {
            multiline: true,
            max_len: Some(3),
            password: false,
            comb: false,
            format: None,
            calc: None
        }));
        assert!(!FieldKind::text().same_family(&FieldKind::Checkbox));
    }

    #[test]
    fn kinds_and_looks_are_checked() {
        let total = FieldKind::Text {
            multiline: false,
            max_len: None,
            password: false,
            comb: false,
            format: Some(crate::formats::FieldFormat::Number {
                decimals: 2,
                separator: Separator::Comma,
                currency: " €".into(),
                currency_before: false,
            }),
            calc: Some(FieldCalc {
                op: CalcOp::Sum,
                fields: vec!["a".into()],
            }),
        };
        assert!(total.validate("total").is_ok());
        assert!(total.validate("a").is_err());
        let comb = FieldKind::Text {
            multiline: false,
            max_len: None,
            password: false,
            comb: true,
            format: None,
            calc: None,
        };
        assert!(comb.validate("x").is_err());
        let mut look = FieldLook::default();
        assert!(look.validate().is_ok());
        look.font_size = 2.0;
        assert!(look.validate().is_err());
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
        let kind: FieldKind =
            serde_json::from_value(serde_json::json!({ "kind": "text" })).unwrap();
        assert_eq!(kind, FieldKind::text());
        let json = serde_json::to_value(FieldKind::Signature).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "signature" }));
    }
}
