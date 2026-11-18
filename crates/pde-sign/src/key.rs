//! Clés privées de signature : ECDSA P-256, P-384 ou RSA.
//!
//! Comme une interface en C# (`AsymmetricAlgorithm`) : l'appelant signe sans
//! savoir quel algorithme est derrière ; l'énumération choisit l'algorithme
//! et son identifiant dans le CMS.

use der::asn1::Null;
use der::{Any, Encode};
use pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
use signature::{SignatureEncoding, Signer};
use spki::AlgorithmIdentifierOwned;
use zeroize::Zeroizing;

use crate::error::{Result, SignError};
use crate::oids;

/// Clé privée d'une identité.
#[derive(Clone)]
pub enum PrivateKey {
    /// ECDSA sur la courbe P-256, empreinte SHA-256 (clés générées par l'application).
    P256(p256::ecdsa::SigningKey),
    /// ECDSA sur la courbe P-384, empreinte SHA-384.
    P384(p384::ecdsa::SigningKey),
    /// RSA, PKCS#1 v1.5 avec SHA-256 (la plupart des certificats du commerce).
    Rsa(Box<rsa::PrivateKey>),
}

impl std::fmt::Debug for PrivateKey {
    // Jamais le contenu d'une clé dans un message ou un journal.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrivateKey({})", self.algorithm_name())
    }
}

/// Empreinte utilisée pour signer : celle de l'attribut `messageDigest` et
/// du contenu couvert par `/ByteRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl DigestAlgorithm {
    /// Empreinte des morceaux donnés, mis bout à bout.
    pub fn digest(self, parts: &[&[u8]]) -> Vec<u8> {
        use sha2::Digest;
        fn run<D: Digest>(parts: &[&[u8]]) -> Vec<u8> {
            let mut hasher = D::new();
            for part in parts {
                hasher.update(part);
            }
            hasher.finalize().to_vec()
        }
        match self {
            DigestAlgorithm::Sha256 => run::<sha2::Sha256>(parts),
            DigestAlgorithm::Sha384 => run::<sha2::Sha384>(parts),
            DigestAlgorithm::Sha512 => run::<sha2::Sha512>(parts),
        }
    }

    /// Identifiant de l'algorithme (paramètres absents, RFC 5754).
    pub fn identifier(self) -> AlgorithmIdentifierOwned {
        AlgorithmIdentifierOwned {
            oid: self.oid(),
            parameters: None,
        }
    }

    pub(crate) fn oid(self) -> const_oid::ObjectIdentifier {
        match self {
            DigestAlgorithm::Sha256 => oids::SHA256,
            DigestAlgorithm::Sha384 => oids::SHA384,
            DigestAlgorithm::Sha512 => oids::SHA512,
        }
    }

    /// Algorithme désigné par cet OID, s'il est pris en charge.
    pub fn from_oid(oid: const_oid::ObjectIdentifier) -> Option<Self> {
        match oid {
            o if o == oids::SHA256 => Some(DigestAlgorithm::Sha256),
            o if o == oids::SHA384 => Some(DigestAlgorithm::Sha384),
            o if o == oids::SHA512 => Some(DigestAlgorithm::Sha512),
            _ => None,
        }
    }
}

impl PrivateKey {
    /// Nouvelle clé P-256 tirée de l'aléa du système.
    pub fn generate_p256() -> Self {
        use p256::elliptic_curve::Generate;
        PrivateKey::P256(p256::ecdsa::SigningKey::generate_from_rng(
            &mut crate::rng::system(),
        ))
    }

    /// Lit une clé au format PKCS#8 (DER), tel qu'il sort d'un PKCS#12.
    pub fn from_pkcs8_der(der: &[u8]) -> Result<Self> {
        if let Ok(key) = p256::SecretKey::from_pkcs8_der(der) {
            return Ok(PrivateKey::P256(key.into()));
        }
        if let Ok(key) = p384::SecretKey::from_pkcs8_der(der) {
            return Ok(PrivateKey::P384(key.into()));
        }
        if let Ok(key) = rsa::PrivateKey::from_pkcs8_der(der) {
            return Ok(PrivateKey::Rsa(Box::new(key)));
        }
        Err(SignError::Certificate(
            "clé privée d'un type non pris en charge (RSA, ECDSA P-256 ou P-384 attendus)".into(),
        ))
    }

