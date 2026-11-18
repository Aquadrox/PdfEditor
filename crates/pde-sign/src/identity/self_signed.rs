//! Certificat auto-signé généré par l'application (S-01).
//!
//! Clé ECDSA P-256 (rapide, signature courte, reconnue par Acrobat et
//! Poppler), validité de cinq ans, usages « signature numérique » et « non
//! répudiation » : ce qu'attend un lecteur PDF d'un certificat de signature.

use std::time::Duration;

use der::asn1::{Any, Ia5StringRef, OctetString, Utf8StringRef};
use der::{Decode, Encode};
use spki::{SubjectPublicKeyInfoOwned, SubjectPublicKeyInfoRef};
use x509_cert::attr::AttributeTypeAndValue;
use x509_cert::builder::profile::BuilderProfile;
use x509_cert::builder::{Builder, CertificateBuilder};
use x509_cert::certificate::TbsCertificate;
use x509_cert::ext::Extension;
use x509_cert::ext::pkix::{BasicConstraints, KeyUsage, KeyUsages};
use x509_cert::name::{Name, RdnSequence, RelativeDistinguishedName};
use x509_cert::serial_number::SerialNumber;
use x509_cert::time::{Time, Validity};

use super::Identity;
use crate::error::{Result, SignError};
use crate::key::PrivateKey;
use crate::oids;

/// Durée de validité d'un certificat généré : cinq ans.
const VALIDITY: Duration = Duration::from_secs(5 * 365 * 24 * 3600 + 24 * 3600);
/// Marge pour une horloge un peu en avance chez le lecteur.
const CLOCK_SKEW: Duration = Duration::from_secs(10 * 60);

/// Titulaire d'un certificat généré.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Subject {
    /// Nom affiché (« Claire Martin »), obligatoire.
    pub name: String,
    /// Adresse électronique, facultative.
    pub email: Option<String>,
    /// Organisation (« Atelier Morel SARL »), facultative.
    pub organization: Option<String>,
}

impl Identity {
    /// Nouvelle identité : clé P-256 et certificat auto-signé, valable cinq
    /// ans à partir de `now` (secondes depuis 1970, UTC).
    pub fn self_signed(subject: &Subject, now: u64) -> Result<Identity> {
        Identity::self_signed_with_key(subject, now, PrivateKey::generate_p256())
    }

    /// Même chose avec une clé fournie (RSA pour les lecteurs anciens, tests).
    pub fn self_signed_with_key(subject: &Subject, now: u64, key: PrivateKey) -> Result<Identity> {
        let name = subject_name(subject)?;
        let spki = SubjectPublicKeyInfoOwned::from_der(&key.public_key_der()?)?;
        let now = Duration::from_secs(now);
        let validity = Validity::new(time(now.saturating_sub(CLOCK_SKEW))?, time(now + VALIDITY)?);
        let serial = SerialNumber::generate(&mut crate::rng::system());
        let builder = CertificateBuilder::new(SelfSigned { name }, serial, validity, spki)
            .map_err(|e| SignError::Certificate(e.to_string()))?;
        let built = match &key {
            PrivateKey::P256(signing_key) => {
                builder.build::<_, p256::ecdsa::DerSignature>(signing_key)
            }
            PrivateKey::P384(signing_key) => {
                builder.build::<_, p384::ecdsa::DerSignature>(signing_key)
            }
            PrivateKey::Rsa(private_key) => {
                let signer =
                    rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key.as_ref().clone());
                builder.build::<_, rsa::pkcs1v15::Signature>(&signer)
            }
        };
        let certificate = built.map_err(|e| SignError::Certificate(e.to_string()))?;
        Identity::new(key, vec![certificate])
    }
}

fn time(since_epoch: Duration) -> Result<Time> {
    Ok(Time::GeneralTime(
        der::asn1::GeneralizedTime::from_unix_duration(since_epoch)?,
    ))
}

