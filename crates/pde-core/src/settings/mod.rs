//! Réglages de l'application (R-01 à R-09), enregistrés dans `config.toml`.
//!
//! Lecture tolérante : une valeur invalide reprend sa valeur par défaut et
//! produit un avertissement affiché à l'utilisateur ; un fichier illisible
//! donne les réglages par défaut. Une rubrique = une section TOML.

mod shortcuts;

use serde::{Deserialize, Serialize};

pub use shortcuts::{Action, Shortcuts, normalize as normalize_shortcut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Celui du système (clair ou sombre).
    #[default]
    System,
    Light,
    Dark,
}

/// Zoom à l'ouverture d'un document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultZoom {
    #[default]
    FitWidth,
    FitPage,
    /// Taille réelle.
    Actual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub theme: Theme,
    /// Documents récents affichés sur l'accueil (0 : aucun, liste non tenue).
    pub recent_files: u32,
    /// Rouvrir le dernier document au démarrage.
    pub reopen_last: bool,
}

pub const RECENT_FILES_RANGE: std::ops::RangeInclusive<u32> = 0..=50;

impl Default for General {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            recent_files: 10,
            reopen_last: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Display {
    pub default_zoom: DefaultZoom,
    /// Mémoire gardée pour les images de pages déjà rendues (Mo).
    pub page_cache_mb: u32,
}

pub const PAGE_CACHE_MB_RANGE: std::ops::RangeInclusive<u32> = 64..=2048;

impl Default for Display {
    fn default() -> Self {
        Self {
            default_zoom: DefaultZoom::FitWidth,
            page_cache_mb: 256,
        }
    }
}

/// Rubrique Fichiers (R-04).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Files {
    /// Intervalle de la sauvegarde automatique des modifications (F-02),
    /// en secondes ; 0 : désactivée.
    pub autosave_seconds: u32,
}

pub const AUTOSAVE_SECONDS_RANGE: std::ops::RangeInclusive<u32> = 0..=3600;
/// En dessous, une sauvegarde automatique écrirait sans cesse.
pub const MIN_AUTOSAVE_SECONDS: u32 = 10;

