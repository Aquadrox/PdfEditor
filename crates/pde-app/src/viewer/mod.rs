//! Visionneuse : documents ouverts, images et texte de leurs pages.

mod edit;
mod text;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pde_core::document::{DocumentId, DocumentInfo, PageIndex, PageSize};
use pde_core::edit::{Edits, ElementId};
use pde_core::engine::PdfEngine;
use pde_core::geometry::Rect;
use pde_core::render::RenderRequest;
use pde_core::text::PageText;

use crate::cache::{PageCache, PageKey};
use crate::error::{AppError, ErrorKind};
use crate::image::encode_png;
use crate::lock;
use crate::view::DocumentView;

/// Plus gros fichier accepté : il est lu entièrement en mémoire.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// Mémoire consacrée aux images de pages déjà rendues.
pub const DEFAULT_CACHE_BYTES: usize = 256 * 1024 * 1024;

struct OpenDocument {
    /// Fichier enregistré par « Enregistrer » (« Enregistrer sous » le change).
    path: PathBuf,
    info: DocumentInfo,
    /// Gardé en mémoire seulement, pour relire le fichier enregistré.
    password: Option<String>,
    edits: Edits,
    /// Place de chaque élément, telle que posée par le moteur.
    placed: HashMap<ElementId, Rect>,
    /// Version de chaque page, augmentée à chaque changement de son contenu
    /// (l'interface la met dans l'adresse de l'image pour la recharger).
    revisions: Vec<u64>,
}

pub struct Viewer {
    engine: Arc<dyn PdfEngine>,
    documents: Mutex<HashMap<DocumentId, OpenDocument>>,
    cache: Mutex<PageCache>,
    /// Texte des pages déjà lues (couche de texte, recherche), oublié à la
    /// fermeture du document.
    texts: Mutex<HashMap<(DocumentId, PageIndex), Arc<PageText>>>,
}

/// Zoom arrondi au centième : deux demandes à 1,499 et 1,5 partagent la
/// même image en cache.
pub fn quantize_scale(scale: f64) -> Result<(u32, f64), AppError> {
    let invalid = || {
        AppError::new(
            ErrorKind::InvalidRequest,
            format!("zoom invalide : {scale}"),
        )
    };
    if !scale.is_finite() || scale <= 0.0 || scale > 100.0 {
        return Err(invalid());
    }
    let centi = (scale * 100.0).round().max(1.0);
    // Borné à 10 000 par le test ci-dessus.
    let centi = centi as u32;
    Ok((centi, f64::from(centi) / 100.0))
}

impl Viewer {
    pub fn new(engine: Arc<dyn PdfEngine>, cache_bytes: usize) -> Self {
        Self {
            engine,
            documents: Mutex::new(HashMap::new()),
            cache: Mutex::new(PageCache::new(cache_bytes)),
            texts: Mutex::new(HashMap::new()),
        }
    }

    /// Ouvre un fichier PDF ; `password` est demandé à l'utilisateur après
    /// une erreur [`ErrorKind::PasswordRequired`].
    pub fn open_file(
        &self,
        path: &Path,
        password: Option<String>,
    ) -> Result<DocumentView, AppError> {
        let bytes = read_file(path)?;
        let size = bytes.len();
        let started = std::time::Instant::now();
        let opened = self.engine.open(bytes, password.clone()).inspect_err(|e| {
            // Jamais le nom du fichier : il peut être personnel.
            log::info!("ouverture refusée : {e}");
        })?;
        log::info!(
            "document ouvert : {} pages, {} Ko, {} ms",
            opened.info.page_count(),
            size / 1024,
            started.elapsed().as_millis()
        );
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let view = DocumentView::new(opened.id.0, file_name, &opened.info);
        lock(&self.documents).insert(
            opened.id,
            OpenDocument {
                path: path.to_path_buf(),
                revisions: vec![0; opened.info.page_count()],
                info: opened.info,
                password,
                edits: Edits::new(),
                placed: HashMap::new(),
            },
        );
        Ok(view)
    }

    /// Ferme le document ; sans effet s'il ne l'est déjà plus.
    pub fn close(&self, id: u64) {
        let id = DocumentId(id);
        if lock(&self.documents).remove(&id).is_some() {
            lock(&self.cache).remove_document(id);
            lock(&self.texts).retain(|(doc, _), _| *doc != id);
            self.engine.close(id);
        }
    }

    /// Change la mémoire consacrée aux images de pages (réglage appliqué à chaud).
    pub fn set_cache_bytes(&self, bytes: usize) {
        lock(&self.cache).set_budget(bytes);
    }

    /// Nombre de documents ouverts.
    pub fn open_count(&self) -> usize {
        lock(&self.documents).len()
    }

    /// Image PNG d'une page, depuis le cache si elle y est.
    pub fn page_png(&self, id: u64, page: PageIndex, scale: f64) -> Result<Arc<Vec<u8>>, AppError> {
        let doc = DocumentId(id);
        self.page_size(doc, page)?;
        let (scale_centi, scale) = quantize_scale(scale)?;
        let key = PageKey {
            doc,
            page,
            scale_centi,
        };
        if let Some(png) = lock(&self.cache).get(&key) {
            return Ok(png);
        }
        // Rendu hors verrou : les autres pages restent servies pendant ce temps.
        let bitmap = self.engine.render(doc, RenderRequest { page, scale })?;
        let png = Arc::new(encode_png(&bitmap)?);
        lock(&self.cache).insert(key, png.clone());
        Ok(png)
    }
}

impl Viewer {
    /// Taille affichée de chaque page du document ouvert `doc`.
    fn page_sizes(&self, doc: DocumentId) -> Result<Vec<PageSize>, AppError> {
        lock(&self.documents)
            .get(&doc)
            .map(|d| d.info.page_sizes.clone())
            .ok_or_else(unknown_document)
    }

