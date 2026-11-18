//! Polices embarquées pour le texte ajouté (fin de E-01).
//!
//! Les polices standard du PDF (Helvetica, Times, Courier) ne couvrent que
//! le jeu WinAnsi : alphabet latin d'Europe de l'Ouest, €, œ… Une zone de
//! texte qui en sort est écrite avec la police Noto équivalente, embarquée
//! dans le PDF (latin étendu, grec, cyrillique). Ce module dit quand c'est
//! nécessaire, fournit la police et signale les caractères qu'aucune
//! police ne couvre (chinois, arabe… : prévus plus tard).

use std::sync::OnceLock;

use pde_core::edit::StandardFont;
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

/// Police embarquée : son fichier et ce qu'on en a lu.
pub struct EmbeddedFont {
    pub name: &'static str,
    pub bytes: &'static [u8],
    font: Option<FontRef<'static>>,
}

impl EmbeddedFont {
    fn new(name: &'static str, bytes: &'static [u8]) -> Self {
        Self {
            name,
            bytes,
            font: FontRef::new(bytes).ok(),
        }
    }

    pub fn covers(&self, c: char) -> bool {
        self.font
            .as_ref()
            .is_some_and(|font| font.charmap().map(c).is_some())
    }

    /// Hauteur du haut des minuscules à hampe (« d ») au-dessus de la ligne
    /// de base, en fraction du corps : même mesure que l'« Ascender » des
    /// polices standard, pour aligner le haut des zones de texte.
    pub fn ascent(&self) -> f64 {
        let Some(font) = &self.font else {
            return 0.75;
        };
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        let units = f64::from(metrics.units_per_em);
        font.charmap()
            .map('d')
            .and_then(|id| {
                font.glyph_metrics(Size::unscaled(), LocationRef::default())
                    .bounds(id)
            })
            .map(|bbox| f64::from(bbox.y_max) / units)
            .unwrap_or_else(|| f64::from(metrics.ascent) / units)
    }

    /// Largeur du texte en points, au corps `size` (caractère absent : la
    /// largeur d'un « n »). Sert à ajuster le corps d'un texte traduit.
    pub fn text_width(&self, text: &str, size: f64) -> f64 {
        let Some(font) = &self.font else {
            return text.chars().count() as f64 * size * 0.55;
        };
        let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
        let units = f64::from(
            font.metrics(Size::unscaled(), LocationRef::default())
                .units_per_em,
        );
        let charmap = font.charmap();
        let advance = |c: char| {
            charmap
                .map(c)
                .and_then(|id| metrics.advance_width(id))
                .map(f64::from)
        };
        let fallback = advance('n').unwrap_or(units * 0.55);
        let total: f64 = text.chars().map(|c| advance(c).unwrap_or(fallback)).sum();
        total / units * size
    }
}

macro_rules! font {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!("../fonts/", $name, ".ttf")) as &[u8],
        )
    };
}

/// Variantes, dans l'ordre de `index` : famille × gras × italique.
const FILES: [(&str, &[u8]); 12] = [
    font!("NotoSans-Regular"),
    font!("NotoSans-Italic"),
    font!("NotoSans-Bold"),
    font!("NotoSans-BoldItalic"),
    font!("NotoSerif-Regular"),
    font!("NotoSerif-Italic"),
    font!("NotoSerif-Bold"),
    font!("NotoSerif-BoldItalic"),
    // Pas d'italique en chasse fixe : la variante droite la remplace.
    font!("NotoSansMono-Regular"),
    font!("NotoSansMono-Regular"),
    font!("NotoSansMono-Bold"),
    font!("NotoSansMono-Bold"),
];

fn index(font: StandardFont, bold: bool, italic: bool) -> usize {
    let family = match font {
        StandardFont::Sans => 0,
        StandardFont::Serif => 4,
        StandardFont::Mono => 8,
    };
    family + usize::from(bold) * 2 + usize::from(italic)
}

/// Police Noto correspondant au style d'une zone de texte.
pub fn embedded(font: StandardFont, bold: bool, italic: bool) -> &'static EmbeddedFont {
    static FONTS: OnceLock<Vec<EmbeddedFont>> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        FILES
            .iter()
            .map(|(name, bytes)| EmbeddedFont::new(name, bytes))
            .collect()
    });
    &fonts[index(font, bold, italic)]
}

