//! Protection des données par le système (repris de VoiceToText) :
//! DPAPI sous Windows, clé gardée dans le trousseau de la session sous
//! Linux ; et le trousseau du système pour les secrets courts (clés API).
//! Ailleurs (Android avant la v1.1), indisponible.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use pde_core::secrets::{DataProtector, SecretStore};

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

/// Trousseau du système (Gestionnaire d'identifiants, Secret Service).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemSecrets;

impl SecretStore for SystemSecrets {
    fn load(&self, name: &str) -> Option<String> {
        #[cfg(windows)]
        return windows::load_secret(name);
        #[cfg(target_os = "linux")]
        return linux::load_secret(name);
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = name;
            None
        }
    }

    fn store(&self, name: &str, value: &str) -> Result<(), String> {
        #[cfg(windows)]
        return windows::store_secret(name, value);
        #[cfg(target_os = "linux")]
        return linux::store_secret(name, value);
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = (name, value);
            Err(UNAVAILABLE.into())
        }
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        #[cfg(windows)]
        return windows::delete_secret(name);
        #[cfg(target_os = "linux")]
        return linux::delete_secret(name);
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = name;
            Ok(())
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
const UNAVAILABLE: &str = "protection des données indisponible sur ce système";