    fn page_size(&self, doc: DocumentId, page: PageIndex) -> Result<PageSize, AppError> {
        let documents = lock(&self.documents);
        let info = &documents.get(&doc).ok_or_else(unknown_document)?.info;
        Ok(info.page_size(page)?)
    }
}

fn unknown_document() -> AppError {
    AppError::new(ErrorKind::UnknownDocument, "ce document n'est plus ouvert")
}

fn read_file(path: &Path) -> Result<Vec<u8>, AppError> {
    let io_error = |e: std::io::Error| {
        let kind = if e.kind() == std::io::ErrorKind::NotFound {
            ErrorKind::FileNotFound
        } else {
            ErrorKind::Io
        };
        AppError::new(kind, format!("{} : {e}", path.display()))
    };
    let size = std::fs::metadata(path).map_err(io_error)?.len();
    if size > MAX_FILE_BYTES {
        return Err(AppError::new(
            ErrorKind::FileTooLarge,
            format!(
                "fichier de {} Mo, limite {} Mo",
                size >> 20,
                MAX_FILE_BYTES >> 20
            ),
        ));
    }
    std::fs::read(path).map_err(io_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeEngine, LOCKED_PDF, PASSWORD, SIMPLE_PDF};
    use std::io::Write;

    struct Fixture {
        engine: Arc<FakeEngine>,
        viewer: Viewer,
        dir: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let engine = Arc::new(FakeEngine::default());
            let viewer = Viewer::new(engine.clone(), 64 * 1024);
            let dir = std::env::temp_dir().join(format!("pde-app-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self {
                engine,
                viewer,
                dir,
            }
        }

        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.dir.join(name);
            std::fs::File::create(&path)
                .unwrap()
                .write_all(bytes)
                .unwrap();
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn opens_a_file_and_describes_it() {
        let f = Fixture::new("open");
        let view = f
            .viewer
            .open_file(&f.file("contrat.pdf", SIMPLE_PDF), None)
            .unwrap();
        assert_eq!(view.file_name, "contrat.pdf");
        assert_eq!(view.pages.len(), 2);
        assert_eq!(f.viewer.open_count(), 1);
    }

    #[test]
    fn missing_file_and_invalid_content() {
        let f = Fixture::new("missing");
        let missing = f
            .viewer
            .open_file(&f.dir.join("absent.pdf"), None)
            .unwrap_err();
        assert_eq!(missing.kind, ErrorKind::FileNotFound);
        let invalid = f
            .viewer
            .open_file(&f.file("x.pdf", b"texte"), None)
            .unwrap_err();
        assert_eq!(invalid.kind, ErrorKind::InvalidFile);
        assert_eq!(f.viewer.open_count(), 0);
    }

    #[test]
    fn password_is_asked_then_checked() {
        let f = Fixture::new("password");
        let path = f.file("secret.pdf", LOCKED_PDF);
        let kind = |p: Option<&str>| f.viewer.open_file(&path, p.map(Into::into)).map(|_| ());
        assert_eq!(kind(None).unwrap_err().kind, ErrorKind::PasswordRequired);
        assert_eq!(
            kind(Some("non")).unwrap_err().kind,
            ErrorKind::WrongPassword
        );
        assert!(kind(Some(PASSWORD)).is_ok());
    }

    #[test]
    fn pages_are_rendered_once_then_cached() {
        let f = Fixture::new("cache");
        let view = f
            .viewer
            .open_file(&f.file("a.pdf", SIMPLE_PDF), None)
            .unwrap();
        let first = f.viewer.page_png(view.id, 0, 0.5).unwrap();
        // 0,499 et 0,5 : même zoom au centième, même image.
        let second = f.viewer.page_png(view.id, 0, 0.499).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(f.engine.render_count(), 1);
        f.viewer.page_png(view.id, 0, 1.0).unwrap();
        assert_eq!(f.engine.render_count(), 2);
    }

    #[test]
    fn invalid_page_or_scale_or_document() {
        let f = Fixture::new("invalid");
        let view = f
            .viewer
            .open_file(&f.file("a.pdf", SIMPLE_PDF), None)
            .unwrap();
        let kind = |id, page, scale| f.viewer.page_png(id, page, scale).unwrap_err().kind;
        assert_eq!(kind(view.id, 5, 1.0), ErrorKind::InvalidRequest);
        assert_eq!(kind(view.id, 0, f64::NAN), ErrorKind::InvalidRequest);
        assert_eq!(kind(view.id + 1, 0, 1.0), ErrorKind::UnknownDocument);
        assert_eq!(f.engine.render_count(), 0);
    }

    #[test]
    fn closing_releases_the_document() {
        let f = Fixture::new("close");
        let view = f
            .viewer
            .open_file(&f.file("a.pdf", SIMPLE_PDF), None)
            .unwrap();
        f.viewer.page_png(view.id, 0, 1.0).unwrap();
        f.viewer.close(view.id);
        f.viewer.close(view.id);
        assert_eq!(f.viewer.open_count(), 0);
        assert_eq!(f.engine.closed(), vec![view.id]);
        let error = f.viewer.page_png(view.id, 0, 1.0).unwrap_err();
        assert_eq!(error.kind, ErrorKind::UnknownDocument);
    }

    #[test]
    fn scale_quantization() {
        assert_eq!(quantize_scale(1.234).unwrap(), (123, 1.23));
        assert_eq!(quantize_scale(0.001).unwrap().0, 1);
        for bad in [0.0, -1.0, f64::INFINITY, 101.0] {
            assert!(quantize_scale(bad).is_err(), "{bad}");
        }
    }
}
