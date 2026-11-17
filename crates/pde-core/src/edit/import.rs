//! PDF insérés dans le document (P-03) : leur contenu, gardé en mémoire
//! pendant la session comme les images, et la taille de leurs pages.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::document::PageSize;

/// Identifiant d'un PDF inséré, unique dans le document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImportId(pub u64);

/// PDF inséré : son contenu (non protégé par mot de passe) et la taille
/// affichée de ses pages.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedPdf {
    pub bytes: Vec<u8>,
    pub page_sizes: Vec<PageSize>,
}

/// PDF insérés, partagés avec le moteur sans copie.
pub type Imports = BTreeMap<ImportId, Arc<ImportedPdf>>;

/// Taille des pages que le plan peut désigner : celles du document
/// d'origine et celles des PDF insérés.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageSources {
    pub originals: Vec<PageSize>,
    pub imported: BTreeMap<ImportId, Vec<PageSize>>,
}

impl PageSources {
    pub fn new(originals: Vec<PageSize>) -> Self {
        Self {
            originals,
            imported: BTreeMap::new(),
        }
    }

    /// Document d'origine et PDF insérés connus du moteur.
    pub fn with_imports(originals: Vec<PageSize>, imports: &Imports) -> Self {
        Self {
            originals,
            imported: imports
                .iter()
                .map(|(id, pdf)| (*id, pdf.page_sizes.clone()))
                .collect(),
        }
    }

    /// Taille d'une page d'un PDF inséré.
    pub fn imported_page(&self, file: ImportId, index: usize) -> Option<PageSize> {
        self.imported.get(&file)?.get(index).copied()
    }
}
