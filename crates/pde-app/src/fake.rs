//! Faux moteur PDF pour les tests : documents décrits par leur contenu,
//! comptage des rendus et des fermetures.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use pde_core::document::{DocumentId, DocumentInfo, PageIndex, PageSize};
use pde_core::engine::{EngineError, OpenedDocument, PdfEngine};
use pde_core::render::{Bitmap, RenderRequest, pixel_size};
use pde_core::text::PageText;

/// Contenu reconnu comme un PDF de deux pages A4.
pub const SIMPLE_PDF: &[u8] = b"%PDF-faux simple";
/// Contenu reconnu comme un PDF protégé par [`PASSWORD`].
pub const LOCKED_PDF: &[u8] = b"%PDF-faux verrouille";
pub const PASSWORD: &str = "ouvre-toi";

#[derive(Default)]
pub struct FakeEngine {
    next_id: AtomicU64,
    renders: AtomicUsize,
    closed: Mutex<Vec<u64>>,
}

impl FakeEngine {
    pub fn render_count(&self) -> usize {
        self.renders.load(Ordering::SeqCst)
    }

    pub fn closed(&self) -> Vec<u64> {
        self.closed.lock().unwrap().clone()
    }

    fn info() -> DocumentInfo {
        DocumentInfo {
            page_sizes: vec![PageSize::new(595.0, 842.0); 2],
            title: None,
            outline: Vec::new(),
        }
    }
}

impl PdfEngine for FakeEngine {
    fn open(
        &self,
        bytes: Vec<u8>,
        password: Option<String>,
    ) -> Result<OpenedDocument, EngineError> {
        match bytes.as_slice() {
            SIMPLE_PDF => {}
            LOCKED_PDF => match password.as_deref() {
                None => return Err(EngineError::PasswordRequired),
                Some(PASSWORD) => {}
                Some(_) => return Err(EngineError::WrongPassword),
            },
            _ => return Err(EngineError::InvalidFile("pas un PDF".into())),
        }
        let id = DocumentId(self.next_id.fetch_add(1, Ordering::SeqCst) + 1);
        Ok(OpenedDocument {
            id,
            info: Self::info(),
        })
    }

    fn render(&self, _doc: DocumentId, request: RenderRequest) -> Result<Bitmap, EngineError> {
        self.renders.fetch_add(1, Ordering::SeqCst);
        let (w, h) = pixel_size(Self::info().page_size(request.page)?, request.scale)?;
        Bitmap::new(w, h, vec![255; (w * h * 4) as usize])
    }

    fn page_text(&self, _doc: DocumentId, _page: PageIndex) -> Result<PageText, EngineError> {
        Ok(PageText::default())
    }

    fn close(&self, doc: DocumentId) {
        self.closed.lock().unwrap().push(doc.0);
    }
}
