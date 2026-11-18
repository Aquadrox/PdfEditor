//! IA de l'éditeur (v0.5) : client d'une API compatible OpenAI (repris de
//! VoiceToText), et les tâches sur le document (analyse, questions).
//!
//! Le document est toujours une **donnée** : placé entre balises, balises
//! désamorcées, jamais exécuté comme consigne. Aucune vraie API en test :
//! [`FakeChatModel`] et [`FakeTransport`] (fonction `testing`).

pub mod client;
pub mod errors;
pub mod hints;
#[cfg(feature = "http")]
pub mod http;
pub mod sanitize;
pub mod stream;
pub mod tasks;
pub mod translate;
pub mod transport;

#[cfg(any(test, feature = "testing"))]
mod fake;

pub use client::{KeyProvider, OpenAiClient, list_models};
#[cfg(any(test, feature = "testing"))]
pub use fake::{FakeChatModel, FakeTransport};
pub use hints::hints;
pub use transport::{Response, Transport};
