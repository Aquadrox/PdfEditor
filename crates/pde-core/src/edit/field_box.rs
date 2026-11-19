//! Conception de formulaires (E-12) : un champ créé, ou un champ du
//! document modifié ou supprimé, est un élément comme les autres — un
//! emplacement sur une page et les réglages de son champ. Plusieurs
//! éléments du même nom forment un seul champ (boutons radio d'un groupe,
//! même champ répété sur plusieurs pages).

use serde::{Deserialize, Serialize};

use super::form::{FieldKind, FieldLook, FieldValue, MAX_TOOLTIP_CHARS};
use crate::document::PageIndex;
use crate::geometry::{Point, Rect};

/// Côté minimal d'un champ (points).
pub const MIN_FIELD_SIDE: f64 = 4.0;
/// Nom de champ le plus long accepté.
pub const MAX_FIELD_NAME_CHARS: usize = 200;
/// Valeur d'export la plus longue (case à cocher, bouton radio).
pub const MAX_EXPORT_CHARS: usize = 100;

/// Emplacement d'un champ du document que l'élément remplace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldOrigin {
    /// Nom complet du champ dans le document d'origine.
    pub name: String,
    /// Rang de l'emplacement parmi ceux du champ.
    pub widget: usize,
}

/// Emplacement d'un champ et réglages de ce champ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldBox {
    pub page: PageIndex,
    /// Coin haut gauche.
    pub top_left: Point,
    pub width: f64,
    pub height: f64,
    /// Nom complet du champ (le désigne ; les boutons d'un groupe radio le partagent).
    pub name: String,
    pub kind: FieldKind,
    #[serde(default)]
    pub tooltip: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub look: FieldLook,
    /// Valeur initiale d'un champ créé.
    #[serde(default)]
    pub value: Option<FieldValue>,
    /// Case à cocher ou bouton radio : valeur quand il est coché.
    #[serde(default)]
    pub export: String,
    /// Emplacement du document remplacé ; aucun : champ créé.
    #[serde(default)]
    pub origin: Option<FieldOrigin>,
    /// Emplacement du document supprimé (avec `origin`).
    #[serde(default)]
    pub deleted: bool,
}

/// Caractères refusés dans un nom PDF (valeur d'export).
fn bad_export_char(c: char) -> bool {
    c.is_whitespace() || c.is_control() || "()<>[]{}/%#".contains(c)
}

pub(crate) fn check_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("le champ doit avoir un nom".into());
    }
    if name.chars().count() > MAX_FIELD_NAME_CHARS {
        return Err(format!(
            "nom de champ de {MAX_FIELD_NAME_CHARS} caractères au plus"
        ));
    }
    if name
        .chars()
        .any(|c| c.is_control() || c == '"' || c == '\\')
    {
        return Err("caractère refusé dans le nom du champ".into());
    }
    Ok(())
}

impl FieldBox {
    pub fn rect(&self) -> Rect {
        Rect::from_corners(
            Point::new(self.top_left.x, self.top_left.y - self.height),
            Point::new(self.top_left.x + self.width, self.top_left.y),
        )
    }

