//! Dialogues de fichiers : au-dessus de la fenêtre principale sur PC ; sur
//! Android, le sélecteur du système s'ouvre de lui-même en plein écran.

use tauri::AppHandle;
#[cfg(desktop)]
use tauri::Manager;
use tauri_plugin_dialog::FileDialogBuilder;

/// Rattache le dialogue à la fenêtre principale (PC seulement).
pub fn over_main_window(
    handle: &AppHandle,
    dialog: FileDialogBuilder<tauri::Wry>,
) -> FileDialogBuilder<tauri::Wry> {
    #[cfg(desktop)]
    if let Some(window) = handle.get_webview_window("main") {
        return dialog.set_parent(&window);
    }
    #[cfg(mobile)]
    let _ = handle;
    dialog
}
