//! Error types for `scterm-core`.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the public error API."
)]

use std::backtrace::Backtrace;
use std::error::Error as StdError;
use std::fmt;
use std::path::Path;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScErrorKind {
    SessionNotFound,
    StaleSocket,
    SelfAttachLoop,
    NoTty,
    InvalidName,
    InvalidPath,
    InvalidValue,
    LogCapParse,
    InvalidPacket,
}

/// A typed domain error produced by `scterm-core`.
#[derive(Debug)]
pub struct ScError {
    kind: ScErrorKind,
    message: &'static str,
    input: Option<Box<str>>,
    path: Option<Box<Path>>,
    source_error: Option<Box<dyn StdError + Send + Sync + 'static>>,
    backtrace: Box<Backtrace>,
}

impl fmt::Display for ScError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl StdError for ScError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source_error
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}

impl ScError {
    /// Creates a session-not-found error for `path`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    /// use std::path::Path;
    ///
    /// let error = ScError::session_not_found(Path::new("/run/scterm/demo"));
    /// assert!(error.is_session_not_found());
    /// ```
    #[must_use]
    pub fn session_not_found(path: &Path) -> Self {
        Self::with_path(ScErrorKind::SessionNotFound, "session was not found", path)
    }

    /// Creates a stale-socket error for `path`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    /// use std::path::Path;
    ///
    /// let error = ScError::stale_socket(Path::new("/run/scterm/demo"));
    /// assert!(error.is_stale_socket());
    /// ```
    #[must_use]
    pub fn stale_socket(path: &Path) -> Self {
        Self::with_path(ScErrorKind::StaleSocket, "session socket is stale", path)
    }

    /// Creates a self-attach-loop error for `path`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    /// use std::path::Path;
    ///
    /// let error = ScError::self_attach_loop(Path::new("/run/scterm/demo"));
    /// assert!(error.is_self_attach_loop());
    /// ```
    #[must_use]
    pub fn self_attach_loop(path: &Path) -> Self {
        Self::with_path(
            ScErrorKind::SelfAttachLoop,
            "session cannot attach to itself",
            path,
        )
    }

    /// Creates a no-tty error.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    ///
    /// let error = ScError::no_tty();
    /// assert!(error.is_no_tty());
    /// ```
    #[must_use]
    pub fn no_tty() -> Self {
        Self::new(ScErrorKind::NoTty, "operation requires a TTY")
    }

    /// Creates an invalid-name error for `input`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    ///
    /// let error = ScError::invalid_name("bad/name");
    /// assert!(error.is_invalid_name());
    /// ```
    #[must_use]
    pub fn invalid_name(input: impl Into<String>) -> Self {
        Self::with_input(ScErrorKind::InvalidName, "session name is invalid", input)
    }

    /// Creates an invalid-path error for `path`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    /// use std::path::Path;
    ///
    /// let error = ScError::invalid_path(Path::new("relative"));
    /// assert!(error.is_invalid_path());
    /// ```
    #[must_use]
    pub fn invalid_path(path: &Path) -> Self {
        Self::with_path(ScErrorKind::InvalidPath, "session path is invalid", path)
    }

    /// Creates an invalid-value error for `input`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    ///
    /// let error = ScError::invalid_value("ring size must be non-zero");
    /// assert!(error.is_invalid_value());
    /// ```
    #[must_use]
    pub fn invalid_value(input: impl Into<String>) -> Self {
        Self::with_input(ScErrorKind::InvalidValue, "value is invalid", input)
    }

    /// Creates a log-cap-parse error for `input`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    ///
    /// let error = ScError::log_cap_parse("5g");
    /// assert!(error.is_log_cap_parse());
    /// ```
    #[must_use]
    pub fn log_cap_parse(input: impl Into<String>) -> Self {
        Self::with_input(
            ScErrorKind::LogCapParse,
            "log cap could not be parsed",
            input,
        )
    }

    /// Creates an invalid-packet error with contextual `input`.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    ///
    /// let error = ScError::invalid_packet("unknown packet type");
    /// assert!(error.is_invalid_packet());
    /// ```
    #[must_use]
    pub fn invalid_packet(input: impl Into<String>) -> Self {
        Self::with_input(ScErrorKind::InvalidPacket, "packet is invalid", input)
    }

    /// Creates an invalid-packet error with a source error.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::ScError;
    /// use std::io;
    ///
    /// let error = ScError::invalid_packet_with_source("bad packet", io::Error::other("decode"));
    /// assert!(error.is_invalid_packet());
    /// ```
    #[must_use]
    pub fn invalid_packet_with_source(
        input: impl Into<String>,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self {
            source_error: Some(Box::new(source)),
            ..Self::invalid_packet(input)
        }
    }

