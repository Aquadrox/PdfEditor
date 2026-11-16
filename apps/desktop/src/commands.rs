//! Commandes appelées par l'interface. Aucune logique ici : chaque commande
//! délègue à `pde-app` et renvoie son résultat ou son erreur en JSON.
//!
//! `async` : exécutées hors du thread de la fenêtre, qui reste fluide
//! pendant la lecture d'un gros fichier.

use std::path::PathBuf;
use std::sync::Arc;

use pde_app::error::{AppError, ErrorKind};
use pde_app::text_view::{PageTextView, SearchResults};
use pde_app::view::DocumentView;
use pde_app::viewer::Viewer;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

type ViewerState<'a> = State<'a, Arc<Viewer>>;

#[tauri::command(async)]
pub fn open_document(
    viewer: ViewerState<'_>,
    path: String,
    password: Option<String>,
) -> Result<DocumentView, AppError> {
    viewer.open_file(&PathBuf::from(path), password)
}

#[tauri::command(async)]
pub fn close_document(viewer: ViewerState<'_>, id: u64) {
    viewer.close(id);
}

/// Texte d'une page, en segments positionnés (couche sélectionnable).
#[tauri::command(async)]
pub fn page_text(viewer: ViewerState<'_>, id: u64, page: usize) -> Result<PageTextView, AppError> {
    viewer.page_text(id, page)
}

/// Occurrences d'un texte dans tout le document.
#[tauri::command(async)]
pub fn search_document(
    viewer: ViewerState<'_>,
    id: u64,
    query: String,
) -> Result<SearchResults, AppError> {
    viewer.search(id, &query)
}

/// Dialogue « Ouvrir » ; `None` si l'utilisateur annule.
#[tauri::command(async)]
pub fn pick_pdf(app: AppHandle) -> Result<Option<String>, AppError> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Ouvrir un PDF")
        .add_filter("Documents PDF", &["pdf"]);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    path.into_path()
        .map(|p| Some(p.display().to_string()))
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))
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
