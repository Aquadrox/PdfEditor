//! Sauvegarde automatique et récupération après plantage (F-02).
//!
//! Chaque lancement écrit dans son propre dossier de session, verrouillé
//! tant qu'il tourne. Au démarrage, une session qu'on peut verrouiller est
//! celle d'un lancement terminé sans fermeture normale : ses documents sont
//! proposés à la récupération. Une fermeture normale efface sa session.

mod manifest;
mod store;

pub use manifest::{ImportRecord, Recovered, Snapshot};
pub use store::{RecoveryStore, RecoveryView};