/// Caractères WinAnsi hors ASCII imprimable et Latin-1 (codes 0x80 à 0x9F).
const WIN_ANSI_EXTRAS: &str = "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ";

/// Le caractère s'écrit avec une police standard du PDF.
pub fn fits_standard_font(c: char) -> bool {
    c.is_control() || matches!(c, ' '..='~' | '\u{a0}'..='\u{ff}') || WIN_ANSI_EXTRAS.contains(c)
}

/// Toute la zone s'écrit avec les polices standard (rien à embarquer).
pub fn fits_standard_fonts(text: &str) -> bool {
    text.chars().all(fits_standard_font)
}

/// Caractères qu'aucune police ne sait écrire, sans doublon, dans l'ordre
/// du texte.
pub fn missing_chars(text: &str, font: StandardFont, bold: bool, italic: bool) -> Vec<char> {
    if fits_standard_fonts(text) {
        return Vec::new();
    }
    let embedded = embedded(font, bold, italic);
    let mut missing = Vec::new();
    for c in text.chars() {
        if !c.is_whitespace() && !c.is_control() && !embedded.covers(c) && !missing.contains(&c) {
            missing.push(c);
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [StandardFont; 3] = [StandardFont::Sans, StandardFont::Serif, StandardFont::Mono];

    #[test]
    fn text_width_grows_with_text_and_size() {
        let font = embedded(StandardFont::Sans, false, false);
        let short = font.text_width("Bonjour", 10.0);
        assert!(short > 20.0 && short < 50.0, "{short}");
        assert!(font.text_width("Bonjour le monde", 10.0) > short);
        assert!((font.text_width("Bonjour", 20.0) - 2.0 * short).abs() < 1e-9);
        assert!(
            font.text_width("你好", 10.0) > 0.0,
            "caractère absent : largeur de repli"
        );
    }

    #[test]
    fn every_bundled_font_parses_and_has_sensible_metrics() {
        for font in ALL {
            for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
                let f = embedded(font, bold, italic);
                assert!(f.font.is_some(), "{} illisible", f.name);
                assert!(
                    (0.6..0.9).contains(&f.ascent()),
                    "{} : {}",
                    f.name,
                    f.ascent()
                );
            }
        }
        assert_eq!(
            embedded(StandardFont::Mono, false, true).name,
            "NotoSansMono-Regular"
        );
        assert_eq!(
            embedded(StandardFont::Serif, true, true).name,
            "NotoSerif-BoldItalic"
        );
    }

    #[test]
    fn bundled_files_match_their_checksums() {
        use sha2::{Digest, Sha256};
        let sums = include_str!("../fonts/SHA256SUMS");
        for (name, bytes) in FILES {
            let digest: String = Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            let line = format!("{digest}  {name}.ttf");
            assert!(
                sums.lines().any(|l| l == line),
                "{name} : empreinte {digest} absente de SHA256SUMS"
            );
        }
    }

    #[test]
    fn western_text_keeps_the_standard_fonts() {
        assert!(fits_standard_fonts(
            "Été à 18 € : œuvre « déjà » vue — 50 % ‰ ™"
        ));
        assert!(fits_standard_fonts("ligne 1\nligne 2\ttabulée"));
        assert!(!fits_standard_fonts("Ελληνικά"));
        assert!(!fits_standard_fonts("Łódź"), "polonais : latin étendu");
    }

    #[test]
    fn greek_cyrillic_and_extended_latin_are_covered() {
        for font in ALL {
            let text = "Ελληνικά Кириллица Łódź Čeština Ğüzel Đà Nẵng";
            assert_eq!(
                missing_chars(text, font, false, false),
                Vec::<char>::new(),
                "{font:?}"
            );
        }
    }

    #[test]
    fn other_scripts_are_reported_once() {
        let missing = missing_chars(
            "Contrat 契約 et 契約, عقد",
            StandardFont::Sans,
            false,
            false,
        );
        assert_eq!(missing, ['契', '約', 'ع', 'ق', 'د']);
        assert!(missing_chars("Contrat signé", StandardFont::Sans, true, true).is_empty());
    }
}
