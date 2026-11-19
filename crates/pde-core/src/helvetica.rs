//! Texte écrit en Helvetica, police standard du PDF, encodage WinAnsi
//! (alphabet latin d'Europe de l'Ouest) : octets à écrire et largeur du
//! texte, pour dessiner soi-même un champ de formulaire. Largeurs tirées
//! des métriques publiées par Adobe pour les 14 polices standard.

/// Largeurs (millièmes d'em) des caractères 32 à 126.
const ASCII_WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // espace à /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // chiffres
    278, 278, 584, 584, 584, 556, 1015, // : à @
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667,
    611, 722, 667, 944, 667, 667, 611, // A à Z
    278, 278, 278, 469, 556, 333, // [ à `
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500,
    278, 556, 500, 722, 500, 500, 500, // a à z
    334, 260, 334, 584, // { à ~
];

/// Lettre de base d'une lettre accentuée (même largeur en Helvetica).
fn base(c: char) -> char {
    match c {
        'À'..='Å' => 'A',
        'Ç' => 'C',
        'È'..='Ë' => 'E',
        'Ì'..='Ï' => 'I',
        'Ñ' => 'N',
        'Ò'..='Ö' | 'Ø' => 'O',
        'Ù'..='Ü' => 'U',
        'Ý' | 'Ÿ' => 'Y',
        'à'..='å' => 'a',
        'ç' => 'c',
        'è'..='ë' => 'e',
        'ì'..='ï' => 'i',
        'ñ' => 'n',
        'ò'..='ö' | 'ø' => 'o',
        'ù'..='ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'Š' => 'S',
        'š' => 's',
        'Ž' => 'Z',
        'ž' => 'z',
        c => c,
    }
}

/// Largeur d'un caractère, en millièmes du corps.
pub fn char_width(c: char) -> u16 {
    let c = base(c);
    match c {
        ' '..='~' => ASCII_WIDTHS[c as usize - 32],
        'Œ' => 1000,
        'œ' => 944,
        'Æ' => 1000,
        'æ' => 889,
        'ß' => 611,
        '‘' | '’' => 222,
        '“' | '”' | '«' | '»' => 556,
        '–' => 556,
        '—' | '…' => 1000,
        '°' => 400,
        '\u{a0}' => 278,
        _ => 556,
    }
}

/// Largeur de `text` au corps `size`, en points.
pub fn text_width(text: &str, size: f64) -> f64 {
    text.chars().map(|c| f64::from(char_width(c))).sum::<f64>() * size / 1000.0
}

/// Caractères en WinAnsi (un caractère = un octet, `?` hors du jeu).
pub fn win_ansi(text: &str) -> Vec<u8> {
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

/// Chaîne littérale PDF (`(…)`, parenthèses et barres obliques échappées).
pub fn pdf_literal(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 2);
    out.push(b'(');
    for &b in bytes {
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.push(b')');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_the_standard_metrics() {
        assert_eq!(char_width('A'), 667);
        assert_eq!(char_width('é'), char_width('e'));
        assert_eq!(char_width('~'), 584);
        assert_eq!(char_width('中'), 556);
        assert!((text_width("Hello", 10.0) - 22.78).abs() < 1e-9);
    }

    #[test]
    fn accents_and_typography_use_win_ansi() {
        assert_eq!(win_ansi("Hélène"), b"H\xe9l\xe8ne");
        assert_eq!(win_ansi("l’œuvre — 5 €"), b"l\x92\x9cuvre \x97 5 \x80");
        assert_eq!(win_ansi("中"), b"?");
        assert_eq!(pdf_literal(b"a(b)\\"), b"(a\\(b\\)\\\\)");
    }
}
