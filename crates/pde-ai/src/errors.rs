//! Messages d'erreur des fournisseurs (repris de VoiceToText).

use serde_json::Value;

const MAX_ERROR_DETAIL_CHARS: usize = 160;

/// Erreur HTTP en mots d'utilisateur, avec le message du fournisseur
/// tronqué (il peut finir dans le journal, jamais le texte envoyé).
pub fn describe_http_error(status: u16, body: &str) -> String {
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let detail: String = parsed["error"]["message"]
        .as_str()
        .or_else(|| parsed["error"].as_str())
        .or_else(|| parsed["message"].as_str())
        .unwrap_or("")
        .chars()
        .take(MAX_ERROR_DETAIL_CHARS)
        .collect();
    let hint = match status {
        401 => "clé API refusée",
        402 => "solde du compte insuffisant",
        404 => "adresse ou modèle introuvable",
        429 => "trop de requêtes",
        500..=599 => "erreur du fournisseur",
        _ => "requête refusée",
    };
    if detail.is_empty() {
        hint.to_owned()
    } else {
        format!("{hint} ({detail})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_message_is_kept_and_shortened() {
        let body = r#"{"error":{"message":"Model not found"}}"#;
        assert_eq!(
            describe_http_error(404, body),
            "adresse ou modèle introuvable (Model not found)"
        );
        assert_eq!(describe_http_error(500, "<html>"), "erreur du fournisseur");
        let long = format!(r#"{{"error":"{}"}}"#, "x".repeat(400));
        assert!(describe_http_error(400, &long).chars().count() < 200);
    }
}
