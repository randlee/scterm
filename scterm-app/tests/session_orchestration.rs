//! Integration tests for `scterm-app` session orchestration.

use anyhow::{anyhow, Result};
use nix::poll::{poll, PollFd, PollFlags};
use nix::pty::openpty;
use scterm_app::{
    log_path_for_session, AttachSession, MasterConfig, NoopOutputObserver, PersistentLog,
    SessionLauncher,
};
use scterm_core::{AttachRequest, LogCap, RingSize, Session, SessionPath, WindowSize};
use scterm_unix::{PtyCommand, SocketTransport};
use std::os::fd::{BorrowedFd, OwnedFd};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const TEST_TIMEOUT: Duration = Duration::from_secs(5);
const TEST_TIMEOUT_MS: u16 = 5_000;

fn wait_for_readable(fd: BorrowedFd<'_>, label: &str) -> Result<()> {
    let mut poll_fds = [PollFd::new(fd, PollFlags::POLLIN)];
    if poll(&mut poll_fds, TEST_TIMEOUT_MS)? == 0 {
        return Err(anyhow!("timed out waiting for {label}"));
    }
    Ok(())
}

fn recv_with_timeout<T>(receiver: &mpsc::Receiver<Result<T>>, label: &str) -> Result<T> {
    receiver
        .recv_timeout(TEST_TIMEOUT)
        .map_err(|_| anyhow!("timed out waiting for {label}"))?
}

#[test]
fn persistent_log_caps_to_the_latest_bytes() -> Result<()> {
    let tempdir = TempDir::new()?;
    let log = PersistentLog::open(tempdir.path().join("session.log"), LogCap::from_bytes(4))?;

    log.append(b"ab")?;
    log.append(b"cdef")?;

    assert_eq!(log.replay()?, b"cdef");
    Ok(())
}

#[test]
fn master_records_pre_attach_output_without_broadcasting() -> Result<()> {
    let tempdir = TempDir::new()?;
    let path = SessionPath::new(tempdir.path().join("session.sock"))?;
    let launcher = SessionLauncher::new(MasterConfig::new(
        RingSize::new(64)?,
        LogCap::from_bytes(1024),
        tempdir.path().join("app-log"),
    ));
    let command = PtyCommand::new("/bin/sh")?.arg("-c")?.arg("cat")?;
    let started = launcher.start(
        Session::new_resolved(path),
        &command,
        Some(WindowSize::new(24, 80, 0, 0)),
        NoopOutputObserver,
    )?;
    let mut master = started.into_master();

    let summary = master.record_pty_output(b"before-attach")?;

    assert_eq!(summary.delivered_clients(), 0);
    assert_eq!(master.log().replay()?, b"before-attach");
    assert_eq!(master.ring_snapshot(), b"before-attach");
    Ok(())
}

#[test]
fn master_toggles_attached_state_and_replays_the_ring() -> Result<()> {
    let tempdir = TempDir::new()?;
    let path = SessionPath::new(tempdir.path().join("session.sock"))?;
    let launcher = SessionLauncher::new(MasterConfig::new(
        RingSize::new(64)?,
        LogCap::from_bytes(1024),
        tempdir.path().join("app-log"),
    ));
    let command = PtyCommand::new("/bin/sh")?.arg("-c")?.arg("cat")?;
    let started = launcher.start(
        Session::new_resolved(path.clone()),
        &command,
        Some(WindowSize::new(24, 80, 0, 0)),
        NoopOutputObserver,
    )?;
    let mut master = started.into_master();
    master.record_pty_output(b"history")?;

    let (client_tx, client_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let transport = scterm_unix::UnixSocketTransport;
        let result = transport
            .connect(&path)
            .map(|_| ())
            .map_err(anyhow::Error::from);
        let _ = client_tx.send(result);
    });
    wait_for_readable(master.listener_fd(), "session client accept")?;
    let stream = master.accept_client()?;
    let ring = master.attach_client(stream, AttachRequest::new(false))?;
    recv_with_timeout(&client_rx, "session client connect")?;

    assert_eq!(ring, b"history");
    assert!(master.attached_state()?);

    master.detach_client(0)?;
    assert!(!master.attached_state()?);
    Ok(())
}

#[test]
fn attach_session_replays_log_before_connecting_and_sends_attach_packets() -> Result<()> {
    let tempdir = TempDir::new()?;
    let path = SessionPath::new(tempdir.path().join("session.sock"))?;
    let transport = scterm_unix::UnixSocketTransport;
    let listener = transport.bind(&path)?;
    let log = PersistentLog::open(log_path_for_session(&path), LogCap::from_bytes(1024))?;
    log.append(b"log-history")?;
    let attach = AttachSession::new(path);

    let (log_replaying, history) = attach.replay_log(&log)?;
    assert_eq!(history, b"log-history");
    let connecting = attach.finish_log_replay(log_replaying);

    let (server_tx, server_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<[u8; scterm_core::PACKET_SIZE]> {
            wait_for_readable(listener.as_fd(), "attach server accept")?;
            let mut stream = listener.accept()?;
            wait_for_readable(stream.as_fd(), "attach packet read")?;
            let mut bytes = [0_u8; scterm_core::PACKET_SIZE];
            stream.read_exact(&mut bytes)?;
            Ok(bytes)
        })();
        let _ = server_tx.send(result);
    });

    let (_ring, _stream) = attach.connect(connecting, false)?;
    let packet = recv_with_timeout(&server_rx, "attach server packet")?;

    assert_eq!(packet[0], 1);
    Ok(())
}

