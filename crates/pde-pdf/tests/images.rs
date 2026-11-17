//! Tests de non-régression des images ajoutées (E-02) : transparence,
//! opacité, JPEG inclus sans recompression, rotation, pages tournées,
//! enregistrement, et image de référence du rendu.

mod common;

use std::sync::Arc;

use common::{assert_matches_reference, engine, open_simple};
use image::ExtendedColorType;
use image::codecs::jpeg::JpegEncoder;
use pde_core::document::DocumentId;
use pde_core::edit::{Element, ElementId, ImageAsset, ImageBox, ImageId, Images, PagePlan};
use pde_core::engine::{OpenedDocument, PdfEngine};
use pde_core::geometry::Point;
use pde_core::render::{Bitmap, RenderRequest};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

/// Image en pixels : `color(x, y)`, y = 0 en haut.
fn pixels(width: u32, height: u32, color: impl Fn(u32, u32) -> [u8; 4]) -> Arc<ImageAsset> {
    let rgba = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| color(x, y))
        .collect();
    Arc::new(ImageAsset::rgba(width, height, rgba).expect("pixels RVBA"))
}

fn jpeg_bytes(width: u32, height: u32, color: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let rgb: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| color(x, y))
        .collect();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .encode(&rgb, width, height, ExtendedColorType::Rgb8)
        .expect("encodage JPEG");
    out
}

fn jpeg(width: u32, height: u32, color: [u8; 3]) -> Arc<ImageAsset> {
    let bytes = jpeg_bytes(width, height, |_, _| color);
    Arc::new(ImageAsset::jpeg(width, height, bytes).expect("JPEG"))
}

fn frame(page: usize, x: f64, top: f64, size: (f64, f64), image: u64) -> ImageBox {
    ImageBox {
        page,
        top_left: Point::new(x, top),
        width: size.0,
        height: size.1,
        image: ImageId(image),
        rotation: 0,
        opacity: 1.0,
        keep_ratio: true,
    }
}

fn images(list: Vec<(u64, Arc<ImageAsset>)>) -> Images {
    list.into_iter().map(|(id, a)| (ImageId(id), a)).collect()
}

fn place(doc: &OpenedDocument, frames: Vec<ImageBox>, images: Images) {
    let elements = frames
        .into_iter()
        .enumerate()
        .map(|(i, f)| (ElementId(i as u64 + 1), Element::Image(f)))
        .collect();
    engine()
        .apply_edits(
            doc.id,
            PagePlan::identity(doc.info.page_count()),
            elements,
            images,
        )
        .expect("images posées");
}

fn render(doc: DocumentId, page: usize) -> Bitmap {
    engine()
        .render(doc, RenderRequest { page, scale: 1.0 })
        .expect("rendu")
}

/// Couleur au point (x, y) de la page affichée (origine en bas).
fn color_at(bitmap: &Bitmap, x: u32, y: u32) -> [u8; 3] {
    let p = bitmap
        .pixel(x, bitmap.height() - y)
        .expect("point dans la page");
    [p[0], p[1], p[2]]
}

fn assert_near(actual: [u8; 3], expected: [u8; 3], tolerance: u8) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(a, e)| a.abs_diff(e) <= tolerance);
    assert!(close, "{actual:?} au lieu de {expected:?} (± {tolerance})");
}

#[test]
fn transparent_pixels_show_the_page_below() {
    let doc = open_simple();
    // Moitié gauche rouge, moitié droite transparente.
    let asset = pixels(20, 10, |x, _| if x < 10 { RED } else { CLEAR });
    place(
        &doc,
        vec![frame(0, 100.0, 500.0, (200.0, 100.0), 1)],
        images(vec![(1, asset)]),
    );
    let page = render(doc.id, 0);
    assert_near(color_at(&page, 150, 450), [255, 0, 0], 8);
    assert_near(color_at(&page, 250, 450), WHITE, 8);
    engine().close(doc.id);
}

#[test]
fn opacity_lets_the_page_show_through() {
    let doc = open_simple();
    let mut half = frame(0, 100.0, 500.0, (100.0, 100.0), 1);
    half.opacity = 0.5;
    let mut jpeg_half = frame(0, 300.0, 500.0, (100.0, 100.0), 2);
    jpeg_half.opacity = 0.5;
    place(
        &doc,
        vec![half, jpeg_half],
        images(vec![
            (1, pixels(4, 4, |_, _| RED)),
            (2, jpeg(16, 16, [0, 0, 255])),
        ]),
    );
    let page = render(doc.id, 0);
    assert_near(color_at(&page, 150, 450), [255, 128, 128], 12);
    assert_near(color_at(&page, 350, 450), [128, 128, 255], 24);
    engine().close(doc.id);
}

