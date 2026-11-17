//! Objets d'origine modifiés, avant tout ajout (leur rang parmi les objets
//! de la page est alors celui de la page arrangée) : images recadrées ou
//! retirées (E-06), texte récrit ou retiré (E-05, `original_text.rs`). La
//! place d'origine attendue est vérifiée : un objet qui n'est plus à sa
//! place n'est pas touché, l'opération est refusée.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use pde_core::document::PageIndex;
use pde_core::edit::{ElementId, ExistingImage, OriginalText};
use pde_core::engine::{EngineError, PageImage};
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{
    PdfDocument, PdfPage, PdfPageObjectCommon, PdfPageObjectType, PdfPageObjectsCommon,
};

use super::original_text::{self, Rewritten};
use crate::convert::{internal, rect};
use crate::display::DisplayTransform;

/// Écart toléré entre le cadre attendu et celui trouvé (points).
const TOLERANCE: f64 = 1.0;

/// Images d'origine d'une page : rang et place sur la page affichée.
pub(crate) fn page_images(
    doc: &PdfDocument,
    page: PageIndex,
) -> Result<Vec<PageImage>, EngineError> {
    let (pdf_page, display) = super::page_of(doc, page)?;
    let mut images = Vec::new();
    for (index, object) in pdf_page.objects().iter().enumerate() {
        if object.object_type() != PdfPageObjectType::Image {
            continue;
        }
        let bounds = display.rect(rect(object.bounds().map_err(internal)?.to_rect()));
        images.push(PageImage {
            object: u32::try_from(index)
                .map_err(|_| EngineError::Internal("trop d'objets".into()))?,
            bounds,
        });
    }
    Ok(images)
}

/// Modifications d'une page : images d'origine et blocs de texte.
#[derive(Default)]
struct PageEdits<'e> {
    images: Vec<&'e ExistingImage>,
    texts: Vec<(ElementId, &'e OriginalText)>,
}

/// Vérifie, recadre, récrit puis retire les objets d'origine modifiés, page
/// par page ; renvoie ce que devient chaque bloc de texte.
pub(super) fn apply(
    doc: &mut PdfDocument,
    images: &[&ExistingImage],
    texts: &[(ElementId, &OriginalText)],
) -> Result<HashMap<ElementId, Rewritten>, EngineError> {
    let mut by_page: BTreeMap<PageIndex, PageEdits> = BTreeMap::new();
    for image in images {
        by_page.entry(image.page).or_default().images.push(image);
    }
    for (id, block) in texts {
        by_page
            .entry(block.page)
            .or_default()
            .texts
            .push((*id, block));
    }
    let mut rewritten = HashMap::new();
    for (page, edits) in by_page {
        let (mut pdf_page, display) = super::page_of(doc, page)?;
        // Tout est vérifié avant la première modification.
        for image in &edits.images {
            check(&pdf_page, &display, image)?;
        }
        for (_, block) in &edits.texts {
            original_text::check(&pdf_page, &display, block)?;
        }
        let known = original_text::known_chars(&pdf_page);
        let mut removed = BTreeSet::new();
        for image in &edits.images {
            match &image.frame {
                Some(frame) => reframe(&mut pdf_page, &display, image.object, frame.rect())?,
                None => {
                    removed.insert(image.object);
                }
            }
        }
        for (id, block) in &edits.texts {
            let (remove, result) = original_text::rewrite(&mut pdf_page, &display, block, &known)?;
            removed.extend(remove);
            rewritten.insert(*id, result);
        }
        // Du dernier rang au premier : les rangs suivants ne bougent pas.
        for object in removed.into_iter().rev() {
            pdf_page
                .objects_mut()
                .remove_object_at_index(object as usize)
                .map_err(internal)?;
        }
        // Un texte changé sur place ne régénère pas le contenu de la page.
        if !edits.texts.is_empty() {
            pdf_page.regenerate_content().map_err(internal)?;
        }
    }
    Ok(rewritten)
}

/// L'objet désigné est une image, à la place attendue.
fn check(
    page: &PdfPage,
    display: &DisplayTransform,
    edit: &ExistingImage,
) -> Result<(), EngineError> {
    let moved = || {
        EngineError::Internal(format!(
            "l'image d'origine n° {} de la page {} n'est plus à sa place",
            edit.object,
            edit.page + 1
        ))
    };
    let object = page
        .objects()
        .get(edit.object as usize)
        .map_err(|_| moved())?;
    if object.object_type() != PdfPageObjectType::Image {
        return Err(moved());
    }
    let found = display.rect(rect(object.bounds().map_err(internal)?.to_rect()));
    if !close(&found, &edit.original.rect()) {
        return Err(moved());
    }
    Ok(())
}

pub(super) fn close(a: &Rect, b: &Rect) -> bool {
    [
        (a.min().x, b.min().x),
        (a.min().y, b.min().y),
        (a.max().x, b.max().x),
        (a.max().y, b.max().y),
    ]
    .iter()
    .all(|(p, q)| (p - q).abs() <= TOLERANCE)
}

/// Rectangle de la page affichée → rectangle de la page PDF.
fn page_rect(display: &DisplayTransform, r: Rect) -> Rect {
    let (x0, y0) = display.page_point(r.min());
    let (x1, y1) = display.page_point(r.max());
    Rect::from_corners(Point::new(x0, y0), Point::new(x1, y1))
}

/// Mise à l'échelle et déplacement qui portent le cadre actuel de l'objet
/// sur `frame` : son orientation (rotation, miroir) est gardée.
fn reframe(
    page: &mut PdfPage,
    display: &DisplayTransform,
    object: u32,
    frame: Rect,
) -> Result<(), EngineError> {
    let mut object = page.objects_mut().get(object as usize).map_err(internal)?;
    let from = rect(object.bounds().map_err(internal)?.to_rect());
    let to = page_rect(display, frame);
    let [sx, sy, tx, ty] = scale_and_shift(&from, &to);
    object
        .transform(sx as f32, 0.0, 0.0, sy as f32, tx as f32, ty as f32)
        .map_err(internal)
}

/// [sx, sy, tx, ty] tels que (x, y) ↦ (sx·x + tx, sy·y + ty) porte `from` sur `to`.
fn scale_and_shift(from: &Rect, to: &Rect) -> [f64; 4] {
    let sx = if from.width() > 0.0 {
        to.width() / from.width()
    } else {
        1.0
    };
    let sy = if from.height() > 0.0 {
        to.height() / from.height()
    } else {
        1.0
    };
    [
        sx,
        sy,
        to.min().x - sx * from.min().x,
        to.min().y - sy * from.min().y,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_frame_lands_on_the_new_one() {
        let from = Rect::from_origin_size(Point::new(100.0, 200.0), 50.0, 20.0);
        let to = Rect::from_origin_size(Point::new(10.0, 30.0), 100.0, 10.0);
        let [sx, sy, tx, ty] = scale_and_shift(&from, &to);
        let map = |p: Point| Point::new(sx * p.x + tx, sy * p.y + ty);
        assert_eq!(map(from.min()), to.min());
        assert_eq!(map(from.max()), to.max());
        assert!(close(
            &from,
            &Rect::from_origin_size(Point::new(100.5, 199.5), 50.0, 20.5)
        ));
        assert!(!close(&from, &to));
    }
}
