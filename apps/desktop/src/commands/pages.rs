//! Pages (P-01, P-02) : insérer une page blanche, supprimer, dupliquer,
//! déplacer, pivoter. Numéros de page à partir de 0.

use pde_app::error::AppError;
use pde_app::view::DocumentView;
use pde_app::viewer::PagesView;

use super::AppState;

/// Description du document tel qu'affiché (après un changement des pages,
/// annulé ou rétabli).
#[tauri::command(async)]
pub fn document_view(app: AppState<'_>, id: u64) -> Result<DocumentView, AppError> {
    app.viewer.document_view(id)
}

#[tauri::command(async)]
pub fn insert_blank_page(app: AppState<'_>, id: u64, at: usize) -> Result<PagesView, AppError> {
    app.viewer.insert_blank_page(id, at)
}

#[tauri::command(async)]
pub fn remove_pages(app: AppState<'_>, id: u64, pages: Vec<usize>) -> Result<PagesView, AppError> {
    app.viewer.remove_pages(id, &pages)
}

#[tauri::command(async)]
pub fn duplicate_pages(
    app: AppState<'_>,
    id: u64,
    pages: Vec<usize>,
) -> Result<PagesView, AppError> {
    app.viewer.duplicate_pages(id, &pages)
}

/// Pages placées devant la page `to` (`to` = nombre de pages : à la fin).
#[tauri::command(async)]
pub fn move_pages(
    app: AppState<'_>,
    id: u64,
    pages: Vec<usize>,
    to: usize,
) -> Result<PagesView, AppError> {
    app.viewer.move_pages(id, &pages, to)
}

/// `turns` : quarts de tour dans le sens horaire (3 : sens inverse).
#[tauri::command(async)]
pub fn rotate_pages(
    app: AppState<'_>,
    id: u64,
    pages: Vec<usize>,
    turns: u8,
) -> Result<PagesView, AppError> {
    app.viewer.rotate_pages(id, &pages, turns)
}
