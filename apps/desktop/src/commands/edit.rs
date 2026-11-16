//! Édition : éléments ajoutés, annuler / rétablir, enregistrer.

use std::path::PathBuf;

use pde_app::edit_view::{EditView, SavedView};
use pde_app::error::{AppError, ErrorKind};
use pde_core::edit::Element;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use super::AppState;

#[tauri::command(async)]
pub fn edit_view(app: AppState<'_>, id: u64) -> Result<EditView, AppError> {
    app.viewer.edit_view(id)
}

#[derive(Serialize)]
pub struct Added {
    id: u64,
    view: EditView,
}

/// Élément en coordonnées de la page affichée, origine en haut à gauche.
#[tauri::command(async)]
pub fn add_element(app: AppState<'_>, id: u64, element: Element) -> Result<Added, AppError> {
    let (element_id, view) = app.viewer.add_element(id, element)?;
    Ok(Added {
        id: element_id,
        view,
    })
}

#[tauri::command(async)]
pub fn update_element(
    app: AppState<'_>,
    id: u64,
    element_id: u64,
    element: Element,
) -> Result<EditView, AppError> {
    app.viewer.update_element(id, element_id, element)
}

#[tauri::command(async)]
pub fn remove_element(app: AppState<'_>, id: u64, element_id: u64) -> Result<EditView, AppError> {
    app.viewer.remove_element(id, element_id)
}

#[tauri::command(async)]
pub fn undo_edit(app: AppState<'_>, id: u64) -> Result<EditView, AppError> {
    app.viewer.undo(id)
}

#[tauri::command(async)]
pub fn redo_edit(app: AppState<'_>, id: u64) -> Result<EditView, AppError> {
    app.viewer.redo(id)
}

/// « Enregistrer » : à la place du fichier ouvert.
#[tauri::command(async)]
pub fn save_document(app: AppState<'_>, id: u64) -> Result<SavedView, AppError> {
    app.save_document(id, None)
}

/// « Enregistrer sous » : dialogue, puis enregistrement ; `None` si annulé.
#[tauri::command(async)]
pub fn save_document_as(
    handle: AppHandle,
    app: AppState<'_>,
    id: u64,
    suggested_name: String,
) -> Result<Option<SavedView>, AppError> {
    let mut dialog = handle
        .dialog()
        .file()
        .set_title("Enregistrer sous")
        .add_filter("Documents PDF", &["pdf"])
        .set_file_name(suggested_name);
    if let Some(window) = handle.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path: PathBuf = path
        .into_path()
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))?;
    app.save_document(id, Some(path)).map(Some)
}
