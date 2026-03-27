//! Process-group and daemonization helpers.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the process helper API."
)]

use std::io;

use nix::unistd::{fork, getpid, setpgid, setsid, ForkResult, Pid};

use crate::UnixError;

/// The result of a daemonization fork.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonizeOutcome {
    /// Returned to the original parent process.
    Parent {
        /// The child process id.
        child_pid: Pid,
    },
    /// Returned in the daemon child process.
    Child,
}

/// Creates a new process group for the current process.
///
/// # Errors
/// Returns [`UnixError`] when `setpgid` fails.
pub fn create_process_group() -> Result<Pid, UnixError> {
    let pid = getpid();
    setpgid(pid, pid).map_err(nix_to_process_error("setpgid"))?;
    Ok(pid)
}

/// Forks and detaches the current process into a daemon session.
///
/// # Errors
/// Returns [`UnixError`] when `fork` or `setsid` fails.
pub fn daemonize() -> Result<DaemonizeOutcome, UnixError> {
    let fork_result = {
        // SAFETY: `fork` is confined to this Unix-only crate. The caller is
        // responsible for invoking `daemonize` before spawning threads or
        // performing non-fork-safe work in the current process.
        unsafe { fork() }
    }
    .map_err(nix_to_process_error("fork"))?;

    match fork_result {
        ForkResult::Parent { child } => Ok(DaemonizeOutcome::Parent { child_pid: child }),
        ForkResult::Child => {
            setsid().map_err(nix_to_process_error("setsid"))?;
            Ok(DaemonizeOutcome::Child)
        }
    }
}

fn nix_to_process_error(operation: &'static str) -> impl FnOnce(nix::Error) -> UnixError + Copy {
    move |error| UnixError::Process {
        operation,
        source: io::Error::from_raw_os_error(error as i32),
    }
}
