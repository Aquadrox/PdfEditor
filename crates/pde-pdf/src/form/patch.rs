//! Valeurs des champs et propriétés du document écrites avec `lopdf` sur
//! un contenu enregistré par PDFium, après la conception du formulaire
//! (`design.rs`). Les champs modifiés perdent leur ancienne apparence et le
//! formulaire demande aux lecteurs de les redessiner (`NeedAppearances`) :
//! PDFium le fait au rechargement. Un nombre est rangé sans mise en forme
//! (ce que lisent les scripts d'Acrobat), sauf si le formulaire est aplati :
//! le texte fondu dans la page est alors celui qu'afficherait un lecteur.

use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object, text_string};
use pde_core::edit::{FieldValue, Metadata};
use pde_core::engine::EngineError;
use pde_core::formats::read_actions;

use super::design::{Designed, design};
use super::structure::{FieldNode, NodeKind, fields};
use super::text_appearance::draw_missing;
use super::tree::{dict_mut, ensure_acroform, error};

/// Ce que `patch` écrit dans le formulaire et les propriétés.
pub(crate) struct FormPatch<'a> {
    pub values: &'a BTreeMap<String, FieldValue>,
    pub metadata: Option<&'a Metadata>,
    pub designed: &'a [Designed<'a>],
    pub flatten: bool,
}

/// Le contenu `bytes`, champs remplis et propriétés changées.
pub(crate) fn patch(bytes: Vec<u8>, changes: &FormPatch) -> Result<Vec<u8>, EngineError> {
    let FormPatch {
        values,
        metadata,
        designed,
        flatten,
    } = *changes;
    if values.is_empty() && metadata.is_none() && designed.is_empty() && !flatten {
        return Ok(bytes);
    }
    let mut doc = Document::load_mem(&bytes).map_err(|e| error(format!("lecture : {e}")))?;
    if doc.trailer.has(b"Encrypt") {
        return Err(error(
            "document protégé par mot de passe : formulaire et propriétés ne se modifient pas encore",
        ));
    }
    design(&mut doc, designed)?;
    if !values.is_empty() || !designed.is_empty() || flatten {
        let nodes = fields(&doc);
        for (name, value) in values {
            let node = nodes
                .iter()
                .find(|n| &n.name == name)
                .ok_or_else(|| error(format!("champ « {name} » introuvable")))?;
            fill(&mut doc, node, value, flatten)?;
        }
        if flatten {
            show_formatted(&mut doc, &nodes, values)?;
        }
        if !nodes.is_empty() {
            ensure_acroform(&mut doc)?;
            let filled = fields(&doc);
            draw_missing(&mut doc, &filled)?;
            need_appearances(&mut doc)?;
        }
    }
    if let Some(metadata) = metadata {
        set_info(&mut doc, metadata);
    }
    let mut out = Vec::with_capacity(bytes.len());
    doc.save_to(&mut out)
        .map_err(|e| error(format!("écriture : {e}")))?;
    Ok(out)
}

/// Texte affiché d'un champ mis en forme (nombre, pourcentage).
fn displayed(node: &FieldNode, text: &str) -> String {
    match read_actions(&node.actions).0 {
        Some(format) => format.display(text),
        None => text.to_string(),
    }
}

/// Formulaire aplati : les nombres des champs non remplis ici sont écrits
/// tels qu'affichés, comme ceux remplis par `fill`.
fn show_formatted(
    doc: &mut Document,
    nodes: &[FieldNode],
    values: &BTreeMap<String, FieldValue>,
) -> Result<(), EngineError> {
    for node in nodes.iter().filter(|n| !values.contains_key(&n.name)) {
        if let (NodeKind::Text { .. }, Some(FieldValue::Text(text))) = (&node.kind, &node.value) {
            let shown = displayed(node, text);
            if shown != *text {
                fill(doc, node, &FieldValue::Text(shown), false)?;
            }
        }
    }
    Ok(())
}

