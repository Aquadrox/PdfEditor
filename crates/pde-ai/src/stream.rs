//! Lecture d'une réponse de `/chat/completions` : en flux (SSE, lignes
//! `data: {…}`) ou d'un bloc (JSON), selon ce que renvoie le serveur.

use std::io::BufRead;

use pde_core::ai::{AiError, CancelFlag, ChatReply, Usage};
use serde_json::Value;

/// Texte et jetons de la réponse ; `on_delta` reçoit chaque morceau.
pub fn read_reply(
    body: &mut dyn BufRead,
    on_delta: &mut dyn FnMut(&str),
    cancel: &CancelFlag,
) -> Result<ChatReply, AiError> {
    let mut text = String::new();
    let mut usage = None;
    let mut whole = String::new();
    let mut streamed = false;
    let mut line = String::new();
    loop {
        cancel.check()?;
        line.clear();
        let read = body
            .read_line(&mut line)
            .map_err(|e| AiError::Network(format!("réponse interrompue ({e})")))?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        let Some(data) = trimmed.strip_prefix("data:") else {
            if !streamed {
                whole.push_str(&line);
            }
            continue;
        };
        streamed = true;
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let chunk: Value = serde_json::from_str(data)
            .map_err(|_| AiError::Format("morceau de réponse illisible".into()))?;
        if let Some(message) = chunk["error"]["message"].as_str() {
            return Err(AiError::Refused(message.chars().take(160).collect()));
        }
        if let Some(delta) = chunk["choices"][0]["delta"]["content"].as_str()
            && !delta.is_empty()
        {
            text.push_str(delta);
            on_delta(delta);
        }
        if let Some(u) = parse_usage(&chunk["usage"]) {
            usage = Some(u);
        }
    }
    if streamed {
        return Ok(ChatReply { text, usage });
    }
    // Serveur qui ignore `stream` : une réponse JSON d'un bloc.
    let value: Value = serde_json::from_str(whole.trim())
        .map_err(|_| AiError::Format("réponse illisible".into()))?;
    let text = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| AiError::Format("réponse sans texte".into()))?
        .to_owned();
    on_delta(&text);
    Ok(ChatReply {
        text,
        usage: parse_usage(&value["usage"]),
    })
}

/// Format OpenAI, avec les variantes de cache de DeepSeek et d'OpenAI.
pub fn parse_usage(value: &Value) -> Option<Usage> {
    if !value.is_object() {
        return None;
    }
    let input = value["prompt_tokens"].as_u64().unwrap_or(0);
    let output = value["completion_tokens"].as_u64().unwrap_or(0);
    let cached = value["prompt_cache_hit_tokens"]
        .as_u64()
        .or_else(|| value["prompt_tokens_details"]["cached_tokens"].as_u64())
        .unwrap_or(0);
    Some(Usage {
        input_tokens: input,
        cached_input_tokens: cached.min(input),
        output_tokens: output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(body: &str) -> (Result<ChatReply, AiError>, Vec<String>) {
        let mut deltas = Vec::new();
        let reply = read_reply(
            &mut body.as_bytes(),
            &mut |d| deltas.push(d.to_owned()),
            &CancelFlag::default(),
        );
        (reply, deltas)
    }

    #[test]
    fn streamed_reply_arrives_piece_by_piece_with_usage() {
        let body = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"Bon\"}}]}\n\n\
                    : commentaire\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"jour\"}}]}\n\n\
                    data: {\"choices\":[],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":2,\"prompt_cache_hit_tokens\":4}}\n\n\
                    data: [DONE]\n\n";
        let (reply, deltas) = read(body);
        let reply = reply.unwrap();
        assert_eq!(reply.text, "Bonjour");
        assert_eq!(deltas, ["Bon", "jour"]);
        assert_eq!(
            reply.usage,
            Some(Usage {
                input_tokens: 12,
                cached_input_tokens: 4,
                output_tokens: 2
            })
        );
    }

    #[test]
    fn whole_json_reply_is_accepted() {
        let body = r#"{"choices":[{"message":{"content":"Réponse"}}],"usage":{"prompt_tokens":5,"completion_tokens":1,"prompt_tokens_details":{"cached_tokens":2}}}"#;
        let (reply, deltas) = read(body);
        let reply = reply.unwrap();
        assert_eq!(reply.text, "Réponse");
        assert_eq!(deltas, ["Réponse"]);
        assert_eq!(reply.usage.unwrap().cached_input_tokens, 2);
    }

    #[test]
    fn errors_inside_the_stream_and_garbage_are_reported() {
        let (reply, _) = read("data: {\"error\":{\"message\":\"quota\"}}\n");
        assert_eq!(reply.unwrap_err(), AiError::Refused("quota".into()));
        let (reply, _) = read("<html>");
        assert!(matches!(reply.unwrap_err(), AiError::Format(_)));
    }

    #[test]
    fn cancellation_stops_reading() {
        let cancel = CancelFlag::default();
        cancel.cancel();
        let reply = read_reply(&mut "data: [DONE]\n".as_bytes(), &mut |_| {}, &cancel);
        assert_eq!(reply.unwrap_err(), AiError::Cancelled);
    }
}
