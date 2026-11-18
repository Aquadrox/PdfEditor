//! Identités de signature enregistrées (S-01, S-02) : un fichier JSON par
//! identité dans le dossier de données, avec ses certificats et sa clé
//! privée **chiffrée par le système** (DPAPI, trousseau) : jamais en clair
//! sur le disque.
//!
//! Un certificat PKCS#12 importé est traité de la même façon : son mot de
//! passe n'est demandé qu'à l'import, puis la clé est protégée par le
//! système comme celle d'un certificat généré.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use pde_core::secrets::DataProtector;
use pde_sign::{CertificateInfo, Identity, SignError, Subject};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;

const FORMAT_VERSION: u32 = 1;
/// Plus gros fichier PKCS#12 accepté.
const MAX_PKCS12_BYTES: u64 = 1024 * 1024;

/// Origine d'une identité.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Certificat généré par l'application.
    SelfSigned,
    /// Certificat importé d'un fichier `.p12` / `.pfx`.
    Pkcs12,
}

/// Ce que l'interface montre d'une identité (jamais la clé).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityView {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub organization: Option<String>,
    pub issuer: String,
    pub self_signed: bool,
    pub origin: Origin,
    /// Fin de validité, secondes depuis 1970.
    pub not_after: u64,
    pub fingerprint: String,
}

/// Fichier d'une identité.
#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    origin: Origin,
    /// Certificats DER en hexadécimal, signataire en tête.
    chain: Vec<String>,
    /// Clé PKCS#8 chiffrée par le système, en hexadécimal.
    key: String,
}

pub struct IdentityStore {
    dir: PathBuf,
    protector: Arc<dyn DataProtector>,
}

impl IdentityStore {
    /// Identités rangées dans `data_dir/identities`.
    pub fn new(data_dir: &Path, protector: Arc<dyn DataProtector>) -> Self {
        Self {
            dir: data_dir.join("identities"),
            protector,
        }
    }

    /// Identités enregistrées, par nom ; un fichier illisible est ignoré.
    pub fn list(&self) -> Vec<IdentityView> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut views: Vec<IdentityView> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "json"))
            .filter_map(|path| match self.read(&path) {
                Ok((stored, view)) if stored.version == FORMAT_VERSION => Some(view),
                Ok(_) => None,
                Err(e) => {
                    log::warn!("identité de signature illisible ignorée : {e}");
                    None
                }
            })
            .collect();
        views.sort_by_key(|v| v.name.to_lowercase());
        views
    }

    /// Nouvelle identité au certificat auto-signé.
    pub fn create(&self, subject: &Subject, now: u64) -> Result<IdentityView, AppError> {
        let identity = Identity::self_signed(subject, now).map_err(sign_error)?;
        self.store(&identity, Origin::SelfSigned)
    }

    /// Importe un fichier PKCS#12 ; déjà présent : l'identité existante.
    pub fn import(&self, path: &Path, password: &str) -> Result<IdentityView, AppError> {
        let size = std::fs::metadata(path).map_err(|e| io(path, e))?.len();
        if size > MAX_PKCS12_BYTES {
            return Err(AppError::new(
                ErrorKind::InvalidFile,
                "ce fichier est trop gros pour être un certificat",
            ));
        }
        let data = std::fs::read(path).map_err(|e| io(path, e))?;
        let identity = Identity::from_pkcs12(&data, password).map_err(sign_error)?;
        self.store(&identity, Origin::Pkcs12)
    }

    /// Supprime l'identité (sa clé ne peut plus servir).
    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let path = self.path(id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(&path, e)),
        }
    }

    /// Identité complète, clé déchiffrée par le système.
    pub fn load(&self, id: &str) -> Result<Identity, AppError> {
        let (stored, _) = self.read(&self.path(id)?)?;
        let protected = decode_hex(&stored.key).ok_or_else(corrupt)?;
        let key = zeroize::Zeroizing::new(self.protector.unprotect(&protected).map_err(|e| {
            AppError::new(
                ErrorKind::Signature,
                format!("clé privée inaccessible : {e}"),
            )
        })?);
        let chain = stored
            .chain
            .iter()
            .map(|hex| decode_hex(hex).ok_or_else(corrupt))
            .collect::<Result<Vec<_>, _>>()?;
        Identity::from_parts(&key, &chain).map_err(sign_error)
    }

    fn store(&self, identity: &Identity, origin: Origin) -> Result<IdentityView, AppError> {
        let info = identity.info().map_err(sign_error)?;
        let id = identifier(&info);
        let path = self.path(&id)?;
        if !path.exists() {
            let key = identity.private_key_pkcs8().map_err(sign_error)?;
            let protected = self.protector.protect(&key).map_err(|e| {
                AppError::new(
                    ErrorKind::Signature,
                    format!("la clé privée ne peut pas être protégée par le système : {e}"),
                )
            })?;
            let stored = Stored {
                version: FORMAT_VERSION,
                origin,
                chain: identity
                    .chain_der()
                    .map_err(sign_error)?
                    .iter()
                    .map(|der| encode_hex(der))
                    .collect(),
                key: encode_hex(&protected),
            };
            let json = serde_json::to_vec_pretty(&stored)
                .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))?;
            write_atomic(&path, &json)?;
            log::info!("identité de signature enregistrée ({origin:?})");
        }
        Ok(view(id, &info, origin))
    }

    fn read(&self, path: &Path) -> Result<(Stored, IdentityView), AppError> {
        let bytes = std::fs::read(path).map_err(|e| io(path, e))?;
        let stored: Stored = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
        let first = stored
            .chain
            .first()
            .and_then(|hex| decode_hex(hex))
            .ok_or_else(corrupt)?;
        let info = CertificateInfo::from_der(&first).map_err(sign_error)?;
        let view = view(identifier(&info), &info, stored.origin);
        Ok((stored, view))
    }

    /// Fichier de l'identité ; l'identifiant vient de l'interface : vérifié.
    fn path(&self, id: &str) -> Result<PathBuf, AppError> {
        if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppError::new(
                ErrorKind::InvalidRequest,
                "identité inconnue",
            ));
        }
        Ok(self.dir.join(format!("{id}.json")))
    }
}

