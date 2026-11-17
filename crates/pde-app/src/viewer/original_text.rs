//! Blocs de texte d'origine d'une page (E-05), tels que l'interface les
//! propose à la modification : rangs des objets, place (origine en haut),
//! texte, police d'origine et style de remplacement. Lus une fois par page
//! et par plan des pages, puis gardés.

use pde_core::document::PageIndex;
use pde_core::edit::TextStyle;
use pde_core::engine::Edition;
use serde::Serialize;

use super::Viewer;
use crate::error::AppError;
use crate::text_view::BoxView;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TextBlockView {
    /// Rangs des objets texte du bloc parmi les objets de la page.
    pub objects: Vec<u32>,
    pub bounds: BoxView,
    pub text: String,
    /// Police d'origine, pour la signaler si elle est remplacée.
    pub font_name: String,
    /// Police standard la plus proche, corps et couleur du bloc.
    pub style: TextStyle,
}

impl Viewer {
    pub fn page_text_blocks(
        &self,
        id: u64,
        page: PageIndex,
    ) -> Result<Vec<TextBlockView>, AppError> {
        self.with_document(id, |open, doc| {
            let size = open.info.page_size(page)?;
            if !open.page_text_blocks.contains_key(&page) {
                let edition = Edition {
                    plan: open.edits.plan().clone(),
                    imports: open.imports.clone(),
                    ..Edition::default()
                };
                let blocks = self.engine.page_text_blocks(doc, edition, page)?;
                open.page_text_blocks.insert(page, blocks);
            }
            Ok(open.page_text_blocks[&page]
                .iter()
                .map(|block| TextBlockView {
                    objects: block.objects.clone(),
                    bounds: BoxView::new(&block.bounds, size),
                    text: block.text.clone(),
                    font_name: block.font_name.clone(),
                    style: block.style.clone(),
                })
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Session;
    use pde_core::edit::{Area, Element, OriginalText};
    use pde_core::geometry::Point;

    #[test]
    fn blocks_are_read_once_per_page_and_layout() {
        let s = Session::new("text-blocks");
        let blocks = s.viewer.page_text_blocks(s.id, 0).unwrap();
        assert_eq!(blocks.len(), 1);
        // Faux moteur : bloc en (72, 755)–(290, 780), origine en bas.
        assert_eq!(blocks[0].bounds.y, 842.0 - 780.0);
        assert_eq!(blocks[0].style.size, 24.0);
        s.viewer.page_text_blocks(s.id, 0).unwrap();
        assert_eq!(s.engine.page_text_block_reads(), 1);
        // Pages réordonnées : relus.
        s.viewer.move_pages(s.id, &[1], 0).unwrap();
        assert!(s.viewer.page_text_blocks(s.id, 0).unwrap().is_empty());
        assert_eq!(s.engine.page_text_block_reads(), 2);
        assert!(s.viewer.page_text_blocks(s.id, 9).is_err());
    }

    fn block(text: &str) -> Element {
        // Coordonnées de l'interface : origine en haut.
        Element::OriginalText(OriginalText {
            page: 0,
            objects: vec![0, 1],
            original: Area {
                top_left: Point::new(72.0, 842.0 - 780.0),
                width: 218.0,
                height: 25.0,
            },
            before: "Bonjour PdfEditor".into(),
            text: text.into(),
            style: Default::default(),
        })
    }

    #[test]
    fn a_rewritten_block_reports_its_replacement_font_and_is_undone_at_once() {
        let s = Session::new("text-rewrite");
        let (added, _) = s.viewer.add_element(s.id, block("Bonsoir")).unwrap();
        let view = s.viewer.edit_view(s.id).unwrap();
        assert!(!view.elements[0].replaced_font);
        assert_eq!(view.elements[0].bounds.y, 842.0 - 780.0);
        s.viewer
            .update_element(s.id, added, block("Привет"))
            .unwrap();
        assert!(s.viewer.edit_view(s.id).unwrap().elements[0].replaced_font);
        // Deux lignes : refusé, rien ne change.
        assert!(s.viewer.update_element(s.id, added, block("a\nb")).is_err());
        s.viewer.undo(s.id).unwrap();
        s.viewer.undo(s.id).unwrap();
        assert!(s.viewer.edit_view(s.id).unwrap().elements.is_empty());
    }
}
