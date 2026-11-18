//! Ouvrir, fermer, lire et chercher dans un document.

use std::path::PathBuf;

use pde_app::error::AppError;
#[cfg(desktop)]
use pde_app::error::ErrorKind;
use pde_app::recent::RecentView;
use pde_app::text_view::{PageTextView, SearchResults};
use pde_app::view::DocumentView;
use tauri::AppHandle;
#[cfg(desktop)]
use tauri::Manager;
#[cfg(desktop)]
use tauri_plugin_dialog::DialogExt;

use super::AppState;

#[tauri::command(async)]
pub fn open_document(
    app: AppState<'_>,
    path: String,
    password: Option<String>,
) -> Result<DocumentView, AppError> {
    app.open_document(&PathBuf::from(path), password)
}

#[tauri::command(async)]
pub fn close_document(app: AppState<'_>, id: u64) {
    app.close_document(id);
}

/// Texte d'une page, en segments positionnés (couche sélectionnable).
#[tauri::command(async)]
pub fn page_text(app: AppState<'_>, id: u64, page: usize) -> Result<PageTextView, AppError> {
    app.viewer.page_text(id, page)
}

/// Occurrences d'un texte dans tout le document.
#[tauri::command(async)]
pub fn search_document(
    app: AppState<'_>,
    id: u64,
    query: String,
) -> Result<SearchResults, AppError> {
    app.viewer.search(id, &query)
}

#[tauri::command(async)]
pub fn recent_files(app: AppState<'_>) -> Vec<RecentView> {
    app.recent_files()
}

#[tauri::command(async)]
pub fn clear_recent_files(app: AppState<'_>) -> Result<(), AppError> {
    app.clear_recent_files()
}

/// Dialogue « Ouvrir » ; `None` si l'utilisateur annule. Android : le
/// sélecteur de documents, document copié dans l'application.
#[tauri::command(async)]
pub fn pick_pdf(handle: AppHandle) -> Result<Option<String>, AppError> {
    #[cfg(mobile)]
    return crate::mobile_files::pick_pdf(&handle);
    #[cfg(desktop)]
    pick_pdf_desktop(handle)
}

#[cfg(desktop)]
fn pick_pdf_desktop(handle: AppHandle) -> Result<Option<String>, AppError> {
    let mut dialog = handle
        .dialog()
        .file()
        .set_title("Ouvrir un PDF")
        .add_filter("Documents PDF", &["pdf"]);
    if let Some(dir) = handle
        .state::<std::sync::Arc<pde_app::app::App>>()
        .dialog_dir()
    {
        dialog = dialog.set_directory(dir);
    }
    dialog = super::dialog::over_main_window(&handle, dialog);
    let Some(path) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    path.into_path()
        .map(|p| Some(p.display().to_string()))
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))
}

/// Document reçu d'une autre application depuis le dernier appel (Android :
/// « Ouvrir avec », « Partager ») ; toujours vide sur PC.
#[tauri::command(async)]
pub fn shared_files(handle: AppHandle) -> Vec<String> {
    #[cfg(mobile)]
    return crate::mobile_files::shared_files(&handle);
    #[cfg(desktop)]
    {
        let _ = handle;
        Vec::new()
    }
}

/// Fichiers passés en ligne de commande (« Ouvrir avec PdfEditor »).
#[tauri::command]
pub fn initial_files() -> Vec<String> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .map(|p| p.display().to_string())
        .collect()
}
