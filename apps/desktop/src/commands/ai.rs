//! IA (v0.5) : état, clé, test des réglages, estimation, actions en flux.
//! Une action tourne sur un fil à part ; son avancement arrive à
//! l'interface par les événements `ai-event` et `ai-done`.

use std::sync::Arc;

use pde_ai::tasks::Task;
use pde_app::actions::assistant::Proposal;
use pde_app::ai::{AiStatus, EstimateView, Scope, TestView};
use pde_app::app::{AcceptedRewrite, App};
use pde_app::error::{AppError, ErrorKind};
use pde_app::recipes::RecipeView;
use pde_app::viewer::PagesView;
use pde_app::viewer::translation::{TranslatedView, TranslationMode};
use pde_core::recipe::Recipe;
use pde_core::settings::Provider;
use pde_core::usage::UsageSummary;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

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
    recipe: Option<String>,
) -> Result<EstimateView, AppError> {
    app.ai_estimate(id, &scope, &task, recipe.as_deref())
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
    recipe: Option<String>,
    consent: bool,
) -> Result<u64, AppError> {
    let app = handle.state::<Arc<App>>().inner().clone();
    let run = app.ai_start()?;
    let number = run.0;
    tauri::async_runtime::spawn_blocking(move || {
        let result = app.ai_run(
            id,
            &scope,
            &task,
            recipe.as_deref(),
            run,
            consent,
            &mut |event| {
                let _ = handle.emit(
                    "ai-event",
                    EventPayload {
                        run: number,
                        data: event,
                    },
                );
            },
        );
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

/// Applique les propositions acceptées de l'assistant (IA-15), en une
/// opération de l'historique.
#[tauri::command(async)]
pub fn assistant_apply(
    app: AppState<'_>,
    id: u64,
    scope: Scope,
    proposals: Vec<Proposal>,
) -> Result<PagesView, AppError> {
    app.assistant_apply(id, &scope, &proposals)
}

/// Recettes de l'assistant (IA-16) : intégrées puis personnelles.
#[tauri::command(async)]
pub fn recipes_list(app: AppState<'_>) -> Result<Vec<RecipeView>, AppError> {
    app.recipes()
}

#[tauri::command(async)]
pub fn recipe_save(
    app: AppState<'_>,
    id: Option<String>,
    recipe: Recipe,
) -> Result<RecipeView, AppError> {
    app.recipe_save(id.as_deref(), recipe)
}

#[tauri::command(async)]
pub fn recipe_delete(app: AppState<'_>, id: String) -> Result<(), AppError> {
    app.recipe_delete(&id)
}

fn recipe_dialog(
    handle: &AppHandle,
    title: &str,
) -> tauri_plugin_dialog::FileDialogBuilder<tauri::Wry> {
    let mut dialog = handle
        .dialog()
        .file()
        .set_title(title)
        .add_filter("Recettes PdfEditor", &["toml"]);
    dialog = super::dialog::over_main_window(handle, dialog);
    dialog
}

/// Importe une recette choisie par l'utilisateur ; `None` si annulé.
#[tauri::command(async)]
pub fn recipe_import(handle: AppHandle) -> Result<Option<RecipeView>, AppError> {
    let Some(path) = recipe_dialog(&handle, "Importer une recette").blocking_pick_file() else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))?;
    handle.state::<Arc<App>>().recipe_import(&path).map(Some)
}

/// Exporte la recette `id` dans un fichier ; faux si annulé.
#[tauri::command(async)]
pub fn recipe_export(handle: AppHandle, id: String, name: String) -> Result<bool, AppError> {
    let file = format!("{}.toml", name.trim().replace(['/', '\\', ':'], "-"));
    let Some(path) = recipe_dialog(&handle, "Exporter la recette")
        .set_file_name(file)
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let path = path
        .into_path()
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))?;
    handle.state::<Arc<App>>().recipe_export(&id, &path)?;
    Ok(true)
}

/// Estimation d'une traduction (IA-06), avant tout envoi.
#[tauri::command(async)]
pub fn ai_translate_estimate(
    app: AppState<'_>,
    id: u64,
    scope: Scope,
    language: String,
) -> Result<EstimateView, AppError> {
    app.ai_translate_estimate(id, &scope, &language)
}

/// Lance une traduction ; avancement et résultat par `ai-event` / `ai-done`.
#[tauri::command(async)]
pub fn ai_translate_start(
    handle: AppHandle,
    id: u64,
    scope: Scope,
    language: String,
    mode: TranslationMode,
    consent: bool,
) -> Result<u64, AppError> {
    let app = handle.state::<Arc<App>>().inner().clone();
    let run = app.ai_start()?;
    let number = run.0;
    tauri::async_runtime::spawn_blocking(move || {
        let result = app.ai_translate(id, &scope, &language, mode, run, consent, &mut |event| {
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

/// Estimation d'une modification par consigne (IA-09).
#[tauri::command(async)]
pub fn ai_rewrite_estimate(
    app: AppState<'_>,
    id: u64,
    scope: Scope,
    instruction: String,
) -> Result<EstimateView, AppError> {
    app.ai_rewrite_estimate(id, &scope, &instruction)
}

/// Lance une modification ; propositions par `ai-done`, rien n'est posé.
#[tauri::command(async)]
pub fn ai_rewrite_start(
    handle: AppHandle,
    id: u64,
    scope: Scope,
    instruction: String,
    consent: bool,
) -> Result<u64, AppError> {
    let app = handle.state::<Arc<App>>().inner().clone();
    let run = app.ai_start()?;
    let number = run.0;
    tauri::async_runtime::spawn_blocking(move || {
        let result = app.ai_rewrite(id, &scope, &instruction, run, consent, &mut |event| {
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

/// Pose les modifications acceptées, en une opération.
#[tauri::command(async)]
pub fn ai_rewrite_apply(
    app: AppState<'_>,
    id: u64,
    accepted: Vec<AcceptedRewrite>,
) -> Result<TranslatedView, AppError> {
    app.ai_rewrite_apply(id, &accepted)
}
