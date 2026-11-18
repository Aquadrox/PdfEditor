//! Signature numérique des PDF (PAdES, S-01 à S-07).
//!
//! - [`Identity`] : clé privée et certificat, générés (auto-signé) ou importés
//!   d'un fichier PKCS#12 ;
//! - [`sign`] : ajoute une signature au PDF par une **mise à jour
//!   incrémentale** (les octets d'origine ne bougent pas, les signatures
//!   précédentes restent valides) ; CMS détaché PAdES B-B ;
//! - [`signatures`] : liste et vérifie les signatures d'un PDF ;
//! - [`append_revision`] : enregistre les modifications d'un document signé
//!   à la suite de ses signatures (F-03).
//!
//! Rust pur, sans PDFium ni système : testable partout, y compris sous
//! Android. Aucune clé n'est écrite dans un journal ni dans un message.

mod appearance;
mod certificate;
mod changes;
mod cms;
mod error;
mod identity;
mod key;
mod oids;
mod pdf;
mod revision;
mod rng;
mod sign;
mod verify;

pub use appearance::{Appearance, MAX_IMAGE_SIDE, SignatureImage};
pub use certificate::CertificateInfo;
pub use changes::Change;
#[cfg(feature = "testing")]
pub use cms::LocalTimestampAuthority;
pub use cms::TimestampAuthority;
pub use error::{Result, SignError};
pub use identity::{Identity, Subject};
pub use key::{DigestAlgorithm, PrivateKey};
pub use revision::append_revision;
pub use sign::{Lock, SignOptions, Target, sign};
pub use verify::{SignatureField, SignatureInfo, SignatureStatus, TimestampInfo, signatures};
