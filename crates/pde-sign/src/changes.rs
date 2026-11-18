//! Ce qui a changé dans le document après une signature (S-07) : la
//! révision signée (les octets couverts par `/ByteRange`) est comparée au
//! document complet, objet par objet et sur le fond. Chaque changement est
//! classé (formulaire, signature, annotation, contenu des pages…) puis
//! confronté au verrouillage DocMDP de la signature de certification.

use std::collections::{BTreeSet, HashSet};

use lopdf::{Document, Object, ObjectId};

use crate::pdf::compare::{same, same_dict};
use crate::pdf::sig_dict::Lock;

/// Genre de modification faite après une signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Change {
    /// Valeurs des champs du formulaire.
    FormFill,
    /// Nouvelle signature (champ, widget, dictionnaire).
    Signature,
    /// Annotations ajoutées ou modifiées (notes, surlignages, dessins).
    Annotation,
    /// Contenu des pages (texte, images, tracés) ou leurs attributs.
    PageContent,
    /// Pages ajoutées, retirées ou réordonnées.
    Pages,
    /// Propriétés du document (titre, auteur…).
    Metadata,
    /// Autre changement de structure.
    Other,
}

impl Change {
    /// Modification permise par une certification de niveau `lock`.
    pub fn permitted_by(self, lock: Lock) -> bool {
        match lock {
            Lock::None => true,
            Lock::FormsAndSignatures => matches!(self, Change::FormFill | Change::Signature),
            Lock::Everything => false,
        }
    }
}

/// Changements entre la révision signée `before` et le document `after`.
pub(crate) fn between(before: &Document, after: &Document) -> BTreeSet<Change> {
    let context = Context::of(before, after);
    let mut changes = BTreeSet::new();
    for (id, object) in &after.objects {
        if context.structural(object) {
            continue;
        }
        match before.objects.get(id) {
            Some(old) if same(old, object) => {}
            Some(old) => {
                if let Some(change) = context.classify_changed(*id, old, object) {
                    changes.insert(change);
                }
            }
            None => {
                if let Some(change) = context.classify_new(object) {
                    changes.insert(change);
                }
            }
        }
    }
    // Pages retirées : la liste des pages a changé.
    if before.get_pages().len() != after.get_pages().len() {
        changes.insert(Change::Pages);
    }
    changes
}

/// Ce qu'on sait des objets de la révision signée.
struct Context<'a> {
    after: &'a Document,
    root: Option<ObjectId>,
    info: Option<ObjectId>,
    acro_form: Option<ObjectId>,
    pages: HashSet<ObjectId>,
    contents: HashSet<ObjectId>,
}

impl<'a> Context<'a> {
    fn of(before: &Document, after: &'a Document) -> Self {
        let reference =
            |doc: &Document, key: &[u8]| doc.trailer.get(key).and_then(Object::as_reference).ok();
        let root = reference(after, b"Root");
        let acro_form = root
            .and_then(|r| after.get_dictionary(r).ok())
            .and_then(|c| c.get(b"AcroForm").and_then(Object::as_reference).ok());
        let pages: HashSet<ObjectId> = before.get_pages().into_values().collect();
        let contents = pages
            .iter()
            .filter_map(|p| before.get_dictionary(*p).ok())
            .filter_map(|page| page.get(b"Contents").ok())
            .flat_map(|contents| match contents {
                Object::Reference(id) => vec![*id],
                Object::Array(items) => {
                    items.iter().filter_map(|i| i.as_reference().ok()).collect()
                }
                _ => Vec::new(),
            })
            .collect();
        Context {
            after,
            root,
            info: reference(after, b"Info"),
            acro_form,
            pages,
            contents,
        }
    }

    /// Flux d'objets et de références croisées : structure du fichier.
    fn structural(&self, object: &Object) -> bool {
        let Object::Stream(stream) = object else {
            return false;
        };
        matches!(
            stream.dict.get(b"Type").and_then(Object::as_name),
            Ok(b"ObjStm" | b"XRef")
        )
    }

    /// Objet modifié ; `None` pour un simple conteneur (formulaire,
    /// catalogue) dont le changement découle d'un autre, classé à part.
    fn classify_changed(&self, id: ObjectId, old: &Object, new: &Object) -> Option<Change> {
        if is_signature_object(self.after, new) {
            return Some(Change::Signature);
        }
        if Some(id) == self.info {
            return Some(Change::Metadata);
        }
        if Some(id) == self.acro_form {
            return None;
        }
        let (Ok(old_dict), Ok(new_dict)) = (dict_of(old), dict_of(new)) else {
            return self.classify_array_or_stream(id, new);
        };
        if Some(id) == self.root {
            let ignored: &[&[u8]] = &[b"AcroForm", b"Perms", b"DSS", b"NeedsRendering"];
            return (!same_dict(old_dict, new_dict, ignored)).then_some(Change::Other);
        }
        if self.pages.contains(&id) {
            if !same_dict(old_dict, new_dict, &[b"Annots"]) {
                return Some(Change::PageContent);
            }
            return Some(
                self.added_annotations(old_dict.get(b"Annots").ok(), new_dict.get(b"Annots").ok()),
            );
        }
        let type_name = new_dict.get(b"Type").and_then(Object::as_name).ok();
        if type_name == Some(b"Pages") {
            return Some(Change::Pages);
        }
        if is_field(new_dict) {
            // Seules la valeur et son apparence changent : remplissage.
            let fill: &[&[u8]] = &[b"V", b"AS", b"AP", b"MK"];
            return Some(if same_dict(old_dict, new_dict, fill) {
                Change::FormFill
            } else {
                Change::Other
            });
        }
        if type_name == Some(b"Annot") || new_dict.has(b"Subtype") && new_dict.has(b"Rect") {
            return Some(Change::Annotation);
        }
        Some(Change::Other)
    }

