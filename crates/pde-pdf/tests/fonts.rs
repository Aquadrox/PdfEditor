//! Texte ajouté hors de l'alphabet latin d'Europe de l'Ouest : police Noto
//! embarquée (grec, cyrillique, latin étendu), texte relu à l'identique,
//! police embarquée une seule fois, rien d'embarqué pour du français.

mod common;

use common::{assert_matches_reference, engine, open_simple};
use pde_core::edit::{Element, ElementId, Images, PagePlan, StandardFont, TextBox, TextStyle};
use pde_core::engine::{OpenedDocument, PdfEngine};
use pde_core::geometry::Point;
use pde_core::render::RenderRequest;

fn text(top: f64, content: &str, font: StandardFont, bold: bool) -> Element {
    Element::Text(TextBox {
        page: 0,
        top_left: Point::new(72.0, top),
        width: 450.0,
        text: content.into(),
        style: TextStyle {
            size: 18.0,
            font,
            bold,
            ..TextStyle::default()
        },
    })
}

fn place(doc: &OpenedDocument, elements: Vec<Element>) {
    let elements = elements
        .into_iter()
        .enumerate()
        .map(|(i, e)| (ElementId(i as u64 + 1), e))
        .collect();
    engine()
        .apply_edits(
            doc.id,
            PagePlan::identity(doc.info.page_count()),
            elements,
            Images::new(),
        )
        .expect("texte posé");
}

/// Nombre de polices embarquées (programmes TrueType) dans un PDF.
fn embedded_fonts(bytes: &[u8]) -> usize {
    bytes.windows(9).filter(|w| w == b"FontFile2").count()
}

const GREEK: &str = "Σύμβαση παροχής υπηρεσιών";
const CYRILLIC: &str = "Договор оказания услуг";
const EXTENDED: &str = "Łódź, Čeština, Đà Nẵng";

#[test]
fn greek_and_cyrillic_are_written_and_read_back() {
    let doc = open_simple();
    place(
        &doc,
        vec![
            text(500.0, GREEK, StandardFont::Sans, false),
            text(450.0, CYRILLIC, StandardFont::Serif, false),
            text(400.0, EXTENDED, StandardFont::Sans, false),
        ],
    );
    let page = engine().page_text(doc.id, 0).expect("texte").text();
    for sample in [GREEK, CYRILLIC, EXTENDED] {
        assert!(page.contains(sample), "« {sample} » absent de : {page}");
    }
    // Toujours lisible après enregistrement et réouverture.
    let bytes = engine().save(doc.id).expect("enregistrement");
    engine().close(doc.id);
    let saved = engine().open(bytes, None).expect("réouverture");
    let page = engine().page_text(saved.id, 0).expect("texte").text();
    assert!(page.contains(GREEK) && page.contains(CYRILLIC), "{page}");
    engine().close(saved.id);
}

#[test]
fn each_variant_is_embedded_once_and_only_when_needed() {
    let doc = open_simple();
    place(
        &doc,
        vec![
            text(500.0, GREEK, StandardFont::Sans, false),
            text(450.0, CYRILLIC, StandardFont::Sans, false),
            text(
                400.0,
                "Été à 18 € : œuvre « déjà » vue",
                StandardFont::Sans,
                false,
            ),
        ],
    );
    let bytes = engine().save(doc.id).expect("enregistrement");
    assert_eq!(
        embedded_fonts(&bytes),
        1,
        "Noto Sans une seule fois pour deux zones"
    );

    // Un autre style : une deuxième police.
    place(
        &doc,
        vec![
            text(500.0, GREEK, StandardFont::Sans, false),
            text(450.0, GREEK, StandardFont::Sans, true),
        ],
    );
    assert_eq!(
        embedded_fonts(&engine().save(doc.id).expect("enregistrement")),
        2
    );

    // Français seul : polices standard, rien d'embarqué, fichier léger.
    place(
        &doc,
        vec![text(
            500.0,
            "Annexe B jointe, œuvre à 18 €",
            StandardFont::Sans,
            false,
        )],
    );
    let bytes = engine().save(doc.id).expect("enregistrement");
    assert_eq!(embedded_fonts(&bytes), 0);
    assert!(bytes.len() < 20_000, "{} octets", bytes.len());
    engine().close(doc.id);
}

#[test]
fn embedded_text_starts_at_the_top_of_its_box() {
    let doc = open_simple();
    place(
        &doc,
        vec![text(500.0, "Παράρτημα", StandardFont::Sans, false)],
    );
    let chars = engine().page_text(doc.id, 0).expect("texte").chars;
    let first = chars.iter().find(|c| c.ch == 'Π').expect("Π");
    // Même critère que pour Helvetica (tests/edit.rs) : boîte « large » du
    // caractère, un peu au-dessus des majuscules (réserve des accents).
    let bounds = first.bounds;
    assert!(
        bounds.max().y <= 506.0 && bounds.min().y > 470.0,
        "{bounds:?}"
    );
    assert!(
        (first.bounds.min().x - 72.0).abs() < 3.0,
        "{:?}",
        first.bounds
    );
    engine().close(doc.id);
}

#[test]
fn scripts_match_the_reference_rendering() {
    let doc = open_simple();
    place(
        &doc,
        vec![
            text(520.0, GREEK, StandardFont::Sans, false),
            text(480.0, CYRILLIC, StandardFont::Serif, true),
            text(440.0, EXTENDED, StandardFont::Mono, false),
            text(400.0, "Latin : œuvre à 18 €", StandardFont::Sans, false),
        ],
    );
    let page = engine()
        .render(
            doc.id,
            RenderRequest {
                page: 0,
                scale: 1.0,
            },
        )
        .expect("rendu");
    assert_matches_reference(&page, "scripts.png");
    engine().close(doc.id);
}
