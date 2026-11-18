//! Fichiers sur Android (A-02) : le système donne des adresses
//! `content://…` (sélecteur de documents, « Ouvrir avec », « Partager »),
//! pas des chemins. Le document est copié dans le dossier de l'application
//! (`pde_app::copies`), ouvert depuis cette copie, puis réécrit dans son
//! origine à chaque enregistrement. Lecture et écriture passent par le
//! module `fs` de Tauri, qui demande au système l'accès à l'adresse.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pde_app::app::App;
use pde_app::copies::{LocalCopies, name_from_uri};
use pde_app::edit_view::SavedView;
use pde_app::error::{AppError, ErrorKind};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_fs::{FsExt, OpenOptions};

/// Fichier où l'activité Android note l'adresse d'un document reçu
/// (« Ouvrir avec », « Partager »), dans le dossier de cache.
const SHARED_FILE: &str = "opened-document.txt";

fn io(error: impl ToString) -> AppError {
    AppError::new(ErrorKind::Io, error.to_string())
}

fn copies(handle: &AppHandle) -> Arc<LocalCopies> {
    handle.state::<Arc<LocalCopies>>().inner().clone()
}

/// Copie locale du document à l'adresse `uri`.
fn import(handle: &AppHandle, uri: &str) -> Result<PathBuf, AppError> {
    let url = tauri::Url::parse(uri).map_err(io)?;
    let bytes = handle.fs().read(FilePath::Url(url)).map_err(io)?;
    copies(handle).store(&name_from_uri(uri), &bytes, Some(uri))
}

/// Chemin à ouvrir pour ce choix du sélecteur (copie locale d'une adresse).
fn local_path(handle: &AppHandle, file: FilePath) -> Result<PathBuf, AppError> {
    match file {
        FilePath::Path(path) => Ok(path),
        FilePath::Url(url) if url.scheme() == "file" => url
            .to_file_path()
            .map_err(|()| io("adresse de fichier invalide")),
        FilePath::Url(url) => import(handle, url.as_str()),
    }
}

/// Sélecteur de documents d'Android ; `None` si annulé.
pub fn pick_pdf(handle: &AppHandle) -> Result<Option<String>, AppError> {
    let dialog = handle
        .dialog()
        .file()
        .set_title("Ouvrir un PDF")
        .add_filter("Documents PDF", &["pdf"]);
    let Some(file) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    local_path(handle, file).map(|p| Some(p.display().to_string()))
}

/// Écrit la copie `path` dans son adresse d'origine, s'il y en a une.
pub fn write_back(handle: &AppHandle, path: &Path) -> Result<(), AppError> {
    let copies = copies(handle);
    let Some(origin) = copies.origin(path) else {
        return Ok(());
    };
    let bytes = copies.read(path)?;
    let url = tauri::Url::parse(&origin).map_err(io)?;
    let mut options = OpenOptions::new();
    options.write(true).truncate(true);
    let mut file = handle.fs().open(FilePath::Url(url), options).map_err(|e| {
        io(format!(
            "document enregistré dans l'application, mais pas à son emplacement d'origine ({e}) : utilisez « Enregistrer sous »"
        ))
    })?;
    file.write_all(&bytes).map_err(io)?;
    file.flush().map_err(io)
}

/// « Enregistrer » : la copie, puis son origine.
pub fn save(handle: &AppHandle, app: &App, id: u64) -> Result<SavedView, AppError> {
    let saved = app.save_document(id, None)?;
    write_back(handle, Path::new(&saved.path))?;
    Ok(saved)
}

/// « Enregistrer sous » : emplacement choisi dans le sélecteur d'Android ;
/// une nouvelle copie locale le garde comme origine. `None` si annulé.
pub fn save_as(handle: &AppHandle, app: &App, id: u64) -> Result<Option<SavedView>, AppError> {
    let (_, name) = app.save_as_proposal(id)?;
    let dialog = handle
        .dialog()
        .file()
        .set_title("Enregistrer sous")
        .add_filter("Documents PDF", &["pdf"])
        .set_file_name(&name);
    let Some(target) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let copies = copies(handle);
    let (local, origin) = match target {
        FilePath::Path(path) => (path, None),
        FilePath::Url(url) => (
            copies.free_path(&name_from_uri(url.as_str())),
            Some(url.to_string()),
        ),
    };
    let saved = app.save_document(id, Some(local.clone()))?;
    if let Some(origin) = origin {
        copies.set_origin(&local, &origin);
        write_back(handle, &local)?;
    }
    Ok(Some(saved))
}

/// Document reçu par « Ouvrir avec » ou « Partager » depuis le dernier
/// appel : sa copie locale, à ouvrir. L'adresse n'est lue qu'une fois.
pub fn shared_files(handle: &AppHandle) -> Vec<String> {
    // L'activité écrit dans `cacheDir` : c'est le dossier de cache de l'app.
    let dirs = [handle.path().app_cache_dir(), handle.path().cache_dir()];
    let Some(pending) = dirs
        .into_iter()
        .flatten()
        .map(|dir| dir.join(SHARED_FILE))
        .find(|path| path.is_file())
    else {
        return Vec::new();
    };
    let uri = std::fs::read_to_string(&pending).unwrap_or_default();
    let _ = std::fs::remove_file(&pending);
    match import(handle, uri.trim()) {
        Ok(path) => vec![path.display().to_string()],
        Err(error) => {
            log::warn!("document reçu illisible : {}", error.message);
            Vec::new()
        }
    }
}
