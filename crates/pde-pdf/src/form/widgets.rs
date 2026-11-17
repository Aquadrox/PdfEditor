//! Champs du formulaire placés sur les pages du document arrangé : PDFium
//! donne chaque emplacement (page, place, rang du bouton radio), `lopdf`
//! la structure du champ (genre, options, valeur d'origine).

use std::collections::BTreeMap;

use lopdf::Document;
use pde_core::edit::{FieldKind, FormField, Widget};
use pde_core::engine::EngineError;
use pdfium_render::prelude::{PdfDocument, PdfFormFieldCommon, PdfPageAnnotationCommon};

use super::structure::{FieldNode, NodeKind, fields};
use crate::convert::{internal, rect};
use crate::display::DisplayTransform;

/// Champs à remplir (ni boutons poussoirs ni signatures) ; vide si le
/// document n'a pas de formulaire ou s'il est protégé par mot de passe.
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
    let mut widgets: BTreeMap<String, Vec<Widget>> = BTreeMap::new();
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
            let bounds = display.rect(rect(annotation.bounds().map_err(internal)?));
            widgets.entry(name).or_default().push(Widget {
                page: index,
                bounds,
                option,
            });
        }
    }
    Ok(nodes
        .into_iter()
        .filter_map(|node| {
            let placed = widgets.remove(&node.name)?;
            Some(to_field(node, placed))
        })
        .collect())
}

fn to_field(node: FieldNode, widgets: Vec<Widget>) -> FormField {
    let kind = match node.kind {
        NodeKind::Text { multiline, max_len } => FieldKind::Text { multiline, max_len },
        NodeKind::Checkbox => FieldKind::Checkbox,
        NodeKind::Radio => FieldKind::Radio {
            options: node
                .on_states
                .iter()
                .map(|s| s.clone().unwrap_or_default())
                .collect(),
        },
        NodeKind::Choice { options, editable } => FieldKind::Choice { options, editable },
    };
    FormField {
        name: node.name,
        kind,
        read_only: node.read_only,
        value: node.value,
        widgets,
    }
}
