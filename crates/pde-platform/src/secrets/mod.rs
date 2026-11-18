//! Protection des données par le système (repris de VoiceToText) :
//! DPAPI sous Windows, clé gardée dans le trousseau de la session sous
//! Linux. Ailleurs (Android avant la v1.1), indisponible.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use pde_core::secrets::DataProtector;

/// Protection fournie par le système d'exploitation.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProtector;

impl DataProtector for SystemProtector {
    fn protect(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        #[cfg(windows)]
        return windows::protect(data);
        #[cfg(target_os = "linux")]
        return linux::protect(data);
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = data;
            Err(UNAVAILABLE.into())
        }
    }

    fn unprotect(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        #[cfg(windows)]
        return windows::unprotect(data);
        #[cfg(target_os = "linux")]
        return linux::unprotect(data);
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = data;
            Err(UNAVAILABLE.into())
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
const UNAVAILABLE: &str = "protection des données indisponible sur ce système";
