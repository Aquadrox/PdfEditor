//! Images d'origine (E-06) : listées, déplacées, redimensionnées,
//! supprimées. Nécessite PDFium (`scripts/fetch-pdfium.sh`).

mod common;

use std::sync::Arc;

use common::{engine, open_simple};
use pde_core::document::DocumentId;
use pde_core::edit::{
    Area, Element, ElementId, ExistingImage, ImageAsset, ImageBox, ImageId, Images, PagePlan,
};
use pde_core::engine::{Edition, OpenedDocument, PageImage, PdfEngine};
use pde_core::geometry::Point;
use pde_core::render::{Bitmap, RenderRequest};

const RED: [u8; 4] = [220, 30, 30, 255];

fn plan() -> PagePlan {
    PagePlan::identity(2)
}

/// simple.pdf avec une image rouge de 100 × 50 pt en (300, 400) (coin haut
/// gauche) sur la page 1, enregistré puis rouvert : elle fait désormais
/// partie du document d'origine.
fn document_with_an_image() -> OpenedDocument {
    let doc = open_simple();
    let mut images = Images::new();
    let rgba = RED.repeat(20 * 10);
    images.insert(
        ImageId(1),
        Arc::new(ImageAsset::rgba(20, 10, rgba).expect("pixels")),
    );
    let frame = Element::Image(ImageBox {
        page: 0,
        top_left: Point::new(300.0, 400.0),
        width: 100.0,
        height: 50.0,
        image: ImageId(1),
        rotation: 0,
        opacity: 1.0,
        keep_ratio: true,
    });
    engine()
        .apply_edits(
            doc.id,
            Edition::new(plan(), vec![(ElementId(1), frame)], images),
        )
        .expect("image posée");
    let bytes = engine().save(doc.id).expect("enregistrement");
    engine().close(doc.id);
    engine().open(bytes, None).expect("réouverture")
}

fn images_of(doc: DocumentId) -> Vec<PageImage> {
    engine()
        .page_images(doc, Edition::new(plan(), Vec::new(), Images::new()), 0)
        .expect("images de la page")
}

fn area(x: f64, top: f64, width: f64, height: f64) -> Area {
    Area {
        top_left: Point::new(x, top),
        width,
        height,
    }
}

fn edit(doc: DocumentId, existing: &PageImage, frame: Option<Area>) -> Result<(), String> {
    let b = existing.bounds;
    let element = Element::ExistingImage(ExistingImage {
        page: 0,
        object: existing.object,
        original: area(b.min().x, b.max().y, b.width(), b.height()),
        frame,
    });
    engine()
        .apply_edits(
            doc,
            Edition::new(plan(), vec![(ElementId(9), element)], Images::new()),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn render(doc: DocumentId) -> Bitmap {
    engine()
        .render(
            doc,
            RenderRequest {
                page: 0,
                scale: 1.0,
            },
        )
        .expect("rendu")
}

/// Le point (x, y) de la page affichée (origine en bas) est rouge.
fn red_at(bitmap: &Bitmap, x: u32, y: u32) -> bool {
    bitmap
        .pixel(x, bitmap.height() - y)
        .is_some_and(|p| p[0] > 180 && p[1] < 90 && p[2] < 90)
}

#[test]
fn original_images_are_listed_with_their_place() {
    let doc = document_with_an_image();
    let images = images_of(doc.id);
    assert_eq!(images.len(), 1);
    let b = images[0].bounds;
    assert!(
        (b.min().x - 300.0).abs() < 1.0 && (b.max().y - 400.0).abs() < 1.0,
        "{b:?}"
    );
    assert!(
        (b.width() - 100.0).abs() < 1.0 && (b.height() - 50.0).abs() < 1.0,
        "{b:?}"
    );
    engine().close(doc.id);
}

#[test]
fn an_original_image_moves_and_resizes() {
    let doc = document_with_an_image();
    let image = images_of(doc.id).remove(0);
    assert!(red_at(&render(doc.id), 350, 375));
    edit(doc.id, &image, Some(area(100.0, 300.0, 50.0, 100.0))).expect("déplacée");
    let after = render(doc.id);
    assert!(!red_at(&after, 350, 375), "plus à l'ancienne place");
    assert!(red_at(&after, 125, 250) && red_at(&after, 105, 205) && red_at(&after, 145, 295));
    assert!(!red_at(&after, 160, 250), "largeur de 50 pt");
    // Toujours une seule image, et elle est enregistrée à sa nouvelle place.
    let saved = engine().save(doc.id).expect("enregistrement");
    let reopened = engine().open(saved, None).expect("réouverture");
    let moved = images_of(reopened.id);
    assert_eq!(moved.len(), 1);
    assert!((moved[0].bounds.min().x - 100.0).abs() < 1.0);
    engine().close(reopened.id);
    engine().close(doc.id);
}

#[test]
fn an_original_image_is_removed_and_the_rest_of_the_page_stays() {
    let doc = document_with_an_image();
    let image = images_of(doc.id).remove(0);
    edit(doc.id, &image, None).expect("supprimée");
    assert!(!red_at(&render(doc.id), 350, 375));
    assert!(
        engine()
            .page_text(doc.id, 0)
            .expect("texte")
            .text()
            .contains("Bonjour")
    );
    let saved = engine().save(doc.id).expect("enregistrement");
    let reopened = engine().open(saved, None).expect("réouverture");
    assert!(images_of(reopened.id).is_empty());
    engine().close(reopened.id);
    engine().close(doc.id);
}

#[test]
fn an_image_that_is_not_where_expected_is_left_alone() {
    let doc = document_with_an_image();
    let mut image = images_of(doc.id).remove(0);
    // Mauvais cadre attendu : refusé, l'affichage ne change pas.
    image.bounds = image.bounds.translate(40.0, 0.0);
    assert!(
        edit(doc.id, &image, None)
            .unwrap_err()
            .contains("plus à sa place")
    );
    // Rang qui n'est pas une image : refusé aussi.
    let mut text = images_of(doc.id).remove(0);
    text.object = 0;
    assert!(edit(doc.id, &text, None).is_err());
    assert!(red_at(&render(doc.id), 350, 375));
    engine().close(doc.id);
}
