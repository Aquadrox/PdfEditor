//! Client d'une API compatible OpenAI (`POST {adresse}/chat/completions`),
//! repris de VoiceToText : DeepSeek, OpenAI, Mistral, GLM, OpenRouter,
//! Ollama, LM Studio… Réponses en flux.

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use pde_core::ai::{AiError, CancelFlag, ChatModel, ChatReply, ChatRequest};
use pde_core::settings::Provider;
use serde_json::{Map, Value, json};

use crate::errors::describe_http_error;
use crate::stream::read_reply;
use crate::transport::Transport;

/// Fournit la clé API au moment de l'appel (trousseau du système).
pub type KeyProvider = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct OpenAiClient {
    provider: Provider,
    extra_body: Map<String, Value>,
    key: KeyProvider,
    transport: Arc<dyn Transport>,
}

impl OpenAiClient {
    pub fn new(provider: Provider, key: KeyProvider, transport: Arc<dyn Transport>) -> Self {
        let extra_body = serde_json::from_str::<Value>(provider.extra_body.trim())
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        Self {
            provider,
            extra_body,
            key,
            transport,
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.provider.normalized_base_url())
    }

    /// Clé à envoyer : jamais en clair hors de ce PC.
    fn key(&self) -> Result<Option<String>, AiError> {
        let key = (self.key)()
            .map(|k| k.trim().to_owned())
            .filter(|k| !k.is_empty() && self.provider.may_send_key());
        if key.is_none() && !self.provider.is_local() {
            return Err(AiError::NoKey);
        }
        Ok(key)
    }

    pub(crate) fn body(&self, request: &ChatRequest) -> Value {
        let host = self.provider.host().unwrap_or_default();
        let mut body = json!({
            "model": self.provider.model.trim(),
            "messages": request.messages,
            "stream": true,
        });
        // L'API OpenAI refuse `temperature` et `max_tokens` sur ses modèles à
        // raisonnement : `max_completion_tokens` et la température par défaut.
        if host == "api.openai.com" {
            body["max_completion_tokens"] = request.max_tokens.into();
        } else {
            body["temperature"] = json!(0.2);
            body["max_tokens"] = request.max_tokens.into();
        }
        // Jetons facturés en fin de flux, chez les fournisseurs qui l'acceptent.
        if matches!(
            host.as_str(),
            "api.openai.com" | "api.deepseek.com" | "openrouter.ai"
        ) {
            body["stream_options"] = json!({ "include_usage": true });
        }
        for (key, value) in &self.extra_body {
            body[key] = value.clone();
        }
        body
    }
}

impl ChatModel for OpenAiClient {
    fn chat(
        &self,
        request: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &CancelFlag,
    ) -> Result<ChatReply, AiError> {
        cancel.check()?;
        let key = self.key()?;
        let body =
            serde_json::to_vec(&self.body(request)).map_err(|e| AiError::Format(e.to_string()))?;
        let timeout = Duration::from_secs(self.provider.timeout_seconds.into());
        let mut response = self
            .transport
            .post_json(&self.endpoint(), key.as_deref(), &body, timeout)
            .map_err(AiError::Network)?;
        if response.status != 200 {
            let mut text = String::new();
            let _ = response.body.take(64 * 1024).read_to_string(&mut text);
            return Err(AiError::Http {
                status: response.status,
                message: describe_http_error(response.status, &text),
            });
        }
        read_reply(&mut response.body, on_delta, cancel)
    }

    fn label(&self) -> String {
        format!(
            "{} ({})",
            self.provider.model.trim(),
            self.provider.host().unwrap_or_default()
        )
    }
}

