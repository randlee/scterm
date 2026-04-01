//! Signal watcher helpers.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the signal watcher API."
)]

use signal_hook::consts::signal::{SIGCHLD, SIGWINCH};
use signal_hook::iterator::Signals;

use crate::UnixError;

/// A signal event observed by [`SignalWatcher`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalEvent {
    /// A child-process state change was observed.
    ChildExit,
    /// A terminal resize signal was observed.
    WindowChange,
}

/// A blocking or polling watcher for `SIGCHLD` and `SIGWINCH`.
#[derive(Debug)]
pub struct SignalWatcher {
    signals: Signals,
}

impl SignalWatcher {
    /// Registers interest in `SIGCHLD` and `SIGWINCH`.
    ///
    /// # Errors
    /// Returns [`UnixError`] when signal registration fails.
    pub fn new() -> Result<Self, UnixError> {
        let signals = Signals::new([SIGCHLD, SIGWINCH]).map_err(|error| UnixError::Signal {
            operation: "register",
            source: error,
        })?;
        Ok(Self { signals })
    }

    /// Returns all currently pending signal events.
    #[must_use]
    pub fn pending(&mut self) -> Vec<SignalEvent> {
        self.signals
            .pending()
            .filter_map(|signal| match signal {
                SIGCHLD => Some(SignalEvent::ChildExit),
                SIGWINCH => Some(SignalEvent::WindowChange),
                _ => None,
            })
            .collect()
    }
}
