//! Rubrique IA (R-07, IA-01) : fournisseur compatible OpenAI, profils
//! enregistrés, profil de secours, seuils de coût, consignes modifiables.
//! Les règles d'adresse viennent de VoiceToText : HTTPS obligatoire hors du
//! réseau local, et la clé API ne part jamais en clair hors de ce PC.

use serde::{Deserialize, Serialize};

/// Prix par million de jetons, dans `currency`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pricing {
    pub input_per_million: f64,
    /// Jetons d'entrée servis par le cache du fournisseur.
    pub cached_input_per_million: f64,
    pub output_per_million: f64,
    pub currency: String,
}

impl Default for Pricing {
    fn default() -> Self {
        Self {
            input_per_million: 0.0,
            cached_input_per_million: 0.0,
            output_per_million: 0.0,
            currency: "USD".into(),
        }
    }
}

/// Un fournisseur et son modèle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Provider {
    /// Adresse de l'API (`https://api.deepseek.com`, `http://localhost:11434/v1`…) ;
    /// vide : IA non configurée.
    pub base_url: String,
    pub model: String,
    /// Délai maximal d'une requête, en secondes.
    pub timeout_seconds: u32,
    /// Jetons envoyés au plus par requête (au-delà, le document est découpé).
    pub max_input_tokens: u32,
    pub pricing: Pricing,
    /// Champs JSON ajoutés à chaque requête, propres au fournisseur (texte JSON).
    pub extra_body: String,
}

impl Default for Provider {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            model: String::new(),
            timeout_seconds: 120,
            max_input_tokens: 24_000,
            pricing: Pricing::default(),
            extra_body: String::new(),
        }
    }
}

pub const TIMEOUT_SECONDS_RANGE: std::ops::RangeInclusive<u32> = 5..=900;
pub const MAX_INPUT_TOKENS_RANGE: std::ops::RangeInclusive<u32> = 2_000..=1_000_000;
pub const MAX_PROFILES: usize = 30;

impl Provider {
    pub fn is_configured(&self) -> bool {
        !self.base_url.trim().is_empty()
    }

    /// Adresse nettoyée : espaces et `/` final retirés, `/chat/completions`
    /// retiré s'il a été recopié (l'application l'ajoute elle-même).
    pub fn normalized_base_url(&self) -> String {
        let mut url = self.base_url.trim().trim_end_matches('/');
        if let Some(stripped) = url.strip_suffix("/chat/completions") {
            url = stripped.trim_end_matches('/');
        }
        url.to_owned()
    }

