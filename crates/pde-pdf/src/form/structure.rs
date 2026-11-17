//! Arbre des champs du formulaire (dictionnaire `AcroForm`) lu avec
//! `lopdf` : nom complet, genre, options, état « coché » de chaque bouton,
//! valeur. Profondeur et nombre de nœuds bornés (fichier piégé).

use std::collections::HashSet;

use lopdf::{Dictionary, Document, Object, ObjectId, decode_text_string};
use pde_core::edit::FieldValue;

const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 20_000;

/// Drapeaux du champ (`/Ff`) utiles ici.
const READ_ONLY: i64 = 1;
const MULTILINE: i64 = 1 << 12;
const RADIO: i64 = 1 << 15;
const PUSH_BUTTON: i64 = 1 << 16;
const EDITABLE_COMBO: i64 = 1 << 18;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NodeKind {
    Text {
        multiline: bool,
        max_len: Option<u32>,
    },
    Checkbox,
    Radio,
    Choice {
        options: Vec<String>,
        editable: bool,
    },
}

/// Champ terminal : ce que l'utilisateur remplit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldNode {
    pub name: String,
    pub id: ObjectId,
    pub kind: NodeKind,
    pub read_only: bool,
    pub value: Option<FieldValue>,
    /// Ses emplacements (annotations « Widget »), dans l'ordre des `Kids`.
    pub widgets: Vec<ObjectId>,
    /// État « coché » de chaque emplacement (case à cocher, bouton radio).
    pub on_states: Vec<Option<String>>,
}

/// Ce qu'un champ hérite de ses parents.
#[derive(Clone, Default)]
struct Inherited {
    name: String,
    field_type: Option<Vec<u8>>,
    flags: i64,
    value: Option<Object>,
    options: Option<Object>,
    max_len: Option<i64>,
}

fn dict<'a>(doc: &'a Document, object: &'a Object) -> Option<(Option<ObjectId>, &'a Dictionary)> {
    let (id, object) = doc.dereference(object).ok()?;
    object.as_dict().ok().map(|d| (id, d))
}

fn text(doc: &Document, object: &Object) -> Option<String> {
    let (_, object) = doc.dereference(object).ok()?;
    decode_text_string(object).ok()
}

/// Champs terminaux du formulaire ; vide s'il n'y en a pas.
pub(crate) fn fields(doc: &Document) -> Vec<FieldNode> {
    let Some(form) = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"AcroForm").ok())
        .and_then(|f| dict(doc, f))
    else {
        return Vec::new();
    };
    let Ok(roots) = form.1.get(b"Fields").and_then(Object::as_array) else {
        return Vec::new();
    };
    let mut walk = Walk {
        doc,
        seen: HashSet::new(),
        out: Vec::new(),
    };
    for root in roots {
        if let Ok(id) = root.as_reference() {
            walk.node(id, &Inherited::default(), 0);
        }
    }
    walk.out
}

struct Walk<'a> {
    doc: &'a Document,
    seen: HashSet<ObjectId>,
    out: Vec<FieldNode>,
}

