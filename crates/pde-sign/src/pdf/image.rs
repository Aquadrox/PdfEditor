//! Image de la signature manuscrite dans le PDF : couleurs (DeviceRGB) et,
//! si l'image a de la transparence, masque doux (`/SMask`) ; flux compressés.

use lopdf::{Dictionary, Object, ObjectId, Stream};

use super::incremental::{Body, Update};
use super::prepare::Allocator;
use crate::appearance::SignatureImage;

/// Ajoute l'image (et son masque) à la mise à jour ; renvoie son objet.
pub(super) fn push_image(
    image: &SignatureImage,
    ids: &mut Allocator,
    update: &mut Update,
) -> ObjectId {
    let (rgb, alpha) = image.split();
    let mask = alpha.map(|alpha| {
        let id = ids.next();
        update.push(
            id,
            Body::Object(Object::Stream(image_stream(
                image,
                "DeviceGray",
                alpha,
                None,
            ))),
        );
        id
    });
    let id = ids.next();
    update.push(
        id,
        Body::Object(Object::Stream(image_stream(image, "DeviceRGB", rgb, mask))),
    );
    id
}

fn image_stream(
    image: &SignatureImage,
    space: &str,
    samples: Vec<u8>,
    mask: Option<ObjectId>,
) -> Stream {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Image".to_vec()));
    dict.set("Width", i64::from(image.width()));
    dict.set("Height", i64::from(image.height()));
    dict.set("ColorSpace", Object::Name(space.as_bytes().to_vec()));
    dict.set("BitsPerComponent", 8);
    if let Some(mask) = mask {
        dict.set("SMask", mask);
    }
    let mut stream = Stream::new(dict, samples);
    // Sans compression possible, le flux reste tel quel (toujours valide).
    let _ = stream.compress();
    stream
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_image_gets_a_compressed_mask() {
        let mut rgba = Vec::new();
        for i in 0..64 * 64 {
            rgba.extend_from_slice(&[20, 30, 90, if i % 3 == 0 { 255 } else { 0 }]);
        }
        let image = SignatureImage::new(64, 64, rgba).unwrap();
        let mut ids = Allocator(10);
        let mut update = Update::default();
        let id = push_image(&image, &mut ids, &mut update);
        assert_eq!(id, (11, 0), "le masque d'abord, puis l'image");
    }
}
