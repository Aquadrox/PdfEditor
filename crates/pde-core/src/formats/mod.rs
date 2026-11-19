//! Champs « intelligents » d'un formulaire (E-08, E-12) : date, heure,
//! nombre, montant, pourcentage, masque de saisie (téléphone, code postal),
//! adresse e-mail, et champs calculés (somme, moyenne…).
//!
//! Le PDF les décrit par les scripts standard d'Acrobat (`AFDate_FormatEx`,
//! `AFNumber_Format`, `AFSimple_Calculate`…) que les autres lecteurs
//! exécutent ; PdfEditor n'exécute jamais de JavaScript : il reconnaît ces
//! appels (`js.rs`) et fait le même travail lui-même (`dates.rs`,
//! `numbers.rs`, `mask.rs`, `calc.rs`). La valeur rangée dans le champ est
//! celle qu'attendent ces scripts : la date telle qu'affichée, le nombre
//! sans séparateur de milliers avec un point décimal.

mod calc;
mod date_input;
mod dates;
mod js;
mod mask;
mod numbers;

use serde::{Deserialize, Serialize};

pub use calc::{CalcOp, FieldCalc, MAX_CALC_FIELDS, parse_number};
pub use js::{FieldActions, read_actions, write_actions};

/// Longueur maximale d'un motif de date, d'heure ou d'un masque.
pub const MAX_PATTERN_CHARS: usize = 40;
/// Chiffres après la virgule acceptés.
pub const MAX_DECIMALS: u8 = 10;
/// Symbole monétaire le plus long accepté (« € », « EUR », « CHF »).
pub const MAX_CURRENCY_CHARS: usize = 8;

/// Écriture des nombres (le `sepStyle` d'Acrobat, de 0 à 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Separator {
    /// 1,234.56
    CommaDot,
    /// 1234.56
    Dot,
    /// 1.234,56
    DotComma,
    /// 1234,56
    #[default]
    Comma,
    /// 1'234.56
    ApostropheDot,
}

impl Separator {
    pub(crate) fn code(self) -> u8 {
        match self {
            Separator::CommaDot => 0,
            Separator::Dot => 1,
            Separator::DotComma => 2,
            Separator::Comma => 3,
            Separator::ApostropheDot => 4,
        }
    }

    pub(crate) fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            0 => Separator::CommaDot,
            1 => Separator::Dot,
            2 => Separator::DotComma,
            3 => Separator::Comma,
            4 => Separator::ApostropheDot,
            _ => return None,
        })
    }

    /// Séparateur des milliers (aucun : `None`) et marque décimale.
    pub(crate) fn marks(self) -> (Option<char>, char) {
        match self {
            Separator::CommaDot => (Some(','), '.'),
            Separator::Dot => (None, '.'),
            Separator::DotComma => (Some('.'), ','),
            Separator::Comma => (None, ','),
            Separator::ApostropheDot => (Some('\''), '.'),
        }
    }
}

/// Format d'un champ texte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldFormat {
    /// Date, motif d'Acrobat : `dd/mm/yyyy`, `d mmmm yyyy`…
    Date { pattern: String },
    /// Heure, motif d'Acrobat : `HH:MM`, `h:MM tt`…
    Time { pattern: String },
    /// Nombre ou montant : `currency` s'écrit tel quel avant ou après
    /// (« € » précédé d'une espace : `" €"`).
    Number {
        decimals: u8,
        #[serde(default)]
        separator: Separator,
        #[serde(default)]
        currency: String,
        #[serde(default)]
        currency_before: bool,
    },
    /// Pourcentage : le champ garde la fraction (0.15 pour 15 %).
    Percent {
        decimals: u8,
        #[serde(default)]
        separator: Separator,
    },
    /// Masque de saisie : `9` chiffre, `A` lettre, `O` lettre ou chiffre,
    /// `X` tout caractère ; le reste est écrit tel quel (« 99 99 99 99 99 »).
    Mask { mask: String },
    /// Adresse e-mail vérifiée.
    Email,
}

fn check_pattern(pattern: &str, what: &str) -> Result<(), String> {
    if pattern.is_empty() || pattern.chars().count() > MAX_PATTERN_CHARS {
        return Err(format!(
            "{what} : de 1 à {MAX_PATTERN_CHARS} caractères attendus"
        ));
    }
    if pattern
        .chars()
        .any(|c| c.is_control() || matches!(c, '"' | '\\' | '\''))
    {
        return Err(format!("{what} : caractère refusé"));
    }
    Ok(())
}

