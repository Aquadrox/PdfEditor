//! Moteur PDF de l'éditeur, construit sur PDFium (le moteur de Chrome).
//!
//! PDFium n'est pas utilisable depuis plusieurs threads : [`PdfiumEngine`]
//! le confie à un thread dédié qui traite les demandes une par une (patron
//! « acteur »). Le reste de l'application ne voit que le trait
//! [`pde_core::engine::PdfEngine`].

mod convert;
mod display;
mod engine;
mod library;
mod reader;

pub use engine::PdfiumEngine;
pub use library::{LIBRARY_DIR_ENV, library_candidates};
