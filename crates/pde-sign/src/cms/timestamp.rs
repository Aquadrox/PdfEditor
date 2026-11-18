//! Horodatage RFC 3161 (S-08, PAdES B-T) : un serveur d'horodatage atteste
//! l'heure à laquelle la signature existait déjà. On lui envoie seulement
//! l'empreinte de la valeur de signature ; il renvoie un jeton (CMS signé
//! par lui) qui est ajouté au CMS de la signature, en attribut non signé.

use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::asn1::{Int, OctetString};
use der::{Decode, Encode};
use spki::AlgorithmIdentifier;
use x509_tsp::cmpv2::status::PkiStatus;
use x509_tsp::{MessageImprint, TimeStampReq, TimeStampResp, TspVersion, TstInfo};

use super::verify::verify_detached;
use crate::certificate::CertificateInfo;
use crate::error::{Result, SignError};
use crate::key::DigestAlgorithm;
use crate::oids;

/// Serveur d'horodatage : reçoit une requête DER (`application/timestamp-query`)
/// et rend la réponse DER (`application/timestamp-reply`). Le transport
/// (HTTP) est fourni par l'application ; les tests en simulent un.
pub trait TimestampAuthority {
    fn request(&self, query: &[u8]) -> std::result::Result<Vec<u8>, String>;
}

/// Jeton vérifié.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenCheck {
    /// Heure attestée, secondes depuis 1970.
    pub time: u64,
    /// Nom (CN) du serveur d'horodatage.
    pub authority: Option<String>,
    nonce: Option<Vec<u8>>,
}

/// Jeton d'horodatage (DER) pour la valeur de signature `signature`.
pub(crate) fn token_for(tsa: &dyn TimestampAuthority, signature: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = crate::rng::bytes::<8>()?;
    // Entier positif, sans zéro de tête.
    nonce[0] = (nonce[0] & 0x7f) | 0x01;
    let request = TimeStampReq {
        version: TspVersion::V1,
        message_imprint: MessageImprint {
            hash_algorithm: AlgorithmIdentifier {
                oid: oids::SHA256,
                parameters: None,
            },
            hashed_message: OctetString::new(DigestAlgorithm::Sha256.digest(&[signature]))?,
        },
        req_policy: None,
        nonce: Some(Int::new(&nonce)?),
        cert_req: true,
        extensions: None,
    };
    let reply = tsa
        .request(&request.to_der()?)
        .map_err(SignError::Timestamp)?;
    let response = TimeStampResp::from_der(&reply)
        .map_err(|_| SignError::Timestamp("réponse du serveur d'horodatage illisible".into()))?;
    if !matches!(
        response.status.status,
        PkiStatus::Accepted | PkiStatus::GrantedWithMods
    ) {
        return Err(SignError::Timestamp(format!(
            "le serveur d'horodatage a refusé la demande ({:?})",
            response.status.status
        )));
    }
    let token = response
        .time_stamp_token
        .ok_or_else(|| SignError::Timestamp("réponse sans jeton d'horodatage".into()))?
        .to_der()?;
    let check = check_token(&token, signature).map_err(SignError::Timestamp)?;
    if check.nonce.as_deref() != Some(nonce.as_slice()) {
        return Err(SignError::Timestamp(
            "le jeton d'horodatage ne répond pas à cette demande".into(),
        ));
    }
    Ok(token)
}

/// Vérifie un jeton : il est signé par son serveur et porte sur `signature`.
pub(crate) fn check_token(
    token: &[u8],
    signature: &[u8],
) -> std::result::Result<TokenCheck, String> {
    let malformed = |what: &str| format!("jeton d'horodatage illisible ({what})");
    let info = ContentInfo::from_der(token).map_err(|_| malformed("structure"))?;
    let signed = info
        .content
        .decode_as::<SignedData>()
        .map_err(|_| malformed("contenu signé"))?;
    if signed.encap_content_info.econtent_type != oids::TST_INFO {
        return Err(malformed("type de contenu"));
    }
    let content = signed
        .encap_content_info
        .econtent
        .as_ref()
        .ok_or_else(|| malformed("contenu absent"))?
        .decode_as::<OctetString>()
        .map_err(|_| malformed("contenu"))?;
    let tst = TstInfo::from_der(content.as_bytes()).map_err(|_| malformed("TSTInfo"))?;
    let algorithm = DigestAlgorithm::from_oid(tst.message_imprint.hash_algorithm.oid)
        .ok_or_else(|| malformed("empreinte non prise en charge"))?;
    if algorithm.digest(&[signature]) != tst.message_imprint.hashed_message.as_bytes() {
        return Err("le jeton d'horodatage porte sur une autre signature".into());
    }
    let verified = verify_detached(token, &[content.as_bytes()])
        .map_err(|e| format!("jeton d'horodatage invalide ({e:?})"))?;
    Ok(TokenCheck {
        time: tst.gen_time.to_unix_duration().as_secs(),
        authority: CertificateInfo::from_der(&verified.certificate)
            .ok()
            .map(|c| c.common_name),
        nonce: tst.nonce.map(|n| n.as_bytes().to_vec()),
    })
}

