//! Structured logging helpers for the app boundary.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Logger setup is runtime-oriented and not improved by const qualification."
)]

use anyhow::{Context, Result};
use sc_observability::{Logger, LoggerConfig};
use sc_observability_types::{
    ActionName, Level, LogEvent, ProcessIdentity, ServiceName, TargetCategory, Timestamp,
};
use serde_json::Map;
use std::path::{Path, PathBuf};

fn service_name() -> ServiceName {
    ServiceName::new("scterm").expect("static service name is valid")
}

/// A minimal structured logger owned by `scterm-app`.
///
/// The implementation keeps the logging boundary local to the application
/// layer while using the logging-only `sc-observability` facade crate.
pub struct AppLogger {
    path: PathBuf,
    logger: Logger,
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
        let config = LoggerConfig::default_for(service_name(), log_root.clone());
        let path = log_root
            .join("scterm")
            .join("logs")
            .join("scterm.log.jsonl");
        let logger = Logger::new(config).context("initialize app structured logger")?;

        Ok(Self { path, logger })
    }

    /// Appends one structured event line.
    ///
    /// # Errors
    /// Returns an error when the log file cannot be opened or written.
    pub fn emit(&self, target: &str, action: &str, message: &str) -> Result<()> {
        let event = LogEvent {
            version: sc_observability_types::constants::OBSERVATION_ENVELOPE_VERSION.to_string(),
            timestamp: Timestamp::UNIX_EPOCH,
            level: Level::Info,
            service: service_name(),
            target: TargetCategory::new(target)
                .with_context(|| format!("validate log target `{target}`"))?,
            action: ActionName::new(action)
                .with_context(|| format!("validate log action `{action}`"))?,
            message: Some(message.to_string()),
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id: None,
            outcome: Some("ok".to_string()),
            diagnostic: None,
            state_transition: None,
            fields: Map::default(),
        };
        self.logger
            .emit(event)
            .with_context(|| format!("emit structured log event to {}", self.path.display()))?;
        self.logger
            .flush()
            .with_context(|| format!("flush structured log event to {}", self.path.display()))?;
        Ok(())
    }

    /// Returns the JSONL log file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
