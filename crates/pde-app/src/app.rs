//! L'application vue par l'interface : visionneuse, réglages et documents
//! récents, et les règles qui les relient (un document ouvert entre dans
//! les récents, un réglage enregistré s'applique tout de suite).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use pde_core::engine::PdfEngine;
use pde_core::settings::Settings;

use crate::edit_view::SavedView;
use crate::error::AppError;
use crate::recent::{RecentFiles, RecentView};
use crate::recovery::RecoveryStore;
use crate::settings_store::{SettingsStore, SettingsView};
use crate::view::DocumentView;
use crate::viewer::Viewer;

pub struct App {
    pub viewer: Viewer,
    pub(crate) settings: SettingsStore,
    recent: RecentFiles,
    /// Sauvegarde automatique (F-02) ; absente si son dossier est inutilisable.
    pub(crate) recovery: Option<RecoveryStore>,
}

fn megabytes(mb: u32) -> usize {
    usize::try_from(mb)
        .unwrap_or(usize::MAX)
        .saturating_mul(1024 * 1024)
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

impl App {
    pub fn new(engine: Arc<dyn PdfEngine>, settings: SettingsStore, recent: RecentFiles) -> Self {
        let cache = megabytes(settings.get().display.page_cache_mb);
        Self {
            viewer: Viewer::new(engine, cache),
            settings,
            recent,
            recovery: None,
        }
    }

    /// Active la sauvegarde automatique dans `store`.
    pub fn with_recovery(mut self, store: RecoveryStore) -> Self {
        self.recovery = Some(store);
        self
    }

    pub fn settings(&self) -> SettingsView {
        self.settings.view()
    }

    /// Enregistre les réglages et les applique (mémoire des pages, liste des récents).
    pub fn save_settings(&self, settings: &Settings) -> Result<Settings, AppError> {
        let saved = self.settings.save(settings)?;
        self.viewer
            .set_cache_bytes(megabytes(saved.display.page_cache_mb));
        if saved.general.recent_files == 0 {
            self.recent.clear()?;
        }
        Ok(saved)
    }

    /// Ouvre un document et l'ajoute aux récents. Un échec d'écriture de la
    /// liste n'empêche pas l'ouverture : il est seulement journalisé.
    pub fn open_document(
        &self,
        path: &Path,
        password: Option<String>,
    ) -> Result<DocumentView, AppError> {
        let view = self.viewer.open_file(path, password)?;
        let max = self.settings.get().general.recent_files as usize;
        if max > 0
            && let Err(e) = self.recent.record(path, now(), max)
        {
            log::warn!("liste des documents récents non enregistrée : {e}");
        }
        Ok(view)
    }

    /// Enregistre ; « Enregistrer sous » (`target`) met la copie dans les récents.
    pub fn save_document(&self, id: u64, target: Option<PathBuf>) -> Result<SavedView, AppError> {
        let save_as = target.is_some();
        let saved = self.viewer.save(id, target)?;
        if let Some(recovery) = &self.recovery {
            recovery.remove(id);
        }
        let max = self.settings.get().general.recent_files as usize;
        if save_as
            && max > 0
            && let Err(e) = self.recent.record(Path::new(&saved.path), now(), max)
        {
            log::warn!("liste des documents récents non enregistrée : {e}");
        }
        Ok(saved)
    }

    pub fn recent_files(&self) -> Vec<RecentView> {
        self.recent
            .list(self.settings.get().general.recent_files as usize)
    }

    pub fn clear_recent_files(&self) -> Result<(), AppError> {
        self.recent.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeEngine, SIMPLE_PDF};
    use std::path::PathBuf;

    struct Fixture {
        dir: PathBuf,
        app: App,
    }

    impl Fixture {
        fn new(name: &str, settings_toml: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("pde-appsvc-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("config.toml"), settings_toml).unwrap();
            let app = App::new(
                Arc::new(FakeEngine::default()),
                SettingsStore::load(dir.join("config.toml")),
                RecentFiles::load(dir.join("recent.json")),
            );
            Self { dir, app }
        }

        fn pdf(&self, name: &str) -> PathBuf {
            let path = self.dir.join(name);
            std::fs::write(&path, SIMPLE_PDF).unwrap();
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn opened_documents_become_recent() {
        let f = Fixture::new("recent", "");
        f.app.open_document(&f.pdf("a.pdf"), None).unwrap();
        f.app.open_document(&f.pdf("b.pdf"), None).unwrap();
        let names: Vec<String> = f.app.recent_files().into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["b.pdf", "a.pdf"]);
    }

    #[test]
    fn recent_list_follows_the_setting() {
        let f = Fixture::new("limit", "[general]\nrecent_files = 1\n");
        f.app.open_document(&f.pdf("a.pdf"), None).unwrap();
        f.app.open_document(&f.pdf("b.pdf"), None).unwrap();
        assert_eq!(f.app.recent_files().len(), 1);

        let mut settings = f.app.settings().settings;
        settings.general.recent_files = 0;
        f.app.save_settings(&settings).unwrap();
        f.app.open_document(&f.pdf("c.pdf"), None).unwrap();
        assert!(f.app.recent_files().is_empty());
    }

    #[test]
    fn save_as_puts_the_copy_in_recent_files() {
        let f = Fixture::new("saveas", "");
        let doc = f.app.open_document(&f.pdf("a.pdf"), None).unwrap();
        f.app.save_document(doc.id, None).unwrap();
        assert_eq!(f.app.recent_files().len(), 1);
        f.app
            .save_document(doc.id, Some(f.dir.join("b.pdf")))
            .unwrap();
        let names: Vec<String> = f.app.recent_files().into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["b.pdf", "a.pdf"]);
    }

    #[test]
    fn failed_opening_is_not_recent() {
        let f = Fixture::new("failed", "");
        let path = f.dir.join("casse.pdf");
        std::fs::write(&path, b"texte").unwrap();
        assert!(f.app.open_document(&path, None).is_err());
        assert!(f.app.recent_files().is_empty());
    }
}
