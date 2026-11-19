//! Parcours complet (E-12, E-13) sur le vrai moteur : un document Word
//! repris par l'import intégré, ses champs détectés sur la page, créés,
//! enregistrés puis relus. Nécessite PDFium (`scripts/fetch-pdfium.sh`).

mod common;

use common::engine;
use pde_core::blank::blank_pdf;
use pde_core::edit::{
    Element, ElementId, FieldBox, FieldKind, FieldLook, PagePlan, detect_fields, name_for_label,
};
use pde_core::engine::{Edition, PdfEngine};
use pde_core::geometry::Point;

const DOCX: &[u8] = include_bytes!("../../pde-docx/tests/fixtures/formulaire.docx");

fn field_from(s: &pde_core::edit::Suggestion, n: usize) -> FieldBox {
    FieldBox {
        page: 0,
        top_left: Point::new(s.rect.min().x, s.rect.max().y),
        width: s.rect.width(),
        height: s.rect.height(),
        name: format!("{}_{n}", name_for_label(&s.label)),
        kind: s.kind.clone(),
        tooltip: s.label.clone(),
        required: false,
        read_only: false,
        look: FieldLook::default(),
        value: None,
        export: if matches!(s.kind, FieldKind::Checkbox) {
            "Oui".into()
        } else {
            String::new()
        },
        origin: None,
        deleted: false,
    }
}

#[test]
fn a_word_form_is_imported_detected_and_saved_with_its_fields() {
    let doc = pde_docx::read(DOCX).expect("docx");
    let laid = pde_docx::layout(&doc);
    let blank = blank_pdf(laid.page_size, laid.pages as u32);
    let opened = engine().open(blank, None).expect("PDF vierge");
    assert_eq!(opened.info.page_count(), laid.pages);
    let elements: Vec<(ElementId, Element)> = laid
        .elements
        .into_iter()
        .enumerate()
        .map(|(i, e)| (ElementId(i as u64 + 1), e))
        .collect();
    engine()
        .apply_edits(
            opened.id,
            Edition::new(PagePlan::identity(laid.pages), elements, Default::default()),
        )
        .expect("texte posé");
    let imported = engine().save(opened.id).expect("PDF produit");
    engine().close(opened.id);

    let doc = engine().open(imported, None).expect("PDF importé");
    let text = engine().page_text(doc.id, 0).expect("texte");
    let plain = text.text();
    assert!(plain.contains("Demande d'inscription"), "{plain}");
    let width = doc.info.page_sizes[0].width;
    let found = detect_fields(&text, width);
    let labels: Vec<&str> = found.iter().map(|s| s.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.contains("Nom")), "{labels:?}");
    assert!(
        found.iter().any(|s| s
            .kind
            .format()
            .is_some_and(|f| matches!(f, pde_core::formats::FieldFormat::Date { .. }))),
        "{found:?}"
    );
    let boxes: Vec<(ElementId, Element)> = found
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                ElementId(i as u64 + 1),
                Element::Field(Box::new(field_from(s, i))),
            )
        })
        .collect();
    let count = boxes.len();
    engine()
        .apply_edits(
            doc.id,
            Edition::new(
                PagePlan::identity(doc.info.page_count()),
                boxes,
                Default::default(),
            ),
        )
        .expect("champs créés");
    let saved = engine()
        .open(engine().save(doc.id).expect("enregistré"), None)
        .expect("relu");
    let fields = engine()
        .form_fields(
            saved.id,
            Edition::new(
                PagePlan::identity(saved.info.page_count()),
                Vec::new(),
                Default::default(),
            ),
        )
        .expect("champs");
    assert_eq!(fields.len(), count, "{fields:#?}");
    engine().close(saved.id);
    engine().close(doc.id);
}
