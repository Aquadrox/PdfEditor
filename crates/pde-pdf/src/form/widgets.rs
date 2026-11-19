//! Champs du formulaire placés sur les pages du document arrangé : PDFium
//! donne chaque emplacement (page, place, rang du bouton radio), `lopdf`
//! la structure du champ (genre, options, valeur d'origine, réglages).
//! Un emplacement PDFium est rattaché au sien dans la structure par son
//! rectangle, ce qui donne son rang parmi les emplacements du champ.

use std::collections::BTreeMap;

use lopdf::Document;
use pde_core::edit::{FieldKind, FormField, Widget};
use pde_core::engine::EngineError;
use pde_core::formats::read_actions;
use pde_core::geometry::Rect;
use pdfium_render::prelude::{PdfDocument, PdfFormFieldCommon, PdfPageAnnotationCommon};

use super::structure::{FieldNode, NodeKind, READ_ONLY, REQUIRED, fields};
use crate::convert::{internal, rect};
use crate::display::DisplayTransform;

/// Emplacement vu par PDFium : page, place dans la page PDF et affichée.
struct Seen {
    page: usize,
    pdf: Rect,
    display: Rect,
    option: Option<usize>,
}

/// Champs à remplir (ni boutons poussoirs) ; vide si le document n'a pas
/// de formulaire ou s'il est protégé par mot de passe.
pub(crate) fn form_fields(
    arranged: &PdfDocument,
    source: &[u8],
) -> Result<Vec<FormField>, EngineError> {
    let Ok(structure) = Document::load_mem(source) else {
        return Ok(Vec::new());
    };
    if structure.trailer.has(b"Encrypt") {
        return Ok(Vec::new());
    }
    let nodes = fields(&structure);
    if nodes.is_empty() {
        return Ok(Vec::new());
    }
    let mut seen: BTreeMap<String, Vec<Seen>> = BTreeMap::new();
    for (index, page) in arranged.pages().iter().enumerate() {
        let display = DisplayTransform::for_page(&page)?;
        for annotation in page.annotations().iter() {
            let Some(field) = annotation.as_form_field() else {
                continue;
            };
            let Some(name) = field.name() else {
                continue;
            };
            let option = field
                .as_radio_button_field()
                .map(|radio| radio.index_in_group() as usize);
            let pdf = rect(annotation.bounds().map_err(internal)?);
            seen.entry(name).or_default().push(Seen {
                page: index,
                pdf,
                display: display.rect(pdf),
                option,
            });
        }
    }
    Ok(nodes
        .into_iter()
        .filter_map(|node| {
            let placed = seen.remove(&node.name)?;
            Some(to_field(node, placed))
        })
        .collect())
}

/// Rang de l'emplacement de la structure au même rectangle, parmi ceux
/// pas encore pris ; à défaut, le premier libre.
fn widget_index(node: &FieldNode, pdf: Rect, taken: &[bool]) -> usize {
    let near = |r: &[f64; 4]| {
        (r[0] - pdf.min().x).abs() < 0.5
            && (r[1] - pdf.min().y).abs() < 0.5
            && (r[2] - pdf.max().x).abs() < 0.5
            && (r[3] - pdf.max().y).abs() < 0.5
    };
    let free = |i: &usize| !taken.get(*i).copied().unwrap_or(true);
    (0..node.rects.len())
        .filter(free)
        .find(|i| node.rects[*i].as_ref().is_some_and(near))
        .or_else(|| (0..node.rects.len()).find(free))
        .unwrap_or(node.rects.len())
}

fn to_field(node: FieldNode, placed: Vec<Seen>) -> FormField {
    let mut taken = vec![false; node.rects.len()];
    let widgets = placed
        .into_iter()
        .map(|seen| {
            let index = widget_index(&node, seen.pdf, &taken);
            if let Some(slot) = taken.get_mut(index) {
                *slot = true;
            }
            let mut widget = Widget::new(seen.page, seen.display, index);
            widget.option = seen.option;
            widget.export = node.on_states.get(index).cloned().flatten();
            widget
        })
        .collect();
    let (format, calc) = read_actions(&node.actions);
    let kind = match node.kind {
        NodeKind::Text {
            multiline,
            max_len,
            password,
            comb,
        } => FieldKind::Text {
            multiline,
            max_len,
            password,
            comb,
            format,
            calc,
        },
        NodeKind::Checkbox => FieldKind::Checkbox,
        NodeKind::Radio => FieldKind::Radio {
            options: node
                .on_states
                .iter()
                .map(|s| s.clone().unwrap_or_default())
                .collect(),
        },
        NodeKind::Choice {
            options,
            editable,
            list,
            multi,
        } => FieldKind::Choice {
            options,
            editable,
            list,
            multi,
        },
        NodeKind::Signature => FieldKind::Signature,
    };
    FormField {
        name: node.name,
        kind,
        read_only: node.flags & READ_ONLY != 0,
        required: node.flags & REQUIRED != 0,
        tooltip: node.tooltip,
        look: node.look,
        value: node.value,
        widgets,
    }
}
