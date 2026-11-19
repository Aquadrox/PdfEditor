//! Dates et heures au motif d'Acrobat (`dd/mm/yyyy`, `HH:MM`…) : saisie
//! souple (« 19/11/26 », « 2026-11-19 », « 19 novembre 2026 », « 14h30 »)
//! vérifiée puis écrite selon le motif. Les noms de mois s'écrivent en
//! anglais, comme le fait Acrobat avec le même motif ; ils se lisent en
//! français ou en anglais.

/// Élément d'un motif.
#[derive(Debug, Clone, PartialEq)]
enum Token {
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

const MONTHS_EN: [&str; 12] = [
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
struct Moment {
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
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

fn read_date(pattern: &[Token], input: &str) -> Option<Moment> {
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

fn read_time(pattern: &[Token], input: &str) -> Option<Moment> {
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
        assert_eq!(month_of("ju"), None);
        assert_eq!(month_of("juil"), Some(7));
        assert_eq!(month_of("févr"), Some(2));
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
