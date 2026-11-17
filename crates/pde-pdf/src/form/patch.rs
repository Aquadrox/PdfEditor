//! Valeurs des champs et propriétés du document écrites avec `lopdf` sur
//! un contenu enregistré par PDFium. Les champs modifiés perdent leur
//! ancienne apparence et le formulaire demande aux lecteurs de les
//! redessiner (`NeedAppearances`) : PDFium le fait au rechargement.

use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object, ObjectId, text_string};
use pde_core::edit::{FieldValue, Metadata};
use pde_core::engine::EngineError;

use super::structure::{FieldNode, NodeKind, fields};

fn error(message: impl Into<String>) -> EngineError {
    EngineError::Internal(message.into())
}

/// Le contenu `bytes`, champs remplis et propriétés changées.
pub(crate) fn patch(
    bytes: Vec<u8>,
    values: &BTreeMap<String, FieldValue>,
    metadata: Option<&Metadata>,
) -> Result<Vec<u8>, EngineError> {
    if values.is_empty() && metadata.is_none() {
        return Ok(bytes);
    }
    let mut doc = Document::load_mem(&bytes).map_err(|e| error(format!("lecture : {e}")))?;
    if doc.trailer.has(b"Encrypt") {
        return Err(error(
            "document protégé par mot de passe : formulaire et propriétés ne se modifient pas encore",
        ));
    }
    if !values.is_empty() {
        let nodes = fields(&doc);
        for (name, value) in values {
            let node = nodes
                .iter()
                .find(|n| &n.name == name)
                .ok_or_else(|| error(format!("champ « {name} » introuvable")))?;
            fill(&mut doc, node, value)?;
        }
        need_appearances(&mut doc)?;
    }
    if let Some(metadata) = metadata {
        set_info(&mut doc, metadata);
    }
    let mut out = Vec::with_capacity(bytes.len());
    doc.save_to(&mut out)
        .map_err(|e| error(format!("écriture : {e}")))?;
    Ok(out)
}

fn dict_mut(doc: &mut Document, id: ObjectId) -> Result<&mut Dictionary, EngineError> {
    doc.get_object_mut(id)
        .and_then(Object::as_dict_mut)
        .map_err(|e| error(format!("objet {} : {e}", id.0)))
}

fn fill(doc: &mut Document, node: &FieldNode, value: &FieldValue) -> Result<(), EngineError> {
    match (&node.kind, value) {
        (NodeKind::Text { .. }, FieldValue::Text(text))
        | (NodeKind::Choice { .. }, FieldValue::Choice(text)) => {
            dict_mut(doc, node.id)?.set("V", text_string(text));
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