/// Nom X.509 du titulaire : CN, O, adresse électronique.
fn subject_name(subject: &Subject) -> Result<Name> {
    let name = subject.name.trim();
    if name.is_empty() {
        return Err(SignError::Certificate("le nom est obligatoire".into()));
    }
    let mut rdns = RdnSequence::default();
    push(
        &mut rdns,
        oids::COMMON_NAME,
        Any::encode_from(&Utf8StringRef::new(name)?)?,
    )?;
    if let Some(organization) = non_empty(&subject.organization) {
        push(
            &mut rdns,
            oids::ORGANIZATION,
            Any::encode_from(&Utf8StringRef::new(organization)?)?,
        )?;
    }
    if let Some(email) = non_empty(&subject.email) {
        let email = Ia5StringRef::new(email).map_err(|_| {
            SignError::Certificate(
                "l'adresse électronique contient des caractères non permis".into(),
            )
        })?;
        push(&mut rdns, oids::EMAIL_ADDRESS, Any::encode_from(&email)?)?;
    }
    Ok(Name::from_der(&rdns.to_der()?)?)
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

fn push(rdns: &mut RdnSequence, oid: const_oid::ObjectIdentifier, value: Any) -> Result<()> {
    let rdn = RelativeDistinguishedName::try_from(vec![AttributeTypeAndValue { oid, value }])?;
    rdns.push(rdn);
    Ok(())
}

/// Profil : émetteur = sujet, extensions d'un certificat de signature.
struct SelfSigned {
    name: Name,
}

impl BuilderProfile for SelfSigned {
    fn get_issuer(&self, _subject: &Name) -> Name {
        self.name.clone()
    }

    fn get_subject(&self) -> Name {
        self.name.clone()
    }

    fn build_extensions(
        &self,
        _spk: SubjectPublicKeyInfoRef<'_>,
        _issuer_spk: SubjectPublicKeyInfoRef<'_>,
        _tbs: &TbsCertificate,
    ) -> x509_cert::builder::Result<Vec<Extension>> {
        let usage = KeyUsage(KeyUsages::DigitalSignature | KeyUsages::NonRepudiation);
        let constraints = BasicConstraints {
            ca: false,
            path_len_constraint: None,
        };
        Ok(vec![
            extension(<KeyUsage as const_oid::AssociatedOid>::OID, true, &usage)?,
            extension(
                <BasicConstraints as const_oid::AssociatedOid>::OID,
                true,
                &constraints,
            )?,
        ])
    }
}

fn extension(
    extn_id: const_oid::ObjectIdentifier,
    critical: bool,
    value: &impl Encode,
) -> der::Result<Extension> {
    Ok(Extension {
        extn_id,
        critical,
        extn_value: OctetString::new(value.to_der()?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 18/11/2026 12:00 UTC.
    const NOW: u64 = 1_795_003_200;

    fn subject() -> Subject {
        Subject {
            name: "Claire Martin".into(),
            email: Some("claire@example.org".into()),
            organization: Some("Atelier Morel SARL".into()),
        }
    }

    #[test]
    fn generated_certificate_describes_its_holder() {
        let identity = Identity::self_signed(&subject(), NOW).unwrap();
        let info = identity.info().unwrap();
        assert_eq!(info.common_name, "Claire Martin");
        assert_eq!(info.organization.as_deref(), Some("Atelier Morel SARL"));
        assert_eq!(info.email.as_deref(), Some("claire@example.org"));
        assert_eq!(info.issuer_common_name, "Claire Martin");
        assert!(info.self_signed);
        assert!(info.valid_at(NOW));
        assert!(info.valid_at(NOW + 4 * 365 * 24 * 3600));
        assert!(!info.valid_at(NOW + 6 * 365 * 24 * 3600));
        assert_eq!(info.fingerprint.len(), 64);
    }

    #[test]
    fn accents_are_kept_in_the_name() {
        let mut holder = subject();
        holder.name = "Hélène Dupré-Çelik".into();
        holder.email = None;
        holder.organization = Some("  ".into());
        let info = Identity::self_signed(&holder, NOW).unwrap().info().unwrap();
        assert_eq!(info.common_name, "Hélène Dupré-Çelik");
        assert_eq!(info.organization, None);
        assert_eq!(info.email, None);
    }

    #[test]
    fn a_name_is_required() {
        let holder = Subject {
            name: "  ".into(),
            ..Subject::default()
        };
        assert!(Identity::self_signed(&holder, NOW).is_err());
    }

    #[test]
    fn non_ascii_email_is_refused() {
        let mut holder = subject();
        holder.email = Some("hélène@exemple.fr".into());
        let error = Identity::self_signed(&holder, NOW).unwrap_err();
        assert!(error.to_string().contains("adresse électronique"));
    }

    #[test]
    fn two_identities_never_share_key_or_serial() {
        let a = Identity::self_signed(&subject(), NOW).unwrap();
        let b = Identity::self_signed(&subject(), NOW).unwrap();
        assert_ne!(a.info().unwrap().fingerprint, b.info().unwrap().fingerprint);
        assert_ne!(
            a.key().public_key_der().unwrap(),
            b.key().public_key_der().unwrap()
        );
    }

    #[test]
    fn stored_form_reloads_the_same_identity() {
        let identity = Identity::self_signed(&subject(), NOW).unwrap();
        let key = identity.private_key_pkcs8().unwrap();
        let chain = identity.chain_der().unwrap();
        let back = Identity::from_parts(&key, &chain).unwrap();
        assert_eq!(back.info().unwrap(), identity.info().unwrap());
    }

    #[test]
    fn key_of_another_identity_is_refused() {
        let a = Identity::self_signed(&subject(), NOW).unwrap();
        let b = Identity::self_signed(&subject(), NOW).unwrap();
        let error = Identity::from_parts(&b.private_key_pkcs8().unwrap(), &a.chain_der().unwrap())
            .unwrap_err();
        assert!(error.to_string().contains("ne correspond pas"));
    }
}
