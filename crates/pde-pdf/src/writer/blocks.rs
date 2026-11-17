//! Blocs de texte d'origine d'une page (E-05) : objets texte droits
//! regroupés par ligne (même police, corps et couleur, côte à côte). Le
//! regroupement est une fonction pure, testée sans PDFium.

use pde_core::edit::{Color, FONT_SIZE_RANGE, StandardFont, TextStyle};
use pde_core::engine::{EngineError, PageTextBlock};
use pde_core::geometry::Rect;
use pdfium_render::prelude::{
    PdfDocument, PdfFontWeight, PdfPageObject, PdfPageObjectCommon, PdfPageObjectsCommon,
};

use crate::convert::{internal, rect};
use crate::display::DisplayTransform;
use pde_core::document::PageIndex;

/// Objet texte lu sur la page.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Run {
    pub object: u32,
    pub bounds: Rect,
    pub text: String,
    /// Nom complet de la police (préfixe de sous-ensemble compris).
    pub font: String,
    pub style: TextStyle,
}

/// Blocs de texte de la page arrangée `page`. Une page tournée n'en
/// propose pas encore (le texte récrit s'écrirait de travers).
pub(crate) fn page_text_blocks(
    doc: &PdfDocument,
    page: PageIndex,
) -> Result<Vec<PageTextBlock>, EngineError> {
    let (pdf_page, display) = super::page_of(doc, page)?;
    if !display.is_upright() {
        return Ok(Vec::new());
    }
    let mut runs = Vec::new();
    for (index, object) in pdf_page.objects().iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| internal_msg("trop d'objets"))?;
        if let Some(run) = run(&object, index, &display)? {
            runs.push(run);
        }
    }
    Ok(group(runs))
}

fn internal_msg(message: &str) -> EngineError {
    EngineError::Internal(message.into())
}

/// L'objet texte, s'il est droit et visible (texte non vide).
pub(super) fn run(
    object: &PdfPageObject,
    index: u32,
    display: &DisplayTransform,
) -> Result<Option<Run>, EngineError> {
    let Some(text_object) = object.as_text_object() else {
        return Ok(None);
    };
    let text = text_object.text();
    if text.trim().is_empty() || text.contains(['\n', '\r']) {
        return Ok(None);
    }
    let matrix = text_object.matrix().map_err(internal)?;
    let (a, b, c, d) = (matrix.a(), matrix.b(), matrix.c(), matrix.d());
    let straight = a > 0.0 && d > 0.0 && b.abs() <= a * 1e-3 && c.abs() <= d * 1e-3;
    if !straight || !text_object.is_visible() {
        return Ok(None);
    }
    let font = text_object.font();
    let name = font.name();
    let size = f64::from(text_object.scaled_font_size().value);
    let color = object.fill_color().map_or(Color::INK, |c| Color {
        r: c.red(),
        g: c.green(),
        b: c.blue(),
    });
    let lower = name.to_lowercase();
    let weight_bold = font.weight().is_ok_and(|w| {
        matches!(
            w,
            PdfFontWeight::Weight700Bold | PdfFontWeight::Weight800 | PdfFontWeight::Weight900
        )
    });
    let style = TextStyle {
        font: family(&lower, font.is_fixed_pitch(), font.is_serif()),
        size: size.clamp(*FONT_SIZE_RANGE.start(), *FONT_SIZE_RANGE.end()),
        bold: weight_bold || font.is_bold_reenforced() || lower.contains("bold"),
        italic: font.is_italic() || lower.contains("italic") || lower.contains("oblique"),
        color,
        ..TextStyle::default()
    };
    Ok(Some(Run {
        object: index,
        bounds: display.rect(rect(object.bounds().map_err(internal)?.to_rect())),
        text,
        font: name,
        style,
    }))
}

/// Police standard la plus proche, d'après le nom puis les indications de
/// la police.
pub(super) fn family(lower_name: &str, fixed: bool, serif: bool) -> StandardFont {
    const MONO: [&str; 3] = ["courier", "mono", "consol"];
    const SERIF: [&str; 5] = ["times", "serif", "georgia", "garamond", "roman"];
    if MONO.iter().any(|m| lower_name.contains(m)) || fixed {
        StandardFont::Mono
    } else if lower_name.contains("sans") {
        StandardFont::Sans
    } else if SERIF.iter().any(|s| lower_name.contains(s)) || serif {
        StandardFont::Serif
    } else {
        StandardFont::Sans
    }
}

