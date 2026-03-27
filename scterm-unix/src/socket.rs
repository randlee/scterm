//! Unix-domain socket transport.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the socket transport API."
)]

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use crate::sealed::Sealed;
use crate::{cwd_sensitive_filesystem_lock, SocketTransport, UnixError};
use scterm_core::SessionPath;

/// A Unix-domain socket transport implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnixSocketTransport;

impl Sealed for UnixSocketTransport {}

impl SocketTransport for UnixSocketTransport {
    fn bind(&self, path: &SessionPath) -> Result<UnixSocketListener, UnixError> {
        let listener = with_socket_path(path.as_path(), |socket_path| {
            let _ = fs::remove_file(socket_path);
            UnixListener::bind(socket_path).map_err(|source| UnixError::Socket {
                operation: "bind",
                path: path.as_path().to_path_buf(),
                source,
            })
        })?;

        listener
            .set_nonblocking(false)
            .map_err(|source| UnixError::Socket {
                operation: "set_nonblocking",
                path: path.as_path().to_path_buf(),
                source,
            })?;

        Ok(UnixSocketListener {
            listener,
            path: path.as_path().to_path_buf(),
        })
    }

    fn connect(&self, path: &SessionPath) -> Result<UnixSocketStream, UnixError> {
        with_socket_path(path.as_path(), |socket_path| {
            UnixStream::connect(socket_path)
                .map(|stream| UnixSocketStream { stream })
                .map_err(|source| classify_connect_error(path.as_path(), source))
        })
    }
}

/// A bound Unix-domain listener.
#[derive(Debug)]
pub struct UnixSocketListener {
    listener: UnixListener,
    path: PathBuf,
}

impl UnixSocketListener {
    /// Accepts the next client connection.
    ///
    /// # Errors
    /// Returns [`UnixError`] when accepting the next client fails.
    pub fn accept(&self) -> Result<UnixSocketStream, UnixError> {
        self.listener
            .accept()
            .map(|(stream, _addr)| UnixSocketStream { stream })
            .map_err(|source| UnixError::Socket {
                operation: "accept",
                path: self.path.clone(),
                source,
            })
    }

    /// Returns the bound socket path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the underlying file descriptor as a borrowed handle.
    #[must_use]
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.listener.as_fd()
    }
}

/// A connected Unix-domain stream.
#[derive(Debug)]
pub struct UnixSocketStream {
    stream: UnixStream,
}

impl UnixSocketStream {
    /// Reads bytes from the stream.
    ///
    /// # Errors
    /// Returns [`UnixError`] when reading fails.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, UnixError> {
        self.stream
            .read(buffer)
            .map_err(|source| UnixError::Socket {
                operation: "read",
                path: PathBuf::from("<connected-stream>"),
                source,
            })
    }

    /// Writes bytes to the stream.
    ///
    /// # Errors
    /// Returns [`UnixError`] when writing fails.
    pub fn write(&mut self, buffer: &[u8]) -> Result<usize, UnixError> {
        self.stream
            .write(buffer)
            .map_err(|source| UnixError::Socket {
                operation: "write",
                path: PathBuf::from("<connected-stream>"),
                source,
            })
    }

    /// Flushes the stream.
    ///
    /// # Errors
    /// Returns [`UnixError`] when flushing fails.
    pub fn flush(&mut self) -> Result<(), UnixError> {
        self.stream.flush().map_err(|source| UnixError::Socket {
            operation: "flush",
            path: PathBuf::from("<connected-stream>"),
            source,
        })
    }

    /// Reads exactly `buffer.len()` bytes from the stream.
    ///
    /// # Errors
    /// Returns [`UnixError`] when the stream is closed early or the read fails.
    pub fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), UnixError> {
        self.stream
            .read_exact(buffer)
            .map_err(|source| UnixError::Socket {
                operation: "read_exact",
                path: PathBuf::from("<connected-stream>"),
                source,
            })
    }

    /// Writes all bytes from `buffer` to the stream.
    ///
    /// # Errors
    /// Returns [`UnixError`] when writing fails.
    pub fn write_all(&mut self, buffer: &[u8]) -> Result<(), UnixError> {
        self.stream
            .write_all(buffer)
            .map_err(|source| UnixError::Socket {
                operation: "write_all",
                path: PathBuf::from("<connected-stream>"),
                source,
            })
    }

    /// Returns the underlying file descriptor as a borrowed handle.
    #[must_use]
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }

    /// Creates another handle to the same connected Unix stream.
    ///
    /// # Errors
    /// Returns [`UnixError`] when duplicating the stream fails.
    pub fn try_clone(&self) -> Result<Self, UnixError> {
        self.stream
            .try_clone()
            .map(|stream| Self { stream })
            .map_err(|source| UnixError::Socket {
                operation: "try_clone",
                path: PathBuf::from("<connected-stream>"),
                source,
            })
    }
}

fn classify_connect_error(path: &Path, source: io::Error) -> UnixError {
    match source.raw_os_error() {
        Some(code) if code == libc::ENOENT => UnixError::absent_session(path.to_path_buf(), source),
        Some(code) if code == libc::ECONNREFUSED => match fs::metadata(path) {
            Ok(metadata) if metadata.file_type().is_socket() => {
                UnixError::stale_socket(path.to_path_buf(), source)
            }
            Ok(_) => UnixError::invalid_session(
                path.to_path_buf(),
                io::Error::from_raw_os_error(libc::ENOTSOCK),
            ),
            Err(_) => UnixError::stale_socket(path.to_path_buf(), source),
        },
        Some(code) if code == libc::ENOTSOCK => {
            UnixError::invalid_session(path.to_path_buf(), source)
        }
        _ => UnixError::Socket {
            operation: "connect",
            path: path.to_path_buf(),
            source,
        },
    }
}

fn with_socket_path<T>(
    path: &Path,
    operation: impl FnOnce(&Path) -> Result<T, UnixError>,
) -> Result<T, UnixError> {
    if path.as_os_str().as_bytes().len() <= sun_path_limit() {
        return operation(path);
    }

    let _lock = cwd_sensitive_filesystem_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let parent = path.parent().ok_or_else(|| {
        UnixError::invalid_session(
            path.to_path_buf(),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "socket path has no parent directory",
            ),
        )
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        UnixError::invalid_session(
            path.to_path_buf(),
            io::Error::new(io::ErrorKind::InvalidInput, "socket path has no basename"),
        )
    })?;

    let cwd = File::open(".").map_err(|source| UnixError::Socket {
        operation: "open-cwd",
        path: path.to_path_buf(),
        source,
    })?;
    std::env::set_current_dir(parent).map_err(|source| UnixError::Socket {
        operation: "chdir-parent",
        path: path.to_path_buf(),
        source,
    })?;

    let result = operation(Path::new(file_name));

    let restore = nix::unistd::fchdir(cwd.as_raw_fd()).map_err(|error| UnixError::Socket {
        operation: "restore-cwd",
        path: path.to_path_buf(),
        source: io::Error::from_raw_os_error(error as i32),
    });

    match (result, restore) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

const fn sun_path_limit() -> usize {
    #[cfg(target_os = "macos")]
    {
        103
    }

    #[cfg(not(target_os = "macos"))]
    {
        107
    }
}
