//! Vérification d'un CMS détaché : empreinte du contenu, signature des
//! attributs, certificat désigné.
//!
//! Lit aussi les signatures des autres logiciels : RSA (PKCS#1 v1.5) et
//! ECDSA P-256 / P-384, empreintes SHA-256 / 384 / 512, avec ou sans
//! attributs signés (`adbe.pkcs7.detached`).

use cms::cert::CertificateChoices;
use cms::content_info::ContentInfo;
use cms::signed_data::{SignedData, SignerIdentifier, SignerInfo};
use der::asn1::OctetString;
use der::{Encode, Reader, SliceReader};
use pkcs8::DecodePublicKey;
use signature::hazmat::PrehashVerifier;
use x509_cert::Certificate;
use x509_cert::ext::pkix::SubjectKeyIdentifier;
use x509_cert::time::Time;

use super::ess::SigningCertificateV2;
use crate::key::DigestAlgorithm;
use crate::oids;

/// Signature cryptographiquement correcte.
#[derive(Debug, Clone)]
pub(crate) struct CmsVerified {
    /// Certificat du signataire (DER).
    pub certificate: Vec<u8>,
    /// Heure de signature déclarée dans le CMS, si présente.
    pub signing_time: Option<u64>,
}

/// Pourquoi la signature n'est pas valable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CmsFailure {
    /// Contenu illisible.
    Malformed(String),
    /// Algorithme ou format que l'application ne sait pas vérifier.
    Unsupported(String),
    /// Les octets signés ont changé.
    DigestMismatch,
    /// La signature ne correspond pas à la clé du certificat.
    BadSignature,
    /// Le certificat joint n'est pas celui désigné par la signature.
    CertificateMismatch,
}

type Outcome<T> = std::result::Result<T, CmsFailure>;

/// Vérifie le CMS `der` (zéros de remplissage tolérés à la fin) pour le
/// contenu formé des morceaux `content`.
pub(crate) fn verify_detached(der: &[u8], content: &[&[u8]]) -> Outcome<CmsVerified> {
    let signed_data = decode_signed_data(der)?;
    let signer = signed_data
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or_else(|| malformed("aucun signataire"))?;
    let certificates = certificates(&signed_data);
    let certificate = find_signer(signer, &certificates)
        .ok_or_else(|| malformed("certificat du signataire absent"))?;
    let certificate_der = certificate.to_der().map_err(malformed)?;

    let digest_algorithm = DigestAlgorithm::from_oid(signer.digest_alg.oid)
        .ok_or_else(|| unsupported(format!("empreinte {}", signer.digest_alg.oid)))?;
    let content_digest = digest_algorithm.digest(content);

    let mut signing_time = None;
    let signed_message = match &signer.signed_attrs {
        Some(attributes) => {
            let mut digest_found = false;
            for attribute in attributes.iter() {
                let Some(value) = attribute.values.iter().next() else {
                    continue;
                };
                if attribute.oid == oids::MESSAGE_DIGEST {
                    let declared = value.decode_as::<OctetString>().map_err(malformed)?;
                    if declared.as_bytes() != content_digest {
                        return Err(CmsFailure::DigestMismatch);
                    }
                    digest_found = true;
                } else if attribute.oid == oids::SIGNING_TIME {
                    signing_time = value
                        .decode_as::<Time>()
                        .ok()
                        .map(|time| time.to_unix_duration().as_secs());
                } else if attribute.oid == oids::SIGNING_CERTIFICATE_V2 {
                    let declared = value
                        .decode_as::<SigningCertificateV2>()
                        .map_err(malformed)?;
                    if declared.designates(&certificate_der) == Some(false) {
                        return Err(CmsFailure::CertificateMismatch);
                    }
                }
            }
            if !digest_found {
                return Err(malformed("empreinte du contenu absente"));
            }
            attributes.to_der().map_err(malformed)?
        }
        // Sans attributs signés, la signature porte directement sur le contenu.
        None => content.concat(),
    };

    let hash_algorithm = signature_hash(signer, digest_algorithm)?;
    let prehash = hash_algorithm.digest(&[&signed_message]);
    check_signature(
        &certificate,
        signer.signature_algorithm.oid,
        hash_algorithm,
        &prehash,
        signer.signature.as_bytes(),
    )?;
    Ok(CmsVerified {
        certificate: certificate_der,
        signing_time,
    })
}

