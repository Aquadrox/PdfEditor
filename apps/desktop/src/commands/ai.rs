//! IA (v0.5) : état, clé, test des réglages, estimation, actions en flux.
//! Une action tourne sur un fil à part ; son avancement arrive à
//! l'interface par les événements `ai-event` et `ai-done`.

use std::sync::Arc;

use pde_ai::tasks::Task;
use pde_app::ai::{AiStatus, EstimateView, Scope, TestView};
use pde_app::app::App;
use pde_app::error::AppError;
use pde_core::settings::Provider;
use pde_core::usage::UsageSummary;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::AppState;

#[tauri::command(async)]
pub fn ai_status(app: AppState<'_>) -> Result<AiStatus, AppError> {
    app.ai_status()
}

#[tauri::command(async)]
pub fn ai_set_key(app: AppState<'_>, provider: Provider, key: String) -> Result<(), AppError> {
    app.ai_set_key(&provider, &key)
}

#[tauri::command(async)]
pub fn ai_test(app: AppState<'_>, provider: Provider) -> Result<TestView, AppError> {
    app.ai_test(&provider)
}

#[tauri::command(async)]
pub fn ai_usage(app: AppState<'_>) -> Result<UsageSummary, AppError> {
    app.ai_usage()
}

#[tauri::command(async)]
pub fn ai_estimate(
    app: AppState<'_>,
    id: u64,
    scope: Scope,
    task: Task,
) -> Result<EstimateView, AppError> {
    app.ai_estimate(id, &scope, &task)
}

#[derive(Clone, Serialize)]
struct EventPayload<T: Serialize> {
    run: u64,
    #[serde(flatten)]
    data: T,
}

/// Lance l'action et rend aussitôt son numéro (pour l'annuler).
#[tauri::command(async)]
pub fn ai_start(
    handle: AppHandle,
    id: u64,
    scope: Scope,
    task: Task,
    consent: bool,
) -> Result<u64, AppError> {
    let app = handle.state::<Arc<App>>().inner().clone();
    let run = app.ai_start()?;
    let number = run.0;
    tauri::async_runtime::spawn_blocking(move || {
        let result = app.ai_run(id, &scope, &task, run, consent, &mut |event| {
            let _ = handle.emit(
                "ai-event",
                EventPayload {
                    run: number,
                    data: event,
                },
            );
        });
        let _ = handle.emit(
            "ai-done",
            EventPayload {
                run: number,
                data: result,
            },
        );
    });
    Ok(number)
}

#[tauri::command(async)]
pub fn ai_cancel(app: AppState<'_>, run: u64) {
    app.ai_cancel(run);
}
