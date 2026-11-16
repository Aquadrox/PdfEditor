//! Tests de non-régression de l'écriture : éléments posés sur le document,
//! relus, enregistrés, et image de référence du rendu.

mod common;

use common::{SIMPLE, engine, open_simple};
use pde_core::edit::{Align, Color, Element, ElementId, Images, TextBox, TextStyle};
use pde_core::engine::PdfEngine;
use pde_core::geometry::Point;
use pde_core::render::RenderRequest;

fn text_box(page: usize, x: f64, top: f64, width: f64, text: &str) -> Element {
    Element::Text(TextBox {
        page,
        top_left: Point::new(x, top),
        width,
        text: text.into(),
        style: TextStyle {
            size: 20.0,
            ..TextStyle::default()
        },
    })
}

#[test]
fn added_text_is_drawn_and_readable() {
    let doc = open_simple();
    let placed = engine()
        .apply_edits(
            doc.id,
            vec![(
                ElementId(1),
                text_box(0, 100.0, 500.0, 300.0, "Annexe B jointe"),
            )],
            Images::new(),
        )
        .unwrap();
    // Une ligne : hauteur = corps × interligne (20 × 1,25).
    assert_eq!(placed[0].id, ElementId(1));
    assert_eq!(placed[0].bounds.min(), Point::new(100.0, 475.0));
    assert_eq!(placed[0].bounds.max(), Point::new(400.0, 500.0));
    let text = engine().page_text(doc.id, 0).unwrap();
    assert!(text.text().contains("Annexe B jointe"), "{}", text.text());
    let a = text.chars.iter().find(|c| c.ch == 'A').unwrap();
    // Boîte « large » du caractère : un peu au-dessus des majuscules.
    assert!((a.bounds.min().x - 100.0).abs() < 3.0, "{:?}", a.bounds);
    assert!(
        a.bounds.max().y <= 506.0 && a.bounds.min().y > 470.0,
        "{:?}",
        a.bounds
    );
    engine().close(doc.id);
}

#[test]
fn long_text_wraps_inside_its_box() {
    let doc = open_simple();
    let words = "un deux trois quatre cinq six sept huit neuf dix";
    let placed = engine()
        .apply_edits(
            doc.id,
            vec![(ElementId(1), text_box(0, 72.0, 400.0, 120.0, words))],
            Images::new(),
        )
        .unwrap();
    let lines = (placed[0].bounds.height() / 25.0).round() as usize;
    assert!(lines >= 3, "{lines} ligne(s)");
    // Aucun caractère ne dépasse la largeur de la zone.
    let text = engine().page_text(doc.id, 0).unwrap();
    for c in text
        .chars
        .iter()
        .filter(|c| !c.bounds.is_empty() && c.bounds.max().y < 401.0)
    {
        assert!(c.bounds.max().x <= 72.0 + 120.0 + 1.0, "{c:?}");
    }
    engine().close(doc.id);
}

#[test]
fn text_stays_upright_on_a_rotated_page() {
    let doc = open_simple();
    // Page 2 affichée en 842 × 595 : texte en haut à gauche de l'écran.
    engine()
        .apply_edits(
            doc.id,
            vec![(ElementId(1), text_box(1, 50.0, 560.0, 400.0, "Droit"))],
            Images::new(),
        )
        .unwrap();
    let text = engine().page_text(doc.id, 1).unwrap();
    let letters: Vec<_> = text
        .chars
        .iter()
        .filter(|c| "Droit".contains(c.ch))
        .collect();
    assert_eq!(letters.len(), 5, "{}", text.text());
    for pair in letters.windows(2) {
        // Lecture de gauche à droite, sur la même ligne, à l'écran.
        assert!(pair[1].bounds.min().x > pair[0].bounds.min().x, "{pair:?}");
        assert!(
            (pair[1].bounds.min().y - pair[0].bounds.min().y).abs() < 3.0,
            "{pair:?}"
        );
    }
    assert!(
        (letters[0].bounds.min().x - 50.0).abs() < 3.0,
        "{:?}",
        letters[0].bounds
    );
    engine().close(doc.id);
}

#[test]
fn accents_euro_and_ligatures_survive() {
    let doc = open_simple();
    let sample = "Été à 18 € : œuvre « déjà » vue";
    engine()
        .apply_edits(
            doc.id,
            vec![(ElementId(1), text_box(0, 72.0, 300.0, 450.0, sample))],
            Images::new(),
        )
        .unwrap();
    let text = engine().page_text(doc.id, 0).unwrap().text();
    assert!(text.contains(sample), "{text}");
    engine().close(doc.id);
}

