//! Moteur de remplacement quand PDFium n'a pas pu être chargé : l'application
//! démarre quand même et explique le problème à chaque ouverture.

use pde_core::document::{DocumentId, PageIndex};
use pde_core::edit::{Element, ElementId};
use pde_core::engine::{EngineError, OpenedDocument, PdfEngine, Placed};
use pde_core::render::{Bitmap, RenderRequest};
use pde_core::text::PageText;

pub struct UnavailableEngine {
    error: EngineError,
}

impl UnavailableEngine {
    pub fn new(error: EngineError) -> Self {
        Self { error }
    }
}

impl PdfEngine for UnavailableEngine {
    fn open(&self, _: Vec<u8>, _: Option<String>) -> Result<OpenedDocument, EngineError> {
        Err(self.error.clone())
    }

    fn render(&self, _: DocumentId, _: RenderRequest) -> Result<Bitmap, EngineError> {
        Err(self.error.clone())
    }

    fn page_text(&self, _: DocumentId, _: PageIndex) -> Result<PageText, EngineError> {
        Err(self.error.clone())
    }

    fn apply_edits(
        &self,
        _: DocumentId,
        _: Vec<(ElementId, Element)>,
    ) -> Result<Vec<Placed>, EngineError> {
        Err(self.error.clone())
    }

    fn save(&self, _: DocumentId) -> Result<Vec<u8>, EngineError> {
        Err(self.error.clone())
    }

    fn close(&self, _: DocumentId) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_request_reports_the_cause() {
        let error = EngineError::Unavailable("pdfium.dll introuvable".into());
        let engine = UnavailableEngine::new(error.clone());
        assert_eq!(engine.open(Vec::new(), None).unwrap_err(), error);
        assert_eq!(engine.page_text(DocumentId(1), 0).unwrap_err(), error);
    }
}
