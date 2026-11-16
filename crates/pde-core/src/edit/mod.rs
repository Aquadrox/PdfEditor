//! Modifications du document (v0.2 : éléments ajoutés).
//!
//! Le PDF d'origine n'est jamais modifié : les éléments ajoutés forment une
//! liste, posée par le moteur sur une copie du document à chaque changement.
//! L'historique garde chaque opération et son inverse (annuler, rétablir).

mod element;
mod history;
mod layout;

pub use element::{Align, Color, Element, ElementId, StandardFont, TextBox, TextStyle};
pub use history::{Change, Edits, Operation};
pub use layout::{leading, wrap};
