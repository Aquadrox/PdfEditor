//! Éléments tournés avec leur page (P-01) : quand une page pivote d'un
//! quart de tour, ses ajouts gardent leur place sur le papier.
//!
//! Un quart de tour dans le sens des aiguilles d'une montre, page affichée
//! de largeur `W` (origine en bas à gauche) : le point `(x, y)` devient
//! `(y, W − x)` sur la page tournée, de taille `H × W`.

use crate::document::PageSize;
use crate::geometry::Point;

use super::annotation::{Area, MarkKind};
use super::element::Element;
use super::note::NOTE_SIZE;

/// L'élément peut tourner avec sa page. Le texte ajouté, le texte
/// d'origine modifié et les traits soulignés ou barrés s'écrivent
/// horizontalement : ils ne pivotent pas encore.
pub fn can_rotate(element: &Element) -> bool {
    match element {
        Element::Text(_) | Element::OriginalText(_) => false,
        Element::Markup(markup) => markup.mark == MarkKind::Highlight,
        Element::Image(_)
        | Element::Shape(_)
        | Element::Ink(_)
        | Element::Note(_)
        | Element::Redact(_)
        | Element::ExistingImage(_) => true,
    }
}

/// L'élément tourné de `turns` quarts de tour (sens horaire) avec sa page
/// de taille affichée `size` (avant rotation).
pub fn rotated(element: &Element, size: PageSize, turns: u8) -> Result<Element, String> {
    if !can_rotate(element) {
        return Err("ce texte ne peut pas encore pivoter avec sa page".into());
    }
    let mut element = element.clone();
    let mut size = size;
    for _ in 0..turns % 4 {
        element = quarter_turn(element, size);
        size = PageSize::new(size.height, size.width);
    }
    Ok(element)
}

fn point(p: Point, size: PageSize) -> Point {
    Point::new(p.y, size.width - p.x)
}

/// Rectangle donné par son coin haut gauche et sa taille : son nouveau
/// coin haut gauche (l'ancien coin bas gauche) ; largeur et hauteur s'échangent.
fn top_left(top_left: Point, height: f64, size: PageSize) -> Point {
    point(Point::new(top_left.x, top_left.y - height), size)
}

fn turned_areas(areas: &[Area], size: PageSize) -> Vec<Area> {
    areas
        .iter()
        .map(|a| Area {
            top_left: top_left(a.top_left, a.height, size),
            width: a.height,
            height: a.width,
        })
        .collect()
}

fn quarter_turn(element: Element, size: PageSize) -> Element {
    match element {
        Element::Image(mut image) => {
            image.top_left = top_left(image.top_left, image.height, size);
            std::mem::swap(&mut image.width, &mut image.height);
            image.rotation = (image.rotation + 1) % 4;
            Element::Image(image)
        }
        Element::Shape(mut shape) => {
            shape.start = point(shape.start, size);
            shape.end = point(shape.end, size);
            Element::Shape(shape)
        }
        Element::Ink(mut ink) => {
            ink.points = ink.points.iter().map(|p| point(*p, size)).collect();
            Element::Ink(ink)
        }
        Element::Markup(mut markup) => {
            markup.areas = turned_areas(&markup.areas, size);
            Element::Markup(markup)
        }
        Element::Redact(mut redact) => {
            redact.areas = turned_areas(&redact.areas, size);
            Element::Redact(redact)
        }
        Element::ExistingImage(mut image) => {
            image.original = turned_areas(&[image.original], size)[0];
            image.frame = image.frame.map(|f| turned_areas(&[f], size)[0]);
            Element::ExistingImage(image)
        }
        // L'icône reste droite : seul son carré change de coin.
        Element::Note(mut note) => {
            note.top_left = top_left(note.top_left, NOTE_SIZE, size);
            Element::Note(note)
        }
        // Écarté par `can_rotate`.
        element @ (Element::Text(_) | Element::OriginalText(_)) => element,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{Color, ImageBox, ImageId, InkBox, MarkupBox, NoteBox, TextBox, TextStyle};

    const A4: PageSize = PageSize::new(595.0, 842.0);

    fn image() -> Element {
        Element::Image(ImageBox {
            page: 0,
            top_left: Point::new(100.0, 800.0),
            width: 200.0,
            height: 50.0,
            image: ImageId(1),
            rotation: 0,
            opacity: 1.0,
            keep_ratio: true,
        })
    }

    #[test]
    fn a_quarter_turn_keeps_the_place_on_the_paper() {
        // Image en haut à gauche d'une page portrait : après un quart de
        // tour horaire, elle est en haut à droite de la page paysage.
        let Element::Image(turned) = rotated(&image(), A4, 1).unwrap() else {
            panic!("image attendue");
        };
        assert_eq!(turned.top_left, Point::new(750.0, 495.0));
        assert_eq!((turned.width, turned.height), (50.0, 200.0));
        assert_eq!(turned.rotation, 1);
        // Toujours dans la page paysage (842 × 595).
        assert!(turned.top_left.x + turned.width <= 842.0);
        assert!(turned.top_left.y <= 595.0 && turned.top_left.y - turned.height >= 0.0);
    }

    #[test]
    fn four_quarter_turns_come_back() {
        let ink = Element::Ink(InkBox {
            page: 0,
            points: vec![Point::new(10.0, 20.0), Point::new(300.0, 700.0)],
            color: Color::INK,
            width: 2.0,
        });
        let note = Element::Note(NoteBox {
            page: 0,
            top_left: Point::new(50.0, 60.0),
            text: String::new(),
            author: String::new(),
            color: Color::INK,
        });
        for element in [image(), ink, note] {
            let mut current = element.clone();
            let mut size = A4;
            for _ in 0..4 {
                current = rotated(&current, size, 1).unwrap();
                size = PageSize::new(size.height, size.width);
            }
            assert_eq!(current, element);
            assert_eq!(rotated(&element, A4, 4).unwrap(), element);
            assert_eq!(rotated(&element, A4, 2), {
                let half = rotated(&element, A4, 1).unwrap();
                rotated(&half, PageSize::new(842.0, 595.0), 1)
            });
        }
    }

    #[test]
    fn highlight_turns_but_underlined_and_added_text_do_not() {
        let markup = |mark| {
            Element::Markup(MarkupBox {
                page: 0,
                mark,
                areas: vec![Area {
                    top_left: Point::new(72.0, 700.0),
                    width: 100.0,
                    height: 12.0,
                }],
                color: Color::INK,
            })
        };
        let Element::Markup(turned) = rotated(&markup(MarkKind::Highlight), A4, 1).unwrap() else {
            panic!("surlignage attendu");
        };
        assert_eq!(turned.areas[0].top_left, Point::new(688.0, 523.0));
        assert_eq!(
            (turned.areas[0].width, turned.areas[0].height),
            (12.0, 100.0)
        );
        assert!(rotated(&markup(MarkKind::Underline), A4, 1).is_err());
        let text = Element::Text(TextBox {
            page: 0,
            top_left: Point::new(72.0, 700.0),
            width: 100.0,
            text: "a".into(),
            style: TextStyle::default(),
        });
        assert!(!can_rotate(&text));
        assert!(rotated(&text, A4, 1).is_err());
    }
}
