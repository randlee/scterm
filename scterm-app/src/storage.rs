//! Session storage helpers for logs and attached-state metadata.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Filesystem-backed helpers are not improved by const qualification."
)]

use anyhow::{Context, Result};
use scterm_core::{LogCap, SessionPath};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const OWNER_EXECUTE_BIT: u32 = 0o100;
const OWNER_ONLY_FILE_MODE: u32 = 0o600;
const SESSION_END_MARKER: &[u8] = b"\n[scterm session exited]\n";

/// A plaintext session log that replays history across reconnects.
#[derive(Debug, Clone)]
pub struct PersistentLog {
    path: PathBuf,
    cap: LogCap,
}

impl PersistentLog {
    /// Opens or creates a persistent log at `path`.
    ///
    /// # Errors
    /// Returns an error when the parent directory or log file cannot be created.
    pub fn open(path: impl Into<PathBuf>, cap: LogCap) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create log directory {}", parent.display()))?;
        }

        if !cap.is_disabled() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .read(true)
                .open(&path)
                .with_context(|| format!("open session log {}", path.display()))?;
            file.set_permissions(fs::Permissions::from_mode(OWNER_ONLY_FILE_MODE))
                .with_context(|| format!("set session log permissions for {}", path.display()))?;
        }

        Ok(Self { path, cap })
    }

    /// Appends bytes to the session log.
    ///
    /// # Errors
    /// Returns an error when the log cannot be written or capped.
    pub fn append(&self, bytes: &[u8]) -> Result<()> {
        if self.cap.is_disabled() {
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .with_context(|| format!("append session log {}", self.path.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("write session log {}", self.path.display()))?;
        self.enforce_cap()
    }

    /// Replays the current log contents.
    ///
    /// # Errors
    /// Returns an error when the log cannot be read.
    pub fn replay(&self) -> Result<Vec<u8>> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => {
                Err(error).with_context(|| format!("read session log {}", self.path.display()))
            }
        }
    }

    /// Clears the current log contents.
    ///
    /// # Errors
    /// Returns an error when the log cannot be truncated.
    pub fn clear(&self) -> Result<()> {
        match fs::write(&self.path, []) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("clear session log {}", self.path.display()))
            }
        }
    }

    /// Appends the session end marker.
    ///
    /// # Errors
    /// Returns an error when the marker cannot be written.
    pub fn append_end_marker(&self) -> Result<()> {
        self.append(SESSION_END_MARKER)
    }

    /// Returns the backing log path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn enforce_cap(&self) -> Result<()> {
        let max_bytes =
            usize::try_from(self.cap.bytes()).context("session log cap exceeds usize")?;
        let bytes = fs::read(&self.path)
            .with_context(|| format!("re-read session log {}", self.path.display()))?;

        if bytes.len() <= max_bytes {
            return Ok(());
        }

        fs::write(&self.path, &bytes[bytes.len() - max_bytes..])
            .with_context(|| format!("truncate capped session log {}", self.path.display()))
    }
}

/// Derives the on-disk log path for `session_path`.
#[must_use]
pub fn log_path_for_session(session_path: &SessionPath) -> PathBuf {
    let socket_path = session_path.as_path();
    let mut file_name = socket_path.file_name().map_or_else(
        || "session".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    file_name.push_str(".log");

    socket_path.parent().map_or_else(
        || PathBuf::from(file_name.clone()),
        |parent| parent.join(file_name.clone()),
    )
}

/// Returns whether the session socket is marked as attached.
///
/// # Errors
/// Returns an error when the socket metadata cannot be read.
pub fn attached_state(path: &Path) -> Result<bool> {
    let mode = fs::metadata(path)
        .with_context(|| format!("read session metadata {}", path.display()))?
        .permissions()
        .mode();
    Ok(mode & OWNER_EXECUTE_BIT != 0)
}

/// Sets the attached-state execute bit on the session socket metadata.
///
/// # Errors
/// Returns an error when the socket metadata cannot be updated.
pub fn set_attached_state(path: &Path, attached: bool) -> Result<()> {
    let mut permissions = fs::metadata(path)
        .with_context(|| format!("read session metadata {}", path.display()))?
        .permissions();
    let mode = permissions.mode();
    let next_mode = if attached {
        mode | OWNER_EXECUTE_BIT
    } else {
        mode & !OWNER_EXECUTE_BIT
    };
    permissions.set_mode(next_mode);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("update attached-state metadata for {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{attached_state, set_attached_state};
    use anyhow::Result;

    #[test]
    fn attached_state_metadata_tracks_the_owner_execute_bit() -> Result<()> {
        let tempdir = tempfile::TempDir::new()?;
        let path = tempdir.path().join("socket");
        std::os::unix::net::UnixListener::bind(&path)?;

        set_attached_state(&path, false)?;
        assert!(!attached_state(&path)?);

        set_attached_state(&path, true)?;
        assert!(attached_state(&path)?);
        Ok(())
    }
}
