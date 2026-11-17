//! Écriture des éléments ajoutés dans un document PDFium : zones de texte
//! (`text.rs`), images (`image.rs`), annotations dessinées (`annotation.rs`),
//! notes (`note.rs`, vraies annotations PDF), et caviardage (`redact.rs`),
//! appliqué en dernier pour effacer aussi ce qui a été ajouté dessous.

mod annotation;
mod blocks;
mod existing;
mod image;
mod note;
mod original_text;
mod redact;
mod text;

use std::collections::BTreeMap;

use pde_core::document::PageIndex;
use pde_core::edit::{Element, ElementId, Images, MIN_WIDTH};
use pde_core::engine::{EngineError, Placed};
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{PdfDocument, PdfPage};

use crate::convert::internal;
use crate::display::DisplayTransform;
use original_text::Rewritten;

pub(crate) use blocks::page_text_blocks;
pub(crate) use existing::page_images;

/// Pose les éléments, page par page ; renvoie leur place sur la page affichée.
pub(crate) fn apply(
    doc: &mut PdfDocument,
    elements: &[(ElementId, Element)],
    images: &Images,
) -> Result<Vec<Placed>, EngineError> {
    let mut placed = Vec::with_capacity(elements.len());
    let mut fonts = text::Fonts::default();
    let mut redactions: BTreeMap<PageIndex, Vec<Rect>> = BTreeMap::new();
    // Objets d'origine d'abord : leur rang est celui de la page arrangée.
    let existing: Vec<_> = elements
        .iter()
        .filter_map(|(_, e)| match e {
            Element::ExistingImage(image) => Some(image),
            _ => None,
        })
        .collect();
    let texts: Vec<_> = elements
        .iter()
        .filter_map(|(id, e)| match e {
            Element::OriginalText(block) => Some((*id, block)),
            _ => None,
        })
        .collect();
    let mut rewritten = existing::apply(doc, &existing, &texts)?;
    for (id, element) in elements {
        let mut replaced_font = false;
        let bounds = match element {
            Element::Text(box_) => text::write_text(doc, box_, &mut fonts)?,
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
            Element::ExistingImage(image) => image.bounds(),
            Element::OriginalText(block) => match rewritten.remove(id) {
                Some(Rewritten::Kept(bounds) | Rewritten::Removed(bounds)) => bounds,
                Some(Rewritten::Replaced(replacement)) => {
                    replaced_font = true;
                    let written = text::write_text(doc, &replacement, &mut fonts)?;
                    // Le cadre écrit va jusqu'au bord de la page : on garde
                    // la largeur d'origine pour la sélection.
                    let right = written.min().x + block.original.width.max(MIN_WIDTH);
                    Rect::from_corners(written.min(), Point::new(right, written.max().y))
                }
                None => block.bounds(),
            },
            Element::Redact(redact) => {
                let areas = redactions.entry(redact.page).or_default();
                areas.extend(redact.areas.iter().map(|a| a.rect()));
                redact.bounds()
            }
        };
        placed.push(Placed {
            id: *id,
            bounds,
            replaced_font,
        });
    }
    for (page, areas) in &redactions {
        redact::redact_page(doc, *page, areas)?;
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
