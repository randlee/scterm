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
    pub fn replay_log(&self, log: &PersistentLog) -> Result<(AttachClient<Connecting>, Vec<u8>)> {
        let history = log.replay()?;
        let connecting =
            AttachClient::<LogReplaying>::new_log_replaying(self.path.clone()).connect();
        Ok((connecting, history))
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
    pub fn go_live<Tty>(
        &self,
        state: AttachClient<Connecting>,
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
