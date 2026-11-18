//! Erreurs présentées à l'interface : un type, pour choisir la réaction
//! (demander un mot de passe…), et un message lisible.

use pde_core::engine::EngineError;
use serde::Serialize;
use thiserror::Error;

/// Catégorie d'erreur, transmise telle quelle à l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    PasswordRequired,
    WrongPassword,
    InvalidFile,
    FileNotFound,
    FileTooLarge,
    Io,
    UnknownDocument,
    InvalidRequest,
    Engine,
    /// Modifications à enregistrer d'abord (signature).
    Unsaved,
    /// Identité ou signature.
    Signature,
    /// Fournisseur d'IA (configuration, réseau, réponse).
    Ai,
    /// Action annulée par l'utilisateur.
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Error, Serialize)]
#[error("{message}")]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<EngineError> for AppError {
    fn from(error: EngineError) -> Self {
        let kind = match &error {
            EngineError::PasswordRequired => ErrorKind::PasswordRequired,
            EngineError::WrongPassword => ErrorKind::WrongPassword,
            EngineError::InvalidFile(_) => ErrorKind::InvalidFile,
            EngineError::UnknownDocument => ErrorKind::UnknownDocument,
            EngineError::PageOutOfRange { .. } | EngineError::InvalidRender(_) => {
                ErrorKind::InvalidRequest
            }
            EngineError::Unavailable(_) | EngineError::Internal(_) => ErrorKind::Engine,
        };
        Self::new(kind, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_errors_keep_their_message() {
        let error = AppError::from(EngineError::WrongPassword);
        assert_eq!(error.kind, ErrorKind::WrongPassword);
        assert_eq!(error.message, "mot de passe incorrect");
    }

    #[test]
    fn serialized_for_the_interface() {
        let json = serde_json::to_string(&AppError::from(EngineError::PasswordRequired)).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"password_required","message":"ce document est protégé par un mot de passe"}"#
        );
    }
}
