//! Lecture d'un certificat X.509 : ce que l'interface affiche (nom,
//! organisation, e-mail, émetteur, validité, empreinte).

use der::{Decode, Encode};
use x509_cert::Certificate;
use x509_cert::ext::pkix::name::DirectoryString;
use x509_cert::name::Name;

use crate::error::{Result, SignError};
use crate::key::DigestAlgorithm;

/// Résumé d'un certificat, sans la clé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateInfo {
    /// Nom (CN) du titulaire.
    pub common_name: String,
    /// Organisation (O) du titulaire.
    pub organization: Option<String>,
    /// Adresse électronique du titulaire.
    pub email: Option<String>,
    /// Nom (CN) de l'émetteur.
    pub issuer_common_name: String,
    /// Le certificat s'est émis lui-même (aucune autorité ne s'en porte garante).
    pub self_signed: bool,
    /// Début de validité, en secondes depuis le 1er janvier 1970 (UTC).
    pub not_before: u64,
    /// Fin de validité, même unité.
    pub not_after: u64,
    /// Empreinte SHA-256 du certificat, en hexadécimal majuscule.
    pub fingerprint: String,
}

impl CertificateInfo {
    /// Résumé du certificat donné en DER.
    pub fn from_der(der: &[u8]) -> Result<Self> {
        let certificate = Certificate::from_der(der)
            .map_err(|e| SignError::Certificate(format!("certificat illisible ({e})")))?;
        Ok(Self::from_certificate(&certificate, der))
    }

    pub(crate) fn from_certificate(certificate: &Certificate, der: &[u8]) -> Self {
        let tbs = certificate.tbs_certificate();
        let subject = tbs.subject();
        let issuer = tbs.issuer();
        let validity = tbs.validity();
        CertificateInfo {
            common_name: common_name(subject),
            organization: subject
                .organization()
                .ok()
                .flatten()
                .map(|value| directory_string(&value)),
            email: subject
                .email_address()
                .ok()
                .flatten()
                .map(|value| value.as_str().to_owned()),
            issuer_common_name: common_name(issuer),
            self_signed: subject == issuer,
            not_before: validity.not_before.to_unix_duration().as_secs(),
            not_after: validity.not_after.to_unix_duration().as_secs(),
            fingerprint: hex_upper(&DigestAlgorithm::Sha256.digest(&[der])),
        }
    }

    /// Le certificat était-il valide à cet instant (secondes depuis 1970) ?
    pub fn valid_at(&self, unix_seconds: u64) -> bool {
        (self.not_before..=self.not_after).contains(&unix_seconds)
    }
}

/// DER d'un certificat déjà décodé.
pub(crate) fn to_der(certificate: &Certificate) -> Result<Vec<u8>> {
    Ok(certificate.to_der()?)
}

fn common_name(name: &Name) -> String {
    match name.common_name() {
        Ok(Some(value)) => directory_string(&value),
        // Pas de CN : on montre le nom complet (RFC 4514).
        _ => name.to_string(),
    }
}

fn directory_string(value: &DirectoryString) -> String {
    match value {
        DirectoryString::Utf8String(text) => text.clone(),
        DirectoryString::PrintableString(text) => text.to_string(),
        DirectoryString::TeletexString(text) => text.to_string(),
        DirectoryString::BmpString(text) => text.to_string(),
    }
}

pub(crate) fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_upper_case_without_separator() {
        assert_eq!(hex_upper(&[0x0a, 0xff, 0x10]), "0AFF10");
    }

    #[test]
    fn garbage_is_rejected_with_a_message() {
        let error = CertificateInfo::from_der(b"nope").unwrap_err();
        assert!(error.to_string().contains("certificat illisible"));
    }
}
