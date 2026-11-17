//! Port vers le moteur PDF : ce que le reste de l'application peut lui
//! demander, sans savoir qu'il s'agit de PDFium.

use thiserror::Error;

use crate::document::{DocumentId, DocumentInfo, PageIndex};
use crate::edit::{Element, ElementId, Images, Imports, PagePlan};
use crate::geometry::Rect;
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

/// Élément posé par le moteur : sa place réelle sur la page affichée (la
/// hauteur d'une zone de texte dépend de ses lignes).
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub id: ElementId,
    pub bounds: Rect,
}

/// Image d'origine d'une page (E-06) : son rang parmi les objets de la
/// page arrangée, avant tout ajout, et sa place sur la page affichée.
#[derive(Debug, Clone, PartialEq)]
pub struct PageImage {
    pub object: u32,
    pub bounds: Rect,
}

/// Ce que le moteur applique au document d'origine : plan des pages,
/// éléments posés, contenu des images et des PDF insérés qu'ils désignent.
#[derive(Debug, Clone, Default)]
pub struct Edition {
    pub plan: PagePlan,
    pub elements: Vec<(ElementId, Element)>,
    pub images: Images,
    pub imports: Imports,
}

impl Edition {
    /// Plan et éléments, sans PDF inséré.
    pub fn new(plan: PagePlan, elements: Vec<(ElementId, Element)>, images: Images) -> Self {
        Self {
            plan,
            elements,
            images,
            imports: Imports::new(),
        }
    }
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

    /// Repart du document tel qu'ouvert, range ses pages selon le plan et
    /// y pose les éléments : l'affichage et l'enregistrement montrent
    /// ensuite exactement ce résultat.
    fn apply_edits(&self, doc: DocumentId, edition: Edition) -> Result<Vec<Placed>, EngineError>;

    /// Contenu d'un autre document tiré de l'original (extraire des pages) :
    /// le document affiché ne change pas.
    fn export(&self, doc: DocumentId, edition: Edition) -> Result<Vec<u8>, EngineError>;

    /// Images d'origine de la page `page` du document arrangé selon le plan
    /// de `edition` (ses éléments ne sont pas posés).
    fn page_images(
        &self,
        doc: DocumentId,
        edition: Edition,
        page: PageIndex,
    ) -> Result<Vec<PageImage>, EngineError>;

    /// Contenu du document dans son état actuel, prêt à écrire.
    fn save(&self, doc: DocumentId) -> Result<Vec<u8>, EngineError>;

    /// Ferme le document et libère sa mémoire ; sans effet s'il est déjà fermé.
    fn close(&self, doc: DocumentId);
}

/// Un moteur partagé par référence (moteur unique du processus) reste un moteur.
impl<T: PdfEngine + ?Sized> PdfEngine for &T {
    fn open(
        &self,
        bytes: Vec<u8>,
        password: Option<String>,
    ) -> Result<OpenedDocument, EngineError> {
        (**self).open(bytes, password)
    }

    fn render(&self, doc: DocumentId, request: RenderRequest) -> Result<Bitmap, EngineError> {
        (**self).render(doc, request)
    }

    fn page_text(&self, doc: DocumentId, page: PageIndex) -> Result<PageText, EngineError> {
        (**self).page_text(doc, page)
    }

    fn apply_edits(&self, doc: DocumentId, edition: Edition) -> Result<Vec<Placed>, EngineError> {
        (**self).apply_edits(doc, edition)
    }

    fn export(&self, doc: DocumentId, edition: Edition) -> Result<Vec<u8>, EngineError> {
        (**self).export(doc, edition)
    }

    fn page_images(
        &self,
        doc: DocumentId,
        edition: Edition,
        page: PageIndex,
    ) -> Result<Vec<PageImage>, EngineError> {
        (**self).page_images(doc, edition, page)
    }

    fn save(&self, doc: DocumentId) -> Result<Vec<u8>, EngineError> {
        (**self).save(doc)
    }

    fn close(&self, doc: DocumentId) {
        (**self).close(doc)
    }
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
