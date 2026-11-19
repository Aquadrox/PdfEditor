//! Conception de formulaires (E-12) écrite avec `lopdf` : champs créés
//! (dictionnaires de champ et annotations « Widget », apparence des
//! boutons), champs du document déplacés, réglés, renommés ou supprimés en
//! place — ce qui n'est pas réglé par PdfEditor (scripts inconnus, autres
//! drapeaux, actions) est gardé.

use lopdf::{Dictionary, Document, Object, ObjectId, text_string};
use pde_core::edit::{FieldBox, FieldKind, FieldValue};
use pde_core::engine::EngineError;
use pde_core::formats::{FieldActions, read_actions, write_actions};

use super::structure::{
    COMB, COMBO, EDITABLE_COMBO, FieldNode, MULTI_SELECT, MULTILINE, PASSWORD, RADIO, READ_ONLY,
    REQUIRED, dict, fields,
};
use super::tree::{
    array_mut, attach_widget, detach_field, detach_widget, dict_mut, ensure_acroform, error,
    page_of_widget, page_rotation, remove_ref,
};
use super::{appearance, look};

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

/// Élément de conception et sa place : page du document, rectangle dans
/// l'espace de la page PDF (x0, y0, x1, y1).
pub(crate) struct Designed<'a> {
    pub field: &'a FieldBox,
    pub page: usize,
    pub rect: [f64; 4],
}

/// Applique les éléments de conception au document.
pub(crate) fn design(doc: &mut Document, boxes: &[Designed]) -> Result<(), EngineError> {
    if boxes.is_empty() {
        return Ok(());
    }
    let form = ensure_acroform(doc)?;
    let nodes = fields(doc);
    let mut origins: Vec<&str> = Vec::new();
    for d in boxes {
        if let Some(origin) = &d.field.origin
            && !origins.contains(&origin.name.as_str())
        {
            origins.push(&origin.name);
        }
    }
    for name in origins {
        let node = nodes
            .iter()
            .find(|n| n.name == name)
            .ok_or_else(|| error(format!("champ « {name} » introuvable")))?;
        let group: Vec<&Designed> = boxes
            .iter()
            .filter(|d| d.field.origin.as_ref().is_some_and(|o| o.name == name))
            .collect();
        change(doc, form, node, &group)?;
    }
    let mut created: Vec<&str> = Vec::new();
    for d in boxes.iter().filter(|d| d.field.origin.is_none()) {
        if !created.contains(&d.field.name.as_str()) {
            created.push(&d.field.name);
        }
    }
    for name in created {
        let group: Vec<&Designed> = boxes
            .iter()
            .filter(|d| d.field.origin.is_none() && d.field.name == name)
            .collect();
        create(doc, form, &group)?;
    }
    Ok(())
}

fn page_id(doc: &Document, index: usize) -> Result<ObjectId, EngineError> {
    let number = u32::try_from(index + 1).map_err(|_| error("page invalide"))?;
    doc.get_pages()
        .get(&number)
        .copied()
        .ok_or_else(|| error(format!("page {number} introuvable")))
}

fn rect_object(rect: [f64; 4]) -> Object {
    Object::Array(rect.iter().map(|v| Object::Real(*v as f32)).collect())
}

fn is_button(kind: &FieldKind) -> bool {
    matches!(kind, FieldKind::Checkbox | FieldKind::Radio { .. })
}

/// Champ du document modifié ou supprimé, emplacement par emplacement.
fn change(
    doc: &mut Document,
    form: ObjectId,
    node: &FieldNode,
    group: &[&Designed],
) -> Result<(), EngineError> {
    let mut removed = 0;
    for d in group {
        let Some(origin) = &d.field.origin else {
            continue;
        };
        let widget = *node.widgets.get(origin.widget).ok_or_else(|| {
            error(format!(
                "emplacement du champ « {} » introuvable",
                node.name
            ))
        })?;
        detach_widget(doc, widget)?;
        if d.field.deleted {
            if widget != node.id {
                remove_ref(doc, node.id, b"Kids", widget)?;
            }
            removed += 1;
            continue;
        }
        let page = page_id(doc, d.page)?;
        attach_widget(doc, page, widget)?;
        let rotation = page_rotation(doc, page);
        let old_mk = doc
            .get_dictionary(widget)
            .ok()
            .and_then(|w| w.get(b"MK").ok())
            .and_then(|mk| dict(doc, mk))
            .map(|(_, d)| d.clone())
            .unwrap_or_default();
        let mut mk = old_mk;
        for key in [&b"BC"[..], b"BG", b"R"] {
            mk.remove(key);
        }
        mk.extend(&look::mk(&d.field.look, rotation, None));
        let w = dict_mut(doc, widget)?;
        w.set("Rect", rect_object(d.rect));
        w.set("MK", Object::Dictionary(mk));
        if !is_button(&node_kind(node)) {
            // Redessinée au nouveau format par les lecteurs.
            w.remove(b"AP");
        }
    }
    if removed == node.widgets.len() {
        detach_field(doc, form, node.id)?;
        return remove_ref(doc, form, b"CO", node.id);
    }
    if let Some(d) = group.iter().find(|d| !d.field.deleted) {
        settings(doc, form, node.id, node, d.field)?;
    }
    Ok(())
}

