//! PTY backend implementation.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the PTY backend API."
)]

use std::env;
use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};

use nix::fcntl::{fcntl, FcntlArg, FdFlag};
use nix::pty::{forkpty, Winsize};
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{execvp, read, write, Pid};
use scterm_core::WindowSize;

use crate::sealed::Sealed;
use crate::{PtyBackend, UnixError};

/// A PTY command builder for Unix process execution.
#[derive(Debug, Clone)]
pub struct PtyCommand {
    program: CString,
    args: Vec<CString>,
    env: Vec<(CString, CString)>,
}

impl PtyCommand {
    /// Creates a command that will execute `program`.
    ///
    /// # Errors
    /// Returns [`UnixError`] when `program` contains an interior NUL byte.
    pub fn new(program: &str) -> Result<Self, UnixError> {
        let program_cstr = cstring(program, "program")?;
        Ok(Self {
            args: vec![program_cstr.clone()],
            program: program_cstr,
            env: Vec::new(),
        })
    }

    /// Appends one argument.
    ///
    /// # Errors
    /// Returns [`UnixError`] when `arg` contains an interior NUL byte.
    pub fn arg(mut self, arg: &str) -> Result<Self, UnixError> {
        self.args.push(cstring(arg, "arg")?);
        Ok(self)
    }

    /// Appends one environment variable assignment.
    ///
    /// # Errors
    /// Returns [`UnixError`] when `key` or `value` contains an interior NUL
    /// byte.
    pub fn env(mut self, key: &str, value: &str) -> Result<Self, UnixError> {
        self.env
            .push((cstring(key, "env-key")?, cstring(value, "env-value")?));
        Ok(self)
    }
}

/// A running PTY-backed child process.
#[derive(Debug)]
pub struct PtyProcess {
    master: OwnedFd,
    child_pid: Pid,
}

impl PtyProcess {
    /// Returns the child process id.
    #[must_use]
    pub fn child_pid(&self) -> Pid {
        self.child_pid
    }

    /// Reads bytes from the PTY master.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the PTY read fails.
    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, UnixError> {
        read(self.master.as_raw_fd(), buffer).map_err(nix_to_pty_error("read"))
    }

    /// Writes bytes to the PTY master.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the PTY write fails.
    pub fn write(&self, buffer: &[u8]) -> Result<usize, UnixError> {
        write(&self.master, buffer).map_err(nix_to_pty_error("write"))
    }

    /// Resizes the PTY window.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the resize ioctl fails.
    pub fn resize(&self, size: WindowSize) -> Result<(), UnixError> {
        let winsize = to_nix_winsize(size);
        let result = {
            // SAFETY: `self.master` is a valid PTY master file descriptor owned
            // by this process. `winsize` points to a properly initialized
            // `libc::winsize` value for the duration of the ioctl call.
            unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) }
        };

        if result == -1 {
            return Err(UnixError::Pty {
                operation: "ioctl(TIOCSWINSZ)",
                source: io::Error::last_os_error(),
            });
        }

        Ok(())
    }

    /// Sends `signal` to the child process group.
    ///
    /// # Errors
    /// Returns [`UnixError`] when signaling fails.
    pub fn signal_group(&self, signal: Signal) -> Result<(), UnixError> {
        kill(Pid::from_raw(-self.child_pid.as_raw()), signal).map_err(nix_to_pty_error("kill"))?;
        Ok(())
    }

    /// Waits for the child process to exit.
    ///
    /// # Errors
    /// Returns [`UnixError`] when `waitpid` fails.
    pub fn wait(&self) -> Result<WaitStatus, UnixError> {
        waitpid(self.child_pid, None).map_err(nix_to_pty_error("waitpid"))
    }
}

/// The Unix PTY backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnixPtyBackend;

impl Sealed for UnixPtyBackend {}

impl PtyBackend for UnixPtyBackend {
    fn spawn(
        &self,
        command: &PtyCommand,
        size: Option<WindowSize>,
    ) -> Result<PtyProcess, UnixError> {
        let (status_read, status_write) = nix::unistd::pipe().map_err(nix_to_pty_error("pipe"))?;
        fcntl(
            status_write.as_raw_fd(),
            FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC),
        )
        .map_err(nix_to_pty_error("fcntl(FD_CLOEXEC)"))?;

        let fork_result = {
            let winsize = size.map(to_nix_winsize);
            // SAFETY: `forkpty` is confined to this Unix-only crate. The caller
            // invokes the PTY backend from a process that is prepared for fork
            // semantics, and the child branch performs only async-signal-safe
            // work before `execvp` or `_exit`.
            unsafe { forkpty(winsize.as_ref(), None) }
        }
        .map_err(nix_to_pty_error("forkpty"))?;

        match fork_result {
            nix::pty::ForkptyResult::Parent { child, master } => {
                drop(status_write);
                let mut status_bytes = [0_u8; 4];
                let read_result = read(status_read.as_raw_fd(), &mut status_bytes);
                drop(status_read);

                match read_result {
                    Ok(0) => Ok(PtyProcess {
                        master,
                        child_pid: child,
                    }),
                    Ok(_) => {
                        let errno = i32::from_ne_bytes(status_bytes);
                        let _ = kill(child, Signal::SIGTERM);
                        Err(UnixError::Pty {
                            operation: "exec-handshake",
                            source: io::Error::from_raw_os_error(errno),
                        })
                    }
                    Err(error) => Err(nix_to_pty_error("read(exec-handshake)")(error)),
                }
            }
            nix::pty::ForkptyResult::Child => {
                drop(status_read);
                for (key, value) in &command.env {
                    // SAFETY: This code runs in the freshly-forked child just
                    // before `execvp`. No other threads are active in the child
                    // branch, so mutating the process environment is safe here.
                    unsafe {
                        env::set_var(cstr_to_string(key), cstr_to_string(value));
                    }
                }

                let argv = command
                    .args
                    .iter()
                    .map(CString::as_c_str)
                    .collect::<Vec<&CStr>>();
                match execvp(&command.program, &argv) {
                    Ok(_) => unreachable!("execvp only returns on error"),
                    Err(error) => {
                        let errno = error as i32;
                        let bytes = errno.to_ne_bytes();
                        let _ = write(&status_write, &bytes);
                        std::process::exit(127);
                    }
                }
            }
        }
    }
}

fn cstring(value: &str, context: &'static str) -> Result<CString, UnixError> {
    CString::new(value).map_err(|source| UnixError::Pty {
        operation: context,
        source: io::Error::new(io::ErrorKind::InvalidInput, source),
    })
}

fn cstr_to_string(value: &CStr) -> String {
    value.to_string_lossy().into_owned()
}

fn to_nix_winsize(size: WindowSize) -> Winsize {
    Winsize {
        ws_row: size.rows(),
        ws_col: size.cols(),
        ws_xpixel: size.xpixel(),
        ws_ypixel: size.ypixel(),
    }
}

fn nix_to_pty_error(operation: &'static str) -> impl FnOnce(nix::Error) -> UnixError + Copy {
    move |error| UnixError::Pty {
        operation,
        source: io::Error::from_raw_os_error(error as i32),
    }
}