    /// Bouton (case à cocher, radio) dont l'état « coché » est `export`.
    pub fn is_button(&self) -> bool {
        matches!(self.kind, FieldKind::Checkbox | FieldKind::Radio { .. })
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let numbers = [self.top_left.x, self.top_left.y, self.width, self.height];
        if numbers.iter().any(|v| !v.is_finite()) {
            return Err("position ou taille du champ invalide".into());
        }
        if self.deleted {
            return match self.origin {
                Some(_) => Ok(()),
                None => Err("seul un champ du document se supprime ainsi".into()),
            };
        }
        if self.width < MIN_FIELD_SIDE || self.height < MIN_FIELD_SIDE {
            return Err(format!("champ trop petit (minimum {MIN_FIELD_SIDE} pt)"));
        }
        check_name(&self.name)?;
        // Un point sépare les niveaux d'un nom : réservé aux champs du document.
        if self.origin.is_none() && self.name.contains('.') {
            return Err("le nom d'un nouveau champ ne contient pas de point".into());
        }
        if self.tooltip.chars().count() > MAX_TOOLTIP_CHARS
            || self.tooltip.chars().any(|c| c.is_control() && c != '\n')
        {
            return Err("infobulle invalide".into());
        }
        self.kind.validate(&self.name)?;
        self.look.validate()?;
        if self.is_button()
            && self.origin.is_none()
            && (self.export.is_empty()
                || self.export.chars().count() > MAX_EXPORT_CHARS
                || self.export == "Off"
                || self.export.chars().any(bad_export_char))
        {
            return Err("valeur du bouton invalide (lettres et chiffres, sans espace)".into());
        }
        if let Some(value) = &self.value {
            value.validate()?;
            if !matches!(self.kind, FieldKind::Radio { .. }) {
                self.kind.accepts(value)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn field(name: &str, kind: FieldKind) -> FieldBox {
        FieldBox {
            page: 0,
            top_left: Point::new(100.0, 700.0),
            width: 150.0,
            height: 20.0,
            name: name.into(),
            kind,
            tooltip: String::new(),
            required: false,
            read_only: false,
            look: FieldLook::default(),
            value: None,
            export: String::new(),
            origin: None,
            deleted: false,
        }
    }

    #[test]
    fn new_fields_are_checked() {
        assert_eq!(field("nom", FieldKind::text()).validate(), Ok(()));
        assert_eq!(
            field("nom", FieldKind::text()).rect().min(),
            Point::new(100.0, 680.0)
        );
        let cases: [fn(&mut FieldBox); 8] = [
            |f| f.name = " ".into(),
            |f| f.name = "client.nom".into(),
            |f| f.width = 1.0,
            |f| f.top_left.x = f64::INFINITY,
            |f| f.tooltip = "\u{1}".into(),
            |f| f.look.font_size = 500.0,
            |f| f.value = Some(FieldValue::Checked(true)),
            |f| f.deleted = true,
        ];
        for change in cases {
            let mut f = field("nom", FieldKind::text());
            change(&mut f);
            assert!(f.validate().is_err(), "{f:?}");
        }
    }

    #[test]
    fn buttons_need_an_export_value() {
        let mut checkbox = field("accord", FieldKind::Checkbox);
        assert!(checkbox.validate().is_err());
        checkbox.export = "Oui".into();
        assert_eq!(checkbox.validate(), Ok(()));
        for wrong in ["Off", "a b", "a/b", "(x)"] {
            checkbox.export = wrong.into();
            assert!(checkbox.validate().is_err(), "{wrong}");
        }
        let mut radio = field(
            "civilite",
            FieldKind::Radio {
                options: Vec::new(),
            },
        );
        radio.export = "Mme".into();
        radio.value = Some(FieldValue::Choice("Mme".into()));
        assert_eq!(radio.validate(), Ok(()));
    }

    #[test]
    fn document_fields_keep_their_dotted_names_and_can_be_deleted() {
        let mut f = field("client.nom", FieldKind::text());
        f.origin = Some(FieldOrigin {
            name: "client.nom".into(),
            widget: 0,
        });
        assert_eq!(f.validate(), Ok(()));
        f.deleted = true;
        f.name = String::new();
        assert_eq!(f.validate(), Ok(()));
    }

    #[test]
    fn json_for_the_interface() {
        let json = serde_json::json!({
            "page": 0, "top_left": { "x": 10.0, "y": 50.0 }, "width": 80.0, "height": 18.0,
            "name": "date", "kind": { "kind": "text", "format": { "kind": "date", "pattern": "dd/mm/yyyy" } }
        });
        let f: FieldBox = serde_json::from_value(json).unwrap();
        assert_eq!(f.validate(), Ok(()));
        assert!(f.kind.format().is_some());
        assert!(f.origin.is_none() && !f.required);
    }
}
