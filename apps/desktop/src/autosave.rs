//! Minuterie de la sauvegarde automatique (F-02) : toutes les quelques
//! secondes, regarde si l'intervalle réglé est écoulé ; l'interface est
//! prévenue de chaque document écrit (heure dans la barre d'état).

use std::sync::Arc;
use std::time::{Duration, Instant};

use pde_app::app::App;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const AUTOSAVED_EVENT: &str = "autosaved";
/// Précision de la minuterie : un changement de réglage prend effet vite.
const TICK: Duration = Duration::from_secs(5);

#[derive(Clone, Serialize)]
struct Autosaved {
    documents: Vec<u64>,
}

pub fn spawn(handle: AppHandle, app: Arc<App>) {
    tauri::async_runtime::spawn(async move {
        let mut last = Instant::now();
        loop {
            tokio::time::sleep(TICK).await;
            let Some(interval) = app.autosave_interval() else {
                continue;
            };
            if last.elapsed() < interval {
                continue;
            }
            last = Instant::now();
            let worker = Arc::clone(&app);
            // Écriture de fichiers : hors du fil des tâches asynchrones.
            let documents = tauri::async_runtime::spawn_blocking(move || worker.autosave())
                .await
                .unwrap_or_default();
            if !documents.is_empty() {
                let _ = handle.emit(AUTOSAVED_EVENT, Autosaved { documents });
            }
        }
    });
}