#[test]
fn jpeg_is_drawn_and_kept_without_recompression() {
    let doc = open_simple();
    let original = engine().save(doc.id).unwrap().len();
    // Contenu varié : un JPEG qui se compresserait mal en pixels.
    let bytes = jpeg_bytes(400, 400, |x, y| {
        [
            (x * 7 % 256) as u8,
            (y * 13 % 256) as u8,
            ((x ^ y) % 256) as u8,
        ]
    });
    let size = bytes.len();
    let asset = Arc::new(ImageAsset::jpeg(400, 400, bytes).unwrap());
    let blue = jpeg(16, 16, [0, 0, 255]);
    place(
        &doc,
        vec![
            frame(0, 72.0, 400.0, (200.0, 200.0), 1),
            frame(0, 350.0, 400.0, (100.0, 100.0), 2),
        ],
        images(vec![(1, asset), (2, blue)]),
    );
    assert_near(color_at(&render(doc.id, 0), 400, 350), [0, 0, 255], 24);
    let saved = engine().save(doc.id).unwrap().len();
    // Le fichier grandit de la taille du JPEG, pas de celle des pixels
    // (400 × 400 × 3 = 480 Ko).
    assert!(
        saved - original < size + 20_000,
        "{original} → {saved} ({size} de JPEG)"
    );
    engine().close(doc.id);
}

#[test]
fn rotation_turns_the_image_inside_its_frame() {
    let doc = open_simple();
    // Haut rouge, bas bleu ; un quart de tour amène le haut à droite.
    let asset = pixels(10, 20, |_, y| if y < 10 { RED } else { BLUE });
    let mut turned = frame(0, 100.0, 500.0, (200.0, 100.0), 1);
    turned.rotation = 1;
    let mut upside_down = frame(0, 350.0, 500.0, (100.0, 200.0), 1);
    upside_down.rotation = 2;
    place(&doc, vec![turned, upside_down], images(vec![(1, asset)]));
    let page = render(doc.id, 0);
    assert_near(color_at(&page, 250, 450), [255, 0, 0], 8);
    assert_near(color_at(&page, 150, 450), [0, 0, 255], 8);
    // Retournée : le rouge en bas.
    assert_near(color_at(&page, 400, 350), [255, 0, 0], 8);
    assert_near(color_at(&page, 400, 450), [0, 0, 255], 8);
    engine().close(doc.id);
}

#[test]
fn image_stays_upright_on_a_rotated_page() {
    let doc = open_simple();
    // Page 2 affichée en 842 × 595 : haut rouge, bas bleu, à l'écran aussi.
    let asset = pixels(10, 10, |_, y| if y < 5 { RED } else { BLUE });
    place(
        &doc,
        vec![frame(1, 50.0, 560.0, (100.0, 100.0), 1)],
        images(vec![(1, asset)]),
    );
    let page = render(doc.id, 1);
    assert_eq!((page.width(), page.height()), (842, 595));
    assert_near(color_at(&page, 100, 540), [255, 0, 0], 8);
    assert_near(color_at(&page, 100, 480), [0, 0, 255], 8);
    engine().close(doc.id);
}

#[test]
fn transparency_and_opacity_survive_saving() {
    let doc = open_simple();
    let asset = pixels(20, 10, |x, _| if x < 10 { RED } else { CLEAR });
    let mut jpeg_half = frame(0, 350.0, 500.0, (100.0, 100.0), 2);
    jpeg_half.opacity = 0.5;
    place(
        &doc,
        vec![frame(0, 100.0, 500.0, (200.0, 100.0), 1), jpeg_half],
        images(vec![(1, asset), (2, jpeg(16, 16, [0, 0, 255]))]),
    );
    let bytes = engine().save(doc.id).unwrap();
    engine().close(doc.id);
    let saved = engine().open(bytes, None).unwrap();
    let page = render(saved.id, 0);
    assert_near(color_at(&page, 150, 450), [255, 0, 0], 8);
    assert_near(color_at(&page, 250, 450), WHITE, 8);
    assert_near(color_at(&page, 400, 450), [128, 128, 255], 24);
    engine().close(saved.id);
}

#[test]
fn missing_image_is_refused_and_display_unchanged() {
    let doc = open_simple();
    place(
        &doc,
        vec![frame(0, 100.0, 500.0, (100.0, 100.0), 1)],
        images(vec![(1, pixels(2, 2, |_, _| RED))]),
    );
    let elements = vec![(
        ElementId(1),
        Element::Image(frame(0, 100.0, 500.0, (100.0, 100.0), 9)),
    )];
    assert!(
        engine()
            .apply_edits(
                doc.id,
                PagePlan::identity(doc.info.page_count()),
                elements,
                Images::new()
            )
            .is_err()
    );
    assert_near(color_at(&render(doc.id, 0), 150, 450), [255, 0, 0], 8);
    engine().close(doc.id);
}

#[test]
fn images_match_the_reference_rendering() {
    let doc = open_simple();
    let checker = pixels(8, 8, |x, y| if (x + y) % 2 == 0 { RED } else { CLEAR });
    let mut faded = frame(0, 320.0, 520.0, (160.0, 90.0), 2);
    faded.opacity = 0.6;
    faded.rotation = 3;
    place(
        &doc,
        vec![frame(0, 72.0, 520.0, (200.0, 200.0), 1), faded],
        images(vec![(1, checker), (2, jpeg(32, 18, [20, 110, 100]))]),
    );
    assert_matches_reference(&render(doc.id, 0), "images.png");
    engine().close(doc.id);
}
