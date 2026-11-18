//! DPAPI : Windows chiffre les données pour la session de l'utilisateur
//! (repris de VoiceToText, `secrets_windows.rs`).

#![allow(unsafe_code)]

use windows::Win32::Foundation::{FILETIME, HLOCAL, LocalFree};
use windows::Win32::Security::Credentials::{
    CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
    CredReadW, CredWriteW,
};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::{HSTRING, PWSTR};

/// Préfixe visible dans le Gestionnaire d'identifiants.
const TARGET_PREFIX: &str = "PdfEditor/";

fn target(name: &str) -> HSTRING {
    HSTRING::from(format!("{TARGET_PREFIX}{name}"))
}

/// Chaîne UTF-16 terminée par un zéro, modifiable (CREDENTIALW attend des PWSTR).
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Secret rangé dans le Gestionnaire d'identifiants, chiffré par Windows
/// pour la session de l'utilisateur.
pub fn store_secret(name: &str, value: &str) -> Result<(), String> {
    let mut user = wide("PdfEditor");
    let mut target_buf = wide(&format!("{TARGET_PREFIX}{name}"));
    let mut blob = value.as_bytes().to_vec();
    let credential = CREDENTIALW {
        Flags: CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target_buf.as_mut_ptr()),
        Comment: PWSTR::null(),
        LastWritten: FILETIME::default(),
        CredentialBlobSize: u32::try_from(blob.len()).map_err(|_| "secret trop long".to_owned())?,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: PWSTR::null(),
        UserName: PWSTR(user.as_mut_ptr()),
    };
    // SAFETY: tous les pointeurs de `credential` désignent des tampons
    // vivants pendant l'appel ; Windows les copie.
    unsafe { CredWriteW(&credential, 0) }
        .map_err(|e| format!("enregistrement dans le Gestionnaire d'identifiants : {e}"))
}

pub fn load_secret(name: &str) -> Option<String> {
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `credential` reçoit une structure allouée par Windows, libérée
    // par CredFree ci-dessous.
    unsafe { CredReadW(&target(name), CRED_TYPE_GENERIC, None, &mut credential) }.ok()?;
    // SAFETY: CredReadW a réussi : `credential` pointe vers une structure
    // valide jusqu'à CredFree, et son bloc fait `CredentialBlobSize` octets.
    unsafe {
        let c = &*credential;
        let size = c.CredentialBlobSize as usize;
        let bytes = std::slice::from_raw_parts(c.CredentialBlob, size);
        let value = String::from_utf8(bytes.to_vec()).ok();
        CredFree(credential as *const _);
        value
    }
}

pub fn delete_secret(name: &str) -> Result<(), String> {
    // SAFETY: simple appel avec un nom valide pendant l'appel.
    match unsafe { CredDeleteW(&target(name), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(()),
        // Déjà absent : rien à faire.
        Err(_) if load_secret(name).is_none() => Ok(()),
        Err(e) => Err(format!(
            "suppression dans le Gestionnaire d'identifiants : {e}"
        )),
    }
}

/// Copie le bloc renvoyé par DPAPI puis le libère.
///
/// SAFETY: `blob` vient de CryptProtectData / CryptUnprotectData réussis.
unsafe fn take_blob(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    // SAFETY: DPAPI a alloué `cbData` octets à `pbData` (LocalAlloc) ; on les
    // copie avant de les rendre au système.
    unsafe {
        let data = std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec();
        LocalFree(Some(HLOCAL(blob.pbData.cast())));
        data
    }
}

/// Chiffre `data` pour la session Windows de l'utilisateur : seul ce
/// compte, sur ce PC, peut le relire.
pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(|_| "bloc trop grand".to_owned())?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: entrée valide pendant l'appel (DPAPI ne l'écrit pas), sortie
    // libérée par take_blob.
    unsafe {
        CryptProtectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| format!("chiffrement par Windows impossible : {e}"))?;
        Ok(take_blob(output))
    }
}

/// Déchiffre un bloc produit par [`protect`] (même compte Windows).
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(|_| "bloc trop grand".to_owned())?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: comme pour protect.
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| format!("déchiffrement par Windows impossible : {e}"))?;
        Ok(take_blob(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_then_unprotect() {
        let sealed = protect(b"cle de test").unwrap();
        assert_ne!(sealed, b"cle de test");
        assert_eq!(unprotect(&sealed).unwrap(), b"cle de test");
        assert!(unprotect(b"abime").is_err());
    }
}
