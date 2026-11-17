//! Caviardage réel (E-07) : sur une page, tout objet qui touche une zone
//! caviardée (texte, image, tracé, formulaire XObject) est retiré du
//! contenu et remplacé par son image, rendue juste avant, où les zones
//! sont noircies ; les annotations qui touchent une zone sont supprimées ;
//! un aplat noir vectoriel couvre enfin chaque zone. Le contenu caviardé
//! n'existe donc plus dans le fichier, et le reste de la page garde son
//! aspect (le texte d'un objet touché devient une image : il n'est plus
//! sélectionnable).

use pde_core::document::PageIndex;
use pde_core::engine::EngineError;
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{
    PdfBitmap, PdfBitmapFormat, PdfColor, PdfDocument, PdfPageAnnotationCommon, PdfPageImageObject,
    PdfPageObjectCommon, PdfPageObjectsCommon, PdfPagePathObject, PdfPoints, PdfRenderConfig,
};

use crate::convert::{internal, rect};
use crate::display::DisplayTransform;

/// Pixels par point de l'image qui remplace un objet touché (216 ppp).
const PATCH_SCALE: f64 = 3.0;
/// Plus grand côté du rendu de la page (pixels) : l'échelle baisse au-delà.
const MAX_RENDER_SIDE: f64 = 12_000.0;

/// Zone de pixels du rendu de la page (origine en haut à gauche).
#[derive(Debug, Clone, Copy, PartialEq)]
struct PixelRect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// Caviarde la page `page` ; `areas` : zones en coordonnées de la page
/// affichée (origine en bas).
pub(super) fn redact_page(
    doc: &mut PdfDocument,
    page: PageIndex,
    areas: &[Rect],
) -> Result<(), EngineError> {
    let (mut pdf_page, display) = super::page_of(doc, page)?;
    let size = (
        f64::from(pdf_page.width().value),
        f64::from(pdf_page.height().value),
    );
    let touches = |r: &Rect| areas.iter().any(|a| a.intersection(r).is_some());

    // 1. Objets touchés : leur place sur la page affichée.
    let mut touched = Vec::new();
    for (index, object) in pdf_page.objects().iter().enumerate() {
        let bounds = display.rect(rect(object.bounds().map_err(internal)?.to_rect()));
        if touches(&bounds) {
            touched.push((index, bounds));
        }
    }

    // 2. Rendu de la page telle quelle (sans les annotations, traitées à part).
    let scale = PATCH_SCALE.min(MAX_RENDER_SIDE / size.0.max(size.1).max(1.0));
    let (width, height) = (
        (size.0 * scale).ceil().max(1.0),
        (size.1 * scale).ceil().max(1.0),
    );
    let mut pixels = if touched.is_empty() {
        Vec::new()
    } else {
        let config = PdfRenderConfig::new()
            .set_target_size(width as i32, height as i32)
            .render_annotations(false)
            .render_form_data(false);
        pdf_page
            .render_with_config(&config)
            .map_err(internal)?
            .as_rgba_bytes()
    };
    let (width, height) = (width as usize, height as usize);
    let to_pixels = |r: &Rect| pixel_rect(r, size.1, scale, width, height);
    // Zones noircies dans le rendu : aucune image de remplacement ne les montre.
    for area in areas {
        if let Some(zone) = to_pixels(area) {
            blacken(&mut pixels, width, zone);
        }
    }

    // 3. Objets retirés (du dernier au premier : les numéros restent bons).
    for (index, _) in touched.iter().rev() {
        pdf_page
            .objects_mut()
            .remove_object_at_index(*index)
            .map_err(internal)?;
    }

    // 4. Chaque objet retiré est remplacé par son image (hors des zones).
    for (_, bounds) in &touched {
        let Some(zone) = to_pixels(bounds) else {
            continue;
        };
        if areas.iter().any(|a| contains(a, bounds)) {
            continue; // Entièrement caviardé : l'aplat suffit.
        }
        let mut bgra = crop_bgra(&pixels, width, zone);
        let bitmap = PdfBitmap::from_bytes(
            zone.width as i32,
            zone.height as i32,
            PdfBitmapFormat::BGRA,
            &mut bgra,
        )
        .map_err(internal)?;
        let mut object = PdfPageImageObject::new(doc).map_err(internal)?;
        object.set_bitmap(&bitmap).map_err(internal)?;
        // Carré unité de l'image → zone de pixels, sur la page affichée.
        let x0 = zone.x as f64 / scale;
        let y0 = size.1 - (zone.y + zone.height) as f64 / scale;
        let w = zone.width as f64 / scale;
        let h = zone.height as f64 / scale;
        place(
            &mut object,
            &display,
            Rect::from_origin_size(Point::new(x0, y0), w, h),
        )?;
        pdf_page
            .objects_mut()
            .add_image_object(object)
            .map_err(internal)?;
    }

    // 5. Annotations qui touchent une zone : supprimées (liens, notes, champs).
    let annotations = pdf_page.annotations_mut();
    for index in (0..annotations.len()).rev() {
        let annotation = annotations.get(index).map_err(internal)?;
        let bounds = display.rect(rect(annotation.bounds().map_err(internal)?));
        if touches(&bounds) {
            annotations
                .delete_annotation(annotation)
                .map_err(internal)?;
        }
    }

    // 6. Aplats noirs.
    for area in areas {
        black_box(doc, &mut pdf_page, &display, area)?;
    }
    Ok(())
}

