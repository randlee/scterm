//! ATM bridge adapter for `scterm`.
//!
//! This crate owns the subprocess-based ATM integration layer: blocking reads,
//! message parsing, sanitization, and per-session de-duplication.
#![allow(
    clippy::missing_const_for_fn,
    reason = "This adapter is runtime-oriented and not improved by const qualification."
)]

use scterm_core::SessionName;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Configuration for one ATM watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtmConfig {
    mailbox: SessionName,
    self_identity: Option<SessionName>,
    username: Option<String>,
    dedup_path: PathBuf,
    timeout_secs: u64,
}

impl AtmConfig {
    /// Creates a watcher configuration for `mailbox` with local dedup storage at `dedup_path`.
    #[must_use]
    pub fn new(mailbox: SessionName, dedup_path: PathBuf) -> Self {
        Self {
            mailbox,
            self_identity: None,
            username: None,
            dedup_path,
            timeout_secs: 600,
        }
    }

    /// Sets the identity used for self-message suppression.
    #[must_use]
    pub fn with_self_identity(mut self, self_identity: SessionName) -> Self {
        self.self_identity = Some(self_identity);
        self
    }

    /// Sets the ambient username accepted by the relevance filter.
    #[must_use]
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Sets the blocking read timeout in seconds.
    #[must_use]
    pub fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Returns the watched mailbox.
    #[must_use]
    pub fn mailbox(&self) -> &SessionName {
        &self.mailbox
    }

    /// Returns the local dedup persistence file path.
    #[must_use]
    pub fn dedup_path(&self) -> &Path {
        &self.dedup_path
    }
}

/// One normalized inbound ATM message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtmEvent {
    sender: String,
    text: String,
    message_id: String,
}

impl AtmEvent {
    /// Creates a normalized event.
    #[must_use]
    pub fn new(
        sender: impl Into<String>,
        text: impl Into<String>,
        message_id: impl Into<String>,
    ) -> Self {
        Self {
            sender: sender.into(),
            text: text.into(),
            message_id: message_id.into(),
        }
    }

    /// Returns the sender identity.
    #[must_use]
    pub fn sender(&self) -> &str {
        &self.sender
    }

    /// Returns the sanitized message text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the stable message identifier used for de-duplication.
    #[must_use]
    pub fn message_id(&self) -> &str {
        &self.message_id
    }

    /// Formats the deterministic PTY injection bytes for this message.
    #[must_use]
    pub fn injection_bytes(&self) -> Vec<u8> {
        format!("[ATM from {}]\n{}\r", self.sender, self.text).into_bytes()
    }
}

/// Typed errors produced by the ATM watcher adapter.
#[derive(Debug, thiserror::Error)]
pub enum AtmError {
    /// The `atm` CLI is not available in `PATH`.
    #[error("atm CLI unavailable: {0}")]
    Unavailable(#[source] std::io::Error),
    /// Launching the `atm` CLI failed before it returned a status.
    #[error("atm CLI I/O failure: {0}")]
    CliIo(#[source] std::io::Error),
    /// The `atm` CLI returned a non-timeout failure.
    #[error("atm CLI failed: {0}")]
    CliFailure(String),
    /// The CLI output could not be parsed as a valid ATM read response.
    #[error("atm CLI parse failure: {0}")]
    ParseFailure(#[source] serde_json::Error),
    /// Local de-duplication state could not be loaded or persisted.
    #[error("ATM dedup persistence failed: {0}")]
    DedupPersistence(#[source] std::io::Error),
}

/// Blocking subprocess watcher over `atm read --json --timeout ...`.
#[derive(Debug)]
pub struct AtmWatcher {
    config: AtmConfig,
    delivered: HashSet<String>,
}

impl AtmWatcher {
    /// Creates a watcher with persisted per-session dedup state.
    ///
    /// # Errors
    /// Returns [`AtmError::DedupPersistence`] when the existing dedup file cannot be read.
    pub fn new(config: AtmConfig) -> Result<Self, AtmError> {
        let delivered = load_dedup(&config.dedup_path)?;
        Ok(Self { config, delivered })
    }

    /// Performs one blocking ATM read and returns newly relevant events.
    ///
    /// # Errors
    /// Returns [`AtmError::Unavailable`] when `atm` is not installed,
    /// [`AtmError::CliFailure`] when the subprocess fails, and
    /// [`AtmError::ParseFailure`] when the JSON response cannot be decoded.
    pub fn poll_once(&mut self) -> Result<Vec<AtmEvent>, AtmError> {
        let output = Command::new("atm")
            .arg("read")
            .arg(self.config.mailbox.as_str())
            .arg("--json")
            .arg("--timeout")
            .arg(self.config.timeout_secs.to_string())
            .output()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    AtmError::Unavailable(error)
                } else {
                    AtmError::CliIo(error)
                }
            })?;

        if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
            return Ok(Vec::new());
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            return Err(AtmError::CliFailure(detail));
        }

        self.parse_output(&output.stdout)
    }

