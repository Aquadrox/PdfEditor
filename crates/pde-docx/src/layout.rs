//! Mise en page simplifiée d'un document Word lu par `parse.rs` : chaque
//! paragraphe devient une zone de texte de l'éditeur (style dominant de
//! ses passages), chaque cellule de tableau une zone dans son cadre ; les
//! lignes sont coupées avec les largeurs d'Helvetica et les pages remplies
//! de haut en bas. Le résultat est posé par le moteur sur des pages
//! blanches comme n'importe quels ajouts.

use pde_core::document::{PageIndex, PageSize};
use pde_core::edit::{
    Color, Element, MAX_TEXT_CHARS, ShapeBox, ShapeKind, StandardFont, TextBox, TextStyle, wrap,
};
use pde_core::geometry::Point;
use pde_core::helvetica::text_width;

use crate::parse::{Block, Docx, Paragraph, Table};

/// Interligne des zones de texte (multiple du corps).
const LINE_HEIGHT: f64 = 1.2;
/// Corps par défaut (Word : 11 pt).
const BODY_SIZE: f64 = 11.0;
/// Marge intérieure d'une cellule de tableau.
const CELL_PADDING: f64 = 3.0;
const GRID: Color = Color {
    r: 0x80,
    g: 0x80,
    b: 0x80,
};

/// Document mis en page : taille et nombre de pages, éléments à poser.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub page_size: PageSize,
    pub pages: usize,
    pub elements: Vec<Element>,
}

/// Style d'un paragraphe : corps, gras et italique de la majorité de son texte.
fn style_of(p: &Paragraph) -> TextStyle {
    let total: usize = p
        .runs
        .iter()
        .map(|r| r.text.chars().count())
        .sum::<usize>()
        .max(1);
    let share = |f: fn(&crate::parse::Run) -> bool| {
        p.runs
            .iter()
            .filter(|r| f(r))
            .map(|r| r.text.chars().count())
            .sum::<usize>()
            * 2
            > total
    };
    let size = p
        .runs
        .iter()
        .max_by_key(|r| r.text.chars().count())
        .and_then(|r| r.size)
        .unwrap_or(match p.heading {
            Some(0) => 22.0,
            Some(1) => 16.0,
            Some(2) => 13.0,
            Some(_) => 12.0,
            None => BODY_SIZE,
        });
    TextStyle {
        font: StandardFont::Sans,
        size: size.clamp(4.0, 72.0),
        bold: p.heading.is_some() || share(|r| r.bold),
        italic: share(|r| r.italic),
        color: Color::INK,
        align: p.align,
        line_height: LINE_HEIGHT,
    }
}

/// Lignes du texte à la largeur `width` (le gras est un peu plus large).
fn lines(text: &str, width: f64, style: &TextStyle) -> Vec<String> {
    let factor = if style.bold { 1.06 } else { 1.0 };
    wrap(text, width.max(10.0), |s| {
        text_width(s, style.size) * factor
    })
}

struct Pager {
    size: PageSize,
    top: f64,
    bottom: f64,
    left: f64,
    width: f64,
    page: PageIndex,
    /// Haut de la place libre (origine en bas).
    y: f64,
    elements: Vec<Element>,
}

impl Pager {
    fn new_page(&mut self) {
        self.page += 1;
        self.y = self.size.height - self.top;
    }

    fn at_top(&self) -> bool {
        (self.y - (self.size.height - self.top)).abs() < 0.01
    }

    /// Fait de la place pour `height` points, sur une nouvelle page s'il le faut.
    fn room(&mut self, height: f64) {
        if self.y - height < self.bottom && !self.at_top() {
            self.new_page();
        }
    }

    fn text(&mut self, x: f64, width: f64, text: String, style: &TextStyle) {
        let text: String = text.chars().take(MAX_TEXT_CHARS).collect();
        self.elements.push(Element::Text(TextBox {
            page: self.page,
            top_left: Point::new(x, self.y),
            width,
            text,
            style: style.clone(),
        }));
    }

