//! Master-session orchestration and startup helpers.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Master orchestration is runtime-oriented and not improved by const qualification."
)]

use anyhow::{anyhow, Context, Result};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use scterm_core::{
    AttachRequest, BoundSocket, ClientReady, LogCap, PtyReady, RingBuffer, RingSize, Session,
    SessionPath, WindowSize,
};
use scterm_unix::{
    PtyBackend, PtyCommand, PtyProcess, SocketTransport, UnixPtyBackend, UnixSocketListener,
    UnixSocketStream, UnixSocketTransport,
};

use crate::logging::AppLogger;
use crate::storage::{attached_state, log_path_for_session, set_attached_state, PersistentLog};

/// A passive tap point for PTY output.
pub trait OutputObserver: Send + Sync {
    /// Observes PTY output bytes without mutating delivery behavior.
    fn observe(&self, bytes: &[u8]);
}

/// A no-op output observer.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopOutputObserver;

impl OutputObserver for NoopOutputObserver {
    fn observe(&self, _bytes: &[u8]) {}
}

/// The origin of synthesized or user-provided PTY input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    /// Interactive keystrokes from an attached client.
    User,
    /// Bytes provided by the `push` command.
    Push,
    /// Redraw-triggering synthetic PTY input.
    Redraw,
    /// Reserved future inbound message injection.
    InboundMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueuedInput {
    source: InputSource,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct ClientConnection {
    stream: UnixSocketStream,
}

/// Configuration for a session master.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterConfig {
    ring_size: RingSize,
    log_cap: LogCap,
    app_log_root: PathBuf,
}

impl MasterConfig {
    /// Creates a new master configuration.
    #[must_use]
    pub fn new(ring_size: RingSize, log_cap: LogCap, app_log_root: PathBuf) -> Self {
        Self {
            ring_size,
            log_cap,
            app_log_root,
        }
    }

    /// Returns the configured ring size.
    #[must_use]
    pub fn ring_size(&self) -> RingSize {
        self.ring_size
    }

    /// Returns the configured persistent log cap.
    #[must_use]
    pub fn log_cap(&self) -> LogCap {
        self.log_cap
    }

    /// Returns the structured app log root.
    #[must_use]
    pub fn app_log_root(&self) -> &Path {
        &self.app_log_root
    }
}

impl Default for MasterConfig {
    fn default() -> Self {
        Self {
            ring_size: RingSize::new(128 * 1024).expect("static ring size is valid"),
            log_cap: LogCap::from_bytes(1_048_576),
            app_log_root: std::env::temp_dir().join("scterm-app-logs"),
        }
    }
}

/// Summary of one PTY output handling pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcastSummary {
    delivered_clients: usize,
}

impl BroadcastSummary {
    /// Returns how many clients received the broadcast.
    #[must_use]
    pub fn delivered_clients(self) -> usize {
        self.delivered_clients
    }
}

/// A started session containing the running typestate and master state.
pub struct StartedSession<O = NoopOutputObserver>
where
    O: OutputObserver,
{
    handle: Session<scterm_core::Running>,
    master: MasterSession<O>,
}

impl<O> StartedSession<O>
where
    O: OutputObserver,
{
    /// Returns the running session handle.
    #[must_use]
    pub fn handle(&self) -> &Session<scterm_core::Running> {
        &self.handle
    }

    /// Returns the live master state.
    #[must_use]
    pub fn master(&self) -> &MasterSession<O> {
        &self.master
    }

    /// Consumes the started session and returns the master state.
    #[must_use]
    pub fn into_master(self) -> MasterSession<O> {
        self.master
    }
}

/// A live session master owning the PTY, listener, ring, log, and clients.
pub struct MasterSession<O = NoopOutputObserver>
where
    O: OutputObserver,
{
    path: SessionPath,
    listener: UnixSocketListener,
    pty: PtyProcess,
    ring: RingBuffer,
    log: PersistentLog,
    clients: Vec<ClientConnection>,
    input_queue: VecDeque<QueuedInput>,
    first_attach_seen: bool,
    observer: O,
    logger: AppLogger,
}