    fn classify_array_or_stream(&self, id: ObjectId, new: &Object) -> Option<Change> {
        if self.contents.contains(&id) {
            return Some(Change::PageContent);
        }
        if let Object::Array(items) = new {
            // Tableau des annotations d'une page, ou des champs du formulaire.
            let annots_of_page = self.after.get_pages().into_values().any(|page| {
                self.after
                    .get_dictionary(page)
                    .ok()
                    .and_then(|p| p.get(b"Annots").and_then(Object::as_reference).ok())
                    == Some(id)
            });
            if annots_of_page {
                return Some(if items.iter().all(|i| self.is_signature_reference(i)) {
                    Change::Signature
                } else {
                    Change::Annotation
                });
            }
            let fields_of_form = self
                .acro_form
                .and_then(|f| self.after.get_dictionary(f).ok())
                .and_then(|f| f.get(b"Fields").and_then(Object::as_reference).ok())
                == Some(id);
            if fields_of_form {
                return None;
            }
        }
        Some(Change::Other)
    }

    /// `/Annots` d'une page : les références ajoutées sont-elles des signatures ?
    fn added_annotations(&self, old: Option<&Object>, new: Option<&Object>) -> Change {
        let refs = |o: Option<&Object>| -> HashSet<ObjectId> {
            o.and_then(|o| o.as_array().ok())
                .map(|a| a.iter().filter_map(|i| i.as_reference().ok()).collect())
                .unwrap_or_default()
        };
        let before = refs(old);
        let added: Vec<ObjectId> = refs(new).difference(&before).copied().collect();
        if added
            .iter()
            .all(|id| self.is_signature_reference(&Object::Reference(*id)))
        {
            Change::Signature
        } else {
            Change::Annotation
        }
    }

    fn is_signature_reference(&self, item: &Object) -> bool {
        item.as_reference()
            .ok()
            .and_then(|id| self.after.get_object(id).ok())
            .is_some_and(|o| is_signature_object(self.after, o))
    }

    /// Objet nouveau : seuls les signatures et annotations comptent (polices,
    /// images et apparences ne valent que par ce qui les utilise).
    fn classify_new(&self, object: &Object) -> Option<Change> {
        if is_signature_object(self.after, object) {
            return Some(Change::Signature);
        }
        let dict = dict_of(object).ok()?;
        if is_field(dict) {
            return Some(Change::FormFill);
        }
        let annotation = dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Annot")
            || (dict.has(b"Subtype") && dict.has(b"Rect") && !dict.has(b"BBox"));
        annotation.then_some(Change::Annotation)
    }
}

fn dict_of(object: &Object) -> lopdf::Result<&lopdf::Dictionary> {
    match object {
        Object::Stream(stream) => Ok(&stream.dict),
        other => other.as_dict(),
    }
}

/// Dictionnaire `/Sig`, ou champ / widget de signature.
fn is_signature_object(doc: &Document, object: &Object) -> bool {
    let Ok(dict) = object.as_dict() else {
        return false;
    };
    if dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Sig") || dict.has(b"ByteRange") {
        return true;
    }
    let own = dict.get(b"FT").and_then(Object::as_name).ok();
    let parent = dict
        .get(b"Parent")
        .and_then(Object::as_reference)
        .ok()
        .and_then(|p| doc.get_dictionary(p).ok())
        .and_then(|p| p.get(b"FT").and_then(Object::as_name).ok());
    own.or(parent) == Some(b"Sig")
}

/// Champ de formulaire (ou widget fusionné à son champ).
fn is_field(dict: &lopdf::Dictionary) -> bool {
    dict.has(b"FT")
        || (dict.has(b"T") && (dict.has(b"Kids") || dict.has(b"V")))
        || dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Widget")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_follow_doc_mdp() {
        assert!(Change::PageContent.permitted_by(Lock::None));
        assert!(Change::FormFill.permitted_by(Lock::FormsAndSignatures));
        assert!(Change::Signature.permitted_by(Lock::FormsAndSignatures));
        assert!(!Change::Annotation.permitted_by(Lock::FormsAndSignatures));
        assert!(!Change::Signature.permitted_by(Lock::Everything));
    }
}
