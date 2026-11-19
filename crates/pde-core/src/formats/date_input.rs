//! Lecture d'une date ou d'une heure tapée librement : nombres et mots de
//! la saisie (« 19 nov. 2026 », « 2026-11-19 », « 14h30 », « 2:30 pm »)
//! rangés selon l'ordre du motif, puis vérifiés (jours du mois, années
//! bissextiles, heures). Noms de mois en français ou en anglais.

use super::dates::Token;

pub(super) const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTHS_FR: [&str; 12] = [
    "janvier",
    "fevrier",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "aout",
    "septembre",
    "octobre",
    "novembre",
    "decembre",
];

fn fold(word: &str) -> String {
    word.chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'É' | 'È' | 'Ê' => 'e',
            'û' | 'ù' | 'Û' => 'u',
            'â' | 'à' | 'Â' => 'a',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// Mois désigné par un mot (« nov », « Novembre », « févr. ») : 1 à 12.
fn month_of(word: &str) -> Option<u32> {
    let word = fold(word);
    if word.chars().count() < 3 {
        return None;
    }
    // « ju » est ambigu, « jui » aussi : on exige un préfixe unique.
    let mut candidates = MONTHS_FR
        .iter()
        .chain(MONTHS_EN.iter())
        .enumerate()
        .filter(|(_, name)| fold(name).starts_with(&word))
        .map(|(i, _)| (i % 12) as u32 + 1);
    let first = candidates.next()?;
    candidates.all(|m| m == first).then_some(first)
}

/// Nombres et mots d'une saisie.
#[derive(Debug, PartialEq)]
enum Piece {
    Number(String),
    Word(String),
}

fn pieces(input: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut digits = false;
    let flush = |current: &mut String, digits: bool, out: &mut Vec<Piece>| {
        if !current.is_empty() {
            let text = std::mem::take(current);
            out.push(if digits {
                Piece::Number(text)
            } else {
                Piece::Word(text)
            });
        }
    };
    for c in input.chars() {
        if c.is_ascii_digit() || c.is_alphabetic() {
            let is_digit = c.is_ascii_digit();
            if is_digit != digits {
                flush(&mut current, digits, &mut out);
                digits = is_digit;
            }
            current.push(c);
        } else {
            flush(&mut current, digits, &mut out);
        }
    }
    flush(&mut current, digits, &mut out);
    out
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        _ => 28,
    }
}