impl<O> MasterSession<O>
where
    O: OutputObserver,
{
    /// Creates a new running master session.
    ///
    /// # Errors
    /// Returns an error when the app logger or attached-state metadata cannot be initialized.
    pub fn new(
        path: SessionPath,
        listener: UnixSocketListener,
        pty: PtyProcess,
        config: &MasterConfig,
        observer: O,
    ) -> Result<Self> {
        set_attached_state(path.as_path(), false)?;
        let logger = AppLogger::new(config.app_log_root().to_path_buf())?;
        logger.emit("master", "new", "session master initialized")?;

        Ok(Self {
            log: PersistentLog::open(log_path_for_session(&path), config.log_cap())?,
            path,
            listener,
            pty,
            ring: RingBuffer::new(config.ring_size()),
            clients: Vec::new(),
            input_queue: VecDeque::new(),
            first_attach_seen: false,
            observer,
            logger,
        })
    }

    /// Accepts the next client connection from the listener.
    ///
    /// # Errors
    /// Returns an error when the next client cannot be accepted.
    pub fn accept_client(&self) -> Result<UnixSocketStream> {
        self.listener.accept().context("accept next session client")
    }

    /// Marks a newly connected client as attached and returns any ring replay bytes.
    ///
    /// # Errors
    /// Returns an error when attached-state metadata cannot be updated.
    pub fn attach_client(
        &mut self,
        stream: UnixSocketStream,
        request: AttachRequest,
    ) -> Result<Vec<u8>> {
        let previous_first_attach_seen = self.first_attach_seen;
        let first_client = self.clients.is_empty();

        if first_client {
            set_attached_state(self.path.as_path(), true)?;
        }

        self.first_attach_seen = true;
        if let Err(error) = self.logger.emit("master", "attach", "client attached") {
            if first_client {
                let _ = set_attached_state(self.path.as_path(), false);
            }
            self.first_attach_seen = previous_first_attach_seen;
            return Err(error);
        }

        self.clients.push(ClientConnection { stream });

        if request.skip_ring_replay() {
            Ok(Vec::new())
        } else {
            Ok(self.ring.snapshot())
        }
    }

    /// Detaches the client at `index`.
    ///
    /// # Errors
    /// Returns an error when attached-state metadata cannot be updated.
    pub fn detach_client(&mut self, index: usize) -> Result<()> {
        if index >= self.clients.len() {
            return Err(anyhow!("client index {index} is out of range"));
        }

        self.clients.remove(index);
        if self.clients.is_empty() {
            set_attached_state(self.path.as_path(), false)?;
        }
        self.logger.emit("master", "detach", "client detached")?;
        Ok(())
    }

    /// Enqueues user input for serialized PTY delivery.
    pub fn enqueue_user_input(&mut self, bytes: impl Into<Vec<u8>>) {
        self.enqueue(InputSource::User, bytes);
    }

    /// Enqueues `push` command input for serialized PTY delivery.
    pub fn enqueue_push_input(&mut self, bytes: impl Into<Vec<u8>>) {
        self.enqueue(InputSource::Push, bytes);
    }

    /// Enqueues redraw-triggering input for serialized PTY delivery.
    pub fn enqueue_redraw_input(&mut self, bytes: impl Into<Vec<u8>>) {
        self.enqueue(InputSource::Redraw, bytes);
    }

    /// Enqueues future inbound message input for serialized PTY delivery.
    pub fn enqueue_inbound_message(&mut self, bytes: impl Into<Vec<u8>>) {
        self.enqueue(InputSource::InboundMessage, bytes);
    }

    /// Drains all pending input in FIFO order through the single PTY write path.
    ///
    /// # Errors
    /// Returns an error when writing to the PTY fails.
    pub fn drain_input_queue(&mut self) -> Result<usize> {
        let mut written = 0_usize;

        while let Some(input) = self.input_queue.pop_front() {
            let bytes = input.bytes;
            self.pty
                .write(&bytes)
                .with_context(|| format!("write {:?} input into PTY", input.source))?;
            written += bytes.len();
        }

        Ok(written)
    }

    /// Records PTY output into the ring and log, then broadcasts if attached.
    ///
    /// # Errors
    /// Returns an error when the persistent log append or any client write fails.
    pub fn record_pty_output(&mut self, bytes: &[u8]) -> Result<BroadcastSummary> {
        self.ring.push(bytes);
        self.log.append(bytes)?;
        self.observer.observe(bytes);

        if !self.first_attach_seen {
            return Ok(BroadcastSummary {
                delivered_clients: 0,
            });
        }

        for client in &mut self.clients {
            client
                .stream
                .write(bytes)
                .context("broadcast PTY output to attached client")?;
            client
                .stream
                .flush()
                .context("flush PTY output broadcast")?;
        }

        Ok(BroadcastSummary {
            delivered_clients: self.clients.len(),
        })
    }

    /// Performs cleanup after the PTY child exits.
    ///
    /// # Errors
    /// Returns an error when the end marker or socket cleanup fails.
    pub fn handle_child_exit(&mut self) -> Result<()> {
        self.log.append_end_marker()?;
        let _ = fs::remove_file(self.path.as_path());
        self.logger.emit("master", "exit", "session child exited")?;
        Ok(())
    }

    /// Returns the session socket path.
    #[must_use]
    pub fn path(&self) -> &SessionPath {
        &self.path
    }

    /// Returns whether at least one attach has occurred.
    #[must_use]
    pub fn first_attach_seen(&self) -> bool {
        self.first_attach_seen
    }

    /// Returns the current attached-client count.
    #[must_use]
    pub fn attached_clients(&self) -> usize {
        self.clients.len()
    }

    /// Returns the current attached-state metadata bit.
    ///
    /// # Errors
    /// Returns an error when the socket metadata cannot be read.
    pub fn attached_state(&self) -> Result<bool> {
        attached_state(self.path.as_path())
    }

    /// Returns the persistent session log.
    #[must_use]
    pub fn log(&self) -> &PersistentLog {
        &self.log
    }

    /// Returns the ring snapshot.
    #[must_use]
    pub fn ring_snapshot(&self) -> Vec<u8> {
        self.ring.snapshot()
    }

    fn enqueue(&mut self, source: InputSource, bytes: impl Into<Vec<u8>>) {
        self.input_queue.push_back(QueuedInput {
            source,
            bytes: bytes.into(),
        });
    }
}

