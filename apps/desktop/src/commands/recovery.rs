//! Récupération après plantage (F-02).

use pde_app::edit_view::EditView;
use pde_app::error::AppError;
use pde_app::recovery::RecoveryView;

use super::AppState;

/// Documents laissés par un lancement terminé sans fermeture normale.
#[tauri::command(async)]
pub fn recoveries(app: AppState<'_>) -> Vec<RecoveryView> {
    app.recoveries()
}

/// Reprend une récupération dans le document `id`, déjà rouvert.
#[tauri::command(async)]
pub fn restore_recovery(
    app: AppState<'_>,
    id: u64,
    recovery_id: String,
) -> Result<EditView, AppError> {
    app.restore_recovery(id, &recovery_id)
}

#[tauri::command(async)]
pub fn discard_recovery(app: AppState<'_>, recovery_id: String) {
    app.discard_recovery(&recovery_id);
}
