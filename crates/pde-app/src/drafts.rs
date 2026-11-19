//! Documents sans titre (E-13) : nouveau document vierge, document Word
//! ou OpenDocument converti en PDF. Le PDF est rangé dans le dossier des
//! brouillons de l'application et ouvert de là ; « Enregistrer » demande
//! alors où l'écrire, et le brouillon disparaît à l'enregistrement ou à la
//! fermeture.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pde_core::document::PageSize;
use serde::Deserialize;

use crate::copies::LocalCopies;
use crate::error::{AppError, ErrorKind};

/// Pages au plus d'un nouveau document.
pub const MAX_NEW_PAGES: u32 = 200;
/// Pages au plus d'un document Word repris par l'import intégré.
pub const MAX_IMPORT_PAGES: u32 = pde_core::blank::MAX_BLANK_PAGES;

pub use pde_core::blank::blank_pdf;
/// Brouillons plus vieux que cela effacés au démarrage (un document sans
/// titre récupéré après un plantage reste proposé jusque-là).
pub const DRAFT_LIFETIME: Duration = Duration::from_secs(30 * 24 * 3600);

/// Formats de papier proposés.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paper {
    #[default]
    A4,
    A3,
    A5,
    Letter,
    Legal,
}

impl Paper {
    /// Taille en points, en portrait.
    pub fn size(self) -> PageSize {
        let mm = |v: f64| v * 72.0 / 25.4;
        match self {
            Paper::A4 => PageSize::new(mm(210.0), mm(297.0)),
            Paper::A3 => PageSize::new(mm(297.0), mm(420.0)),
            Paper::A5 => PageSize::new(mm(148.0), mm(210.0)),
            Paper::Letter => PageSize::new(612.0, 792.0),
            Paper::Legal => PageSize::new(612.0, 1008.0),
        }
    }
}

/// Nouveau document vierge demandé par l'interface.
#[derive(Debug, Clone, Deserialize)]
pub struct NewDocument {
    #[serde(default)]
    pub paper: Paper,
    #[serde(default)]
    pub landscape: bool,
    pub pages: u32,
}

impl NewDocument {
    pub fn page_size(&self) -> PageSize {
        let size = self.paper.size();
        if self.landscape {
            PageSize::new(size.height, size.width)
        } else {
            size
        }
    }
}

/// Dossier des brouillons.
pub struct Drafts {
    dir: PathBuf,
    files: LocalCopies,
}

impl Drafts {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            files: LocalCopies::new(dir),
        }
    }

    /// Range `bytes` sous un nom libre tiré de `name`.
    pub fn store(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, AppError> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| AppError::new(ErrorKind::Io, format!("dossier des brouillons : {e}")))?;
        self.files.store(name, bytes, None)
    }

    /// Le fichier est un brouillon de ce dossier.
    pub fn contains(&self, path: &Path) -> bool {
        path.parent() == Some(self.dir.as_path())
    }

    /// Efface un brouillon (enregistré ailleurs, ou abandonné).
    pub fn remove(&self, path: &Path) {
        if self.contains(path) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Efface les brouillons plus vieux que `lifetime`.
    pub fn purge(&self, lifetime: Duration) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let now = SystemTime::now();
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|m| now.duration_since(m).ok())
                .is_some_and(|age| age > lifetime);
            if old {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Nom PDF tiré d'un fichier importé (« bail.docx » → « bail.pdf »).
pub fn pdf_name(source: &Path) -> String {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Sans titre".into());
    format!("{stem}.pdf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn papers_and_orientation() {
        let a4 = Paper::A4.size();
        assert!((a4.width - 595.2756).abs() < 1e-3 && (a4.height - 841.8898).abs() < 1e-3);
        let landscape = NewDocument {
            paper: Paper::Letter,
            landscape: true,
            pages: 1,
        };
        assert_eq!(landscape.page_size(), PageSize::new(792.0, 612.0));
        let request: NewDocument =
            serde_json::from_value(serde_json::json!({ "pages": 2 })).unwrap();
        assert_eq!(request.paper, Paper::A4);
    }

    #[test]
    fn drafts_are_stored_named_and_removed() {
        let dir = std::env::temp_dir().join(format!("pde-drafts-{}", std::process::id()));
        let drafts = Drafts::new(&dir);
        let first = drafts.store("Sans titre.pdf", b"%PDF-a").unwrap();
        let second = drafts.store("Sans titre.pdf", b"%PDF-b").unwrap();
        assert_ne!(first, second);
        assert!(drafts.contains(&first));
        assert!(!drafts.contains(Path::new("/tmp/autre.pdf")));
        drafts.remove(&first);
        assert!(!first.exists());
        drafts.purge(Duration::ZERO);
        assert!(!second.exists());
        assert_eq!(pdf_name(Path::new("/docs/bail 2026.docx")), "bail 2026.pdf");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
