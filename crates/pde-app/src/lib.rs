//! Cas d'usage de l'éditeur : ce que fait l'application, indépendamment de
//! la fenêtre qui l'affiche. Le moteur PDF est reçu par le trait
//! [`pde_core::engine::PdfEngine`] : les tests utilisent un faux moteur.

pub mod app;
pub mod autosave;
pub mod cache;
pub mod edit_view;
pub mod error;
pub mod files;
pub mod images;
pub mod news;
pub mod page_png;
pub mod recent;
pub mod recovery;
pub mod settings_store;
pub mod text_view;
pub mod unavailable;
pub mod view;
pub mod viewer;

pub use pde_core::engine::PdfEngine;

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Verrou même si un autre thread a paniqué en le tenant : les données
/// protégées (tables, caches, listes) restent cohérentes à chaque instruction.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod fake;
