//! Transport HTTP abstrait : le client ne connaît que ce trait. Le vrai
//! transport (`ureq`, fonction `http`) est dans [`crate::http`] ; les tests
//! rejouent des réponses écrites à la main.

use std::io::BufRead;
use std::time::Duration;

/// Réponse HTTP : statut et corps lu au fil de l'eau (réponses en flux).
pub struct Response {
    pub status: u16,
    pub body: Box<dyn BufRead + Send>,
}

pub trait Transport: Send + Sync {
    /// `POST` d'un corps JSON ; `key` part dans `Authorization: Bearer`.
    fn post_json(
        &self,
        url: &str,
        key: Option<&str>,
        body: &[u8],
        timeout: Duration,
    ) -> Result<Response, String>;

    /// `GET` (liste des modèles).
    fn get(&self, url: &str, key: Option<&str>, timeout: Duration) -> Result<Response, String>;
}
