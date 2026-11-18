//! Enregistrer un document signé sans invalider ses signatures (F-03).
//!
//! Le moteur PDF réécrit tout le document. On compare ce fichier réécrit au
//! fichier signé, objet par objet et **sur le fond** (ordre des clés et
//! compression des flux ignorés), puis on ajoute à la fin du fichier signé
//! une mise à jour incrémentale qui ne contient que les objets changés ou
//! nouveaux. Les octets signés ne bougent pas ; les dictionnaires de
//! signature existants ne sont jamais réécrits.
//!
//! PDFium garde les numéros des objets qu'il réécrit : un objet de même
//! numéro est le même objet, éventuellement modifié.

use std::collections::BTreeSet;

use lopdf::{Object, ObjectId};

use crate::error::{Result, SignError};
use crate::pdf::compare::same;
use crate::pdf::incremental::{Body, Update};
use crate::pdf::source::{Source, open};

/// `signed` suivi des changements de `rewritten` (même document réécrit
/// par le moteur avec des modifications). Rien à ajouter : `signed` tel quel.
/// `password` : celui d'un document protégé (les deux fichiers le sont).
pub fn append_revision(signed: &[u8], rewritten: &[u8], password: Option<&str>) -> Result<Vec<u8>> {
    let source = Source::load(signed, password)?;
    let after = open(rewritten, password)?;
    let protected = signature_dictionaries(&source);
    let mut update = Update::default();
    let mut changed = 0usize;
    for (&id, object) in &after.objects {
        if protected.contains(&id) || is_structural(object) {
            continue;
        }
        let unchanged = source
            .document
            .objects
            .get(&id)
            .is_some_and(|before| same(before, object));
        if !unchanged {
            update.push(id, Body::Object(object.clone()));
            changed += 1;
        }
    }
    let root = after
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|_| SignError::Pdf("catalogue introuvable".into()))?;
    if changed == 0 && root == source.root_id()? {
        return Ok(signed.to_vec());
    }
    let mut trailer = source.trailer(crate::rng::bytes::<16>()?)?;
    trailer.root = root;
    trailer.info = after
        .trailer
        .get(b"Info")
        .and_then(Object::as_reference)
        .ok();
    trailer.size = trailer.size.max(after.max_id + 1);
    let written = update.write(signed, &trailer, source.xref_style(), source.encryption())?;
    Ok(written.bytes)
}

/// Dictionnaires `/Sig` du fichier signé : jamais remplacés.
fn signature_dictionaries(source: &Source) -> BTreeSet<ObjectId> {
    source
        .document
        .objects
        .iter()
        .filter(|(_, object)| {
            object
                .as_dict()
                .ok()
                .and_then(|d| d.get(b"Type").and_then(Object::as_name).ok())
                == Some(b"Sig")
                || object
                    .as_dict()
                    .is_ok_and(|d| d.has(b"ByteRange") && d.has(b"Contents"))
        })
        .map(|(&id, _)| id)
        .collect()
}

/// Flux d'objets et flux de références croisées : structure du fichier
/// réécrit, recréée par notre propre section.
fn is_structural(object: &Object) -> bool {
    let Object::Stream(stream) = object else {
        return false;
    };
    matches!(
        stream.dict.get(b"Type").and_then(Object::as_name),
        Ok(b"ObjStm" | b"XRef")
    )
}
