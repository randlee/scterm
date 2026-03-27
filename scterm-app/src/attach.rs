//! Attach-client orchestration for log replay, socket connect, and raw mode.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Attach flow helpers are runtime-oriented and not improved by const qualification."
)]

use anyhow::{Context, Result};
use scterm_core::{
    AttachClient, AttachRequest, Connecting, Detached, Live, LogReplaying, Packet, RedrawMethod,
    RedrawRequest, RingReplaying, SessionPath, WindowSize,
};
use scterm_unix::{RawModeGuard, SocketTransport, UnixSocketStream, UnixSocketTransport};
use std::os::fd::AsFd;

use crate::storage::PersistentLog;

/// Drives the attach-client typestate sequence.
#[derive(Debug, Clone)]
pub struct AttachSession {
    path: SessionPath,
    transport: UnixSocketTransport,
}

/// Proof that the on-disk log already covers any would-be ring replay bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogReplayCovered;

impl LogReplayCovered {
    /// Creates a proof when `log_history` already ends with `ring_bytes`, or when
    /// there is no ring replay to deliver.
    #[must_use]
    pub fn from_history_and_ring(log_history: &[u8], ring_bytes: &[u8]) -> Option<Self> {
        if ring_bytes.is_empty() || log_history.ends_with(ring_bytes) {
            Some(Self)
        } else {
            None
        }
    }
}

/// A live attached client with raw-mode restoration on drop.
#[derive(Debug)]
pub struct LiveAttachment<Tty> {
    state: AttachClient<Live>,
    stream: UnixSocketStream,
    raw_mode: RawModeGuard,
    tty: Tty,
}

impl AttachSession {
    /// Creates a new attach-session runner for `path`.
    #[must_use]
    pub fn new(path: SessionPath) -> Self {
        Self {
            path,
            transport: UnixSocketTransport,
        }
    }

    /// Replays the on-disk log before any socket connection occurs.
    ///
    /// # Errors
    /// Returns an error when the persistent log cannot be read.
    pub fn replay_log(&self, log: &PersistentLog) -> Result<(AttachClient<LogReplaying>, Vec<u8>)> {
        let history = log.replay()?;
        Ok((
            AttachClient::<LogReplaying>::new_log_replaying(self.path.clone()),
            history,
        ))
    }

    /// Completes log replay and transitions the attach client into socket connection setup.
    #[must_use]
    pub fn finish_log_replay(&self, state: AttachClient<LogReplaying>) -> AttachClient<Connecting> {
        state.connect()
    }

    /// Connects to the session socket and sends the attach packet.
    ///
    /// # Errors
    /// Returns an error when the socket connect or packet write fails.
    pub fn connect(
        &self,
        state: AttachClient<Connecting>,
        skip_ring_replay: bool,
    ) -> Result<(AttachClient<RingReplaying>, UnixSocketStream)> {
        let mut stream = self.transport.connect(state.path())?;
        let attach = Packet::Attach(AttachRequest::new(skip_ring_replay)).encode();
        stream
            .write(&attach)
            .context("write attach packet to session socket")?;
        stream.flush().context("flush attach packet")?;
        Ok((state.begin_ring_replay(), stream))
    }

    /// Sends the redraw request needed for a live attach.
    ///
    /// # Errors
    /// Returns an error when the redraw packet cannot be written.
    pub fn request_redraw(&self, stream: &mut UnixSocketStream, size: WindowSize) -> Result<()> {
        let redraw = Packet::Redraw(RedrawRequest::new(RedrawMethod::Winch, size)).encode();
        stream
            .write(&redraw)
            .context("write redraw packet to session socket")?;
        stream.flush().context("flush redraw packet")
    }

    /// Enters raw mode and transitions into live streaming after ring replay.
    ///
    /// # Errors
    /// Returns an error when raw mode cannot be installed.
    pub fn go_live_from_ring<Tty>(
        &self,
        state: AttachClient<RingReplaying>,
        stream: UnixSocketStream,
        tty: Tty,
    ) -> Result<LiveAttachment<Tty>>
    where
        Tty: AsFd,
    {
        let raw_mode = RawModeGuard::new(&tty)?;
        Ok(LiveAttachment {
            state: state.go_live(),
            stream,
            raw_mode,
            tty,
        })
    }

    /// Enters raw mode and transitions directly into live streaming.
    ///
    /// # Errors
    /// Returns an error when raw mode cannot be installed.
    pub fn go_live_skip_ring<Tty>(
        &self,
        state: AttachClient<Connecting>,
        coverage: LogReplayCovered,
        stream: UnixSocketStream,
        tty: Tty,
    ) -> Result<LiveAttachment<Tty>>
    where
        Tty: AsFd,
    {
        let _ = coverage;
        let raw_mode = RawModeGuard::new(&tty)?;
        Ok(LiveAttachment {
            state: state.go_live_skip_ring(),
            stream,
            raw_mode,
            tty,
        })
    }
}

impl<Tty> LiveAttachment<Tty>
where
    Tty: AsFd,
{
    /// Returns the live attach-client typestate handle.
    #[must_use]
    pub fn state(&self) -> &AttachClient<Live> {
        &self.state
    }

    /// Returns the raw-mode guard used for the live attachment.
    #[must_use]
    pub fn raw_mode(&self) -> &RawModeGuard {
        &self.raw_mode
    }

    /// Returns mutable access to the connected session stream.
    #[must_use]
    pub fn stream_mut(&mut self) -> &mut UnixSocketStream {
        &mut self.stream
    }

    /// Consumes the live attachment and sends a detach packet.
    ///
    /// # Errors
    /// Returns an error when the detach packet cannot be written.
    pub fn detach(mut self) -> Result<AttachClient<Detached>> {
        let detach = Packet::Detach.encode();
        self.stream
            .write(&detach)
            .context("write detach packet to session socket")?;
        self.stream.flush().context("flush detach packet")?;
        let detached = self.state.detach();
        drop(self.raw_mode);
        let _ = self.tty.as_fd();
        Ok(detached)
    }
}