    /// La clé au format PKCS#8 (DER), effacée de la mémoire après usage.
    pub fn to_pkcs8_der(&self) -> Result<Zeroizing<Vec<u8>>> {
        let document = match self {
            PrivateKey::P256(key) => key.to_pkcs8_der(),
            PrivateKey::P384(key) => key.to_pkcs8_der(),
            PrivateKey::Rsa(key) => key.to_pkcs8_der(),
        }
        .map_err(|e| SignError::Encoding(e.to_string()))?;
        Ok(Zeroizing::new(document.as_bytes().to_vec()))
    }

    /// Clé publique (SubjectPublicKeyInfo, DER), pour la comparer au certificat.
    pub fn public_key_der(&self) -> Result<Vec<u8>> {
        let document = match self {
            PrivateKey::P256(key) => key.verifying_key().to_public_key_der(),
            PrivateKey::P384(key) => key.verifying_key().to_public_key_der(),
            PrivateKey::Rsa(key) => key.to_public_key().to_public_key_der(),
        }
        .map_err(|e| SignError::Encoding(e.to_string()))?;
        Ok(document.as_bytes().to_vec())
    }

    /// Empreinte associée à la clé.
    pub fn digest_algorithm(&self) -> DigestAlgorithm {
        match self {
            PrivateKey::P256(_) | PrivateKey::Rsa(_) => DigestAlgorithm::Sha256,
            PrivateKey::P384(_) => DigestAlgorithm::Sha384,
        }
    }

    /// Identifiant de l'algorithme de signature écrit dans le CMS.
    pub fn signature_algorithm(&self) -> Result<AlgorithmIdentifierOwned> {
        Ok(match self {
            PrivateKey::P256(_) => AlgorithmIdentifierOwned {
                oid: oids::ECDSA_WITH_SHA256,
                parameters: None,
            },
            PrivateKey::P384(_) => AlgorithmIdentifierOwned {
                oid: oids::ECDSA_WITH_SHA384,
                parameters: None,
            },
            // RFC 4055 : paramètres NULL explicites pour RSA.
            PrivateKey::Rsa(_) => AlgorithmIdentifierOwned {
                oid: oids::SHA256_WITH_RSA,
                parameters: Some(Any::encode_from(&Null)?),
            },
        })
    }

    /// Signe le message (l'empreinte est calculée par l'algorithme).
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        Ok(match self {
            PrivateKey::P256(key) => {
                let signature: p256::ecdsa::Signature = key.sign(message);
                signature.to_der().to_der()?
            }
            PrivateKey::P384(key) => {
                let signature: p384::ecdsa::Signature = key.sign(message);
                signature.to_der().to_der()?
            }
            PrivateKey::Rsa(key) => {
                let signer = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(key.as_ref().clone());
                let signature: rsa::pkcs1v15::Signature = signer.sign(message);
                signature.to_vec()
            }
        })
    }

    fn algorithm_name(&self) -> &'static str {
        match self {
            PrivateKey::P256(_) => "ECDSA P-256",
            PrivateKey::P384(_) => "ECDSA P-384",
            PrivateKey::Rsa(_) => "RSA",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p256_key_round_trips_through_pkcs8() {
        let key = PrivateKey::generate_p256();
        let der = key.to_pkcs8_der().unwrap();
        let back = PrivateKey::from_pkcs8_der(&der).unwrap();
        assert_eq!(
            key.public_key_der().unwrap(),
            back.public_key_der().unwrap()
        );
        assert_eq!(back.digest_algorithm(), DigestAlgorithm::Sha256);
    }

    #[test]
    fn debug_never_shows_key_material() {
        let key = PrivateKey::generate_p256();
        assert_eq!(format!("{key:?}"), "PrivateKey(ECDSA P-256)");
    }

    #[test]
    fn garbage_is_not_a_key() {
        assert!(PrivateKey::from_pkcs8_der(b"pas une cle").is_err());
    }

    #[test]
    fn digests_concatenate_parts() {
        let whole = DigestAlgorithm::Sha256.digest(&[b"abcdef"]);
        let parts = DigestAlgorithm::Sha256.digest(&[b"abc", b"def"]);
        assert_eq!(whole, parts);
        assert_eq!(DigestAlgorithm::Sha384.digest(&[b"x"]).len(), 48);
        assert_eq!(
            DigestAlgorithm::from_oid(oids::SHA512),
            Some(DigestAlgorithm::Sha512)
        );
    }
}