    fn parse_output(&mut self, bytes: &[u8]) -> Result<Vec<AtmEvent>, AtmError> {
        let response: RawReadResponse =
            serde_json::from_slice(bytes).map_err(AtmError::ParseFailure)?;
        let mut events = Vec::new();

        for mut message in response.messages {
            if !self.is_relevant(&message) {
                continue;
            }

            let message_id = message
                .message_id
                .clone()
                .unwrap_or_else(|| synthesize_message_id(&message));
            let Some(sender) = message.from.take().filter(|sender| !sender.is_empty()) else {
                continue;
            };
            let Some(text) = message.text.take() else {
                continue;
            };
            if self.delivered.contains(&message_id) {
                continue;
            }

            persist_dedup(&self.config.dedup_path, &message_id)?;
            self.delivered.insert(message_id.clone());
            events.push(AtmEvent::new(sender, sanitize_text(&text), message_id));
        }

        Ok(events)
    }

    fn is_relevant(&self, message: &RawMessage) -> bool {
        if let (Some(sender), Some(self_identity)) = (
            message.from.as_deref(),
            self.config.self_identity.as_ref().map(SessionName::as_str),
        ) {
            if sender == self_identity {
                return false;
            }
        }

        let target = message
            .to
            .as_deref()
            .unwrap_or(self.config.mailbox.as_str());
        target == self.config.mailbox.as_str()
            || self
                .config
                .self_identity
                .as_ref()
                .is_some_and(|identity| target == identity.as_str())
            || self
                .config
                .username
                .as_deref()
                .is_some_and(|username| target == username)
    }
}

#[derive(Debug, Deserialize)]
struct RawReadResponse {
    #[serde(default)]
    messages: Vec<RawMessage>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawMessage {
    from: Option<String>,
    text: Option<String>,
    timestamp: Option<String>,
    summary: Option<String>,
    to: Option<String>,
    message_id: Option<String>,
}

fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|ch| matches!(ch, '\n' | '\r' | '\t') || !ch.is_control())
        .collect()
}

fn synthesize_message_id(message: &RawMessage) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        message.timestamp.as_deref().unwrap_or_default(),
        message.from.as_deref().unwrap_or_default(),
        message.summary.as_deref().unwrap_or_default(),
        message.text.as_deref().unwrap_or_default()
    )
}

fn load_dedup(path: &Path) -> Result<HashSet<String>, AtmError> {
    if !path.exists() {
        return Ok(HashSet::new());
    }

    let file = fs::File::open(path).map_err(AtmError::DedupPersistence)?;
    let mut delivered = HashSet::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(AtmError::DedupPersistence)?;
        if !line.is_empty() {
            delivered.insert(line);
        }
    }
    Ok(delivered)
}

fn persist_dedup(path: &Path, message_id: &str) -> Result<(), AtmError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(AtmError::DedupPersistence)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(AtmError::DedupPersistence)?;
    writeln!(file, "{message_id}").map_err(AtmError::DedupPersistence)
}

