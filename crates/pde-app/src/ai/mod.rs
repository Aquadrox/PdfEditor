//! IA vue de l'application (IA-01 à IA-05, IA-12) : clé du fournisseur au
//! trousseau, estimation et confirmation avant l'envoi, plafond mensuel,
//! exécution annulable avec réponse en flux, profil de secours,
//! consommation. Jamais de texte du document ni de question dans les
//! journaux : seulement des longueurs et des durées.

mod usage_store;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pde_ai::tasks::{self, Event, PageText, Plan, Task};
use pde_ai::{KeyProvider, OpenAiClient, Transport};
use pde_core::ai::{AiError, CancelFlag, ChatMessage, ChatModel, ChatRequest};
use pde_core::secrets::SecretStore;
use pde_core::settings::{Ai, Provider};
use pde_core::usage::UsageSummary;
use serde::{Deserialize, Serialize};

pub use usage_store::UsageStore;

use crate::error::{AppError, ErrorKind};
use crate::lock;

/// Portée d'une action IA.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Scope {
    /// Texte sélectionné sur une page (rang à partir de 0).
    Selection {
        text: String,
        page: usize,
    },
    /// Pages choisies (rangs à partir de 0).
    Pages {
        pages: Vec<usize>,
    },
    Document,
}

/// État de l'IA pour l'interface (jamais la clé).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiStatus {
    pub configured: bool,
    /// Fournisseur et modèle (« deepseek-flash (api.deepseek.com) »).
    pub label: String,
    pub host: String,
    pub has_key: bool,
    /// Serveur sur ce PC ou le réseau local : rien ne sort sur Internet.
    pub local: bool,
}

/// Estimation montrée avant l'envoi (IA-03).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EstimateView {
    pub requests: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
    pub currency: String,
    pub characters: usize,
    pub host: String,
    /// À confirmer : premier envoi à ce fournisseur, coût au-dessus du
    /// seuil, ou confirmation demandée à chaque envoi.
    pub needs_confirmation: bool,
    /// Le plafond mensuel serait dépassé : envoi refusé.
    pub over_cap: bool,
    pub month_cost: f64,
}

/// Réponse d'une action IA.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnswerView {
    pub text: String,
    pub cost: f64,
    pub currency: String,
    /// Modèle qui a répondu (le profil de secours, le cas échéant).
    pub model: String,
    pub fallback_used: bool,
}

/// Avancement envoyé à l'interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiEvent {
    Reading { done: usize, total: usize },
    Delta { text: String },
}

/// Résultat du test des réglages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestView {
    pub ok: bool,
    pub message: String,
    pub hints: Vec<String>,
    pub models: Vec<String>,
}

pub struct AiService {
    secrets: Arc<dyn SecretStore>,
    transport: Arc<dyn Transport>,
    usage: UsageStore,
    runs: Mutex<HashMap<u64, CancelFlag>>,
    next_run: AtomicU64,
}

fn key_name(provider: &Provider) -> String {
    format!("ai-key/{}", provider.host().unwrap_or_default())
}

fn ai_error(error: AiError) -> AppError {
    let kind = match error {
        AiError::Cancelled => ErrorKind::Cancelled,
        _ => ErrorKind::Ai,
    };
    AppError::new(kind, error.to_string())
}

