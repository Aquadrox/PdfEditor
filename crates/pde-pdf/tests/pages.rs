//! Pages rangées selon le plan (P-01, P-02) : ordre, pages blanches,
//! copies, rotations, enregistrement. Nécessite PDFium
//! (`scripts/fetch-pdfium.sh`).

mod common;

use common::{ENCRYPTED, TEST_PASSWORD, engine, open_simple};
use pde_core::document::{DocumentId, PageSize};
use pde_core::edit::{Element, ElementId, Images, PagePlan, PlannedPage, TextBox, TextStyle};
use pde_core::engine::{OpenedDocument, PdfEngine};
use pde_core::geometry::Point;
use pde_core::render::RenderRequest;

/// simple.pdf : page 1 en A4 (« Bonjour PdfEditor »), page 2 déjà tournée
/// d'un quart de tour (« Page deux », paysage).
const A4: PageSize = PageSize::new(595.0, 842.0);
const LANDSCAPE: PageSize = PageSize::new(842.0, 595.0);

fn arrange(doc: DocumentId, pages: Vec<PlannedPage>, elements: Vec<(ElementId, Element)>) {
    engine()
        .apply_edits(doc, PagePlan::new(pages), elements, Images::new())
        .expect("pages rangées");
}

fn turned(index: usize, rotation: u8) -> PlannedPage {
    PlannedPage {
        rotation,
        ..PlannedPage::original(index)
    }
}

fn text(doc: DocumentId, page: usize) -> String {
    engine().page_text(doc, page).expect("texte").text()
}

/// Taille de la page rendue à l'échelle 1 (points arrondis).
fn size(doc: DocumentId, page: usize) -> (u32, u32) {
    let bitmap = engine()
        .render(doc, RenderRequest { page, scale: 1.0 })
        .expect("rendu");
    (bitmap.width(), bitmap.height())
}

fn reopen(doc: &OpenedDocument, password: Option<&str>) -> OpenedDocument {
    let bytes = engine().save(doc.id).expect("enregistrement");
    engine()
        .open(bytes, password.map(Into::into))
        .expect("réouverture")
}

#[test]
fn pages_in_reverse_order() {
    let doc = open_simple();
    arrange(
        doc.id,
        vec![PlannedPage::original(1), PlannedPage::original(0)],
        Vec::new(),
    );
    assert!(text(doc.id, 0).contains("Page deux"));
    assert!(text(doc.id, 1).contains("Bonjour PdfEditor"));
    assert_eq!(size(doc.id, 0), (842, 595));
    assert_eq!(size(doc.id, 1), (595, 842));
    // Revenir au plan d'origine rend le document tel qu'ouvert.
    arrange(doc.id, PagePlan::identity(2).pages().to_vec(), Vec::new());
    assert!(text(doc.id, 0).contains("Bonjour PdfEditor"));
    engine().close(doc.id);
}

#[test]
fn blank_pages_copies_and_rotations() {
    let doc = open_simple();
    arrange(
        doc.id,
        vec![
            turned(0, 1),
            PlannedPage::blank(PageSize::new(200.0, 300.0)),
            PlannedPage::original(0),
            // Déjà tournée d'un quart de tour : un de plus la remet en portrait.
            turned(1, 1),
        ],
        Vec::new(),
    );
    assert_eq!(size(doc.id, 0), (842, 595));
    assert_eq!(size(doc.id, 1), (200, 300));
    assert!(text(doc.id, 1).trim().is_empty());
    assert!(text(doc.id, 0).contains("Bonjour PdfEditor"));
    assert!(text(doc.id, 2).contains("Bonjour PdfEditor"));
    assert_eq!(size(doc.id, 3), (595, 842));
    assert!(text(doc.id, 3).contains("Page deux"));

    let saved = reopen(&doc, None);
    assert_eq!(
        saved.info.page_sizes,
        [LANDSCAPE, PageSize::new(200.0, 300.0), A4, A4]
    );
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn removed_page_and_elements_on_the_arranged_pages() {
    let doc = open_simple();
    let note = Element::Text(TextBox {
        page: 0,
        top_left: Point::new(72.0, 500.0),
        width: 300.0,
        text: "Ajout sur la page deux".into(),
        style: TextStyle::default(),
    });
    // Seule la page 2 reste ; l'élément de la page 0 du plan y est posé.
    arrange(
        doc.id,
        vec![PlannedPage::original(1)],
        vec![(ElementId(1), note)],
    );
    let page = text(doc.id, 0);
    assert!(page.contains("Page deux") && page.contains("Ajout sur la page deux"));
    let saved = reopen(&doc, None);
    assert_eq!(saved.info.page_sizes, [LANDSCAPE]);
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn bookmarks_follow_the_pages_that_were_not_copied() {
    let doc = open_simple();
    // La page 1 passe derrière la page 2 : la plus longue suite gardée
    // est une seule page ; l'autre est une copie.
    arrange(
        doc.id,
        vec![PlannedPage::original(1), PlannedPage::original(0)],
        Vec::new(),
    );
    let saved = reopen(&doc, None);
    let introduction = saved
        .info
        .outline
        .iter()
        .find(|item| item.title == "Introduction")
        .expect("signet Introduction");
    // « Introduction » visait la page 1 d'origine, gardée telle quelle.
    assert_eq!(introduction.page, Some(1));
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn protected_document_keeps_its_password_after_a_copy() {
    let doc = engine()
        .open(ENCRYPTED.to_vec(), Some(TEST_PASSWORD.into()))
        .expect("ouverture");
    let count = doc.info.page_count();
    let mut pages = PagePlan::identity(count).pages().to_vec();
    pages.push(PlannedPage::original(0));
    arrange(doc.id, pages, Vec::new());
    let bytes = engine().save(doc.id).expect("enregistrement");
    assert!(
        engine().open(bytes.clone(), None).is_err(),
        "mot de passe perdu"
    );
    let saved = engine()
        .open(bytes, Some(TEST_PASSWORD.into()))
        .expect("réouverture");
    assert_eq!(saved.info.page_count(), count + 1);
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn invalid_plan_leaves_the_document_unchanged() {
    let doc = open_simple();
    let result = engine().apply_edits(
        doc.id,
        PagePlan::new(vec![PlannedPage::original(7)]),
        Vec::new(),
        Images::new(),
    );
    assert!(result.is_err());
    assert!(text(doc.id, 0).contains("Bonjour PdfEditor"));
    engine().close(doc.id);
}
