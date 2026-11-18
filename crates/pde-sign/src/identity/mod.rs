//! Identités de signature : une clé privée et son certificat (S-01, S-02).

mod pkcs12;
mod self_signed;

use der::Decode;
use der::Encode;
use x509_cert::Certificate;
use zeroize::Zeroizing;

use crate::certificate::CertificateInfo;
use crate::error::{Result, SignError};
use crate::key::PrivateKey;

pub use self_signed::Subject;

/// Clé privée, certificat du signataire et éventuels certificats de son
/// autorité (chaîne), du titulaire vers la racine.
#[derive(Debug, Clone)]
pub struct Identity {
    key: PrivateKey,
    chain: Vec<Certificate>,
}

impl Identity {
    /// Assemble une identité ; la clé doit correspondre au premier certificat.
    pub fn new(key: PrivateKey, chain: Vec<Certificate>) -> Result<Self> {
        let Some(signer) = chain.first() else {
            return Err(SignError::Certificate("aucun certificat".into()));
        };
        let certified = signer
            .tbs_certificate()
            .subject_public_key_info()
            .to_der()?;
        if certified != key.public_key_der()? {
            return Err(SignError::Certificate(
                "la clé privée ne correspond pas au certificat".into(),
            ));
        }
        Ok(Identity { key, chain })
    }

    /// Identité rechargée depuis sa forme enregistrée : clé PKCS#8 et
    /// certificats DER (voir [`Identity::private_key_pkcs8`] et [`Identity::chain_der`]).
    pub fn from_parts(key_pkcs8: &[u8], chain_der: &[Vec<u8>]) -> Result<Self> {
        let key = PrivateKey::from_pkcs8_der(key_pkcs8)?;
        let chain = chain_der
            .iter()
            .map(|der| {
                Certificate::from_der(der)
                    .map_err(|e| SignError::Certificate(format!("certificat illisible ({e})")))
            })
            .collect::<Result<Vec<_>>>()?;
        Identity::new(key, chain)
    }

    /// Clé privée.
    pub fn key(&self) -> &PrivateKey {
        &self.key
    }

    /// Certificat du signataire.
    pub fn certificate(&self) -> &Certificate {
        // `new` garantit au moins un certificat.
        &self.chain[0]
    }

    /// Chaîne complète, signataire en tête.
    pub fn chain(&self) -> &[Certificate] {
        &self.chain
    }

    /// Chaîne en DER, pour l'enregistrer.
    pub fn chain_der(&self) -> Result<Vec<Vec<u8>>> {
        self.chain.iter().map(crate::certificate::to_der).collect()
    }

    /// Clé privée en PKCS#8, pour la confier au trousseau du système.
    pub fn private_key_pkcs8(&self) -> Result<Zeroizing<Vec<u8>>> {
        self.key.to_pkcs8_der()
    }

    /// Ce que l'interface montre du certificat du signataire.
    pub fn info(&self) -> Result<CertificateInfo> {
        let der = crate::certificate::to_der(self.certificate())?;
        Ok(CertificateInfo::from_certificate(self.certificate(), &der))
    }
}
