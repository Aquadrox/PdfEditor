//! Pont vers l'installateur d'Android (A-04) : petit module Kotlin
//! (`gen/android/…/ApkUpdaterPlugin.kt`), seul moyen d'appeler le
//! `PackageInstaller` d'Android. Rust télécharge et vérifie l'APK ; Kotlin
//! obtient les autorisations, relit le paquet et l'installe.

use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

/// Module Kotlin chargé au démarrage (absent hors Android).
pub struct ApkInstaller(PluginHandle<Wry>);

/// Résultat de l'installation envoyé plus tard par Android.
#[derive(Debug, Deserialize)]
pub struct InstallEvent {
    /// `confirm` (fenêtre d'Android affichée), `installed` ou `failed`.
    pub state: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Deserialize)]
struct Prepared {
    apk: String,
}

#[derive(Serialize)]
struct InstallArgs<'a> {
    path: &'a str,
    version: &'a str,
    results: Channel<serde_json::Value>,
}

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("apk-updater")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle =
                    api.register_android_plugin("fr.aquadrox.pdfeditor", "ApkUpdaterPlugin")?;
                app.manage(ApkInstaller(handle));
            }
            #[cfg(not(target_os = "android"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}

impl ApkInstaller {
    /// Autorisations (installer des applications, notifications) ; renvoie
    /// le chemin où ranger l'APK. Peut attendre le retour des réglages d'Android.
    pub async fn prepare(&self) -> Result<String, String> {
        self.0
            .run_mobile_plugin_async::<Prepared>("prepare", ())
            .await
            .map(|prepared| prepared.apk)
            .map_err(|e| e.to_string())
    }

    /// Confie l'APK vérifié à Android ; `on_result` reçoit la suite (fenêtre
    /// de confirmation, échec…), sur un autre fil.
    pub async fn install(
        &self,
        path: &str,
        version: &str,
        on_result: impl Fn(InstallEvent) + Send + Sync + 'static,
    ) -> Result<(), String> {
        let results = Channel::new(move |body| {
            let event = match body {
                InvokeResponseBody::Json(json) => serde_json::from_str(&json),
                InvokeResponseBody::Raw(bytes) => serde_json::from_slice(&bytes),
            };
            match event {
                Ok(event) => on_result(event),
                Err(e) => log::warn!("mise à jour : résultat d'Android illisible : {e}"),
            }
            Ok(())
        });
        let args = InstallArgs {
            path,
            version,
            results,
        };
        self.0
            .run_mobile_plugin_async::<serde_json::Value>("install", args)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
