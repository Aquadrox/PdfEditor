//! Faux modèle et faux transport : aucune vraie API en test (règle du
//! projet). Les réponses sont écrites à l'avance, y compris malformées ou
//! malveillantes ; chaque demande est gardée pour les vérifications.

use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::Duration;

use pde_core::ai::{AiError, CancelFlag, ChatModel, ChatReply, ChatRequest, Usage};

use crate::transport::{Response, Transport};

/// Modèle qui rend des réponses écrites à l'avance, morceau par morceau.
#[derive(Default)]
pub struct FakeChatModel {
    replies: Mutex<VecDeque<Result<String, AiError>>>,
    requests: Mutex<Vec<ChatRequest>>,
}

impl FakeChatModel {
    /// Réponses rendues dans l'ordre ; la dernière est répétée.
    pub fn new(replies: Vec<Result<String, AiError>>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Modèle qui répond toujours `text`.
    pub fn answering(text: &str) -> Self {
        Self::new(vec![Ok(text.to_owned())])
    }

    /// Demandes reçues.
    pub fn requests(&self) -> Vec<ChatRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl ChatModel for FakeChatModel {
    fn chat(
        &self,
        request: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &CancelFlag,
    ) -> Result<ChatReply, AiError> {
        cancel.check()?;
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(request.clone());
        let reply = {
            let mut replies = self.replies.lock().unwrap_or_else(|e| e.into_inner());
            if replies.len() > 1 {
                replies.pop_front()
            } else {
                replies.front().cloned()
            }
        }
        .unwrap_or_else(|| Ok(String::new()))?;
        // Réponse en flux : par mots.
        for piece in reply.split_inclusive(' ') {
            cancel.check()?;
            on_delta(piece);
        }
        let input: usize = request
            .messages
            .iter()
            .map(|m| m.content.chars().count() / 4)
            .sum();
        Ok(ChatReply {
            usage: Some(Usage {
                input_tokens: input as u64,
                cached_input_tokens: 0,
                output_tokens: (reply.chars().count() / 4) as u64,
            }),
            text: reply,
        })
    }

    fn label(&self) -> String {
        "faux modèle".into()
    }
}

/// Transport qui rend des réponses HTTP écrites à l'avance et garde les demandes.
#[derive(Default)]
pub struct FakeTransport {
    responses: Mutex<VecDeque<Result<(u16, String), String>>>,
    /// (adresse, clé envoyée, corps) de chaque demande.
    pub sent: Mutex<Vec<(String, Option<String>, String)>>,
}

impl FakeTransport {
    pub fn new(responses: Vec<Result<(u16, String), String>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            sent: Mutex::new(Vec::new()),
        }
    }

    fn next(&self, url: &str, key: Option<&str>, body: &[u8]) -> Result<Response, String> {
        self.sent.lock().unwrap_or_else(|e| e.into_inner()).push((
            url.to_owned(),
            key.map(str::to_owned),
            String::from_utf8_lossy(body).into_owned(),
        ));
        let (status, text) = self
            .responses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
            .unwrap_or_else(|| Err("plus de réponse prévue".into()))?;
        Ok(Response {
            status,
            body: Box::new(Cursor::new(text.into_bytes())),
        })
    }
}

impl Transport for FakeTransport {
    fn post_json(
        &self,
        url: &str,
        key: Option<&str>,
        body: &[u8],
        _timeout: Duration,
    ) -> Result<Response, String> {
        self.next(url, key, body)
    }

    fn get(&self, url: &str, key: Option<&str>, _timeout: Duration) -> Result<Response, String> {
        self.next(url, key, b"")
    }
}
