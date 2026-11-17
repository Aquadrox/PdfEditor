//! Tracé des annotations dessinées (E-04) dans le contenu de la page :
//! chemins PDFium (traits, courbes, remplissages). Les points viennent de
//! la page affichée et sont ramenés à la page PDF un par un : une
//! transformation affine garde les droites et les courbes de Bézier.

use pde_core::edit::{
    Color, FILL_OPACITY, InkBox, MarkKind, MarkupBox, ShapeBox, ShapeKind, ellipse_curves,
};
use pde_core::engine::EngineError;
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{
    PdfColor, PdfDocument, PdfPageObjectBlendMode, PdfPageObjectCommon, PdfPageObjectLineCap,
    PdfPageObjectLineJoin, PdfPageObjectsCommon, PdfPagePathObject, PdfPoints,
};

use crate::convert::internal;
use crate::display::DisplayTransform;

fn pdf_color(c: Color, opacity: f64) -> PdfColor {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    PdfColor::new(c.r, c.g, c.b, alpha)
}

fn points(display: &DisplayTransform, p: Point) -> (PdfPoints, PdfPoints) {
    let (x, y) = display.page_point(p);
    (PdfPoints::new(x as f32), PdfPoints::new(y as f32))
}

/// Chemin dessiné en coordonnées de la page affichée.
struct Pen<'a, 'd> {
    path: PdfPagePathObject<'a>,
    display: &'d DisplayTransform,
}

impl<'a, 'd> Pen<'a, 'd> {
    fn new(
        doc: &PdfDocument<'a>,
        display: &'d DisplayTransform,
        start: Point,
        stroke: Option<(PdfColor, f64)>,
        fill: Option<PdfColor>,
    ) -> Result<Self, EngineError> {
        let (x, y) = points(display, start);
        let mut path = PdfPagePathObject::new(
            doc,
            x,
            y,
            stroke.map(|(c, _)| c),
            stroke.map(|(_, w)| PdfPoints::new(w as f32)),
            fill,
        )
        .map_err(internal)?;
        if stroke.is_some() {
            path.set_line_cap(PdfPageObjectLineCap::Round)
                .map_err(internal)?;
            path.set_line_join(PdfPageObjectLineJoin::Round)
                .map_err(internal)?;
        }
        Ok(Self { path, display })
    }

    fn line_to(&mut self, p: Point) -> Result<(), EngineError> {
        let (x, y) = points(self.display, p);
        self.path.line_to(x, y).map_err(internal)
    }

    fn curve_to(&mut self, [c1, c2, end]: [Point; 3]) -> Result<(), EngineError> {
        let (x, y) = points(self.display, end);
        let (x1, y1) = points(self.display, c1);
        let (x2, y2) = points(self.display, c2);
        self.path.bezier_to(x, y, x1, y1, x2, y2).map_err(internal)
    }

    fn close(&mut self) -> Result<(), EngineError> {
        self.path.close_path().map_err(internal)
    }

    /// Rectangle fermé (départ au coin `min`).
    fn rect(&mut self, r: Rect) -> Result<(), EngineError> {
        self.line_to(Point::new(r.max().x, r.min().y))?;
        self.line_to(r.max())?;
        self.line_to(Point::new(r.min().x, r.max().y))?;
        self.close()
    }

    fn ellipse(&mut self, curves: [[Point; 3]; 4]) -> Result<(), EngineError> {
        for curve in curves {
            self.curve_to(curve)?;
        }
        self.close()
    }
}

/// Ajoute les chemins à la page de l'élément.
fn add(
    doc: &mut PdfDocument,
    page: usize,
    paths: Vec<PdfPagePathObject>,
) -> Result<(), EngineError> {
    let (mut page, _) = super::page_of(doc, page)?;
    for path in paths {
        page.objects_mut().add_path_object(path).map_err(internal)?;
    }
    Ok(())
}

pub(super) fn write_shape(doc: &mut PdfDocument, shape: &ShapeBox) -> Result<Rect, EngineError> {
    let (_, display) = super::page_of(doc, shape.page)?;
    let stroke = Some((pdf_color(shape.color, 1.0), shape.width));
    let fill = shape.filled.then(|| pdf_color(shape.color, FILL_OPACITY));
    let frame = Rect::from_corners(shape.start, shape.end);
    let mut paths = Vec::new();
    match shape.shape {
        ShapeKind::Rectangle => {
            let mut pen = Pen::new(doc, &display, frame.min(), stroke, fill)?;
            pen.rect(frame)?;
            paths.push(pen.path);
        }
        ShapeKind::Ellipse => {
            let (start, curves) = ellipse_curves(frame);
            let mut pen = Pen::new(doc, &display, start, stroke, fill)?;
            pen.ellipse(curves)?;
            paths.push(pen.path);
        }
        ShapeKind::Line => {
            let mut pen = Pen::new(doc, &display, shape.start, stroke, None)?;
            pen.line_to(shape.end)?;
            paths.push(pen.path);
        }
        ShapeKind::Arrow => {
            let [tip, left, right] = shape.arrow_head();
            // Le trait s'arrête à la base de la pointe : il ne la dépasse pas.
            let base = Point::new((left.x + right.x) / 2.0, (left.y + right.y) / 2.0);
            let mut shaft = Pen::new(doc, &display, shape.start, stroke, None)?;
            shaft.line_to(base)?;
            let mut head = Pen::new(doc, &display, tip, None, Some(pdf_color(shape.color, 1.0)))?;
            head.line_to(left)?;
            head.line_to(right)?;
            head.close()?;
            paths.extend([shaft.path, head.path]);
        }
    }
    add(doc, shape.page, paths)?;
    Ok(shape.bounds())
}

pub(super) fn write_ink(doc: &mut PdfDocument, ink: &InkBox) -> Result<Rect, EngineError> {
    let (_, display) = super::page_of(doc, ink.page)?;
    let color = pdf_color(ink.color, 1.0);
    let path = match ink.points.as_slice() {
        // Un simple clic : un point rond de l'épaisseur du trait.
        [single] => {
            let r = ink.width / 2.0;
            let dot = Rect::from_corners(
                Point::new(single.x - r, single.y - r),
                Point::new(single.x + r, single.y + r),
            );
            let (start, curves) = ellipse_curves(dot);
            let mut pen = Pen::new(doc, &display, start, None, Some(color))?;
            pen.ellipse(curves)?;
            pen.path
        }
        [first, rest @ ..] => {
            let mut pen = Pen::new(doc, &display, *first, Some((color, ink.width)), None)?;
            for p in rest {
                pen.line_to(*p)?;
            }
            pen.path
        }
        [] => return Ok(ink.bounds()),
    };
    add(doc, ink.page, vec![path])?;
    Ok(ink.bounds())
}

pub(super) fn write_markup(doc: &mut PdfDocument, markup: &MarkupBox) -> Result<Rect, EngineError> {
    let (_, display) = super::page_of(doc, markup.page)?;
    let color = pdf_color(markup.color, 1.0);
    let mut paths = Vec::with_capacity(markup.areas.len());
    for area in &markup.areas {
        let band = markup.band(area);
        let mut pen = Pen::new(doc, &display, band.min(), None, Some(color))?;
        pen.rect(band)?;
        if markup.mark == MarkKind::Highlight {
            // « Produit » : le texte noir reste noir, le fond prend la couleur.
            pen.path
                .set_blend_mode(PdfPageObjectBlendMode::Multiply)
                .map_err(internal)?;
        }
        paths.push(pen.path);
    }
    add(doc, markup.page, paths)?;
    Ok(markup.bounds())
}
