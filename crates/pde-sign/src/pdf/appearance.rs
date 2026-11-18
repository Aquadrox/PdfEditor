//! Apparence d'une signature visible : quelques lignes de texte dans le
//! cadre du champ (Helvetica, police standard du PDF). L'apparence
//! manuscrite (dessinée, tapée, importée) viendra avec S-03.

use lopdf::{Dictionary, Object, Stream};

/// Corps de police : borné pour rester lisible.
const MIN_SIZE: f32 = 4.0;
const MAX_SIZE: f32 = 14.0;
/// Largeur moyenne d'un caractère Helvetica, en fraction du corps (prudente).
const AVERAGE_WIDTH: f32 = 0.56;
const LEADING: f32 = 1.25;
const PADDING: f32 = 2.0;

/// Flux d'apparence (XObject de formulaire) pour un cadre `width` × `height`
/// en points ; `font` désigne l'objet police Helvetica.
pub(crate) fn text_appearance(
    lines: &[String],
    width: f32,
    height: f32,
    font: lopdf::ObjectId,
) -> Stream {
    let mut content = String::new();
    let lines: Vec<&String> = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if !lines.is_empty() {
        let longest = lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(1)
            .max(1) as f32;
        let by_height = (height - 2.0 * PADDING) / (lines.len() as f32 * LEADING);
        let by_width = (width - 2.0 * PADDING) / (longest * AVERAGE_WIDTH);
        let size = by_height.min(by_width).clamp(MIN_SIZE, MAX_SIZE);
        let block = lines.len() as f32 * size * LEADING;
        // Bloc centré verticalement ; première ligne en haut.
        let top = (height + block) / 2.0 - size;
        content.push_str("q\nBT\n0.11 0.14 0.19 rg\n");
        content.push_str(&format!("/F1 {size:.2} Tf\n{:.2} TL\n", size * LEADING));
        content.push_str(&format!("{PADDING:.2} {top:.2} Td\n"));
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                content.push_str("T*\n");
            }
            content.push('(');
            content.push_str(&literal(&win_ansi(line)));
            content.push_str(") Tj\n");
        }
        content.push_str("ET\nQ\n");
    }
    let mut fonts = Dictionary::new();
    fonts.set("F1", font);
    let mut resources = Dictionary::new();
    resources.set("Font", fonts);
    let mut dictionary = Dictionary::new();
    dictionary.set("Type", Object::Name(b"XObject".to_vec()));
    dictionary.set("Subtype", Object::Name(b"Form".to_vec()));
    dictionary.set(
        "BBox",
        vec![
            0.into(),
            0.into(),
            Object::Real(width),
            Object::Real(height),
        ],
    );
    dictionary.set("Resources", resources);
    Stream::new(dictionary, latin_bytes(&content))
}

/// Police Helvetica, encodage WinAnsi (accents d'Europe de l'Ouest).
pub(crate) fn helvetica() -> Object {
    let mut font = Dictionary::new();
    font.set("Type", Object::Name(b"Font".to_vec()));
    font.set("Subtype", Object::Name(b"Type1".to_vec()));
    font.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
    font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
    Object::Dictionary(font)
}

/// Caractères en WinAnsi (un caractère = un octet, `?` hors du jeu).
fn win_ansi(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| match c {
            ' '..='~' => c as u8,
            '\u{a0}'..='\u{ff}' => c as u32 as u8,
            '€' => 0x80,
            '…' => 0x85,
            'Š' => 0x8A,
            'Œ' => 0x8C,
            'Ž' => 0x8E,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '–' => 0x96,
            '—' => 0x97,
            'š' => 0x9A,
            'œ' => 0x9C,
            'ž' => 0x9E,
            'Ÿ' => 0x9F,
            _ => b'?',
        })
        .collect()
}

/// Octets échappés pour une chaîne littérale, gardés dans un `String`
/// caractère par caractère (octet n → U+00nn) jusqu'à l'écriture.
fn literal(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            _ => out.push(char::from(b)),
        }
    }
    out
}

/// Retour aux octets (chaque caractère vaut moins de 256).
fn latin_bytes(text: &str) -> Vec<u8> {
    text.chars().map(|c| c as u32 as u8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_and_typography_use_win_ansi() {
        assert_eq!(win_ansi("Hélène"), b"H\xe9l\xe8ne");
        assert_eq!(win_ansi("l’œuvre — 5 €"), b"l\x92\x9cuvre \x97 5 \x80");
        assert_eq!(win_ansi("中"), b"?");
    }

    #[test]
    fn parentheses_are_escaped() {
        assert_eq!(literal(b"a(b)\\"), "a\\(b\\)\\\\");
    }

    #[test]
    fn appearance_fits_its_box() {
        let lines = vec![
            "Signé par Claire Martin".to_string(),
            "18/11/2026".to_string(),
        ];
        let stream = text_appearance(&lines, 200.0, 50.0, (9, 0));
        let has = |needle: &[u8]| stream.content.windows(needle.len()).any(|w| w == needle);
        assert!(has(b"(Sign\xe9 par Claire Martin) Tj"));
        let content = String::from_utf8_lossy(&stream.content).to_string();
        assert!(content.contains("T*"));
        let size: f32 = content
            .split("/F1 ")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|s| s.parse().ok())
            .unwrap();
        assert!((MIN_SIZE..=MAX_SIZE).contains(&size));
        assert!(size * 23.0 * AVERAGE_WIDTH <= 200.0);
    }

    #[test]
    fn empty_lines_give_an_empty_appearance() {
        let stream = text_appearance(&[" ".into()], 100.0, 20.0, (9, 0));
        assert!(stream.content.is_empty());
    }
}