fn contains(outer: &Rect, inner: &Rect) -> bool {
    outer.min().x <= inner.min().x
        && outer.min().y <= inner.min().y
        && outer.max().x >= inner.max().x
        && outer.max().y >= inner.max().y
}

/// Rectangle de la page affichée → pixels du rendu, borné à la page ;
/// `None` s'il n'y reste rien.
fn pixel_rect(
    r: &Rect,
    page_height: f64,
    scale: f64,
    width: usize,
    height: usize,
) -> Option<PixelRect> {
    let x0 = (r.min().x * scale).floor().clamp(0.0, width as f64) as usize;
    let x1 = (r.max().x * scale).ceil().clamp(0.0, width as f64) as usize;
    let y0 = ((page_height - r.max().y) * scale)
        .floor()
        .clamp(0.0, height as f64) as usize;
    let y1 = ((page_height - r.min().y) * scale)
        .ceil()
        .clamp(0.0, height as f64) as usize;
    (x1 > x0 && y1 > y0).then_some(PixelRect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

/// Pixels RVBA noirs et opaques dans la zone.
fn blacken(rgba: &mut [u8], width: usize, zone: PixelRect) {
    for row in zone.y..zone.y + zone.height {
        let start = (row * width + zone.x) * 4;
        let end = start + zone.width * 4;
        if let Some(line) = rgba.get_mut(start..end) {
            for pixel in line.as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
}

/// Zone du rendu RVBA, en BVRA (ordre de PDFium).
fn crop_bgra(rgba: &[u8], width: usize, zone: PixelRect) -> Vec<u8> {
    let mut out = Vec::with_capacity(zone.width * zone.height * 4);
    for row in zone.y..zone.y + zone.height {
        let start = (row * width + zone.x) * 4;
        if let Some(line) = rgba.get(start..start + zone.width * 4) {
            for p in line.as_chunks::<4>().0 {
                out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
    }
    out
}

/// Place l'image (carré unité) sur le rectangle `r` de la page affichée.
fn place(
    object: &mut PdfPageImageObject,
    display: &DisplayTransform,
    r: Rect,
) -> Result<(), EngineError> {
    let (a, b) = display.page_vector(r.width(), 0.0);
    let (c, d) = display.page_vector(0.0, r.height());
    let (e, f) = display.page_point(r.min());
    object
        .transform(a as f32, b as f32, c as f32, d as f32, e as f32, f as f32)
        .map_err(internal)
}

fn black_box<'a>(
    doc: &PdfDocument<'a>,
    page: &mut pdfium_render::prelude::PdfPage<'a>,
    display: &DisplayTransform,
    area: &Rect,
) -> Result<(), EngineError> {
    let corners = [
        area.min(),
        Point::new(area.max().x, area.min().y),
        area.max(),
        Point::new(area.min().x, area.max().y),
    ];
    let point = |p: Point| {
        let (x, y) = display.page_point(p);
        (PdfPoints::new(x as f32), PdfPoints::new(y as f32))
    };
    let (x, y) = point(corners[0]);
    let black = PdfColor::new(0, 0, 0, 255);
    let mut path = PdfPagePathObject::new(doc, x, y, None, None, Some(black)).map_err(internal)?;
    for corner in &corners[1..] {
        let (x, y) = point(*corner);
        path.line_to(x, y).map_err(internal)?;
    }
    path.close_path().map_err(internal)?;
    page.objects_mut().add_path_object(path).map_err(internal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_rectangles_become_pixel_zones_inside_the_render() {
        // Page de 100 × 50 pt rendue à 2 px/pt : 200 × 100 px.
        let r = Rect::from_origin_size(Point::new(10.0, 30.0), 20.0, 10.0);
        let zone = pixel_rect(&r, 50.0, 2.0, 200, 100).unwrap();
        assert_eq!(
            zone,
            PixelRect {
                x: 20,
                y: 20,
                width: 40,
                height: 20
            }
        );
        // Débordement : borné à la page ; hors page : rien.
        let wide = Rect::from_origin_size(Point::new(-10.0, -10.0), 30.0, 20.0);
        let zone = pixel_rect(&wide, 50.0, 2.0, 200, 100).unwrap();
        assert_eq!((zone.x, zone.y + zone.height), (0, 100));
        let outside = Rect::from_origin_size(Point::new(200.0, 0.0), 5.0, 5.0);
        assert_eq!(pixel_rect(&outside, 50.0, 2.0, 200, 100), None);
    }

    #[test]
    fn blackened_pixels_and_bgra_crop() {
        // Image 2 × 2 : rouge partout.
        let mut rgba = [255, 0, 0, 255].repeat(4);
        blacken(
            &mut rgba,
            2,
            PixelRect {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
            },
        );
        assert_eq!(&rgba[..8], &[255, 0, 0, 255, 0, 0, 0, 255]);
        let bgra = crop_bgra(
            &rgba,
            2,
            PixelRect {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            },
        );
        assert_eq!(bgra, [0, 0, 255, 255, 0, 0, 0, 255]);
        assert!(contains(
            &Rect::from_origin_size(Point::new(0.0, 0.0), 10.0, 10.0),
            &Rect::from_origin_size(Point::new(1.0, 1.0), 2.0, 2.0),
        ));
    }
}
