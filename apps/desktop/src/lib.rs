//! Application : fenêtre, commandes et protocole `pde://`, sur PC
//! (`main.rs`) et sur Android (point d'entrée mobile de Tauri).
//!
//! Ce fichier ne fait qu'assembler : la logique est dans `pde-app`.

mod autosave;
mod closing;
mod commands;
mod logging;
#[cfg(mobile)]
mod mobile_files;
mod protocol;
#[cfg(desktop)]
mod updates;
// Android : pas de module de mise à jour de Tauri ; même interface, inactive.
#[cfg(mobile)]
#[path = "updates_mobile.rs"]
mod updates;

use std::sync::Arc;

use pde_ai::http::UreqTransport;
use pde_app::ai::{AiService, UsageStore};
use pde_app::app::App;
use pde_app::news::News;
use pde_app::recent::RecentFiles;
use pde_app::recipes::RecipeStore;
use pde_app::recovery::RecoveryStore;
use pde_app::settings_store::SettingsStore;
use pde_app::signing::{DrawingStore, IdentityStore};
use pde_app::unavailable::UnavailableEngine;
use pde_pdf::PdfiumEngine;
use pde_platform::dirs::AppDirs;
use pde_platform::secrets::{SystemProtector, SystemSecrets};
use tauri::Manager;

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
/// Sur Android, le dossier privé de l'application, donné par Tauri.
fn app_dirs(handle: &tauri::AppHandle) -> AppDirs {
    #[cfg(desktop)]
    let located = {
        let _ = handle;
        AppDirs::locate()
    };
    #[cfg(mobile)]
    let located = handle
        .path()
        .app_data_dir()
        .map(|dir| AppDirs::under(&dir))
        .map_err(|e| e.to_string());
    let dirs = located.unwrap_or_else(|_| AppDirs::under(&std::env::temp_dir().join("pdfeditor")));
    let _ = dirs.create();
    dirs
}

/// Assemble l'application (réglages, journaux, documents, IA, signature)
/// dans `dirs` ; renvoie aussi le réglage de vérification des mises à jour.
fn assemble(dirs: &AppDirs) -> (Arc<App>, Arc<News>, bool) {
    let settings = SettingsStore::load(dirs.settings_file());
    logging::init(&dirs.logs, settings.get().diagnostic.log_level);
    log::info!("PdfEditor {} démarre", env!("CARGO_PKG_VERSION"));

    let news = Arc::new(News::launch(
        dirs.version_file(),
        env!("CARGO_PKG_VERSION"),
        updates::platform(),
        settings.existed(),
    ));
    let check_updates = settings.get().updates.check;
    let recent = RecentFiles::load(dirs.recent_file());
    let mut app = App::new(engine(), settings, recent);
    match RecoveryStore::open(&dirs.recovery_dir()) {
        Ok(store) => app = app.with_recovery(store),
        Err(e) => log::warn!("sauvegarde automatique désactivée : {e}"),
    }
    let identities = IdentityStore::new(&dirs.data, Arc::new(SystemProtector));
    let ai = AiService::new(
        Arc::new(SystemSecrets),
        Arc::new(UreqTransport),
        UsageStore::open(&dirs.data),
    );
    let app = Arc::new(
        app.with_identities(identities, DrawingStore::new(&dirs.data))
            .with_ai(ai)
            .with_recipes(RecipeStore::new(&dirs.data)),
    );
    (app, news, check_updates)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    // Android : lecture et écriture des adresses `content://` (A-02).
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_fs::init());
    let result = builder
        .on_window_event(closing::on_window_event)
        // L'application s'assemble une fois Tauri prêt : sur Android, ses
        // dossiers viennent de lui.
        .setup(|tauri_app| {
            let handle = tauri_app.handle().clone();
            let dirs = app_dirs(&handle);
            let (app, news, check_updates) = assemble(&dirs);
            tauri_app.manage(app.clone());
            #[cfg(mobile)]
            tauri_app.manage(Arc::new(pde_app::copies::LocalCopies::new(
                &dirs.data.join("documents"),
            )));
            tauri_app.manage(dirs);
            tauri_app.manage(news);
            let updates = updates::Updates::new(&handle, check_updates);
            updates::spawn_checks(handle.clone(), updates.clone());
            tauri_app.manage(updates);
            autosave::spawn(handle, app);
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("pde", |ctx, request, responder| {
            let app = ctx.app_handle().state::<Arc<App>>().inner().clone();
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
            commands::document::shared_files,
            commands::edit::edit_view,
            commands::edit::add_element,
            commands::edit::update_element,
            commands::edit::remove_element,
            commands::edit::add_elements,
            commands::edit::update_elements,
            commands::edit::remove_elements,
            commands::edit::edit_elements,
            commands::form::form_fields,
            commands::form::set_field,
            commands::form::set_flatten,
            commands::form::document_metadata,
            commands::form::set_metadata,
            commands::ai::ai_status,
            commands::ai::ai_set_key,
            commands::ai::ai_test,
            commands::ai::ai_usage,
            commands::ai::ai_estimate,
            commands::ai::ai_start,
            commands::ai::ai_cancel,
            commands::ai::assistant_apply,
            commands::ai::ai_translate_estimate,
            commands::ai::ai_translate_start,
            commands::ai::ai_rewrite_estimate,
            commands::ai::ai_rewrite_start,
            commands::ai::ai_rewrite_apply,
            commands::ai::recipes_list,
            commands::ai::recipe_save,
            commands::ai::recipe_delete,
            commands::ai::recipe_import,
            commands::ai::recipe_export,
            commands::sign::identities,
            commands::sign::create_identity,
            commands::sign::import_identity,
            commands::sign::delete_identity,
            commands::sign::pick_certificate,
            commands::sign::document_signatures,
            commands::sign::sign_document,
            commands::sign::signature_drawings,
            commands::sign::save_signature_drawing,
            commands::sign::delete_signature_drawing,
            commands::edit::page_images,
            commands::edit::page_text_blocks,
            commands::edit::undo_edit,
            commands::edit::redo_edit,
            commands::edit::save_document,
            commands::edit::save_document_as,
            commands::pages::document_view,
            commands::pages::insert_blank_page,
            commands::pages::remove_pages,
            commands::pages::duplicate_pages,
            commands::pages::move_pages,
            commands::pages::rotate_pages,
            commands::pages::insert_pdf,
            commands::pages::insert_pdf_files,
            commands::pages::extract_pages,
            commands::image::pick_image,
            commands::image::import_image_file,
            commands::image::import_image_bytes,
            commands::recovery::recoveries,
            commands::recovery::restore_recovery,
            commands::recovery::discard_recovery,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::default_settings,
            commands::settings::action_catalog,
            commands::settings::pick_folder,
            commands::settings::open_logs_folder,
            commands::settings::app_info,
            commands::updates::update_status,
            commands::updates::check_update,
            commands::updates::install_update,
            commands::updates::changelog_view,
            commands::updates::dismiss_update_news,
            closing::quit_app,
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(tauri_app) => tauri_app.run(|handle, event| {
            if let tauri::RunEvent::Exit = event
                && let Some(app) = handle.try_state::<Arc<App>>()
            {
                app.on_exit();
            }
        }),
        Err(error) => {
            log::error!("PdfEditor n'a pas pu démarrer : {error}");
            std::process::exit(1);
        }
    }
}
