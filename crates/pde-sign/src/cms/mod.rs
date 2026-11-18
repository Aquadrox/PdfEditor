//! CMS détaché (RFC 5652) au format PAdES : ce qui est écrit dans
//! `/Contents` du dictionnaire de signature.

mod build;
mod ess;
mod verify;

pub(crate) use build::sign_detached;
pub(crate) use verify::{CmsFailure, verify_detached};
