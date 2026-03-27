use std::sync::{Mutex, OnceLock};

static CWD_SENSITIVE_FILESYSTEM_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn cwd_sensitive_filesystem_lock() -> &'static Mutex<()> {
    CWD_SENSITIVE_FILESYSTEM_LOCK.get_or_init(|| Mutex::new(()))
}
