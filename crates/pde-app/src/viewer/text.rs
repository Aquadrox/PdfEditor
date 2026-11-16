//! Texte des pages : couche de texte sélectionnable et recherche.

use std::sync::Arc;

use pde_core::document::{DocumentId, PageIndex};
use pde_core::search::{find, match_rects};
use pde_core::text::PageText;

use super::Viewer;
use crate::error::AppError;
use crate::lock;
use crate::text_view::{BoxView, PageTextView, SearchHit, SearchResults};

/// Au-delà, la recherche s'arrête : l'interface n'en afficherait pas plus.
pub const MAX_SEARCH_HITS: usize = 1000;

impl Viewer {
    /// Texte d'une page, lu une fois puis gardé jusqu'à la fermeture.
    fn text(&self, doc: DocumentId, page: PageIndex) -> Result<Arc<PageText>, AppError> {
        if let Some(text) = lock(&self.texts).get(&(doc, page)) {
            return Ok(text.clone());
        }
        let text = Arc::new(self.engine.page_text(doc, page)?);
        lock(&self.texts).insert((doc, page), text.clone());
        Ok(text)
    }

    /// Segments de texte d'une page, pour la couche sélectionnable.
    pub fn page_text(&self, id: u64, page: PageIndex) -> Result<PageTextView, AppError> {
        let doc = DocumentId(id);
        let size = self.page_size(doc, page)?;
        Ok(PageTextView::new(self.text(doc, page)?.runs(), size))
    }

    /// Occurrences de `query` dans tout le document, page par page.
    pub fn search(&self, id: u64, query: &str) -> Result<SearchResults, AppError> {
        let doc = DocumentId(id);
        let mut hits = Vec::new();
        for (page, size) in self.page_sizes(doc)?.into_iter().enumerate() {
            let text = self.text(doc, page)?;
            for range in find(&text, query) {
                if hits.len() == MAX_SEARCH_HITS {
                    return Ok(SearchResults {
                        hits,
                        truncated: true,
                    });
                }
                let rects = match_rects(&text, range)
                    .iter()
                    .map(|r| BoxView::new(r, size))
                    .collect();
                hits.push(SearchHit { page, rects });
            }
        }
        Ok(SearchResults {
            hits,
            truncated: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::error::ErrorKind;
    use crate::fake::{FakeEngine, SIMPLE_PDF};
    use crate::viewer::Viewer;
    use std::sync::Arc;

    fn open() -> (Arc<FakeEngine>, Viewer, u64, std::path::PathBuf) {
        let engine = Arc::new(FakeEngine::default());
        let viewer = Viewer::new(engine.clone(), 1024);
        let path = std::env::temp_dir().join(format!(
            "pde-text-{}-{:?}.pdf",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&path, SIMPLE_PDF).unwrap();
        let id = viewer.open_file(&path, None).unwrap().id;
        (engine, viewer, id, path)
    }

    #[test]
    fn page_text_is_grouped_in_runs_with_top_origin() {
        let (_, viewer, id, path) = open();
        let view = viewer.page_text(id, 0).unwrap();
        assert_eq!(view.runs.len(), 1);
        assert_eq!(view.runs[0].text, "Bonjour monde");
        // Texte posé à y = 800 (bas du texte) sur une page de 842 pt.
        assert_eq!(view.runs[0].area.y, 842.0 - 810.0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn text_is_read_once_per_page() {
        let (engine, viewer, id, path) = open();
        viewer.page_text(id, 0).unwrap();
        viewer.page_text(id, 0).unwrap();
        viewer.search(id, "monde").unwrap();
        assert_eq!(engine.text_reads(), 2); // pages 0 et 1, une fois chacune
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn search_covers_all_pages() {
        let (_, viewer, id, path) = open();
        let results = viewer.search(id, "MONDE").unwrap();
        let pages: Vec<usize> = results.hits.iter().map(|h| h.page).collect();
        assert_eq!(pages, [0, 1]);
        assert!(!results.truncated);
        assert_eq!(results.hits[0].rects.len(), 1);
        assert!(viewer.search(id, "absent").unwrap().hits.is_empty());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn closed_document_has_no_text() {
        let (_, viewer, id, path) = open();
        viewer.page_text(id, 0).unwrap();
        viewer.close(id);
        assert_eq!(
            viewer.page_text(id, 0).unwrap_err().kind,
            ErrorKind::UnknownDocument
        );
        assert_eq!(
            viewer.search(id, "x").unwrap_err().kind,
            ErrorKind::UnknownDocument
        );
        std::fs::remove_file(path).unwrap();
    }
}
