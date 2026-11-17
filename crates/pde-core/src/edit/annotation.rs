//! Annotations dessinées (E-04) : surlignage, soulignement et barré du
//! texte, dessin à main levée, formes (rectangle, ellipse, trait, flèche).
//! Comme le texte et les images ajoutés, elles sont posées dans le contenu
//! de la page : même rendu dans tous les lecteurs.
//!
//! Coordonnées : page affichée, points, origine en bas à gauche, comme le
//! reste de `pde-core`. Le calcul des tracés (pointe de flèche, ellipse en
//! courbes de Bézier) est ici, sans dépendance au moteur.

use serde::{Deserialize, Serialize};

use super::element::Color;
use crate::document::PageIndex;
use crate::geometry::{Point, Rect};

/// Épaisseur de trait acceptée (points).
pub const STROKE_WIDTH_RANGE: std::ops::RangeInclusive<f64> = 0.25..=48.0;
/// Opacité du remplissage d'un rectangle ou d'une ellipse.
pub const FILL_OPACITY: f64 = 0.25;
/// Limites d'un élément (saisie à la souris : largement suffisant).
pub const MAX_INK_POINTS: usize = 20_000;
pub const MAX_MARKUP_AREAS: usize = 2_000;

fn default_width() -> f64 {
    2.0
}

fn red() -> Color {
    Color {
        r: 0xc6,
        g: 0x28,
        b: 0x28,
    }
}

fn yellow() -> Color {
    Color {
        r: 0xff,
        g: 0xd5,
        b: 0x4a,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
    Line,
    Arrow,
}

/// Forme : deux coins opposés (rectangle, ellipse) ou deux extrémités
/// (trait, flèche : la pointe est à `end`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeBox {
    pub page: PageIndex,
    pub shape: ShapeKind,
    pub start: Point,
    pub end: Point,
    #[serde(default = "red")]
    pub color: Color,
    #[serde(default = "default_width")]
    pub width: f64,
    /// Intérieur teinté (rectangle, ellipse) : la couleur à 25 %.
    #[serde(default)]
    pub filled: bool,
}

/// Trait à main levée : une suite de points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InkBox {
    pub page: PageIndex,
    pub points: Vec<Point>,
    #[serde(default = "red")]
    pub color: Color,
    #[serde(default = "default_width")]
    pub width: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkKind {
    Highlight,
    Underline,
    StrikeOut,
}

/// Zone de texte marquée (une par morceau de ligne sélectionné).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Area {
    /// Coin haut gauche.
    pub top_left: Point,
    pub width: f64,
    pub height: f64,
}

impl Area {
    pub fn rect(&self) -> Rect {
        Rect::from_corners(
            Point::new(self.top_left.x, self.top_left.y - self.height),
            Point::new(self.top_left.x + self.width, self.top_left.y),
        )
    }
}

/// Texte surligné, souligné ou barré.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkupBox {
    pub page: PageIndex,
    pub mark: MarkKind,
    pub areas: Vec<Area>,
    #[serde(default = "yellow")]
    pub color: Color,
}

fn grow(rect: Rect, by: f64) -> Rect {
    Rect::from_corners(
        Point::new(rect.min().x - by, rect.min().y - by),
        Point::new(rect.max().x + by, rect.max().y + by),
    )
}

fn bounding(points: impl IntoIterator<Item = Point>) -> Option<Rect> {
    points
        .into_iter()
        .map(|p| Rect::from_corners(p, p))
        .reduce(|a, b| a.union(&b))
}

fn all_finite(points: impl IntoIterator<Item = Point>) -> bool {
    points
        .into_iter()
        .all(|p| p.x.is_finite() && p.y.is_finite())
}

fn check_width(width: f64) -> Result<(), String> {
    if STROKE_WIDTH_RANGE.contains(&width) {
        Ok(())
    } else {
        Err(format!(
            "épaisseur entre {} et {} pt",
            STROKE_WIDTH_RANGE.start(),
            STROKE_WIDTH_RANGE.end()
        ))
    }
}

impl ShapeBox {
    /// Pointe d'une flèche : sommet, puis les deux coins de sa base.
    pub fn arrow_head(&self) -> [Point; 3] {
        let (dx, dy) = (self.end.x - self.start.x, self.end.y - self.start.y);
        let length = dx.hypot(dy).max(f64::EPSILON);
        let (ux, uy) = (dx / length, dy / length);
        // Pointe proportionnée au trait, jamais plus longue que la flèche.
        let size = (4.0 * self.width).max(8.0).min(length);
        let half = size * 0.45;
        let base = Point::new(self.end.x - ux * size, self.end.y - uy * size);
        [
            self.end,
            Point::new(base.x - uy * half, base.y + ux * half),
            Point::new(base.x + uy * half, base.y - ux * half),
        ]
    }

