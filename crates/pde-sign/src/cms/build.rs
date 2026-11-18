//! Construction du CMS détaché signé (PAdES B-B).

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, EncapsulatedContentInfo, SignedData, SignerIdentifier, SignerInfo, SignerInfos,
};
use der::Encode;
use der::asn1::{Any, OctetString, SetOfVec};
use x509_cert::attr::Attribute;

use super::ess::SigningCertificateV2;
use crate::error::Result;
use crate::identity::Identity;
use crate::oids;

/// CMS `SignedData` détaché pour le contenu dont l'empreinte est `digest`
/// (calculée avec [`crate::PrivateKey::digest_algorithm`] de la clé).
///
/// Attributs signés : type de contenu, empreinte, certificat du signataire.
/// Pas d'heure de signature dans le CMS : PAdES la met dans `/M`.
pub(crate) fn sign_detached(identity: &Identity, digest: &[u8]) -> Result<Vec<u8>> {
    let key = identity.key();
    let certificate = identity.certificate();
    let certificate_der = certificate.to_der()?;

    let mut attributes = SetOfVec::new();
    attributes.insert(attribute(
        oids::CONTENT_TYPE,
        Any::encode_from(&oids::DATA)?,
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
        unsigned_attrs: None,
    };

    let mut digest_algorithms = SetOfVec::new();
    digest_algorithms.insert(key.digest_algorithm().identifier())?;
    let mut certificates = SetOfVec::new();
    for certificate in identity.chain() {
        certificates.insert(CertificateChoices::Certificate(certificate.clone()))?;
    }
    let mut signers = SetOfVec::new();
    signers.insert(signer)?;
    let signed_data = SignedData {
        version: CmsVersion::V1,
        digest_algorithms,
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: oids::DATA,
            econtent: None,
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

fn attribute(oid: const_oid::ObjectIdentifier, value: Any) -> Result<Attribute> {
    let mut values = SetOfVec::new();
    values.insert(value)?;
    Ok(Attribute { oid, values })
}
