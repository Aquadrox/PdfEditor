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

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::error::{Result, SignError};
use crate::pdf::incremental::{Body, Update};
use crate::pdf::source::Source;

/// Plus gros flux décompressé pour la comparaison (au-delà : réécrit).
const MAX_COMPARED_STREAM: usize = 64 * 1024 * 1024;

/// `signed` suivi des changements de `rewritten` (même document réécrit
/// par le moteur avec des modifications). Rien à ajouter : `signed` tel quel.
pub fn append_revision(signed: &[u8], rewritten: &[u8]) -> Result<Vec<u8>> {
    let source = Source::load(signed)?;
    let after = Document::load_mem(rewritten)?;
    if after.is_encrypted() || after.trailer.get(b"Encrypt").is_ok() {
        return Err(SignError::Encrypted);
    }
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
    let written = update.write(signed, &trailer, source.xref_style())?;
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

/// Même valeur, quels que soient l'ordre des clés, l'écriture des nombres et
/// des chaînes, ou la compression des flux.
fn same(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Dictionary(x), Object::Dictionary(y)) => same_dict(x, y, &[]),
        (Object::Array(x), Object::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Object::Stream(x), Object::Stream(y)) => {
            let ignored: &[&[u8]] = &[b"Length", b"Filter", b"DecodeParms"];
            same_dict(&x.dict, &y.dict, ignored) && same_content(x, y)
        }
        (Object::String(x, _), Object::String(y, _)) => x == y,
        (Object::Integer(_) | Object::Real(_), Object::Integer(_) | Object::Real(_)) => {
            match (a.as_float(), b.as_float()) {
                (Ok(x), Ok(y)) => (x - y).abs() <= f32::EPSILON * x.abs().max(1.0),
                _ => false,
            }
        }
        _ => a == b,
    }
}

fn same_dict(x: &Dictionary, y: &Dictionary, ignored: &[&[u8]]) -> bool {
    let keys = |d: &Dictionary| -> BTreeSet<Vec<u8>> {
        d.iter()
            .map(|(k, _)| k.clone())
            .filter(|k| !ignored.contains(&k.as_slice()))
            .collect()
    };
    let keys_x = keys(x);
    keys_x == keys(y)
        && keys_x.iter().all(|k| match (x.get(k), y.get(k)) {
            (Ok(p), Ok(q)) => same(p, q),
            _ => false,
        })
}

fn same_content(x: &lopdf::Stream, y: &lopdf::Stream) -> bool {
    let filters = |s: &lopdf::Stream| s.dict.get(b"Filter").ok().cloned();
    if filters(x) == filters(y)
        && x.dict.get(b"DecodeParms").ok() == y.dict.get(b"DecodeParms").ok()
    {
        return x.content == y.content;
    }
    match (
        x.decompressed_content_with_limit(MAX_COMPARED_STREAM),
        y.decompressed_content_with_limit(MAX_COMPARED_STREAM),
    ) {
        (Ok(p), Ok(q)) => p == q,
        // Filtre inconnu (image JPEG…) : considéré comme changé, réécrit.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use lopdf::{StringFormat, dictionary};

    use super::*;

    #[test]
    fn key_order_and_number_writing_do_not_matter() {
        let a = Object::Dictionary(dictionary! { "A" => 1, "B" => Object::Real(2.0) });
        let b = Object::Dictionary(dictionary! { "B" => 2, "A" => Object::Real(1.0) });
        assert!(same(&a, &b));
        let c = Object::Dictionary(dictionary! { "A" => 1, "B" => 3 });
        assert!(!same(&a, &c));
        assert!(same(
            &Object::String(b"x".to_vec(), StringFormat::Literal),
            &Object::String(b"x".to_vec(), StringFormat::Hexadecimal)
        ));
    }

    #[test]
    fn recompressed_streams_are_the_same() {
        // Assez long pour que la compression soit appliquée.
        let plain = lopdf::Stream::new(Dictionary::new(), b"BT (a) Tj ET\n".repeat(50));
        let mut packed = plain.clone();
        packed.compress().unwrap();
        assert!(packed.dict.has(b"Filter"));
        assert!(same(
            &Object::Stream(plain.clone()),
            &Object::Stream(packed)
        ));
        let other = lopdf::Stream::new(Dictionary::new(), b"BT (b) Tj ET".to_vec());
        assert!(!same(&Object::Stream(plain), &Object::Stream(other)));
    }
}
