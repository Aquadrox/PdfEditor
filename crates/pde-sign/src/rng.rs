//! Aléa du système (clés, numéros de série).

use getrandom::SysRng;
use getrandom::rand_core::UnwrapErr;

use crate::error::{Result, SignError};

/// Générateur cryptographique du système d'exploitation.
pub(crate) fn system() -> UnwrapErr<SysRng> {
    UnwrapErr(SysRng)
}

/// `N` octets aléatoires.
pub(crate) fn bytes<const N: usize>() -> Result<[u8; N]> {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).map_err(|_| SignError::Random)?;
    Ok(out)
}