    fn paragraph(&mut self, p: &Paragraph) {
        let style = style_of(p);
        let leading = style.size * LINE_HEIGHT;
        let before = p
            .before
            .unwrap_or(if p.heading.is_some() { 12.0 } else { 0.0 });
        let after = p
            .after
            .unwrap_or(if p.heading.is_some() { 6.0 } else { 8.0 });
        let indent = p.indent.min(self.width / 2.0) + if p.bullet { 18.0 } else { 0.0 };
        let mut text = p.text();
        if p.bullet {
            text = format!("• {text}");
        }
        if !self.at_top() {
            self.y -= before;
        }
        let width = self.width - indent;
        let all = lines(&text, width, &style);
        if text.trim().is_empty() {
            self.room(leading);
            self.y -= leading + after;
            return;
        }
        // Paragraphe coupé entre les pages, ligne à ligne.
        let mut rest: &[String] = &all;
        while !rest.is_empty() {
            self.room(leading);
            let fit = (((self.y - self.bottom) / leading).floor() as usize).clamp(1, rest.len());
            let (chunk, next) = rest.split_at(fit);
            self.text(self.left + indent, width, chunk.join("\n"), &style);
            self.y -= leading * chunk.len() as f64;
            rest = next;
        }
        self.y -= after;
    }

