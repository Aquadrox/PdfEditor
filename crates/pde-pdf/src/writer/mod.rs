//! Écriture des éléments ajoutés dans un document PDFium : zones de texte
//! (`text.rs`), images (`image.rs`), annotations dessinées (`annotation.rs`),
//! notes (`note.rs`, vraies annotations PDF).

mod annotation;
mod image;
mod note;
mod text;

use pde_core::edit::{Element, ElementId, Images};
use pde_core::engine::{EngineError, Placed};
use pdfium_render::prelude::{PdfDocument, PdfPage};

use crate::convert::internal;
use crate::display::DisplayTransform;

/// Pose les éléments, page par page ; renvoie leur place sur la page affichée.
pub(crate) fn apply(
    doc: &mut PdfDocument,
    elements: &[(ElementId, Element)],
    images: &Images,
) -> Result<Vec<Placed>, EngineError> {
    let mut placed = Vec::with_capacity(elements.len());
    for (id, element) in elements {
        let bounds = match element {
            Element::Text(box_) => text::write_text(doc, box_)?,
            Element::Image(box_) => {
                let asset = images.get(&box_.image).ok_or_else(|| {
                    EngineError::Internal(format!("image {} absente", box_.image.0))
                })?;
                image::write_image(doc, box_, asset)?
            }
            Element::Shape(shape) => annotation::write_shape(doc, shape)?,
            Element::Ink(ink) => annotation::write_ink(doc, ink)?,
            Element::Markup(markup) => annotation::write_markup(doc, markup)?,
            Element::Note(note) => note::write_note(doc, note)?,
        };
        placed.push(Placed { id: *id, bounds });
    }
    Ok(placed)
}

/// Page d'un élément et passage de la page affichée à la page PDF.
pub(super) fn page_of<'a>(
    doc: &PdfDocument<'a>,
    page: usize,
) -> Result<(PdfPage<'a>, DisplayTransform), EngineError> {
    let index = i32::try_from(page).map_err(|_| EngineError::InvalidRender(page.to_string()))?;
    let page = doc.pages().get(index).map_err(internal)?;
    let display = DisplayTransform::for_page(&page)?;
    Ok((page, display))
}
