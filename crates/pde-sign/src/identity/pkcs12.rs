//! Import d'un certificat PKCS#12 (`.p12` / `.pfx`, S-02).
//!
//! Le fichier contient une clé privée chiffrée par mot de passe et un ou
//! plusieurs certificats. On garde la clé, le certificat qui lui correspond
//! et, à sa suite, les certificats de ses autorités trouvés dans le fichier.

use der::{Decode, Encode};
use p12_keystore::Pkcs12Archive;
use x509_cert::Certificate;

use super::Identity;
use crate::error::{Result, SignError};
use crate::key::PrivateKey;

impl Identity {
    /// Identité lue dans un fichier PKCS#12 protégé par `password`.
    pub fn from_pkcs12(data: &[u8], password: &str) -> Result<Identity> {
        let archive = Pkcs12Archive::from_pkcs12(data, password).map_err(|error| match error {
            p12_keystore::error::Error::MacError(_) => SignError::WrongPassword,
            other => SignError::Certificate(format!("fichier PKCS#12 illisible ({other})")),
        })?;
        let certificates = archive
            .certs
            .iter()
            .filter_map(|bag| Certificate::from_der(bag.cert.as_der()).ok())
            .collect::<Vec<_>>();
        // Le fichier peut contenir plusieurs clés : la première qui a son
        // certificat l'emporte.
        for bag in &archive.keys {
            let Ok(key) = PrivateKey::from_pkcs8_der(bag.key.as_der()) else {
                continue;
            };
            let public = key.public_key_der()?;
            let leaf = certificates.iter().find(|certificate| {
                certificate
                    .tbs_certificate()
                    .subject_public_key_info()
                    .to_der()
                    .is_ok_and(|der| der == public)
            });
            if let Some(leaf) = leaf {
                let chain = chain_from(leaf, &certificates);
                return Identity::new(key, chain);
            }
        }
        Err(SignError::Certificate(
            "le fichier ne contient pas de clé privée avec son certificat".into(),
        ))
    }
}

/// Le certificat puis ses émetteurs successifs présents dans le fichier.
fn chain_from(leaf: &Certificate, all: &[Certificate]) -> Vec<Certificate> {
    let mut chain = vec![leaf.clone()];
    while let Some(current) = chain.last() {
        let tbs = current.tbs_certificate();
        if tbs.issuer() == tbs.subject() {
            break;
        }
        let issuer = all.iter().find(|candidate| {
            candidate.tbs_certificate().subject() == tbs.issuer() && !chain.contains(candidate)
        });
        match issuer {
            Some(issuer) => chain.push(issuer.clone()),
            None => break,
        }
    }
    chain
}

#[cfg(test)]
mod tests {
    use p12_keystore::{KeyStore, KeyStoreEntry, PrivateKeyChain};

    use super::*;
    use crate::identity::Subject;

    const NOW: u64 = 1_795_003_200;

    /// PKCS#12 fabriqué pour le test (aucune clé réelle dans le dépôt).
    fn pkcs12_of(identity: &Identity, password: &str) -> Vec<u8> {
        let key =
            p12_keystore::PrivateKey::from_der(&identity.private_key_pkcs8().unwrap()).unwrap();
        let certs = identity
            .chain_der()
            .unwrap()
            .iter()
            .map(|der| p12_keystore::Certificate::from_der(der).unwrap())
            .collect::<Vec<_>>();
        let mut store = KeyStore::new();
        store.add_entry(
            "signature",
            KeyStoreEntry::PrivateKeyChain(PrivateKeyChain::new(b"id-1".to_vec(), key, certs)),
        );
        store.writer(password).write().unwrap()
    }

    fn identity() -> Identity {
        Identity::self_signed(
            &Subject {
                name: "Hugo Bernard".into(),
                ..Subject::default()
            },
            NOW,
        )
        .unwrap()
    }

    #[test]
    fn imports_key_and_certificate() {
        let original = identity();
        let data = pkcs12_of(&original, "secret de test");
        let imported = Identity::from_pkcs12(&data, "secret de test").unwrap();
        assert_eq!(imported.info().unwrap(), original.info().unwrap());
        assert_eq!(
            imported.key().public_key_der().unwrap(),
            original.key().public_key_der().unwrap()
        );
    }

    #[test]
    fn wrong_password_is_reported_as_such() {
        let data = pkcs12_of(&identity(), "bon");
        let error = Identity::from_pkcs12(&data, "mauvais").unwrap_err();
        assert!(matches!(error, SignError::WrongPassword), "{error:?}");
    }

    #[test]
    fn garbage_is_not_a_pkcs12() {
        let error = Identity::from_pkcs12(b"pas un p12", "x").unwrap_err();
        assert!(error.to_string().contains("PKCS#12"));
    }
}
