//! Rangement des champs dans le document (`lopdf`) : dictionnaire
//! `AcroForm` (créé s'il manque, polices du formulaire), liste des champs,
//! enfants d'un champ, annotations d'une page. Les tableaux peuvent être
//! directs ou désignés par référence : les deux sont traités.

use lopdf::{Dictionary, Document, Object, ObjectId};
use pde_core::engine::EngineError;

pub(crate) fn error(message: impl Into<String>) -> EngineError {
    EngineError::Internal(message.into())
}

pub(crate) fn dict_mut(doc: &mut Document, id: ObjectId) -> Result<&mut Dictionary, EngineError> {
    doc.get_object_mut(id)
        .and_then(Object::as_dict_mut)
        .map_err(|e| error(format!("objet {} : {e}", id.0)))
}

fn root_id(doc: &Document) -> Result<ObjectId, EngineError> {
    doc.trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|e| error(format!("catalogue : {e}")))
}

/// Dictionnaire `AcroForm`, rendu indirect (créé s'il manque), avec ses
/// polices `/Helv` et `/ZaDb` et une apparence par défaut.
pub(crate) fn ensure_acroform(doc: &mut Document) -> Result<ObjectId, EngineError> {
    let root = root_id(doc)?;
    let current = dict_mut(doc, root)?.get(b"AcroForm").ok().cloned();
    let id = match current {
        Some(Object::Reference(id)) if doc.get_dictionary(id).is_ok() => id,
        Some(Object::Dictionary(inline)) => {
            let id = doc.add_object(inline);
            dict_mut(doc, root)?.set("AcroForm", id);
            id
        }
        _ => {
            let id = doc.add_object(Dictionary::new());
            dict_mut(doc, root)?.set("AcroForm", id);
            id
        }
    };
    let form = doc.get_dictionary(id).map_err(|e| error(e.to_string()))?;
    let deref = |object: &Object| -> Option<Dictionary> {
        doc.dereference(object)
            .ok()
            .and_then(|(_, o)| o.as_dict().ok())
            .cloned()
    };
    let mut resources = form.get(b"DR").ok().and_then(deref).unwrap_or_default();
    let mut fonts = resources
        .get(b"Font")
        .ok()
        .and_then(deref)
        .unwrap_or_default();
    if !fonts.has(b"Helv") {
        let helv = doc.add_object(font("Helvetica", true));
        fonts.set("Helv", helv);
    }
    if !fonts.has(b"ZaDb") {
        let zadb = doc.add_object(font("ZapfDingbats", false));
        fonts.set("ZaDb", zadb);
    }
    let form = dict_mut(doc, id)?;
    resources.set("Font", Object::Dictionary(fonts));
    form.set("DR", Object::Dictionary(resources));
    if !form.has(b"DA") {
        form.set("DA", Object::string_literal("/Helv 0 Tf 0 g"));
    }
    if !form.has(b"Fields") {
        form.set("Fields", Object::Array(Vec::new()));
    }
    Ok(id)
}

fn font(name: &str, win_ansi: bool) -> Dictionary {
    let mut font = Dictionary::new();
    font.set("Type", Object::Name(b"Font".to_vec()));
    font.set("Subtype", Object::Name(b"Type1".to_vec()));
    font.set("BaseFont", Object::Name(name.as_bytes().to_vec()));
    if win_ansi {
        font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
    }
    font
}

/// Tableau `key` de l'objet `owner`, direct ou désigné par référence ;
/// créé vide s'il manque.
pub(crate) fn array_mut<'a>(
    doc: &'a mut Document,
    owner: ObjectId,
    key: &[u8],
) -> Result<&'a mut Vec<Object>, EngineError> {
    let reference = match dict_mut(doc, owner)?.get(key) {
        Ok(Object::Reference(id)) => Some(*id),
        Ok(Object::Array(_)) => None,
        _ => {
            dict_mut(doc, owner)?.set(key.to_vec(), Object::Array(Vec::new()));
            None
        }
    };
    let object = match reference {
        Some(id) => doc
            .get_object_mut(id)
            .map_err(|e| error(format!("tableau {} : {e}", id.0)))?,
        None => dict_mut(doc, owner)?
            .get_mut(key)
            .map_err(|e| error(e.to_string()))?,
    };
    object.as_array_mut().map_err(|e| error(e.to_string()))
}

/// Retire `target` du tableau `key` de `owner`, s'il y est.
pub(crate) fn remove_ref(
    doc: &mut Document,
    owner: ObjectId,
    key: &[u8],
    target: ObjectId,
) -> Result<(), EngineError> {
    if dict_mut(doc, owner)?.get(key).is_err() {
        return Ok(());
    }
    array_mut(doc, owner, key)?.retain(|o| o.as_reference().ok() != Some(target));
    Ok(())
}

