//! Pages (P-01 à P-03) : insérer une page blanche ou d'autres PDF,
//! supprimer, dupliquer, déplacer, pivoter, extraire. Numéros de page à
//! partir de 0.

use std::path::PathBuf;

use pde_app::edit_view::SavedView;
use pde_app::error::{AppError, ErrorKind};
use pde_app::view::DocumentView;
use pde_app::viewer::PagesView;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use super::AppState;

fn path_error(error: impl ToString) -> AppError {
    AppError::new(ErrorKind::Io, error.to_string())
}

/// Description du document tel qu'affiché (après un changement des pages,
/// annulé ou rétabli).
#[tauri::command(async)]
pub fn document_view(app: AppState<'_>, id: u64) -> Result<DocumentView, AppError> {
    app.viewer.document_view(id)
}

#[tauri::command(async)]
pub fn insert_blank_page(app: AppState<'_>, id: u64, at: usize) -> Result<PagesView, AppError> {
    app.viewer.insert_blank_page(id, at)
}

#[tauri::command(async)]
pub fn remove_pages(app: AppState<'_>, id: u64, pages: Vec<usize>) -> Result<PagesView, AppError> {
    app.viewer.remove_pages(id, &pages)
}

#[tauri::command(async)]
pub fn duplicate_pages(
    app: AppState<'_>,
    id: u64,
    pages: Vec<usize>,
) -> Result<PagesView, AppError> {
    app.viewer.duplicate_pages(id, &pages)
}

/// Pages placées devant la page `to` (`to` = nombre de pages : à la fin).
#[tauri::command(async)]
pub fn move_pages(
    app: AppState<'_>,
    id: u64,
    pages: Vec<usize>,
    to: usize,
) -> Result<PagesView, AppError> {
    app.viewer.move_pages(id, &pages, to)
}

/// `turns` : quarts de tour dans le sens horaire (3 : sens inverse).
#[tauri::command(async)]
pub fn rotate_pages(
    app: AppState<'_>,
    id: u64,
    pages: Vec<usize>,
    turns: u8,
) -> Result<PagesView, AppError> {
    app.viewer.rotate_pages(id, &pages, turns)
}

/// « Insérer des PDF » : dialogue (plusieurs fichiers), puis toutes leurs
/// pages à la place `at`, en une opération ; `None` si annulé.
#[tauri::command(async)]
pub fn insert_pdf(
    handle: AppHandle,
    app: AppState<'_>,
    id: u64,
    at: usize,
) -> Result<Option<PagesView>, AppError> {
    let mut dialog = handle
        .dialog()
        .file()
        .set_title("Insérer les pages de PDF")
        .add_filter("Documents PDF", &["pdf"]);
    if let Some(dir) = app.dialog_dir() {
        dialog = dialog.set_directory(dir);
    }
    if let Some(window) = handle.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(files) = dialog.blocking_pick_files() else {
        return Ok(None);
    };
    let paths = files
        .into_iter()
        .map(|f| f.into_path().map_err(path_error))
        .collect::<Result<Vec<PathBuf>, _>>()?;
    app.viewer.insert_pdf_files(id, &paths, at).map(Some)
}

/// PDF déposés sur la grille du mode Pages : insérés à la place `at`.
#[tauri::command(async)]
pub fn insert_pdf_files(
    app: AppState<'_>,
    id: u64,
    paths: Vec<PathBuf>,
    at: usize,
) -> Result<PagesView, AppError> {
    app.viewer.insert_pdf_files(id, &paths, at)
}

/// « Extraire » : dialogue (dossier du document, nom proposé par
/// `pde-app`), puis écriture des pages dans un nouveau PDF ; `None` si annulé.
#[tauri::command(async)]
pub fn extract_pages(
    handle: AppHandle,
    app: AppState<'_>,
    id: u64,
    pages: Vec<usize>,
) -> Result<Option<SavedView>, AppError> {
    let (dir, name) = app.extract_proposal(id, &pages)?;
    let mut dialog = handle
        .dialog()
        .file()
        .set_title("Extraire les pages")
        .add_filter("Documents PDF", &["pdf"])
        .set_file_name(name);
    if let Some(dir) = dir {
        dialog = dialog.set_directory(dir);
    }
    if let Some(window) = handle.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path = path.into_path().map_err(path_error)?;
    app.viewer.export_pages(id, &pages, &path).map(Some)
}
