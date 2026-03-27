//! Unix runtime integration for `scterm`.
//!
//! This crate provides sealed Unix backends for PTYs and Unix-domain sockets,
//! plus supporting raw-mode, signal, and daemonization helpers.

mod error;
mod process;
pub(crate) mod process_lock;
mod pty;
mod raw_mode;
mod signal;
mod socket;

mod sealed {
    pub trait Sealed {}
}

#[doc(inline)]
pub use error::UnixError;
#[doc(inline)]
pub use process::{create_process_group, daemonize, DaemonizeOutcome};
#[doc(inline)]
pub use pty::{PtyCommand, PtyProcess, UnixPtyBackend};
#[doc(inline)]
pub use raw_mode::{terminal_window_size, RawModeGuard};
#[doc(inline)]
pub use signal::{SignalEvent, SignalWatcher};
#[doc(inline)]
pub use socket::{UnixSocketListener, UnixSocketStream, UnixSocketTransport};

/// A sealed PTY backend for Unix platforms.
pub trait PtyBackend: sealed::Sealed {
    /// Spawns a child process attached to a PTY.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the PTY cannot be created, the child process
    /// cannot be started, or the exec handshake reports a failure.
    fn spawn(
        &self,
        command: &PtyCommand,
        size: Option<scterm_core::WindowSize>,
    ) -> Result<PtyProcess, UnixError>;
}

/// A sealed Unix-domain socket transport.
pub trait SocketTransport: sealed::Sealed {
    /// Binds a listening socket at `path`.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the socket cannot be created or bound.
    fn bind(&self, path: &scterm_core::SessionPath) -> Result<UnixSocketListener, UnixError>;

    /// Connects a stream socket to `path`.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the path is absent, stale, invalid, or
    /// otherwise fails to connect.
    fn connect(&self, path: &scterm_core::SessionPath) -> Result<UnixSocketStream, UnixError>;
}
