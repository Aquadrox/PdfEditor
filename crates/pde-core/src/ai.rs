//! IA (v0.5) : le port [`ChatModel`] qu'implémentent le client compatible
//! OpenAI (`pde-ai`) et les faux modèles des tests, et ce qu'il échange.
//! Aucune dépendance réseau ici.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

/// Auteur d'un message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
}

/// Outil que le modèle peut appeler (« function calling ») : une action du
/// catalogue de l'éditeur, proposée à l'utilisateur, jamais exécutée seule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// Paramètres en JSON Schema (texte JSON d'un objet).
    pub parameters: String,
}

/// Appel d'outil proposé par le modèle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    /// Arguments en texte JSON, tels qu'envoyés (à vérifier avant tout usage).
    pub arguments: String,
}

/// Demande au modèle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    /// Plus longue réponse acceptée, en jetons.
    pub max_tokens: usize,
    /// Outils proposés au modèle (aucun : réponse en texte seulement).
    pub tools: Vec<ToolSpec>,
}

/// Jetons facturés pour une requête.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Jetons envoyés, y compris ceux servis par le cache du fournisseur.
    pub input_tokens: u64,
    /// Part de `input_tokens` servie par le cache (facturée moins cher).
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    pub fn add(&mut self, other: &Usage) {
        self.input_tokens += other.input_tokens;
        self.cached_input_tokens += other.cached_input_tokens;
        self.output_tokens += other.output_tokens;
    }
}

/// Réponse complète du modèle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatReply {
    pub text: String,
    /// Jetons facturés, si le fournisseur les indique.
    pub usage: Option<Usage>,
    /// Outils que le modèle demande d'appeler, dans l'ordre.
    pub tool_calls: Vec<ToolCall>,
}

/// Échec d'un appel au modèle, en mots d'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AiError {
    #[error("aucune clé API enregistrée pour ce fournisseur")]
    NoKey,
    #[error("le fournisseur répond HTTP {status} : {message}")]
    Http { status: u16, message: String },
    #[error("fournisseur injoignable : {0}")]
    Network(String),
    #[error("réponse inattendue du fournisseur : {0}")]
    Format(String),
    #[error("action IA annulée")]
    Cancelled,
    #[error("{0}")]
    Refused(String),
}

/// Annulation demandée par l'utilisateur (IA-12), partagée entre le fil qui
/// attend la réponse et l'interface.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// `Err(Cancelled)` si l'annulation a été demandée.
    pub fn check(&self) -> Result<(), AiError> {
        if self.is_cancelled() {
            Err(AiError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Un modèle de langage. `on_delta` reçoit le texte au fil de l'eau
/// (IA-12) ; un modèle qui ne sait pas répondre en flux l'appelle une fois.
pub trait ChatModel: Send + Sync {
    fn chat(
        &self,
        request: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &CancelFlag,
    ) -> Result<ChatReply, AiError>;

    /// Nom affiché (modèle et fournisseur), pour l'interface et les journaux.
    fn label(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_shared_between_clones() {
        let flag = CancelFlag::default();
        let other = flag.clone();
        assert!(flag.check().is_ok());
        other.cancel();
        assert_eq!(flag.check(), Err(AiError::Cancelled));
    }

    #[test]
    fn usage_adds_up() {
        let mut total = Usage::default();
        total.add(&Usage {
            input_tokens: 10,
            cached_input_tokens: 4,
            output_tokens: 3,
        });
        total.add(&Usage {
            input_tokens: 1,
            cached_input_tokens: 0,
            output_tokens: 2,
        });
        assert_eq!(
            total,
            Usage {
                input_tokens: 11,
                cached_input_tokens: 4,
                output_tokens: 5
            }
        );
    }
}
