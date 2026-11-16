//! Application de bureau : fenêtre, commandes et protocole `pde://`.
//!
//! Ce fichier ne fait qu'assembler : la logique est dans `pde-app`.

// Pas de console en plus de la fenêtre sous Windows (version publiée).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod logging;
mod protocol;

use std::sync::Arc;

use pde_app::app::App;
use pde_app::recent::RecentFiles;
use pde_app::settings_store::SettingsStore;
use pde_app::unavailable::UnavailableEngine;
use pde_pdf::PdfiumEngine;
use pde_platform::dirs::AppDirs;

/// Moteur PDFium, ou moteur de remplacement qui explique pourquoi il manque.
fn engine() -> Arc<dyn pde_app::PdfEngine> {
    match PdfiumEngine::global() {
        Ok(engine) => Arc::new(engine),
        Err(error) => {
            log::error!("{error}");
            Arc::new(UnavailableEngine::new(error))
        }
    }
}

/// Dossiers de l'application ; à défaut, un dossier temporaire (rien ne
/// sera gardé d'un lancement à l'autre, mais l'application fonctionne).
fn app_dirs() -> AppDirs {
    let dirs = AppDirs::locate()
        .unwrap_or_else(|_| AppDirs::under(&std::env::temp_dir().join("pdfeditor")));
    let _ = dirs.create();
    dirs
}

fn main() {
    let dirs = app_dirs();
    let settings = SettingsStore::load(dirs.settings_file());
    logging::init(&dirs.logs, settings.get().diagnostic.log_level);
    log::info!("PdfEditor {} démarre", env!("CARGO_PKG_VERSION"));

    let recent = RecentFiles::load(dirs.recent_file());
    let app = Arc::new(App::new(engine(), settings, recent));
    let protocol_app = app.clone();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(app)
        .manage(dirs)
        .register_asynchronous_uri_scheme_protocol("pde", move |_ctx, request, responder| {
            let app = protocol_app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(protocol::respond(&app.viewer, &request));
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::document::open_document,
            commands::document::close_document,
            commands::document::page_text,
            commands::document::search_document,
            commands::document::recent_files,
            commands::document::clear_recent_files,
            commands::document::pick_pdf,
            commands::document::initial_files,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::default_settings,
            commands::settings::open_logs_folder,
            commands::settings::app_info,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        log::error!("PdfEditor n'a pas pu démarrer : {error}");
        std::process::exit(1);
    }
}
