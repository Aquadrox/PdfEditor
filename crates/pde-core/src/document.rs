//! Description d'un document ouvert, indépendante du moteur PDF.

use crate::engine::EngineError;

/// Numéro de page à partir de 0 (l'interface affiche `index + 1`).
pub type PageIndex = usize;

/// Identifiant d'un document ouvert dans le moteur, unique pendant la session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(pub u64);

/// Taille affichée d'une page en points, rotation de la page déjà appliquée
/// (une page A4 tournée d'un quart de tour mesure 842 × 595).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    pub width: f64,
    pub height: f64,
}

impl PageSize {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    pub fn is_landscape(&self) -> bool {
        self.width > self.height
    }
}

/// Entrée du plan (signets) du document.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineItem {
    pub title: String,
    /// Page visée ; `None` si le signet ne mène pas à une page du document
    /// (lien externe, destination invalide).
    pub page: Option<PageIndex>,
    pub children: Vec<OutlineItem>,
}

/// Ce que l'interface doit savoir d'un document à son ouverture.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentInfo {
    /// Taille de chaque page, dans l'ordre.
    pub page_sizes: Vec<PageSize>,
    /// Titre des métadonnées, s'il est renseigné.
    pub title: Option<String>,
    pub outline: Vec<OutlineItem>,
}

impl DocumentInfo {
    pub fn page_count(&self) -> usize {
        self.page_sizes.len()
    }

    /// Taille de la page `page`, ou erreur si elle n'existe pas.
    pub fn page_size(&self, page: PageIndex) -> Result<PageSize, EngineError> {
        self.page_sizes
            .get(page)
            .copied()
            .ok_or(EngineError::PageOutOfRange {
                page,
                count: self.page_count(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> DocumentInfo {
        DocumentInfo {
            page_sizes: vec![PageSize::new(595.0, 842.0), PageSize::new(842.0, 595.0)],
            title: None,
            outline: Vec::new(),
        }
    }

    #[test]
    fn page_size_of_existing_pages() {
        let info = info();
        assert_eq!(info.page_count(), 2);
        assert!(!info.page_size(0).unwrap().is_landscape());
        assert!(info.page_size(1).unwrap().is_landscape());
    }

    #[test]
    fn page_size_out_of_range() {
        assert_eq!(
            info().page_size(2),
            Err(EngineError::PageOutOfRange { page: 2, count: 2 })
        );
    }
}
