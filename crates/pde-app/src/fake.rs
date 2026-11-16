//! Faux moteur PDF pour les tests : documents décrits par leur contenu,
//! comptage des rendus et des fermetures.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use pde_core::document::{DocumentId, DocumentInfo, PageIndex, PageSize};
use pde_core::edit::{Element, ElementId, leading};
use pde_core::engine::{EngineError, OpenedDocument, PdfEngine, Placed};
use pde_core::geometry::{Point, Rect};
use pde_core::render::{Bitmap, RenderRequest, pixel_size};
use pde_core::text::{PageText, TextChar};

/// Contenu reconnu comme un PDF de deux pages A4.
pub const SIMPLE_PDF: &[u8] = b"%PDF-faux simple";
/// Contenu reconnu comme un PDF protégé par [`PASSWORD`].
pub const LOCKED_PDF: &[u8] = b"%PDF-faux verrouille";
pub const PASSWORD: &str = "ouvre-toi";
/// Texte qu'un faux moteur refuse d'écrire (test des échecs).
pub const UNWRITABLE: &str = "ÉCHEC";

#[derive(Default)]
pub struct FakeEngine {
    next_id: AtomicU64,
    renders: AtomicUsize,
    text_reads: AtomicUsize,
    closed: Mutex<Vec<u64>>,
}

impl FakeEngine {
    pub fn render_count(&self) -> usize {
        self.renders.load(Ordering::SeqCst)
    }

    pub fn text_reads(&self) -> usize {
        self.text_reads.load(Ordering::SeqCst)
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

    /// Page 0 : « Bonjour monde » ; page 1 : « le monde entier » ; une
    /// ligne posée à y = 800, caractères de 6 × 10 pt.
    fn page_text(&self, _doc: DocumentId, page: PageIndex) -> Result<PageText, EngineError> {
        self.text_reads.fetch_add(1, Ordering::SeqCst);
        let line = match page {
            0 => "Bonjour monde",
            1 => "le monde entier",
            _ => "",
        };
        let chars = line
            .chars()
            .enumerate()
            .map(|(i, ch)| {
                let x = 72.0 + 6.0 * i as f64;
                TextChar {
                    ch,
                    bounds: Rect::from_corners(Point::new(x, 800.0), Point::new(x + 6.0, 810.0)),
                }
            })
            .collect();
        Ok(PageText { chars })
    }

    /// Chaque zone de texte occupe une ligne ; le texte UNWRITABLE échoue.
    fn apply_edits(
        &self,
        _doc: DocumentId,
        elements: Vec<(ElementId, Element)>,
    ) -> Result<Vec<Placed>, EngineError> {
        elements
            .into_iter()
            .map(|(id, element)| match element {
                Element::Text(text) if text.text == UNWRITABLE => {
                    Err(EngineError::Internal("écriture impossible".into()))
                }
                Element::Text(text) => {
                    let height = leading(&text.style);
                    Ok(Placed {
                        id,
                        bounds: Rect::from_corners(
                            Point::new(text.top_left.x, text.top_left.y - height),
                            Point::new(text.top_left.x + text.width, text.top_left.y),
                        ),
                    })
                }
            })
            .collect()
    }

    /// Enregistre un PDF reconnu par ce faux moteur.
    fn save(&self, _doc: DocumentId) -> Result<Vec<u8>, EngineError> {
        Ok(SIMPLE_PDF.to_vec())
    }

    fn close(&self, doc: DocumentId) {
        self.closed.lock().unwrap().push(doc.0);
    }
}
