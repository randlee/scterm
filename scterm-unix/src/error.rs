//! Typed runtime errors for `scterm-unix`.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the public Unix error API."
)]

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// A typed Unix runtime error.
#[derive(Debug, Error)]
pub enum UnixError {
    /// The socket path does not exist.
    #[error("socket path does not exist: {path}")]
    AbsentSession {
        /// The missing socket path.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The socket path exists but no listener is alive.
    #[error("socket is stale: {path}")]
    StaleSocket {
        /// The stale socket path.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The socket path exists but is not a socket.
    #[error("socket path is not a valid session socket: {path}")]
    InvalidSession {
        /// The invalid socket path.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// A Unix-domain socket operation failed.
    #[error("socket operation `{operation}` failed for {path}: {source}")]
    Socket {
        /// The failed operation name.
        operation: &'static str,
        /// The affected path.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// PTY creation or child setup failed.
    #[error("pty operation `{operation}` failed: {source}")]
    Pty {
        /// The failed PTY operation.
        operation: &'static str,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// Entering or restoring raw mode failed.
    #[error("raw terminal mode operation `{operation}` failed: {source}")]
    RawMode {
        /// The failed raw-mode operation.
        operation: &'static str,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// Signal registration or delivery failed.
    #[error("signal operation `{operation}` failed: {source}")]
    Signal {
        /// The failed signal operation.
        operation: &'static str,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// Daemonization or process-group setup failed.
    #[error("process operation `{operation}` failed: {source}")]
    Process {
        /// The failed process operation.
        operation: &'static str,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
}

impl UnixError {
    /// Creates a stale-socket error.
    #[must_use]
    pub fn stale_socket(path: PathBuf, source: io::Error) -> Self {
        Self::StaleSocket { path, source }
    }

    /// Creates an invalid-session error.
    #[must_use]
    pub fn invalid_session(path: PathBuf, source: io::Error) -> Self {
        Self::InvalidSession { path, source }
    }

    /// Creates an absent-session error.
    #[must_use]
    pub fn absent_session(path: PathBuf, source: io::Error) -> Self {
        Self::AbsentSession { path, source }
    }

    /// Returns whether the error represents a stale socket.
    #[must_use]
    pub fn is_stale_socket(&self) -> bool {
        matches!(self, Self::StaleSocket { .. })
    }

    /// Returns whether the error represents an invalid session path.
    #[must_use]
    pub fn is_invalid_session(&self) -> bool {
        matches!(self, Self::InvalidSession { .. })
    }

    /// Returns whether the error represents an absent session.
    #[must_use]
    pub fn is_absent_session(&self) -> bool {
        matches!(self, Self::AbsentSession { .. })
    }
}