    /// Nom d'hôte, sans port. Analyse stricte : l'hôte décide de la clé
    /// envoyée ; une adresse ambiguë n'a pas d'hôte et est refusée.
    pub fn host(&self) -> Option<String> {
        let url = self.normalized_base_url();
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))?;
        let authority = rest.split('/').next()?;
        let (host, port) = match authority.strip_prefix('[') {
            Some(v6) => {
                let (host, after) = v6.split_once(']')?;
                let valid = !host.is_empty()
                    && host
                        .chars()
                        .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.');
                (valid.then_some(host)?, after)
            }
            None => {
                let end = authority.find(':').unwrap_or(authority.len());
                let host = &authority[..end];
                let valid = !host.is_empty()
                    && host
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
                (valid.then_some(host)?, &authority[end..])
            }
        };
        let port_ok = match port.strip_prefix(':') {
            None => port.is_empty(),
            Some(digits) => !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()),
        };
        port_ok.then(|| host.to_ascii_lowercase())
    }

    /// Serveur sur ce PC (`localhost`, `127.x.x.x`, `::1`).
    pub fn is_loopback(&self) -> bool {
        match self.host().as_deref() {
            Some("localhost" | "::1") => true,
            Some(host) => host
                .parse::<std::net::Ipv4Addr>()
                .is_ok_and(|ip| ip.is_loopback()),
            None => false,
        }
    }

    /// Serveur sur ce PC ou le réseau local (Ollama, LM Studio…) : rien ne
    /// sort sur Internet, clé facultative, HTTP accepté.
    pub fn is_local(&self) -> bool {
        self.is_loopback()
            || self.host().is_some_and(|host| {
                host.ends_with(".local")
                    || host
                        .parse::<std::net::Ipv4Addr>()
                        .is_ok_and(|ip| ip.is_private() || ip.is_link_local())
            })
    }

    /// La clé API peut être envoyée : HTTPS, ou HTTP vers ce PC seulement.
    pub fn may_send_key(&self) -> bool {
        self.host().is_some()
            && (self.normalized_base_url().starts_with("https://") || self.is_loopback())
    }

    /// Vérifie une configuration saisie dans les réglages.
    pub fn validate(&self) -> Result<(), String> {
        if !self.is_configured() {
            return Ok(());
        }
        if self.host().is_none() {
            return Err("adresse invalide : attendu https://hôte/…".into());
        }
        if !self.normalized_base_url().starts_with("https://") && !self.is_local() {
            return Err(
                "adresse en https:// obligatoire (http:// seulement sur ce PC ou le réseau local)"
                    .into(),
            );
        }
        if self.model.trim().is_empty() {
            return Err("nom du modèle manquant".into());
        }
        if !TIMEOUT_SECONDS_RANGE.contains(&self.timeout_seconds) {
            return Err(format!(
                "délai entre {} et {} secondes",
                TIMEOUT_SECONDS_RANGE.start(),
                TIMEOUT_SECONDS_RANGE.end()
            ));
        }
        if !MAX_INPUT_TOKENS_RANGE.contains(&self.max_input_tokens) {
            return Err(format!(
                "jetons par requête entre {} et {}",
                MAX_INPUT_TOKENS_RANGE.start(),
                MAX_INPUT_TOKENS_RANGE.end()
            ));
        }
        let p = &self.pricing;
        if [
            p.input_per_million,
            p.cached_input_per_million,
            p.output_per_million,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err("tarifs : nombres positifs attendus".into());
        }
        if p.currency.len() != 3 || !p.currency.chars().all(|c| c.is_ascii_uppercase()) {
            return Err("devise : code à 3 lettres (USD, EUR…)".into());
        }
        let extra = self.extra_body.trim();
        if !extra.is_empty() && !(extra.starts_with('{') && extra.ends_with('}')) {
            return Err("paramètres supplémentaires : objet JSON attendu ({…})".into());
        }
        Ok(())
    }
}

/// Fournisseur enregistré sous un nom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(flatten)]
    pub provider: Provider,
}

/// Consignes système modifiables (IA-11) ; vide : consigne intégrée.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prompts {
    pub summary: String,
    pub key_points: String,
    pub entities: String,
    pub risks: String,
    pub question: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ai {
    /// Fournisseur utilisé.
    pub provider: Provider,
    /// Fournisseurs enregistrés.
    pub profiles: Vec<Profile>,
    /// Profil essayé si le fournisseur échoue ; vide : aucun.
    pub fallback: String,
    /// Au-delà de ce coût estimé, confirmation avant l'envoi (0 : toujours).
    pub confirm_above: f64,
    /// Plafond mensuel (0 : aucun) : au-delà, plus d'envoi ce mois-ci.
    pub monthly_cap: f64,
    /// Confirmer avant chaque envoi, quel que soit le coût.
    pub confirm_each_send: bool,
    pub prompts: Prompts,
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            provider: Provider::default(),
            profiles: Vec::new(),
            fallback: String::new(),
            confirm_above: 0.05,
            monthly_cap: 0.0,
            confirm_each_send: false,
            prompts: Prompts::default(),
        }
    }
}

impl Ai {
    /// Profil de secours, s'il existe et qu'il est configuré.
    pub fn fallback_provider(&self) -> Option<&Provider> {
        let name = self.fallback.trim();
        if name.is_empty() {
            return None;
        }
        self.profiles
            .iter()
            .find(|p| p.name.trim().eq_ignore_ascii_case(name))
            .map(|p| &p.provider)
            .filter(|p| p.is_configured())
    }