impl FieldFormat {
    /// Vérifie le format (venu de l'interface ou d'un fichier).
    pub fn validate(&self) -> Result<(), String> {
        match self {
            FieldFormat::Date { pattern } => {
                check_pattern(pattern, "motif de date")?;
                dates::check_date_pattern(pattern)
            }
            FieldFormat::Time { pattern } => {
                check_pattern(pattern, "motif d'heure")?;
                dates::check_time_pattern(pattern)
            }
            FieldFormat::Number {
                decimals, currency, ..
            } => {
                if *decimals > MAX_DECIMALS {
                    return Err(format!("{MAX_DECIMALS} décimales au plus"));
                }
                if currency.chars().count() > MAX_CURRENCY_CHARS
                    || currency
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '"' | '\\' | '\''))
                    || currency.chars().any(|c| c.is_ascii_digit())
                {
                    return Err("symbole monétaire invalide".into());
                }
                Ok(())
            }
            FieldFormat::Percent { decimals, .. } if *decimals > MAX_DECIMALS => {
                Err(format!("{MAX_DECIMALS} décimales au plus"))
            }
            FieldFormat::Percent { .. } | FieldFormat::Email => Ok(()),
            FieldFormat::Mask { mask } => {
                check_pattern(mask, "masque")?;
                if !mask.chars().any(mask::is_placeholder) {
                    return Err("le masque ne demande aucun caractère (9, A, O ou X)".into());
                }
                Ok(())
            }
        }
    }

    /// La saisie de l'utilisateur, vérifiée et mise en forme : la valeur à
    /// ranger dans le champ. Une saisie vide reste vide.
    pub fn normalize(&self, input: &str) -> Result<String, String> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(String::new());
        }
        match self {
            FieldFormat::Date { pattern } => dates::normalize_date(pattern, input),
            FieldFormat::Time { pattern } => dates::normalize_time(pattern, input),
            FieldFormat::Number {
                decimals,
                separator,
                ..
            } => numbers::normalize(input, *decimals, *separator, false),
            FieldFormat::Percent {
                decimals,
                separator,
            } => numbers::normalize(input, *decimals, *separator, true),
            FieldFormat::Mask { mask } => mask::apply(mask, input),
            FieldFormat::Email => normalize_email(input),
        }
    }

    /// La valeur rangée, telle qu'un lecteur l'affiche (« 1234.5 » →
    /// « 1234,50 € ») ; une valeur illisible est montrée telle quelle.
    pub fn display(&self, value: &str) -> String {
        match self {
            FieldFormat::Number {
                decimals,
                separator,
                currency,
                currency_before,
            } => numbers::display(value, *decimals, *separator, false)
                .map(|n| {
                    if *currency_before {
                        format!("{currency}{n}")
                    } else {
                        format!("{n}{currency}")
                    }
                })
                .unwrap_or_else(|| value.to_string()),
            FieldFormat::Percent {
                decimals,
                separator,
            } => numbers::display(value, *decimals, *separator, true)
                .map(|n| format!("{n}%"))
                .unwrap_or_else(|| value.to_string()),
            _ => value.to_string(),
        }
    }

    /// Résultat d'un calcul, tel que rangé dans un champ de ce format.
    pub fn canonical(&self, value: f64) -> String {
        match self {
            FieldFormat::Number { decimals, .. } => numbers::plain(value, usize::from(*decimals)),
            FieldFormat::Percent { decimals, .. } => {
                numbers::plain(value, usize::from(*decimals) + 2)
            }
            _ => plain_number(value),
        }
    }

    /// Exemple de saisie attendue, pour les messages et l'aide.
    pub fn example(&self) -> String {
        match self {
            FieldFormat::Date { pattern } | FieldFormat::Time { pattern } => dates::sample(pattern),
            FieldFormat::Number { .. } | FieldFormat::Percent { .. } => {
                self.display(&self.normalize("1234.5").unwrap_or_default())
            }
            FieldFormat::Mask { mask } => mask::sample(mask),
            FieldFormat::Email => "nom@exemple.fr".into(),
        }
    }
}

