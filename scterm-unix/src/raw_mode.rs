//! Raw terminal mode helpers.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the raw-mode guard API."
)]

use std::io;
use std::os::fd::{AsFd, AsRawFd, RawFd};

use nix::sys::termios::{self, SetArg, Termios};

use crate::UnixError;

/// An RAII guard that restores the original terminal mode on drop.
#[derive(Debug)]
pub struct RawModeGuard {
    fd: RawFd,
    original: Termios,
}

impl RawModeGuard {
    /// Enters raw mode for `fd`.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the current terminal state cannot be read or
    /// raw mode cannot be installed.
    pub fn new(fd: &impl AsFd) -> Result<Self, UnixError> {
        let borrowed = fd.as_fd();
        let mut raw = termios::tcgetattr(borrowed).map_err(nix_to_raw_mode_error("tcgetattr"))?;
        let original = raw.clone();
        termios::cfmakeraw(&mut raw);
        termios::tcsetattr(borrowed, SetArg::TCSANOW, &raw)
            .map_err(nix_to_raw_mode_error("tcsetattr"))?;

        Ok(Self {
            fd: borrowed.as_raw_fd(),
            original,
        })
    }

    /// Restores the original terminal mode immediately.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the original mode cannot be restored.
    pub fn restore(&self) -> Result<(), UnixError> {
        // SAFETY: `self.fd` was captured from a live file descriptor when the
        // guard was created. The caller must keep that descriptor open for the
        // lifetime of the guard.
        let borrowed = unsafe { std::os::fd::BorrowedFd::borrow_raw(self.fd) };
        termios::tcsetattr(borrowed, SetArg::TCSANOW, &self.original)
            .map_err(nix_to_raw_mode_error("tcsetattr"))
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn nix_to_raw_mode_error(operation: &'static str) -> impl FnOnce(nix::Error) -> UnixError + Copy {
    move |error| UnixError::RawMode {
        operation,
        source: io::Error::from_raw_os_error(error as i32),
    }
}
