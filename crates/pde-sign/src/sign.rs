//! Signer un PDF (S-05, S-06) : préparer la mise à jour incrémentale avec un
//! trou pour le CMS, calculer l'empreinte de tout le reste, signer, remplir.

use crate::appearance::Appearance;
use crate::cms::{self, TimestampAuthority};
use crate::error::{Result, SignError};
use crate::identity::Identity;
use crate::pdf::date;
use crate::pdf::prepare::{self, Request};
use crate::pdf::sig_dict::{self, Details};
use crate::pdf::source::Source;

pub use crate::pdf::prepare::Target;
pub use crate::pdf::sig_dict::Lock;

/// Marge du CMS au-delà des certificats : signature (RSA 4096 = 512
/// octets), attributs signés et structure.
const CMS_OVERHEAD: usize = 3 * 1024;
/// Place d'un jeton d'horodatage (CMS du serveur, avec sa chaîne de certificats).
const TIMESTAMP_RESERVE: usize = 16 * 1024;

/// Ce que demande l'utilisateur.
#[derive(Clone)]
pub struct SignOptions<'a> {
    pub identity: &'a Identity,
    pub target: Target,
    /// Motif (« J'approuve ce document »), facultatif.
    pub reason: Option<String>,
    /// Lieu (« Lyon »), facultatif.
    pub location: Option<String>,
    pub lock: Lock,
    /// Instant de la signature, secondes depuis 1970 (UTC).
    pub time: u64,
    /// Ce qu'affiche le cadre d'une signature visible.
    pub appearance: Appearance,
    /// Serveur d'horodatage (PAdES B-T), ou aucun.
    pub timestamp: Option<&'a dyn TimestampAuthority>,
    /// Mot de passe d'un document protégé.
    pub password: Option<&'a str>,
}

/// Le PDF signé : `pdf` inchangé suivi de la mise à jour qui porte la signature.
pub fn sign(pdf: &[u8], options: &SignOptions<'_>) -> Result<Vec<u8>> {
    let source = Source::load(pdf, options.password)?;
    let identity = options.identity;
    let info = identity.info()?;
    let date = date::format(options.time);
    let reserve = if options.timestamp.is_some() {
        TIMESTAMP_RESERVE
    } else {
        0
    };
    let capacity = capacity_for(identity)? + reserve;
    let request = Request {
        target: &options.target,
        details: Details {
            name: Some(&info.common_name),
            reason: options.reason.as_deref(),
            location: options.location.as_deref(),
            contact: info.email.as_deref(),
            date: &date,
            lock: options.lock,
        },
        appearance: &options.appearance,
    };
    let prepared = prepare::prepare(&source, &request, capacity)?;
    let trailer = source.trailer(crate::rng::bytes::<16>()?)?;
    let written = prepared
        .update
        .write(pdf, &trailer, source.xref_style(), source.encryption())?;
    let base = *written
        .bodies
        .get(&prepared.sig_id)
        .ok_or_else(|| SignError::Encoding("dictionnaire de signature absent".into()))?;
    let mut bytes = written.bytes;

    // Positions : tout sauf `<…>` de /Contents est couvert.
    let open = base + prepared.sig.contents_at;
    let close = open + 2 * prepared.sig.capacity + 2;
    let range = [0, open, close, bytes.len() - close];
    let text = sig_dict::byte_range_text(&range);
    let at = base + prepared.sig.byte_range_at;
    if text.len() != prepared.sig.byte_range_len {
        return Err(SignError::TooLarge);
    }
    bytes[at..at + text.len()].copy_from_slice(text.as_bytes());

    let digest = identity
        .key()
        .digest_algorithm()
        .digest(&[&bytes[..open], &bytes[close..]]);
    let signature = cms::sign_detached(identity, &digest, options.timestamp)?;
    if signature.len() > prepared.sig.capacity {
        return Err(SignError::TooLarge);
    }
    let hex = crate::certificate::hex_upper(&signature);
    bytes[open + 1..open + 1 + hex.len()].copy_from_slice(hex.as_bytes());
    Ok(bytes)
}

/// Place réservée au CMS : certificats joints et marge.
fn capacity_for(identity: &Identity) -> Result<usize> {
    let certificates: usize = identity.chain_der()?.iter().map(Vec::len).sum();
    Ok((certificates + CMS_OVERHEAD).next_multiple_of(1024))
}
