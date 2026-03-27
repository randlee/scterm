//! Structured logging helpers for the app boundary.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Logger setup is runtime-oriented and not improved by const qualification."
)]

use anyhow::{Context, Result};
use serde_json::json;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// A minimal structured logger owned by `scterm-app`.
///
/// The implementation keeps the logging boundary local to the application
/// layer while using the logging-only `sc-observability` facade crate.
pub struct AppLogger {
    path: PathBuf,
    file: Mutex<File>,
}

impl std::fmt::Debug for AppLogger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppLogger")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl AppLogger {
    /// Creates an application-scoped JSONL logger rooted at `log_root`.
    ///
    /// # Errors
    /// Returns an error when the log directory cannot be created.
    pub fn new(log_root: impl Into<PathBuf>) -> Result<Self> {
        let log_root = log_root.into();
        let path = log_root
            .join("scterm")
            .join("logs")
            .join("scterm.log.jsonl");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create app log directory {}", parent.display()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open app structured log {}", path.display()))?;

        Ok(Self {
            path,
            file: Mutex::new(file),
        })
    }

    /// Appends one structured event line.
    ///
    /// # Errors
    /// Returns an error when the log file cannot be opened or written.
    pub fn emit(&self, target: &str, action: &str, message: &str) -> Result<()> {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock precedes UNIX_EPOCH")?
            .as_millis();
        let event = json!({
            "version": 1,
            "timestamp_ms": timestamp_ms,
            "level": "INFO",
            "service": "scterm",
            "target": target,
            "action": action,
            "message": message,
            "outcome": "ok",
        });
        let mut file = self
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("app structured logger mutex poisoned"))?;
        writeln!(file, "{event}")
            .with_context(|| format!("emit structured log event to {}", self.path.display()))?;
        file.flush()
            .with_context(|| format!("flush structured log event to {}", self.path.display()))?;
        drop(file);
        Ok(())
    }

    /// Returns the JSONL log file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