impl Default for Files {
    fn default() -> Self {
        Self {
            autosave_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Updates {
    /// Vérifier régulièrement si une nouvelle version est publiée. Rien
    /// n'est jamais installé sans l'accord de l'utilisateur.
    pub check: bool,
}

impl Default for Updates {
    fn default() -> Self {
        Self { check: true }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Diagnostic {
    pub log_level: LogLevel,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub display: Display,
    pub files: Files,
    pub shortcuts: Shortcuts,
    pub updates: Updates,
    pub diagnostic: Diagnostic,
}

/// Réglages lus, et ce qui a dû être corrigé pour les lire.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub settings: Settings,
    pub warnings: Vec<String>,
}

impl Settings {
    /// Lit `config.toml` sans jamais échouer : voir le commentaire du module.
    pub fn from_toml(text: &str) -> Loaded {
        let mut warnings = Vec::new();
        let parsed = match toml::from_str::<Settings>(text) {
            Ok(settings) => settings,
            Err(e) => {
                warnings.push(format!(
                    "config.toml illisible, réglages par défaut utilisés : {}",
                    e.message()
                ));
                Settings::default()
            }
        };
        let settings = parsed.clamped(&mut warnings).unwrap_or_else(|e| {
            warnings.push(format!("{e} : raccourcis par défaut rétablis"));
            Settings {
                shortcuts: Shortcuts::default(),
                ..parsed
            }
        });
        Loaded { settings, warnings }
    }

    pub fn to_toml(&self) -> String {
        // Sérialiser ces structures simples ne peut pas échouer.
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// Vérifie des réglages saisis dans l'interface : toute valeur hors
    /// limites est refusée avec son explication.
    pub fn validated(&self) -> Result<Settings, String> {
        let mut warnings = Vec::new();
        let settings = self.clamped(&mut warnings)?;
        match warnings.first() {
            Some(first) => Err(first.clone()),
            None => Ok(settings),
        }
    }

    /// Ramène chaque valeur dans ses limites, en notant chaque correction.
    fn clamped(&self, warnings: &mut Vec<String>) -> Result<Settings, String> {
        let mut s = self.clone();
        clamp(
            &mut s.general.recent_files,
            RECENT_FILES_RANGE,
            "general.recent_files",
            warnings,
        );
        clamp(
            &mut s.display.page_cache_mb,
            PAGE_CACHE_MB_RANGE,
            "display.page_cache_mb",
            warnings,
        );
        clamp(
            &mut s.files.autosave_seconds,
            AUTOSAVE_SECONDS_RANGE,
            "files.autosave_seconds",
            warnings,
        );
        if (1..MIN_AUTOSAVE_SECONDS).contains(&s.files.autosave_seconds) {
            warnings.push(format!(
                "files.autosave_seconds = {} : au moins {MIN_AUTOSAVE_SECONDS} (0 pour désactiver)",
                s.files.autosave_seconds
            ));
            s.files.autosave_seconds = MIN_AUTOSAVE_SECONDS;
        }
        s.shortcuts = self.shortcuts.validated(warnings)?;
        Ok(s)
    }
}

fn clamp(
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    name: &str,
    warnings: &mut Vec<String>,
) {
    let clamped = (*value).clamp(*range.start(), *range.end());
    if clamped != *value {
        warnings.push(format!(
            "{name} = {} hors de {} à {} : ramené à {clamped}",
            *value,
            range.start(),
            range.end()
        ));
        *value = clamped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults_without_warning() {
        let loaded = Settings::from_toml("");
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn round_trip_through_toml() {
        let mut settings = Settings::default();
        settings.general.theme = Theme::Dark;
        settings.display.default_zoom = DefaultZoom::FitPage;
        settings.diagnostic.log_level = LogLevel::Debug;
        settings.updates.check = false;
        let loaded = Settings::from_toml(&settings.to_toml());
        assert_eq!(loaded.settings, settings);
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn readable_example() {
        let loaded = Settings::from_toml(
            "[general]\ntheme = \"light\"\nrecent_files = 5\n\n[shortcuts]\nsearch = \"F3\"\n",
        );
        assert_eq!(loaded.settings.general.theme, Theme::Light);
        assert_eq!(loaded.settings.general.recent_files, 5);
        // Valeurs absentes : défauts de leur section.
        assert!(!loaded.settings.general.reopen_last);
        assert_eq!(loaded.settings.shortcuts.0[&Action::Search], "F3");
        assert_eq!(loaded.settings.shortcuts.0[&Action::Open], "Ctrl+O");
    }

    #[test]
    fn autosave_interval_is_bounded_and_can_be_off() {
        let off = Settings::from_toml("[files]\nautosave_seconds = 0\n");
        assert_eq!(off.settings.files.autosave_seconds, 0);
        assert!(off.warnings.is_empty());
        let too_fast = Settings::from_toml("[files]\nautosave_seconds = 2\n");
        assert_eq!(too_fast.settings.files.autosave_seconds, 10);
        assert_eq!(too_fast.warnings.len(), 1);
        let too_slow = Settings::from_toml("[files]\nautosave_seconds = 99999\n");
        assert_eq!(too_slow.settings.files.autosave_seconds, 3600);
        assert_eq!(Settings::default().files.autosave_seconds, 60);
        let mut ui = Settings::default();
        ui.files.autosave_seconds = 5;
        assert!(ui.validated().is_err());
    }

    #[test]
    fn out_of_range_values_are_clamped_with_a_warning() {
        let loaded = Settings::from_toml("[display]\npage_cache_mb = 10\n");
        assert_eq!(loaded.settings.display.page_cache_mb, 64);
        assert_eq!(loaded.warnings.len(), 1);
    }

    #[test]
    fn broken_file_gives_defaults_and_explains() {
        for broken in ["[general\ntheme =", "[general]\ntheme = \"violet\"\n"] {
            let loaded = Settings::from_toml(broken);
            assert_eq!(loaded.settings, Settings::default());
            assert!(
                loaded.warnings[0].contains("illisible"),
                "{:?}",
                loaded.warnings
            );
        }
    }

    #[test]
    fn duplicate_shortcuts_in_file_reset_shortcuts_only() {
        let loaded =
            Settings::from_toml("[general]\nrecent_files = 3\n[shortcuts]\nopen = \"Ctrl+F\"\n");
        assert_eq!(loaded.settings.general.recent_files, 3);
        assert_eq!(loaded.settings.shortcuts, Shortcuts::default());
        assert_eq!(loaded.warnings.len(), 1);
    }

    #[test]
    fn interface_values_out_of_range_are_refused() {
        let mut settings = Settings::default();
        settings.general.recent_files = 99;
        assert!(settings.validated().is_err());
        settings.general.recent_files = 20;
        assert!(settings.validated().is_ok());
    }
}
