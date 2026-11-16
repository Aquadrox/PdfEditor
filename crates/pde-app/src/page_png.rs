//! Encodage des pages rendues en PNG pour l'affichage.

use pde_core::render::Bitmap;

use crate::error::{AppError, ErrorKind};

/// PNG sans perte, compression rapide : une page A4 à 200 % s'encode en
/// quelques dizaines de millisecondes, et le texte reste net.
pub fn encode_png(bitmap: &Bitmap) -> Result<Vec<u8>, AppError> {
    let failed = |e: png::EncodingError| AppError::new(ErrorKind::Engine, format!("PNG : {e}"));
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, bitmap.width(), bitmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(failed)?;
    writer.write_image_data(bitmap.rgba()).map_err(failed)?;
    writer.finish().map_err(failed)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let rgba: Vec<u8> = (0..3 * 2 * 4).map(|i| (i * 10) as u8).collect();
        let bitmap = Bitmap::new(3, 2, rgba.clone()).unwrap();
        let png_bytes = encode_png(&bitmap).unwrap();
        assert_eq!(&png_bytes[..8], b"\x89PNG\r\n\x1a\n");

        let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
            .read_info()
            .unwrap();
        let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut decoded).unwrap();
        assert_eq!((info.width, info.height), (3, 2));
        assert_eq!(&decoded[..info.buffer_size()], rgba.as_slice());
    }
}
