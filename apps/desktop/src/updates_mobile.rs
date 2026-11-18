//! Mises à jour sur Android : le module de Tauri n'existe pas sur mobile.
//! Même interface que `updates.rs`, sans rien vérifier ni installer ; les
//! mises à jour par APK signé viendront ensuite (A-04).

use std::sync::Arc;

use pde_core::changelog::{Platform, Release};
use serde::Serialize;
use tauri::AppHandle;

/// Système dont l'application affiche les notes de version.
pub fn platform() -> Platform {
    Platform::Android
}

pub struct Updates;

pub type UpdatesState = Arc<Updates>;

impl Updates {
    pub fn new(_app: &AppHandle, _auto: bool) -> UpdatesState {
        Arc::new(Self)
    }
}

pub fn spawn_checks(_app: AppHandle, _updates: UpdatesState) {}

pub fn set_auto(_app: &AppHandle, _updates: &UpdatesState, _enabled: bool) {}

/// État affiché par le bandeau et les réglages : rien de configuré.
#[derive(Serialize)]
pub struct UpdateStatus {
    current: String,
    available: Option<String>,
    notes: Vec<Release>,
    configured: bool,
}

pub fn status(app: &AppHandle, _updates: &Updates) -> UpdateStatus {
    UpdateStatus {
        current: app.package_info().version.to_string(),
        available: None,
        notes: Vec::new(),
        configured: false,
    }
}

pub async fn check_now(app: &AppHandle, updates: &UpdatesState) -> Result<UpdateStatus, String> {
    Ok(status(app, updates))
}

pub async fn install(_app: &AppHandle, _updates: &Updates) -> Result<(), String> {
    Err("mises à jour automatiques indisponibles sur Android pour l'instant".into())
}
