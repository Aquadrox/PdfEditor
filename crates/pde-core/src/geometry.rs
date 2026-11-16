//! Géométrie dans l'espace d'une page PDF.
//!
//! Convention : coordonnées en points, origine en bas à gauche de la page,
//! axe Y vers le haut (comme le PDF). L'interface, dont l'axe Y descend,
//! convertit avec [`Rect::flip_y`] ; aucune autre conversion n'existe.

/// Point en coordonnées de page (points PDF).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Rectangle aligné sur les axes, toujours normalisé : `min` est le coin
/// bas-gauche, `max` le coin haut-droit (largeur et hauteur jamais négatives).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    min: Point,
    max: Point,
}

impl Rect {
    /// Rectangle passant par deux coins opposés quelconques (ordre libre,
    /// comme un glisser de souris dans n'importe quel sens).
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self {
            min: Point::new(a.x.min(b.x), a.y.min(b.y)),
            max: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    /// Rectangle d'origine `origin` (coin bas-gauche) et de taille donnée ;
    /// une taille négative est retournée dans l'autre sens.
    pub fn from_origin_size(origin: Point, width: f64, height: f64) -> Self {
        Self::from_corners(origin, Point::new(origin.x + width, origin.y + height))
    }

    pub fn min(&self) -> Point {
        self.min
    }

    pub fn max(&self) -> Point {
        self.max
    }

    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Surface nulle (un clic sans glisser, par exemple).
    pub fn is_empty(&self) -> bool {
        self.width() <= 0.0 || self.height() <= 0.0
    }

    /// Le point est dans le rectangle, bords compris.
    pub fn contains(&self, p: Point) -> bool {
        (self.min.x..=self.max.x).contains(&p.x) && (self.min.y..=self.max.y).contains(&p.y)
    }

    /// Partie commune aux deux rectangles, s'il y en a une d'aire non nulle.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let r = Rect {
            min: Point::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y)),
            max: Point::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y)),
        };
        (r.width() > 0.0 && r.height() > 0.0).then_some(r)
    }

    /// Plus petit rectangle contenant les deux.
    pub fn union(&self, other: &Rect) -> Rect {
        Rect {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// Déplace le rectangle sans changer sa taille.
    pub fn translate(&self, dx: f64, dy: f64) -> Rect {
        Rect {
            min: Point::new(self.min.x + dx, self.min.y + dy),
            max: Point::new(self.max.x + dx, self.max.y + dy),
        }
    }

    /// Passe de l'axe Y du PDF (vers le haut) à celui de l'écran (vers le
    /// bas) pour une page de hauteur `page_height`, et inversement : la
    /// conversion est sa propre réciproque.
    pub fn flip_y(&self, page_height: f64) -> Rect {
        Rect::from_corners(
            Point::new(self.min.x, page_height - self.min.y),
            Point::new(self.max.x, page_height - self.max.y),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect::from_corners(Point::new(x0, y0), Point::new(x1, y1))
    }

    #[test]
    fn corners_are_normalized_whatever_the_drag_direction() {
        let expected = rect(10.0, 20.0, 110.0, 70.0);
        assert_eq!(rect(110.0, 70.0, 10.0, 20.0), expected);
        assert_eq!(rect(10.0, 70.0, 110.0, 20.0), expected);
        assert_eq!(expected.width(), 100.0);
        assert_eq!(expected.height(), 50.0);
    }

    #[test]
    fn negative_size_is_flipped() {
        let r = Rect::from_origin_size(Point::new(50.0, 50.0), -20.0, -10.0);
        assert_eq!(r, rect(30.0, 40.0, 50.0, 50.0));
    }

    #[test]
    fn empty_when_no_area() {
        assert!(rect(5.0, 5.0, 5.0, 40.0).is_empty());
        assert!(Rect::default().is_empty());
        assert!(!rect(0.0, 0.0, 1.0, 1.0).is_empty());
    }

    #[test]
    fn contains_includes_edges() {
        let r = rect(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(Point::new(0.0, 10.0)));
        assert!(r.contains(Point::new(5.0, 5.0)));
        assert!(!r.contains(Point::new(10.1, 5.0)));
    }

    #[test]
    fn intersection_of_overlapping_and_disjoint_rects() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(
            a.intersection(&rect(5.0, 5.0, 20.0, 20.0)),
            Some(rect(5.0, 5.0, 10.0, 10.0))
        );
        assert_eq!(a.intersection(&rect(20.0, 20.0, 30.0, 30.0)), None);
        // Bords communs seulement : aire nulle, pas d'intersection.
        assert_eq!(a.intersection(&rect(10.0, 0.0, 20.0, 10.0)), None);
    }

    #[test]
    fn union_contains_both() {
        let u = rect(0.0, 0.0, 10.0, 10.0).union(&rect(5.0, -5.0, 20.0, 8.0));
        assert_eq!(u, rect(0.0, -5.0, 20.0, 10.0));
    }

    #[test]
    fn translate_keeps_size() {
        let r = rect(0.0, 0.0, 10.0, 5.0).translate(3.0, -2.0);
        assert_eq!(r, rect(3.0, -2.0, 13.0, 3.0));
    }

    #[test]
    fn flip_y_maps_pdf_to_screen_and_back() {
        // Page A4 (842 pt de haut) : un bloc à 42 pt du bas est à 790 pt du haut.
        let pdf = rect(72.0, 42.0, 172.0, 62.0);
        let screen = pdf.flip_y(842.0);
        assert_eq!(screen, rect(72.0, 780.0, 172.0, 800.0));
        assert_eq!(screen.flip_y(842.0), pdf);
    }
}
