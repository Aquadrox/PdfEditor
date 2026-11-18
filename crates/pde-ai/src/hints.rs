//! Pistes de correction pour une erreur du fournisseur, affichées par le
//! test des réglages (repris de VoiceToText).

use pde_core::settings::Provider;

pub fn hints(provider: &Provider, error: &str) -> Vec<String> {
    let url = provider.normalized_base_url();
    let lower = error.to_lowercase();
    let mut hints = Vec::new();
    let local_without_v1 = provider.is_local() && !url.ends_with("/v1");
    if lower.contains("404") || lower.contains("405") || lower.contains("introuvable") {
        if local_without_v1 {
            hints.push(format!(
                "Ollama, LM Studio et la plupart des serveurs locaux attendent une adresse qui se \
                 termine par /v1 : essayez {url}/v1"
            ));
        } else {
            hints.push(format!(
                "Cette adresse ne répond pas comme une API compatible OpenAI ({url}/chat/completions \
                 introuvable), ou le modèle n'existe pas. Vérifiez le chemin (souvent …/v1) et le \
                 nom du modèle."
            ));
        }
    }
    if lower.contains("401") || lower.contains("403") || lower.contains("clé api refusée") {
        hints.push("Clé refusée : vérifiez qu'elle correspond à ce fournisseur.".into());
    }
    if lower.contains("400") || lower.contains("422") || lower.contains("requête refusée") {
        hints.push(
            "Requête refusée : nom de modèle inexact (voir la liste des modèles) ou paramètre \
             supplémentaire non pris en charge par ce fournisseur."
                .into(),
        );
    }
    if lower.contains("402") || lower.contains("solde") {
        hints.push("Solde du compte insuffisant chez le fournisseur.".into());
    }
    if lower.contains("refused") {
        hints.push(
            "Rien n'écoute à cette adresse : le service est-il démarré, sur ce port ? Un Ollama \
             sur une autre machine doit écouter sur le réseau (OLLAMA_HOST=0.0.0.0)."
                .into(),
        );
    }
    if lower.contains("dns") || lower.contains("resolve") || lower.contains("lookup") {
        hints.push("Nom d'hôte introuvable : vérifiez l'adresse (faute de frappe ?).".into());
    }
    if lower.contains("timed out") || lower.contains("timeout") || lower.contains("délai") {
        hints.push(
            "Délai dépassé : augmentez le délai maximal. Un modèle local met souvent du temps à \
             se charger au premier appel."
                .into(),
        );
    }
    if lower.contains("aucune clé") {
        hints.push("Aucune clé pour ce fournisseur : saisissez-la dans le champ Clé API.".into());
    }
    if hints.is_empty() && local_without_v1 {
        hints.push(format!(
            "Pour un serveur local, l'adresse se termine généralement par /v1 : {url}/v1"
        ));
    }
    hints
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
    fn local_server_without_v1_is_suggested() {
        let tips = hints(
            &provider("http://localhost:11434"),
            "HTTP 404 : introuvable",
        );
        assert!(tips[0].contains("/v1"), "{tips:?}");
    }

    #[test]
    fn key_and_timeout_problems_get_their_tip() {
        let tips = hints(
            &provider("https://api.deepseek.com"),
            "HTTP 401 : clé API refusée",
        );
        assert!(tips.iter().any(|t| t.contains("Clé refusée")));
        let tips = hints(&provider("https://api.deepseek.com"), "timed out");
        assert!(tips.iter().any(|t| t.contains("Délai")));
    }
}
