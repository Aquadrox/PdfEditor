//! Mises à jour (repris de VoiceToText) : vérification régulière des
//! Releases GitHub, ou sur demande depuis les réglages ; annonce dans un
//! bandeau ; installation sur clic seulement.
//!
//! L'installeur téléchargé n'est exécuté qu'après vérification de sa
//! signature (clé publique dans tauri.conf.json) et de la version qu'elle
//! couvre (`requireSignedVersion`, contre le retour forcé à une ancienne
//! version). Sans clé publique configurée, rien n'est vérifié ni proposé.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use pde_core::changelog::{self, Platform, Release};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Première vérification après le démarrage (ne pas ralentir le lancement).
const FIRST_CHECK: Duration = Duration::from_secs(30);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Mise à jour trouvée (version), pour le bandeau de l'interface.
const AVAILABLE_EVENT: &str = "update-available";
/// Progression du téléchargement (en %), pour le bandeau.
const PROGRESS_EVENT: &str = "update-progress";

/// Système dont l'application affiche les notes de version.
pub fn platform() -> Platform {
    if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

#[derive(Default)]
struct Inner {
    pending: Option<Update>,
    installing: bool,
}

pub struct Updates {
    inner: Mutex<Inner>,
    /// Clé publique présente dans la configuration.
    configured: bool,
    /// Vérification régulière (réglage `updates.check`), modifiable à chaud.
    auto: AtomicBool,
}

pub type UpdatesState = Arc<Updates>;

impl Updates {
    pub fn new(app: &AppHandle, auto: bool) -> UpdatesState {
        let configured = app
            .config()
            .plugins
            .0
            .get("updater")
            .and_then(|u| u.get("pubkey"))
            .and_then(|k| k.as_str())
            .is_some_and(|k| !k.trim().is_empty());
        if !configured {
            log::info!("mises à jour : clé publique absente, vérification désactivée");
        }
        Arc::new(Self {
            inner: Mutex::new(Inner::default()),
            configured,
            auto: AtomicBool::new(auto),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Lance les vérifications périodiques (sautées tant que le réglage est coupé).
pub fn spawn_checks(app: AppHandle, updates: UpdatesState) {
    if !updates.configured {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if updates.auto.load(Ordering::SeqCst) {
                let _ = check(&app, &updates).await;
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Réglage enregistré : active ou coupe la vérification régulière ; à
/// l'activation, vérifie tout de suite.
pub fn set_auto(app: &AppHandle, updates: &UpdatesState, enabled: bool) {
    let was = updates.auto.swap(enabled, Ordering::SeqCst);
    if enabled && !was && updates.configured {
        let app = app.clone();
        let updates = Arc::clone(updates);
        tauri::async_runtime::spawn(async move {
            let _ = check(&app, &updates).await;
        });
    }
}

/// Vérifie une fois ; renvoie la version disponible.
async fn check(app: &AppHandle, updates: &Updates) -> Result<Option<String>, String> {
    let updater = app.updater().map_err(|e| {
        log::warn!("mises à jour : {e}");
        e.to_string()
    })?;
    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let first = {
                let mut inner = updates.lock();
                let first =
                    inner.pending.as_ref().map(|u| u.version.as_str()) != Some(version.as_str());
                inner.pending = Some(update);
                first
            };
            if first {
                log::info!("mise à jour disponible : {version}");
                let _ = app.emit(AVAILABLE_EVENT, &version);
            }
            Ok(Some(version))
        }
        Ok(None) => {
            updates.lock().pending = None;
            Ok(None)
        }
        Err(e) => {
            log::warn!("vérification des mises à jour : {e}");
            Err(format!("vérification impossible : {e}"))
        }
    }
}

/// Télécharge, vérifie la signature, installe et relance.
pub async fn install(app: &AppHandle, updates: &Updates) -> Result<(), String> {
    let update = {
        let mut inner = updates.lock();
        if inner.installing {
            return Err("installation déjà en cours".into());
        }
        let Some(update) = inner.pending.take() else {
            return Err("aucune mise à jour à installer : vérifiez d'abord".into());
        };
        inner.installing = true;
        update
    };
    log::info!("installation de la version {}", update.version);
    let mut received: u64 = 0;
    let mut last_percent = None;
    // Sous Windows, l'installeur ferme l'application puis la relance.
    let result = update
        .download_and_install(
            |chunk, total| {
                received += chunk as u64;
                let percent = total
                    .filter(|t| *t > 0)
                    .map(|t| (received * 100 / t).min(100));
                if percent != last_percent {
                    last_percent = percent;
                    let _ = app.emit(PROGRESS_EVENT, percent);
                }
            },
            || {},
        )
        .await;
    if let Err(e) = result {
        log::error!("mise à jour : {e}");
        let mut inner = updates.lock();
        inner.installing = false;
        inner.pending = Some(update);
        return Err(format!("échec de la mise à jour : {e}"));
    }
    app.restart();
}

/// État affiché par le bandeau et les réglages.
#[derive(Serialize)]
pub struct UpdateStatus {
    current: String,
    available: Option<String>,
    /// Nouveautés de la version disponible (et des versions sautées).
    notes: Vec<Release>,
    configured: bool,
}

pub fn status(app: &AppHandle, updates: &Updates) -> UpdateStatus {
    let inner = updates.lock();
    UpdateStatus {
        current: app.package_info().version.to_string(),
        available: inner.pending.as_ref().map(|u| u.version.clone()),
        notes: inner
            .pending
            .as_ref()
            .map(pending_notes)
            .unwrap_or_default(),
        configured: updates.configured,
    }
}

/// Vérifie tout de suite (bouton des réglages) puis renvoie l'état.
pub async fn check_now(app: &AppHandle, updates: &UpdatesState) -> Result<UpdateStatus, String> {
    if updates.configured {
        check(app, updates).await?;
    }
    Ok(status(app, updates))
}

/// Nouveautés publiées avec la mise à jour proposée (champ `changelog` du
/// manifeste, ajouté par la CI), depuis la version installée.
fn pending_notes(update: &Update) -> Vec<Release> {
    let text = update
        .raw_json
        .get("changelog")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    changelog::releases(
        text,
        platform(),
        Some(&update.current_version),
        Some(&update.version),
    )
}
