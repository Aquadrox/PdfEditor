//! Réglages enregistrés : lus au démarrage, modifiés depuis la page
//! Réglages, écrits de façon sûre.

use std::path::PathBuf;
use std::sync::Mutex;

use pde_core::settings::Settings;
use serde::Serialize;

use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;
use crate::lock;

/// Réglages et avertissements de lecture, pour la page Réglages.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingsView {
    pub settings: Settings,
    pub warnings: Vec<String>,
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
    warnings: Vec<String>,
    existed: bool,
}

impl SettingsStore {
    /// Lit `path` ; fichier absent : réglages par défaut, sans avertissement.
    pub fn load(path: PathBuf) -> Self {
        let existed = path.is_file();
        let (settings, warnings) = match std::fs::read_to_string(&path) {
            Ok(text) => {
                let loaded = Settings::from_toml(&text);
                (loaded.settings, loaded.warnings)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Settings::default(), Vec::new()),
            Err(e) => (
                Settings::default(),
                vec![format!(
                    "{} illisible ({e}) : réglages par défaut",
                    path.display()
                )],
            ),
        };
        for warning in &warnings {
            log::warn!("réglages : {warning}");
        }
        Self {
            path,
            current: Mutex::new(settings),
            warnings,
            existed,
        }
    }

    /// Le fichier existait au lancement : l'application avait déjà servi.
    pub fn existed(&self) -> bool {
        self.existed
    }

    pub fn get(&self) -> Settings {
        lock(&self.current).clone()
    }

    pub fn view(&self) -> SettingsView {
        SettingsView {
            settings: self.get(),
            warnings: self.warnings.clone(),
        }
    }

    /// Vérifie puis enregistre ; renvoie les réglages tels qu'enregistrés
    /// (raccourcis normalisés).
    pub fn save(&self, settings: &Settings) -> Result<Settings, AppError> {
        let settings = settings
            .validated()
            .map_err(|e| AppError::new(ErrorKind::InvalidRequest, e))?;
        write_atomic(&self.path, settings.to_toml().as_bytes())?;
        *lock(&self.current) = settings.clone();
        log::info!("réglages enregistrés");
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pde_core::settings::{Action, Theme};

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pde-settings-{name}-{}.toml", std::process::id()))
    }

    /// Les tests de l'interface simulent les réglages par défaut avec ce
    /// fichier : il doit rester identique à ce qu'envoie l'application.
    #[test]
    fn interface_test_fixture_matches_defaults() {
        let fixture = include_str!("../../../apps/desktop/ui-tests/fixtures/default-settings.json");
        let fixture: serde_json::Value = serde_json::from_str(fixture).unwrap();
        assert_eq!(fixture, serde_json::to_value(Settings::default()).unwrap());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let store = SettingsStore::load(temp("absent"));
        assert_eq!(store.get(), Settings::default());
        assert!(store.view().warnings.is_empty());
    }

    #[test]
    fn saved_settings_are_read_back() {
        let path = temp("save");
        let store = SettingsStore::load(path.clone());
        let mut settings = Settings::default();
        settings.general.theme = Theme::Dark;
        settings
            .shortcuts
            .0
            .insert(Action::Search, "ctrl+shift+f".into());
        let saved = store.save(&settings).unwrap();
        assert_eq!(saved.shortcuts.0[&Action::Search], "Ctrl+Shift+F");
        assert_eq!(SettingsStore::load(path.clone()).get(), saved);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_settings_are_not_saved() {
        let path = temp("invalid");
        let store = SettingsStore::load(path.clone());
        let mut settings = Settings::default();
        settings.display.page_cache_mb = 1;
        assert_eq!(
            store.save(&settings).unwrap_err().kind,
            ErrorKind::InvalidRequest
        );
        assert!(!path.exists());
        assert_eq!(store.get(), Settings::default());
    }

    #[test]
    fn broken_file_is_reported() {
        let path = temp("broken");
        std::fs::write(&path, "[general\n").unwrap();
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.view().warnings.len(), 1);
        std::fs::remove_file(path).unwrap();
    }
}