/// Modèles proposés par le serveur (`GET {adresse}/models`, format OpenAI,
/// servi aussi par Ollama et LM Studio).
pub fn list_models(
    provider: &Provider,
    key: Option<String>,
    transport: &dyn Transport,
) -> Result<Vec<String>, String> {
    let url = format!("{}/models", provider.normalized_base_url());
    let key = key.filter(|k| !k.trim().is_empty() && provider.may_send_key());
    let response = transport.get(&url, key.as_deref(), Duration::from_secs(10))?;
    let mut text = String::new();
    response
        .body
        .take(4 * 1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|e| format!("réponse interrompue ({e})"))?;
    if response.status != 200 {
        return Err(format!(
            "HTTP {} : {}",
            response.status,
            describe_http_error(response.status, &text)
        ));
    }
    let value: Value =
        serde_json::from_str(&text).map_err(|_| "liste des modèles illisible".to_owned())?;
    let mut models: Vec<String> = value["data"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|m| m["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    models.sort();
    Ok(models)
}

#[cfg(test)]
mod tests {
    use pde_core::ai::{ChatMessage, Usage};

    use super::*;
    use crate::FakeTransport;

    fn provider(url: &str) -> Provider {
        Provider {
            base_url: url.into(),
            model: "modele-x".into(),
            extra_body: r#"{"thinking":{"type":"disabled"}}"#.into(),
            ..Provider::default()
        }
    }

    fn client(url: &str, key: Option<&str>, transport: Arc<FakeTransport>) -> OpenAiClient {
        let key = key.map(str::to_owned);
        OpenAiClient::new(provider(url), Arc::new(move || key.clone()), transport)
    }

    fn request() -> ChatRequest {
        ChatRequest {
            messages: vec![
                ChatMessage::system("consigne"),
                ChatMessage::user("question"),
            ],
            max_tokens: 500,
        }
    }

    const STREAM: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"Oui\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":1}}\n\ndata: [DONE]\n";

    #[test]
    fn request_carries_model_messages_stream_and_extra_fields() {
        let transport = Arc::new(FakeTransport::new(vec![Ok((200, STREAM.into()))]));
        let model = client("https://api.deepseek.com", Some(" cle "), transport.clone());
        let mut deltas = Vec::new();
        let reply = model
            .chat(
                &request(),
                &mut |d| deltas.push(d.to_owned()),
                &CancelFlag::default(),
            )
            .unwrap();
        assert_eq!(reply.text, "Oui");
        assert_eq!(
            reply.usage,
            Some(Usage {
                input_tokens: 7,
                cached_input_tokens: 0,
                output_tokens: 1
            })
        );
        let sent = transport.sent.lock().unwrap();
        let (url, key, body) = &sent[0];
        assert_eq!(url, "https://api.deepseek.com/chat/completions");
        assert_eq!(key.as_deref(), Some("cle"));
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["model"], "modele-x");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 500);
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert_eq!(body["thinking"]["type"], "disabled");
        assert_eq!(body["messages"][1]["role"], "user");
    }

    #[test]
    fn openai_reasoning_models_get_their_own_parameters() {
        let model = client(
            "https://api.openai.com/v1",
            Some("k"),
            Arc::new(FakeTransport::default()),
        );
        let body = model.body(&request());
        assert_eq!(body["max_completion_tokens"], 500);
        assert!(body.get("temperature").is_none() && body.get("max_tokens").is_none());
    }

    #[test]
    fn key_is_required_remotely_and_never_sent_in_clear_text() {
        let transport = Arc::new(FakeTransport::default());
        let remote = client("https://api.mistral.ai/v1", None, transport.clone());
        let error = remote.chat(&request(), &mut |_| {}, &CancelFlag::default());
        assert_eq!(error.unwrap_err(), AiError::NoKey);
        assert!(
            transport.sent.lock().unwrap().is_empty(),
            "rien n'est parti"
        );

        // Réseau local en HTTP : la clé enregistrée n'est pas envoyée.
        let transport = Arc::new(FakeTransport::new(vec![Ok((200, STREAM.into()))]));
        let lan = client(
            "http://192.168.1.20:11434/v1",
            Some("secret"),
            transport.clone(),
        );
        lan.chat(&request(), &mut |_| {}, &CancelFlag::default())
            .unwrap();
        assert_eq!(transport.sent.lock().unwrap()[0].1, None);
    }

    #[test]
    fn http_errors_carry_the_provider_message() {
        let transport = Arc::new(FakeTransport::new(vec![Ok((
            401,
            r#"{"error":{"message":"Invalid key"}}"#.into(),
        ))]));
        let model = client("https://api.deepseek.com", Some("k"), transport);
        let error = model
            .chat(&request(), &mut |_| {}, &CancelFlag::default())
            .unwrap_err();
        assert_eq!(
            error,
            AiError::Http {
                status: 401,
                message: "clé API refusée (Invalid key)".into()
            }
        );
        let unreachable = client(
            "https://api.deepseek.com",
            Some("k"),
            Arc::new(FakeTransport::new(vec![Err("connection refused".into())])),
        );
        assert!(matches!(
            unreachable.chat(&request(), &mut |_| {}, &CancelFlag::default()),
            Err(AiError::Network(_))
        ));
    }

    #[test]
    fn models_are_listed_and_sorted() {
        let transport = FakeTransport::new(vec![Ok((
            200,
            r#"{"data":[{"id":"zeta"},{"id":"alpha"}]}"#.into(),
        ))]);
        let models = list_models(&provider("http://localhost:11434/v1"), None, &transport).unwrap();
        assert_eq!(models, ["alpha", "zeta"]);
        assert_eq!(
            transport.sent.lock().unwrap()[0].0,
            "http://localhost:11434/v1/models"
        );
    }
}