    /// Place occupée, épaisseur du trait et pointe comprises.
    pub fn bounds(&self) -> Rect {
        let mut rect = Rect::from_corners(self.start, self.end);
        if self.shape == ShapeKind::Arrow {
            rect = self
                .arrow_head()
                .iter()
                .fold(rect, |r, p| r.union(&Rect::from_corners(*p, *p)));
        }
        grow(rect, self.width / 2.0)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !all_finite([self.start, self.end]) || !self.width.is_finite() {
            return Err("position invalide".into());
        }
        check_width(self.width)
    }
}

/// Ellipse inscrite dans `rect` : point de départ, puis quatre courbes de
/// Bézier (deux points de contrôle et le point d'arrivée chacune).
pub fn ellipse_curves(rect: Rect) -> (Point, [[Point; 3]; 4]) {
    // Écart des points de contrôle pour approcher un quart de cercle.
    const KAPPA: f64 = 0.552_284_749_8;
    let (cx, cy) = (
        (rect.min().x + rect.max().x) / 2.0,
        (rect.min().y + rect.max().y) / 2.0,
    );
    let (rx, ry) = (rect.width() / 2.0, rect.height() / 2.0);
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    let p = Point::new;
    (
        p(cx + rx, cy),
        [
            [p(cx + rx, cy + ky), p(cx + kx, cy + ry), p(cx, cy + ry)],
            [p(cx - kx, cy + ry), p(cx - rx, cy + ky), p(cx - rx, cy)],
            [p(cx - rx, cy - ky), p(cx - kx, cy - ry), p(cx, cy - ry)],
            [p(cx + kx, cy - ry), p(cx + rx, cy - ky), p(cx + rx, cy)],
        ],
    )
}

impl InkBox {
    pub fn bounds(&self) -> Rect {
        grow(
            bounding(self.points.iter().copied()).unwrap_or_default(),
            self.width / 2.0,
        )
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.points.is_empty() || self.points.len() > MAX_INK_POINTS {
            return Err(format!("tracé de 1 à {MAX_INK_POINTS} points"));
        }
        if !all_finite(self.points.iter().copied()) || !self.width.is_finite() {
            return Err("tracé invalide".into());
        }
        check_width(self.width)
    }
}

impl MarkupBox {
    pub fn bounds(&self) -> Rect {
        self.areas
            .iter()
            .map(Area::rect)
            .reduce(|a, b| a.union(&b))
            .unwrap_or_default()
    }

