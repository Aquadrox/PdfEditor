//! Tests du service IA : faux transport HTTP (aucune vraie API), faux trousseau.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pde_ai::FakeTransport;
use pde_ai::tasks::{PageText, Task};
use pde_core::secrets::SecretStore;
use pde_core::settings::{Ai, Pricing, Profile, Provider};

use super::*;

#[derive(Default)]
struct FakeSecrets(Mutex<HashMap<String, String>>);

impl SecretStore for FakeSecrets {
    fn load(&self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get(name).cloned()
    }
    fn store(&self, name: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), value.into());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

const TODAY: &str = "2026-11-18";

fn sse(text: &str) -> String {
    format!(
        "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\ndata: {{\"choices\":[],\"usage\":{{\"prompt_tokens\":1000,\"completion_tokens\":100}}}}\n\ndata: [DONE]\n",
        serde_json::to_string(text).unwrap()
    )
}

fn provider(url: &str) -> Provider {
    Provider {
        base_url: url.into(),
        model: "modele".into(),
        pricing: Pricing {
            input_per_million: 1.0,
            cached_input_per_million: 0.1,
            output_per_million: 2.0,
            currency: "EUR".into(),
        },
        ..Provider::default()
    }
}

fn ai() -> Ai {
    Ai {
        provider: provider("https://api.deepseek.com"),
        ..Ai::default()
    }
}

fn pages() -> Vec<PageText> {
    vec![PageText {
        page: 0,
        text: "Le contrat dure deux ans.".into(),
    }]
}

fn service(
    responses: Vec<Result<(u16, String), String>>,
) -> (AiService, Arc<FakeTransport>, Arc<FakeSecrets>) {
    let transport = Arc::new(FakeTransport::new(responses));
    let secrets = Arc::new(FakeSecrets::default());
    let service = AiService::new(secrets.clone(), transport.clone(), UsageStore::in_memory());
    (service, transport, secrets)
}

#[test]
fn key_is_kept_per_provider_and_never_shown() {
    let (service, _, secrets) = service(vec![]);
    let settings = ai();
    assert!(!service.status(&settings).has_key);
    service.set_key(&settings.provider, "  sk-test  ").unwrap();
    assert_eq!(
        secrets.load("ai-key/api.deepseek.com").as_deref(),
        Some("sk-test")
    );
    let status = service.status(&settings);
    assert!(status.has_key && status.configured && !status.local);
    assert_eq!(status.label, "modele (api.deepseek.com)");
    service.set_key(&settings.provider, "").unwrap();
    assert!(!service.status(&settings).has_key);
}

#[test]
fn first_send_to_a_provider_needs_consent() {
    let (service, transport, _) = service(vec![Ok((200, sse("Deux ans [p. 1].")))]);
    let settings = ai();
    service.set_key(&settings.provider, "k").unwrap();
    let task = Task::Question("Durée ?".into());
    let estimate = service.estimate(&settings, &pages(), &task, TODAY);
    assert!(estimate.needs_confirmation && !estimate.over_cap);
    assert_eq!(estimate.host, "api.deepseek.com");

    let refused = service.run(
        &settings,
        &pages(),
        &task,
        TODAY,
        service.start(),
        false,
        &mut |_| {},
    );
    assert_eq!(refused.unwrap_err().message, "envoi non confirmé");
    assert!(
        transport.sent.lock().unwrap().is_empty(),
        "rien n'est parti"
    );

    let mut streamed = String::new();
    let answer = service
        .run(
            &settings,
            &pages(),
            &task,
            TODAY,
            service.start(),
            true,
            &mut |event| {
                if let AiEvent::Delta { text } = event {
                    streamed.push_str(&text);
                }
            },
        )
        .unwrap();
    assert_eq!(answer.text, "Deux ans [p. 1].");
    assert_eq!(streamed, answer.text);
    assert!((answer.cost - (1000.0 * 1.0 + 100.0 * 2.0) / 1_000_000.0).abs() < 1e-12);
    // Accord gardé : le petit envoi suivant passe sans question.
    assert!(
        !service
            .estimate(&settings, &pages(), &task, TODAY)
            .needs_confirmation
    );
    let usage = service.usage(TODAY);
    assert_eq!(usage.today.requests, 1);
    assert_eq!(usage.today.output_tokens, 100);
}

#[test]
fn monthly_cap_blocks_sending() {
    let (service, transport, _) = service(vec![]);
    let mut settings = ai();
    settings.monthly_cap = 0.000_001;
    service.set_key(&settings.provider, "k").unwrap();
    let estimate = service.estimate(&settings, &pages(), &Task::Summary, TODAY);
    assert!(estimate.over_cap);
    let error = service
        .run(
            &settings,
            &pages(),
            &Task::Summary,
            TODAY,
            service.start(),
            true,
            &mut |_| {},
        )
        .unwrap_err();
    assert!(error.message.contains("plafond"), "{error}");
    assert!(transport.sent.lock().unwrap().is_empty());
}

