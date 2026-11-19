//! Formulaire (E-08), conception de formulaires (E-12) et propriétés du
//! document (E-11).

use pde_app::edit_view::EditView;
use pde_app::error::AppError;
use pde_app::viewer::{FieldView, WidgetPlace, WidgetTarget};
use pde_core::edit::{FieldBox, FieldValue, Metadata};
use serde::Serialize;

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

#[derive(Serialize)]
pub struct AddedFields {
    ids: Vec<u64>,
    view: EditView,
}

/// Champs créés (E-12), origine en haut ; un nom déjà pris reçoit un suffixe.
#[tauri::command(async)]
pub fn add_form_fields(
    app: AppState<'_>,
    id: u64,
    fields: Vec<FieldBox>,
) -> Result<AddedFields, AppError> {
    let (ids, view) = app.viewer.add_form_fields(id, fields)?;
    Ok(AddedFields { ids, view })
}

/// Réglages d'un champ appliqués à tous ses emplacements.
#[tauri::command(async)]
pub fn set_form_field(
    app: AppState<'_>,
    id: u64,
    name: String,
    field: FieldBox,
) -> Result<EditView, AppError> {
    app.viewer.set_form_field(id, &name, field)
}

/// Emplacements déplacés ou redimensionnés (origine en haut).
#[tauri::command(async)]
pub fn place_form_widgets(
    app: AppState<'_>,
    id: u64,
    places: Vec<WidgetPlace>,
) -> Result<EditView, AppError> {
    app.viewer.place_form_widgets(id, places)
}

/// Champs entiers ou emplacements supprimés.
#[tauri::command(async)]
pub fn delete_form_widgets(
    app: AppState<'_>,
    id: u64,
    targets: Vec<WidgetTarget>,
) -> Result<EditView, AppError> {
    app.viewer.delete_form_widgets(id, targets)
}

/// Champs proposés pour une page de formulaire « papier » (rien n'est ajouté).
#[tauri::command(async)]
pub fn suggest_form_fields(
    app: AppState<'_>,
    id: u64,
    page: usize,
) -> Result<Vec<FieldBox>, AppError> {
    app.viewer.suggest_form_fields(id, page)
}