fn full_year(text: &str) -> Option<u32> {
    let year: u32 = text.parse().ok()?;
    match text.len() {
        1 | 2 if year < 50 => Some(2000 + year),
        1 | 2 => Some(1900 + year),
        4 => Some(year),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct Moment {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// Ordre des nombres attendus par le motif : 'd', 'm', 'y' puis l'heure.
fn order(tokens: &[Token]) -> Vec<char> {
    let mut order = Vec::new();
    for token in tokens {
        let key = match token {
            Token::Day(_) => 'd',
            Token::Month(_) => 'm',
            Token::Year(_) => 'y',
            Token::Hour24(_) | Token::Hour12(_) => 'H',
            Token::Minute(_) => 'M',
            Token::Second(_) => 's',
            _ => continue,
        };
        if !order.contains(&key) {
            order.push(key);
        }
    }
    order
}

/// Huit ou six chiffres collés (« 19112026 ») : coupés selon le motif.
fn split_digits(text: &str, order: &[char]) -> Option<Vec<String>> {
    let date: Vec<char> = order
        .iter()
        .copied()
        .filter(|k| matches!(k, 'd' | 'm' | 'y'))
        .collect();
    let year_len = match text.len() {
        8 => 4,
        6 => 2,
        _ => return None,
    };
    if date.len() != 3 {
        return None;
    }
    let mut out = Vec::new();
    let mut rest = text;
    for key in date {
        let len = if key == 'y' { year_len } else { 2 };
        let (head, tail) = rest.split_at(len.min(rest.len()));
        out.push(head.to_string());
        rest = tail;
    }
    Some(out)
}

pub(super) fn read_date(pattern: &[Token], input: &str) -> Option<Moment> {
    let order = order(pattern);
    let parts = pieces(input);
    let mut numbers: Vec<String> = Vec::new();
    let mut month_word = None;
    let mut pm = None;
    for part in parts {
        match part {
            Piece::Number(n) => numbers.push(n),
            Piece::Word(w) if fold(&w) == "pm" => pm = Some(true),
            Piece::Word(w) if fold(&w) == "am" => pm = Some(false),
            Piece::Word(w) if fold(&w) == "h" => {}
            Piece::Word(w) => month_word = Some(month_of(&w)?),
        }
    }
    if numbers.len() == 1
        && month_word.is_none()
        && let Some(split) = split_digits(&numbers[0], &order)
    {
        numbers = split;
    }
    let mut m = Moment {
        year: 2000,
        month: 1,
        day: 1,
        ..Moment::default()
    };
    let mut keys: Vec<char> = order.clone();
    if let Some(month) = month_word {
        m.month = month;
        keys.retain(|k| *k != 'm');
    } else if numbers.first().is_some_and(|n| n.len() == 4) && numbers.len() >= 3 {
        // Écriture ISO (2026-11-19), quel que soit le motif.
        keys = vec!['y', 'm', 'd'];
        keys.extend(order.iter().filter(|k| matches!(k, 'H' | 'M' | 's')));
    } else if numbers.get(2).is_some_and(|n| n.len() == 4) && order.first() == Some(&'y') {
        // Année en dernier alors que le motif la met en tête : jour, mois, année.
        keys = vec!['d', 'm', 'y'];
        keys.extend(order.iter().filter(|k| matches!(k, 'H' | 'M' | 's')));
    }
    let wanted = keys.iter().filter(|k| !matches!(k, 's')).count();
    if numbers.len() < wanted || numbers.len() > keys.len() {
        return None;
    }
    for (key, text) in keys.iter().zip(&numbers) {
        let value: u32 = text.parse().ok()?;
        match key {
            'd' => m.day = value,
            'm' => m.month = value,
            'y' => m.year = full_year(text)?,
            'H' => m.hour = value,
            'M' => m.minute = value,
            _ => m.second = value,
        }
    }
    hour_of_day(&mut m, pm)?;
    let valid = (1..=12).contains(&m.month)
        && (1..=days_in_month(m.year, m.month)).contains(&m.day)
        && (1..=9999).contains(&m.year);
    valid.then_some(m)
}

/// Matin / après-midi appliqués, heure et minutes vérifiées.
fn hour_of_day(m: &mut Moment, pm: Option<bool>) -> Option<()> {
    match pm {
        Some(true) if (1..=11).contains(&m.hour) => m.hour += 12,
        Some(false) if m.hour == 12 => m.hour = 0,
        Some(_) if !(1..=12).contains(&m.hour) => return None,
        _ => {}
    }
    (m.hour < 24 && m.minute < 60 && m.second < 60).then_some(())
}

pub(super) fn read_time(pattern: &[Token], input: &str) -> Option<Moment> {
    let mut numbers = Vec::new();
    let mut pm = None;
    for part in pieces(input) {
        match part {
            Piece::Number(n) => numbers.push(n),
            Piece::Word(w) => match fold(&w).as_str() {
                "pm" | "p" => pm = Some(true),
                "am" | "a" => pm = Some(false),
                "h" => {}
                _ => return None,
            },
        }
    }
    // « 1430 » : heures et minutes collées.
    if numbers.len() == 1 && matches!(numbers[0].len(), 3 | 4) {
        let text = numbers.remove(0);
        let (h, m) = text.split_at(text.len() - 2);
        numbers = vec![h.to_string(), m.to_string()];
    }
    if numbers.is_empty() || numbers.len() > 3 {
        return None;
    }
    let mut values = numbers.iter().map(|n| n.parse::<u32>().ok());
    let mut m = Moment {
        hour: values.next()??,
        minute: values.next().unwrap_or(Some(0))?,
        second: values.next().unwrap_or(Some(0))?,
        ..Moment::default()
    };
    if !pattern.iter().any(|t| matches!(t, Token::Second(_))) {
        m.second = 0;
    }
    hour_of_day(&mut m, pm)?;
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_are_read_by_unique_prefix() {
        assert_eq!(month_of("ju"), None);
        assert_eq!(month_of("juil"), Some(7));
        assert_eq!(month_of("févr"), Some(2));
        assert_eq!(month_of("Nov."), None);
        assert_eq!(month_of("nov"), Some(11));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(full_year("62"), Some(1962));
        assert_eq!(full_year("05"), Some(2005));
    }
}