    fn table(&mut self, table: &Table) {
        let count = table
            .rows
            .iter()
            .map(|r| r.iter().map(|c| c.span).sum::<usize>())
            .max()
            .unwrap_or(0)
            .max(table.columns.len());
        if count == 0 {
            return;
        }
        let mut columns = table.columns.clone();
        columns.resize(count, self.width / count as f64);
        let total: f64 = columns.iter().sum();
        if total > self.width || total <= 0.0 {
            let scale = self.width / total.max(1.0);
            columns.iter_mut().for_each(|c| *c *= scale);
        }
        let style = TextStyle {
            size: 10.0,
            line_height: LINE_HEIGHT,
            ..TextStyle::default()
        };
        let leading = style.size * LINE_HEIGHT;
        for row in &table.rows {
            let mut cells = Vec::new();
            let mut column = 0;
            for cell in row {
                let x = self.left + columns[..column.min(count)].iter().sum::<f64>();
                let end = (column + cell.span).min(count);
                let width = columns[column.min(count)..end].iter().sum::<f64>();
                column = end;
                let text = cell
                    .paragraphs
                    .iter()
                    .map(|p| {
                        if p.bullet {
                            format!("• {}", p.text())
                        } else {
                            p.text()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let n = lines(&text, width - 2.0 * CELL_PADDING, &style)
                    .len()
                    .max(1);
                cells.push((x, width, text, n));
            }
            let height = cells
                .iter()
                .map(|c| c.3 as f64 * leading + 2.0 * CELL_PADDING)
                .fold(18.0, f64::max);
            self.room(height);
            for (x, width, text, _) in cells {
                if width < 2.0 * CELL_PADDING + 8.0 {
                    continue;
                }
                self.elements.push(Element::Shape(ShapeBox {
                    page: self.page,
                    shape: ShapeKind::Rectangle,
                    start: Point::new(x, self.y),
                    end: Point::new(x + width, self.y - height),
                    color: GRID,
                    width: 0.5,
                    filled: false,
                }));
                if !text.trim().is_empty() {
                    let y = self.y;
                    self.y -= CELL_PADDING;
                    self.text(x + CELL_PADDING, width - 2.0 * CELL_PADDING, text, &style);
                    self.y = y;
                }
            }
            self.y -= height;
        }
        self.y -= 8.0;
    }
}

/// Pages et éléments du document.
pub fn layout(doc: &Docx) -> Layout {
    let setup = doc.page;
    let width = setup.width.clamp(72.0, 14_400.0);
    let height = setup.height.clamp(72.0, 14_400.0);
    let left = setup.left.min(width / 3.0);
    let right = setup.right.min(width / 3.0);
    let top = setup.top.min(height / 3.0);
    let bottom = setup.bottom.min(height / 3.0);
    let mut pager = Pager {
        size: PageSize::new(width, height),
        top,
        bottom,
        left,
        width: width - left - right,
        page: 0,
        y: height - top,
        elements: Vec::new(),
    };
    for block in &doc.blocks {
        match block {
            Block::Paragraph(p) => pager.paragraph(p),
            Block::Table(t) => pager.table(t),
            Block::PageBreak => pager.new_page(),
        }
    }
    Layout {
        page_size: pager.size,
        pages: pager.page + 1,
        elements: pager.elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{Cell, PageSetup, Run};
    use pde_core::edit::Align;

    fn para(text: &str) -> Block {
        Block::Paragraph(Paragraph {
            runs: vec![Run {
                text: text.into(),
                ..Run::default()
            }],
            ..Paragraph::default()
        })
    }

    #[test]
    fn paragraphs_fill_pages_from_the_top() {
        let doc = Docx {
            page: PageSetup::default(),
            blocks: vec![para("Bonjour"), Block::PageBreak, para("Page deux")],
            skipped_images: 0,
        };
        let out = layout(&doc);
        assert_eq!(out.pages, 2);
        let Element::Text(first) = &out.elements[0] else {
            panic!();
        };
        assert_eq!(first.page, 0);
        assert!((first.top_left.y - (841.89 - 70.87)).abs() < 1e-6);
        assert_eq!(out.elements[1].page(), 1);
        for element in &out.elements {
            assert_eq!(element.validate(out.pages, None), Ok(()), "{element:?}");
        }
    }

    #[test]
    fn long_text_continues_on_the_next_page() {
        let long = "mot ".repeat(4000);
        let doc = Docx {
            page: PageSetup::default(),
            blocks: vec![para(&long)],
            skipped_images: 0,
        };
        let out = layout(&doc);
        assert!(out.pages >= 3, "{}", out.pages);
        assert!(
            out.elements
                .iter()
                .all(|e| e.validate(out.pages, None).is_ok())
        );
    }

    #[test]
    fn tables_get_cells_with_borders_and_text() {
        let cell = |text: &str, span| Cell {
            span,
            paragraphs: vec![Paragraph {
                runs: vec![Run {
                    text: text.into(),
                    ..Run::default()
                }],
                ..Paragraph::default()
            }],
        };
        let table = Table {
            columns: vec![1000.0, 1000.0],
            rows: vec![
                vec![cell("Date", 1), cell("", 1)],
                vec![cell("Signature", 2)],
            ],
        };
        let doc = Docx {
            page: PageSetup::default(),
            blocks: vec![Block::Table(table)],
            skipped_images: 0,
        };
        let out = layout(&doc);
        let shapes = out
            .elements
            .iter()
            .filter(|e| matches!(e, Element::Shape(_)))
            .count();
        let texts = out
            .elements
            .iter()
            .filter(|e| matches!(e, Element::Text(_)))
            .count();
        assert_eq!((shapes, texts), (3, 2));
        // Colonnes ramenées à la largeur utile.
        let Element::Shape(signature) = &out.elements[3] else {
            panic!("{:?}", out.elements);
        };
        assert!((signature.end.x - signature.start.x - (595.28 - 2.0 * 70.87)).abs() < 1e-6);
    }

    #[test]
    fn styles_follow_the_majority_of_the_text() {
        let p = Paragraph {
            runs: vec![
                Run {
                    text: "Gras long".into(),
                    bold: true,
                    size: Some(14.0),
                    ..Run::default()
                },
                Run {
                    text: "x".into(),
                    ..Run::default()
                },
            ],
            align: Align::Center,
            ..Paragraph::default()
        };
        let style = style_of(&p);
        assert!(style.bold && !style.italic);
        assert_eq!((style.size, style.align), (14.0, Align::Center));
    }
}
