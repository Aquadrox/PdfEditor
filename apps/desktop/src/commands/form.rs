//! Formulaire (E-08) et propriétés du document (E-11).

use pde_app::edit_view::EditView;
use pde_app::error::AppError;
use pde_app::viewer::FieldView;
use pde_core::edit::{FieldValue, Metadata};

use super::AppState;

/// Champs du formulaire et leurs emplacements (origine en haut).
#[tauri::command(async)]
pub fn form_fields(app: AppState<'_>, id: u64) -> Result<Vec<FieldView>, AppError> {
    app.viewer.form_fields(id)
}

/// Valeur saisie (`null` : celle du document d'origine).
#[tauri::command(async)]
pub fn set_field(
    app: AppState<'_>,
    id: u64,
    name: String,
    value: Option<FieldValue>,
) -> Result<EditView, AppError> {
    app.viewer.set_field(id, &name, value)
}

#[tauri::command(async)]
pub fn set_flatten(app: AppState<'_>, id: u64, flatten: bool) -> Result<EditView, AppError> {
    app.viewer.set_flatten(id, flatten)
}

/// Propriétés du document : changées, sinon celles du document d'origine.
#[tauri::command(async)]
pub fn document_metadata(app: AppState<'_>, id: u64) -> Result<Metadata, AppError> {
    app.viewer.metadata(id)
}

#[tauri::command(async)]
pub fn set_metadata(app: AppState<'_>, id: u64, metadata: Metadata) -> Result<EditView, AppError> {
    app.viewer.set_metadata(id, metadata)
}