#[test]
fn local_server_needs_no_key_nor_consent() {
    let (service, transport, _) = service(vec![Ok((200, sse("Résumé")))]);
    let settings = Ai {
        provider: provider("http://localhost:11434/v1"),
        confirm_above: 1.0,
        ..Ai::default()
    };
    assert!(
        !service
            .estimate(&settings, &pages(), &Task::Summary, TODAY)
            .needs_confirmation
    );
    let answer = service
        .run(
            &settings,
            &pages(),
            &Task::Summary,
            TODAY,
            service.start(),
            false,
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(answer.text, "Résumé");
    assert_eq!(
        transport.sent.lock().unwrap()[0].1,
        None,
        "aucune clé envoyée"
    );
}

#[test]
fn fallback_profile_answers_when_the_provider_fails() {
    let (service, transport, _) = service(vec![
        Ok((503, r#"{"error":{"message":"surcharge"}}"#.into())),
        Ok((200, sse("Réponse de secours"))),
    ]);
    let mut settings = ai();
    settings.profiles = vec![Profile {
        name: "Local".into(),
        provider: provider("http://localhost:11434/v1"),
    }];
    settings.fallback = "Local".into();
    service.set_key(&settings.provider, "k").unwrap();
    let answer = service
        .run(
            &settings,
            &pages(),
            &Task::Summary,
            TODAY,
            service.start(),
            true,
            &mut |_| {},
        )
        .unwrap();
    assert!(answer.fallback_used);
    assert_eq!(answer.text, "Réponse de secours");
    let sent = transport.sent.lock().unwrap();
    assert_eq!(sent[1].0, "http://localhost:11434/v1/chat/completions");
}

#[test]
fn cancelled_action_reports_it() {
    let (service, _, _) = service(vec![Ok((200, sse("x")))]);
    let settings = ai();
    service.set_key(&settings.provider, "k").unwrap();
    let run = service.start();
    service.cancel(run.0);
    let error = service
        .run(
            &settings,
            &pages(),
            &Task::Summary,
            TODAY,
            run,
            true,
            &mut |_| {},
        )
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Cancelled);
}

#[test]
fn unconfigured_ai_explains_where_to_set_it() {
    let (service, _, _) = service(vec![]);
    let error = service
        .run(
            &Ai::default(),
            &pages(),
            &Task::Summary,
            TODAY,
            service.start(),
            true,
            &mut |_| {},
        )
        .unwrap_err();
    assert!(error.message.contains("Réglages"), "{error}");
}

#[test]
fn settings_test_reports_models_and_hints() {
    let (first, _, _) = service(vec![
        Ok((200, r#"{"data":[{"id":"b"},{"id":"a"}]}"#.into())),
        Ok((200, sse("OK"))),
    ]);
    let local = provider("http://localhost:11434/v1");
    let result = first.test(&local);
    assert!(result.ok, "{result:?}");
    assert_eq!(result.models, ["a", "b"]);

    let (other, _, _) = service(vec![Err("x".into()), Ok((404, "{}".into()))]);
    let result = other.test(&provider("http://localhost:11434"));
    assert!(!result.ok);
    assert!(result.hints.iter().any(|h| h.contains("/v1")), "{result:?}");
}

#[test]
fn assistant_offers_catalogued_tools_and_keeps_their_calls() {
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Je caviarde le nom.\"}}]}\n\n\
               data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"redact\",\"arguments\":\"{\\\"text\\\":\\\"deux ans\\\"}\"}}]}}]}\n\n\
               data: [DONE]\n";
    let (service, transport, _) = service(vec![Ok((200, sse.into()))]);
    let settings = ai();
    service.set_key(&settings.provider, "k").unwrap();
    let task = Task::Assist("caviarde la durée".into());
    let estimate = service.estimate(&settings, &pages(), &task, TODAY);
    assert_eq!(estimate.requests, 1);
    let answer = service
        .run(
            &settings,
            &pages(),
            &task,
            TODAY,
            service.start(),
            true,
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(answer.text, "Je caviarde le nom.");
    assert_eq!(answer.tool_calls[0].name, "redact");
    let sent = transport.sent.lock().unwrap();
    let body: serde_json::Value = serde_json::from_str(&sent[0].2).unwrap();
    let names: Vec<&str> = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["function"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "edit_text",
            "highlight",
            "add_note",
            "redact",
            "rotate_pages",
            "remove_pages"
        ]
    );
}
