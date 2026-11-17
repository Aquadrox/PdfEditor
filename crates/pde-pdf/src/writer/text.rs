//! Écriture des zones de texte ajoutées dans un document PDFium.
//!
//! Les positions viennent de la page affichée : elles sont ramenées à la
//! page PDF par l'inverse de [`DisplayTransform`], et chaque ligne reçoit la
//! même rotation que la page, pour rester droite à l'écran.
//!
//! Police : standard du PDF si tout le texte de la zone s'y écrit (rien à
//! embarquer), sinon la police Noto équivalente, embarquée une fois par
//! variante et par pose (`pde-fonts`).

use std::collections::HashMap;

use pde_core::edit::{Align, StandardFont, TextBox, leading, wrap};
use pde_core::engine::EngineError;
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{
    PdfColor, PdfDocument, PdfFontToken, PdfPageObjectCommon, PdfPageObjectsCommon,
    PdfPageTextObject, PdfPoints,
};

use crate::convert::internal;

/// Hauteur au-dessus de la ligne de base (fraction du corps) des polices
/// standard : Helvetica 718, Times 683, Courier 629 (millièmes, AFM).
pub(super) fn ascent(font: StandardFont) -> f64 {
    match font {
        StandardFont::Sans => 0.718,
        StandardFont::Serif => 0.683,
        StandardFont::Mono => 0.629,
    }
}

/// Polices Noto déjà chargées dans le document en cours de pose.
#[derive(Default)]
pub(super) struct Fonts {
    embedded: HashMap<(StandardFont, bool, bool), PdfFontToken>,
}

impl Fonts {
    /// Police et hauteur au-dessus de la ligne de base pour la zone `text`.
    fn choose(
        &mut self,
        doc: &mut PdfDocument,
        text: &TextBox,
    ) -> Result<(PdfFontToken, f64), EngineError> {
        let (font, bold, italic) = (text.style.font, text.style.bold, text.style.italic);
        if pde_fonts::fits_standard_fonts(&text.text) {
            return Ok((standard_token(doc, text), ascent(font)));
        }
        let embedded = pde_fonts::embedded(font, bold, italic);
        let token = match self.embedded.get(&(font, bold, italic)) {
            Some(token) => *token,
            None => {
                let token = doc
                    .fonts_mut()
                    .load_true_type_from_bytes(embedded.bytes, true)
                    .map_err(internal)?;
                self.embedded.insert((font, bold, italic), token);
                token
            }
        };
        Ok((token, embedded.ascent()))
    }
}

fn standard_token(doc: &mut PdfDocument, text: &TextBox) -> PdfFontToken {
    let fonts = doc.fonts_mut();
    let (bold, italic) = (text.style.bold, text.style.italic);
    match (text.style.font, bold, italic) {
        (StandardFont::Sans, false, false) => fonts.helvetica(),
        (StandardFont::Sans, true, false) => fonts.helvetica_bold(),
        (StandardFont::Sans, false, true) => fonts.helvetica_oblique(),
        (StandardFont::Sans, true, true) => fonts.helvetica_bold_oblique(),
        (StandardFont::Serif, false, false) => fonts.times_roman(),
        (StandardFont::Serif, true, false) => fonts.times_bold(),
        (StandardFont::Serif, false, true) => fonts.times_italic(),
        (StandardFont::Serif, true, true) => fonts.times_bold_italic(),
        (StandardFont::Mono, false, false) => fonts.courier(),
        (StandardFont::Mono, true, false) => fonts.courier_bold(),
        (StandardFont::Mono, false, true) => fonts.courier_oblique(),
        (StandardFont::Mono, true, true) => fonts.courier_bold_oblique(),
    }
}

pub(super) fn write_text(
    doc: &mut PdfDocument,
    text: &TextBox,
    fonts: &mut Fonts,
) -> Result<Rect, EngineError> {
    let (font, top_to_baseline) = fonts.choose(doc, text)?;
    let size = PdfPoints::new(text.style.size as f32);
    let measure = |s: &str| -> f64 {
        if s.is_empty() {
            return 0.0;
        }
        PdfPageTextObject::new(doc, s, font, size)
            .and_then(|o| o.width())
            .map(|w| f64::from(w.value))
            .unwrap_or(0.0)
    };
    let lines = wrap(&text.text, text.width, measure);
    let widths: Vec<f64> = lines.iter().map(|l| measure(l)).collect();

    let (mut page, display) = super::page_of(doc, text.page)?;
    let step = leading(&text.style);
    let (ux, uy) = display.page_vector(1.0, 0.0);
    let (vx, vy) = display.page_vector(0.0, 1.0);
    let color = PdfColor::new(
        text.style.color.r,
        text.style.color.g,
        text.style.color.b,
        255,
    );

    for (i, (line, width)) in lines.iter().zip(&widths).enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let offset = match text.style.align {
            Align::Left => 0.0,
            Align::Center => (text.width - width) / 2.0,
            Align::Right => text.width - width,
        };
        let baseline = Point::new(
            text.top_left.x + offset,
            text.top_left.y - top_to_baseline * text.style.size - step * i as f64,
        );
        let (px, py) = display.page_point(baseline);
        let mut object = PdfPageTextObject::new(doc, line, font, size).map_err(internal)?;
        object.set_fill_color(color).map_err(internal)?;
        object
            .transform(
                ux as f32, uy as f32, vx as f32, vy as f32, px as f32, py as f32,
            )
            .map_err(internal)?;
        page.objects_mut()
            .add_text_object(object)
            .map_err(internal)?;
    }
    let height = step * lines.len().max(1) as f64;
    Ok(Rect::from_corners(
        Point::new(text.top_left.x, text.top_left.y - height),
        Point::new(text.top_left.x + text.width, text.top_left.y),
    ))
}