    /// Returns the human-readable error message.
    #[must_use]
    pub fn message(&self) -> &'static str {
        self.message
    }

    /// Returns the captured input context when present.
    #[must_use]
    pub fn input(&self) -> Option<&str> {
        self.input.as_deref()
    }

    /// Returns the captured path context when present.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the captured backtrace.
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    /// Returns whether the error represents a missing session.
    #[must_use]
    pub fn is_session_not_found(&self) -> bool {
        self.kind == ScErrorKind::SessionNotFound
    }

    /// Returns whether the error represents a stale socket.
    #[must_use]
    pub fn is_stale_socket(&self) -> bool {
        self.kind == ScErrorKind::StaleSocket
    }

    /// Returns whether the error represents a self-attach loop.
    #[must_use]
    pub fn is_self_attach_loop(&self) -> bool {
        self.kind == ScErrorKind::SelfAttachLoop
    }

    /// Returns whether the error represents a missing TTY.
    #[must_use]
    pub fn is_no_tty(&self) -> bool {
        self.kind == ScErrorKind::NoTty
    }

    /// Returns whether the error represents an invalid session name.
    #[must_use]
    pub fn is_invalid_name(&self) -> bool {
        self.kind == ScErrorKind::InvalidName
    }

    /// Returns whether the error represents an invalid path.
    #[must_use]
    pub fn is_invalid_path(&self) -> bool {
        self.kind == ScErrorKind::InvalidPath
    }

    /// Returns whether the error represents an invalid value.
    #[must_use]
    pub fn is_invalid_value(&self) -> bool {
        self.kind == ScErrorKind::InvalidValue
    }

    /// Returns whether the error represents a log-cap parse failure.
    #[must_use]
    pub fn is_log_cap_parse(&self) -> bool {
        self.kind == ScErrorKind::LogCapParse
    }

    /// Returns whether the error represents an invalid packet.
    #[must_use]
    pub fn is_invalid_packet(&self) -> bool {
        self.kind == ScErrorKind::InvalidPacket
    }

    fn new(kind: ScErrorKind, message: &'static str) -> Self {
        Self {
            kind,
            message,
            input: None,
            path: None,
            source_error: None,
            backtrace: Box::new(Backtrace::capture()),
        }
    }

    fn with_input(kind: ScErrorKind, message: &'static str, input: impl Into<String>) -> Self {
        Self {
            input: Some(input.into().into_boxed_str()),
            ..Self::new(kind, message)
        }
    }

    fn with_path(kind: ScErrorKind, message: &'static str, path: &Path) -> Self {
        Self {
            path: Some(path.to_path_buf().into_boxed_path()),
            ..Self::new(kind, message)
        }
    }
}

impl fmt::Display for ScErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::SessionNotFound => "session-not-found",
            Self::StaleSocket => "stale-socket",
            Self::SelfAttachLoop => "self-attach-loop",
            Self::NoTty => "no-tty",
            Self::InvalidName => "invalid-name",
            Self::InvalidPath => "invalid-path",
            Self::InvalidValue => "invalid-value",
            Self::LogCapParse => "log-cap-parse",
            Self::InvalidPacket => "invalid-packet",
        };
        formatter.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::ScError;
    use std::path::Path;

    #[test]
    fn helper_predicates_match_their_constructor() {
        let missing = ScError::session_not_found(Path::new("/run/scterm/missing"));
        assert!(missing.is_session_not_found());
        assert_eq!(missing.path(), Some(Path::new("/run/scterm/missing")));

        let stale = ScError::stale_socket(Path::new("/run/scterm/stale"));
        assert!(stale.is_stale_socket());

        let self_attach = ScError::self_attach_loop(Path::new("/run/scterm/loop"));
        assert!(self_attach.is_self_attach_loop());

        let no_tty = ScError::no_tty();
        assert!(no_tty.is_no_tty());

        let invalid_name = ScError::invalid_name("bad/name");
        assert!(invalid_name.is_invalid_name());
        assert_eq!(invalid_name.input(), Some("bad/name"));

        let invalid_path = ScError::invalid_path(Path::new("relative/path"));
        assert!(invalid_path.is_invalid_path());

        let invalid_value = ScError::invalid_value("bad");
        assert!(invalid_value.is_invalid_value());

        let log_cap = ScError::log_cap_parse("bogus");
        assert!(log_cap.is_log_cap_parse());
    }

    #[test]
    fn invalid_packet_helper_captures_input() {
        let error = ScError::invalid_packet("unknown packet type 9");
        assert!(error.is_invalid_packet());
        assert_eq!(error.input(), Some("unknown packet type 9"));
    }
}
