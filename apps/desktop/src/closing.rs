//! Fermeture de la fenêtre : si un document a des modifications non
//! enregistrées, la fermeture est suspendue et l'interface demande quoi
//! faire (enregistrer, ne pas enregistrer, annuler) avant `quit_app`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pde_app::app::App;
use tauri::{AppHandle, Emitter, Manager, Window, WindowEvent};

/// Événement envoyé à l'interface quand la fermeture attend sa réponse.
const CLOSE_REQUESTED_EVENT: &str = "close-requested";

/// L'utilisateur a confirmé : la prochaine fermeture n'est plus retenue.
static CONFIRMED: AtomicBool = AtomicBool::new(false);

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let dirty = window.state::<Arc<App>>().viewer.any_dirty();
    if dirty && !CONFIRMED.load(Ordering::SeqCst) {
        api.prevent_close();
        let _ = window.emit(CLOSE_REQUESTED_EVENT, ());
    }
}

/// Fermeture confirmée par l'interface (modifications enregistrées ou abandonnées).
#[tauri::command]
pub fn quit_app(handle: AppHandle) {
    CONFIRMED.store(true, Ordering::SeqCst);
    // Choix fait : rien à proposer au prochain lancement.
    handle.state::<Arc<App>>().shutdown();
    handle.exit(0);
}
