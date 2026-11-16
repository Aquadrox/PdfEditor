//! Import des images (E-02) : format reconnu à son contenu, taille bornée
//! avant tout décodage. Un JPEG est gardé tel quel (le PDF l'inclut sans
//! le recompresser) ; PNG, WebP et les JPEG pivotés par leur EXIF sont
//! décodés en pixels.

use std::io::Cursor;

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use pde_core::edit::ImageAsset;

use crate::error::{AppError, ErrorKind};

/// Plus gros fichier d'image accepté.
pub const MAX_IMAGE_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Plus grande image acceptée : 60 mégapixels (photo d'appareil haut de gamme).
pub const MAX_IMAGE_PIXELS: u64 = 60_000_000;
/// Plus grand côté accepté (pixels).
const MAX_SIDE: u32 = 30_000;

fn unsupported() -> AppError {
    AppError::new(
        ErrorKind::InvalidFile,
        "format d'image non pris en charge : PNG, JPEG ou WebP",
    )
}

fn unreadable(e: image::ImageError) -> AppError {
    AppError::new(ErrorKind::InvalidFile, format!("image illisible : {e}"))
}

pub fn decode(bytes: Vec<u8>) -> Result<ImageAsset, AppError> {
    decode_with_limit(bytes, MAX_IMAGE_PIXELS)
}

fn decode_with_limit(bytes: Vec<u8>, max_pixels: u64) -> Result<ImageAsset, AppError> {
    let format = image::guess_format(&bytes).map_err(|_| unsupported())?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(unsupported());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes.as_slice()), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(max_pixels.saturating_mul(8));
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(unreadable)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > max_pixels {
        return Err(AppError::new(
            ErrorKind::FileTooLarge,
            format!(
                "image de {width} × {height} pixels, limite {} mégapixels",
                max_pixels / 1_000_000
            ),
        ));
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    if format == ImageFormat::Jpeg && orientation == Orientation::NoTransforms {
        drop(decoder);
        return ImageAsset::jpeg(width, height, bytes).ok_or_else(unsupported);
    }
    let mut image = DynamicImage::from_decoder(decoder).map_err(unreadable)?;
    image.apply_orientation(orientation);
    let rgba = image.into_rgba8();
    let (width, height) = rgba.dimensions();
    ImageAsset::rgba(width, height, rgba.into_raw()).ok_or_else(unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::jpeg::JpegEncoder;
    use image::codecs::png::PngEncoder;
    use image::codecs::webp::WebPEncoder;
    use image::{ExtendedColorType, ImageEncoder};
    use pde_core::edit::ImageData;

    fn rgba(width: u32, height: u32) -> Vec<u8> {
        (0..width * height)
            .flat_map(|i| [200, 10, (i % 256) as u8, if i % 2 == 0 { 255 } else { 0 }])
            .collect()
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(
                &rgba(width, height),
                width,
                height,
                ExtendedColorType::Rgba8,
            )
            .unwrap();
        out
    }

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let rgb = vec![120; (width * height * 3) as usize];
        let mut out = Vec::new();
        JpegEncoder::new(&mut out)
            .encode(&rgb, width, height, ExtendedColorType::Rgb8)
            .unwrap();
        out
    }

    /// JPEG avec un segment EXIF « pivoter de 90° » (orientation 6).
    fn rotated_jpeg(width: u32, height: u32) -> Vec<u8> {
        let mut tiff = b"Exif\0\0II*\0\x08\0\0\0\x01\0".to_vec();
        tiff.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0]);
        tiff.extend_from_slice(&[0, 0, 0, 0]);
        let length = u16::try_from(tiff.len() + 2).unwrap().to_be_bytes();
        let plain = jpeg(width, height);
        let mut out = plain[..2].to_vec();
        out.extend_from_slice(&[0xff, 0xe1, length[0], length[1]]);
        out.extend_from_slice(&tiff);
        out.extend_from_slice(&plain[2..]);
        out
    }

    #[test]
    fn png_is_decoded_with_its_transparency() {
        let asset = decode(png(3, 2)).unwrap();
        assert_eq!((asset.width(), asset.height()), (3, 2));
        let ImageData::Rgba(pixels) = asset.data() else {
            panic!("PNG attendu en pixels");
        };
        assert_eq!(pixels, &rgba(3, 2));
    }

    #[test]
    fn jpeg_is_kept_as_is() {
        let bytes = jpeg(16, 8);
        let asset = decode(bytes.clone()).unwrap();
        assert_eq!((asset.width(), asset.height()), (16, 8));
        assert_eq!(asset.data(), &ImageData::Jpeg(bytes));
    }

    #[test]
    fn exif_rotation_is_applied() {
        let asset = decode(rotated_jpeg(16, 8)).unwrap();
        // Pivotée d'un quart de tour : largeur et hauteur échangées.
        assert_eq!((asset.width(), asset.height()), (8, 16));
        assert!(matches!(asset.data(), ImageData::Rgba(_)));
    }

    #[test]
    fn webp_is_decoded() {
        let mut out = Vec::new();
        WebPEncoder::new_lossless(&mut out)
            .encode(&rgba(4, 4), 4, 4, ExtendedColorType::Rgba8)
            .unwrap();
        let asset = decode(out).unwrap();
        assert_eq!((asset.width(), asset.height()), (4, 4));
    }

    #[test]
    fn other_formats_and_garbage_are_refused() {
        for bytes in [
            b"GIF89a\x01\0\x01\0".to_vec(),
            b"pas une image".to_vec(),
            Vec::new(),
        ] {
            assert_eq!(decode(bytes).unwrap_err().kind, ErrorKind::InvalidFile);
        }
        // En-tête PNG sans contenu.
        let truncated = png(3, 2)[..30].to_vec();
        assert_eq!(decode(truncated).unwrap_err().kind, ErrorKind::InvalidFile);
    }

    #[test]
    fn oversized_images_are_refused_before_decoding() {
        let error = decode_with_limit(png(5, 5), 20).unwrap_err();
        assert_eq!(error.kind, ErrorKind::FileTooLarge);
        assert!(error.message.contains("5 × 5"), "{}", error.message);
    }
}
