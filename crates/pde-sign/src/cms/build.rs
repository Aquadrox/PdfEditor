//! Construction des CMS `SignedData` : signature détachée PAdES (contenu
//! hors du CMS : les octets du PDF) et, pour les jetons d'horodatage,
//! contenu encapsulé.

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, EncapsulatedContentInfo, SignedData, SignerIdentifier, SignerInfo, SignerInfos,
};
use const_oid::ObjectIdentifier;
use der::asn1::{Any, OctetString, SetOfVec};
use der::{Decode, Encode};
use x509_cert::attr::Attribute;

use super::ess::SigningCertificateV2;
use super::timestamp::{self, TimestampAuthority};
use crate::error::Result;
use crate::identity::Identity;
use crate::oids;

/// CMS `SignedData` détaché pour le contenu dont l'empreinte est `digest`
/// (calculée avec [`crate::PrivateKey::digest_algorithm`] de la clé).
///
/// Attributs signés : type de contenu, empreinte, certificat du signataire.
/// Pas d'heure de signature dans le CMS : PAdES la met dans `/M`. Avec un
/// serveur d'horodatage, son jeton portant sur la signature est ajouté en
/// attribut non signé (PAdES B-T).
pub(crate) fn sign_detached(
    identity: &Identity,
    digest: &[u8],
    tsa: Option<&dyn TimestampAuthority>,
) -> Result<Vec<u8>> {
    build(identity, oids::DATA, None, digest, tsa)
}

/// CMS dont le contenu `content` (de type `content_type`) est encapsulé
/// (jetons du serveur d'horodatage simulé).
#[cfg(any(test, feature = "testing"))]
pub(crate) fn sign_encapsulated(
    identity: &Identity,
    content_type: ObjectIdentifier,
    content: &[u8],
) -> Result<Vec<u8>> {
    let digest = identity.key().digest_algorithm().digest(&[content]);
    build(identity, content_type, Some(content), &digest, None)
}

fn build(
    identity: &Identity,
    content_type: ObjectIdentifier,
    content: Option<&[u8]>,
    digest: &[u8],
    tsa: Option<&dyn TimestampAuthority>,
) -> Result<Vec<u8>> {
    let key = identity.key();
    let certificate = identity.certificate();
    let certificate_der = certificate.to_der()?;

    let mut attributes = SetOfVec::new();
    attributes.insert(attribute(
        oids::CONTENT_TYPE,
        Any::encode_from(&content_type)?,
    )?)?;
    attributes.insert(attribute(
        oids::MESSAGE_DIGEST,
        Any::encode_from(&OctetString::new(digest)?)?,
    )?)?;
    attributes.insert(attribute(
        oids::SIGNING_CERTIFICATE_V2,
        Any::encode_from(&SigningCertificateV2::for_certificate(
            certificate,
            &certificate_der,
        )?)?,
    )?)?;
    // RFC 5652 § 5.4 : on signe l'encodage DER du SET OF (étiquette 0x31).
    let signature = key.sign(&attributes.to_der()?)?;
    let unsigned_attrs = match tsa {
        Some(tsa) => {
            let token = timestamp::token_for(tsa, &signature)?;
            let mut set = SetOfVec::new();
            set.insert(attribute(oids::TIMESTAMP_TOKEN, Any::from_der(&token)?)?)?;
            Some(set)
        }
        None => None,
    };

    let tbs = certificate.tbs_certificate();
    let signer = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: tbs.issuer().clone(),
            serial_number: tbs.serial_number().clone(),
        }),
        digest_alg: key.digest_algorithm().identifier(),
        signed_attrs: Some(attributes),
        signature_algorithm: key.signature_algorithm()?,
        signature: OctetString::new(signature)?,
        unsigned_attrs,
    };

    let mut digest_algorithms = SetOfVec::new();
    digest_algorithms.insert(key.digest_algorithm().identifier())?;
    let mut certificates = SetOfVec::new();
    for certificate in identity.chain() {
        certificates.insert(CertificateChoices::Certificate(certificate.clone()))?;
    }
    let mut signers = SetOfVec::new();
    signers.insert(signer)?;
    let econtent = match content {
        Some(bytes) => Some(Any::encode_from(&OctetString::new(bytes)?)?),
        None => None,
    };
    let signed_data = SignedData {
        // RFC 5652 § 5.1 : version 3 quand le contenu n'est pas « data ».
        version: if content_type == oids::DATA {
            CmsVersion::V1
        } else {
            CmsVersion::V3
        },
        digest_algorithms,
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: content_type,
            econtent,
        },
        certificates: Some(CertificateSet(certificates)),
        crls: None,
        signer_infos: SignerInfos(signers),
    };
    let content = ContentInfo {
        content_type: oids::SIGNED_DATA,
        content: Any::encode_from(&signed_data)?,
    };
    Ok(content.to_der()?)
}

fn attribute(oid: ObjectIdentifier, value: Any) -> Result<Attribute> {
    let mut values = SetOfVec::new();
    values.insert(value)?;
    Ok(Attribute { oid, values })
}