fn fill(
    doc: &mut Document,
    node: &FieldNode,
    value: &FieldValue,
    flatten: bool,
) -> Result<(), EngineError> {
    match (&node.kind, value) {
        (NodeKind::Text { .. }, FieldValue::Text(text))
        | (NodeKind::Choice { .. }, FieldValue::Choice(text)) => {
            let text = if flatten {
                displayed(node, text)
            } else {
                text.clone()
            };
            dict_mut(doc, node.id)?.set("V", text_string(&text));
            // Ancienne apparence retirée : elle montrerait l'ancienne valeur.
            for widget in &node.widgets {
                dict_mut(doc, *widget)?.remove(b"AP");
            }
        }
        (NodeKind::Checkbox, FieldValue::Checked(checked)) => {
            let on = node.on_states.iter().flatten().next().cloned();
            let state = match (checked, on) {
                (true, Some(on)) => on,
                (true, None) => "Yes".to_string(),
                (false, _) => "Off".to_string(),
            };
            dict_mut(doc, node.id)?.set("V", Object::Name(state.clone().into_bytes()));
            for widget in &node.widgets {
                dict_mut(doc, *widget)?.set("AS", Object::Name(state.clone().into_bytes()));
            }
        }
        (NodeKind::Radio, FieldValue::Choice(choice)) => {
            if !node.on_states.iter().flatten().any(|s| s == choice) {
                return Err(error(format!("option « {choice} » inconnue")));
            }
            dict_mut(doc, node.id)?.set("V", Object::Name(choice.clone().into_bytes()));
            for (widget, on) in node.widgets.iter().zip(&node.on_states) {
                let state = if on.as_deref() == Some(choice.as_str()) {
                    choice.as_str()
                } else {
                    "Off"
                };
                dict_mut(doc, *widget)?.set("AS", Object::Name(state.as_bytes().to_vec()));
            }
        }
        (NodeKind::Choice { options, .. }, FieldValue::Choices(items)) => {
            let field = dict_mut(doc, node.id)?;
            field.set(
                "V",
                Object::Array(items.iter().map(|i| text_string(i)).collect()),
            );
            // Rangs des options choisies (`/I`), croissants.
            let mut indices: Vec<i64> = items
                .iter()
                .filter_map(|i| options.iter().position(|o| o == i))
                .filter_map(|i| i64::try_from(i).ok())
                .collect();
            indices.sort_unstable();
            field.set(
                "I",
                Object::Array(indices.into_iter().map(Object::from).collect()),
            );
            for widget in &node.widgets {
                dict_mut(doc, *widget)?.remove(b"AP");
            }
        }
        _ => {
            return Err(error(format!(
                "valeur qui ne convient pas au champ « {} »",
                node.name
            )));
        }
    }
    Ok(())
}

/// `/NeedAppearances true` dans le dictionnaire `AcroForm` (direct ou non).
fn need_appearances(doc: &mut Document) -> Result<(), EngineError> {
    let root = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|e| error(format!("catalogue : {e}")))?;
    let form = dict_mut(doc, root)?
        .get(b"AcroForm")
        .cloned()
        .map_err(|e| error(format!("formulaire : {e}")))?;
    match form {
        Object::Reference(id) => {
            dict_mut(doc, id)?.set("NeedAppearances", true);
        }
        Object::Dictionary(_) => {
            if let Ok(Object::Dictionary(inline)) = dict_mut(doc, root)?.get_mut(b"AcroForm") {
                inline.set("NeedAppearances", true);
            }
        }
        _ => return Err(error("formulaire illisible")),
    }
    Ok(())
}

/// Propriétés dans le dictionnaire d'information (créé s'il manque) ; une
/// valeur vide retire la propriété.
fn set_info(doc: &mut Document, metadata: &Metadata) {
    let existing = doc.trailer.get(b"Info").and_then(Object::as_reference).ok();
    let id = match existing {
        Some(id) if doc.get_dictionary(id).is_ok() => id,
        _ => {
            let id = doc.add_object(Dictionary::new());
            doc.trailer.set("Info", id);
            id
        }
    };
    let Ok(info) = doc.get_object_mut(id).and_then(Object::as_dict_mut) else {
        return;
    };
    for (key, value) in [
        ("Title", &metadata.title),
        ("Author", &metadata.author),
        ("Subject", &metadata.subject),
        ("Keywords", &metadata.keywords),
    ] {
        if value.trim().is_empty() {
            info.remove(key.as_bytes());
        } else {
            info.set(key, text_string(value.trim()));
        }
    }
}
