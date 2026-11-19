//! Domaine de l'éditeur PDF, sans dépendance au système ni au moteur PDF.
//!
//! Ce crate ne connaît ni PDFium, ni le réseau, ni l'interface : il décrit
//! le document et ses modifications, et définit les traits (ports) que les
//! autres crates implémentent. Tout ce qu'il contient se teste sous Linux.

pub mod ai;
pub mod apk_update;
pub mod blank;
pub mod changelog;
pub mod convert;
pub mod document;
pub mod edit;
pub mod engine;
pub mod formats;
pub mod geometry;
pub mod helvetica;
pub mod recipe;
pub mod render;
pub mod search;
pub mod secrets;
pub mod settings;
pub mod text;
pub mod units;
pub mod usage;
