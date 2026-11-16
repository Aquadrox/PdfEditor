//! Mises à jour et notes de version.

use std::sync::Arc;

use pde_app::news::{ChangelogView, News};
use tauri::{AppHandle, State};

use crate::updates::{self, UpdateStatus, UpdatesState};

#[tauri::command]
pub fn update_status(app: AppHandle, updates: State<'_, UpdatesState>) -> UpdateStatus {
    updates::status(&app, &updates)
}

/// Bouton « Rechercher une mise à jour » des réglages.
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    updates: State<'_, UpdatesState>,
) -> Result<UpdateStatus, String> {
    updates::check_now(&app, updates.inner()).await
}

/// Bouton « Mettre à jour » : télécharge, vérifie, installe et relance.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    updates: State<'_, UpdatesState>,
) -> Result<(), String> {
    let updates = updates.inner().clone();
    updates::install(&app, &updates).await
}

#[tauri::command]
pub fn changelog_view(news: State<'_, Arc<News>>) -> ChangelogView {
    news.view()
}

/// Bandeau « mis à jour » fermé : nouveautés vues.
#[tauri::command]
pub fn dismiss_update_news(news: State<'_, Arc<News>>) {
    news.dismiss();
}
