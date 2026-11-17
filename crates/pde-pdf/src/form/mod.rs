//! Formulaire (E-08) et propriétés du document (E-11).
//!
//! PDFium sait lire les champs et dessiner leur apparence, mais pas écrire
//! leur valeur proprement ni changer les propriétés du document : ces deux
//! écritures passent par `lopdf` sur le contenu enregistré (`patch.rs`),
//! qui demande aux lecteurs de redessiner les champs (`NeedAppearances`) ;
//! le document est ensuite rechargé par PDFium, qui les redessine. L'arbre
//! des champs est lu par `lopdf` (`structure.rs`) et placé sur les pages
//! par PDFium (`widgets.rs`).

mod patch;
mod structure;
mod widgets;

pub(crate) use patch::patch;
pub(crate) use widgets::form_fields;