/// Identifiant stable : début de l'empreinte du certificat.
fn identifier(info: &CertificateInfo) -> String {
    info.fingerprint
        .chars()
        .take(16)
        .collect::<String>()
        .to_lowercase()
}

fn view(id: String, info: &CertificateInfo, origin: Origin) -> IdentityView {
    IdentityView {
        id,
        name: info.common_name.clone(),
        email: info.email.clone(),
        organization: info.organization.clone(),
        issuer: info.issuer_common_name.clone(),
        self_signed: info.self_signed,
        origin,
        not_after: info.not_after,
        fingerprint: info.fingerprint.clone(),
    }
}

pub(crate) fn sign_error(error: SignError) -> AppError {
    let kind = match error {
        SignError::WrongPassword => ErrorKind::WrongPassword,
        _ => ErrorKind::Signature,
    };
    AppError::new(kind, error.to_string())
}

fn corrupt() -> AppError {
    AppError::new(ErrorKind::Signature, "fichier d'identité abîmé")
}

fn io(path: &Path, error: std::io::Error) -> AppError {
    let kind = if error.kind() == std::io::ErrorKind::NotFound {
        ErrorKind::FileNotFound
    } else {
        ErrorKind::Io
    };
    AppError::new(kind, format!("{} : {error}", path.display()))
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Faux système : « chiffre » en inversant les octets, pour vérifier
    /// que la clé n'est jamais écrite telle quelle.
    pub(crate) struct FakeProtector;

    impl DataProtector for FakeProtector {
        fn protect(&self, data: &[u8]) -> Result<Vec<u8>, String> {
            Ok(data.iter().map(|b| !b).collect())
        }
        fn unprotect(&self, data: &[u8]) -> Result<Vec<u8>, String> {
            Ok(data.iter().map(|b| !b).collect())
        }
    }

    struct Broken;

    impl DataProtector for Broken {
        fn protect(&self, _: &[u8]) -> Result<Vec<u8>, String> {
            Err("trousseau absent".into())
        }
        fn unprotect(&self, _: &[u8]) -> Result<Vec<u8>, String> {
            Err("trousseau absent".into())
        }
    }

    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pde-identities-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub(crate) fn subject(name: &str) -> Subject {
        Subject {
            name: name.into(),
            email: Some("claire@example.org".into()),
            organization: None,
        }
    }

    #[test]
    fn created_identity_is_listed_and_reloaded() {
        let dir = temp_dir("create");
        let store = IdentityStore::new(&dir, Arc::new(FakeProtector));
        let created = store
            .create(&subject("Claire Martin"), 1_795_003_200)
            .unwrap();
        assert_eq!(created.name, "Claire Martin");
        assert_eq!(created.origin, Origin::SelfSigned);
        assert!(created.self_signed);
        assert_eq!(store.list(), vec![created.clone()]);
        let identity = store.load(&created.id).unwrap();
        assert_eq!(identity.info().unwrap().common_name, "Claire Martin");

        // La clé n'est pas écrite en clair.
        let file =
            std::fs::read_to_string(dir.join("identities").join(format!("{}.json", created.id)))
                .unwrap();
        let clear = encode_hex(&identity.private_key_pkcs8().unwrap());
        assert!(!file.contains(&clear));

        store.delete(&created.id).unwrap();
        assert!(store.list().is_empty());
        assert_eq!(
            store.load(&created.id).unwrap_err().kind,
            ErrorKind::FileNotFound
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn identities_are_sorted_and_bad_files_ignored() {
        let dir = temp_dir("list");
        let store = IdentityStore::new(&dir, Arc::new(FakeProtector));
        store.create(&subject("zoé"), 1_795_003_200).unwrap();
        store.create(&subject("Adèle"), 1_795_003_200).unwrap();
        std::fs::write(dir.join("identities/abc.json"), "{pas du json").unwrap();
        let names: Vec<String> = store.list().into_iter().map(|v| v.name).collect();
        assert_eq!(names, ["Adèle", "zoé"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn system_without_protection_refuses_to_store_keys() {
        let dir = temp_dir("broken");
        let store = IdentityStore::new(&dir, Arc::new(Broken));
        let error = store.create(&subject("Claire"), 1_795_003_200).unwrap_err();
        assert!(error.message.contains("protégée par le système"), "{error}");
        assert!(store.list().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn identifiers_from_the_interface_are_checked() {
        let store = IdentityStore::new(Path::new("/nulle-part"), Arc::new(FakeProtector));
        for bad in ["", "../config", "abc/def", &"a".repeat(65)] {
            assert_eq!(
                store.load(bad).unwrap_err().kind,
                ErrorKind::InvalidRequest,
                "{bad}"
            );
        }
    }

    #[test]
    fn missing_pkcs12_file_is_reported() {
        let store = IdentityStore::new(Path::new("/nulle-part"), Arc::new(FakeProtector));
        let error = store
            .import(Path::new("/nulle-part/absent.p12"), "x")
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::FileNotFound);
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(
            decode_hex(&encode_hex(&[0, 255, 16])),
            Some(vec![0, 255, 16])
        );
        assert_eq!(decode_hex("abc"), None);
        assert_eq!(decode_hex("zz"), None);
    }
}
