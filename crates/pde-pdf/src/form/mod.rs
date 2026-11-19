//! Formulaire (E-08) et propriétés du document (E-11).
//!
//! PDFium sait lire les champs et dessiner leur apparence, mais pas écrire
//! leur valeur proprement ni changer les propriétés du document : ces deux
//! écritures passent par `lopdf` sur le contenu enregistré (`patch.rs`),
//! qui demande aux lecteurs de redessiner les champs (`NeedAppearances`) ;
//! le document est ensuite rechargé par PDFium, qui les redessine. L'arbre
//! des champs est lu par `lopdf` (`structure.rs`) et placé sur les pages
//! par PDFium (`widgets.rs`). Les champs créés, déplacés, réglés ou
//! supprimés (E-12) sont écrits avant les valeurs (`design.rs`).

mod appearance;
mod design;
mod field_settings;
mod look;
mod patch;
mod structure;
mod text_appearance;
mod tree;
mod widgets;

pub(crate) use design::Designed;
pub(crate) use patch::{FormPatch, patch};
pub(crate) use widgets::form_fields;
