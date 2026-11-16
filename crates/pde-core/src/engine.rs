//! Port vers le moteur PDF : ce que le reste de l'application peut lui
//! demander, sans savoir qu'il s'agit de PDFium.

use thiserror::Error;

use crate::document::{DocumentId, DocumentInfo, PageIndex};
use crate::render::{Bitmap, RenderRequest};
use crate::text::PageText;

/// Erreurs du moteur, formulées pour être affichées à l'utilisateur.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EngineError {
    #[error("ce document est protégé par un mot de passe")]
    PasswordRequired,
    #[error("mot de passe incorrect")]
    WrongPassword,
    #[error("ce fichier n'est pas un PDF lisible ({0})")]
    InvalidFile(String),
    #[error("ce document n'est plus ouvert")]
    UnknownDocument,
    #[error("la page {} n'existe pas (le document en compte {count})", page + 1)]
    PageOutOfRange { page: PageIndex, count: usize },
    #[error("rendu impossible : {0}")]
    InvalidRender(String),
    #[error("moteur PDF indisponible : {0}")]
    Unavailable(String),
    #[error("erreur interne du moteur PDF : {0}")]
    Internal(String),
}

/// Document ouvert : son identifiant pour les demandes suivantes, et sa description.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenedDocument {
    pub id: DocumentId,
    pub info: DocumentInfo,
}

/// Moteur PDF partagé par toute l'application.
///
/// Les implémentations sont utilisables depuis n'importe quel thread ; un
/// moteur qui ne l'est pas (PDFium) sérialise lui-même les appels.
pub trait PdfEngine: Send + Sync {
    /// Ouvre un document à partir de son contenu.
    fn open(&self, bytes: Vec<u8>, password: Option<String>)
    -> Result<OpenedDocument, EngineError>;

    /// Rend une page en image.
    fn render(&self, doc: DocumentId, request: RenderRequest) -> Result<Bitmap, EngineError>;

    /// Texte d'une page, caractère par caractère, avec sa position.
    fn page_text(&self, doc: DocumentId, page: PageIndex) -> Result<PageText, EngineError>;

    /// Ferme le document et libère sa mémoire ; sans effet s'il est déjà fermé.
    fn close(&self, doc: DocumentId);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_numbers_are_shown_from_one() {
        let message = EngineError::PageOutOfRange { page: 4, count: 3 }.to_string();
        assert_eq!(message, "la page 5 n'existe pas (le document en compte 3)");
    }
}
