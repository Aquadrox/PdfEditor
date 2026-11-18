//! Comparer deux objets PDF sur le fond : même valeur quels que soient
//! l'ordre des clés, l'écriture des nombres et des chaînes, ou la
//! compression des flux. Sert à l'enregistrement des documents signés
//! (F-03) et à l'analyse des modifications faites après une signature.

use std::collections::BTreeSet;

use lopdf::{Dictionary, Object};

/// Plus gros flux décompressé pour la comparaison (au-delà : différent).
const MAX_COMPARED_STREAM: usize = 64 * 1024 * 1024;

/// Même valeur, quels que soient l'ordre des clés, l'écriture des nombres et
/// des chaînes, ou la compression des flux.
pub(crate) fn same(a: &Object, b: &Object) -> bool {
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

pub(crate) fn same_dict(x: &Dictionary, y: &Dictionary, ignored: &[&[u8]]) -> bool {
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
