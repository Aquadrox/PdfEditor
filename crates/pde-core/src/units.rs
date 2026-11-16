//! Unités de longueur.
//!
//! Le PDF mesure tout en points typographiques (1 pt = 1/72 pouce).
//! L'utilisateur, lui, peut choisir millimètres ou pouces dans les réglages :
//! les conversions se font ici et nulle part ailleurs.

/// Points par pouce, fixé par la norme PDF (ISO 32000).
pub const POINTS_PER_INCH: f64 = 72.0;
/// Millimètres par pouce.
pub const MM_PER_INCH: f64 = 25.4;

/// Unité affichée à l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Unit {
    /// Point typographique PDF (1/72 pouce).
    #[default]
    Point,
    Millimeter,
    Inch,
}

impl Unit {
    /// Nombre de points dans une unité.
    pub fn points_per_unit(self) -> f64 {
        match self {
            Self::Point => 1.0,
            Self::Millimeter => POINTS_PER_INCH / MM_PER_INCH,
            Self::Inch => POINTS_PER_INCH,
        }
    }

    /// Convertit `value` (exprimée dans cette unité) en points.
    pub fn to_points(self, value: f64) -> f64 {
        value * self.points_per_unit()
    }

    /// Convertit `points` dans cette unité.
    pub fn from_points(self, points: f64) -> f64 {
        points / self.points_per_unit()
    }

    /// Symbole affiché après une valeur.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Point => "pt",
            Self::Millimeter => "mm",
            Self::Inch => "in",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn a4_width_in_points() {
        // A4 : 210 mm = 595,2756 pt (valeur des MediaBox A4 courantes : 595).
        assert!(close(
            Unit::Millimeter.to_points(210.0),
            595.275_590_551_181_1
        ));
    }

    #[test]
    fn inch_is_72_points() {
        assert!(close(Unit::Inch.to_points(1.0), 72.0));
        assert!(close(Unit::Inch.from_points(144.0), 2.0));
    }

    #[test]
    fn round_trip_every_unit() {
        for unit in [Unit::Point, Unit::Millimeter, Unit::Inch] {
            let value = 123.456;
            assert!(
                close(unit.from_points(unit.to_points(value)), value),
                "{unit:?}"
            );
        }
    }

    #[test]
    fn symbols() {
        assert_eq!(Unit::Point.symbol(), "pt");
        assert_eq!(Unit::Millimeter.symbol(), "mm");
        assert_eq!(Unit::Inch.symbol(), "in");
    }
}