impl Walk<'_> {
    fn node(&mut self, id: ObjectId, parent: &Inherited, depth: usize) {
        if depth > MAX_DEPTH || self.seen.len() > MAX_NODES || !self.seen.insert(id) {
            return;
        }
        let Ok(node) = self.doc.get_dictionary(id) else {
            return;
        };
        let mut here = parent.clone();
        if let Some(partial) = node.get(b"T").ok().and_then(|t| text(self.doc, t)) {
            here.name = if parent.name.is_empty() {
                partial
            } else {
                format!("{}.{partial}", parent.name)
            };
        }
        if let Ok(kind) = node.get(b"FT").and_then(Object::as_name) {
            here.field_type = Some(kind.to_vec());
        }
        if let Ok(flags) = node.get(b"Ff").and_then(Object::as_i64) {
            here.flags = flags;
        }
        if let Ok(value) = node.get(b"V") {
            here.value = Some(value.clone());
        }
        if let Ok(options) = node.get(b"Opt") {
            here.options = Some(options.clone());
        }
        if let Ok(max_len) = node.get(b"MaxLen").and_then(Object::as_i64) {
            here.max_len = Some(max_len);
        }
        let kids: Vec<ObjectId> = node
            .get(b"Kids")
            .and_then(Object::as_array)
            .map(|kids| kids.iter().filter_map(|k| k.as_reference().ok()).collect())
            .unwrap_or_default();
        // Enfants nommés : des champs ; les autres : ses emplacements.
        let (fields, widgets): (Vec<ObjectId>, Vec<ObjectId>) = kids.into_iter().partition(|kid| {
            self.doc
                .get_dictionary(*kid)
                .map(|d| d.has(b"T"))
                .unwrap_or(false)
        });
        for field in fields {
            self.node(field, &here, depth + 1);
        }
        let widgets = if widgets.is_empty() && node.has(b"Rect") {
            vec![id]
        } else {
            widgets
        };
        if widgets.is_empty() || here.name.is_empty() {
            return;
        }
        if let Some(field) = self.terminal(id, &here, widgets) {
            self.out.push(field);
        }
    }

    fn terminal(
        &self,
        id: ObjectId,
        here: &Inherited,
        widgets: Vec<ObjectId>,
    ) -> Option<FieldNode> {
        let flags = here.flags;
        let value = here.value.as_ref();
        let (kind, value) = match here.field_type.as_deref()? {
            b"Tx" => (
                NodeKind::Text {
                    multiline: flags & MULTILINE != 0,
                    max_len: here.max_len.and_then(|m| u32::try_from(m).ok()),
                },
                value.and_then(|v| text(self.doc, v)).map(FieldValue::Text),
            ),
            b"Btn" if flags & PUSH_BUTTON != 0 => return None,
            b"Btn" => {
                let state = value
                    .and_then(|v| v.as_name().ok())
                    .map(|n| String::from_utf8_lossy(n).into_owned());
                if flags & RADIO != 0 {
                    (
                        NodeKind::Radio,
                        state.filter(|s| s != "Off").map(FieldValue::Choice),
                    )
                } else {
                    let checked = state.is_some_and(|s| s != "Off");
                    (NodeKind::Checkbox, Some(FieldValue::Checked(checked)))
                }
            }
            b"Ch" => (
                NodeKind::Choice {
                    options: self.options(here.options.as_ref()),
                    editable: flags & EDITABLE_COMBO != 0,
                },
                value.and_then(|v| self.choice(v)).map(FieldValue::Choice),
            ),
            _ => return None,
        };
        let on_states = widgets.iter().map(|w| self.on_state(*w)).collect();
        Some(FieldNode {
            name: here.name.clone(),
            id,
            kind,
            read_only: flags & READ_ONLY != 0,
            value,
            widgets,
            on_states,
        })
    }

    /// Options d'une liste : chaîne, ou paire [valeur d'export, libellé].
    fn options(&self, options: Option<&Object>) -> Vec<String> {
        let Some(Ok((_, Ok(items)))) = options.map(|o| {
            self.doc
                .dereference(o)
                .map(|(id, obj)| (id, obj.as_array()))
        }) else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| match item {
                Object::Array(pair) => pair.first().and_then(|e| text(self.doc, e)),
                other => text(self.doc, other),
            })
            .collect()
    }

    fn choice(&self, value: &Object) -> Option<String> {
        match value {
            Object::Array(values) => values.first().and_then(|v| text(self.doc, v)),
            other => text(self.doc, other),
        }
    }

    /// Nom de l'apparence « coché » d'un emplacement : la clé de `/AP /N`
    /// autre que `Off`.
    fn on_state(&self, widget: ObjectId) -> Option<String> {
        let widget = self.doc.get_dictionary(widget).ok()?;
        let (_, ap) = dict(self.doc, widget.get(b"AP").ok()?)?;
        let (_, normal) = dict(self.doc, ap.get(b"N").ok()?)?;
        normal
            .iter()
            .map(|(key, _)| String::from_utf8_lossy(key).into_owned())
            .find(|key| key != "Off")
    }
}
