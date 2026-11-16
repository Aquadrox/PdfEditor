//! Visionneuse : documents ouverts et images de leurs pages.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pde_core::document::{DocumentId, DocumentInfo, PageIndex};
use pde_core::engine::PdfEngine;
use pde_core::render::RenderRequest;

use crate::cache::{PageCache, PageKey};
use crate::error::{AppError, ErrorKind};
use crate::image::encode_png;
use crate::view::DocumentView;

/// Plus gros fichier accepté : il est lu entièrement en mémoire.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// Mémoire consacrée aux images de pages déjà rendues.
pub const DEFAULT_CACHE_BYTES: usize = 256 * 1024 * 1024;

struct OpenDocument {
    #[allow(dead_code)] // servira à « Enregistrer » (v0.2)
    path: PathBuf,
    info: DocumentInfo,
}

pub struct Viewer {
    engine: Arc<dyn PdfEngine>,
    documents: Mutex<HashMap<DocumentId, OpenDocument>>,
    cache: Mutex<PageCache>,
}

/// Verrou même si un autre thread a paniqué en le tenant : les données
/// protégées (table, cache) restent cohérentes à chaque instruction.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
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
        let opened = self.engine.open(bytes, password)?;
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let view = DocumentView::new(opened.id.0, file_name, &opened.info);
        lock(&self.documents).insert(
            opened.id,
            OpenDocument {
                path: path.to_path_buf(),
                info: opened.info,
            },
        );
        Ok(view)
    }

    /// Ferme le document ; sans effet s'il ne l'est déjà plus.
    pub fn close(&self, id: u64) {
        let id = DocumentId(id);
        if lock(&self.documents).remove(&id).is_some() {
            lock(&self.cache).remove_document(id);
            self.engine.close(id);
        }
    }

    /// Nombre de documents ouverts.
    pub fn open_count(&self) -> usize {
        lock(&self.documents).len()
    }

    /// Image PNG d'une page, depuis le cache si elle y est.
    pub fn page_png(&self, id: u64, page: PageIndex, scale: f64) -> Result<Arc<Vec<u8>>, AppError> {
        let doc = DocumentId(id);
        lock(&self.documents)
            .get(&doc)
            .ok_or_else(|| {
                AppError::new(ErrorKind::UnknownDocument, "ce document n'est plus ouvert")
            })?
            .info
            .page_size(page)?;
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
