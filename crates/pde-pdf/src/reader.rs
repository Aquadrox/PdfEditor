//! Lecture d'un document ouvert dans PDFium : description, rendu, texte.
//!
//! Fonctions appelées uniquement depuis le thread du moteur.

use pde_core::document::{DocumentInfo, OutlineItem, PageIndex, PageSize};
use pde_core::engine::EngineError;
use pde_core::render::{Bitmap, RenderRequest, pixel_size};
use pde_core::text::{PageText, TextChar};
use pdfium_render::prelude::{
    PdfBookmark, PdfDocument, PdfDocumentMetadataTagType, PdfPage, PdfRenderConfig,
};

use crate::convert::{internal, rect};

/// Profondeur maximale du plan : un PDF malveillant peut contenir un plan
/// cyclique ou démesuré, qui ne doit ni boucler ni épuiser la mémoire.
const MAX_OUTLINE_DEPTH: usize = 32;
/// Nombre maximal d'entrées du plan lues.
const MAX_OUTLINE_ITEMS: usize = 10_000;

pub(crate) fn document_info(doc: &PdfDocument) -> Result<DocumentInfo, EngineError> {
    let page_sizes = doc
        .pages()
        .iter()
        .map(|page| {
            PageSize::new(
                f64::from(page.width().value),
                f64::from(page.height().value),
            )
        })
        .collect();
    let title = doc
        .metadata()
        .get(PdfDocumentMetadataTagType::Title)
        .map(|tag| tag.value().trim().to_owned())
        .filter(|title| !title.is_empty());
    let mut budget = MAX_OUTLINE_ITEMS;
    let outline = outline_level(doc.bookmarks().root(), 0, &mut budget);
    Ok(DocumentInfo {
        page_sizes,
        title,
        outline,
    })
}

/// Entrées d'un niveau du plan, à partir de la première, avec leurs enfants.
fn outline_level(first: Option<PdfBookmark>, depth: usize, budget: &mut usize) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut current = first;
    while let Some(bookmark) = current {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let children = if depth + 1 < MAX_OUTLINE_DEPTH {
            outline_level(bookmark.first_child(), depth + 1, budget)
        } else {
            Vec::new()
        };
        items.push(OutlineItem {
            title: bookmark.title().unwrap_or_default().trim().to_owned(),
            page: bookmark
                .destination()
                .and_then(|d| d.page_index().ok())
                .and_then(|index| usize::try_from(index).ok()),
            children,
        });
        current = bookmark.next_sibling();
    }
    items
}

fn page<'a>(doc: &'a PdfDocument, index: PageIndex) -> Result<PdfPage<'a>, EngineError> {
    let count = usize::try_from(doc.pages().len()).unwrap_or(0);
    let out_of_range = EngineError::PageOutOfRange { page: index, count };
    let pdfium_index = i32::try_from(index).map_err(|_| out_of_range.clone())?;
    if index >= count {
        return Err(out_of_range);
    }
    doc.pages().get(pdfium_index).map_err(internal)
}

pub(crate) fn render(doc: &PdfDocument, request: RenderRequest) -> Result<Bitmap, EngineError> {
    let page = page(doc, request.page)?;
    let size = PageSize::new(
        f64::from(page.width().value),
        f64::from(page.height().value),
    );
    let (width, height) = pixel_size(size, request.scale)?;
    // Côtés bornés par pixel_size (≤ 16 384 px) : la conversion en i32 réussit.
    let to_i32 = |v: u32| i32::try_from(v).map_err(|_| EngineError::InvalidRender(v.to_string()));
    let config = PdfRenderConfig::new()
        .set_target_size(to_i32(width)?, to_i32(height)?)
        .render_form_data(true)
        .render_annotations(true);
    let bitmap = page.render_with_config(&config).map_err(internal)?;
    let (w, h) = (bitmap.width(), bitmap.height());
    let to_u32 = |v: i32| u32::try_from(v).map_err(|_| EngineError::Internal(v.to_string()));
    Bitmap::new(to_u32(w)?, to_u32(h)?, bitmap.as_rgba_bytes())
}

pub(crate) fn page_text(doc: &PdfDocument, index: PageIndex) -> Result<PageText, EngineError> {
    let page = page(doc, index)?;
    let text = page.text().map_err(internal)?;
    let chars = text
        .chars()
        .iter()
        .filter_map(|c| {
            let ch = c.unicode_char()?;
            // Caractères générés par PDFium sans position (retours à la
            // ligne déduits) : gardés pour le texte, rectangle vide.
            let bounds = c.loose_bounds().map(rect).unwrap_or_default();
            Some(TextChar { ch, bounds })
        })
        .collect();
    Ok(PageText { chars })
}
