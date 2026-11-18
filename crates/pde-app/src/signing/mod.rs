//! Signature des documents (S-01 à S-07) vue de l'application : identités
//! enregistrées, signatures du document ouvert, demande de signature.

pub mod identities;
pub mod view;

pub use identities::{IdentityStore, IdentityView, Origin};
pub use view::{LockChoice, SignRequest, SignatureState, SignatureView, TargetRequest};
