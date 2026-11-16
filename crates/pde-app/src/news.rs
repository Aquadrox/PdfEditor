//! Nouveautés : versions lancées sur ce poste (`version.json`), pour montrer
//! ce qui a changé après une mise à jour, jusqu'à ce que ce soit vu.

use std::path::PathBuf;
use std::sync::Mutex;

use pde_core::changelog::{self, Launches, Platform, Release};
use serde::Serialize;

use crate::files::write_atomic;
use crate::lock;

/// Notes de version pour l'interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangelogView {
    pub current: String,
    /// Nouveautés de chaque version jusqu'à l'installée, la plus récente d'abord.
    pub releases: Vec<Release>,
    /// Version d'avant une mise à jour pas encore vue (vide si inconnue) ;
    /// `None` : rien à annoncer.
    pub updated_from: Option<String>,
    /// Nouveautés de cette mise à jour (bandeau de l'accueil).
    pub news: Vec<Release>,
}

pub struct News {
    path: PathBuf,
    current: String,
    platform: Platform,
    launches: Mutex<Launches>,
}

impl News {
    /// Note le lancement de `current`. `existing_install` : l'application
    /// avait déjà servi (réglages présents) avant ce suivi.
    pub fn launch(
        path: PathBuf,
        current: &str,
        platform: Platform,
        existing_install: bool,
    ) -> Self {
        let mut launches: Launches = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if launches.launch(current, existing_install) {
            log::info!("mis à jour : version {current}");
        }
        let news = Self {
            path,
            current: current.to_owned(),
            platform,
            launches: Mutex::new(launches),
        };
        news.save();
        news
    }

    pub fn view(&self) -> ChangelogView {
        let launches = lock(&self.launches);
        ChangelogView {
            current: self.current.clone(),
            releases: changelog::releases(
                changelog::BUNDLED,
                self.platform,
                None,
                Some(&self.current),
            ),
            updated_from: launches.unseen_since.clone(),
            news: launches.news(changelog::BUNDLED, self.platform),
        }
    }

    /// Bandeau « mis à jour » fermé : nouveautés vues.
    pub fn dismiss(&self) {
        let seen = lock(&self.launches).unseen_since.take().is_some();
        if seen {
            self.save();
        }
    }

    fn save(&self) {
        let json = serde_json::to_vec_pretty(&*lock(&self.launches)).unwrap_or_default();
        if let Err(e) = write_atomic(&self.path, &json) {
            log::warn!("suivi des versions non enregistré : {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pde-news-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn first_installation_announces_nothing() {
        let path = temp("fresh");
        let news = News::launch(path.clone(), "0.1.0", Platform::Windows, false);
        assert_eq!(news.view().updated_from, None);
        assert!(news.view().news.is_empty());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn update_is_announced_until_dismissed() {
        let path = temp("update");
        News::launch(path.clone(), "0.0.1", Platform::Windows, false);
        let news = News::launch(path.clone(), "0.0.2", Platform::Windows, true);
        assert_eq!(news.view().updated_from.as_deref(), Some("0.0.1"));
        news.dismiss();
        assert_eq!(news.view().updated_from, None);
        // Relancé : toujours vu.
        let again = News::launch(path.clone(), "0.0.2", Platform::Windows, true);
        assert_eq!(again.view().updated_from, None);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn bundled_notes_cover_the_running_version() {
        let path = temp("bundled");
        let version = env!("CARGO_PKG_VERSION");
        let news = News::launch(path.clone(), version, Platform::Windows, false);
        assert_eq!(news.view().releases[0].version, version);
        std::fs::remove_file(path).unwrap();
    }
}
