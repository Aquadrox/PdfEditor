//! Passage des coordonnées de la page PDF (origine et rotation propres au
//! fichier) à celles de la page affichée (rotation appliquée, origine en bas
//! à gauche), en interrogeant PDFium : il connaît la rotation et les boîtes
//! de la page mieux que nous.

use pde_core::engine::EngineError;
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{PdfPage, PdfPoints, PdfRenderConfig};

use crate::convert::internal;

/// Pixels par point du repère virtuel interrogé : PDFium renvoie des pixels
/// entiers, l'erreur d'arrondi reste ainsi inférieure à 1/64 pt.
const SAMPLE_SCALE: f32 = 64.0;
/// Distance (en points) des points d'échantillonnage à l'origine.
const SAMPLE_SPAN: f32 = 1000.0;

/// Transformation affine : affichage = origine + x·ax + y·ay (axe y vers le
/// bas), puis retournement vertical selon la hauteur affichée.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DisplayTransform {
    origin: (f64, f64),
    x_axis: (f64, f64),
    y_axis: (f64, f64),
    height: f64,
}

impl DisplayTransform {
    pub(crate) fn new(
        origin: (f64, f64),
        x_axis: (f64, f64),
        y_axis: (f64, f64),
        height: f64,
    ) -> Self {
        Self {
            origin,
            x_axis,
            y_axis,
            height,
        }
    }

    pub(crate) fn for_page(page: &PdfPage) -> Result<Self, EngineError> {
        let config = PdfRenderConfig::new().scale_page_by_factor(SAMPLE_SCALE);
        let device = |x: f32, y: f32| -> Result<(f64, f64), EngineError> {
            let (px, py) = page
                .points_to_pixels(PdfPoints::new(x), PdfPoints::new(y), &config)
                .map_err(internal)?;
            Ok((
                f64::from(px) / f64::from(SAMPLE_SCALE),
                f64::from(py) / f64::from(SAMPLE_SCALE),
            ))
        };
        let origin = device(0.0, 0.0)?;
        let x_end = device(SAMPLE_SPAN, 0.0)?;
        let y_end = device(0.0, SAMPLE_SPAN)?;
        let span = f64::from(SAMPLE_SPAN);
        let axis = |end: (f64, f64)| ((end.0 - origin.0) / span, (end.1 - origin.1) / span);
        Ok(Self::new(
            origin,
            axis(x_end),
            axis(y_end),
            f64::from(page.height().value),
        ))
    }

    /// Point de la page PDF → point de la page affichée (origine en bas).
    fn point(&self, x: f64, y: f64) -> Point {
        let down_x = self.origin.0 + x * self.x_axis.0 + y * self.y_axis.0;
        let down_y = self.origin.1 + x * self.x_axis.1 + y * self.y_axis.1;
        Point::new(down_x, self.height - down_y)
    }

    /// Partie linéaire de la transformation (page → affichée, origine en bas).
    fn linear(&self) -> [[f64; 2]; 2] {
        [
            [self.x_axis.0, self.y_axis.0],
            [-self.x_axis.1, -self.y_axis.1],
        ]
    }

    /// Vecteur de la page affichée → vecteur de la page PDF (inverse de la
    /// partie linéaire ; la rotation d'une page est toujours inversible).
    pub(crate) fn page_vector(&self, dx: f64, dy: f64) -> (f64, f64) {
        let [[a, b], [c, d]] = self.linear();
        let det = a * d - b * c;
        ((d * dx - b * dy) / det, (-c * dx + a * dy) / det)
    }

    /// Point de la page affichée (origine en bas) → point de la page PDF.
    pub(crate) fn page_point(&self, p: Point) -> (f64, f64) {
        let (tx, ty) = (self.origin.0, self.height - self.origin.1);
        self.page_vector(p.x - tx, p.y - ty)
    }

    /// Rectangle de la page PDF → rectangle de la page affichée.
    pub(crate) fn rect(&self, r: Rect) -> Rect {
        Rect::from_corners(
            self.point(r.min().x, r.min().y),
            self.point(r.max().x, r.max().y),
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
    fn unrotated_page_keeps_coordinates() {
        // A4 sans rotation : y vers le bas depuis le haut de la page.
        let t = DisplayTransform::new((0.0, 842.0), (1.0, 0.0), (0.0, -1.0), 842.0);
        assert_eq!(
            t.rect(rect(72.0, 760.0, 84.0, 780.0)),
            rect(72.0, 760.0, 84.0, 780.0)
        );
    }

    #[test]
    fn inverse_brings_points_back_to_the_page() {
        for t in [
            DisplayTransform::new((0.0, 842.0), (1.0, 0.0), (0.0, -1.0), 842.0),
            DisplayTransform::new((0.0, 0.0), (0.0, 1.0), (1.0, 0.0), 595.0),
        ] {
            let page = (72.0, 760.0);
            let shown = t.point(page.0, page.1);
            let back = t.page_point(shown);
            assert!(
                (back.0 - page.0).abs() < 1e-9 && (back.1 - page.1).abs() < 1e-9,
                "{back:?}"
            );
        }
    }

    #[test]
    fn display_x_axis_follows_the_rotation() {
        // Page tournée de 90° : avancer vers la droite à l'écran revient à
        // monter dans la page PDF.
        let t = DisplayTransform::new((0.0, 0.0), (0.0, 1.0), (1.0, 0.0), 595.0);
        assert_eq!(t.page_vector(1.0, 0.0), (0.0, 1.0));
        // Sans rotation : identique.
        let t = DisplayTransform::new((0.0, 842.0), (1.0, 0.0), (0.0, -1.0), 842.0);
        assert_eq!(t.page_vector(1.0, 0.0), (1.0, 0.0));
        assert_eq!(t.page_vector(0.0, 1.0), (0.0, 1.0));
    }

    #[test]
    fn quarter_turn_moves_text_to_the_right_edge() {
        // A4 tournée de 90° : affichée en 842 × 595. Le bas de la page PDF
        // devient le bord gauche, le haut devient le bord droit.
        let t = DisplayTransform::new((0.0, 0.0), (0.0, 1.0), (1.0, 0.0), 595.0);
        // Texte en (72, 760) du PDF : à droite et en haut une fois affiché.
        assert_eq!(
            t.rect(rect(72.0, 760.0, 84.0, 780.0)),
            rect(760.0, 511.0, 780.0, 523.0)
        );
    }
}
