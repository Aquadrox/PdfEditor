//! Arbre des champs du formulaire (dictionnaire `AcroForm`) lu avec
//! `lopdf` : nom complet, genre, options, état « coché » de chaque bouton,
//! valeur, réglages (obligatoire, infobulle, apparence, scripts de format
//! et de calcul). Profondeur et nombre de nœuds bornés (fichier piégé).

use std::collections::HashSet;

use lopdf::{Dictionary, Document, Object, ObjectId, decode_text_string};
use pde_core::edit::{FieldLook, FieldValue};
use pde_core::formats::FieldActions;

use super::look;

const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 20_000;
/// Script le plus long lu (au-delà, ignoré).
const MAX_SCRIPT_BYTES: usize = 64 * 1024;

/// Drapeaux du champ (`/Ff`) utiles ici.
pub(crate) const READ_ONLY: i64 = 1;
pub(crate) const REQUIRED: i64 = 1 << 1;
pub(crate) const MULTILINE: i64 = 1 << 12;
pub(crate) const PASSWORD: i64 = 1 << 13;
pub(crate) const RADIO: i64 = 1 << 15;
pub(crate) const PUSH_BUTTON: i64 = 1 << 16;
pub(crate) const COMBO: i64 = 1 << 17;
pub(crate) const EDITABLE_COMBO: i64 = 1 << 18;
pub(crate) const MULTI_SELECT: i64 = 1 << 21;
pub(crate) const COMB: i64 = 1 << 24;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NodeKind {
    Text {
        multiline: bool,
        max_len: Option<u32>,
        password: bool,
        comb: bool,
    },
    Checkbox,
    Radio,
    Choice {
        options: Vec<String>,
        editable: bool,
        list: bool,
        multi: bool,
    },
    Signature,
}

/// Champ terminal : ce que l'utilisateur remplit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldNode {
    pub name: String,
    /// Nom complet du parent (« client » pour « client.nom »), vide à la racine.
    pub prefix: String,
    pub id: ObjectId,
    pub kind: NodeKind,
    pub flags: i64,
    pub value: Option<FieldValue>,
    pub tooltip: String,
    pub look: FieldLook,
    pub actions: FieldActions,
    /// Ses emplacements (annotations « Widget »), dans l'ordre des `Kids`.
    pub widgets: Vec<ObjectId>,
    /// État « coché » de chaque emplacement (case à cocher, bouton radio).
    pub on_states: Vec<Option<String>>,
    /// Rectangle (`/Rect`) de chaque emplacement, espace de la page PDF.
    pub rects: Vec<Option<[f64; 4]>>,
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
    appearance: Option<String>,
    quadding: Option<i64>,
}

pub(crate) fn dict<'a>(
    doc: &'a Document,
    object: &'a Object,
) -> Option<(Option<ObjectId>, &'a Dictionary)> {
    let (id, object) = doc.dereference(object).ok()?;
    object.as_dict().ok().map(|d| (id, d))
}

pub(crate) fn text(doc: &Document, object: &Object) -> Option<String> {
    let (_, object) = doc.dereference(object).ok()?;
    decode_text_string(object).ok()
}

/// Dictionnaire `AcroForm` du catalogue.
pub(crate) fn acroform(doc: &Document) -> Option<&Dictionary> {
    let catalog = doc.catalog().ok()?;
    dict(doc, catalog.get(b"AcroForm").ok()?).map(|(_, d)| d)
}

