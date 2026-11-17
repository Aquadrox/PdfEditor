//! Caviardage réel (E-07) : le contenu sous les zones disparaît du fichier,
//! le reste de la page garde son aspect. Nécessite PDFium
//! (`scripts/fetch-pdfium.sh`).

mod common;

use common::{engine, open_simple};
use pde_core::document::DocumentId;
use pde_core::edit::{Area, Color, Element, ElementId, Images, NoteBox, PagePlan, RedactBox};
use pde_core::engine::{Edition, PdfEngine};
use pde_core::geometry::Point;
use pde_core::render::{Bitmap, RenderRequest};

/// simple.pdf, page 1 : « Bonjour PdfEditor » en corps 24 posé en
/// (72, 760) ; rectangle noir de (72, 600) à (272, 700).
fn redact(page: usize, x: f64, top: f64, width: f64, height: f64) -> Element {
    Element::Redact(RedactBox {
        page,
        areas: vec![Area {
            top_left: Point::new(x, top),
            width,
            height,
        }],
    })
}

fn apply(doc: DocumentId, elements: Vec<Element>) {
    let elements = elements
        .into_iter()
        .enumerate()
        .map(|(i, e)| (ElementId(i as u64 + 1), e))
        .collect();
    engine()
        .apply_edits(
            doc,
            Edition::new(PagePlan::identity(2), elements, Images::new()),
        )
        .expect("caviardage appliqué");
}

fn render(doc: DocumentId, page: usize) -> Bitmap {
    engine()
        .render(doc, RenderRequest { page, scale: 1.0 })
        .expect("rendu")
}

/// Pixels sombres dans le rectangle [x0, x1] × [y0, y1] de la page
/// affichée (origine en bas).
fn dark_pixels(bitmap: &Bitmap, (x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> usize {
    (x0..x1)
        .flat_map(|x| (y0..y1).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            bitmap
                .pixel(x, bitmap.height() - y)
                .is_some_and(|p| p[0] < 100 && p[1] < 100 && p[2] < 100)
        })
        .count()
}

fn text(doc: DocumentId, page: usize) -> String {
    engine().page_text(doc, page).expect("texte").text()
}

#[test]
fn redacted_text_leaves_the_file_and_the_rest_of_the_line_stays_visible() {
    let doc = open_simple();
    let before = render(doc.id, 0);
    let visible_before = dark_pixels(&before, (180, 290), (758, 784));
    assert!(visible_before > 50, "« PdfEditor » visible avant");

    // « Bonjour » seulement.
    apply(doc.id, vec![redact(0, 70.0, 790.0, 85.0, 36.0)]);
    let after = render(doc.id, 0);
    // Zone entièrement noire.
    let zone = 75 * 30;
    assert_eq!(dark_pixels(&after, (75, 150), (757, 787)), zone);
    // Le reste de la ligne a gardé son aspect (devenu une image).
    let visible_after = dark_pixels(&after, (180, 290), (758, 784));
    assert!(
        visible_after.abs_diff(visible_before) < visible_before / 5,
        "{visible_after} pixels sombres au lieu d'environ {visible_before}"
    );
    // Rectangle hors zone : intact.
    assert_eq!(dark_pixels(&after, (100, 110), (640, 650)), 100);
    // Le texte touché n'existe plus en tant que texte, ni dans le fichier.
    assert!(!text(doc.id, 0).contains("Bonjour"));
    let saved = engine().save(doc.id).expect("enregistrement");
    let reopened = engine().open(saved, None).expect("réouverture");
    assert!(!text(reopened.id, 0).contains("Bonjour"));
    assert!(
        text(reopened.id, 1).contains("Page deux"),
        "autre page intacte"
    );
    engine().close(reopened.id);
    engine().close(doc.id);
}

#[test]
fn redaction_on_a_rotated_page_follows_the_display() {
    let doc = open_simple();
    // Page 2, tournée d'un quart de tour (842 × 595 affichée) : le haut de
    // la page PDF devient le bord droit, « Page deux » s'y lit de haut en
    // bas, vers x = 760, depuis 72 pt sous le haut.
    let before = render(doc.id, 1);
    let ink_before = dark_pixels(&before, (730, 790), (335, 535));
    assert!(ink_before > 20, "texte visible avant ({ink_before})");
    apply(doc.id, vec![redact(1, 720.0, 545.0, 80.0, 220.0)]);
    assert!(!text(doc.id, 1).contains("Page deux"));
    let after = render(doc.id, 1);
    assert_eq!(dark_pixels(&after, (725, 795), (330, 540)), 70 * 210);
    engine().close(doc.id);
}

#[test]
fn annotations_under_a_zone_are_removed() {
    let doc = open_simple();
    let note = Element::Note(NoteBox {
        page: 0,
        top_left: Point::new(300.0, 400.0),
        text: "Commentaire confidentiel".into(),
        author: "Test".into(),
        color: Color::INK,
    });
    apply(doc.id, vec![note.clone()]);
    let with_note = engine().save(doc.id).expect("enregistrement");
    assert!(contains(&with_note, b"/Annot"));
    apply(doc.id, vec![note, redact(0, 290.0, 410.0, 40.0, 40.0)]);
    let saved = engine().save(doc.id).expect("enregistrement");
    let reopened = engine().open(saved.clone(), None).expect("réouverture");
    assert!(
        !contains(&saved, "Commentaire".as_bytes()),
        "la note caviardée est restée dans le fichier"
    );
    engine().close(reopened.id);
    engine().close(doc.id);
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}
