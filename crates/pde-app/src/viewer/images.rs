//! Images importées dans un document (E-02) : décodées hors du verrou des
//! documents, puis gardées avec lui pour toute la session.

use std::path::Path;
use std::sync::Arc;

use pde_core::edit::ImageId;

use super::{Viewer, read_file};
use crate::edit_view::ImportedImage;
use crate::error::{AppError, ErrorKind};
use crate::images::{MAX_IMAGE_FILE_BYTES, decode};

impl Viewer {
    /// Image choisie dans un fichier (dialogue, glisser-déposer).
    pub fn import_image_file(&self, id: u64, path: &Path) -> Result<ImportedImage, AppError> {
        self.with_document(id, |_, _| Ok(()))?;
        let bytes = read_file(path, MAX_IMAGE_FILE_BYTES)?;
        self.import_image(id, bytes)
    }

    /// Image reçue en octets (presse-papiers).
    pub fn import_image(&self, id: u64, bytes: Vec<u8>) -> Result<ImportedImage, AppError> {
        if bytes.len() as u64 > MAX_IMAGE_FILE_BYTES {
            return Err(AppError::new(
                ErrorKind::FileTooLarge,
                format!("image de plus de {} Mo", MAX_IMAGE_FILE_BYTES >> 20),
            ));
        }
        // Document fermé : inutile de décoder.
        self.with_document(id, |_, _| Ok(()))?;
        let size = bytes.len();
        let asset = decode(bytes)?;
        let (width, height) = (asset.width(), asset.height());
        self.with_document(id, |open, _| {
            let image_id = ImageId(open.images.keys().next_back().map_or(1, |last| last.0 + 1));
            open.images.insert(image_id, Arc::new(asset));
            log::info!("image importée : {width} × {height} px, {} Ko", size / 1024);
            Ok(ImportedImage {
                image_id: image_id.0,
                width,
                height,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use image::codecs::png::PngEncoder;
    use image::{ExtendedColorType, ImageEncoder};
    use pde_core::edit::{Element, ImageBox};
    use pde_core::geometry::Point;

    use super::super::testing::Session;
    use super::*;

    fn png() -> Vec<u8> {
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(&[255; 4 * 6], 3, 2, ExtendedColorType::Rgba8)
            .unwrap();
        out
    }

    /// Cadre en coordonnées de l'interface (origine en haut).
    fn frame(image: u64, top: f64) -> Element {
        Element::Image(ImageBox {
            page: 0,
            top_left: Point::new(100.0, top),
            width: 150.0,
            height: 100.0,
            image: ImageId(image),
            rotation: 0,
            opacity: 1.0,
            keep_ratio: true,
        })
    }

    #[test]
    fn imported_image_can_be_placed_moved_and_undone() {
        let s = Session::new("image");
        let first = s.viewer.import_image(s.id, png()).unwrap();
        let second = s.viewer.import_image(s.id, png()).unwrap();
        assert_eq!((first.image_id, first.width, first.height), (1, 3, 2));
        assert_eq!(second.image_id, 2);

        let (element_id, view) = s.viewer.add_element(s.id, frame(1, 50.0)).unwrap();
        // Coordonnées rendues telles que reçues (origine en haut).
        assert_eq!(view.elements[0].element, frame(1, 50.0));
        assert_eq!(view.elements[0].bounds.y, 50.0);
        assert_eq!(view.elements[0].bounds.height, 100.0);

        let view = s
            .viewer
            .update_element(s.id, element_id, frame(1, 80.0))
            .unwrap();
        assert_eq!(view.elements[0].bounds.y, 80.0);
        let view = s.viewer.undo(s.id).unwrap();
        assert_eq!(view.elements[0].bounds.y, 50.0);
        // L'image reste connue après avoir annulé son ajout.
        s.viewer.undo(s.id).unwrap();
        let view = s.viewer.redo(s.id).unwrap();
        assert_eq!(view.elements.len(), 1);
    }

    #[test]
    fn unknown_image_is_refused() {
        let s = Session::new("image-inconnue");
        let error = s.viewer.add_element(s.id, frame(7, 50.0)).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidRequest);
        assert!(
            error.message.contains("image inconnue"),
            "{}",
            error.message
        );
    }

    #[test]
    fn import_needs_an_open_document_and_a_real_image() {
        let s = Session::new("image-erreurs");
        assert_eq!(
            s.viewer.import_image(999, png()).unwrap_err().kind,
            ErrorKind::UnknownDocument
        );
        assert_eq!(
            s.viewer
                .import_image(s.id, b"texte".to_vec())
                .unwrap_err()
                .kind,
            ErrorKind::InvalidFile
        );
        let path = s.dir.join("photo.png");
        std::fs::write(&path, png()).unwrap();
        assert_eq!(s.viewer.import_image_file(s.id, &path).unwrap().width, 3);
        assert_eq!(
            s.viewer
                .import_image_file(s.id, &s.dir.join("absente.png"))
                .unwrap_err()
                .kind,
            ErrorKind::FileNotFound
        );
    }
}
