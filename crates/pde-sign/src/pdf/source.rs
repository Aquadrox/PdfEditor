//! Le PDF à signer ou à vérifier : ses octets et sa structure lue par `lopdf`.

use lopdf::xref::XrefType;
use lopdf::{Dictionary, Document, EncryptionState, Object, ObjectId};

use super::incremental::{Trailer, XrefStyle};
use crate::error::{Result, SignError};

/// PDF lu, prêt à recevoir une mise à jour incrémentale.
pub(crate) struct Source<'a> {
    pub bytes: &'a [u8],
    pub document: Document,
}

impl<'a> Source<'a> {
    /// PDF lu ; un document protégé est déchiffré avec `password` (ou le
    /// mot de passe vide, pour un document protégé seulement contre la
    /// modification).
    pub fn load(bytes: &'a [u8], password: Option<&str>) -> Result<Self> {
        Ok(Source {
            bytes,
            document: open(bytes, password)?,
        })
    }

    /// Chiffrement du document, s'il est protégé.
    pub fn encryption(&self) -> Option<&EncryptionState> {
        self.document.encryption_state.as_ref()
    }

    /// Position de la dernière section de références croisées (`startxref`).
    pub fn last_xref(&self) -> Result<usize> {
        let tail_start = self.bytes.len().saturating_sub(2048);
        let tail = &self.bytes[tail_start..];
        let keyword = b"startxref";
        let at = tail
            .windows(keyword.len())
            .rposition(|window| window == keyword)
            .ok_or_else(|| SignError::Pdf("startxref introuvable".into()))?;
        let digits: String = tail[at + keyword.len()..]
            .iter()
            .map(|&b| b as char)
            .skip_while(|c| c.is_ascii_whitespace())
            .take_while(char::is_ascii_digit)
            .collect();
        digits
            .parse()
            .map_err(|_| SignError::Pdf("startxref illisible".into()))
    }

    pub fn xref_style(&self) -> XrefStyle {
        match self.document.reference_table.cross_reference_type {
            XrefType::CrossReferenceStream => XrefStyle::Stream,
            XrefType::CrossReferenceTable => XrefStyle::Table,
        }
    }

    /// Prochain numéro d'objet libre.
    pub fn next_id(&self) -> u32 {
        let declared = self
            .document
            .trailer
            .get(b"Size")
            .and_then(Object::as_i64)
            .ok()
            .and_then(|size| u32::try_from(size).ok())
            .unwrap_or(0);
        declared
            .max(self.document.max_id + 1)
            .max(self.document.reference_table.size)
    }

    pub fn root_id(&self) -> Result<ObjectId> {
        self.document
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|_| SignError::Pdf("catalogue introuvable".into()))
    }

    pub fn catalog(&self) -> Result<&Dictionary> {
        Ok(self.document.get_dictionary(self.root_id()?)?)
    }

    /// Le `trailer` de la mise à jour : reprise du précédent, second
    /// identifiant renouvelé.
    pub fn trailer(&self, new_id: [u8; 16]) -> Result<Trailer> {
        let trailer = &self.document.trailer;
        let info = trailer.get(b"Info").and_then(Object::as_reference).ok();
        let first = trailer
            .get(b"ID")
            .and_then(Object::as_array)
            .ok()
            .and_then(|ids| ids.first())
            .and_then(|first| first.as_str().ok())
            .map(<[u8]>::to_vec)
            .unwrap_or_else(|| new_id.to_vec());
        Ok(Trailer {
            prev: self.last_xref()?,
            size: self.next_id(),
            root: self.root_id()?,
            info,
            id: Some((first, new_id.to_vec())),
            encrypt: self
                .encryption()
                .and_then(|state| state.encrypt_object_id()),
        })
    }

    /// Chiffre `object` (objet `id`) comme le reste du document, s'il est protégé.
    pub fn encrypt(&self, id: ObjectId, object: Object) -> Result<Object> {
        encrypt(self.encryption(), id, object)
    }

    /// Objet désigné, référence suivie.
    pub fn resolve<'b>(&'b self, object: &'b Object) -> Result<&'b Object> {
        Ok(self.document.dereference(object)?.1)
    }

    /// Objet de la page `index` (à partir de 0).
    pub fn page_id(&self, index: usize) -> Result<ObjectId> {
        self.document
            .get_pages()
            .get(&(index as u32 + 1))
            .copied()
            .ok_or_else(|| SignError::Target(format!("la page {} n'existe pas", index + 1)))
    }

    /// Rang (à partir de 0) de la page d'objet `id`.
    pub fn page_index(&self, id: ObjectId) -> Option<usize> {
        self.document
            .get_pages()
            .into_iter()
            .find(|(_, page)| *page == id)
            .map(|(number, _)| number as usize - 1)
    }
}

/// Chiffre `object` (objet `id`) avec `state` ; les chaînes chiffrées sont
/// écrites en hexadécimal (octets quelconques, aucun échappement à risque).
pub(crate) fn encrypt(
    state: Option<&EncryptionState>,
    id: ObjectId,
    mut object: Object,
) -> Result<Object> {
    let Some(state) = state else {
        return Ok(object);
    };
    lopdf::encryption::encrypt_object(state, id, &mut object)
        .map_err(|e| SignError::Pdf(format!("chiffrement impossible ({e})")))?;
    hexadecimal(&mut object);
    Ok(object)
}

fn hexadecimal(object: &mut Object) {
    match object {
        Object::String(_, format) => *format = lopdf::StringFormat::Hexadecimal,
        Object::Array(items) => items.iter_mut().for_each(hexadecimal),
        Object::Dictionary(dict) => dict.iter_mut().for_each(|(_, v)| hexadecimal(v)),
        Object::Stream(stream) => stream.dict.iter_mut().for_each(|(_, v)| hexadecimal(v)),
        _ => {}
    }
}

/// Ouvre un PDF, déchiffré s'il est protégé.
pub(crate) fn open(bytes: &[u8], password: Option<&str>) -> Result<Document> {
    let document = match password {
        Some(password) => {
            Document::load_mem_with_options(bytes, lopdf::LoadOptions::with_password(password))
        }
        None => Document::load_mem(bytes),
    }
    .map_err(|error| match error {
        lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword => SignError::Encrypted,
        other => SignError::Pdf(other.to_string()),
    })?;
    // Toujours chiffré : le mot de passe manque ou ne convient pas.
    if document.is_encrypted() {
        return Err(SignError::Encrypted);
    }
    Ok(document)
}
