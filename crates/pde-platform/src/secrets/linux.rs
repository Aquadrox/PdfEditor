//! Linux : une clé aléatoire de 256 bits, créée au premier usage et gardée
//! dans le trousseau de la session (KWallet, GNOME Keyring, via l'API
//! Secret Service de D-Bus), chiffre les données avec ChaCha20-Poly1305
//! (authentifié : un fichier modifié est refusé). Repris de VoiceToText,
//! `secrets_linux.rs`.

use std::collections::HashMap;
use std::sync::Mutex;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use secret_service::EncryptionType;
use secret_service::blocking::SecretService;

/// Attributs de la clé dans le trousseau.
const APPLICATION: &str = "pdfeditor";
const DATA_KEY: &str = "data-key";
/// Libellé visible dans KWallet ou « Mots de passe et clés ».
const LABEL: &str = "PdfEditor/data-key";
/// Format : `[1][nonce 12 octets][données + étiquette 16 octets]`.
const FORMAT_VERSION: u8 = 1;
const NONCE_LEN: usize = 12;

fn attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([("application", APPLICATION), ("name", DATA_KEY)])
}

fn service() -> Result<SecretService<'static>, String> {
    // Dh : le secret est chiffré pendant son passage sur le bus D-Bus.
    SecretService::connect(EncryptionType::Dh).map_err(|e| {
        format!(
            "trousseau de la session introuvable (KWallet ou GNOME Keyring doit être actif) : {e}"
        )
    })
}

fn load_key() -> Result<Option<[u8; 32]>, String> {
    let service = service()?;
    let found = service
        .search_items(attributes())
        .map_err(|e| format!("recherche dans le trousseau : {e}"))?;
    let Some(item) = found.unlocked.first().or(found.locked.first()) else {
        return Ok(None);
    };
    item.ensure_unlocked()
        .map_err(|e| format!("trousseau verrouillé : {e}"))?;
    let secret = item
        .get_secret()
        .map_err(|e| format!("lecture dans le trousseau : {e}"))?;
    <[u8; 32]>::try_from(secret.as_slice())
        .map(Some)
        .map_err(|_| "clé de chiffrement illisible dans le trousseau".to_owned())
}

fn store_key(key: &[u8; 32]) -> Result<(), String> {
    let service = service()?;
    let collection = service
        .get_any_collection()
        .map_err(|e| format!("trousseau sans portefeuille : {e}"))?;
    collection
        .ensure_unlocked()
        .map_err(|e| format!("trousseau verrouillé : {e}"))?;
    collection
        .create_item(LABEL, attributes(), key, true, "application/octet-stream")
        .map(|_| ())
        .map_err(|e| format!("enregistrement dans le trousseau : {e}"))
}

/// Clé gardée en mémoire après la première lecture.
static CACHED: Mutex<Option<[u8; 32]>> = Mutex::new(None);

fn data_key(create: bool) -> Result<[u8; 32], String> {
    let mut cached = CACHED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(key) = *cached {
        return Ok(key);
    }
    let key = match load_key()? {
        Some(key) => key,
        None if create => {
            let mut key = [0u8; 32];
            getrandom::fill(&mut key).map_err(|e| format!("aléa indisponible : {e}"))?;
            store_key(&key)?;
            key
        }
        None => return Err("clé de chiffrement absente du trousseau".into()),
    };
    *cached = Some(key);
    Ok(key)
}

pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    seal(&data_key(true)?, data)
}

pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    open(&data_key(false)?, data)
}

fn seal(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|e| format!("aléa indisponible : {e}"))?;
    let sealed = ChaCha20Poly1305::new(Key::from_slice(key))
        .encrypt(Nonce::from_slice(&nonce), data)
        .map_err(|_| "chiffrement impossible".to_owned())?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + sealed.len());
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

fn open(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    let (&version, rest) = data.split_first().ok_or("bloc chiffré vide")?;
    if version != FORMAT_VERSION || rest.len() < NONCE_LEN {
        return Err("bloc chiffré inconnu".into());
    }
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    ChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(Nonce::from_slice(nonce), sealed)
        .map_err(|_| "déchiffrement : clé différente ou données abîmées".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_then_open() {
        let key = [7u8; 32];
        let sealed = seal(&key, b"bonjour").unwrap();
        assert_eq!(sealed[0], FORMAT_VERSION);
        assert_eq!(sealed.len(), 1 + NONCE_LEN + 7 + 16);
        assert_eq!(open(&key, &sealed).unwrap(), b"bonjour");
        // Nonce aléatoire : deux chiffrements diffèrent.
        assert_ne!(seal(&key, b"bonjour").unwrap(), sealed);
    }

    #[test]
    fn open_rejects_wrong_key_or_tampering() {
        let sealed = seal(&[1u8; 32], b"secret").unwrap();
        assert!(open(&[2u8; 32], &sealed).is_err());
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(open(&[1u8; 32], &tampered).is_err());
        assert!(open(&[1u8; 32], &[]).is_err());
        assert!(open(&[1u8; 32], &[9, 0, 0]).is_err());
    }
}
