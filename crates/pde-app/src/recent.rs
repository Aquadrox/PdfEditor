//! Documents récents, affichés sur l'accueil (`recent.json`). Seuls les
//! chemins et dates d'ouverture sont gardés, jamais le contenu.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::files::write_atomic;
use crate::lock;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Entry {
    path: String,
    /// Secondes depuis le 1er janvier 1970.
    opened_at: u64,
}

/// Document récent tel que l'accueil l'affiche.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecentView {
    pub path: String,
    pub name: String,
    pub folder: String,
    pub opened_at: u64,
    /// Faux si le fichier a été déplacé ou supprimé depuis.
    pub exists: bool,
}

pub struct RecentFiles {
    path: PathBuf,
    entries: Mutex<Vec<Entry>>,
}

impl RecentFiles {
    /// Lit la liste ; fichier absent ou abîmé : liste vide.
    pub fn load(path: PathBuf) -> Self {
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }

    /// Note l'ouverture de `file` (en tête de liste, sans doublon), garde
    /// au plus `max` documents et enregistre.
    pub fn record(&self, file: &Path, now: u64, max: usize) -> Result<(), AppError> {
        let path = file.display().to_string();
        let mut entries = lock(&self.entries);
        entries.retain(|e| e.path != path);
        entries.insert(
            0,
            Entry {
                path,
                opened_at: now,
            },
        );
        entries.truncate(max);
        self.save(&entries)
    }

    pub fn clear(&self) -> Result<(), AppError> {
        let mut entries = lock(&self.entries);
        entries.clear();
        self.save(&entries)
    }

    /// Au plus `max` documents, du plus récent au plus ancien.
    pub fn list(&self, max: usize) -> Vec<RecentView> {
        lock(&self.entries)
            .iter()
            .take(max)
            .map(|e| {
                let path = Path::new(&e.path);
                RecentView {
                    name: path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| e.path.clone()),
                    folder: path
                        .parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    exists: path.is_file(),
                    opened_at: e.opened_at,
                    path: e.path.clone(),
                }
            })
            .collect()
    }

    fn save(&self, entries: &[Entry]) -> Result<(), AppError> {
        // Sérialiser une liste de chaînes et de nombres ne peut pas échouer.
        let json = serde_json::to_vec_pretty(entries).unwrap_or_default();
        write_atomic(&self.path, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);

    impl Temp {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("pde-recent-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, b"%PDF").unwrap();
            path
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn names(recent: &RecentFiles) -> Vec<String> {
        recent.list(50).into_iter().map(|r| r.name).collect()
    }

    #[test]
    fn most_recent_first_without_duplicates() {
        let temp = Temp::new("order");
        let recent = RecentFiles::load(temp.0.join("recent.json"));
        let (a, b) = (temp.file("a.pdf"), temp.file("b.pdf"));
        recent.record(&a, 1, 10).unwrap();
        recent.record(&b, 2, 10).unwrap();
        recent.record(&a, 3, 10).unwrap();
        assert_eq!(names(&recent), ["a.pdf", "b.pdf"]);
        let first = &recent.list(1)[0];
        assert_eq!(first.opened_at, 3);
        assert!(first.exists);
        assert_eq!(Path::new(&first.folder), temp.0);
    }

    #[test]
    fn list_is_limited_and_saved() {
        let temp = Temp::new("limit");
        let file = temp.0.join("recent.json");
        let recent = RecentFiles::load(file.clone());
        for (i, name) in ["a.pdf", "b.pdf", "c.pdf"].iter().enumerate() {
            recent.record(&temp.file(name), i as u64, 2).unwrap();
        }
        assert_eq!(names(&RecentFiles::load(file)), ["c.pdf", "b.pdf"]);
    }

    #[test]
    fn missing_files_are_flagged_and_list_can_be_cleared() {
        let temp = Temp::new("missing");
        let recent = RecentFiles::load(temp.0.join("recent.json"));
        let file = temp.file("parti.pdf");
        recent.record(&file, 1, 10).unwrap();
        std::fs::remove_file(&file).unwrap();
        assert!(!recent.list(10)[0].exists);
        recent.clear().unwrap();
        assert!(recent.list(10).is_empty());
    }

    #[test]
    fn damaged_file_gives_an_empty_list() {
        let temp = Temp::new("damaged");
        let file = temp.0.join("recent.json");
        std::fs::write(&file, "{pas du json").unwrap();
        assert!(RecentFiles::load(file).list(10).is_empty());
    }
}
