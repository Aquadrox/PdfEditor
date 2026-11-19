//! Champs calculés (`AFSimple_Calculate` d'Acrobat) : somme, produit,
//! moyenne, minimum ou maximum d'autres champs. Un champ vide ou illisible
//! compte pour zéro, comme dans Acrobat.

use serde::{Deserialize, Serialize};

pub use super::numbers::parse_number;

/// Champs qu'un calcul peut réunir.
pub const MAX_CALC_FIELDS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalcOp {
    Sum,
    Product,
    Average,
    Min,
    Max,
}

impl CalcOp {
    /// Nom dans `AFSimple_Calculate`.
    pub(crate) fn js_name(self) -> &'static str {
        match self {
            CalcOp::Sum => "SUM",
            CalcOp::Product => "PRD",
            CalcOp::Average => "AVG",
            CalcOp::Min => "MIN",
            CalcOp::Max => "MAX",
        }
    }

    pub(crate) fn from_js_name(name: &str) -> Option<Self> {
        Some(match name.to_ascii_uppercase().as_str() {
            "SUM" => CalcOp::Sum,
            "PRD" => CalcOp::Product,
            "AVG" => CalcOp::Average,
            "MIN" => CalcOp::Min,
            "MAX" => CalcOp::Max,
            _ => return None,
        })
    }
}

/// Valeur d'un champ tirée d'autres champs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldCalc {
    pub op: CalcOp,
    /// Noms complets des champs réunis.
    pub fields: Vec<String>,
}

impl FieldCalc {
    /// `own` : le nom du champ calculé (il ne peut pas se citer).
    pub fn validate(&self, own: &str) -> Result<(), String> {
        if self.fields.is_empty() || self.fields.len() > MAX_CALC_FIELDS {
            return Err(format!("de 1 à {MAX_CALC_FIELDS} champs à calculer"));
        }
        if self.fields.iter().any(|f| f == own) {
            return Err("un champ ne peut pas entrer dans son propre calcul".into());
        }
        if self
            .fields
            .iter()
            .any(|f| f.is_empty() || f.chars().any(|c| c.is_control() || c == '"' || c == '\\'))
        {
            return Err("nom de champ invalide dans le calcul".into());
        }
        Ok(())
    }

    /// Le résultat, `value(nom)` donnant le texte de chaque champ.
    pub fn evaluate<'a>(&self, value: impl Fn(&str) -> Option<&'a str>) -> f64 {
        let numbers: Vec<f64> = self
            .fields
            .iter()
            .map(|name| {
                value(name)
                    .and_then(|v| parse_number(v, None))
                    .unwrap_or(0.0)
            })
            .collect();
        let result = match self.op {
            CalcOp::Sum => numbers.iter().sum(),
            CalcOp::Product => numbers.iter().product(),
            CalcOp::Average => numbers.iter().sum::<f64>() / numbers.len().max(1) as f64,
            CalcOp::Min => numbers.iter().copied().fold(f64::INFINITY, f64::min),
            CalcOp::Max => numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        };
        if result.is_finite() { result } else { 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn results_count_empty_fields_as_zero() {
        let values: HashMap<&str, &str> = [("a", "10.5"), ("b", "2"), ("c", "")].into();
        let calc = |op| FieldCalc {
            op,
            fields: vec!["a".into(), "b".into(), "c".into(), "absent".into()],
        };
        let get = |name: &str| values.get(name).copied();
        assert_eq!(calc(CalcOp::Sum).evaluate(get), 12.5);
        assert_eq!(calc(CalcOp::Product).evaluate(get), 0.0);
        assert_eq!(calc(CalcOp::Average).evaluate(get), 12.5 / 4.0);
        assert_eq!(calc(CalcOp::Min).evaluate(get), 0.0);
        assert_eq!(calc(CalcOp::Max).evaluate(get), 10.5);
        assert!(calc(CalcOp::Sum).validate("total").is_ok());
        assert!(calc(CalcOp::Sum).validate("a").is_err());
        let empty = FieldCalc {
            op: CalcOp::Sum,
            fields: Vec::new(),
        };
        assert!(empty.validate("total").is_err());
        assert_eq!(CalcOp::from_js_name("prd"), Some(CalcOp::Product));
    }
}
