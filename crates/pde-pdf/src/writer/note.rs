//! Notes : vraies annotations PDF « Texte » (icône, commentaire, auteur),
//! que les autres lecteurs affichent et listent comme les leurs.

use pde_core::edit::NoteBox;
use pde_core::engine::EngineError;
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{PdfColor, PdfDocument, PdfPageAnnotationCommon, PdfRect};

use crate::convert::internal;

pub(super) fn write_note(doc: &mut PdfDocument, note: &NoteBox) -> Result<Rect, EngineError> {
    let (mut page, display) = super::page_of(doc, note.page)?;
    // Cadre de l'icône ramené à la page PDF (rotation comprise).
    let bounds = note.bounds();
    let (x0, y0) = display.page_point(bounds.min());
    let (x1, y1) = display.page_point(bounds.max());
    let rect = Rect::from_corners(Point::new(x0, y0), Point::new(x1, y1));
    let mut annotation = page
        .annotations_mut()
        .create_text_annotation(&note.text)
        .map_err(internal)?;
    annotation
        .set_bounds(PdfRect::new_from_values(
            rect.min().y as f32,
            rect.min().x as f32,
            rect.max().y as f32,
            rect.max().x as f32,
        ))
        .map_err(internal)?;
    if !note.author.is_empty() {
        annotation.set_creator(&note.author).map_err(internal)?;
    }
    let c = note.color;
    annotation
        .set_stroke_color(PdfColor::new(c.r, c.g, c.b, 255))
        .map_err(internal)?;
    Ok(bounds)
}