/// Nombre sans mise en forme (« 12.5 », « 3 »), six décimales au plus.
pub fn plain_number(value: f64) -> String {
    let text = numbers::plain(value, 6);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" {
        "0".into()
    } else {
        text.to_string()
    }
}

fn normalize_email(input: &str) -> Result<String, String> {
    let invalid = || Err("adresse e-mail invalide (exemple : nom@exemple.fr)".to_string());
    if input.chars().any(char::is_whitespace) || input.chars().count() > 254 {
        return invalid();
    }
    let Some((local, domain)) = input.split_once('@') else {
        return invalid();
    };
    let tld = domain.rsplit('.').next().unwrap_or_default();
    if local.is_empty()
        || domain.contains('@')
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.contains("..")
        || tld.chars().count() < 2
    {
        return invalid();
    }
    Ok(input.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(decimals: u8, separator: Separator, currency: &str) -> FieldFormat {
        FieldFormat::Number {
            decimals,
            separator,
            currency: currency.into(),
            currency_before: false,
        }
    }

    #[test]
    fn formats_are_checked() {
        let ok = [
            FieldFormat::Date {
                pattern: "dd/mm/yyyy".into(),
            },
            FieldFormat::Time {
                pattern: "HH:MM".into(),
            },
            number(2, Separator::Comma, " €"),
            FieldFormat::Mask {
                mask: "99 99 99 99 99".into(),
            },
            FieldFormat::Email,
        ];
        for format in ok {
            assert_eq!(format.validate(), Ok(()), "{format:?}");
        }
        let wrong = [
            FieldFormat::Date {
                pattern: "dd\"); app.alert(1); (\"".into(),
            },
            FieldFormat::Date {
                pattern: String::new(),
            },
            FieldFormat::Date {
                pattern: "qq/zz".into(),
            },
            number(11, Separator::Comma, ""),
            number(2, Separator::Comma, "1€"),
            FieldFormat::Mask { mask: "--".into() },
        ];
        for format in wrong {
            assert!(format.validate().is_err(), "{format:?}");
        }
    }

    #[test]
    fn email_addresses() {
        let email = FieldFormat::Email;
        assert_eq!(
            email.normalize(" jean.dupont@exemple.fr ").unwrap(),
            "jean.dupont@exemple.fr"
        );
        for wrong in [
            "jean",
            "jean@",
            "@exemple.fr",
            "a@b",
            "a b@c.fr",
            "a@b.c",
            "a@@b.fr",
        ] {
            assert!(email.normalize(wrong).is_err(), "{wrong}");
        }
        assert_eq!(email.normalize("  ").unwrap(), "");
    }

    #[test]
    fn amounts_are_stored_plain_and_shown_formatted() {
        let euros = number(2, Separator::Comma, " €");
        let stored = euros.normalize("1 234,5 €").unwrap();
        assert_eq!(stored, "1234.50");
        assert_eq!(euros.display(&stored), "1234,50 €");
        let dollars = FieldFormat::Number {
            decimals: 2,
            separator: Separator::CommaDot,
            currency: "$".into(),
            currency_before: true,
        };
        assert_eq!(dollars.display("1234567.891"), "$1,234,567.89");
        assert_eq!(dollars.example(), "$1,234.50");
        assert_eq!(euros.display("pas un nombre"), "pas un nombre");
        let percent = FieldFormat::Percent {
            decimals: 1,
            separator: Separator::Comma,
        };
        assert_eq!(percent.normalize("15,5 %").unwrap(), "0.155");
        assert_eq!(percent.display("0.155"), "15,5%");
        assert_eq!(euros.canonical(12.345), "12.35");
        assert_eq!(percent.canonical(0.5), "0.500");
        assert_eq!(plain_number(12.5), "12.5");
        assert_eq!(plain_number(3.0), "3");
        assert_eq!(plain_number(-0.0000001), "0");
    }

    #[test]
    fn json_for_the_interface() {
        let json = serde_json::to_value(number(2, Separator::Comma, " €")).unwrap();
        assert_eq!(json["kind"], "number");
        assert_eq!(json["separator"], "comma");
        let back: FieldFormat =
            serde_json::from_value(serde_json::json!({ "kind": "number", "decimals": 0 })).unwrap();
        assert_eq!(back, number(0, Separator::Comma, ""));
    }
}
