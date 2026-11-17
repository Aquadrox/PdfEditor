//! Images d'origine d'une page (E-06), telles que l'interface les propose à
//! la sélection : rang de l'objet et place (origine en haut). Lues une fois
//! par page et par plan des pages (le moteur range une copie de l'original
//! pour les trouver), puis gardées.

use pde_core::document::PageIndex;
use pde_core::engine::Edition;
use serde::Serialize;

use super::Viewer;
use crate::error::AppError;
use crate::text_view::BoxView;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageImageView {
    /// Rang de l'objet image parmi les objets de la page.
    pub object: u32,
    pub bounds: BoxView,
}

impl Viewer {
    pub fn page_images(&self, id: u64, page: PageIndex) -> Result<Vec<PageImageView>, AppError> {
        self.with_document(id, |open, doc| {
            let size = open.info.page_size(page)?;
            if !open.page_images.contains_key(&page) {
                let edition = Edition {
                    plan: open.edits.plan().clone(),
                    imports: open.imports.clone(),
                    ..Edition::default()
                };
                let images = self.engine.page_images(doc, edition, page)?;
                open.page_images.insert(page, images);
            }
            Ok(open.page_images[&page]
                .iter()
                .map(|image| PageImageView {
                    object: image.object,
                    bounds: BoxView::new(&image.bounds, size),
                })
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Session;

    #[test]
    fn images_are_read_once_per_page_and_layout() {
        let s = Session::new("page-images");
        let images = s.viewer.page_images(s.id, 0).unwrap();
        assert_eq!(images.len(), 1);
        // Faux moteur : image en (100, 600)–(300, 700), origine en bas.
        assert_eq!(images[0].bounds.y, 842.0 - 700.0);
        s.viewer.page_images(s.id, 0).unwrap();
        assert_eq!(s.engine.page_image_reads(), 1);
        // Pages réordonnées : relues.
        s.viewer.move_pages(s.id, &[1], 0).unwrap();
        assert!(s.viewer.page_images(s.id, 0).unwrap().is_empty());
        assert_eq!(s.engine.page_image_reads(), 2);
        assert!(s.viewer.page_images(s.id, 9).is_err());
    }
}