/// Genre lu (pour savoir s'il s'agit d'un bouton).
fn node_kind(node: &FieldNode) -> FieldKind {
    use super::structure::NodeKind;
    match node.kind {
        NodeKind::Checkbox => FieldKind::Checkbox,
        NodeKind::Radio => FieldKind::Radio {
            options: Vec::new(),
        },
        NodeKind::Signature => FieldKind::Signature,
        _ => FieldKind::text(),
    }
}

/// Réglages du champ `id` d'après `field` ; `node` : le champ tel qu'il
/// est dans le document (nom de son parent, drapeaux, scripts), s'il existe.
fn settings(
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

fn field_type(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Text { .. } => "Tx",
        FieldKind::Checkbox | FieldKind::Radio { .. } => "Btn",
        FieldKind::Choice { .. } => "Ch",
        FieldKind::Signature => "Sig",
    }
}

/// Valeur initiale d'un champ créé (`/V` et `/DV`).
fn initial_value(field: &FieldBox) -> Option<Object> {
    let value = field.value.as_ref()?;
    Some(match (&field.kind, value) {
        (FieldKind::Checkbox, FieldValue::Checked(true)) => {
            Object::Name(field.export.as_bytes().to_vec())
        }
        (FieldKind::Checkbox, FieldValue::Checked(false)) => Object::Name(b"Off".to_vec()),
        (FieldKind::Radio { .. }, FieldValue::Choice(choice)) => {
            Object::Name(choice.as_bytes().to_vec())
        }
        (_, FieldValue::Choices(items)) => {
            Object::Array(items.iter().map(|i| text_string(i)).collect())
        }
        (_, FieldValue::Text(text) | FieldValue::Choice(text)) => text_string(text),
        _ => return None,
    })
}

/// Champ créé : un dictionnaire de champ et ses emplacements (le même
/// objet s'il n'en a qu'un et n'est pas un groupe radio).
fn create(doc: &mut Document, form: ObjectId, group: &[&Designed]) -> Result<(), EngineError> {
    let Some(first) = group.first() else {
        return Ok(());
    };
    let field = first.field;
    let mut dict = Dictionary::new();
    dict.set(
        "FT",
        Object::Name(field_type(&field.kind).as_bytes().to_vec()),
    );
    let initial = initial_value(field);
    if let Some(value) = &initial {
        dict.set("V", value.clone());
        dict.set("DV", value.clone());
    }
    let id = doc.add_object(dict);
    let blank = FieldNode {
        name: field.name.clone(),
        prefix: String::new(),
        id,
        kind: super::structure::NodeKind::Signature,
        flags: 0,
        value: None,
        tooltip: String::new(),
        look: field.look.clone(),
        actions: FieldActions::default(),
        widgets: Vec::new(),
        on_states: Vec::new(),
        rects: Vec::new(),
    };
    settings(doc, form, id, &blank, field)?;
    let single = group.len() == 1 && !matches!(field.kind, FieldKind::Radio { .. });
    let mut kids = Vec::new();
    for d in group {
        let widget = if single {
            id
        } else {
            let mut kid = Dictionary::new();
            kid.set("Parent", id);
            doc.add_object(kid)
        };
        widget_keys(doc, widget, d, initial.as_ref())?;
        kids.push(Object::Reference(widget));
    }
    if !single {
        dict_mut(doc, id)?.set("Kids", Object::Array(kids));
    }
    array_mut(doc, form, b"Fields")?.push(Object::Reference(id));
    Ok(())
}

fn widget_keys(
    doc: &mut Document,
    widget: ObjectId,
    d: &Designed,
    initial: Option<&Object>,
) -> Result<(), EngineError> {
    let page = page_id(doc, d.page)?;
    let rotation = page_rotation(doc, page);
    let size = ((d.rect[2] - d.rect[0]).abs(), (d.rect[3] - d.rect[1]).abs());
    let field = d.field;
    let radio = matches!(field.kind, FieldKind::Radio { .. });
    let caption = match field.kind {
        FieldKind::Checkbox => Some("4"),
        FieldKind::Radio { .. } => Some("l"),
        _ => None,
    };
    let ap = match &field.kind {
        FieldKind::Checkbox | FieldKind::Radio { .. } => Some(appearance::button(
            doc,
            &field.look,
            size,
            &field.export,
            radio,
        )),
        FieldKind::Signature => Some(appearance::signature(doc, &field.look, size)),
        _ => None,
    };
    let on = initial.and_then(|v| v.as_name().ok()) == Some(field.export.as_bytes());
    let w = dict_mut(doc, widget)?;
    w.set("Type", Object::Name(b"Annot".to_vec()));
    w.set("Subtype", Object::Name(b"Widget".to_vec()));
    w.set("Rect", rect_object(d.rect));
    w.set("F", 4);
    w.set(
        "MK",
        Object::Dictionary(look::mk(&field.look, rotation, caption)),
    );
    if let Some(ap) = ap {
        w.set("AP", Object::Dictionary(ap));
    }
    if is_button(&field.kind) {
        let state = if on { field.export.as_bytes() } else { b"Off" };
        w.set("AS", Object::Name(state.to_vec()));
    }
    attach_widget(doc, page, widget)?;
    debug_assert!(page_of_widget(doc, widget) == Some(page));
    Ok(())
}
