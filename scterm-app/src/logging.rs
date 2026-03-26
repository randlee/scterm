//! Structured logging helpers for the app boundary.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Logger setup is runtime-oriented and not improved by const qualification."
)]

use anyhow::{Context, Result};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A minimal structured logger owned by `scterm-app`.
///
/// The implementation keeps the logging boundary local to the application
/// layer while anchoring the dependency on the logging-only
/// `sc-observability` facade crate.
#[derive(Debug)]
pub struct AppLogger {
    path: PathBuf,
    _marker: PhantomData<sc_observability::Logger>,
}

impl AppLogger {
    /// Creates an application-scoped JSONL logger rooted at `log_root`.
    ///
    /// # Errors
    /// Returns an error when the log directory cannot be created.
    pub fn new(log_root: impl Into<PathBuf>) -> Result<Self> {
        let log_root = log_root.into();
        fs::create_dir_all(&log_root)
            .with_context(|| format!("create app log directory at {}", log_root.display()))?;

        Ok(Self {
            path: log_root.join("scterm-app.jsonl"),
            _marker: PhantomData,
        })
    }

    /// Appends one structured event line.
    ///
    /// # Errors
    /// Returns an error when the log file cannot be opened or written.
    pub fn emit(&self, target: &str, action: &str, message: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open structured log at {}", self.path.display()))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before the unix epoch")?
            .as_secs();
        let event = json!({
            "timestamp_unix": timestamp,
            "service": "scterm",
            "target": target,
            "action": action,
            "message": message,
        });

        writeln!(file, "{event}")
            .with_context(|| format!("append structured log event to {}", self.path.display()))?;
        Ok(())
    }

    /// Returns the JSONL log file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
