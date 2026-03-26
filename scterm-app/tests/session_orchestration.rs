//! Integration tests for `scterm-app` session orchestration.

use anyhow::Result;
use nix::pty::openpty;
use scterm_app::{
    log_path_for_session, AppLogger, AttachSession, MasterConfig, NoopOutputObserver,
    PersistentLog, SessionLauncher,
};
use scterm_core::{AttachRequest, LogCap, RingSize, Session, SessionPath, WindowSize};
use scterm_unix::{PtyCommand, SocketTransport};
use std::os::fd::OwnedFd;
use tempfile::TempDir;

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

    let client = std::thread::spawn(move || {
        let transport = scterm_unix::UnixSocketTransport;
        transport.connect(&path).expect("connect session client")
    });
    let stream = master.accept_client()?;
    let ring = master.attach_client(stream, AttachRequest::new(false))?;
    let _client = client.join().expect("join client");

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

    let server = std::thread::spawn(move || -> Result<[u8; scterm_core::PACKET_SIZE]> {
        let mut stream = listener.accept()?;
        let mut bytes = [0_u8; scterm_core::PACKET_SIZE];
        let _ = stream.read(&mut bytes)?;
        Ok(bytes)
    });

    let (_ring, _stream) = attach.connect(connecting, false)?;
    let packet = server.join().expect("join attach server")?;

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

    let server = std::thread::spawn(move || -> Result<[u8; scterm_core::PACKET_SIZE]> {
        let mut stream = listener.accept()?;
        let mut attach_packet = [0_u8; scterm_core::PACKET_SIZE];
        let _ = stream.read(&mut attach_packet)?;
        let mut detach_packet = [0_u8; scterm_core::PACKET_SIZE];
        let _ = stream.read(&mut detach_packet)?;
        Ok(detach_packet)
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
    assert_eq!(server.join().expect("join detach server")?[0], 2);
    Ok(())
}

#[test]
fn app_logger_writes_jsonl_events() -> Result<()> {
    let tempdir = TempDir::new()?;
    let logger = AppLogger::new(tempdir.path())?;

    logger.emit("master", "start", "session starting")?;

    let contents = std::fs::read_to_string(logger.path())?;
    assert!(contents.contains("\"target\":\"master\""));
    assert!(contents.contains("\"action\":\"start\""));
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

    let client = std::thread::spawn(move || {
        let transport = scterm_unix::UnixSocketTransport;
        transport.connect(&path).expect("connect session client")
    });
    let stream = master.accept_client()?;
    let ring = master.attach_client(stream, AttachRequest::new(true))?;
    let _client = client.join().expect("join client");

    assert!(ring.is_empty(), "ring replay should have been skipped");
    Ok(())
}
