//! Validated domain newtypes for `scterm-core`.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the validated newtype API contract."
)]

use std::fmt;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::ScError;

/// A validated logical session name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionName(String);

impl SessionName {
    /// Creates a validated session name.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::SessionName;
    ///
    /// let name = SessionName::new("demo-1")?;
    /// assert_eq!(name.as_str(), "demo-1");
    /// # Ok::<(), scterm_core::ScError>(())
    /// ```
    ///
    /// # Errors
    /// Returns [`ScError`] when `value` is empty, contains `/`, or uses
    /// characters outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, ScError> {
        let value = value.into();

        if value.is_empty()
            || value.contains('/')
            || !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        {
            return Err(ScError::invalid_name(value));
        }

        Ok(Self(value))
    }

    /// Returns the underlying session name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SessionName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SessionName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SessionName {
    type Err = ScError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// A validated absolute session socket path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionPath(PathBuf);

impl SessionPath {
    /// Creates a validated absolute session path.
    ///
    /// # Examples
    /// ```
    /// use scterm_core::SessionPath;
    ///
    /// let path = SessionPath::new("/tmp/demo.sock")?;
    /// assert_eq!(path.as_path().to_str(), Some("/tmp/demo.sock"));
    /// # Ok::<(), scterm_core::ScError>(())
    /// ```
    ///
    /// # Errors
    /// Returns [`ScError`] when `path` is empty or not absolute.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, ScError> {
        let path = path.into();

        if path.as_os_str().is_empty() || !path.is_absolute() {
            return Err(ScError::invalid_path(&path));
        }

        Ok(Self(path))
    }

    /// Returns the underlying path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Returns the final path segment when one exists.
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        self.0.file_name().and_then(std::ffi::OsStr::to_str)
    }
}

impl AsRef<Path> for SessionPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl fmt::Display for SessionPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0.display().to_string())
    }
}

/// A validated persistent log size limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogCap(u64);

impl LogCap {
    /// Creates a log cap from an exact byte count.
    ///
    /// Any `u64` value is valid. A value of `0` disables persistent logging.
    #[must_use]
    pub fn from_bytes(bytes: u64) -> Self {
        Self(bytes)
    }

    /// Creates a disabled log cap.
    #[must_use]
    pub fn disabled() -> Self {
        Self(0)
    }

    /// Parses a human-readable log cap.
    ///
    /// # Errors
    /// Returns [`ScError`] when `value` is empty, malformed, or uses an
    /// unsupported suffix.
    pub fn parse(value: &str) -> Result<Self, ScError> {
        if value.is_empty() {
            return Err(ScError::log_cap_parse(value));
        }

        let (digits, multiplier) = match value.as_bytes().last().copied() {
            Some(b'k' | b'K') => (&value[..value.len() - 1], 1_024_u64),
            Some(b'm' | b'M') => (&value[..value.len() - 1], 1_048_576_u64),
            Some(last) if last.is_ascii_digit() => (value, 1_u64),
            _ => return Err(ScError::log_cap_parse(value)),
        };

        if digits.is_empty() {
            return Err(ScError::log_cap_parse(value));
        }

        let parsed = digits
            .parse::<u64>()
            .map_err(|_| ScError::log_cap_parse(value))?;
        let bytes = parsed
            .checked_mul(multiplier)
            .ok_or_else(|| ScError::log_cap_parse(value))?;

        Ok(Self(bytes))
    }

    /// Returns whether logging is disabled.
    #[must_use]
    pub fn is_disabled(self) -> bool {
        self.0 == 0
    }

    /// Returns the size limit in bytes.
    #[must_use]
    pub fn bytes(self) -> u64 {
        self.0
    }
}

impl fmt::Display for LogCap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl FromStr for LogCap {
    type Err = ScError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// A validated in-memory ring-buffer size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RingSize(NonZeroUsize);

impl RingSize {
    /// Creates a ring size from `value`.
    ///
    /// # Errors
    /// Returns [`ScError`] when `value` is zero.
    pub fn new(value: usize) -> Result<Self, ScError> {
        NonZeroUsize::new(value)
            .map(Self)
            .ok_or_else(|| ScError::invalid_value("ring size must be non-zero"))
    }

    /// Returns the ring size in bytes.
    #[must_use]
    pub fn get(self) -> usize {
        self.0.get()
    }
}

impl fmt::Display for RingSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.get())
    }
}

#[cfg(test)]
mod tests {
    use super::{LogCap, RingSize, SessionName, SessionPath};
    use std::path::Path;

    #[test]
    fn session_name_rejects_invalid_inputs() {
        assert!(SessionName::new("").is_err());
        assert!(SessionName::new("bad/name").is_err());
        assert!(SessionName::new("white space").is_err());
        assert!(SessionName::new("valid-name_1.2").is_ok());
    }

    #[test]
    fn session_path_must_be_absolute() {
        assert!(SessionPath::new("relative/path").is_err());
        assert!(SessionPath::new(Path::new("/tmp/session")).is_ok());
    }

    #[test]
    fn log_cap_parses_suffixes_and_zero() {
        assert_eq!(LogCap::parse("0").expect("zero log cap").bytes(), 0);
        assert_eq!(LogCap::parse("128k").expect("128k").bytes(), 131_072);
        assert_eq!(LogCap::parse("4M").expect("4M").bytes(), 4_194_304);
        assert!(LogCap::parse("5g").is_err());
    }

    #[test]
    fn ring_size_must_be_non_zero() {
        assert!(RingSize::new(0).is_err());
        assert_eq!(RingSize::new(128).expect("ring size").get(), 128);
    }
}