fn decode_signed_data(der: &[u8]) -> Outcome<SignedData> {
    // `/Contents` est complété par des zéros : on ne lit que le premier élément.
    let mut reader = SliceReader::new(der).map_err(malformed)?;
    let content_info: ContentInfo = reader.decode().map_err(malformed)?;
    if content_info.content_type != oids::SIGNED_DATA {
        return Err(malformed("pas une signature CMS"));
    }
    content_info
        .content
        .decode_as::<SignedData>()
        .map_err(malformed)
}

fn certificates(signed_data: &SignedData) -> Vec<Certificate> {
    signed_data
        .certificates
        .iter()
        .flat_map(|set| set.0.iter())
        .filter_map(|choice| match choice {
            CertificateChoices::Certificate(certificate) => Some(certificate.clone()),
            CertificateChoices::Other(_) => None,
        })
        .collect()
}

fn find_signer(signer: &SignerInfo, certificates: &[Certificate]) -> Option<Certificate> {
    certificates
        .iter()
        .find(|certificate| {
            let tbs = certificate.tbs_certificate();
            match &signer.sid {
                SignerIdentifier::IssuerAndSerialNumber(id) => {
                    tbs.issuer() == &id.issuer && tbs.serial_number() == &id.serial_number
                }
                SignerIdentifier::SubjectKeyIdentifier(id) => matches!(
                    tbs.get_extension::<SubjectKeyIdentifier>(),
                    Ok(Some((_, ref ski))) if ski == id
                ),
            }
        })
        .cloned()
}

/// Empreinte utilisée par l'algorithme de signature : celle qu'il nomme,
/// sinon celle du signataire (`rsaEncryption`, `ecPublicKey`).
fn signature_hash(signer: &SignerInfo, digest: DigestAlgorithm) -> Outcome<DigestAlgorithm> {
    let oid = signer.signature_algorithm.oid;
    Ok(match oid {
        o if o == oids::SHA256_WITH_RSA || o == oids::ECDSA_WITH_SHA256 => DigestAlgorithm::Sha256,
        o if o == oids::SHA384_WITH_RSA || o == oids::ECDSA_WITH_SHA384 => DigestAlgorithm::Sha384,
        o if o == oids::SHA512_WITH_RSA || o == oids::ECDSA_WITH_SHA512 => DigestAlgorithm::Sha512,
        o if o == oids::RSA_ENCRYPTION || o == oids::EC_PUBLIC_KEY => digest,
        other => return Err(unsupported(format!("algorithme de signature {other}"))),
    })
}

fn check_signature(
    certificate: &Certificate,
    algorithm: const_oid::ObjectIdentifier,
    hash: DigestAlgorithm,
    prehash: &[u8],
    signature: &[u8],
) -> Outcome<()> {
    let spki = certificate.tbs_certificate().subject_public_key_info();
    let spki_der = spki.to_der().map_err(malformed)?;
    let is_rsa = [
        oids::RSA_ENCRYPTION,
        oids::SHA256_WITH_RSA,
        oids::SHA384_WITH_RSA,
        oids::SHA512_WITH_RSA,
    ]
    .contains(&algorithm);
    let verified = if is_rsa {
        let key = rsa::PublicKey::from_public_key_der(&spki_der)
            .map_err(|_| unsupported("clé RSA attendue"))?;
        let signature =
            rsa::pkcs1v15::Signature::try_from(signature).map_err(|_| CmsFailure::BadSignature)?;
        match hash {
            DigestAlgorithm::Sha256 => rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(key)
                .verify_prehash(prehash, &signature),
            DigestAlgorithm::Sha384 => rsa::pkcs1v15::VerifyingKey::<sha2::Sha384>::new(key)
                .verify_prehash(prehash, &signature),
            DigestAlgorithm::Sha512 => rsa::pkcs1v15::VerifyingKey::<sha2::Sha512>::new(key)
                .verify_prehash(prehash, &signature),
        }
    } else {
        let curve = spki
            .algorithm
            .parameters
            .as_ref()
            .and_then(|parameters| parameters.decode_as::<const_oid::ObjectIdentifier>().ok());
        match curve {
            Some(oid) if oid == oids::SECP256R1 => {
                let key = p256::ecdsa::VerifyingKey::from_public_key_der(&spki_der)
                    .map_err(|_| malformed("clé P-256 illisible"))?;
                let signature = p256::ecdsa::DerSignature::try_from(signature)
                    .map_err(|_| CmsFailure::BadSignature)?;
                key.verify_prehash(prehash, &signature)
            }
            Some(oid) if oid == oids::SECP384R1 => {
                let key = p384::ecdsa::VerifyingKey::from_public_key_der(&spki_der)
                    .map_err(|_| malformed("clé P-384 illisible"))?;
                let signature = p384::ecdsa::DerSignature::try_from(signature)
                    .map_err(|_| CmsFailure::BadSignature)?;
                key.verify_prehash(prehash, &signature)
            }
            _ => return Err(unsupported("courbe elliptique")),
        }
    };
    verified.map_err(|_| CmsFailure::BadSignature)
}

