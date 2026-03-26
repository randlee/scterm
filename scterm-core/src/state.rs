//! Coarse typestate markers for session and attach lifecycles.

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
    #[must_use]
    pub fn new(path: SessionPath) -> Self {
        Self {
            path,
            _state: PhantomData,
        }
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
    #[must_use]
    pub fn new(path: SessionPath) -> Self {
        Self {
            path,
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
    use super::{AttachClient, LogReplaying, Resolved, Session};
    use crate::SessionPath;

    #[test]
    fn session_typestate_wraps_a_validated_path() {
        let path = SessionPath::new("/tmp/session").expect("session path");
        let session = Session::<Resolved>::new(path.clone());

        assert_eq!(session.path(), &path);
        assert_eq!(session.into_path(), path);
    }

    #[test]
    fn attach_client_typestate_wraps_a_validated_path() {
        let path = SessionPath::new("/tmp/session").expect("session path");
        let client = AttachClient::<LogReplaying>::new(path.clone());

        assert_eq!(client.path(), &path);
        assert_eq!(client.into_path(), path);
    }
}
