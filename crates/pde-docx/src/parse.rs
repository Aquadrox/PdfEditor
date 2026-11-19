//! Contenu d'un document Word (`word/document.xml`) : paragraphes et leurs
//! passages (gras, italique, corps), titres, listes, alignement,
//! espacements, tableaux, sauts de page, format de la page. Ce qui n'est
//! pas lu (images, zones de texte, en-têtes) est compté pour être signalé.

use pde_core::edit::Align;

use crate::xml::{Event, events};

/// Points par twip (1/20 de point, l'unité de Word).
const TWIP: f64 = 1.0 / 20.0;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    /// Corps en points, s'il est donné.
    pub size: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Paragraph {
    pub runs: Vec<Run>,
    pub align: Align,
    /// Niveau de titre (0 : titre du document, 1 à 6 : titres).
    pub heading: Option<u8>,
    /// Élément d'une liste.
    pub bullet: bool,
    /// Espace avant et après (points).
    pub before: Option<f64>,
    pub after: Option<f64>,
    /// Retrait à gauche (points).
    pub indent: f64,
    pub page_break_before: bool,
}

impl Paragraph {
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell {
    /// Colonnes couvertes.
    pub span: usize,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    /// Largeur de chaque colonne (points).
    pub columns: Vec<f64>,
    pub rows: Vec<Vec<Cell>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    PageBreak,
}

/// Format et marges de la page (points).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSetup {
    pub width: f64,
    pub height: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for PageSetup {
    /// A4, marges de 2,5 cm (réglages par défaut de Word en France).
    fn default() -> Self {
        Self {
            width: 595.28,
            height: 841.89,
            top: 70.87,
            right: 70.87,
            bottom: 70.87,
            left: 70.87,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Docx {
    pub page: PageSetup,
    pub blocks: Vec<Block>,
    /// Images et dessins non repris.
    pub skipped_images: usize,
}

fn number(event: &Event, key: &str) -> Option<f64> {
    event
        .attr(key)?
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
}

/// Propriété binaire (`<w:b/>`, `<w:b w:val="0"/>`).
fn on(event: &Event) -> bool {
    !matches!(event.attr("w:val"), Some("0" | "false" | "off" | "none"))
}

fn heading_level(style: &str) -> Option<u8> {
    let s = style.to_lowercase();
    if s == "title" || s == "titre" {
        return Some(0);
    }
    for prefix in ["heading", "titre"] {
        if let Some(level) = s.strip_prefix(prefix).and_then(|l| l.parse::<u8>().ok()) {
            return Some(level.clamp(1, 6));
        }
    }
    None
}

struct Reader<'a> {
    events: &'a [Event],
    at: usize,
    doc: Docx,
}

/// Nom d'une balise ouvrante.
fn start_name(event: &Event) -> Option<&str> {
    match event {
        Event::Start { name, .. } => Some(name),
        _ => None,
    }
}

impl Reader<'_> {
    /// Fin de l'élément ouvert juste avant `self.at` (balise `name`).
    fn skip(&mut self, name: &str) {
        let mut depth = 1;
        while let Some(event) = self.events.get(self.at) {
            self.at += 1;
            match event {
                Event::Start {
                    name: n,
                    empty: false,
                    ..
                } if n == name => depth += 1,
                Event::End(n) if n == name => {
                    depth -= 1;
                    if depth == 0 {
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    fn page_setup(&mut self, event: &Event) {
        let page = &mut self.doc.page;
        if start_name(event) == Some("w:pgSz") {
            page.width = number(event, "w:w").map_or(page.width, |v| v * TWIP);
            page.height = number(event, "w:h").map_or(page.height, |v| v * TWIP);
            return;
        }
        for (key, side) in [
            ("w:top", &mut page.top),
            ("w:right", &mut page.right),
            ("w:bottom", &mut page.bottom),
            ("w:left", &mut page.left),
        ] {
            if let Some(v) = number(event, key) {
                *side = (v * TWIP).abs();
            }
        }
    }

    fn body(&mut self) {
        while let Some(event) = self.events.get(self.at).cloned() {
            self.at += 1;
            match start_name(&event) {
                Some("w:p") if !is_empty(&event) => {
                    let paragraphs = self.paragraph();
                    self.push_paragraphs(paragraphs);
                }
                Some("w:tbl") => {
                    let table = self.table();
                    self.doc.blocks.push(Block::Table(table));
                }
                Some("w:pgSz" | "w:pgMar") => self.page_setup(&event),
                _ => {}
            }
        }
    }

    /// Un paragraphe, coupé en plusieurs aux sauts de page forcés.
    fn push_paragraphs(&mut self, paragraphs: Vec<(Paragraph, bool)>) {
        for (paragraph, break_after) in paragraphs {
            if paragraph.page_break_before {
                self.doc.blocks.push(Block::PageBreak);
            }
            self.doc.blocks.push(Block::Paragraph(paragraph));
            if break_after {
                self.doc.blocks.push(Block::PageBreak);
            }
        }
    }

    /// Le paragraphe ouvert (`<w:p>` lu) : ses morceaux, et pour chacun
    /// s'il est suivi d'un saut de page.
    fn paragraph(&mut self) -> Vec<(Paragraph, bool)> {
        let mut out = Vec::new();
        let mut p = Paragraph::default();
        let mut run = Run::default();
        let mut in_text = false;
        while let Some(event) = self.events.get(self.at).cloned() {
            self.at += 1;
            match &event {
                Event::End(name) if name == "w:p" => break,
                Event::End(name) if name == "w:t" => in_text = false,
                Event::End(name) if name == "w:r" => {
                    if !run.text.is_empty() {
                        p.runs.push(std::mem::take(&mut run));
                    }
                    run = Run::default();
                }
                Event::Text(text) if in_text => run.text.push_str(text),
                Event::Start { name, empty, .. } => match name.as_str() {
                    "w:t" if !empty => in_text = true,
                    "w:pStyle" => p.heading = event.attr("w:val").and_then(heading_level),
                    "w:jc" => {
                        p.align = match event.attr("w:val") {
                            Some("center") => Align::Center,
                            Some("right" | "end") => Align::Right,
                            _ => Align::Left,
                        }
                    }
                    "w:numPr" => p.bullet = true,
                    "w:spacing" => {
                        p.before = number(&event, "w:before").map(|v| v * TWIP).or(p.before);
                        p.after = number(&event, "w:after").map(|v| v * TWIP).or(p.after);
                    }
                    "w:ind" => {
                        p.indent = number(&event, "w:left")
                            .or_else(|| number(&event, "w:start"))
                            .map_or(p.indent, |v| (v * TWIP).max(0.0));
                    }
                    "w:pageBreakBefore" => p.page_break_before = on(&event),
                    "w:b" => run.bold = on(&event),
                    "w:i" => run.italic = on(&event),
                    "w:sz" => run.size = number(&event, "w:val").map(|v| v / 2.0),
                    "w:tab" => run.text.push_str("    "),
                    "w:br" if event.attr("w:type") == Some("page") => {
                        if !run.text.is_empty() {
                            p.runs.push(std::mem::take(&mut run));
                        }
                        let style = Paragraph {
                            runs: Vec::new(),
                            ..p.clone()
                        };
                        out.push((std::mem::replace(&mut p, style), true));
                    }
                    "w:br" | "w:cr" => run.text.push('\n'),
                    "w:drawing" | "w:pict" | "w:object" => {
                        self.doc.skipped_images += 1;
                        if !empty {
                            self.skip(name);
                        }
                    }
                    // Texte supprimé (suivi des modifications), codes de champ : sautés.
                    "w:del" | "w:instrText" | "mc:Fallback" if !empty => self.skip(name),
                    // Section terminée par ce paragraphe : son format de page.
                    "w:pgSz" | "w:pgMar" => self.page_setup(&event),
                    _ => {}
                },
                _ => {}
            }
        }
        if !run.text.is_empty() {
            p.runs.push(run);
        }
        out.push((p, false));
        out
    }

    fn table(&mut self) -> Table {
        let mut table = Table::default();
        while let Some(event) = self.events.get(self.at).cloned() {
            self.at += 1;
            match &event {
                Event::End(name) if name == "w:tbl" => break,
                Event::Start { name, .. } => match name.as_str() {
                    "w:gridCol" => table
                        .columns
                        .push(number(&event, "w:w").map_or(72.0, |v| v * TWIP)),
                    "w:tr" => table.rows.push(Vec::new()),
                    "w:tc" => {
                        if table.rows.is_empty() {
                            table.rows.push(Vec::new());
                        }
                        let cell = self.cell();
                        if let Some(row) = table.rows.last_mut() {
                            row.push(cell);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        table
    }

    fn cell(&mut self) -> Cell {
        let mut cell = Cell {
            span: 1,
            paragraphs: Vec::new(),
        };
        while let Some(event) = self.events.get(self.at).cloned() {
            self.at += 1;
            match start_name(&event) {
                Some("w:gridSpan") => {
                    cell.span = number(&event, "w:val").map_or(1, |v| v.clamp(1.0, 64.0) as usize)
                }
                Some("w:p") if !is_empty(&event) => {
                    cell.paragraphs
                        .extend(self.paragraph().into_iter().map(|(p, _)| p));
                }
                // Tableau dans une cellule : son texte, paragraphe par paragraphe.
                Some("w:tbl") => {
                    let inner = self.table();
                    for row in inner.rows {
                        for c in row {
                            cell.paragraphs.extend(c.paragraphs);
                        }
                    }
                }
                _ => {}
            }
            if matches!(&event, Event::End(name) if name == "w:tc") {
                break;
            }
        }
        cell
    }
}

fn is_empty(event: &Event) -> bool {
    matches!(event, Event::Start { empty: true, .. })
}

/// Contenu de `word/document.xml`.
pub fn parse(document_xml: &str) -> Docx {
    let events = events(document_xml);
    let mut reader = Reader {
        events: &events,
        at: 0,
        doc: Docx::default(),
    };
    reader.body();
    reader.doc
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"<w:document><w:body>
<w:p><w:pPr><w:pStyle w:val="Titre1"/><w:jc w:val="center"/></w:pPr><w:r><w:t>Contrat</w:t></w:r></w:p>
<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">Nom : </w:t></w:r><w:r><w:t>________</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr><w:spacing w:after="120"/></w:pPr><w:r><w:t>Point</w:t><w:br w:type="page"/><w:t>Suite</w:t></w:r></w:p>
<w:p><w:r><w:drawing><wp:inline><w:t>caché</w:t></wp:inline></w:drawing></w:r></w:p>
<w:tbl><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="4000"/></w:tblGrid>
<w:tr><w:tc><w:p><w:r><w:t>Date</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>Signature</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1080" w:bottom="1440" w:left="1080"/></w:sectPr>
</w:body></w:document>"#;

    #[test]
    fn paragraphs_tables_and_page_are_read() {
        let doc = parse(DOC);
        assert_eq!((doc.page.width, doc.page.height), (612.0, 792.0));
        assert_eq!((doc.page.top, doc.page.left), (72.0, 54.0));
        assert_eq!(doc.skipped_images, 1);
        let Block::Paragraph(title) = &doc.blocks[0] else {
            panic!("{:?}", doc.blocks[0]);
        };
        assert_eq!(
            (title.heading, title.align, title.text().as_str()),
            (Some(1), Align::Center, "Contrat")
        );
        let Block::Paragraph(name) = &doc.blocks[1] else {
            panic!();
        };
        assert!(name.runs[0].bold && !name.runs[1].bold);
        assert_eq!(name.text(), "Nom : ________");
        let Block::Paragraph(point) = &doc.blocks[2] else {
            panic!();
        };
        assert!(point.bullet);
        assert_eq!(point.after, Some(6.0));
        assert_eq!(point.text(), "Point");
        assert_eq!(doc.blocks[3], Block::PageBreak);
        let Block::Paragraph(next) = &doc.blocks[4] else {
            panic!();
        };
        assert_eq!(next.text(), "Suite");
        assert!(next.bullet);
        let Block::Table(table) = &doc.blocks[6] else {
            panic!("{:?}", doc.blocks);
        };
        assert_eq!(table.columns, vec![100.0, 200.0]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[1][0].span, 2);
        assert_eq!(table.rows[0][0].paragraphs[0].text(), "Date");
    }

    #[test]
    fn headings_and_switches() {
        assert_eq!(heading_level("Heading2"), Some(2));
        assert_eq!(heading_level("Title"), Some(0));
        assert_eq!(heading_level("Normal"), None);
        let doc = parse(
            r#"<w:body><w:p><w:r><w:rPr><w:b w:val="0"/><w:i/><w:sz w:val="28"/></w:rPr><w:t>x</w:t><w:tab/><w:t>y</w:t></w:r></w:p></w:body>"#,
        );
        let Block::Paragraph(p) = &doc.blocks[0] else {
            panic!();
        };
        assert_eq!(
            p.runs[0],
            Run {
                text: "x    y".into(),
                bold: false,
                italic: true,
                size: Some(14.0)
            }
        );
    }
}
