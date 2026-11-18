//! Champs de signature du formulaire (`/AcroForm`) : noms, valeurs, widgets
//! et pages. Sert à signer un champ existant et à lister les signatures.

use std::collections::HashSet;

use lopdf::{Dictionary, Object, ObjectId};

use super::source::Source;

/// Profondeur maximale de l'arbre des champs (fichiers piégés).
const MAX_DEPTH: usize = 32;

/// Un champ terminal du formulaire.
#[derive(Debug, Clone)]
pub(crate) struct Field {
    /// Objet du champ (absent si le champ est écrit directement dans `/Fields`).
    pub id: Option<ObjectId>,
    /// Nom complet (`parent.enfant`).
    pub name: String,
    /// Champ de signature (`/FT /Sig`, éventuellement hérité).
    pub is_signature: bool,
    /// Valeur `/V` : pour une signature, le dictionnaire `/Sig` (souvent une référence).
    pub value: Option<Object>,
    /// Widgets du champ (le champ lui-même quand ils sont fusionnés).
    pub widgets: Vec<ObjectId>,
}

/// Tous les champs terminaux du formulaire.
pub(crate) fn all(source: &Source) -> Vec<Field> {
    let mut fields = Vec::new();
    let Some(roots) = root_fields(source) else {
        return fields;
    };
    let mut seen = HashSet::new();
    for root in roots {
        walk(source, &root, "", None, 0, &mut seen, &mut fields);
    }
    fields
}

/// Le dictionnaire `/AcroForm` du catalogue et son objet s'il est indirect.
pub(crate) fn acro_form(source: &Source) -> Option<(Option<ObjectId>, Dictionary)> {
    let catalog = source.catalog().ok()?;
    let entry = catalog.get(b"AcroForm").ok()?;
    let id = entry.as_reference().ok();
    let dictionary = source.resolve(entry).ok()?.as_dict().ok()?.clone();
    Some((id, dictionary))
}

fn root_fields(source: &Source) -> Option<Vec<Object>> {
    let (_, form) = acro_form(source)?;
    let fields = source.resolve(form.get(b"Fields").ok()?).ok()?;
    Some(fields.as_array().ok()?.clone())
}

fn walk(
    source: &Source,
    node: &Object,
    parent_name: &str,
    inherited_type: Option<&[u8]>,
    depth: usize,
    seen: &mut HashSet<ObjectId>,
    out: &mut Vec<Field>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let id = node.as_reference().ok();
    if let Some(id) = id
        && !seen.insert(id)
    {
        return;
    }
    let Ok(Ok(dictionary)) = source.resolve(node).map(Object::as_dict) else {
        return;
    };
    let partial = dictionary
        .get(b"T")
        .ok()
        .and_then(|t| lopdf::decode_text_string(t).ok());
    let name = match (&partial, parent_name.is_empty()) {
        (Some(partial), true) => partial.clone(),
        (Some(partial), false) => format!("{parent_name}.{partial}"),
        (None, _) => parent_name.to_owned(),
    };
    let field_type = dictionary
        .get(b"FT")
        .and_then(Object::as_name)
        .ok()
        .or(inherited_type);
    let kids = dictionary
        .get(b"Kids")
        .ok()
        .and_then(|kids| source.resolve(kids).ok())
        .and_then(|kids| kids.as_array().ok());
    // Des enfants avec un nom sont des champs ; sans nom, des widgets.
    let child_fields: Vec<&Object> = kids
        .into_iter()
        .flatten()
        .filter(|kid| {
            source
                .resolve(kid)
                .ok()
                .and_then(|k| k.as_dict().ok())
                .is_some_and(|k| k.has(b"T"))
        })
        .collect();
    if !child_fields.is_empty() {
        for kid in child_fields {
            walk(source, kid, &name, field_type, depth + 1, seen, out);
        }
        return;
    }
    let mut widgets: Vec<ObjectId> = kids
        .into_iter()
        .flatten()
        .filter_map(|kid| kid.as_reference().ok())
        .collect();
    if widgets.is_empty()
        && let Some(id) = id
        && dictionary.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Widget")
    {
        widgets.push(id);
    }
    out.push(Field {
        id,
        name,
        is_signature: field_type == Some(b"Sig"),
        value: dictionary.get(b"V").ok().cloned(),
        widgets,
    });
}

/// Page (rang à partir de 0) et rectangle du premier widget.
pub(crate) fn placement(source: &Source, field: &Field) -> (Option<usize>, Option<[f32; 4]>) {
    let Some(&widget) = field.widgets.first() else {
        return (None, None);
    };
    let Ok(dictionary) = source.document.get_dictionary(widget) else {
        return (None, None);
    };
    let rect = dictionary
        .get(b"Rect")
        .and_then(Object::as_array)
        .ok()
        .and_then(|values| {
            let numbers: Vec<f32> = values.iter().filter_map(|v| v.as_float().ok()).collect();
            <[f32; 4]>::try_from(numbers).ok()
        });
    let page = dictionary
        .get(b"P")
        .and_then(Object::as_reference)
        .ok()
        .and_then(|page| source.page_index(page))
        .or_else(|| page_listing(source, widget));
    (page, rect)
}

/// Page dont `/Annots` cite ce widget.
fn page_listing(source: &Source, widget: ObjectId) -> Option<usize> {
    source
        .document
        .get_pages()
        .into_iter()
        .find(|(_, page)| {
            source
                .document
                .get_dictionary(*page)
                .ok()
                .and_then(|page| page.get(b"Annots").ok())
                .and_then(|annots| source.resolve(annots).ok())
                .and_then(|annots| annots.as_array().ok())
                .is_some_and(|annots| annots.iter().any(|a| a.as_reference().ok() == Some(widget)))
        })
        .map(|(number, _)| number as usize - 1)
}
