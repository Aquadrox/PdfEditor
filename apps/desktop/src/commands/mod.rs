//! Commandes appelées par l'interface. Aucune logique ici : chaque commande
//! délègue à `pde-app` et renvoie son résultat ou son erreur en JSON.
//!
//! `async` : exécutées hors du thread de la fenêtre, qui reste fluide
//! pendant la lecture d'un gros fichier.

pub mod document;
pub mod edit;
pub mod image;
pub mod settings;
pub mod updates;

use std::sync::Arc;

use pde_app::app::App;
use tauri::State;

pub type AppState<'a> = State<'a, Arc<App>>;
