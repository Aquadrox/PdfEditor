//! Rubrique Signature (R-05) : horodatage et verrouillage proposés.

use serde::{Deserialize, Serialize};

/// Serveur d'horodatage proposé : public, gratuit, en HTTP (les jetons
/// d'horodatage sont signés par le serveur ; seule une empreinte lui est envoyée).
pub const DEFAULT_TIMESTAMP_URL: &str = "http://timestamp.digicert.com";
const MAX_URL_CHARS: usize = 300;

/// Verrouillage proposé dans le dialogue de signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultLock {
    /// Laisser modifiable.
    #[default]
    None,
    /// Verrouiller, sauf formulaires et signatures.
    Forms,
    /// Verrouiller entièrement.
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Signature {
    /// Horodater les nouvelles signatures (S-08), coché par défaut dans le dialogue.
    pub timestamp: bool,
    /// Adresse du serveur d'horodatage RFC 3161 (`http://…`).
    pub timestamp_url: String,
    pub default_lock: DefaultLock,
}

impl Default for Signature {
    fn default() -> Self {
        Self {
            timestamp: false,
            timestamp_url: DEFAULT_TIMESTAMP_URL.into(),
            default_lock: DefaultLock::None,
        }
    }
}

impl Signature {
    /// Valeurs corrigées ; une adresse invalide reprend celle par défaut.
    pub fn checked(&self, warnings: &mut Vec<String>) -> Signature {
        let mut s = self.clone();
        let url = s.timestamp_url.trim().to_owned();
        if url.is_empty() {
            s.timestamp_url = url;
        } else if let Err(problem) = check_url(&url) {
            warnings.push(format!(
                "signature.timestamp_url : {problem} ; adresse par défaut rétablie"
            ));
            s.timestamp_url = DEFAULT_TIMESTAMP_URL.into();
        } else {
            s.timestamp_url = url;
        }
        if s.timestamp && s.timestamp_url.is_empty() {
            s.timestamp = false;
        }
        s
    }
}

/// Adresse de serveur acceptée : `http://hôte[/chemin]`, sans espace.
pub fn check_url(url: &str) -> Result<(), String> {
    if url.chars().count() > MAX_URL_CHARS {
        return Err(format!("plus de {MAX_URL_CHARS} caractères"));
    }
    if url.starts_with("https://") {
        return Err("seule une adresse http:// est possible pour l'instant".into());
    }
    let Some(rest) = url.strip_prefix("http://") else {
        return Err("adresse http:// attendue".into());
    };
    let host = rest.split(['/', '?']).next().unwrap_or_default();
    if host.is_empty() || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("adresse invalide".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_checked() {
        assert!(check_url("http://timestamp.digicert.com").is_ok());
        assert!(check_url("http://freetsa.org/tsr").is_ok());
        assert!(check_url("http://127.0.0.1:3180/tsa").is_ok());
        assert!(check_url("https://freetsa.org/tsr").is_err());
        assert!(check_url("ftp://x").is_err());
        assert!(check_url("http://").is_err());
        assert!(check_url("http://a b").is_err());
    }

    #[test]
    fn invalid_url_falls_back_with_a_warning() {
        let mut warnings = Vec::new();
        let s = Signature {
            timestamp: true,
            timestamp_url: "  https://exemple.fr  ".into(),
            ..Signature::default()
        }
        .checked(&mut warnings);
        assert_eq!(s.timestamp_url, DEFAULT_TIMESTAMP_URL);
        assert_eq!(warnings.len(), 1);
        // Sans serveur, pas d'horodatage par défaut.
        let none = Signature {
            timestamp: true,
            timestamp_url: " ".into(),
            ..Signature::default()
        }
        .checked(&mut warnings);
        assert!(!none.timestamp && none.timestamp_url.is_empty());
    }
}
