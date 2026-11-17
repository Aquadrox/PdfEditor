//! Tests de non-régression des annotations dessinées (E-04) : formes,
//! dessin à main levée, surlignage / soulignement / barré, page tournée,
//! enregistrement, et image de référence du rendu.

mod common;

use common::{assert_matches_reference, engine, open_simple};
use pde_core::document::DocumentId;
use pde_core::edit::{
    Area, Color, Element, ElementId, Images, InkBox, MarkKind, MarkupBox, ShapeBox, ShapeKind,
};
use pde_core::engine::PdfEngine;
use pde_core::geometry::Point;
use pde_core::render::{Bitmap, RenderRequest};

const RED: Color = Color {
    r: 0xc6,
    g: 0x28,
    b: 0x28,
};
const YELLOW: Color = Color {
    r: 0xff,
    g: 0xd5,
    b: 0x4a,
};
const WHITE: [u8; 3] = [255, 255, 255];

fn shape(page: usize, kind: ShapeKind, start: (f64, f64), end: (f64, f64)) -> ShapeBox {
    ShapeBox {
        page,
        shape: kind,
        start: Point::new(start.0, start.1),
        end: Point::new(end.0, end.1),
        color: RED,
        width: 4.0,
        filled: false,
    }
}

fn place(doc: DocumentId, elements: Vec<Element>) {
    let elements = elements
        .into_iter()
        .enumerate()
        .map(|(i, e)| (ElementId(i as u64 + 1), e))
        .collect();
    engine()
        .apply_edits(doc, elements, Images::new())
        .expect("annotations posées");
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

const RED_RGB: [u8; 3] = [0xc6, 0x28, 0x28];

#[test]
fn rectangle_outline_and_fill() {
    let doc = open_simple();
    let mut filled = shape(0, ShapeKind::Rectangle, (300.0, 400.0), (400.0, 500.0));
    filled.filled = true;
    place(
        doc.id,
        vec![
            Element::Shape(shape(
                0,
                ShapeKind::Rectangle,
                (100.0, 400.0),
                (200.0, 500.0),
            )),
            Element::Shape(filled),
        ],
    );
    let page = render(doc.id, 0);
    assert_near(color_at(&page, 100, 450), RED_RGB, 12);
    assert_near(color_at(&page, 150, 450), WHITE, 4);
    // Intérieur teinté à 25 %.
    assert_near(color_at(&page, 350, 450), [241, 201, 201], 10);
    engine().close(doc.id);
}

#[test]
fn ellipse_follows_its_curve() {
    let doc = open_simple();
    place(
        doc.id,
        vec![Element::Shape(shape(
            0,
            ShapeKind::Ellipse,
            (100.0, 300.0),
            (300.0, 400.0),
        ))],
    );
    let page = render(doc.id, 0);
    assert_near(color_at(&page, 300, 350), RED_RGB, 20);
    assert_near(color_at(&page, 200, 400), RED_RGB, 20);
    assert_near(color_at(&page, 200, 350), WHITE, 4);
    // Coin du cadre : hors de l'ellipse.
    assert_near(color_at(&page, 103, 397), WHITE, 4);
    engine().close(doc.id);
}

#[test]
fn arrow_has_a_filled_head_at_its_end() {
    let doc = open_simple();
    place(
        doc.id,
        vec![Element::Shape(shape(
            0,
            ShapeKind::Arrow,
            (100.0, 300.0),
            (300.0, 300.0),
        ))],
    );
    let page = render(doc.id, 0);
    assert_near(color_at(&page, 150, 300), RED_RGB, 12);
    // Pointe de 16 pt (4 × 4) : pleine près de la base, au-dessus du trait.
    assert_near(color_at(&page, 287, 304), RED_RGB, 24);
    assert_near(color_at(&page, 150, 306), WHITE, 4);
    engine().close(doc.id);
}

#[test]
fn ink_stroke_passes_through_its_points() {
    let doc = open_simple();
    let points = [(100.0, 300.0), (150.0, 350.0), (200.0, 300.0)]
        .map(|(x, y)| Point::new(x, y))
        .to_vec();
    let dot = InkBox {
        page: 0,
        points: vec![Point::new(400.0, 300.0)],
        color: RED,
        width: 10.0,
    };
    place(
        doc.id,
        vec![
            Element::Ink(InkBox {
                page: 0,
                points,
                color: RED,
                width: 4.0,
            }),
            Element::Ink(dot),
        ],
    );
    let page = render(doc.id, 0);
    assert_near(color_at(&page, 125, 325), RED_RGB, 20);
    assert_near(color_at(&page, 150, 349), RED_RGB, 20);
    assert_near(color_at(&page, 150, 310), WHITE, 4);
    // Un clic seul : un point de l'épaisseur du trait.
    assert_near(color_at(&page, 400, 300), RED_RGB, 12);
    engine().close(doc.id);
}

/// Zone couvrant « Bonjour » sur la page 1 de simple.pdf.
fn word_area(doc: DocumentId) -> Area {
    let text = engine().page_text(doc, 0).expect("texte");
    let letters: Vec<_> = text.chars.iter().take(7).collect();
    assert_eq!(letters.iter().map(|c| c.ch).collect::<String>(), "Bonjour");
    let first = letters[0].bounds;
    let last = letters[6].bounds;
    Area {
        top_left: Point::new(first.min().x, first.max().y + 2.0),
        width: last.max().x - first.min().x,
        height: first.height() + 6.0,
    }
}

fn markup(mark: MarkKind, area: Area) -> Element {
    Element::Markup(MarkupBox {
        page: 0,
        mark,
        areas: vec![area],
        color: YELLOW,
    })
}

/// Pixels de la zone surlignée, bords exclus. Marge de 2 points arrondie
/// vers l'intérieur : la rangée sous un bord fractionnaire (750,94 avec les
/// boîtes de caractères de Windows) n'est couverte qu'en partie.
fn area_pixels(page: &Bitmap, area: Area) -> Vec<[u8; 3]> {
    let rect = area.rect();
    let inner = |min: f64, max: f64| ((min + 2.0).ceil() as u32, (max - 2.0).floor() as u32);
    let (x0, x1) = inner(rect.min().x, rect.max().x);
    let (y0, y1) = inner(rect.min().y, rect.max().y);
    (x0..x1)
        .flat_map(|x| (y0..y1).map(move |y| (x, y)))
        .map(|(x, y)| color_at(page, x, y))
        .collect()
}

/// Le fond de la zone est jaune, les lettres restent sombres. `step` dit
/// quelle étape échoue, avec la part de pixels restés blancs.
fn assert_highlighted(page: &Bitmap, area: Area, step: &str) {
    let pixels = area_pixels(page, area);
    let white = pixels.iter().filter(|p| p.iter().all(|&v| v > 240)).count();
    assert_eq!(
        white,
        0,
        "{step} : {white} pixels blancs sur {} dans la zone {:?}",
        pixels.len(),
        area.rect()
    );
    let sum = |p: &&[u8; 3]| p.iter().map(|&v| u32::from(v)).sum::<u32>();
    let darkest = pixels.iter().min_by_key(sum).expect("pixels");
    let brightest = pixels.iter().max_by_key(sum).expect("pixels");
    assert_near(*brightest, [0xff, 0xd5, 0x4a], 6);
    assert!(
        darkest.iter().all(|&v| v < 90),
        "{step} : texte éclairci {darkest:?}"
    );
}

#[test]
fn highlight_colours_the_background_and_keeps_text_dark() {
    let doc = open_simple();
    let area = word_area(doc.id);
    place(doc.id, vec![markup(MarkKind::Highlight, area)]);
    assert_highlighted(&render(doc.id, 0), area, "avant enregistrement");

    // Bords fractionnaires relevés sous Windows (boîtes de caractères de
    // sa police de remplacement) : la rangée de bord ne compte pas.
    let windows_area = Area {
        top_left: Point::new(72.0, 783.72),
        width: 83.04,
        height: 32.78,
    };
    place(doc.id, vec![markup(MarkKind::Highlight, windows_area)]);
    assert_highlighted(&render(doc.id, 0), windows_area, "bords fractionnaires");
    place(doc.id, vec![markup(MarkKind::Highlight, area)]);

    // Toujours vrai après enregistrement (mode de fusion écrit dans le PDF).
    let bytes = engine().save(doc.id).expect("enregistrement");
    engine().close(doc.id);
    let saved = engine().open(bytes, None).expect("réouverture");
    assert_highlighted(&render(saved.id, 0), area, "après enregistrement");
    engine().close(saved.id);
}

#[test]
fn underline_and_strike_out_draw_thin_lines() {
    let doc = open_simple();
    let area = word_area(doc.id);
    let mut under = MarkupBox {
        page: 0,
        mark: MarkKind::Underline,
        areas: vec![area],
        color: RED,
    };
    let band = under.band(&area);
    place(doc.id, vec![Element::Markup(under.clone())]);
    let page = render(doc.id, 0);
    let mid_x = (band.min().x + band.max().x) as u32 / 2;
    let mid_y = ((band.min().y + band.max().y) / 2.0).round() as u32;
    assert_near(color_at(&page, mid_x, mid_y), RED_RGB, 40);

    under.mark = MarkKind::StrikeOut;
    let band = under.band(&area);
    place(doc.id, vec![Element::Markup(under)]);
    let page = render(doc.id, 0);
    let mid_y = ((band.min().y + band.max().y) / 2.0).round() as u32;
    assert_near(color_at(&page, mid_x + 1, mid_y), RED_RGB, 40);
    engine().close(doc.id);
}

#[test]
fn shapes_stay_where_drawn_on_a_rotated_page() {
    let doc = open_simple();
    // Page 2 affichée en 842 × 595 : rectangle en haut à gauche de l'écran.
    place(
        doc.id,
        vec![Element::Shape(shape(
            1,
            ShapeKind::Rectangle,
            (50.0, 450.0),
            (250.0, 550.0),
        ))],
    );
    let page = render(doc.id, 1);
    assert_eq!((page.width(), page.height()), (842, 595));
    assert_near(color_at(&page, 150, 550), RED_RGB, 12);
    assert_near(color_at(&page, 250, 500), RED_RGB, 12);
    assert_near(color_at(&page, 150, 500), WHITE, 4);
    engine().close(doc.id);
}

#[test]
fn annotations_match_the_reference_rendering() {
    let doc = open_simple();
    let area = word_area(doc.id);
    let mut ellipse = shape(0, ShapeKind::Ellipse, (330.0, 420.0), (520.0, 520.0));
    ellipse.filled = true;
    ellipse.color = Color {
        r: 0x27,
        g: 0x47,
        b: 0xb8,
    };
    let wave = (0..=40)
        .map(|i| {
            let x = 80.0 + f64::from(i) * 10.0;
            Point::new(x, 300.0 + 25.0 * (f64::from(i) / 3.0).sin())
        })
        .collect();
    place(
        doc.id,
        vec![
            markup(MarkKind::Highlight, area),
            Element::Shape(shape(
                0,
                ShapeKind::Rectangle,
                (80.0, 420.0),
                (280.0, 520.0),
            )),
            Element::Shape(ellipse),
            Element::Shape(shape(0, ShapeKind::Arrow, (80.0, 380.0), (520.0, 360.0))),
            Element::Ink(InkBox {
                page: 0,
                points: wave,
                color: Color {
                    r: 0x1e,
                    g: 0x73,
                    b: 0x44,
                },
                width: 3.0,
            }),
        ],
    );
    assert_matches_reference(&render(doc.id, 0), "annotations.png");
    engine().close(doc.id);
}
