//! Texte d'origine (E-05) : blocs d'une page, texte récrit avec sa police
//! ou une police de remplacement, retiré, refusé s'il n'est plus à sa
//! place. Nécessite PDFium (`scripts/fetch-pdfium.sh`).

mod common;

use common::{engine, open_simple};
use pde_core::document::DocumentId;
use pde_core::edit::{
    Area, Element, ElementId, Images, OriginalText, PagePlan, StandardFont, TextBox, TextStyle,
};
use pde_core::engine::{Edition, OpenedDocument, PageTextBlock, PdfEngine, Placed};
use pde_core::geometry::Point;

fn plan() -> PagePlan {
    PagePlan::identity(2)
}

fn blocks(doc: DocumentId, page: usize) -> Vec<PageTextBlock> {
    engine()
        .page_text_blocks(doc, Edition::new(plan(), Vec::new(), Images::new()), page)
        .expect("blocs de la page")
}

fn block_with(doc: DocumentId, text: &str) -> PageTextBlock {
    blocks(doc, 0)
        .into_iter()
        .find(|b| b.text == text)
        .unwrap_or_else(|| panic!("bloc « {text} » absent"))
}

fn rewritten(block: &PageTextBlock, text: &str) -> Element {
    Element::OriginalText(OriginalText {
        page: 0,
        objects: block.objects.clone(),
        original: Area {
            top_left: Point::new(block.bounds.min().x, block.bounds.max().y),
            width: block.bounds.width(),
            height: block.bounds.height(),
        },
        before: block.text.clone(),
        text: text.into(),
        style: block.style.clone(),
    })
}

fn apply(doc: DocumentId, elements: Vec<Element>) -> Result<Vec<Placed>, String> {
    let elements = elements
        .into_iter()
        .enumerate()
        .map(|(i, e)| (ElementId(i as u64 + 1), e))
        .collect();
    engine()
        .apply_edits(doc, Edition::new(plan(), elements, Images::new()))
        .map_err(|e| e.to_string())
}

fn text(doc: DocumentId) -> String {
    engine().page_text(doc, 0).expect("texte").text()
}

#[test]
fn blocks_of_a_page_with_their_look() {
    let doc = open_simple();
    let block = block_with(doc.id, "Bonjour PdfEditor");
    assert_eq!(block.style.size, 24.0);
    assert_eq!(block.style.font, StandardFont::Sans);
    assert!(block.font_name.contains("Helvetica"), "{}", block.font_name);
    // Ligne de base à 760 : le bloc l'entoure.
    assert!(block.bounds.min().y < 760.0 && block.bounds.max().y > 760.0);
    // Page tournée : aucun bloc proposé pour l'instant.
    assert!(blocks(doc.id, 1).is_empty());
    engine().close(doc.id);
}

#[test]
fn a_standard_font_rewrites_the_block_in_place() {
    let doc = open_simple();
    let block = block_with(doc.id, "Bonjour PdfEditor");
    let placed = apply(doc.id, vec![rewritten(&block, "Bonsoir l'été")]).expect("récrit");
    assert!(!placed[0].replaced_font);
    // Même départ de ligne.
    assert!((placed[0].bounds.min().x - block.bounds.min().x).abs() < 1.0);
    let page = text(doc.id);
    assert!(
        page.contains("Bonsoir l'été") && !page.contains("Bonjour"),
        "{page}"
    );
    // Enregistré, rouvert : le nouveau texte fait partie du document.
    let bytes = engine().save(doc.id).expect("enregistrement");
    let saved = engine().open(bytes, None).expect("réouverture");
    assert!(text(saved.id).contains("Bonsoir l'été"));
    assert!(
        blocks(saved.id, 0)
            .iter()
            .any(|b| b.text == "Bonsoir l'été")
    );
    engine().close(saved.id);
    // Retour à l'original : le document tel qu'ouvert.
    apply(doc.id, Vec::new()).expect("original");
    assert!(text(doc.id).contains("Bonjour PdfEditor"));
    engine().close(doc.id);
}

#[test]
fn characters_outside_the_font_use_a_replacement_font() {
    let doc = open_simple();
    let block = block_with(doc.id, "Bonjour PdfEditor");
    let placed = apply(doc.id, vec![rewritten(&block, "Привет PdfEditor")]).expect("récrit");
    assert!(placed[0].replaced_font);
    let page = text(doc.id);
    assert!(
        page.contains("Привет") && !page.contains("Bonjour"),
        "{page}"
    );
    engine().close(doc.id);
}

#[test]
fn an_empty_text_removes_the_block() {
    let doc = open_simple();
    let block = block_with(doc.id, "Bonjour PdfEditor");
    let placed = apply(doc.id, vec![rewritten(&block, " ")]).expect("retiré");
    assert!(!placed[0].replaced_font);
    assert!(!text(doc.id).contains("Bonjour"));
    engine().close(doc.id);
}

#[test]
fn a_block_that_moved_is_refused_and_the_display_is_kept() {
    let doc = open_simple();
    let block = block_with(doc.id, "Bonjour PdfEditor");
    let Element::OriginalText(mut moved) = rewritten(&block, "Autre") else {
        unreachable!()
    };
    moved.original.top_left.y -= 50.0;
    let error = apply(doc.id, vec![Element::OriginalText(moved)]).unwrap_err();
    assert!(error.contains("plus à sa place"), "{error}");
    assert!(text(doc.id).contains("Bonjour PdfEditor"));
    engine().close(doc.id);
}

/// simple.pdf avec une ligne en cyrillique (police Noto embarquée),
/// enregistré puis rouvert : elle fait partie du document d'origine.
fn document_with_an_embedded_font() -> OpenedDocument {
    let doc = open_simple();
    let line = Element::Text(TextBox {
        page: 0,
        top_left: Point::new(72.0, 500.0),
        width: 400.0,
        text: "Привет мир".into(),
        style: TextStyle {
            size: 16.0,
            ..TextStyle::default()
        },
    });
    apply(doc.id, vec![line]).expect("ligne posée");
    let bytes = engine().save(doc.id).expect("enregistrement");
    engine().close(doc.id);
    engine().open(bytes, None).expect("réouverture")
}

#[test]
fn an_embedded_font_is_kept_only_for_the_characters_it_already_wrote() {
    let doc = document_with_an_embedded_font();
    let block = block_with(doc.id, "Привет мир");
    let kept = apply(doc.id, vec![rewritten(&block, "мир Привет")]).expect("récrit");
    assert!(!kept[0].replaced_font);
    assert!(text(doc.id).contains("мир Привет"));
    // « Ж » n'a jamais été écrit avec cette police : remplacée.
    let replaced = apply(doc.id, vec![rewritten(&block, "Жми")]).expect("récrit");
    assert!(replaced[0].replaced_font);
    let page = text(doc.id);
    assert!(page.contains("Жми") && !page.contains("Привет"), "{page}");
    engine().close(doc.id);
}
