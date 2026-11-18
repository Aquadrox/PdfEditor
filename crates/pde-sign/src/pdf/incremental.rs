//! Mise à jour incrémentale (ISO 32000, § 7.5.6) : les objets nouveaux ou
//! modifiés, une section de références croisées et un `trailer` sont ajoutés
//! **à la fin** du fichier, sans toucher un octet de l'existant. C'est ce qui
//! garde valides les signatures déjà présentes.
//!
//! La section suit le style du fichier : table `xref` classique, ou flux de
//! références croisées (PDF 1.5+) si le fichier en utilise un.

use std::collections::BTreeMap;

use lopdf::{Dictionary, EncryptionState, Object, ObjectId, Writer};

use crate::error::Result;

/// Contenu d'un objet écrit par la mise à jour.
pub(crate) enum Body {
    /// Objet sérialisé par `lopdf`.
    Object(Object),
    /// Octets écrits tels quels (dictionnaire de signature à trous réservés).
    Raw(Vec<u8>),
}

/// Forme de la section de références croisées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XrefStyle {
    Table,
    Stream,
}

/// Ce que le nouveau `trailer` reprend du précédent.
pub(crate) struct Trailer {
    /// Position de la section précédente (`/Prev`).
    pub prev: usize,
    /// Plus grand numéro d'objet + 1 (`/Size`), avant les nouveaux objets.
    pub size: u32,
    pub root: ObjectId,
    pub info: Option<ObjectId>,
    /// Identifiant du fichier : le premier reste, le second change.
    pub id: Option<(Vec<u8>, Vec<u8>)>,
    /// Dictionnaire de chiffrement d'un document protégé (inchangé).
    pub encrypt: Option<ObjectId>,
}

/// Objets de la mise à jour, dans l'ordre d'écriture.
#[derive(Default)]
pub(crate) struct Update {
    objects: Vec<(ObjectId, Body)>,
}

/// Fichier complet et position de chaque objet écrit.
pub(crate) struct Written {
    pub bytes: Vec<u8>,
    /// Début du corps de chaque objet (après `n g obj\n`).
    pub bodies: BTreeMap<ObjectId, usize>,
}

impl Update {
    pub fn push(&mut self, id: ObjectId, body: Body) {
        self.objects.retain(|(existing, _)| *existing != id);
        self.objects.push((id, body));
    }

    /// Ajoute la mise à jour à `source`.
    /// Ajoute la mise à jour à `source` ; les objets sont chiffrés avec
    /// `encryption` si le document est protégé.
    pub fn write(
        self,
        source: &[u8],
        trailer: &Trailer,
        style: XrefStyle,
        encryption: Option<&EncryptionState>,
    ) -> Result<Written> {
        let mut out = Vec::with_capacity(source.len() + 16 * 1024);
        out.extend_from_slice(source);
        if !matches!(source.last(), Some(b'\n' | b'\r')) {
            out.push(b'\n');
        }
        let mut offsets = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        let mut size = trailer.size;
        for (id, body) in &self.objects {
            offsets.insert(id.0, (out.len(), id.1));
            out.extend_from_slice(format!("{} {} obj\n", id.0, id.1).as_bytes());
            bodies.insert(*id, out.len());
            match body {
                Body::Object(object) => {
                    let object = super::source::encrypt(encryption, *id, object.clone())?;
                    Writer::write_object(&mut out, &object)?;
                }
                Body::Raw(bytes) => out.extend_from_slice(bytes),
            }
            out.extend_from_slice(b"\nendobj\n");
            size = size.max(id.0 + 1);
        }
        match style {
            XrefStyle::Table => write_table(&mut out, &offsets, trailer, size)?,
            XrefStyle::Stream => write_stream(&mut out, &mut offsets, trailer, size)?,
        }
        Ok(Written { bytes: out, bodies })
    }
}

