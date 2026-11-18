//! Transport HTTP réel : `ureq` (HTTPS par rustls). Fonction `http`,
//! activée par l'application de bureau.

use std::io::BufReader;
use std::time::Duration;

use crate::transport::{Response, Transport};

#[derive(Default)]
pub struct UreqTransport;

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        // Lire le corps des réponses d'erreur : message du fournisseur.
        .http_status_as_error(false)
        .build()
        .new_agent()
}

impl Transport for UreqTransport {
    fn post_json(
        &self,
        url: &str,
        key: Option<&str>,
        body: &[u8],
        timeout: Duration,
    ) -> Result<Response, String> {
        let mut request = agent(timeout)
            .post(url)
            .header("Content-Type", "application/json");
        if let Some(key) = key {
            request = request.header("Authorization", &format!("Bearer {key}"));
        }
        let response = request.send(body).map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let reader = response.into_body().into_reader();
        Ok(Response {
            status,
            body: Box::new(BufReader::new(reader)),
        })
    }

    fn get(&self, url: &str, key: Option<&str>, timeout: Duration) -> Result<Response, String> {
        let mut request = agent(timeout).get(url);
        if let Some(key) = key {
            request = request.header("Authorization", &format!("Bearer {key}"));
        }
        let response = request.call().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let reader = response.into_body().into_reader();
        Ok(Response {
            status,
            body: Box::new(BufReader::new(reader)),
        })
    }
}
