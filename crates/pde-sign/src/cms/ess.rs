//! Attribut `signingCertificateV2` (RFC 5035) : lie la signature au
//! certificat du signataire. Exigé par PAdES B-B : sans lui, un certificat
//! substitué (même clé, autre identité) passerait inaperçu.

use der::Sequence;
use der::asn1::OctetString;
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;
use x509_cert::ext::pkix::name::{GeneralName, GeneralNames};
use x509_cert::serial_number::SerialNumber;

use crate::error::Result;
use crate::key::DigestAlgorithm;

/// `SigningCertificateV2 ::= SEQUENCE { certs SEQUENCE OF ESSCertIDv2, policies … OPTIONAL }`
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub(crate) struct SigningCertificateV2 {
    pub certs: Vec<EssCertIdV2>,
}

/// `ESSCertIDv2 ::= SEQUENCE { hashAlgorithm DEFAULT sha256, certHash, issuerSerial OPTIONAL }`
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub(crate) struct EssCertIdV2 {
    #[asn1(optional = "true")]
    pub hash_algorithm: Option<AlgorithmIdentifierOwned>,
    pub cert_hash: OctetString,
    #[asn1(optional = "true")]
    pub issuer_serial: Option<IssuerSerial>,
}

/// `IssuerSerial ::= SEQUENCE { issuer GeneralNames, serialNumber }`
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub(crate) struct IssuerSerial {
    pub issuer: GeneralNames,
    pub serial_number: SerialNumber,
}

impl SigningCertificateV2 {
    /// Attribut pour ce certificat (empreinte SHA-256, valeur par défaut omise).
    pub fn for_certificate(certificate: &Certificate, der: &[u8]) -> Result<Self> {
        let tbs = certificate.tbs_certificate();
        Ok(SigningCertificateV2 {
            certs: vec![EssCertIdV2 {
                hash_algorithm: None,
                cert_hash: OctetString::new(DigestAlgorithm::Sha256.digest(&[der]))?,
                issuer_serial: Some(IssuerSerial {
                    issuer: vec![GeneralName::DirectoryName(tbs.issuer().clone())],
                    serial_number: tbs.serial_number().clone(),
                }),
            }],
        })
    }

    /// Le premier certificat désigné est-il bien celui-ci ? `None` si
    /// l'empreinte utilise un algorithme non pris en charge.
    pub fn designates(&self, der: &[u8]) -> Option<bool> {
        let first = self.certs.first()?;
        let algorithm = match &first.hash_algorithm {
            None => DigestAlgorithm::Sha256,
            Some(identifier) => DigestAlgorithm::from_oid(identifier.oid)?,
        };
        Some(algorithm.digest(&[der]) == first.cert_hash.as_bytes())
    }
}
