use std::sync::{Mutex, OnceLock};

static CWD_SENSITIVE_FILESYSTEM_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[allow(
    clippy::redundant_pub_crate,
    reason = "This helper is intentionally crate-visible and not part of the public Unix API."
)]
pub(crate) fn cwd_sensitive_filesystem_lock() -> &'static Mutex<()> {
    CWD_SENSITIVE_FILESYSTEM_LOCK.get_or_init(|| Mutex::new(()))
}
