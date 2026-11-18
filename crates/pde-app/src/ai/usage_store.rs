//! Consommation de l'IA et fournisseurs déjà acceptés, dans `ai-usage.json`
//! du dossier de données. Jamais de texte : des compteurs et des noms d'hôtes.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pde_core::ai::Usage;
use pde_core::settings::Pricing;
use pde_core::usage::{UsageLog, UsageSummary};
use serde::{Deserialize, Serialize};

use crate::files::write_atomic;
use crate::lock;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct UsageFile {
    log: UsageLog,
    /// Fournisseurs à qui l'utilisateur a accepté d'envoyer du texte.
    consented_hosts: Vec<String>,
}

pub struct UsageStore {
    path: Option<PathBuf>,
    data: Mutex<UsageFile>,
}

impl UsageStore {
    /// Fichier dans `data_dir` ; illisible : on repart de zéro.
    pub fn open(data_dir: &Path) -> Self {
        let path = data_dir.join("ai-usage.json");
        let data = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path: Some(path),
            data: Mutex::new(data),
        }
    }

    /// En mémoire seulement (tests).
    pub fn in_memory() -> Self {
        Self {
            path: None,
            data: Mutex::new(UsageFile::default()),
        }
    }

    pub fn record(&self, day: &str, usage: &Usage, pricing: &Pricing) {
        let mut data = lock(&self.data);
        data.log.record(day, usage, pricing);
        self.save(&data);
    }

    pub fn summary(&self, today: &str) -> UsageSummary {
        lock(&self.data).log.summary(today)
    }

    pub fn consented(&self, host: &str) -> bool {
        lock(&self.data).consented_hosts.iter().any(|h| h == host)
    }

    pub fn consent(&self, host: &str) {
        let mut data = lock(&self.data);
        if !data.consented_hosts.iter().any(|h| h == host) {
            data.consented_hosts.push(host.to_owned());
            self.save(&data);
        }
    }

    fn save(&self, data: &UsageFile) {
        let Some(path) = &self.path else { return };
        match serde_json::to_vec_pretty(data) {
            Ok(bytes) => {
                if let Err(e) = write_atomic(path, &bytes) {
                    log::warn!("consommation de l'IA non enregistrée : {e}");
                }
            }
            Err(e) => log::warn!("consommation de l'IA non enregistrée : {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_and_consent_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("pde-ai-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = UsageStore::open(&dir);
        let usage = Usage {
            input_tokens: 100,
            cached_input_tokens: 0,
            output_tokens: 10,
        };
        store.record("2026-11-18", &usage, &Pricing::default());
        store.consent("api.deepseek.com");
        store.consent("api.deepseek.com");
        let reopened = UsageStore::open(&dir);
        assert_eq!(reopened.summary("2026-11-18").today.requests, 1);
        assert!(reopened.consented("api.deepseek.com"));
        assert!(!reopened.consented("api.openai.com"));
        let file = std::fs::read_to_string(dir.join("ai-usage.json")).unwrap();
        assert_eq!(file.matches("api.deepseek.com").count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