#[test]
fn live_attachment_uses_raw_mode_and_detaches_cleanly() -> Result<()> {
    let tempdir = TempDir::new()?;
    let path = SessionPath::new(tempdir.path().join("session.sock"))?;
    let transport = scterm_unix::UnixSocketTransport;
    let listener = transport.bind(&path)?;
    let attach = AttachSession::new(path.clone());

    let (server_tx, server_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<[u8; scterm_core::PACKET_SIZE]> {
            wait_for_readable(listener.as_fd(), "detach server accept")?;
            let mut stream = listener.accept()?;
            let mut attach_packet = [0_u8; scterm_core::PACKET_SIZE];
            wait_for_readable(stream.as_fd(), "detach attach packet read")?;
            stream.read_exact(&mut attach_packet)?;
            let mut detach_packet = [0_u8; scterm_core::PACKET_SIZE];
            wait_for_readable(stream.as_fd(), "detach packet read")?;
            stream.read_exact(&mut detach_packet)?;
            Ok(detach_packet)
        })();
        let _ = server_tx.send(result);
    });

    let pty = openpty(None, None)?;
    let slave: OwnedFd = pty.slave;
    let connecting =
        scterm_core::AttachClient::<scterm_core::LogReplaying>::new_log_replaying(path.clone())
            .connect();
    let (ring_replaying, stream) = attach.connect(connecting, false)?;
    let live = attach.go_live_from_ring(ring_replaying, stream, slave)?;
    let detached = live.detach()?;

    assert_eq!(detached.path(), &path);
    assert_eq!(recv_with_timeout(&server_rx, "detach server packet")?[0], 2);
    Ok(())
}

#[test]
fn session_launcher_reports_exec_handshake_failures() -> Result<()> {
    let tempdir = TempDir::new()?;
    let path = SessionPath::new(tempdir.path().join("bad.sock"))?;
    let launcher = SessionLauncher::new(MasterConfig::new(
        RingSize::new(64)?,
        LogCap::from_bytes(1024),
        tempdir.path().join("app-log"),
    ));
    let command = PtyCommand::new("__scterm_no_such_command__")?;

    let Err(error) = launcher.start(
        Session::new_resolved(path),
        &command,
        Some(WindowSize::new(24, 80, 0, 0)),
        NoopOutputObserver,
    ) else {
        panic!("bad command should fail startup");
    };

    assert!(!error.to_string().is_empty());
    assert!(!tempdir.path().join("bad.sock").exists());
    Ok(())
}

#[test]
fn attach_request_can_skip_ring_replay_after_log_history_is_covered() -> Result<()> {
    let tempdir = TempDir::new()?;
    let path = SessionPath::new(tempdir.path().join("session.sock"))?;
    let launcher = SessionLauncher::new(MasterConfig::new(
        RingSize::new(64)?,
        LogCap::from_bytes(1024),
        tempdir.path().join("app-log"),
    ));
    let command = PtyCommand::new("/bin/sh")?.arg("-c")?.arg("cat")?;
    let started = launcher.start(
        Session::new_resolved(path.clone()),
        &command,
        Some(WindowSize::new(24, 80, 0, 0)),
        NoopOutputObserver,
    )?;
    let mut master = started.into_master();
    master.record_pty_output(b"covered-history")?;

    let (client_tx, client_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let transport = scterm_unix::UnixSocketTransport;
        let result = transport
            .connect(&path)
            .map(|_| ())
            .map_err(anyhow::Error::from);
        let _ = client_tx.send(result);
    });
    wait_for_readable(master.listener_fd(), "skip-ring client accept")?;
    let stream = master.accept_client()?;
    let ring = master.attach_client(stream, AttachRequest::new(true))?;
    recv_with_timeout(&client_rx, "skip-ring client connect")?;

    assert!(ring.is_empty(), "ring replay should have been skipped");
    Ok(())
}

#[test]
fn master_serializes_inbound_messages_through_the_pty_queue() -> Result<()> {
    let tempdir = TempDir::new()?;
    let path = SessionPath::new(tempdir.path().join("atm.sock"))?;
    let launcher = SessionLauncher::new(MasterConfig::new(
        RingSize::new(64)?,
        LogCap::from_bytes(1024),
        tempdir.path().join("app-log"),
    ));
    let command = PtyCommand::new("/bin/sh")?.arg("-c")?.arg("cat")?;
    let started = launcher.start(
        Session::new_resolved(path),
        &command,
        Some(WindowSize::new(24, 80, 0, 0)),
        NoopOutputObserver,
    )?;
    let mut master = started.into_master();

    master.enqueue_inbound_message(b"[ATM from arch-term]\nbridge hello\r".to_vec());
    assert!(master.drain_input_queue()? > 0);

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut output = Vec::new();
    while Instant::now() < deadline {
        let mut poll_fds = [PollFd::new(master.pty_fd(), PollFlags::POLLIN)];
        if poll(&mut poll_fds, 100_u16)? == 0 {
            continue;
        }
        let mut buffer = [0_u8; 4096];
        let read = master.read_pty(&mut buffer)?;
        if read == 0 {
            continue;
        }
        output.extend_from_slice(&buffer[..read]);
        if String::from_utf8_lossy(&output).contains("bridge hello") {
            break;
        }
    }

    let text = String::from_utf8_lossy(&output);
    assert!(text.contains("[ATM from arch-term]"), "{text}");
    assert!(text.contains("bridge hello"), "{text}");
    Ok(())
}