#[cfg(test)]
mod tests {
    use super::{AtmConfig, AtmError, AtmWatcher};
    use scterm_core::SessionName;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static PATH_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn poll_once_reads_json_and_sanitizes_text() -> Result<(), Box<dyn std::error::Error>> {
        let _path_lock = PATH_LOCK.lock().expect("PATH lock");
        let tempdir = TempDir::new()?;
        let args_path = tempdir.path().join("args.txt");
        let script = format!(
            "#!/bin/sh\nprintf '%s ' \"$@\" > \"{}\"\ncat <<'JSON'\n{{\"messages\":[{{\"from\":\"arch-term\",\"to\":\"demo\",\"text\":\"line\\u0007\\nnext\",\"timestamp\":\"2026-03-26T00:00:00Z\"}}]}}\nJSON\n",
            args_path.display()
        );
        install_fake_atm(tempdir.path(), &script)?;
        let old_path = std::env::var_os("PATH");
        set_path_with_fake_atm(tempdir.path(), old_path.as_deref())?;

        let config = AtmConfig::new(SessionName::new("demo")?, tempdir.path().join("dedup.txt"))
            .with_self_identity(SessionName::new("demo")?)
            .with_timeout_secs(600);
        let mut watcher = AtmWatcher::new(config)?;
        let events = watcher.poll_once()?;

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sender(), "arch-term");
        assert_eq!(events[0].text(), "line\nnext");
        assert_eq!(
            String::from_utf8(events[0].injection_bytes())?,
            "[ATM from arch-term]\nline\nnext\r"
        );
        let args = fs::read_to_string(args_path)?;
        assert!(args.contains("read"));
        assert!(args.contains("demo"));
        assert!(args.contains("--json"));
        assert!(args.contains("--timeout"));
        assert!(args.contains("600"));