fn trailer_dictionary(trailer: &Trailer, size: u32) -> Dictionary {
    let mut dictionary = Dictionary::new();
    dictionary.set("Size", i64::from(size));
    dictionary.set("Root", trailer.root);
    if let Some(info) = trailer.info {
        dictionary.set("Info", info);
    }
    if let Some((first, second)) = &trailer.id {
        dictionary.set(
            "ID",
            vec![
                Object::String(first.clone(), lopdf::StringFormat::Hexadecimal),
                Object::String(second.clone(), lopdf::StringFormat::Hexadecimal),
            ],
        );
    }
    if let Some(encrypt) = trailer.encrypt {
        dictionary.set("Encrypt", encrypt);
    }
    dictionary.set("Prev", trailer.prev as i64);
    dictionary
}

/// Numéros consécutifs regroupés : `(premier, nombre)`.
fn subsections(numbers: impl Iterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut groups: Vec<(u32, u32)> = Vec::new();
    for number in numbers {
        match groups.last_mut() {
            Some((first, count)) if *first + *count == number => *count += 1,
            _ => groups.push((number, 1)),
        }
    }
    groups
}

fn write_table(
    out: &mut Vec<u8>,
    offsets: &BTreeMap<u32, (usize, u16)>,
    trailer: &Trailer,
    size: u32,
) -> Result<()> {
    let start = out.len();
    out.extend_from_slice(b"xref\n");
    for (first, count) in subsections(offsets.keys().copied()) {
        out.extend_from_slice(format!("{first} {count}\n").as_bytes());
        for number in first..first + count {
            let (offset, generation) = offsets[&number];
            // Chaque entrée fait exactement 20 octets.
            out.extend_from_slice(format!("{offset:010} {generation:05} n\r\n").as_bytes());
        }
    }
    out.extend_from_slice(b"trailer\n");
    Writer::write_object(out, &Object::Dictionary(trailer_dictionary(trailer, size)))?;
    out.extend_from_slice(format!("\nstartxref\n{start}\n%%EOF\n").as_bytes());
    Ok(())
}

fn write_stream(
    out: &mut Vec<u8>,
    offsets: &mut BTreeMap<u32, (usize, u16)>,
    trailer: &Trailer,
    size: u32,
) -> Result<()> {
    // Le flux est lui-même un objet de la section.
    let own = size;
    let start = out.len();
    offsets.insert(own, (start, 0));
    let size = own + 1;
    let width = offset_width(start);
    let mut index = Vec::new();
    let mut data = Vec::new();
    for (first, count) in subsections(offsets.keys().copied()) {
        index.push(Object::Integer(i64::from(first)));
        index.push(Object::Integer(i64::from(count)));
        for number in first..first + count {
            let (offset, generation) = offsets[&number];
            data.push(1u8);
            data.extend_from_slice(&(offset as u64).to_be_bytes()[8 - width..]);
            data.extend_from_slice(&generation.to_be_bytes());
        }
    }
    let mut dictionary = trailer_dictionary(trailer, size);
    dictionary.set("Type", Object::Name(b"XRef".to_vec()));
    dictionary.set(
        "W",
        vec![
            Object::Integer(1),
            Object::Integer(width as i64),
            Object::Integer(2),
        ],
    );
    dictionary.set("Index", index);
    dictionary.set("Length", data.len() as i64);
    out.extend_from_slice(format!("{own} 0 obj\n").as_bytes());
    Writer::write_object(out, &Object::Dictionary(dictionary))?;
    out.extend_from_slice(b"\nstream\n");
    out.extend_from_slice(&data);
    out.extend_from_slice(b"\nendstream\nendobj\n");
    out.extend_from_slice(format!("startxref\n{start}\n%%EOF\n").as_bytes());
    Ok(())
}

/// Octets nécessaires pour écrire une position (au moins 4).
fn offset_width(largest: usize) -> usize {
    let bits = usize::BITS - largest.leading_zeros();
    (bits.div_ceil(8) as usize).max(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_numbers_share_a_subsection() {
        assert_eq!(
            subsections([3, 4, 5, 9, 11, 12].into_iter()),
            vec![(3, 3), (9, 1), (11, 2)]
        );
        assert_eq!(subsections(std::iter::empty()), vec![]);
    }

    #[test]
    fn offsets_use_at_least_four_bytes() {
        assert_eq!(offset_width(10), 4);
        assert_eq!(offset_width(u32::MAX as usize), 4);
        assert_eq!(offset_width(u32::MAX as usize + 1), 5);
    }
}
