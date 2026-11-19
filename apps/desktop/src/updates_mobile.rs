//! Mises à jour sur Android (A-04), comme VoiceToText : le module de mise à
//! jour de Tauri n'existe pas sur mobile. Même interface que `updates.rs`.
//!
//! Vérification au démarrage puis toutes les 6 h (réglage `updates.check`)
//! du manifeste `latest-android.json` publié par la CI ; annonce dans le
//! bandeau ; sur appui de « Mettre à jour » seulement : autorisations,
//! téléchargement (HTTPS, taille et SHA-256 publiés vérifiés), puis
//! installation par Android (`apk_installer`), qui exige la même clé de
//! signature que l'application installée et refuse une version plus ancienne.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use pde_app::apk_download;
use pde_core::apk_update::{self, ApkRelease};
use pde_core::changelog::{self, Platform, Release};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::apk_installer::{ApkInstaller, InstallEvent};

/// Manifeste publié à chaque tag (à part de `latest.json`, lu par le PC).
const MANIFEST: &str =
    "https://github.com/Aquadrox/PdfEditor/releases/latest/download/latest-android.json";
/// Seules adresses de téléchargement acceptées.
const DOWNLOADS: &str = "https://github.com/Aquadrox/PdfEditor/releases/download/";
const FIRST_CHECK: Duration = Duration::from_secs(30);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const AVAILABLE_EVENT: &str = "update-available";
const PROGRESS_EVENT: &str = "update-progress";
/// Suite de l'installation donnée par Android : `{ state, message }`.
const INSTALL_EVENT: &str = "update-install";

/// Système dont l'application affiche les notes de version.
pub fn platform() -> Platform {
    Platform::Android
}

#[derive(Default)]
struct Inner {
    pending: Option<ApkRelease>,
    installing: bool,
}

pub struct Updates {
    inner: Mutex<Inner>,
    /// Version publiée (APK de release) : la version de test, signée avec
    /// une autre clé, ne peut pas être mise à jour.
    configured: bool,
    auto: AtomicBool,
}

pub type UpdatesState = Arc<Updates>;

impl Updates {
    pub fn new(_app: &AppHandle, auto: bool) -> UpdatesState {
        let configured = !cfg!(debug_assertions);
        if !configured {
            log::info!("mises à jour : version de test, vérification désactivée");
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

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        // Connexion bloquée : le bouton « Mettre à jour » revient au plus tard.
        .timeout_recv_body(Some(Duration::from_secs(15 * 60)))
        .user_agent(concat!("PdfEditor-Android/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Lit le manifeste et garde la version à proposer.
fn fetch(current: &str) -> Result<Option<ApkRelease>, String> {
    let response = agent().get(MANIFEST).call().map_err(|e| match e {
        ureq::Error::StatusCode(404) => {
            "aucune version Android publiée avec mise à jour automatique".to_string()
        }
        ureq::Error::StatusCode(status) => format!("GitHub a répondu HTTP {status}"),
        other => other.to_string(),
    })?;
    let manifest = apk_download::read_manifest(response.into_body().into_reader())
        .map_err(|e| e.to_string())?;
    apk_update::select(&manifest, current, DOWNLOADS).map_err(|e| e.to_string())
}

/// Vérifie une fois ; renvoie la version disponible.
async fn check(app: &AppHandle, updates: &Updates) -> Result<Option<String>, String> {
    let current = app.package_info().version.to_string();
    let fetched = tauri::async_runtime::spawn_blocking(move || fetch(&current))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
    match fetched {
        Ok(Some(release)) => {
            let version = release.version.clone();
            let first = {
                let mut inner = updates.lock();
                let first =
                    inner.pending.as_ref().map(|r| r.version.as_str()) != Some(version.as_str());
                inner.pending = Some(release);
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

/// Autorisations, téléchargement vérifié, puis installation par Android, qui
/// ferme PdfEditor le temps de la mise à jour. La suite (confirmation,
/// échec) arrive par l'événement `update-install`.
pub async fn install(app: &AppHandle, updates: &UpdatesState) -> Result<(), String> {
    let release = {
        let mut inner = updates.lock();
        if inner.installing {
            return Err("installation déjà en cours".into());
        }
        let Some(release) = inner.pending.clone() else {
            return Err("aucune mise à jour à installer : vérifiez d'abord".into());
        };
        inner.installing = true;
        release
    };
    log::info!("installation de la version {}", release.version);
    let result = hand_over(app, updates, &release).await;
    if let Err(e) = &result {
        log::error!("mise à jour : {e}");
        updates.lock().installing = false;
    }
    result
}

async fn hand_over(
    app: &AppHandle,
    updates: &UpdatesState,
    release: &ApkRelease,
) -> Result<(), String> {
    let installer = app
        .try_state::<ApkInstaller>()
        .ok_or("installateur d'Android indisponible")?;
    let apk = installer.prepare().await?;
    let path = apk.clone();
    let downloaded = {
        let app = app.clone();
        let release = release.clone();
        tauri::async_runtime::spawn_blocking(move || download(&app, &release, &path))
    };
    downloaded.await.map_err(|e| e.to_string())??;
    let app = app.clone();
    let updates = Arc::clone(updates);
    installer
        .install(&apk, &release.version, move |event| {
            on_install_event(&app, &updates, event)
        })
        .await
}

fn download(app: &AppHandle, release: &ApkRelease, path: &str) -> Result<(), String> {
    let response = agent()
        .get(&release.url)
        .call()
        .map_err(|e| format!("téléchargement impossible : {e}"))?;
    let mut last = None;
    apk_download::download(
        response.into_body().into_reader(),
        release,
        std::path::Path::new(path),
        |percent| {
            if last != Some(percent) {
                last = Some(percent);
                let _ = app.emit(PROGRESS_EVENT, Some(percent));
            }
        },
    )
    .map_err(|e| e.to_string())
}

#[derive(Clone, Serialize)]
struct InstallNotice {
    state: String,
    message: Option<String>,
}

/// Suite donnée par Android : relayée à l'interface ; un échec rend la main.
fn on_install_event(app: &AppHandle, updates: &Updates, event: InstallEvent) {
    match event.state.as_str() {
        "failed" => {
            log::warn!(
                "mise à jour non installée : {}",
                event.message.as_deref().unwrap_or("")
            );
            updates.lock().installing = false;
        }
        state => log::info!("mise à jour : {state}"),
    }
    let _ = app.emit(
        INSTALL_EVENT,
        InstallNotice {
            state: event.state,
            message: event.message,
        },
    );
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
    let current = app.package_info().version.to_string();
    let inner = updates.lock();
    let notes = inner
        .pending
        .as_ref()
        .map(|r| changelog::releases(&r.changelog, platform(), Some(&current), Some(&r.version)))
        .unwrap_or_default();
    UpdateStatus {
        available: inner.pending.as_ref().map(|r| r.version.clone()),
        notes,
        configured: updates.configured,
        current,
    }
}

/// Vérifie tout de suite (bouton des réglages) puis renvoie l'état.
pub async fn check_now(app: &AppHandle, updates: &UpdatesState) -> Result<UpdateStatus, String> {
    if updates.configured {
        check(app, updates).await?;
    }
    Ok(status(app, updates))
}
