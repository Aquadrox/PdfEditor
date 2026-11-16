//! Écriture des images ajoutées : un JPEG est inclus tel quel (aucune
//! recompression), les autres formats en pixels avec leur transparence ;
//! l'opacité réduit la transparence de chaque pixel.

use std::io::Cursor;

use pde_core::edit::{ImageAsset, ImageBox, ImageData};
use pde_core::engine::EngineError;
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{
    PdfBitmap, PdfBitmapFormat, PdfColor, PdfDocument, PdfPageImageObject, PdfPageObjectCommon,
    PdfPageObjectsCommon,
};

use crate::convert::internal;

pub(super) fn write_image(
    doc: &mut PdfDocument,
    image: &ImageBox,
    asset: &ImageAsset,
) -> Result<Rect, EngineError> {
    let mut object = match asset.data() {
        ImageData::Jpeg(bytes) => {
            let mut object =
                PdfPageImageObject::new_from_jpeg_reader(doc, Cursor::new(bytes.as_slice()))
                    .map_err(internal)?;
            if image.opacity < 1.0 {
                // Pas de transparence dans un JPEG : opacité de l'objet.
                object
                    .set_fill_color(PdfColor::new(0, 0, 0, alpha(image.opacity)))
                    .map_err(internal)?;
            }
            object
        }
        ImageData::Rgba(rgba) => {
            let mut bgra = to_bgra(rgba, image.opacity);
            let width = pixels(asset.width())?;
            let height = pixels(asset.height())?;
            let bitmap = PdfBitmap::from_bytes(width, height, PdfBitmapFormat::BGRA, &mut bgra)
                .map_err(internal)?;
            let mut object = PdfPageImageObject::new(doc).map_err(internal)?;
            object.set_bitmap(&bitmap).map_err(internal)?;
            object
        }
    };

    let (mut page, display) = super::page_of(doc, image.page)?;
    let [a, b, c, d, e, f] = image.placement();
    let (a, b) = display.page_vector(a, b);
    let (c, d) = display.page_vector(c, d);
    let (e, f) = display.page_point(Point::new(e, f));
    object
        .transform(a as f32, b as f32, c as f32, d as f32, e as f32, f as f32)
        .map_err(internal)?;
    page.objects_mut()
        .add_image_object(object)
        .map_err(internal)?;
    Ok(image.bounds())
}

fn pixels(n: u32) -> Result<i32, EngineError> {
    i32::try_from(n).map_err(|_| EngineError::Internal(format!("image trop grande : {n} px")))
}

fn alpha(opacity: f64) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// RVBA → BVRA (ordre des octets de PDFium), transparence × opacité.
fn to_bgra(rgba: &[u8], opacity: f64) -> Vec<u8> {
    let factor = opacity.clamp(0.0, 1.0);
    rgba.chunks_exact(4)
        .flat_map(|p| {
            let a = (f64::from(p[3]) * factor).round() as u8;
            [p[2], p[1], p[0], a]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_order_and_opacity() {
        assert_eq!(
            to_bgra(&[1, 2, 3, 255, 4, 5, 6, 100], 1.0),
            [3, 2, 1, 255, 6, 5, 4, 100]
        );
        assert_eq!(to_bgra(&[1, 2, 3, 200], 0.5), [3, 2, 1, 100]);
        assert_eq!(alpha(0.5), 128);
    }
}