#[test]
fn removing_every_element_returns_to_the_original() {
    let doc = open_simple();
    engine()
        .apply_edits(
            doc.id,
            vec![(ElementId(1), text_box(0, 72.0, 300.0, 300.0, "Temporaire"))],
            Images::new(),
        )
        .unwrap();
    engine()
        .apply_edits(doc.id, Vec::new(), Images::new())
        .unwrap();
    let text = engine().page_text(doc.id, 0).unwrap().text();
    assert!(!text.contains("Temporaire"));
    assert!(text.contains("Bonjour PdfEditor"));
    engine().close(doc.id);
}

#[test]
fn saved_document_reopens_with_the_added_text() {
    let doc = open_simple();
    engine()
        .apply_edits(
            doc.id,
            vec![(
                ElementId(1),
                text_box(0, 72.0, 300.0, 300.0, "Signé à Lyon"),
            )],
            Images::new(),
        )
        .unwrap();
    let bytes = engine().save(doc.id).unwrap();
    engine().close(doc.id);
    assert!(bytes.starts_with(b"%PDF-"));
    let reopened = engine().open(bytes, None).unwrap();
    assert_eq!(reopened.info.page_sizes.len(), 2);
    assert!(
        engine()
            .page_text(reopened.id, 0)
            .unwrap()
            .text()
            .contains("Signé à Lyon")
    );
    engine().close(reopened.id);
    // L'original n'a pas changé.
    let original = engine().open(SIMPLE.to_vec(), None).unwrap();
    assert!(
        !engine()
            .page_text(original.id, 0)
            .unwrap()
            .text()
            .contains("Signé")
    );
    engine().close(original.id);
}

/// Image de référence : la page 1 avec trois zones (gauche, centrée, à
/// droite, en couleur). `PDE_UPDATE_EXPECTED=1 cargo test` la régénère après
/// un changement voulu ; sinon, le rendu doit rester (presque) identique.
#[test]
fn rendering_matches_the_reference_image() {
    let doc = open_simple();
    let styled = |align, color, text: &str, top| {
        Element::Text(TextBox {
            page: 0,
            top_left: Point::new(72.0, top),
            width: 450.0,
            text: text.into(),
            style: TextStyle {
                size: 18.0,
                align,
                color,
                bold: align == Align::Center,
                ..TextStyle::default()
            },
        })
    };
    engine()
        .apply_edits(
            doc.id,
            vec![
                (
                    ElementId(1),
                    styled(Align::Left, Color::INK, "À gauche", 520.0),
                ),
                (
                    ElementId(2),
                    styled(
                        Align::Center,
                        Color {
                            r: 0x27,
                            g: 0x47,
                            b: 0xb8,
                        },
                        "Au centre",
                        480.0,
                    ),
                ),
                (
                    ElementId(3),
                    styled(
                        Align::Right,
                        Color {
                            r: 0x8a,
                            g: 0x1c,
                            b: 0x2b,
                        },
                        "À droite",
                        440.0,
                    ),
                ),
            ],
            Images::new(),
        )
        .unwrap();
    let bitmap = engine()
        .render(
            doc.id,
            RenderRequest {
                page: 0,
                scale: 1.0,
            },
        )
        .unwrap();
    engine().close(doc.id);
    common::assert_matches_reference(&bitmap, "text-boxes.png");
}

/// Un document protégé le reste une fois enregistré : même mot de passe,
/// éléments ajoutés compris.
#[test]
fn saved_protected_document_keeps_its_password() {
    let doc = engine()
        .open(
            common::ENCRYPTED.to_vec(),
            Some(common::TEST_PASSWORD.into()),
        )
        .unwrap();
    engine()
        .apply_edits(
            doc.id,
            vec![(
                ElementId(1),
                text_box(0, 72.0, 300.0, 300.0, "Confidentiel"),
            )],
            Images::new(),
        )
        .unwrap();
    let bytes = engine().save(doc.id).unwrap();
    engine().close(doc.id);
    assert_eq!(
        engine().open(bytes.clone(), None).unwrap_err(),
        pde_core::engine::EngineError::PasswordRequired
    );
    let reopened = engine()
        .open(bytes, Some(common::TEST_PASSWORD.into()))
        .unwrap();
    assert!(
        engine()
            .page_text(reopened.id, 0)
            .unwrap()
            .text()
            .contains("Confidentiel")
    );
    engine().close(reopened.id);
}
