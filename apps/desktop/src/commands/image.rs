//! Images ajoutées (E-02) : choisies dans un fichier, déposées ou collées.

use std::path::PathBuf;

use pde_app::edit_view::ImportedImage;
use pde_app::error::{AppError, ErrorKind};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use super::AppState;

/// En-tête qui porte le document visé par `import_image_bytes`.
const DOCUMENT_HEADER: &str = "x-document";

fn bad_request(message: &str) -> AppError {
    AppError::new(ErrorKind::InvalidRequest, message)
}

/// Dialogue « Choisir une image », puis import ; `None` si annulé.
#[tauri::command(async)]
pub fn pick_image(
    handle: AppHandle,
    app: AppState<'_>,
    id: u64,
) -> Result<Option<ImportedImage>, AppError> {
    let mut dialog = handle
        .dialog()
        .file()
        .set_title("Ajouter une image")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"]);
    if let Some(window) = handle.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))?;
    app.viewer.import_image_file(id, &path).map(Some)
}

/// Fichier d'image déposé sur la page.
#[tauri::command(async)]
pub fn import_image_file(
    app: AppState<'_>,
    id: u64,
    path: String,
) -> Result<ImportedImage, AppError> {
    app.viewer.import_image_file(id, &PathBuf::from(path))
}

/// Image collée : octets bruts dans le corps, document dans l'en-tête
/// (pas de JSON pour plusieurs mégaoctets).
#[tauri::command(async)]
pub fn import_image_bytes(
    app: AppState<'_>,
    request: Request<'_>,
) -> Result<ImportedImage, AppError> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(bad_request("image attendue en octets"));
    };
    let id = request
        .headers()
        .get(DOCUMENT_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or_else(|| bad_request("document non précisé"))?;
    app.viewer.import_image(id, bytes.clone())
}
