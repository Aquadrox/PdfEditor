//! DPAPI : Windows chiffre les données pour la session de l'utilisateur
//! (repris de VoiceToText, `secrets_windows.rs`).

#![allow(unsafe_code)]

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

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