    /// Bande dessinée pour une zone : toute la zone (surlignage), un trait
    /// sous le texte (souligné) ou à mi-hauteur (barré).
    pub fn band(&self, area: &Area) -> Rect {
        let rect = area.rect();
        let thickness = (area.height * 0.07).max(0.75);
        let bottom = rect.min().y;
        match self.mark {
            MarkKind::Highlight => rect,
            MarkKind::Underline => Rect::from_origin_size(
                Point::new(rect.min().x, bottom + area.height * 0.06),
                area.width,
                thickness,
            ),
            MarkKind::StrikeOut => Rect::from_origin_size(
                Point::new(rect.min().x, bottom + area.height * 0.42),
                area.width,
                thickness,
            ),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.areas.is_empty() || self.areas.len() > MAX_MARKUP_AREAS {
            return Err(format!("de 1 à {MAX_MARKUP_AREAS} zones de texte"));
        }
        let sizes_ok = self.areas.iter().all(|a| {
            a.width.is_finite() && a.height.is_finite() && a.width > 0.0 && a.height > 0.0
        });
        if !sizes_ok || !all_finite(self.areas.iter().map(|a| a.top_left)) {
            return Err("zone de texte invalide".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(kind: ShapeKind) -> ShapeBox {
        ShapeBox {
            page: 0,
            shape: kind,
            start: Point::new(100.0, 100.0),
            end: Point::new(200.0, 100.0),
            color: red(),
            width: 2.0,
            filled: false,
        }
    }

    #[test]
    fn arrow_head_points_at_the_end_and_stays_short() {
        let arrow = shape(ShapeKind::Arrow);
        let [tip, left, right] = arrow.arrow_head();
        assert_eq!(tip, Point::new(200.0, 100.0));
        // Pointe de 8 pt (trait de 2 pt), symétrique de part et d'autre.
        assert!((left.x - 192.0).abs() < 1e-9 && (right.x - 192.0).abs() < 1e-9);
        assert!((left.y - 103.6).abs() < 1e-9 && (right.y - 96.4).abs() < 1e-9);
        // Flèche très courte : la pointe ne dépasse pas le départ.
        let mut tiny = arrow;
        tiny.end = Point::new(103.0, 100.0);
        assert!(tiny.arrow_head()[1].x >= 100.0 - 1e-9);
    }

    #[test]
    fn bounds_include_stroke_and_arrow_head() {
        let line = shape(ShapeKind::Line).bounds();
        assert_eq!(
            (line.min(), line.max()),
            (Point::new(99.0, 99.0), Point::new(201.0, 101.0))
        );
        let arrow = shape(ShapeKind::Arrow).bounds();
        assert!(arrow.max().y > 103.6 && arrow.min().y < 96.4);
        let ink = InkBox {
            page: 0,
            points: vec![Point::new(10.0, 20.0), Point::new(30.0, 5.0)],
            color: red(),
            width: 4.0,
        };
        assert_eq!(
            (ink.bounds().min(), ink.bounds().max()),
            (Point::new(8.0, 3.0), Point::new(32.0, 22.0))
        );
    }

    #[test]
    fn ellipse_curves_stay_on_the_ellipse() {
        let rect = Rect::from_corners(Point::new(0.0, 0.0), Point::new(200.0, 100.0));
        let (start, curves) = ellipse_curves(rect);
        assert_eq!(start, Point::new(200.0, 50.0));
        let ends: Vec<_> = curves.iter().map(|c| c[2]).collect();
        assert_eq!(
            ends,
            [
                Point::new(100.0, 100.0),
                Point::new(0.0, 50.0),
                Point::new(100.0, 0.0),
                Point::new(200.0, 50.0)
            ]
        );
    }

    #[test]
    fn markup_bands() {
        let markup = |mark| MarkupBox {
            page: 0,
            mark,
            areas: vec![Area {
                top_left: Point::new(50.0, 500.0),
                width: 100.0,
                height: 20.0,
            }],
            color: yellow(),
        };
        let m = markup(MarkKind::Highlight);
        assert_eq!(m.band(&m.areas[0]), m.areas[0].rect());
        let u = markup(MarkKind::Underline);
        let band = u.band(&u.areas[0]);
        assert!(band.min().y > 480.0 && band.max().y < 484.0, "{band:?}");
        let s = markup(MarkKind::StrikeOut);
        let band = s.band(&s.areas[0]);
        assert!(band.min().y > 488.0 && band.max().y < 492.0, "{band:?}");
        assert_eq!(m.bounds(), m.areas[0].rect());
    }

    #[test]
    fn invalid_annotations_are_explained() {
        let mut s = shape(ShapeKind::Rectangle);
        s.width = 0.0;
        assert!(s.validate().is_err());
        s.width = 2.0;
        s.end.x = f64::NAN;
        assert!(s.validate().is_err());
        let ink = InkBox {
            page: 0,
            points: Vec::new(),
            color: red(),
            width: 2.0,
        };
        assert!(ink.validate().is_err());
        let markup = MarkupBox {
            page: 0,
            mark: MarkKind::Underline,
            areas: vec![Area {
                top_left: Point::new(0.0, 0.0),
                width: -1.0,
                height: 5.0,
            }],
            color: yellow(),
        };
        assert!(markup.validate().is_err());
    }

    #[test]
    fn json_shape_and_defaults() {
        let json = r#"{"page":0,"shape":"arrow","start":{"x":1,"y":2},"end":{"x":3,"y":4}}"#;
        let s: ShapeBox = serde_json::from_str(json).unwrap();
        assert_eq!(
            (s.shape, s.width, s.filled, s.color),
            (ShapeKind::Arrow, 2.0, false, red())
        );
        let m: MarkupBox = serde_json::from_str(
            r#"{"page":1,"mark":"strike_out","areas":[{"top_left":{"x":1,"y":2},"width":3,"height":4}]}"#,
        )
        .unwrap();
        assert_eq!((m.mark, m.color), (MarkKind::StrikeOut, yellow()));
    }
}
