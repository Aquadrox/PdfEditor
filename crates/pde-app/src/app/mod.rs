//! L'application vue par l'interface : visionneuse, réglages et documents
//! récents, et les règles qui les relient (un document ouvert entre dans
//! les récents, un réglage enregistré s'applique tout de suite).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

mod ai;

use pde_core::engine::PdfEngine;
use pde_core::settings::Settings;

use crate::ai::AiService;
use crate::edit_view::SavedView;
use crate::error::AppError;
use crate::error::ErrorKind;
use crate::files::{copy_name, extract_name};
use crate::recent::{RecentFiles, RecentView};
use crate::recipes::RecipeStore;
use crate::recovery::RecoveryStore;
use crate::settings_store::{SettingsStore, SettingsView};
use crate::signing::HttpTimestampAuthority;
use crate::signing::drawings::png_from_base64;
use crate::signing::{DrawingStore, DrawingView, IdentityStore, IdentityView, SignRequest};
use crate::view::DocumentView;
use crate::viewer::Viewer;
use pde_sign::Subject;

pub struct App {
    pub viewer: Viewer,
    pub(crate) settings: SettingsStore,
    recent: RecentFiles,
    /// Sauvegarde automatique (F-02) ; absente si son dossier est inutilisable.
    pub(crate) recovery: Option<RecoveryStore>,
    /// Identités de signature ; absentes tant que le dossier n'est pas connu.
    identities: Option<IdentityStore>,
    /// Signatures manuscrites enregistrées (« Mes signatures »).
    drawings: Option<DrawingStore>,
    /// IA (v0.5) ; absente tant qu'elle n'est pas branchée.
    ai: Option<AiService>,
    /// Recettes de l'assistant (IA-16).
    recipes: Option<RecipeStore>,
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
            identities: None,
            drawings: None,
            ai: None,
            recipes: None,
        }
    }

    /// Identités de signature et signatures manuscrites rangées dans `data_dir`.
    pub fn with_identities(mut self, store: IdentityStore, drawings: DrawingStore) -> Self {
        self.identities = Some(store);
        self.drawings = Some(drawings);
        self
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

    /// Dossier des dialogues Ouvrir et Ajouter une image (réglage), s'il existe.
    pub fn dialog_dir(&self) -> Option<PathBuf> {
        let dir = PathBuf::from(self.settings.get().files.default_dir);
        (!dir.as_os_str().is_empty() && dir.is_dir()).then_some(dir)
    }

    /// « Enregistrer sous » : dossier du document et nom de copie proposé.
    pub fn save_as_proposal(&self, id: u64) -> Result<(Option<PathBuf>, String), AppError> {
        let path = self.viewer.document_path(id)?;
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = copy_name(&file_name, &self.settings.get().files.copy_suffix);
        Ok((path.parent().map(Path::to_path_buf), name))
    }

    fn identity_store(&self) -> Result<&IdentityStore, AppError> {
        self.identities.as_ref().ok_or_else(|| {
            AppError::new(ErrorKind::Signature, "identités de signature indisponibles")
        })
    }

    /// Identités de signature enregistrées.
    pub fn identities(&self) -> Vec<IdentityView> {
        self.identities
            .as_ref()
            .map(IdentityStore::list)
            .unwrap_or_default()
    }

    /// Nouvelle identité au certificat auto-signé (S-01).
    pub fn create_identity(&self, subject: &Subject) -> Result<IdentityView, AppError> {
        self.identity_store()?.create(subject, now())
    }

    /// Importe un certificat PKCS#12 (S-02).
    pub fn import_identity(&self, path: &Path, password: &str) -> Result<IdentityView, AppError> {
        self.identity_store()?.import(path, password)
    }

    pub fn delete_identity(&self, id: &str) -> Result<(), AppError> {
        self.identity_store()?.delete(id)
    }

    /// Signatures manuscrites enregistrées.
    pub fn drawings(&self) -> Vec<DrawingView> {
        self.drawings
            .as_ref()
            .map(DrawingStore::list)
            .unwrap_or_default()
    }

    /// Enregistre une signature manuscrite (PNG en base64).
    pub fn save_drawing(&self, png: &str) -> Result<DrawingView, AppError> {
        let store = self.drawings.as_ref().ok_or_else(|| {
            AppError::new(
                ErrorKind::Signature,
                "signatures enregistrées indisponibles",
            )
        })?;
        store.save(&png_from_base64(png)?)
    }

    pub fn delete_drawing(&self, id: &str) -> Result<(), AppError> {
        match &self.drawings {
            Some(store) => store.delete(id),
            None => Ok(()),
        }
    }

    /// Signe le document avec l'identité demandée (S-05, S-06).
    pub fn sign_document(&self, id: u64, request: &SignRequest) -> Result<SavedView, AppError> {
        let identity = self.identity_store()?.load(&request.identity)?;
        let tsa = if request.timestamp {
            let url = self.settings.get().signature.timestamp_url;
            if url.is_empty() {
                return Err(AppError::new(
                    ErrorKind::Signature,
                    "aucun serveur d'horodatage dans les réglages (rubrique Signature)",
                ));
            }
            Some(HttpTimestampAuthority::new(&url).map_err(|e| {
                AppError::new(ErrorKind::Signature, format!("serveur d'horodatage : {e}"))
            })?)
        } else {
            None
        };
        let saved = self.viewer.sign(
            id,
            &identity,
            request,
            now(),
            tsa.as_ref().map(|t| t as &dyn pde_sign::TimestampAuthority),
        )?;
        if let Some(recovery) = &self.recovery {
            recovery.remove(id);
        }
        Ok(saved)
    }

    /// Extraire des pages : dossier du document et nom proposé.
    pub fn extract_proposal(
        &self,
        id: u64,
        pages: &[usize],
    ) -> Result<(Option<PathBuf>, String), AppError> {
        let path = self.viewer.document_path(id)?;
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok((
            path.parent().map(Path::to_path_buf),
            extract_name(&file_name, pages),
        ))
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

    #[test]
    fn save_as_proposes_a_copy_next_to_the_document() {
        let f = Fixture::new("copie", "[files]\ncopy_suffix = \" - relu\"\n");
        let pdf = f.pdf("contrat.pdf");
        let id = f.app.open_document(&pdf, None).unwrap().id;
        let (dir, name) = f.app.save_as_proposal(id).unwrap();
        assert_eq!(dir.as_deref(), pdf.parent());
        assert_eq!(name, "contrat - relu.pdf");
        assert!(f.app.save_as_proposal(999).is_err());
    }

    #[test]
    fn dialog_dir_only_when_it_exists() {
        let f = Fixture::new("dossier", "");
        assert_eq!(f.app.dialog_dir(), None);
        let existing = Fixture::new(
            "dossier-ok",
            &format!("[files]\ndefault_dir = {:?}\n", f.dir.display().to_string()),
        );
        assert_eq!(existing.app.dialog_dir().as_deref(), Some(f.dir.as_path()));
        let missing = Fixture::new("dossier-absent", "[files]\ndefault_dir = \"/nulle/part\"\n");
        assert_eq!(missing.app.dialog_dir(), None);
    }
}
