//! Unix integration tests for `scterm-unix`.

use std::error::Error;
use std::fs;
use std::os::fd::BorrowedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nix::poll::{poll, PollFd, PollFlags};
use nix::pty::openpty;
use nix::sys::signal::Signal;
use nix::sys::termios::{self, LocalFlags};
use scterm_core::{SessionPath, WindowSize};
use scterm_unix::{
    PtyBackend, PtyCommand, RawModeGuard, SocketTransport, UnixPtyBackend, UnixSocketTransport,
};
use tempfile::TempDir;

const TEST_TIMEOUT: Duration = Duration::from_secs(5);
const TEST_TIMEOUT_MS: u16 = 5_000;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

fn wait_for_readable(fd: BorrowedFd<'_>, label: &str) -> TestResult<()> {
    let mut poll_fds = [PollFd::new(fd, PollFlags::POLLIN)];
    if poll(&mut poll_fds, TEST_TIMEOUT_MS)? == 0 {
        return Err(Box::new(std::io::Error::other(format!(
            "timed out waiting for {label}"
        ))));
    }
    Ok(())
}

fn recv_with_timeout<T>(receiver: &mpsc::Receiver<TestResult<T>>, label: &str) -> TestResult<T> {
    receiver
        .recv_timeout(TEST_TIMEOUT)
        .map_err(|_| std::io::Error::other(format!("timed out waiting for {label}")))?
}

#[test]
fn socket_transport_supports_bind_connect_accept_and_close() -> TestResult<()> {
    let tempdir = TempDir::new()?;
    let path = SessionPath::new(tempdir.path().join("socket"))?;
    let transport = UnixSocketTransport;
    let listener = transport.bind(&path)?;

    let (client_tx, client_rx) = mpsc::channel();
    thread::spawn(move || {
        let transport = UnixSocketTransport;
        let result = (|| -> TestResult<()> {
            let mut stream = transport.connect(&path)?;
            stream.write(b"ping")?;
            Ok(())
        })();
        let _ = client_tx.send(result);
    });

    wait_for_readable(listener.as_fd(), "socket accept")?;
    let mut server = listener.accept()?;
    let mut buffer = [0_u8; 4];
    wait_for_readable(server.as_fd(), "socket read")?;
    let read = server.read(&mut buffer)?;

    recv_with_timeout(&client_rx, "client thread")?;
    assert_eq!(&buffer[..read], b"ping");
    Ok(())
}

#[test]
fn socket_transport_uses_chdir_indirection_for_long_paths() -> TestResult<()> {
    let tempdir = TempDir::new()?;
    let prefix_len = tempdir.path().as_os_str().as_bytes().len();
    let padding = "x".repeat(108usize.saturating_sub(prefix_len) + 10);
    let long_dir = tempdir.path().join(padding);
    fs::create_dir_all(&long_dir)?;

    let full_path = long_dir.join("s.sock");
    assert!(full_path.as_os_str().as_bytes().len() > 107);

    let session_path = SessionPath::new(full_path.clone())?;
    let transport = UnixSocketTransport;
    let listener = transport.bind(&session_path)?;

    let (client_tx, client_rx) = mpsc::channel();
    thread::spawn(move || {
        let transport = UnixSocketTransport;
        let result = transport
            .connect(&session_path)
            .map(|_| ())
            .map_err(Into::into);
        let _ = client_tx.send(result);
    });

    wait_for_readable(listener.as_fd(), "long-path accept")?;
    let _server = listener.accept()?;
    recv_with_timeout(&client_rx, "long-path client")?;
    assert!(fs::metadata(full_path)?.file_type().is_socket());
    Ok(())
}

#[test]
fn pty_backend_spawns_and_echoes_through_the_master() -> TestResult<()> {
    let backend = UnixPtyBackend;
    let command = PtyCommand::new("/bin/sh")?.arg("-c")?.arg("cat")?;
    let process = backend.spawn(&command, Some(WindowSize::new(24, 80, 0, 0)))?;

    process.write(b"hello from pty\n")?;
    wait_for_readable(process.as_fd(), "PTY output")?;

    let mut buffer = [0_u8; 256];
    let read = process.read(&mut buffer)?;
    let output = String::from_utf8_lossy(&buffer[..read]);

    assert!(output.contains("hello from pty"));
    process.signal_group(Signal::SIGTERM)?;
    let _ = process.wait();
    Ok(())
}

#[test]
fn pty_backend_resizes_the_master_fd() {
    let backend = UnixPtyBackend;
    let command = PtyCommand::new("/bin/sh")
        .expect("command")
        .arg("-c")
        .expect("arg")
        .arg("sleep 1")
        .expect("arg");
    let process = backend.spawn(&command, None).expect("spawn pty process");

    process
        .resize(WindowSize::new(40, 100, 0, 0))
        .expect("resize pty");
    process
        .signal_group(Signal::SIGTERM)
        .expect("terminate child");
    let _ = process.wait();
}

#[test]
fn raw_mode_guard_restores_termios_on_drop_even_after_panic() {
    let pty = openpty(None, None).expect("openpty");
    let before = termios::tcgetattr(&pty.slave).expect("termios before");

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _guard = RawModeGuard::new(&pty.slave).expect("enter raw mode");
        panic!("force guard drop");
    }));

    let after = termios::tcgetattr(&pty.slave).expect("termios after");
    assert_eq!(
        before.local_flags - LocalFlags::PENDIN,
        after.local_flags - LocalFlags::PENDIN
    );
}

#[test]
fn raw_mode_guard_changes_the_terminal_while_live() {
    let pty = openpty(None, None).expect("openpty");
    let before = termios::tcgetattr(&pty.slave).expect("termios before");
    let guard = RawModeGuard::new(&pty.slave).expect("enter raw mode");
    let during = termios::tcgetattr(&pty.slave).expect("termios during");

    assert_ne!(
        before.local_flags.contains(LocalFlags::ICANON),
        during.local_flags.contains(LocalFlags::ICANON)
    );
    drop(guard);
}
