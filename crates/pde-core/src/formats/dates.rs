//! Dates et heures au motif d'Acrobat (`dd/mm/yyyy`, `HH:MM`…) : saisie
//! souple (« 19/11/26 », « 2026-11-19 », « 19 novembre 2026 », « 14h30 »)
//! vérifiée puis écrite selon le motif. Les noms de mois s'écrivent en
//! anglais, comme le fait Acrobat avec le même motif ; ils se lisent en
//! français ou en anglais (lecture de la saisie : `date_input.rs`).

use super::date_input::{MONTHS_EN, Moment, read_date, read_time};

/// Élément d'un motif.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Token {
    Day(usize),
    Month(usize),
    Year(usize),
    Hour24(usize),
    Hour12(usize),
    Minute(usize),
    Second(usize),
    AmPm,
    Literal(char),
}

impl Token {
    fn is_date(&self) -> bool {
        matches!(self, Token::Day(_) | Token::Month(_) | Token::Year(_))
    }

    fn is_time(&self) -> bool {
        matches!(
            self,
            Token::Hour24(_) | Token::Hour12(_) | Token::Minute(_) | Token::Second(_) | Token::AmPm
        )
    }
}

fn tokens(pattern: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let run = chars[i..].iter().take_while(|&&x| x == c).count();
        let token = match (c, run) {
            ('d', 1 | 2) => Token::Day(run),
            ('m', 1..=4) => Token::Month(run),
            ('y', 2 | 4) => Token::Year(run),
            ('H', 1 | 2) => Token::Hour24(run),
            ('h', 1 | 2) => Token::Hour12(run),
            ('M', 1 | 2) => Token::Minute(run),
            ('s', 1 | 2) => Token::Second(run),
            ('t', 2) => Token::AmPm,
            (c, _) if c.is_alphabetic() => {
                return Err(format!("« {} » n'est pas compris dans le motif", c));
            }
            (c, _) => {
                out.extend(std::iter::repeat_n(Token::Literal(c), run));
                i += run;
                continue;
            }
        };
        out.push(token);
        i += run;
    }
    Ok(out)
}

pub(super) fn check_date_pattern(pattern: &str) -> Result<(), String> {
    if tokens(pattern)?.iter().any(Token::is_date) {
        Ok(())
    } else {
        Err("le motif de date ne contient ni jour, ni mois, ni année".into())
    }
}

pub(super) fn check_time_pattern(pattern: &str) -> Result<(), String> {
    let tokens = tokens(pattern)?;
    if tokens.iter().any(Token::is_date) {
        return Err("le motif d'heure contient une date".into());
    }
    if tokens.iter().any(Token::is_time) {
        Ok(())
    } else {
        Err("le motif d'heure ne contient ni heure ni minute".into())
    }
}

fn write(tokens: &[Token], m: Moment) -> String {
    let pad = |value: u32, width: usize| format!("{value:0width$}");
    let mut out = String::new();
    for token in tokens {
        match *token {
            Token::Day(n) => out += &pad(m.day, n),
            Token::Month(n) if n <= 2 => out += &pad(m.month, n),
            Token::Month(n) => {
                let name = MONTHS_EN[(m.month as usize).saturating_sub(1).min(11)];
                out += if n == 3 { &name[..3] } else { name };
            }
            Token::Year(2) => out += &pad(m.year % 100, 2),
            Token::Year(_) => out += &pad(m.year, 4),
            Token::Hour24(n) => out += &pad(m.hour, n),
            Token::Hour12(n) => out += &pad((m.hour + 11) % 12 + 1, n),
            Token::Minute(n) => out += &pad(m.minute, n),
            Token::Second(n) => out += &pad(m.second, n),
            Token::AmPm => out += if m.hour < 12 { "am" } else { "pm" },
            Token::Literal(c) => out.push(c),
        }
    }
    out
}

