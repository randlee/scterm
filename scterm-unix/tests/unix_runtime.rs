//! Unix integration tests for `scterm-unix`.

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::thread;
use std::time::Duration;

use nix::pty::openpty;
use nix::sys::signal::Signal;
use nix::sys::termios::{self, LocalFlags};
use scterm_core::{SessionPath, WindowSize};
use scterm_unix::{
    PtyBackend, PtyCommand, RawModeGuard, SocketTransport, UnixPtyBackend, UnixSocketTransport,
};
use tempfile::TempDir;

#[test]
fn socket_transport_supports_bind_connect_accept_and_close() {
    let tempdir = TempDir::new().expect("tempdir");
    let path = SessionPath::new(tempdir.path().join("socket")).expect("session path");
    let transport = UnixSocketTransport;
    let listener = transport.bind(&path).expect("bind listener");

    let client_thread = thread::spawn(move || {
        let transport = UnixSocketTransport;
        let mut stream = transport.connect(&path).expect("connect stream");
        stream.write(b"ping").expect("write ping");
    });

    let mut server = listener.accept().expect("accept client");
    let mut buffer = [0_u8; 4];
    let read = server.read(&mut buffer).expect("read from client");

    client_thread.join().expect("join client");
    assert_eq!(&buffer[..read], b"ping");
}

#[test]
fn socket_transport_uses_chdir_indirection_for_long_paths() {
    let tempdir = TempDir::new().expect("tempdir");
    let prefix_len = tempdir.path().as_os_str().as_bytes().len();
    let padding = "x".repeat(108usize.saturating_sub(prefix_len) + 10);
    let long_dir = tempdir.path().join(padding);
    fs::create_dir_all(&long_dir).expect("create long directory");

    let full_path = long_dir.join("s.sock");
    assert!(full_path.as_os_str().as_bytes().len() > 107);

    let session_path = SessionPath::new(full_path.clone()).expect("session path");
    let transport = UnixSocketTransport;
    let listener = transport.bind(&session_path).expect("bind listener");

    let client_thread = thread::spawn(move || {
        let transport = UnixSocketTransport;
        transport.connect(&session_path).expect("connect long path")
    });

    let _server = listener.accept().expect("accept long-path client");
    let _client = client_thread.join().expect("join client");
    assert!(fs::metadata(full_path)
        .expect("socket metadata")
        .file_type()
        .is_socket());
}

#[test]
fn pty_backend_spawns_and_echoes_through_the_master() {
    let backend = UnixPtyBackend;
    let command = PtyCommand::new("/bin/sh")
        .expect("command")
        .arg("-c")
        .expect("arg")
        .arg("cat")
        .expect("arg");
    let process = backend
        .spawn(&command, Some(WindowSize::new(24, 80, 0, 0)))
        .expect("spawn pty process");

    process.write(b"hello from pty\n").expect("write to pty");
    thread::sleep(Duration::from_millis(50));

    let mut buffer = [0_u8; 256];
    let read = process.read(&mut buffer).expect("read from pty");
    let output = String::from_utf8_lossy(&buffer[..read]);

    assert!(output.contains("hello from pty"));
    process
        .signal_group(Signal::SIGTERM)
        .expect("terminate child");
    let _ = process.wait();
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
