//! Le PDF à signer ou à vérifier : ses octets et sa structure lue par `lopdf`.

use lopdf::xref::XrefType;
use lopdf::{Dictionary, Document, Object, ObjectId};

use super::incremental::{Trailer, XrefStyle};
use crate::error::{Result, SignError};

/// PDF lu, prêt à recevoir une mise à jour incrémentale.
pub(crate) struct Source<'a> {
    pub bytes: &'a [u8],
    pub document: Document,
}

impl<'a> Source<'a> {
    pub fn load(bytes: &'a [u8]) -> Result<Self> {
        let document = Document::load_mem(bytes)?;
        if document.is_encrypted() || document.trailer.get(b"Encrypt").is_ok() {
            return Err(SignError::Encrypted);
        }
        Ok(Source { bytes, document })
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
        })
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