fn malformed(error: impl std::fmt::Display) -> CmsFailure {
    CmsFailure::Malformed(error.to_string())
}

fn unsupported(what: impl std::fmt::Display) -> CmsFailure {
    CmsFailure::Unsupported(what.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::sign_detached;
    use super::*;
    use crate::identity::{Identity, Subject};
    use crate::key::PrivateKey;

    const NOW: u64 = 1_795_003_200;

    fn identity() -> Identity {
        Identity::self_signed(
            &Subject {
                name: "Test".into(),
                ..Subject::default()
            },
            NOW,
        )
        .unwrap()
    }

    fn signed(identity: &Identity, content: &[u8]) -> Vec<u8> {
        let digest = identity.key().digest_algorithm().digest(&[content]);
        sign_detached(identity, &digest).unwrap()
    }

    #[test]
    fn own_signature_verifies_even_with_zero_padding() {
        let identity = identity();
        let mut cms = signed(&identity, b"contenu du PDF");
        cms.extend_from_slice(&[0; 300]);
        let verified = verify_detached(&cms, &[b"contenu ", b"du PDF"]).unwrap();
        assert_eq!(verified.certificate, identity.chain_der().unwrap()[0]);
        assert_eq!(verified.signing_time, None);
    }

    #[test]
    fn changed_content_is_detected() {
        let cms = signed(&identity(), b"contenu du PDF");
        assert_eq!(
            verify_detached(&cms, &[b"contenu du PDf"]).unwrap_err(),
            CmsFailure::DigestMismatch
        );
    }

    #[test]
    fn altered_signature_is_detected() {
        let identity = identity();
        let mut cms = signed(&identity, b"x");
        // L'octet de signature le plus à la fin : dernier élément du CMS.
        let last = cms.len() - 3;
        cms[last] ^= 0x01;
        assert!(verify_detached(&cms, &[b"x"]).is_err());
    }

    #[test]
    fn rsa_signature_verifies() {
        // Clé courte : le test reste rapide en mode debug (jamais en production).
        let key = rsa::PrivateKey::new(&mut crate::rng::system(), 1024).unwrap();
        let identity = Identity::self_signed_with_key(
            &Subject {
                name: "RSA".into(),
                ..Subject::default()
            },
            NOW,
            PrivateKey::Rsa(Box::new(key)),
        )
        .unwrap();
        let cms = signed(&identity, b"contenu");
        verify_detached(&cms, &[b"contenu"]).unwrap();
        assert_eq!(
            verify_detached(&cms, &[b"autre"]).unwrap_err(),
            CmsFailure::DigestMismatch
        );
    }

    #[test]
    fn p384_signature_verifies() {
        use p384::elliptic_curve::Generate;
        let key = p384::ecdsa::SigningKey::generate_from_rng(&mut crate::rng::system());
        let identity = Identity::self_signed_with_key(
            &Subject {
                name: "P384".into(),
                ..Subject::default()
            },
            NOW,
            PrivateKey::P384(key),
        )
        .unwrap();
        let cms = signed(&identity, b"contenu");
        verify_detached(&cms, &[b"contenu"]).unwrap();
    }

    #[test]
    fn garbage_is_malformed() {
        assert!(matches!(
            verify_detached(b"\x30\x03\x02\x01\x00", &[b"x"]),
            Err(CmsFailure::Malformed(_))
        ));
    }
}
