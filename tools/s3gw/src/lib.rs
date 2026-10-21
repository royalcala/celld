#![allow(
    clippy::wildcard_imports,
    clippy::missing_errors_doc, // TODO: docs
    clippy::let_underscore_untyped,
    clippy::module_name_repetitions,
    clippy::multiple_crate_versions, // TODO: check later
)]

#[macro_use]
mod error;

mod checksum;
mod fs;
mod s3;
mod utils;

pub use self::error::*;
pub use self::fs::FileSystem;

/// 1us: the CAS lock is ON by default. `US_S3GW_NO_CAS_LOCK=1` turns it off so the
/// concurrency test can show the failure it prevents. Diagnostic only.
pub(crate) fn cas_lock_enabled() -> bool {
    use std::sync::OnceLock;
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("US_S3GW_NO_CAS_LOCK").is_none())
}