/// Champs terminaux du formulaire ; vide s'il n'y en a pas.
pub(crate) fn fields(doc: &Document) -> Vec<FieldNode> {
    let Some(form) = acroform(doc) else {
        return Vec::new();
    };
    let Ok(roots) = form.get(b"Fields").and_then(Object::as_array) else {
        return Vec::new();
    };
    let root = Inherited {
        appearance: form.get(b"DA").ok().and_then(|d| text(doc, d)),
        quadding: form.get(b"Q").and_then(Object::as_i64).ok(),
        ..Inherited::default()
    };
    let mut walk = Walk {
        doc,
        seen: HashSet::new(),
        out: Vec::new(),
    };
    for root_field in roots {
        if let Ok(id) = root_field.as_reference() {
            walk.node(id, &root, 0);
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
        if let Some(da) = node.get(b"DA").ok().and_then(|d| text(self.doc, d)) {
            here.appearance = Some(da);
        }
        if let Ok(q) = node.get(b"Q").and_then(Object::as_i64) {
            here.quadding = Some(q);
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
        let prefix = parent.name.clone();
        if let Some(field) = self.terminal(id, &here, prefix, widgets) {
            self.out.push(field);
        }
    }

    fn terminal(
        &self,
        id: ObjectId,
        here: &Inherited,
        prefix: String,
        widgets: Vec<ObjectId>,
    ) -> Option<FieldNode> {
        let flags = here.flags;
        let value = here.value.as_ref();
        let (kind, value) = match here.field_type.as_deref()? {
            b"Tx" => (
                NodeKind::Text {
                    multiline: flags & MULTILINE != 0,
                    max_len: here.max_len.and_then(|m| u32::try_from(m).ok()),
                    password: flags & PASSWORD != 0,
                    comb: flags & COMB != 0,
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
            b"Ch" => {
                let multi = flags & MULTI_SELECT != 0 && flags & COMBO == 0;
                let value = value.and_then(|v| self.choice(v, multi));
                (
                    NodeKind::Choice {
                        options: self.options(here.options.as_ref()),
                        editable: flags & EDITABLE_COMBO != 0 && flags & COMBO != 0,
                        list: flags & COMBO == 0,
                        multi,
                    },
                    value,
                )
            }
            b"Sig" => (NodeKind::Signature, None),
            _ => return None,
        };
        let node = self.doc.get_dictionary(id).ok()?;
        let first_widget = widgets
            .first()
            .and_then(|w| self.doc.get_dictionary(*w).ok());
        let on_states = widgets.iter().map(|w| self.on_state(*w)).collect();
        let rects = widgets.iter().map(|w| self.rect(*w)).collect();
        Some(FieldNode {
            name: here.name.clone(),
            prefix,
            id,
            kind,
            flags,
            value,
            tooltip: node
                .get(b"TU")
                .ok()
                .and_then(|t| text(self.doc, t))
                .unwrap_or_default(),
            look: look::read(
                self.doc,
                here.appearance.as_deref(),
                here.quadding,
                first_widget,
            ),
            actions: self.actions(node),
            widgets,
            on_states,
            rects,
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

    fn choice(&self, value: &Object, multi: bool) -> Option<FieldValue> {
        match value {
            Object::Array(values) if multi => Some(FieldValue::Choices(
                values.iter().filter_map(|v| text(self.doc, v)).collect(),
            )),
            Object::Array(values) => values
                .first()
                .and_then(|v| text(self.doc, v))
                .map(FieldValue::Choice),
            other if multi => text(self.doc, other).map(|t| FieldValue::Choices(vec![t])),
            other => text(self.doc, other).map(FieldValue::Choice),
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

    fn rect(&self, widget: ObjectId) -> Option<[f64; 4]> {
        let widget = self.doc.get_dictionary(widget).ok()?;
        let (_, array) = self.doc.dereference(widget.get(b"Rect").ok()?).ok()?;
        let values: Vec<f64> = array
            .as_array()
            .ok()?
            .iter()
            .filter_map(|v| v.as_float().ok().map(f64::from))
            .collect();
        match values.as_slice() {
            [a, b, c, d] => Some([a.min(*c), b.min(*d), a.max(*c), b.max(*d)]),
            _ => None,
        }
    }

    /// Scripts des actions `/AA` du champ (mise en forme, frappe,
    /// validation, calcul) : lus comme du texte, jamais exécutés.
    fn actions(&self, node: &Dictionary) -> FieldActions {
        let Some((_, aa)) = node.get(b"AA").ok().and_then(|a| dict(self.doc, a)) else {
            return FieldActions::default();
        };
        let script = |key: &[u8]| -> Option<String> {
            let (_, action) = dict(self.doc, aa.get(key).ok()?)?;
            let (_, js) = self.doc.dereference(action.get(b"JS").ok()?).ok()?;
            let bytes = match js {
                Object::Stream(stream) => stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone()),
                other => return decode_text_string(other).ok(),
            };
            (bytes.len() <= MAX_SCRIPT_BYTES).then(|| String::from_utf8_lossy(&bytes).into_owned())
        };
        FieldActions {
            format: script(b"F"),
            keystroke: script(b"K"),
            validate: script(b"V"),
            calculate: script(b"C"),
        }
    }
}