impl AiService {
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        transport: Arc<dyn Transport>,
        usage: UsageStore,
    ) -> Self {
        Self {
            secrets,
            transport,
            usage,
            runs: Mutex::new(HashMap::new()),
            next_run: AtomicU64::new(1),
        }
    }

    pub fn status(&self, ai: &Ai) -> AiStatus {
        let p = &ai.provider;
        AiStatus {
            configured: p.is_configured(),
            label: if p.is_configured() {
                format!("{} ({})", p.model.trim(), p.host().unwrap_or_default())
            } else {
                String::new()
            },
            host: p.host().unwrap_or_default(),
            has_key: p.is_configured() && self.key_of(p).is_some(),
            local: p.is_local(),
        }
    }

    fn key_of(&self, provider: &Provider) -> Option<String> {
        self.secrets
            .load(&key_name(provider))
            .filter(|k| !k.trim().is_empty())
    }

    /// Enregistre (ou retire, si vide) la clé du fournisseur au trousseau.
    pub fn set_key(&self, provider: &Provider, key: &str) -> Result<(), AppError> {
        if provider.host().is_none() {
            return Err(AppError::new(
                ErrorKind::InvalidRequest,
                "adresse du fournisseur invalide",
            ));
        }
        let name = key_name(provider);
        let result = if key.trim().is_empty() {
            self.secrets.delete(&name)
        } else {
            self.secrets.store(&name, key.trim())
        };
        result.map_err(|e| AppError::new(ErrorKind::Ai, e))
    }

    fn model(&self, provider: &Provider) -> Arc<dyn ChatModel> {
        let secrets = self.secrets.clone();
        let name = key_name(provider);
        let key: KeyProvider = Arc::new(move || secrets.load(&name));
        Arc::new(OpenAiClient::new(
            provider.clone(),
            key,
            self.transport.clone(),
        ))
    }

    /// Estimation de la tâche sur ces pages, avant tout envoi.
    pub fn estimate(&self, ai: &Ai, pages: &[PageText], task: &Task, today: &str) -> EstimateView {
        let provider = &ai.provider;
        let plan = Plan::new(pages, provider.max_input_tokens as usize);
        let estimate = tasks::estimate(&plan, task, &ai.prompts, &provider.pricing);
        let host = provider.host().unwrap_or_default();
        let month_cost = self.usage.summary(today).month.cost;
        EstimateView {
            requests: estimate.requests,
            input_tokens: estimate.input_tokens,
            output_tokens: estimate.output_tokens,
            cost: estimate.cost,
            currency: provider.pricing.currency.clone(),
            characters: estimate.characters,
            needs_confirmation: ai.confirm_each_send
                || estimate.cost > ai.confirm_above
                || (!provider.is_local() && !self.usage.consented(&host)),
            over_cap: ai.monthly_cap > 0.0 && month_cost + estimate.cost > ai.monthly_cap,
            month_cost,
            host,
        }
    }

    /// Nouvelle action, annulable par [`AiService::cancel`].
    pub fn start(&self) -> (u64, CancelFlag) {
        let id = self.next_run.fetch_add(1, Ordering::SeqCst);
        let flag = CancelFlag::default();
        lock(&self.runs).insert(id, flag.clone());
        (id, flag)
    }

    /// Annule l'action `run` (IA-12) ; sans effet si elle est finie.
    pub fn cancel(&self, run: u64) {
        if let Some(flag) = lock(&self.runs).get(&run) {
            flag.cancel();
        }
    }

    /// Exécute la tâche (bloquant). `consent` : l'utilisateur a accepté
    /// l'envoi à ce fournisseur. Essaie le profil de secours si le
    /// fournisseur échoue (sauf annulation).
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        ai: &Ai,
        pages: &[PageText],
        task: &Task,
        today: &str,
        run: (u64, CancelFlag),
        consent: bool,
        on_event: &mut dyn FnMut(AiEvent),
    ) -> Result<AnswerView, AppError> {
        let (id, cancel) = run;
        let result = self.run_inner(ai, pages, task, today, &cancel, consent, on_event);
        lock(&self.runs).remove(&id);
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn run_inner(
        &self,
        ai: &Ai,
        pages: &[PageText],
        task: &Task,
        today: &str,
        cancel: &CancelFlag,
        consent: bool,
        on_event: &mut dyn FnMut(AiEvent),
    ) -> Result<AnswerView, AppError> {
        if !ai.provider.is_configured() {
            return Err(AppError::new(
                ErrorKind::Ai,
                "aucun fournisseur d'IA : choisissez-en un dans les Réglages (rubrique IA)",
            ));
        }
        let estimate = self.estimate(ai, pages, task, today);
        if estimate.over_cap {
            return Err(AppError::new(
                ErrorKind::Ai,
                format!(
                    "plafond mensuel de {:.2} {} atteint : envoi refusé",
                    ai.monthly_cap, estimate.currency
                ),
            ));
        }
        if estimate.needs_confirmation && !consent {
            return Err(AppError::new(ErrorKind::Ai, "envoi non confirmé"));
        }
        if consent && !ai.provider.is_local() {
            self.usage.consent(&estimate.host);
        }
        let started = std::time::Instant::now();
        let attempt = |provider: &Provider, on_event: &mut dyn FnMut(AiEvent)| {
            let model = self.model(provider);
            let plan = Plan::new(pages, provider.max_input_tokens as usize);
            let outcome = tasks::run(
                model.as_ref(),
                task,
                &ai.prompts,
                &plan,
                &mut |event| {
                    on_event(match event {
                        Event::Reading { done, total } => AiEvent::Reading { done, total },
                        Event::Delta(text) => AiEvent::Delta { text },
                    })
                },
                cancel,
            )?;
            Ok::<_, AiError>((outcome, model.label(), provider.pricing.clone()))
        };
        let (result, fallback_used) = match attempt(&ai.provider, on_event) {
            Err(error) if !matches!(error, AiError::Cancelled | AiError::Refused(_)) => {
                match ai.fallback_provider() {
                    Some(fallback) => {
                        log::warn!("fournisseur d'IA en échec ({error}) : profil de secours");
                        (attempt(fallback, on_event), true)
                    }
                    None => (Err(error), false),
                }
            }
            other => (other, false),
        };
        let (outcome, model, pricing) = result.map_err(ai_error)?;
        self.usage.record(today, &outcome.usage, &pricing);
        log::info!(
            "action IA terminée : {} caractères envoyés, {} jetons reçus, {} ms",
            estimate.characters,
            outcome.usage.output_tokens,
            started.elapsed().as_millis()
        );
        Ok(AnswerView {
            cost: pricing.cost(&outcome.usage),
            currency: pricing.currency,
            text: outcome.text,
            model,
            fallback_used,
        })
    }

    /// Teste un fournisseur : une courte requête, et la liste des modèles.
    pub fn test(&self, provider: &Provider) -> TestView {
        if let Err(e) = provider.validate().and_then(|()| {
            provider
                .is_configured()
                .then_some(())
                .ok_or_else(|| "adresse manquante".to_owned())
        }) {
            return TestView {
                ok: false,
                hints: pde_ai::hints(provider, &e),
                message: e,
                models: Vec::new(),
            };
        }
        let models = pde_ai::list_models(provider, self.key_of(provider), self.transport.as_ref())
            .unwrap_or_default();
        let request = ChatRequest {
            messages: vec![ChatMessage::user("Réponds seulement : OK")],
            max_tokens: 16,
        };
        match self
            .model(provider)
            .chat(&request, &mut |_| {}, &CancelFlag::default())
        {
            Ok(reply) => TestView {
                ok: true,
                message: format!(
                    "Le modèle répond : « {} »",
                    reply.text.trim().chars().take(40).collect::<String>()
                ),
                hints: Vec::new(),
                models,
            },
            Err(error) => {
                let message = error.to_string();
                TestView {
                    ok: false,
                    hints: pde_ai::hints(provider, &message),
                    message,
                    models,
                }
            }
        }
    }

    pub fn usage(&self, today: &str) -> UsageSummary {
        self.usage.summary(today)
    }
}

#[cfg(test)]
mod tests;