/// Launches a running session and constructs the master state around it.
#[derive(Debug, Clone)]
pub struct SessionLauncher {
    config: MasterConfig,
    socket_transport: UnixSocketTransport,
    pty_backend: UnixPtyBackend,
}

impl SessionLauncher {
    /// Creates a new session launcher.
    #[must_use]
    pub fn new(config: MasterConfig) -> Self {
        Self {
            config,
            socket_transport: UnixSocketTransport,
            pty_backend: UnixPtyBackend,
        }
    }

    /// Starts a resolved session by creating the listener, PTY, and master state.
    ///
    /// # Errors
    /// Returns an error when startup readiness fails or the master cannot be initialized.
    pub fn start<O>(
        &self,
        session: Session<scterm_core::Resolved>,
        command: &PtyCommand,
        size: Option<WindowSize>,
        observer: O,
    ) -> Result<StartedSession<O>>
    where
        O: OutputObserver,
    {
        let path = session.path().clone();
        let listener = self
            .socket_transport
            .bind(&path)
            .context("bind session control socket")?;
        let pty = self
            .pty_backend
            .spawn(command, size)
            .context("spawn PTY-backed child process")?;

        let readiness_probe = self
            .socket_transport
            .connect(&path)
            .context("verify session socket readiness")?;
        drop(readiness_probe);

        let bound_socket = BoundSocket::new(&path);
        let pty_ready = PtyReady::new(&path);
        let client_ready = ClientReady::new(&path);
        let handle = session
            .start(bound_socket, pty_ready, client_ready)
            .context("transition resolved session to running")?;
        let master = MasterSession::new(path, listener, pty, &self.config, observer)?;

        Ok(StartedSession { handle, master })
    }
}
