//! Lecture d'une réponse de `/chat/completions` : en flux (SSE, lignes
//! `data: {…}`) ou d'un bloc (JSON), selon ce que renvoie le serveur.

use std::io::BufRead;

use pde_core::ai::{AiError, CancelFlag, ChatReply, ToolCall, Usage};
use serde_json::Value;

/// Appels d'outils reçus en morceaux : le nom arrive d'abord, les arguments
/// (texte JSON) par bouts, chaque appel repéré par son rang.
#[derive(Default)]
struct ToolCalls(Vec<ToolCall>);

impl ToolCalls {
    fn add_delta(&mut self, delta: &Value) {
        for call in delta.as_array().into_iter().flatten() {
            let index = call["index"].as_u64().map_or(self.0.len(), |i| i as usize);
            if index >= self.0.len() {
                // Un rang hors de la suite : un serveur fautif, pas une raison d'allouer sans fin.
                if index > self.0.len() + 16 {
                    continue;
                }
                self.0.resize_with(index + 1, || ToolCall {
                    name: String::new(),
                    arguments: String::new(),
                });
            }
            let function = &call["function"];
            if let Some(name) = function["name"].as_str() {
                self.0[index].name.push_str(name);
            }
            if let Some(arguments) = function["arguments"].as_str() {
                self.0[index].arguments.push_str(arguments);
            }
        }
    }

    fn finish(self) -> Vec<ToolCall> {
        self.0.into_iter().filter(|c| !c.name.is_empty()).collect()
    }
}

/// Appels d'outils d'une réponse JSON d'un bloc.
fn whole_tool_calls(message: &Value) -> Vec<ToolCall> {
    let mut calls = ToolCalls::default();
    for (index, call) in message["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let mut call = call.clone();
        call["index"] = Value::from(index);
        // Certains serveurs envoient les arguments en objet plutôt qu'en texte.
        if call["function"]["arguments"].is_object() {
            call["function"]["arguments"] = Value::from(call["function"]["arguments"].to_string());
        }
        calls.add_delta(&Value::Array(vec![call]));
    }
    calls.finish()
}

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
    let mut tool_calls = ToolCalls::default();
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
        tool_calls.add_delta(&chunk["choices"][0]["delta"]["tool_calls"]);
        if let Some(u) = parse_usage(&chunk["usage"]) {
            usage = Some(u);
        }
    }
    if streamed {
        return Ok(ChatReply {
            text,
            usage,
            tool_calls: tool_calls.finish(),
        });
    }
    // Serveur qui ignore `stream` : une réponse JSON d'un bloc.
    let value: Value = serde_json::from_str(whole.trim())
        .map_err(|_| AiError::Format("réponse illisible".into()))?;
    let message = &value["choices"][0]["message"];
    let tool_calls = whole_tool_calls(message);
    let text = match message["content"].as_str() {
        Some(text) => text.to_owned(),
        // Une réponse faite seulement d'appels d'outils n'a pas de texte.
        None if !tool_calls.is_empty() => String::new(),
        None => return Err(AiError::Format("réponse sans texte".into())),
    };
    if !text.is_empty() {
        on_delta(&text);
    }
    Ok(ChatReply {
        text,
        usage: parse_usage(&value["usage"]),
        tool_calls,
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
    fn streamed_tool_calls_are_put_back_together() {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Je propose\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"a\",\"function\":{\"name\":\"redact\",\"arguments\":\"\"}}]}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"text\\\":\"}}]}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"Dupont\\\"}\"}}]}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"function\":{\"name\":\"rotate_pages\",\"arguments\":\"{}\"}}]}}]}\n\n\
                    data: [DONE]\n\n";
        let (reply, deltas) = read(body);
        let reply = reply.unwrap();
        assert_eq!(deltas, ["Je propose"]);
        assert_eq!(
            reply.tool_calls,
            [
                ToolCall {
                    name: "redact".into(),
                    arguments: r#"{"text":"Dupont"}"#.into()
                },
                ToolCall {
                    name: "rotate_pages".into(),
                    arguments: "{}".into()
                }
            ]
        );
    }

    #[test]
    fn whole_json_tool_calls_without_text_are_accepted() {
        let body = r#"{"choices":[{"message":{"content":null,"tool_calls":[
            {"type":"function","function":{"name":"redact","arguments":"{\"text\":\"A\"}"}},
            {"type":"function","function":{"name":"add_note","arguments":{"text":"B"}}}
        ]}}]}"#;
        let (reply, deltas) = read(body);
        let reply = reply.unwrap();
        assert!(reply.text.is_empty() && deltas.is_empty());
        assert_eq!(reply.tool_calls.len(), 2);
        assert_eq!(reply.tool_calls[0].arguments, r#"{"text":"A"}"#);
        assert_eq!(reply.tool_calls[1].arguments, r#"{"text":"B"}"#);
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