/// Serveur d'horodatage simulé, qui signe avec une identité locale : pour
/// les tests et les essais hors réseau (fonction `testing`).
#[cfg(any(test, feature = "testing"))]
pub struct LocalTimestampAuthority {
    identity: crate::identity::Identity,
    time: u64,
}

#[cfg(any(test, feature = "testing"))]
impl LocalTimestampAuthority {
    /// Serveur qui atteste l'heure `time` (secondes depuis 1970).
    pub fn new(identity: crate::identity::Identity, time: u64) -> Self {
        Self { identity, time }
    }
}

#[cfg(any(test, feature = "testing"))]
impl TimestampAuthority for LocalTimestampAuthority {
    fn request(&self, query: &[u8]) -> std::result::Result<Vec<u8>, String> {
        use x509_tsp::GeneralizedTimeNanos;
        use x509_tsp::cmpv2::status::PkiStatusInfo;
        let request = TimeStampReq::from_der(query).map_err(|e| e.to_string())?;
        let tst = TstInfo {
            version: TspVersion::V1,
            policy: const_oid::ObjectIdentifier::new_unwrap("1.3.6.1.4.1.99999.1"),
            message_imprint: request.message_imprint,
            serial_number: Int::new(&[0x01, 0x23]).map_err(|e| e.to_string())?,
            gen_time: GeneralizedTimeNanos::from_unix_duration(std::time::Duration::from_secs(
                self.time,
            ))
            .map_err(|e| e.to_string())?,
            accuracy: None,
            ordering: false,
            nonce: request.nonce,
            tsa: None,
            extensions: None,
        };
        let content = tst.to_der().map_err(|e| e.to_string())?;
        let token = super::build::sign_encapsulated(&self.identity, oids::TST_INFO, &content)
            .map_err(|e| e.to_string())?;
        TimeStampResp {
            status: PkiStatusInfo {
                status: PkiStatus::Accepted,
                status_string: None,
                fail_info: None,
            },
            time_stamp_token: Some(ContentInfo::from_der(&token).map_err(|e| e.to_string())?),
        }
        .to_der()
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{Identity, Subject};

    const NOW: u64 = 1_795_003_200;

    fn tsa() -> LocalTimestampAuthority {
        let identity = Identity::self_signed(
            &Subject {
                name: "Horodatage de test".into(),
                ..Subject::default()
            },
            NOW,
        )
        .unwrap();
        LocalTimestampAuthority::new(identity, NOW + 5)
    }

    #[test]
    fn token_attests_the_time_of_this_signature() {
        let token = token_for(&tsa(), b"valeur de signature").unwrap();
        let check = check_token(&token, b"valeur de signature").unwrap();
        assert_eq!(check.time, NOW + 5);
        assert_eq!(check.authority.as_deref(), Some("Horodatage de test"));
        let other = check_token(&token, b"autre signature").unwrap_err();
        assert!(other.contains("autre signature"), "{other}");
    }

    struct Refusing;
    impl TimestampAuthority for Refusing {
        fn request(&self, _: &[u8]) -> std::result::Result<Vec<u8>, String> {
            Err("serveur injoignable".into())
        }
    }

    struct Garbage;
    impl TimestampAuthority for Garbage {
        fn request(&self, _: &[u8]) -> std::result::Result<Vec<u8>, String> {
            Ok(b"<html>erreur 404</html>".to_vec())
        }
    }

    #[test]
    fn transport_errors_and_garbage_are_reported() {
        let error = token_for(&Refusing, b"s").unwrap_err();
        assert!(error.to_string().contains("serveur injoignable"), "{error}");
        let error = token_for(&Garbage, b"s").unwrap_err();
        assert!(error.to_string().contains("illisible"), "{error}");
    }
}
