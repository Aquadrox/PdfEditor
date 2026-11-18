//! Signature (S-01 à S-07) : identités, signatures du document, signer.

use pde_app::edit_view::SavedView;
use pde_app::error::{AppError, ErrorKind};
use pde_app::signing::{DrawingView, IdentityView, SignRequest, SignatureView};
use pde_sign::Subject;
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use super::AppState;

/// Titulaire d'un nouveau certificat, tel que saisi.
#[derive(Deserialize)]
pub struct SubjectInput {
    name: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    organization: Option<String>,
}

#[tauri::command(async)]
pub fn identities(app: AppState<'_>) -> Vec<IdentityView> {
    app.identities()
}

#[tauri::command(async)]
pub fn create_identity(app: AppState<'_>, subject: SubjectInput) -> Result<IdentityView, AppError> {
    app.create_identity(&Subject {
        name: subject.name,
        email: subject.email,
        organization: subject.organization,
    })
}

#[tauri::command(async)]
pub fn import_identity(
    app: AppState<'_>,
    path: String,
    password: String,
) -> Result<IdentityView, AppError> {
    app.import_identity(std::path::Path::new(&path), &password)
}

#[tauri::command(async)]
pub fn delete_identity(app: AppState<'_>, id: String) -> Result<(), AppError> {
    app.delete_identity(&id)
}

/// Dialogue de choix d'un certificat `.p12` / `.pfx` ; `None` si annulé.
#[tauri::command(async)]
pub fn pick_certificate(handle: AppHandle) -> Result<Option<String>, AppError> {
    let mut dialog = handle
        .dialog()
        .file()
        .set_title("Importer un certificat")
        .add_filter("Certificats PKCS#12", &["p12", "pfx"]);
    if let Some(window) = handle.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(path) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    path.into_path()
        .map(|p| Some(p.display().to_string()))
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))
}

/// Signatures manuscrites enregistrées (« Mes signatures »).
#[tauri::command(async)]
pub fn signature_drawings(app: AppState<'_>) -> Vec<DrawingView> {
    app.drawings()
}

/// Enregistre une signature manuscrite (PNG en base64).
#[tauri::command(async)]
pub fn save_signature_drawing(app: AppState<'_>, png: String) -> Result<DrawingView, AppError> {
    app.save_drawing(&png)
}

#[tauri::command(async)]
pub fn delete_signature_drawing(app: AppState<'_>, id: String) -> Result<(), AppError> {
    app.delete_drawing(&id)
}

/// Champs de signature du document (origine en haut).
#[tauri::command(async)]
pub fn document_signatures(app: AppState<'_>, id: u64) -> Result<Vec<SignatureView>, AppError> {
    app.viewer.signatures(id)
}

#[tauri::command(async)]
pub fn sign_document(
    app: AppState<'_>,
    id: u64,
    request: SignRequest,
) -> Result<SavedView, AppError> {
    app.sign_document(id, &request)
}
