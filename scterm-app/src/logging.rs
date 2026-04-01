//! Structured logging helpers for the app boundary.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Logger setup is runtime-oriented and not improved by const qualification."
)]

use anyhow::{Context, Result};
use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::{
    ActionName, Level, LogEvent, ProcessIdentity, ServiceName, TargetCategory,
};
use serde_json::Map;
use std::path::PathBuf;
use time::OffsetDateTime;

const SERVICE_NAME: &str = "scterm";

/// Application-scoped structured logger backed by `sc-observability`.
///
/// Wraps [`sc_observability::Logger`] with a simple `emit(target, action, message)` API
/// consistent with the Rust ecosystem logging convention. Log root and service name are
/// fixed at construction; the caller supplies the root directory.
///
/// Log files are written to `{log_root}/scterm/logs/scterm.log.jsonl` (rotated by
/// the sc-observability defaults).
///
/// The log root can also be set via the `SC_LOG_ROOT` environment variable when `log_root`
/// is empty. The ATM app layer sets `SC_LOG_ROOT` at launch to unify scterm and schook
/// logs under a common root without requiring this crate to read `ATM_HOME`.
pub struct AppLogger {
    inner: Logger,
    service: ServiceName,
}

impl std::fmt::Debug for AppLogger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppLogger")
            .field("service", &self.service)
            .finish_non_exhaustive()
    }
}

impl AppLogger {
    /// Creates an application-scoped structured logger rooted at `log_root`.
    ///
    /// # Errors
    /// Returns an error when the log directory cannot be created or the logger
    /// fails to initialize.
    pub fn new(log_root: impl Into<PathBuf>) -> Result<Self> {
        let service =
            ServiceName::new(SERVICE_NAME).expect("'scterm' is a valid service name identifier");
        let config = LoggerConfig::default_for(service.clone(), log_root.into());
        let inner = Logger::new(config).context("initialize sc-observability logger for scterm")?;
        Ok(Self { inner, service })
    }

    /// Appends one structured event line.
    ///
    /// `target` and `action` must satisfy `[A-Za-z0-9._-]+`.
    ///
    /// # Errors
    /// Returns an error when `target` or `action` fail validation, or when the
    /// underlying log sink cannot be written.
    pub fn emit(&self, target: &str, action: &str, message: &str) -> Result<()> {
        let target = TargetCategory::new(target)
            .with_context(|| format!("invalid log target category: {target:?}"))?;
        let action = ActionName::new(action)
            .with_context(|| format!("invalid log action name: {action:?}"))?;
        let event = LogEvent {
            version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: OffsetDateTime::now_utc(),
            level: Level::Info,
            service: self.service.clone(),
            target,
            action,
            message: Some(message.to_owned()),
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: Some("ok".to_owned()),
            diagnostic: None,
            state_transition: None,
            fields: Map::default(),
        };
        self.inner
            .emit(event)
            .context("emit structured log event to sc-observability")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppLogger;
    use anyhow::Result;
    use tempfile::TempDir;

    #[test]
    fn app_logger_writes_jsonl_events() -> Result<()> {
        let tempdir = TempDir::new()?;
        let logger = AppLogger::new(tempdir.path())?;

        logger.emit("master", "start", "session starting")?;

        let log_path = tempdir
            .path()
            .join("scterm")
            .join("logs")
            .join("scterm.log.jsonl");
        let contents = std::fs::read_to_string(&log_path).expect("log file should exist");
        assert!(contents.contains("\"target\":\"master\""));
        assert!(contents.contains("\"action\":\"start\""));
        Ok(())
    }
}
