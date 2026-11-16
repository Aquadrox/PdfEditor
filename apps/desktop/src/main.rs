//! Application de bureau : fenêtre, commandes et protocole `pde://`.
//!
//! Ce fichier ne fait qu'assembler : la logique est dans `pde-app`.

// Pas de console en plus de la fenêtre sous Windows (version publiée).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod protocol;

use std::sync::Arc;

use pde_app::unavailable::UnavailableEngine;
use pde_app::viewer::{DEFAULT_CACHE_BYTES, Viewer};
use pde_pdf::PdfiumEngine;

/// Moteur PDFium, ou moteur de remplacement qui explique pourquoi il manque.
fn engine() -> Arc<dyn pde_app::PdfEngine> {
    match PdfiumEngine::global() {
        Ok(engine) => Arc::new(engine),
        Err(error) => Arc::new(UnavailableEngine::new(error)),
    }
}

fn main() {
    let viewer = Arc::new(Viewer::new(engine(), DEFAULT_CACHE_BYTES));
    let protocol_viewer = viewer.clone();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(viewer)
        .register_asynchronous_uri_scheme_protocol("pde", move |_ctx, request, responder| {
            let viewer = protocol_viewer.clone();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(protocol::respond(&viewer, &request));
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_document,
            commands::close_document,
            commands::page_text,
            commands::search_document,
            commands::pick_pdf,
            commands::initial_files,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        // Pas encore de journal (v0.1, étape suivante) : sortie d'erreur.
        #[allow(clippy::print_stderr)]
        {
            eprintln!("PdfEditor n'a pas pu démarrer : {error}");
        }
        std::process::exit(1);
    }
}