        restore_path(old_path);
        Ok(())
    }

    #[test]
    fn poll_once_deduplicates_across_restart() -> Result<(), Box<dyn std::error::Error>> {
        let _path_lock = PATH_LOCK.lock().expect("PATH lock");
        let tempdir = TempDir::new()?;
        install_fake_atm(
            tempdir.path(),
            "#!/bin/sh\ncat <<'JSON'\n{\"messages\":[{\"from\":\"arch-term\",\"to\":\"demo\",\"text\":\"hello\",\"timestamp\":\"2026-03-26T00:00:00Z\"}]}\nJSON\n",
        )?;
        let old_path = std::env::var_os("PATH");
        set_path_with_fake_atm(tempdir.path(), old_path.as_deref())?;

        let config = AtmConfig::new(SessionName::new("demo")?, tempdir.path().join("dedup.txt"));
        let mut first = AtmWatcher::new(config.clone())?;
        assert_eq!(first.poll_once()?.len(), 1);

        let mut second = AtmWatcher::new(config)?;
        assert!(second.poll_once()?.is_empty());

        restore_path(old_path);
        Ok(())
    }

    #[test]
    fn poll_once_returns_empty_on_timeout() -> Result<(), Box<dyn std::error::Error>> {
        let _path_lock = PATH_LOCK.lock().expect("PATH lock");
        let tempdir = TempDir::new()?;
        install_fake_atm(tempdir.path(), "#!/bin/sh\nexit 1\n")?;
        let old_path = std::env::var_os("PATH");
        set_path_with_fake_atm(tempdir.path(), old_path.as_deref())?;

        let config = AtmConfig::new(SessionName::new("demo")?, tempdir.path().join("dedup.txt"));
        let mut watcher = AtmWatcher::new(config)?;
        assert!(watcher.poll_once()?.is_empty());

        restore_path(old_path);
        Ok(())
    }

    #[test]
    fn poll_once_reports_parse_failure() -> Result<(), Box<dyn std::error::Error>> {
        let _path_lock = PATH_LOCK.lock().expect("PATH lock");
        let tempdir = TempDir::new()?;
        install_fake_atm(tempdir.path(), "#!/bin/sh\nprintf 'not-json'\n")?;
        let old_path = std::env::var_os("PATH");
        set_path_with_fake_atm(tempdir.path(), old_path.as_deref())?;

        let config = AtmConfig::new(SessionName::new("demo")?, tempdir.path().join("dedup.txt"));
        let mut watcher = AtmWatcher::new(config)?;
        match watcher.poll_once() {
            Err(AtmError::ParseFailure(_)) => {}
            other => panic!("expected parse failure, got {other:?}"),
        }

        restore_path(old_path);
        Ok(())
    }

    #[test]
    fn poll_once_excludes_self_sender() -> Result<(), Box<dyn std::error::Error>> {
        let _path_lock = PATH_LOCK.lock().expect("PATH lock");
        let tempdir = TempDir::new()?;
        install_fake_atm(
            tempdir.path(),
            "#!/bin/sh\ncat <<'JSON'\n{\"messages\":[{\"from\":\"demo\",\"to\":\"demo\",\"text\":\"hello\",\"timestamp\":\"2026-03-26T00:00:00Z\"}]}\nJSON\n",
        )?;
        let old_path = std::env::var_os("PATH");
        set_path_with_fake_atm(tempdir.path(), old_path.as_deref())?;

        let config = AtmConfig::new(SessionName::new("demo")?, tempdir.path().join("dedup.txt"))
            .with_self_identity(SessionName::new("demo")?);
        let mut watcher = AtmWatcher::new(config)?;
        assert!(watcher.poll_once()?.is_empty());

        restore_path(old_path);
        Ok(())
    }

    #[test]
    fn poll_once_accepts_messages_addressed_to_the_configured_username(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _path_lock = PATH_LOCK.lock().expect("PATH lock");
        let tempdir = TempDir::new()?;
        install_fake_atm(
            tempdir.path(),
            "#!/bin/sh\ncat <<'JSON'\n{\"messages\":[{\"from\":\"arch-term\",\"to\":\"codex-user\",\"text\":\"hello\",\"message_id\":\"msg-1\"}]}\nJSON\n",
        )?;
        let old_path = std::env::var_os("PATH");
        set_path_with_fake_atm(tempdir.path(), old_path.as_deref())?;

        let config = AtmConfig::new(SessionName::new("demo")?, tempdir.path().join("dedup.txt"))
            .with_self_identity(SessionName::new("demo")?)
            .with_username("codex-user");
        let mut watcher = AtmWatcher::new(config)?;
        let events = watcher.poll_once()?;

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sender(), "arch-term");
        assert_eq!(events[0].text(), "hello");

        restore_path(old_path);
        Ok(())
    }

    #[test]
    fn poll_once_filters_messages_for_unknown_usernames() -> Result<(), Box<dyn std::error::Error>>
    {
        let _path_lock = PATH_LOCK.lock().expect("PATH lock");
        let tempdir = TempDir::new()?;
        install_fake_atm(
            tempdir.path(),
            "#!/bin/sh\ncat <<'JSON'\n{\"messages\":[{\"from\":\"arch-term\",\"to\":\"other-user\",\"text\":\"hello\",\"message_id\":\"msg-2\"}]}\nJSON\n",
        )?;
        let old_path = std::env::var_os("PATH");
        set_path_with_fake_atm(tempdir.path(), old_path.as_deref())?;

        let config = AtmConfig::new(SessionName::new("demo")?, tempdir.path().join("dedup.txt"))
            .with_self_identity(SessionName::new("demo")?)
            .with_username("codex-user");
        let mut watcher = AtmWatcher::new(config)?;
        assert!(watcher.poll_once()?.is_empty());

        restore_path(old_path);
        Ok(())
    }

    fn install_fake_atm(
        root: &std::path::Path,
        body: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = root.join("atm");
        fs::write(&path, body)?;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions)?;
        Ok(())
    }

    fn restore_path(path: Option<std::ffi::OsString>) {
        if let Some(path) = path {
            // SAFETY: guarded by PATH_LOCK; these tests mutate PATH only while holding
            // the process-wide mutex so no concurrent environment access occurs here.
            #[allow(unsafe_code, reason = "test-only PATH mutation under PATH_LOCK")]
            unsafe {
                std::env::set_var("PATH", path);
            }
        } else {
            // SAFETY: guarded by PATH_LOCK; these tests mutate PATH only while holding
            // the process-wide mutex so no concurrent environment access occurs here.
            #[allow(unsafe_code, reason = "test-only PATH mutation under PATH_LOCK")]
            unsafe {
                std::env::remove_var("PATH");
            }
        }
    }

    fn set_path_with_fake_atm(
        fake_root: &std::path::Path,
        old_path: Option<&std::ffi::OsStr>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut paths = vec![fake_root.to_path_buf()];
        if let Some(old_path) = old_path {
            paths.extend(std::env::split_paths(old_path));
        }
        let joined: OsString = std::env::join_paths(paths)?;
        // SAFETY: guarded by PATH_LOCK; these tests mutate PATH only while holding
        // the process-wide mutex so no concurrent environment access occurs here.
        #[allow(unsafe_code, reason = "test-only PATH mutation under PATH_LOCK")]
        unsafe {
            std::env::set_var("PATH", joined);
        }
        Ok(())
    }
}