    /// Valeurs corrigées : une configuration invalide est signalée et vidée.
    pub fn checked(&self, warnings: &mut Vec<String>) -> Ai {
        let mut ai = self.clone();
        ai.provider.base_url = ai.provider.normalized_base_url();
        if let Err(e) = ai.provider.validate() {
            warnings.push(format!("ai.provider : {e} ; IA désactivée"));
            ai.provider = Provider::default();
        }
        let mut seen = std::collections::HashSet::new();
        ai.profiles.retain_mut(|profile| {
            profile.name = profile.name.trim().to_owned();
            profile.provider.base_url = profile.provider.normalized_base_url();
            let valid = !profile.name.is_empty()
                && seen.insert(profile.name.to_lowercase())
                && profile.provider.validate().is_ok()
                && profile.provider.is_configured();
            if !valid {
                warnings.push(format!(
                    "ai.profiles : profil « {} » invalide, retiré",
                    profile.name
                ));
            }
            valid
        });
        ai.profiles.truncate(MAX_PROFILES);
        for value in [&mut ai.confirm_above, &mut ai.monthly_cap] {
            if !value.is_finite() || *value < 0.0 {
                warnings.push("ai : montants positifs attendus".into());
                *value = 0.0;
            }
        }
        ai
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(url: &str) -> Provider {
        Provider {
            base_url: url.into(),
            model: "m".into(),
            ..Provider::default()
        }
    }

    #[test]
    fn hosts_are_parsed_strictly() {
        assert_eq!(
            provider("https://api.deepseek.com/").host().as_deref(),
            Some("api.deepseek.com")
        );
        assert_eq!(
            provider("http://localhost:11434/v1").host().as_deref(),
            Some("localhost")
        );
        assert_eq!(
            provider("http://[::1]:8080/v1").host().as_deref(),
            Some("::1")
        );
        assert_eq!(
            provider("https://api.deepseek.com:x@autre.site").host(),
            None
        );
        assert_eq!(provider("https://a.b?x=/c").host(), None);
        assert_eq!(provider("ftp://a.b").host(), None);
    }

    #[test]
    fn key_never_leaves_in_clear_text() {
        assert!(provider("https://api.openai.com/v1").may_send_key());
        assert!(provider("http://127.0.0.1:1234/v1").may_send_key());
        // Réseau local : HTTP accepté, mais sans clé.
        let lan = provider("http://192.168.1.20:11434/v1");
        assert!(lan.is_local() && lan.validate().is_ok() && !lan.may_send_key());
        assert!(provider("http://exemple.com/v1").validate().is_err());
    }

    #[test]
    fn pasted_endpoint_is_normalized() {
        assert_eq!(
            provider(" https://api.mistral.ai/v1/chat/completions/ ").normalized_base_url(),
            "https://api.mistral.ai/v1"
        );
    }

    #[test]
    fn invalid_settings_are_reported_and_cleared() {
        let mut warnings = Vec::new();
        let ai = Ai {
            provider: Provider {
                model: String::new(),
                ..provider("https://api.deepseek.com")
            },
            profiles: vec![
                Profile {
                    name: "Local".into(),
                    provider: provider("http://localhost:11434/v1"),
                },
                Profile {
                    name: "local".into(),
                    provider: provider("http://localhost:1234/v1"),
                },
            ],
            fallback: "LOCAL".into(),
            monthly_cap: -1.0,
            ..Ai::default()
        }
        .checked(&mut warnings);
        assert!(!ai.provider.is_configured());
        assert_eq!(ai.profiles.len(), 1, "doublon retiré");
        assert_eq!(ai.monthly_cap, 0.0);
        assert_eq!(warnings.len(), 3);
        assert_eq!(
            ai.fallback_provider().map(|p| p.base_url.as_str()),
            Some("http://localhost:11434/v1")
        );
    }

    #[test]
    fn unconfigured_ai_is_valid() {
        assert!(Provider::default().validate().is_ok());
        assert!(!Provider::default().is_configured());
    }
}
