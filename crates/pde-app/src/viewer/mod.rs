//! Visionneuse : documents ouverts, images et texte de leurs pages.

mod edit;
mod existing;
mod form;
mod group;
mod images;
mod import;
mod original_text;
mod pages;

pub use existing::PageImageView;
pub use form::{FieldView, WidgetView};
pub use import::ImportFile;
pub use original_text::TextBlockView;
pub use pages::PagesView;
mod recovery;
mod signatures;
#[cfg(test)]
mod testing;
mod text;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pde_core::document::{DocumentId, DocumentInfo, PageIndex, PageSize};
use std::collections::BTreeMap;

use pde_core::edit::{Edits, ElementId, Images, ImportId, Imports};
use pde_core::engine::{PageImage, PageTextBlock, PdfEngine, Placed};
use pde_core::render::RenderRequest;
use pde_core::text::PageText;

use crate::cache::{PageCache, PageKey};
use crate::error::{AppError, ErrorKind};
use crate::lock;
use crate::page_png::encode_png;
use crate::view::DocumentView;

/// Plus gros fichier accepté : il est lu entièrement en mémoire.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// Mémoire consacrée aux images de pages déjà rendues.
pub const DEFAULT_CACHE_BYTES: usize = 256 * 1024 * 1024;

struct OpenDocument {
    /// Fichier enregistré par « Enregistrer » (« Enregistrer sous » le change).
    path: PathBuf,
    /// Le document tel qu'ouvert (tailles, signets).
    original: DocumentInfo,
    /// Le document arrangé selon le plan des pages : ce qui est affiché.
    info: DocumentInfo,
    /// Version du plan des pages, augmentée à chaque changement de l'ordre,
    /// du nombre ou de la taille des pages.
    layout: u64,
    /// Gardé en mémoire seulement, pour relire le fichier enregistré.
    password: Option<String>,
    /// SHA-256 du fichier ouvert (hexadécimal) : une sauvegarde automatique
    /// ne se réapplique qu'au même contenu.
    digest: String,
    /// SHA-256 du fichier tel qu'enregistré en dernier (ouvert, enregistré
    /// ou signé) : on ne signe que ce contenu-là.
    saved_digest: String,
    /// Champs de signature du fichier enregistré.
    signatures: Vec<pde_sign::SignatureField>,
    /// Version des éléments à la dernière sauvegarde automatique.
    autosaved: Option<u64>,
    edits: Edits,
    /// Images importées (partagées par leurs copies et avec le moteur),
    /// gardées pendant toute la session pour annuler / rétablir.
    images: Images,
    /// PDF insérés (P-03), gardés comme les images ; et leur fichier, pour
    /// la sauvegarde automatique.
    imports: Imports,
    import_files: BTreeMap<ImportId, ImportFile>,
    /// Images d'origine de chaque page déjà lue (oubliées si le plan change).
    page_images: HashMap<PageIndex, Vec<PageImage>>,
    /// Blocs de texte d'origine de chaque page déjà lue (plan actuel).
    page_text_blocks: HashMap<PageIndex, Vec<PageTextBlock>>,
    /// Champs du formulaire (lus une fois, oubliés si le plan change).
    form_fields: Option<Vec<pde_core::edit::FormField>>,
    /// Place de chaque élément, telle que posée par le moteur.
    placed: HashMap<ElementId, Placed>,
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
        let bytes = read_file(path, MAX_FILE_BYTES)?;
        let size = bytes.len();
        let digest = sha256_hex(&bytes);
        let signatures = signatures::read_signatures(&bytes);
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
                edits: Edits::with_pages(opened.info.page_sizes.clone()),
                original: opened.info.clone(),
                info: opened.info,
                layout: 0,
                password,
                saved_digest: digest.clone(),
                digest,
                signatures,
                autosaved: None,
                images: Images::new(),
                imports: Imports::new(),
                import_files: BTreeMap::new(),
                page_images: HashMap::new(),
                page_text_blocks: HashMap::new(),
                form_fields: None,
                placed: HashMap::new(),
            },
        );
        Ok(view)
    }

    /// Fichier du document ouvert (celui qu'« Enregistrer » remplace).
    pub fn document_path(&self, id: u64) -> Result<PathBuf, AppError> {
        lock(&self.documents)
            .get(&DocumentId(id))
            .map(|open| open.path.clone())
            .ok_or_else(unknown_document)
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

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn unknown_document() -> AppError {
    AppError::new(ErrorKind::UnknownDocument, "ce document n'est plus ouvert")
}

/// Lit un fichier entier, refusé au-delà de `limit` octets.
fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>, AppError> {
    let io_error = |e: std::io::Error| {
        let kind = if e.kind() == std::io::ErrorKind::NotFound {
            ErrorKind::FileNotFound
        } else {
            ErrorKind::Io
        };
        AppError::new(kind, format!("{} : {e}", path.display()))
    };
    let size = std::fs::metadata(path).map_err(io_error)?.len();
    if size > limit {
        return Err(AppError::new(
            ErrorKind::FileTooLarge,
            format!("fichier de {} Mo, limite {} Mo", size >> 20, limit >> 20),
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
