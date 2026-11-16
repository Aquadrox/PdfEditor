//! Cas d'usage de l'éditeur : ce que fait l'application, indépendamment de
//! la fenêtre qui l'affiche. Le moteur PDF est reçu par le trait
//! [`pde_core::engine::PdfEngine`] : les tests utilisent un faux moteur.

pub mod cache;
pub mod error;
pub mod image;
pub mod text_view;
pub mod unavailable;
pub mod view;
pub mod viewer;

pub use pde_core::engine::PdfEngine;

#[cfg(test)]
mod fake;