/// Page qui porte l'annotation `widget` : `/P`, sinon cherchée dans les pages.
pub(crate) fn page_of_widget(doc: &Document, widget: ObjectId) -> Option<ObjectId> {
    let pages = doc.get_pages();
    if let Ok(page) = doc
        .get_dictionary(widget)
        .and_then(|w| w.get(b"P"))
        .and_then(Object::as_reference)
        && pages.values().any(|p| *p == page)
    {
        return Some(page);
    }
    pages.values().copied().find(|page| {
        doc.get_dictionary(*page)
            .ok()
            .and_then(|p| p.get(b"Annots").ok())
            .and_then(|a| doc.dereference(a).ok())
            .and_then(|(_, a)| a.as_array().ok())
            .is_some_and(|annots| annots.iter().any(|a| a.as_reference().ok() == Some(widget)))
    })
}

/// Retire le champ `field` de l'arbre : des `Kids` de son parent, ou de la
/// liste des champs du formulaire.
pub(crate) fn detach_field(
    doc: &mut Document,
    form: ObjectId,
    field: ObjectId,
) -> Result<(), EngineError> {
    let parent = doc
        .get_dictionary(field)
        .ok()
        .and_then(|f| f.get(b"Parent").ok())
        .and_then(|p| p.as_reference().ok());
    match parent {
        Some(parent) => {
            remove_ref(doc, parent, b"Kids", field)?;
            let empty = doc
                .get_dictionary(parent)
                .ok()
                .and_then(|p| p.get(b"Kids").ok())
                .and_then(|k| doc.dereference(k).ok())
                .and_then(|(_, k)| k.as_array().ok().map(Vec::is_empty))
                .unwrap_or(true);
            // Un parent sans enfant ne décrit plus rien : il part aussi.
            if empty {
                detach_field(doc, form, parent)?;
            }
            Ok(())
        }
        None => remove_ref(doc, form, b"Fields", field),
    }
}

/// Retire l'annotation `widget` de sa page.
pub(crate) fn detach_widget(doc: &mut Document, widget: ObjectId) -> Result<(), EngineError> {
    if let Some(page) = page_of_widget(doc, widget) {
        remove_ref(doc, page, b"Annots", widget)?;
    }
    Ok(())
}

/// Pose l'annotation `widget` sur la page `page`.
pub(crate) fn attach_widget(
    doc: &mut Document,
    page: ObjectId,
    widget: ObjectId,
) -> Result<(), EngineError> {
    dict_mut(doc, widget)?.set("P", page);
    let annots = array_mut(doc, page, b"Annots")?;
    if !annots.iter().any(|a| a.as_reference().ok() == Some(widget)) {
        annots.push(Object::Reference(widget));
    }
    Ok(())
}

/// Rotation de la page (`/Rotate`, héritée), en degrés de 0 à 270.
pub(crate) fn page_rotation(doc: &Document, page: ObjectId) -> i64 {
    let mut current = Some(page);
    for _ in 0..32 {
        let Some(id) = current else {
            break;
        };
        let Ok(dict) = doc.get_dictionary(id) else {
            break;
        };
        if let Ok(rotate) = dict.get(b"Rotate").and_then(Object::as_i64) {
            return rotate.rem_euclid(360) / 90 * 90;
        }
        current = dict.get(b"Parent").and_then(Object::as_reference).ok();
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn document() -> (Document, ObjectId) {
        let mut doc = Document::with_version("1.7");
        let pages = doc.new_object_id();
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages, "Rotate" => 90,
        });
        doc.objects.insert(
            pages,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        doc.trailer.set("Root", catalog);
        (doc, page)
    }

    #[test]
    fn form_and_annotations_are_created_then_detached() {
        let (mut doc, page) = document();
        let form = ensure_acroform(&mut doc).unwrap();
        assert_eq!(ensure_acroform(&mut doc).unwrap(), form);
        let widget = doc.add_object(Dictionary::new());
        attach_widget(&mut doc, page, widget).unwrap();
        attach_widget(&mut doc, page, widget).unwrap();
        array_mut(&mut doc, form, b"Fields")
            .unwrap()
            .push(Object::Reference(widget));
        assert_eq!(page_of_widget(&doc, widget), Some(page));
        assert_eq!(page_rotation(&doc, page), 90);
        detach_widget(&mut doc, widget).unwrap();
        detach_field(&mut doc, form, widget).unwrap();
        assert_eq!(array_mut(&mut doc, page, b"Annots").unwrap().len(), 0);
        assert_eq!(array_mut(&mut doc, form, b"Fields").unwrap().len(), 0);
    }
}
