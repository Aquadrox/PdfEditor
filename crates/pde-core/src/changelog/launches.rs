//! Versions lancées sur cet appareil : nouveautés à montrer après une mise
//! à jour, jusqu'à ce que l'utilisateur les ait vues (`version.json`).

use serde::{Deserialize, Serialize};

use super::{Platform, Release, parse_version, releases};

/// Versions lancées : voir le commentaire du module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Launches {
    /// Dernière version lancée.
    pub last_run: String,
    /// Mise à jour dont les nouveautés n'ont pas encore été vues : version
    /// d'avant (vide si inconnue : installation antérieure à ce suivi).
    pub unseen_since: Option<String>,
}

impl Launches {
    /// Lancement de `current`. `existing_install` : l'application avait déjà
    /// servi (réglages présents), utile quand rien n'a encore été enregistré.
    /// Renvoie vrai juste après une mise à jour.
    pub fn launch(&mut self, current: &str, existing_install: bool) -> bool {
        let updated = if self.last_run.is_empty() {
            existing_install
        } else {
            matches!(
                (parse_version(&self.last_run), parse_version(current)),
                (Some(before), Some(now)) if before < now
            )
        };
        if updated && self.unseen_since.is_none() {
            // Deux mises à jour sans ouvrir l'accueil : les nouveautés des deux.
            self.unseen_since = Some(self.last_run.clone());
        }
        self.last_run = current.to_owned();
        updated
    }

    /// Nouveautés de la mise à jour pas encore vues (vide si aucune). Version
    /// d'avant inconnue : celles de la version installée seulement.
    pub fn news(&self, text: &str, platform: Platform) -> Vec<Release> {
        let Some(since) = self.unseen_since.as_deref() else {
            return Vec::new();
        };
        let mut news = releases(text, platform, Some(since), Some(&self.last_run));
        if parse_version(since).is_none() {
            news.truncate(1);
        }
        news
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "## 0.22.0 — 2026-11-08\n- Commun.\n- [Linux] Linux.\n\n## 0.21.0\n- [Linux] Ancien.\n\n## 0.20.0\n- Plus ancien.\n";

    fn versions(releases: &[Release]) -> Vec<&str> {
        releases.iter().map(|r| r.version.as_str()).collect()
    }

    #[test]
    fn launches() {
        // Première installation : rien à annoncer.
        let mut fresh = Launches::default();
        assert!(!fresh.launch("0.22.0", false));
        assert_eq!(fresh.unseen_since, None);
        assert!(!fresh.launch("0.22.0", true));

        // Mise à jour : nouveautés depuis la version d'avant, gardées jusqu'à
        // ce qu'elles soient vues, même après une seconde mise à jour.
        let mut seen = fresh.clone();
        assert!(seen.launch("0.23.0", true));
        assert_eq!(seen.unseen_since.as_deref(), Some("0.22.0"));
        assert!(seen.launch("0.24.0", true));
        assert_eq!(seen.unseen_since.as_deref(), Some("0.22.0"));
        assert!(!seen.launch("0.24.0", true));
        assert_eq!(seen.last_run, "0.24.0");

        // Retour à une version plus ancienne : rien de neuf.
        let mut older = fresh.clone();
        assert!(!older.launch("0.21.0", true));
        assert_eq!(older.unseen_since, None);

        // Mise à jour depuis une version d'avant ce suivi (rien d'enregistré).
        let mut legacy = Launches::default();
        assert!(legacy.launch("0.22.0", true));
        assert_eq!(legacy.unseen_since.as_deref(), Some(""));
    }

    #[test]
    fn news_after_update() {
        let mut launches = Launches {
            last_run: "0.20.0".into(),
            unseen_since: None,
        };
        assert!(launches.news(SAMPLE, Platform::Linux).is_empty());
        launches.launch("0.22.0", true);
        let news = launches.news(SAMPLE, Platform::Linux);
        assert_eq!(versions(&news), ["0.22.0", "0.21.0"]);
        // Version d'avant inconnue : seulement la version installée.
        launches.unseen_since = Some(String::new());
        assert_eq!(
            versions(&launches.news(SAMPLE, Platform::Linux)),
            ["0.22.0"]
        );
        // Vu : plus rien.
        launches.unseen_since = None;
        assert!(launches.news(SAMPLE, Platform::Windows).is_empty());
    }
}
