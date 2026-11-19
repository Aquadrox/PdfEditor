//! Nombres, montants et pourcentages : lecture souple (« 1 234,5 € »,
//! « 1,234.50 », « (12) »), valeur rangée sans séparateur de milliers avec
//! un point décimal (ce que lisent les scripts d'Acrobat), affichage selon
//! l'écriture choisie.

use super::Separator;

/// Plus grande valeur absolue acceptée (au-delà, les décimales se perdent).
const MAX_ABS: f64 = 1e15;

/// Lit un nombre saisi ; `separator` : l'écriture attendue, qui tranche
/// « 1,234 » (mille deux cent trente-quatre ou un virgule deux…).
pub fn parse_number(input: &str, separator: Option<Separator>) -> Option<f64> {
    let trimmed = input.trim();
    let negative_paren = trimmed.starts_with('(') && trimmed.ends_with(')');
    let mut negative = negative_paren;
    let mut text = String::new();
    for c in trimmed.chars() {
        match c {
            '0'..='9' | ',' | '.' => text.push(c),
            '-' | '−' if text.is_empty() => negative = true,
            // Espaces, apostrophes, symboles monétaires, signe « + » et « % ».
            _ => {}
        }
    }
    if text.is_empty() || !text.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let group = separator.and_then(|s| s.marks().0);
    let commas = text.matches(',').count();
    let dots = text.matches('.').count();
    let decimal = match (commas, dots) {
        (0, 0) => None,
        (_, 0) if commas > 1 => None,
        (0, _) if dots > 1 => None,
        (1, 0) => (!looks_grouped(&text, ',', group)).then_some(','),
        (0, 1) => (!looks_grouped(&text, '.', group)).then_some('.'),
        _ => {
            let last_comma = text.rfind(',');
            let last_dot = text.rfind('.');
            Some(if last_comma > last_dot { ',' } else { '.' })
        }
    };
    let plain: String = text
        .chars()
        .filter_map(|c| match c {
            ',' | '.' if Some(c) == decimal => Some('.'),
            ',' | '.' => None,
            c => Some(c),
        })
        .collect();
    let value: f64 = plain.parse().ok()?;
    let value = if negative { -value } else { value };
    (value.is_finite() && value.abs() < MAX_ABS).then_some(value)
}

/// Une seule marque suivie de trois chiffres, et c'est la marque des
/// milliers de l'écriture attendue : « 1,234 » vaut mille deux cent…
fn looks_grouped(text: &str, mark: char, group: Option<char>) -> bool {
    group == Some(mark)
        && text
            .split_once(mark)
            .is_some_and(|(before, after)| after.len() == 3 && (1..=3).contains(&before.len()))
}

pub(super) fn plain(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    // « -0.00 » : le signe d'un zéro arrondi ne veut rien dire.
    if text.starts_with('-') && text[1..].chars().all(|c| c == '0' || c == '.') {
        text[1..].to_string()
    } else {
        text
    }
}

pub(super) fn normalize(
    input: &str,
    decimals: u8,
    separator: Separator,
    percent: bool,
) -> Result<String, String> {
    let value = parse_number(input, Some(separator)).ok_or_else(|| {
        if percent {
            "pourcentage invalide (exemple : 15,5 %)".to_string()
        } else {
            "nombre invalide (exemple : 1234,50)".to_string()
        }
    })?;
    if percent {
        Ok(plain(value / 100.0, usize::from(decimals) + 2))
    } else {
        Ok(plain(value, usize::from(decimals)))
    }
}

pub(super) fn display(
    value: &str,
    decimals: u8,
    separator: Separator,
    percent: bool,
) -> Option<String> {
    let mut number = parse_number(value, Some(Separator::Dot))?;
    if percent {
        number *= 100.0;
    }
    let text = plain(number, usize::from(decimals));
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text.as_str()),
    };
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let (group, mark) = separator.marks();
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if let Some(group) = group
            && i > 0
            && (int.len() - i).is_multiple_of(3)
        {
            grouped.push(group);
        }
        grouped.push(c);
    }
    Some(if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}{mark}{frac}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_read_whatever_the_writing() {
        let read = |s: &str, sep| parse_number(s, Some(sep));
        assert_eq!(read("1 234,5 €", Separator::Comma), Some(1234.5));
        assert_eq!(read("1.234,56", Separator::DotComma), Some(1234.56));
        assert_eq!(read("1,234.56", Separator::Comma), Some(1234.56));
        assert_eq!(read("1'234.5", Separator::ApostropheDot), Some(1234.5));
        assert_eq!(read("1,234", Separator::CommaDot), Some(1234.0));
        assert_eq!(read("1,234", Separator::Comma), Some(1.234));
        assert_eq!(read("1.234", Separator::DotComma), Some(1234.0));
        assert_eq!(read("1.234", Separator::Dot), Some(1.234));
        assert_eq!(read("1.234.567", Separator::Comma), Some(1_234_567.0));
        assert_eq!(read("-12,5", Separator::Comma), Some(-12.5));
        assert_eq!(read("(12)", Separator::Comma), Some(-12.0));
        assert_eq!(read("$ 3", Separator::Comma), Some(3.0));
        assert_eq!(parse_number("abc", None), None);
        assert_eq!(parse_number("", None), None);
        assert_eq!(parse_number("1e300", None), Some(1300.0));
        assert_eq!(parse_number("99999999999999999", None), None);
    }

    #[test]
    fn stored_values_and_display() {
        assert_eq!(
            normalize("12,345", 2, Separator::Comma, false).unwrap(),
            "12.35"
        );
        assert_eq!(
            normalize("-0,001", 2, Separator::Comma, false).unwrap(),
            "0.00"
        );
        assert_eq!(normalize("7", 0, Separator::Comma, false).unwrap(), "7");
        assert!(normalize("douze", 2, Separator::Comma, false).is_err());
        assert_eq!(
            normalize("20 %", 0, Separator::Comma, true).unwrap(),
            "0.20"
        );
        let show = |v: &str, sep| display(v, 2, sep, false);
        assert_eq!(
            show("1234567.891", Separator::CommaDot).unwrap(),
            "1,234,567.89"
        );
        assert_eq!(
            show("1234567.891", Separator::DotComma).unwrap(),
            "1.234.567,89"
        );
        assert_eq!(show("1234567.891", Separator::Comma).unwrap(), "1234567,89");
        assert_eq!(
            show("-1234", Separator::ApostropheDot).unwrap(),
            "-1'234.00"
        );
        assert_eq!(
            display("123", 0, Separator::CommaDot, false).unwrap(),
            "123"
        );
        assert_eq!(display("0.5", 0, Separator::Comma, true).unwrap(), "50");
    }
}