/// Nom de la police sans le préfixe de sous-ensemble (« ABCDEF+Calibri »).
pub(super) fn display_name(name: &str) -> String {
    match name.split_once('+') {
        Some((prefix, rest))
            if prefix.len() == 6 && prefix.chars().all(|c| c.is_ascii_uppercase()) =>
        {
            rest.to_string()
        }
        _ => name.to_string(),
    }
}

/// Les objets texte, dans l'ordre de la page, regroupés par ligne.
pub(super) fn group(runs: Vec<Run>) -> Vec<PageTextBlock> {
    let mut blocks: Vec<(PageTextBlock, Run)> = Vec::new();
    for run in runs {
        if let Some((block, last)) = blocks.last_mut()
            && joins(last, &run)
        {
            let gap = run.bounds.min().x - last.bounds.max().x;
            if gap > 0.2 * last.style.size
                && !block.text.ends_with(' ')
                && !run.text.starts_with(' ')
            {
                block.text.push(' ');
            }
            block.text.push_str(&run.text);
            block.objects.push(run.object);
            block.bounds = block.bounds.union(&run.bounds);
            *last = run;
            continue;
        }
        let block = PageTextBlock {
            objects: vec![run.object],
            bounds: run.bounds,
            text: run.text.clone(),
            font_name: display_name(&run.font),
            style: run.style.clone(),
        };
        blocks.push((block, run));
    }
    blocks
        .into_iter()
        .map(|(mut block, _)| {
            block.text = block.text.trim().to_string();
            block
        })
        .collect()
}

/// `next` prolonge la ligne de `last` : même police, corps et couleur,
/// même hauteur, et juste à sa droite.
fn joins(last: &Run, next: &Run) -> bool {
    let size = last.style.size;
    let same_look = last.font == next.font
        && (size - next.style.size).abs() < 0.5
        && last.style.color == next.style.color;
    let overlap =
        last.bounds.max().y.min(next.bounds.max().y) - last.bounds.min().y.max(next.bounds.min().y);
    let same_line = overlap >= 0.5 * last.bounds.height().min(next.bounds.height());
    let gap = next.bounds.min().x - last.bounds.max().x;
    same_look && same_line && gap > -0.5 * size && gap < 1.5 * size
}

#[cfg(test)]
mod tests {
    use super::*;
    use pde_core::geometry::Point;

    fn run(object: u32, x: f64, y: f64, width: f64, text: &str) -> Run {
        Run {
            object,
            bounds: Rect::from_corners(Point::new(x, y), Point::new(x + width, y + 10.0)),
            text: text.into(),
            font: "ABCDEF+Calibri".into(),
            style: TextStyle {
                size: 12.0,
                ..TextStyle::default()
            },
        }
    }

    #[test]
    fn words_of_a_line_make_one_block_other_lines_another() {
        let blocks = group(vec![
            run(1, 72.0, 700.0, 40.0, "Bonjour"),
            // Espace absent du texte : ajouté d'après l'écart.
            run(3, 116.0, 700.0, 30.0, "tout"),
            run(4, 146.5, 700.0, 30.0, "efois"),
            // Ligne suivante.
            run(5, 72.0, 680.0, 50.0, "Seconde"),
        ]);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].objects, [1, 3, 4]);
        assert_eq!(blocks[0].text, "Bonjour toutefois");
        assert_eq!(blocks[0].font_name, "Calibri");
        assert_eq!(blocks[0].bounds.max().x, 176.5);
        assert_eq!(blocks[1].objects, [5]);
    }

    #[test]
    fn a_different_size_color_or_a_far_word_starts_a_new_block() {
        let mut bigger = run(2, 113.0, 700.0, 30.0, "Titre");
        bigger.style.size = 18.0;
        let mut red = run(3, 144.0, 700.0, 30.0, "rouge");
        red.style.color = Color { r: 255, g: 0, b: 0 };
        let far = run(4, 400.0, 700.0, 30.0, "loin");
        let blocks = group(vec![run(1, 72.0, 700.0, 40.0, "Un"), bigger, red, far]);
        assert_eq!(blocks.len(), 4);
    }

    #[test]
    fn closest_standard_font_and_name_without_subset_prefix() {
        assert_eq!(
            family("abcdef+timesnewroman", false, false),
            StandardFont::Serif
        );
        assert_eq!(family("dejavusans", false, true), StandardFont::Sans);
        assert_eq!(family("consolas", false, false), StandardFont::Mono);
        assert_eq!(family("calibri", false, false), StandardFont::Sans);
        assert_eq!(family("minion", false, true), StandardFont::Serif);
        assert_eq!(display_name("ABCDEF+Calibri-Bold"), "Calibri-Bold");
        assert_eq!(display_name("Helvetica"), "Helvetica");
        assert_eq!(display_name("Ab+C"), "Ab+C");
    }
}