pub(super) fn normalize_date(pattern: &str, input: &str) -> Result<String, String> {
    let tokens = tokens(pattern)?;
    read_date(&tokens, input)
        .map(|m| write(&tokens, m))
        .ok_or_else(|| format!("date invalide (exemple : {})", sample(pattern)))
}

pub(super) fn normalize_time(pattern: &str, input: &str) -> Result<String, String> {
    let tokens = tokens(pattern)?;
    read_time(&tokens, input)
        .map(|m| write(&tokens, m))
        .ok_or_else(|| format!("heure invalide (exemple : {})", sample(pattern)))
}

/// Le motif appliqué au 19 novembre 2026, 14 h 30.
pub(super) fn sample(pattern: &str) -> String {
    let moment = Moment {
        year: 2026,
        month: 11,
        day: 19,
        hour: 14,
        minute: 30,
        second: 0,
    };
    tokens(pattern)
        .map(|t| write(&t, moment))
        .unwrap_or_else(|_| pattern.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(pattern: &str, input: &str) -> Result<String, String> {
        normalize_date(pattern, input)
    }

    #[test]
    fn dates_are_read_in_many_ways() {
        let p = "dd/mm/yyyy";
        for input in [
            "19/11/2026",
            "19-11-2026",
            "19.11.26",
            "19 11 2026",
            "2026-11-19",
            "19112026",
            "19 novembre 2026",
            "19 nov. 2026",
            "Nov 19, 2026",
        ] {
            assert_eq!(date(p, input).as_deref(), Ok("19/11/2026"), "{input}");
        }
        assert_eq!(date(p, "1/2/62").as_deref(), Ok("01/02/1962"));
        assert_eq!(
            date("mm/dd/yyyy", "11/19/2026").as_deref(),
            Ok("11/19/2026")
        );
        assert_eq!(
            date("yyyy-mm-dd", "19/11/2026").as_deref(),
            Ok("2026-11-19")
        );
        assert_eq!(
            date("d mmmm yyyy", "19/11/2026").as_deref(),
            Ok("19 November 2026")
        );
        assert_eq!(date("mm/yyyy", "11/2026").as_deref(), Ok("11/2026"));
    }

    #[test]
    fn impossible_dates_are_refused_with_an_example() {
        let p = "dd/mm/yyyy";
        for input in [
            "31/02/2026",
            "29/02/2025",
            "00/01/2026",
            "12/13/2026",
            "19",
            "demain",
            "1/2/3/4",
        ] {
            let error = date(p, input).unwrap_err();
            assert!(error.contains("19/11/2026"), "{input} : {error}");
        }
        assert_eq!(date(p, "29/02/2024").as_deref(), Ok("29/02/2024"));
    }

    #[test]
    fn times() {
        let t = |p: &str, i: &str| normalize_time(p, i);
        assert_eq!(t("HH:MM", "14h30").as_deref(), Ok("14:30"));
        assert_eq!(t("HH:MM", "9:05").as_deref(), Ok("09:05"));
        assert_eq!(t("HH:MM", "1430").as_deref(), Ok("14:30"));
        assert_eq!(t("HH:MM", "2:30 pm").as_deref(), Ok("14:30"));
        assert_eq!(t("h:MM tt", "14:30").as_deref(), Ok("2:30 pm"));
        assert_eq!(t("HH:MM:ss", "8:01:02").as_deref(), Ok("08:01:02"));
        assert!(t("HH:MM", "25:00").is_err());
        assert!(t("HH:MM", "midi").is_err());
        assert_eq!(sample("HH:MM"), "14:30");
    }

    #[test]
    fn patterns_are_checked() {
        assert!(check_date_pattern("dd/mm/yyyy").is_ok());
        assert!(check_date_pattern("m/d/yy h:MM tt").is_ok());
        assert!(check_date_pattern("HH:MM").is_err());
        assert!(check_date_pattern("ddd").is_err());
        assert!(check_time_pattern("HH:MM").is_ok());
        assert!(check_time_pattern("dd HH").is_err());
        assert!(check_time_pattern("--").is_err());
    }
}
