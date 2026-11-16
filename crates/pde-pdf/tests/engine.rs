//! Tests de non-régression du moteur PDFium sur les PDF de `tests/fixtures`
//! (régénérables avec `make_fixtures.py`).
//!
//! PDFium doit être présent : `scripts/fetch-pdfium.sh` le télécharge.

use pde_core::document::{OutlineItem, PageSize};
use pde_core::engine::{EngineError, OpenedDocument, PdfEngine};
use pde_core::render::RenderRequest;
use pde_pdf::PdfiumEngine;

const SIMPLE: &[u8] = include_bytes!("fixtures/simple.pdf");
const ENCRYPTED: &[u8] = include_bytes!("fixtures/encrypted.pdf");
/// Mot de passe fictif du PDF chiffré de test (voir make_fixtures.py).
const TEST_PASSWORD: &str = "mot-de-passe-de-test";

fn engine() -> &'static PdfiumEngine {
    PdfiumEngine::global().expect("PDFium absent : lancer scripts/fetch-pdfium.sh")
}

fn open_simple() -> OpenedDocument {
    engine()
        .open(SIMPLE.to_vec(), None)
        .expect("simple.pdf doit s'ouvrir")
}

fn titles(items: &[OutlineItem]) -> Vec<(&str, Option<usize>)> {
    items.iter().map(|i| (i.title.as_str(), i.page)).collect()
}

#[test]
fn describes_pages_title_and_outline() {
    let doc = open_simple();
    assert_eq!(
        doc.info.page_sizes,
        // Page 2 tournée d'un quart de tour : affichée en paysage.
        vec![PageSize::new(595.0, 842.0), PageSize::new(842.0, 595.0)]
    );
    assert_eq!(doc.info.title.as_deref(), Some("Document de test"));
    let outline = &doc.info.outline;
    assert_eq!(
        titles(outline),
        [("Introduction", Some(0)), ("Annexe", Some(1))]
    );
    assert_eq!(titles(&outline[1].children), [("Détail", Some(1))]);
    engine().close(doc.id);
}

#[test]
fn renders_at_the_requested_scale() {
    let doc = open_simple();
    for (scale, size) in [(1.0, (595, 842)), (2.0, (1190, 1684)), (0.25, (149, 211))] {
        let bitmap = engine()
            .render(doc.id, RenderRequest { page: 0, scale })
            .unwrap();
        assert_eq!((bitmap.width(), bitmap.height()), size, "zoom {scale}");
    }
    engine().close(doc.id);
}

#[test]
fn rendered_page_shows_its_content() {
    let doc = open_simple();
    let bitmap = engine()
        .render(
            doc.id,
            RenderRequest {
                page: 0,
                scale: 1.0,
            },
        )
        .unwrap();
    // Fond blanc dans les coins.
    assert_eq!(bitmap.pixel(2, 2), Some([255, 255, 255, 255]));
    // Rectangle noir de 72,600 à 272,700 (en points PDF, origine en bas) :
    // à l'écran, de y = 842 - 700 = 142 à y = 242.
    let [r, g, b, _] = bitmap.pixel(172, 192).unwrap();
    assert!(
        r < 30 && g < 30 && b < 30,
        "pixel ({r}, {g}, {b}) attendu noir"
    );
    engine().close(doc.id);
}

#[test]
fn rotated_page_is_rendered_in_landscape() {
    let doc = open_simple();
    let bitmap = engine()
        .render(
            doc.id,
            RenderRequest {
                page: 1,
                scale: 1.0,
            },
        )
        .unwrap();
    assert_eq!((bitmap.width(), bitmap.height()), (842, 595));
    engine().close(doc.id);
}

#[test]
fn extracts_text_with_positions() {
    let doc = open_simple();
    let text = engine().page_text(doc.id, 0).unwrap();
    assert!(
        text.text().contains("Bonjour PdfEditor"),
        "{:?}",
        text.text()
    );
    // Texte posé à x = 72, ligne de base y = 760, corps 24 pt.
    let b = text.chars.iter().find(|c| c.ch == 'B').unwrap();
    assert!((b.bounds.min().x - 72.0).abs() < 2.0, "{:?}", b.bounds);
    assert!(
        b.bounds.min().y > 750.0 && b.bounds.max().y < 790.0,
        "{:?}",
        b.bounds
    );
    assert!(
        engine()
            .page_text(doc.id, 1)
            .unwrap()
            .text()
            .contains("Page deux")
    );
    engine().close(doc.id);
}

#[test]
fn encrypted_document_needs_the_right_password() {
    assert_eq!(
        engine().open(ENCRYPTED.to_vec(), None).unwrap_err(),
        EngineError::PasswordRequired
    );
    assert_eq!(
        engine()
            .open(ENCRYPTED.to_vec(), Some("autre".into()))
            .unwrap_err(),
        EngineError::WrongPassword
    );
    let doc = engine()
        .open(ENCRYPTED.to_vec(), Some(TEST_PASSWORD.into()))
        .unwrap();
    assert_eq!(doc.info.page_sizes.len(), 2);
    assert!(
        engine()
            .page_text(doc.id, 0)
            .unwrap()
            .text()
            .contains("Bonjour")
    );
    engine().close(doc.id);
}

#[test]
fn invalid_files_are_refused() {
    for bytes in [
        b"ceci n'est pas un PDF".to_vec(),
        Vec::new(),
        SIMPLE[..200].to_vec(),
    ] {
        let error = engine().open(bytes, None).unwrap_err();
        assert!(matches!(error, EngineError::InvalidFile(_)), "{error:?}");
    }
}

#[test]
fn page_out_of_range_and_bad_scale() {
    let doc = open_simple();
    let out_of_range = EngineError::PageOutOfRange { page: 2, count: 2 };
    assert_eq!(
        engine()
            .render(
                doc.id,
                RenderRequest {
                    page: 2,
                    scale: 1.0
                }
            )
            .unwrap_err(),
        out_of_range
    );
    assert_eq!(engine().page_text(doc.id, 2).unwrap_err(), out_of_range);
    assert!(matches!(
        engine().render(
            doc.id,
            RenderRequest {
                page: 0,
                scale: 0.0
            }
        ),
        Err(EngineError::InvalidRender(_))
    ));
    engine().close(doc.id);
}

#[test]
fn closed_document_is_unknown() {
    let doc = open_simple();
    engine().close(doc.id);
    engine().close(doc.id); // deuxième fermeture : sans effet
    assert_eq!(
        engine().page_text(doc.id, 0).unwrap_err(),
        EngineError::UnknownDocument
    );
}

#[test]
fn usable_from_several_threads() {
    let doc = open_simple();
    let id = doc.id;
    let workers: Vec<_> = (0..4)
        .map(|i| {
            std::thread::spawn(move || {
                let page = i % 2;
                engine()
                    .render(id, RenderRequest { page, scale: 0.5 })
                    .unwrap()
            })
        })
        .collect();
    for worker in workers {
        assert!(worker.join().unwrap().width() > 0);
    }
    engine().close(id);
}
