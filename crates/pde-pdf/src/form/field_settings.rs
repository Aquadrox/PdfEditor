//! Réglages d'un champ écrits en place (E-12) : nom partiel (`/T`, sous le
//! nom de son parent), drapeaux réglés par PdfEditor (les autres gardés),
//! infobulle, apparence par défaut, alignement, longueur, options, scripts
//! de format et de calcul (`/AA`, réécrits seulement s'ils changent ; une
//! action inconnue est gardée) et ordre des calculs (`/CO`).

use lopdf::{Dictionary, Document, Object, ObjectId, text_string};
use pde_core::edit::{FieldBox, FieldKind};
use pde_core::engine::EngineError;
use pde_core::formats::{FieldActions, read_actions, write_actions};

use super::look;
use super::structure::{
    COMB, COMBO, EDITABLE_COMBO, FieldNode, MULTI_SELECT, MULTILINE, PASSWORD, RADIO, READ_ONLY,
    REQUIRED, dict,
};
use super::tree::{array_mut, dict_mut, error, remove_ref};

/// Bouton radio qu'on ne peut pas décocher en cliquant dessus.
const NO_TOGGLE_TO_OFF: i64 = 1 << 14;
/// Drapeaux réglés par PdfEditor ; les autres sont gardés.
const MANAGED: i64 = READ_ONLY
    | REQUIRED
    | MULTILINE
    | PASSWORD
    | COMB
    | COMBO
    | EDITABLE_COMBO
    | MULTI_SELECT
    | RADIO
    | NO_TOGGLE_TO_OFF;

/// Réglages du champ `id` d'après `field` ; `node` : le champ tel qu'il
/// est dans le document (nom de son parent, drapeaux, scripts), s'il existe.
pub(super) fn settings(
    doc: &mut Document,
    form: ObjectId,
    id: ObjectId,
    node: &FieldNode,
    field: &FieldBox,
) -> Result<(), EngineError> {
    let partial = if node.prefix.is_empty() {
        field.name.clone()
    } else {
        field
            .name
            .strip_prefix(&format!("{}.", node.prefix))
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                error(format!(
                    "le nom du champ doit commencer par « {}. »",
                    node.prefix
                ))
            })?
    };
    let flags = (node.flags & !MANAGED) | flags_of(field);
    let (format, calc) = (field.kind.format().cloned(), field.kind.calc().cloned());
    let actions_changed = read_actions(&node.actions) != (format.clone(), calc.clone());
    let actions = write_actions(format.as_ref(), calc.as_ref());
    let old_aa = doc
        .get_dictionary(id)
        .ok()
        .and_then(|f| f.get(b"AA").ok())
        .and_then(|aa| dict(doc, aa))
        .map(|(_, d)| d.clone())
        .unwrap_or_default();
    let target = dict_mut(doc, id)?;
    target.set("T", text_string(&partial));
    target.set("Ff", flags);
    if field.tooltip.trim().is_empty() {
        target.remove(b"TU");
    } else {
        target.set("TU", text_string(field.tooltip.trim()));
    }
    if !matches!(field.kind, FieldKind::Signature) {
        target.set("DA", Object::string_literal(look::da(&field.look)));
        target.set("Q", look::quadding(&field.look));
    }
    match &field.kind {
        FieldKind::Text { max_len, .. } => match max_len {
            Some(max) => target.set("MaxLen", i64::from(*max)),
            None => {
                target.remove(b"MaxLen");
            }
        },
        FieldKind::Choice { options, .. } => {
            target.set(
                "Opt",
                Object::Array(options.iter().map(|o| text_string(o)).collect()),
            );
        }
        _ => {}
    }
    if actions_changed {
        let aa = actions_dict(old_aa, &actions);
        if aa.is_empty() {
            target.remove(b"AA");
        } else {
            target.set("AA", Object::Dictionary(aa));
        }
    }
    remove_ref(doc, form, b"CO", id)?;
    if calc.is_some() {
        array_mut(doc, form, b"CO")?.push(Object::Reference(id));
    }
    Ok(())
}

fn flags_of(field: &FieldBox) -> i64 {
    let mut flags = 0;
    if field.read_only {
        flags |= READ_ONLY;
    }
    if field.required {
        flags |= REQUIRED;
    }
    match &field.kind {
        FieldKind::Text {
            multiline,
            password,
            comb,
            ..
        } => {
            for (on, flag) in [
                (*multiline, MULTILINE),
                (*password, PASSWORD),
                (*comb, COMB),
            ] {
                if on {
                    flags |= flag;
                }
            }
        }
        FieldKind::Choice {
            editable,
            list,
            multi,
            ..
        } => {
            if !list {
                flags |= COMBO;
                if *editable {
                    flags |= EDITABLE_COMBO;
                }
            } else if *multi {
                flags |= MULTI_SELECT;
            }
        }
        FieldKind::Radio { .. } => flags |= RADIO | NO_TOGGLE_TO_OFF,
        FieldKind::Checkbox | FieldKind::Signature => {}
    }
    flags
}

/// Actions `/AA` : celles de PdfEditor remplacées, les autres gardées.
fn actions_dict(mut aa: Dictionary, actions: &FieldActions) -> Dictionary {
    for (key, script) in [
        ("F", &actions.format),
        ("K", &actions.keystroke),
        ("V", &actions.validate),
        ("C", &actions.calculate),
    ] {
        match script {
            Some(script) => {
                let mut action = Dictionary::new();
                action.set("S", Object::Name(b"JavaScript".to_vec()));
                action.set("JS", text_string(script));
                aa.set(key, Object::Dictionary(action));
            }
            None => {
                aa.remove(key.as_bytes());
            }
        }
    }
    aa
}
