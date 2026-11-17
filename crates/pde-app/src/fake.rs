//! Faux moteur PDF pour les tests : documents décrits par leur contenu,
//! comptage des rendus et des fermetures.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use pde_core::document::{DocumentId, DocumentInfo, PageIndex, PageSize};
use pde_core::edit::{Element, ElementId, Images, PagePlan, leading};
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
/// Début d'un faux PDF de N pages A4 (« %PDF-faux pages=3 »), produit à
/// l'enregistrement d'un document dont le nombre de pages a changé.
const PAGES_PDF: &str = "%PDF-faux pages=";

/// Contenu d'un faux PDF de `count` pages A4.
pub fn pdf_with_pages(count: usize) -> Vec<u8> {
    if count == 2 {
        SIMPLE_PDF.to_vec()
    } else {
        format!("{PAGES_PDF}{count}").into_bytes()
    }
}

#[derive(Default)]
pub struct FakeEngine {
    next_id: AtomicU64,
    renders: AtomicUsize,
    text_reads: AtomicUsize,
    closed: Mutex<Vec<u64>>,
    /// Taille des pages de chaque document : à l'ouverture, puis selon le
    /// dernier plan appliqué.
    pages: Mutex<HashMap<DocumentId, Pages>>,
}

/// Tailles des pages d'un document : à l'ouverture, et telles qu'affichées.
type Pages = (Vec<PageSize>, Vec<PageSize>);

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

    fn info(count: usize) -> DocumentInfo {
        DocumentInfo {
            page_sizes: vec![PageSize::new(595.0, 842.0); count],
            title: None,
            outline: Vec::new(),
        }
    }

    fn current_sizes(&self, doc: DocumentId) -> Vec<PageSize> {
        self.pages
            .lock()
            .unwrap()
            .get(&doc)
            .map(|(_, current)| current.clone())
            .unwrap_or_default()
    }
}

impl PdfEngine for FakeEngine {
    fn open(
        &self,
        bytes: Vec<u8>,
        password: Option<String>,
    ) -> Result<OpenedDocument, EngineError> {
        let count = match bytes.as_slice() {
            SIMPLE_PDF => 2,
            LOCKED_PDF => match password.as_deref() {
                None => return Err(EngineError::PasswordRequired),
                Some(PASSWORD) => 2,
                Some(_) => return Err(EngineError::WrongPassword),
            },
            other => std::str::from_utf8(other)
                .ok()
                .and_then(|s| s.strip_prefix(PAGES_PDF))
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| EngineError::InvalidFile("pas un PDF".into()))?,
        };
        let id = DocumentId(self.next_id.fetch_add(1, Ordering::SeqCst) + 1);
        let info = Self::info(count);
        self.pages
            .lock()
            .unwrap()
            .insert(id, (info.page_sizes.clone(), info.page_sizes.clone()));
        Ok(OpenedDocument { id, info })
    }

    fn render(&self, doc: DocumentId, request: RenderRequest) -> Result<Bitmap, EngineError> {
        self.renders.fetch_add(1, Ordering::SeqCst);
        let size = self.current_sizes(doc).get(request.page).copied().ok_or(
            EngineError::PageOutOfRange {
                page: request.page,
                count: self.current_sizes(doc).len(),
            },
        )?;
        let (w, h) = pixel_size(size, request.scale)?;
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

    /// Pages rangées selon le plan ; chaque zone de texte occupe une
    /// ligne ; le texte UNWRITABLE échoue.
    fn apply_edits(
        &self,
        doc: DocumentId,
        plan: PagePlan,
        elements: Vec<(ElementId, Element)>,
        _images: Images,
    ) -> Result<Vec<Placed>, EngineError> {
        let mut pages = self.pages.lock().unwrap();
        let (originals, current) = pages.get_mut(&doc).ok_or(EngineError::UnknownDocument)?;
        plan.validate(originals.len())
            .map_err(EngineError::Internal)?;
        *current = plan.sizes(originals);
        drop(pages);
        elements
            .into_iter()
            .map(|(id, element)| match element {
                Element::Text(text) if text.text == UNWRITABLE => {
                    Err(EngineError::Internal("écriture impossible".into()))
                }
                Element::Image(image) => Ok(Placed {
                    id,
                    bounds: image.bounds(),
                }),
                Element::Shape(shape) => Ok(Placed {
                    id,
                    bounds: shape.bounds(),
                }),
                Element::Ink(ink) => Ok(Placed {
                    id,
                    bounds: ink.bounds(),
                }),
                Element::Markup(markup) => Ok(Placed {
                    id,
                    bounds: markup.bounds(),
                }),
                Element::Note(note) => Ok(Placed {
                    id,
                    bounds: note.bounds(),
                }),
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

    /// Enregistre un PDF reconnu par ce faux moteur, avec le bon nombre de pages.
    fn save(&self, doc: DocumentId) -> Result<Vec<u8>, EngineError> {
        Ok(pdf_with_pages(self.current_sizes(doc).len()))
    }

    fn close(&self, doc: DocumentId) {
        self.pages.lock().unwrap().remove(&doc);
        self.closed.lock().unwrap().push(doc.0);
    }
}
