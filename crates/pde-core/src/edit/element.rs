//! Éléments ajoutés à un document : zones de texte (E-01), images (E-02),
//! annotations dessinées (E-04).
//!
//! Coordonnées : page affichée (rotation appliquée), points, origine en bas
//! à gauche, comme le reste de `pde-core`.

use serde::{Deserialize, Serialize};

use super::annotation::{InkBox, MarkupBox, ShapeBox};
use super::image::{ImageBox, Images};
use super::note::NoteBox;
use super::redact::RedactBox;
use crate::document::PageIndex;
use crate::geometry::Point;

/// Identifiant d'un élément, unique dans le document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElementId(pub u64);

/// Polices standard du PDF : présentes dans tous les lecteurs, rien à
/// embarquer (alphabet latin, € et œ compris).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandardFont {
    /// Sans empattement (Helvetica, équivalent d'Arial).
    #[default]
    Sans,
    /// Avec empattements (Times).
    Serif,
    /// Chasse fixe (Courier).
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Couleur RVB, 8 bits par composante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const INK: Color = Color {
        r: 0x1c,
        g: 0x24,
        b: 0x30,
    };
}

impl Default for Color {
    fn default() -> Self {
        Self::INK
    }
}

/// Corps accepté (points).
pub const FONT_SIZE_RANGE: std::ops::RangeInclusive<f64> = 4.0..=200.0;
/// Interligne accepté (multiple du corps).
pub const LINE_HEIGHT_RANGE: std::ops::RangeInclusive<f64> = 0.8..=3.0;
/// Largeur minimale d'une zone de texte (points).
pub const MIN_WIDTH: f64 = 8.0;
/// Texte d'une zone : au-delà, la saisie est refusée.
pub const MAX_TEXT_CHARS: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub font: StandardFont,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub color: Color,
    pub align: Align,
    /// Interligne, en multiple du corps.
    pub line_height: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: StandardFont::Sans,
            size: 11.0,
            bold: false,
            italic: false,
            color: Color::INK,
            align: Align::Left,
            line_height: 1.25,
        }
    }
}

/// Zone de texte : coin haut gauche et largeur fixés, hauteur selon le
/// nombre de lignes (calculée par le moteur, qui connaît les polices).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBox {
    pub page: PageIndex,
    /// Coin haut gauche.
    pub top_left: Point,
    pub width: f64,
    pub text: String,
    #[serde(default)]
    pub style: TextStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Element {
    Text(TextBox),
    Image(ImageBox),
    Shape(ShapeBox),
    Ink(InkBox),
    Markup(MarkupBox),
    Note(NoteBox),
    Redact(RedactBox),
}

impl Element {
    pub fn page(&self) -> PageIndex {
        match self {
            Element::Text(text) => text.page,
            Element::Image(image) => image.page,
            Element::Shape(shape) => shape.page,
            Element::Ink(ink) => ink.page,
            Element::Markup(markup) => markup.page,
            Element::Note(note) => note.page,
            Element::Redact(redact) => redact.page,
        }
    }

    /// Le même élément, posé sur la page `page`.
    pub fn with_page(mut self, page: PageIndex) -> Element {
        match &mut self {
            Element::Text(text) => text.page = page,
            Element::Image(image) => image.page = page,
            Element::Shape(shape) => shape.page = page,
            Element::Ink(ink) => ink.page = page,
            Element::Markup(markup) => markup.page = page,
            Element::Note(note) => note.page = page,
            Element::Redact(redact) => redact.page = page,
        }
        self
    }

    /// Vérifie les valeurs (venues de l'interface) avant toute écriture ;
    /// `images` : images importées, si l'appelant les connaît.
    pub fn validate(&self, page_count: usize, images: Option<&Images>) -> Result<(), String> {
        if self.page() >= page_count {
            return Err(format!("la page {} n'existe pas", self.page() + 1));
        }
        match self {
            Element::Image(image) => image.validate(images),
            Element::Shape(shape) => shape.validate(),
            Element::Ink(ink) => ink.validate(),
            Element::Markup(markup) => markup.validate(),
            Element::Note(note) => note.validate(),
            Element::Redact(redact) => redact.validate(),
            Element::Text(text) => {
                let numbers = [
                    text.top_left.x,
                    text.top_left.y,
                    text.width,
                    text.style.size,
                    text.style.line_height,
                ];
                if numbers.iter().any(|v| !v.is_finite()) {
                    return Err("position ou taille invalide".into());
                }
                if text.width < MIN_WIDTH {
                    return Err(format!(
                        "zone de texte trop étroite (minimum {MIN_WIDTH} pt)"
                    ));
                }
                if !FONT_SIZE_RANGE.contains(&text.style.size) {
                    return Err(format!(
                        "corps entre {} et {} pt",
                        FONT_SIZE_RANGE.start(),
                        FONT_SIZE_RANGE.end()
                    ));
                }
                if !LINE_HEIGHT_RANGE.contains(&text.style.line_height) {
                    return Err("interligne entre 0,8 et 3".into());
                }
                if text.text.chars().count() > MAX_TEXT_CHARS {
                    return Err(format!(
                        "texte trop long (plus de {MAX_TEXT_CHARS} caractères)"
                    ));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_box() -> TextBox {
        TextBox {
            page: 0,
            top_left: Point::new(72.0, 700.0),
            width: 200.0,
            text: "Bonjour".into(),
            style: TextStyle::default(),
        }
    }

    #[test]
    fn valid_text_box() {
        assert_eq!(Element::Text(text_box()).validate(1, None), Ok(()));
        assert_eq!(Element::Text(text_box()).page(), 0);
        assert_eq!(Element::Text(text_box()).with_page(3).page(), 3);
    }

    #[test]
    fn invalid_values_are_explained() {
        let cases: [fn(&mut TextBox); 6] = [
            |t| t.page = 1,
            |t| t.width = 2.0,
            |t| t.top_left.x = f64::NAN,
            |t| t.style.size = 500.0,
            |t| t.style.line_height = 10.0,
            |t| t.text = "x".repeat(MAX_TEXT_CHARS + 1),
        ];
        for change in cases {
            let mut text = text_box();
            change(&mut text);
            assert!(Element::Text(text).validate(1, None).is_err());
        }
    }

    #[test]
    fn json_shape_for_the_interface() {
        let json = serde_json::to_value(Element::Text(text_box())).unwrap();
        assert_eq!(json["kind"], "text");
        assert_eq!(json["style"]["font"], "sans");
        assert_eq!(json["style"]["color"]["r"], 0x1c);
        // Style absent : style par défaut.
        let mut without_style = json.clone();
        without_style.as_object_mut().unwrap().remove("style");
        let element: Element = serde_json::from_value(without_style).unwrap();
        assert_eq!(element, Element::Text(text_box()));
    }
}
