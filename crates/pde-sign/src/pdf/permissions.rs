//! Signature de certification (DocMDP, ISO 32000 § 12.8.2.2) : le
//! catalogue désigne par `/Perms /DocMDP` la signature qui verrouille, et
//! son `/Reference` donne le niveau `/P`.

use lopdf::{Dictionary, Object, ObjectId};

use super::source::Source;

/// Dictionnaire `/Sig` désigné par `/Perms /DocMDP`, et sa permission `/P`.
pub(crate) fn certification(source: &Source) -> Option<(Option<ObjectId>, i64)> {
    let catalog = source.catalog().ok()?;
    let perms = source
        .resolve(catalog.get(b"Perms").ok()?)
        .ok()?
        .as_dict()
        .ok()?;
    let entry = perms.get(b"DocMDP").ok()?;
    let id = entry.as_reference().ok();
    let signature = source.resolve(entry).ok()?.as_dict().ok()?;
    Some((id, permission(source, signature).unwrap_or(2)))
}

/// `/P` de la référence DocMDP d'un dictionnaire `/Sig`, s'il en a une.
pub(crate) fn permission(source: &Source, signature: &Dictionary) -> Option<i64> {
    let references = source
        .resolve(signature.get(b"Reference").ok()?)
        .ok()?
        .as_array()
        .ok()?;
    references.iter().find_map(|reference| {
        let reference = source.resolve(reference).ok()?.as_dict().ok()?;
        if reference
            .get(b"TransformMethod")
            .and_then(Object::as_name)
            .ok()
            != Some(b"DocMDP")
        {
            return None;
        }
        let params = reference
            .get(b"TransformParams")
            .ok()
            .and_then(|p| source.resolve(p).ok())
            .and_then(|p| p.as_dict().ok());
        Some(
            params
                .and_then(|p| p.get(b"P").and_then(Object::as_i64).ok())
                .unwrap_or(2),
        )
    })
}
