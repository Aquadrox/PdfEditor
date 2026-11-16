//! Réglages, journaux et informations sur l'application.

use pde_app::error::{AppError, ErrorKind};
use pde_app::settings_store::SettingsView;
use pde_core::settings::Settings;
use pde_platform::dirs::AppDirs;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::AppState;
use crate::logging;

#[tauri::command(async)]
pub fn get_settings(app: AppState<'_>) -> SettingsView {
    app.settings()
}

/// Réglages par défaut (bouton « Rétablir »).
#[tauri::command]
pub fn default_settings() -> Settings {
    Settings::default()
}

/// Enregistre et applique ; renvoie les réglages tels qu'enregistrés.
#[tauri::command(async)]
pub fn save_settings(app: AppState<'_>, settings: Settings) -> Result<Settings, AppError> {
    let saved = app.save_settings(&settings)?;
    logging::set_level(saved.diagnostic.log_level);
    Ok(saved)
}

#[tauri::command(async)]
pub fn open_logs_folder(handle: AppHandle, dirs: State<'_, AppDirs>) -> Result<(), AppError> {
    handle
        .opener()
        .open_path(dirs.logs.display().to_string(), None::<&str>)
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))
}

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
    }
}
