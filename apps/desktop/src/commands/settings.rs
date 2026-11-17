//! Réglages, journaux et informations sur l'application.

use pde_app::error::{AppError, ErrorKind};
use pde_app::settings_store::SettingsView;
use pde_core::settings::Settings;
use pde_platform::dirs::AppDirs;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::AppState;

/// Choix d'un dossier (réglage Fichiers : dossier par défaut) ; `None` si annulé.
#[tauri::command(async)]
pub fn pick_folder(handle: AppHandle) -> Result<Option<String>, AppError> {
    let mut dialog = handle.dialog().file().set_title("Dossier par défaut");
    if let Some(window) = handle.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(folder) = dialog.blocking_pick_folder() else {
        return Ok(None);
    };
    folder
        .into_path()
        .map(|p| Some(p.display().to_string()))
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))
}
use crate::logging;
use crate::updates::UpdatesState;

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
pub fn save_settings(
    handle: AppHandle,
    app: AppState<'_>,
    updates: State<'_, UpdatesState>,
    settings: Settings,
) -> Result<Settings, AppError> {
    let saved = app.save_settings(&settings)?;
    logging::set_level(saved.diagnostic.log_level);
    crate::updates::set_auto(&handle, updates.inner(), saved.updates.check);
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
