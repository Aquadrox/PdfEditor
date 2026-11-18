//! Application de bureau : tout est dans `lib.rs`, partagé avec Android.

// Pas de console en plus de la fenêtre sous Windows (version publiée).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pdfeditor_lib::run();
}
