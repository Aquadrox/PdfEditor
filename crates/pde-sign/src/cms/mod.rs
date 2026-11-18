//! CMS détaché (RFC 5652) au format PAdES : ce qui est écrit dans
//! `/Contents` du dictionnaire de signature.

mod build;
mod ess;
mod timestamp;
mod verify;

pub(crate) use build::sign_detached;
#[cfg(feature = "testing")]
pub use timestamp::LocalTimestampAuthority;
pub use timestamp::TimestampAuthority;
pub(crate) use timestamp::TokenCheck;
pub(crate) use verify::{CmsFailure, verify_detached};
