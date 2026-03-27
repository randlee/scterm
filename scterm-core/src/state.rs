//! Coarse typestate markers for session and attach lifecycles.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the typestate API contract."
)]

use std::marker::PhantomData;

use crate::SessionPath;

/// A resolved session that has not yet entered the running state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Resolved;

/// A session whose runtime readiness checks have completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Running;

/// A session whose socket path is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Stale;

/// An attach client replaying log history from disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LogReplaying;

/// An attach client connecting to the session socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Connecting;

/// An attach client replaying in-memory ring history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RingReplaying;

/// An attach client streaming live PTY output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Live;

/// An attach client that has detached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Detached;

/// A typestated session handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Session<S> {
    path: SessionPath,
    _state: PhantomData<S>,
}

impl Session<Resolved> {
    /// Creates a resolved session handle for `path`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::{Resolved, Session, SessionPath};
    ///
    /// let path = SessionPath::new("/tmp/demo.sock")?;
    /// let session = Session::<Resolved>::new_resolved(path.clone());
    /// assert_eq!(session.path(), &path);
    /// # Ok::<(), scterm_core::ScError>(())
    /// ```
    #[must_use]
    pub fn new_resolved(path: SessionPath) -> Self {
        Self {
            path,
            _state: PhantomData,
        }
    }

    /// Transitions a resolved session into the running state.
    ///
    /// Phase 1 exposes the consuming transition contract only. Later phases
    /// wire this method to the runtime startup path.
    ///
    /// # Errors
    /// Returns [`crate::ScError`] when the runtime startup path fails.
    pub fn start(self) -> Result<Session<Running>, crate::ScError> {
        Ok(Session {
            path: self.path,
            _state: PhantomData,
        })
    }

    /// Checks whether the resolved session path has gone stale.
    ///
    /// # Errors
    /// Returns `Err(Session<Stale>)` when the path resolves to a stale session.
    pub fn check_stale(self) -> Result<Self, Session<Stale>> {
        Ok(self)
    }
}

impl Session<Stale> {
    /// Recovers a stale session handle back into the resolved state.
    ///
    /// # Errors
    /// Returns [`crate::ScError`] when stale-session recovery fails.
    pub fn recover(self) -> Result<Session<Resolved>, crate::ScError> {
        Ok(Session {
            path: self.path,
            _state: PhantomData,
        })
    }
}

impl<S> Session<S> {
    /// Returns the session path.
    #[must_use]
    pub fn path(&self) -> &SessionPath {
        &self.path
    }

    /// Consumes the handle and returns the session path.
    #[must_use]
    pub fn into_path(self) -> SessionPath {
        self.path
    }
}

/// A typestated attach-client handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttachClient<S> {
    path: SessionPath,
    _state: PhantomData<S>,
}

impl AttachClient<LogReplaying> {
    /// Creates a new attach-client handle at the start of log replay.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::{AttachClient, LogReplaying, SessionPath};
    ///
    /// let path = SessionPath::new("/tmp/demo.sock")?;
    /// let client = AttachClient::<LogReplaying>::new_log_replaying(path.clone());
    /// assert_eq!(client.path(), &path);
    /// # Ok::<(), scterm_core::ScError>(())
    /// ```
    #[must_use]
    pub fn new_log_replaying(path: SessionPath) -> Self {
        Self {
            path,
            _state: PhantomData,
        }
    }

    /// Transitions from log replay into socket connection setup.
    #[must_use]
    pub fn connect(self) -> AttachClient<Connecting> {
        AttachClient {
            path: self.path,
            _state: PhantomData,
        }
    }
}

impl AttachClient<Connecting> {
    /// Transitions from socket connection into in-memory ring replay.
    #[must_use]
    pub fn begin_ring_replay(self) -> AttachClient<RingReplaying> {
        AttachClient {
            path: self.path,
            _state: PhantomData,
        }
    }

    /// Transitions directly into live streaming when ring replay is skipped.
    #[must_use]
    pub fn go_live(self) -> AttachClient<Live> {
        AttachClient {
            path: self.path,
            _state: PhantomData,
        }
    }
}

impl AttachClient<RingReplaying> {
    /// Transitions from ring replay into live streaming.
    #[must_use]
    pub fn go_live(self) -> AttachClient<Live> {
        AttachClient {
            path: self.path,
            _state: PhantomData,
        }
    }
}

impl AttachClient<Live> {
    /// Transitions a live client into the detached terminal state.
    #[must_use]
    pub fn detach(self) -> AttachClient<Detached> {
        AttachClient {
            path: self.path,
            _state: PhantomData,
        }
    }
}

impl<S> AttachClient<S> {
    /// Returns the target session path.
    #[must_use]
    pub fn path(&self) -> &SessionPath {
        &self.path
    }

    /// Consumes the handle and returns the session path.
    #[must_use]
    pub fn into_path(self) -> SessionPath {
        self.path
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttachClient, Connecting, Detached, Live, LogReplaying, Resolved, RingReplaying, Running,
        Session, Stale,
    };
    use crate::SessionPath;
    use std::marker::PhantomData;

    #[test]
    fn session_typestate_wraps_a_validated_path() {
        let path = SessionPath::new("/tmp/session").expect("session path");
        let session = Session::<Resolved>::new_resolved(path.clone());

        assert_eq!(session.path(), &path);
        assert_eq!(session.into_path(), path);
    }

    #[test]
    fn attach_client_typestate_wraps_a_validated_path() {
        let path = SessionPath::new("/tmp/session").expect("session path");
        let client = AttachClient::<LogReplaying>::new_log_replaying(path.clone());

        assert_eq!(client.path(), &path);
        assert_eq!(client.into_path(), path);
    }

    #[test]
    fn session_transitions_consume_and_preserve_the_path() {
        let path = SessionPath::new("/tmp/session").expect("session path");
        let running = Session::<Resolved>::new_resolved(path.clone())
            .start()
            .expect("start transition");
        let running_path = running.into_path();

        assert_eq!(running_path, path);

        let resolved = Session::<Resolved>::new_resolved(running_path.clone())
            .check_stale()
            .expect("resolved transition");
        assert_eq!(resolved.into_path(), running_path);

        let recovered = Session::<Stale> {
            path: path.clone(),
            _state: PhantomData,
        }
        .recover()
        .expect("recover transition");
        assert_eq!(recovered.into_path(), path);
    }

    #[test]
    fn attach_client_transitions_consume_and_preserve_the_path() {
        let path = SessionPath::new("/tmp/session").expect("session path");

        let live_from_ring = AttachClient::<LogReplaying>::new_log_replaying(path.clone())
            .connect()
            .begin_ring_replay()
            .go_live();
        let detached = live_from_ring.detach();
        assert_eq!(detached.into_path().as_path(), path.as_path());

        let live_direct = AttachClient::<Connecting> {
            path: path.clone(),
            _state: PhantomData,
        }
        .go_live();
        assert_eq!(live_direct.into_path(), path);
    }

    #[test]
    fn typestate_markers_remain_zero_sized() {
        assert_eq!(std::mem::size_of::<Resolved>(), 0);
        assert_eq!(std::mem::size_of::<Running>(), 0);
        assert_eq!(std::mem::size_of::<Stale>(), 0);
        assert_eq!(std::mem::size_of::<LogReplaying>(), 0);
        assert_eq!(std::mem::size_of::<Connecting>(), 0);
        assert_eq!(std::mem::size_of::<RingReplaying>(), 0);
        assert_eq!(std::mem::size_of::<Live>(), 0);
        assert_eq!(std::mem::size_of::<Detached>(), 0);
    }
}
