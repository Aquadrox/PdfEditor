//! Apparence d'une signature visible (S-03) : la signature manuscrite
//! (image dessinée, tapée ou importée) en haut du cadre, et quelques lignes
//! de texte en dessous (Helvetica, police standard du PDF).

use lopdf::{Dictionary, Object, ObjectId, Stream};

/// Corps de police : borné pour rester lisible.
const MIN_SIZE: f32 = 4.0;
const MAX_SIZE: f32 = 14.0;
/// Largeur moyenne d'un caractère Helvetica, en fraction du corps (prudente).
const AVERAGE_WIDTH: f32 = 0.56;
const LEADING: f32 = 1.25;
const PADDING: f32 = 2.0;
/// Part de la hauteur donnée à l'image quand il y a aussi du texte.
const IMAGE_SHARE: f32 = 0.62;

/// Image posée dans l'apparence : son objet et sa taille en pixels.
pub(crate) struct PlacedImage {
    pub id: ObjectId,
    pub width: u32,
    pub height: u32,
}

/// Flux d'apparence (XObject de formulaire) pour un cadre `width` × `height`
/// en points ; `font` désigne l'objet police Helvetica.
pub(crate) fn appearance_stream(
    lines: &[String],
    image: Option<&PlacedImage>,
    width: f32,
    height: f32,
    font: ObjectId,
) -> Stream {
    let lines: Vec<&String> = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let mut content = String::new();
    // Zone du texte : tout le cadre, ou sa partie basse sous l'image.
    let text_height = match (image, lines.is_empty()) {
        (Some(_), false) => height * (1.0 - IMAGE_SHARE),
        _ => height,
    };
    if let Some(image) = image {
        let zone = (
            PADDING,
            if lines.is_empty() {
                PADDING
            } else {
                text_height
            },
            width - 2.0 * PADDING,
            if lines.is_empty() {
                height - 2.0 * PADDING
            } else {
                height - text_height - PADDING
            },
        );
        let (x, y, w, h) = fit(image.width, image.height, zone);
        content.push_str(&format!(
            "q\n{w:.2} 0 0 {h:.2} {x:.2} {y:.2} cm\n/Im1 Do\nQ\n"
        ));
    }
    if !lines.is_empty() {
        text_block(&mut content, &lines, width, text_height);
    }
    let mut fonts = Dictionary::new();
    fonts.set("F1", font);
    let mut resources = Dictionary::new();
    resources.set("Font", fonts);
    if let Some(image) = image {
        let mut xobjects = Dictionary::new();
        xobjects.set("Im1", image.id);
        resources.set("XObject", xobjects);
    }
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

/// Image de `px_w` × `px_h` pixels ajustée dans la zone `(x, y, w, h)`,
/// proportions gardées, calée à gauche et centrée verticalement.
fn fit(px_w: u32, px_h: u32, (x, y, w, h): (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let ratio = px_w.max(1) as f32 / px_h.max(1) as f32;
    let (mut fw, mut fh) = (w.max(1.0), w.max(1.0) / ratio);
    if fh > h {
        fh = h.max(1.0);
        fw = fh * ratio;
    }
    (x, y + (h - fh) / 2.0, fw, fh)
}

/// Lignes de texte dans la zone basse `[0, height]` du cadre.
fn text_block(content: &mut String, lines: &[&String], width: f32, height: f32) {
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
    // Bloc centré verticalement dans sa zone ; première ligne en haut.
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
        let stream = appearance_stream(&lines, None, 200.0, 50.0, (9, 0));
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
    fn image_sits_above_the_text_with_its_proportions() {
        let image = PlacedImage {
            id: (8, 0),
            width: 400,
            height: 100,
        };
        let lines = vec!["Claire Martin".to_string()];
        let stream = appearance_stream(&lines, Some(&image), 200.0, 80.0, (9, 0));
        let content = String::from_utf8_lossy(&stream.content).to_string();
        // Zone image : 196 × 47,6 au-dessus du texte (80 × 0,38 = 30,4) ;
        // image 4:1 limitée par la hauteur → 190,4 × 47,6.
        assert!(
            content.contains("190.40 0 0 47.60 2.00 30.40 cm"),
            "{content}"
        );
        assert!(content.contains("/Im1 Do"));
        let resources = stream.dict.get(b"Resources").unwrap().as_dict().unwrap();
        assert!(
            resources
                .get(b"XObject")
                .unwrap()
                .as_dict()
                .unwrap()
                .has(b"Im1")
        );
        // Sans texte, l'image prend tout le cadre (hauteur limitante ici).
        let alone = appearance_stream(&[], Some(&image), 200.0, 20.0, (9, 0));
        let content = String::from_utf8_lossy(&alone.content).to_string();
        assert!(
            content.contains("64.00 0 0 16.00 2.00 2.00 cm"),
            "{content}"
        );
    }

    #[test]
    fn empty_lines_give_an_empty_appearance() {
        let stream = appearance_stream(&[" ".into()], None, 100.0, 20.0, (9, 0));
        assert!(stream.content.is_empty());
    }
}
