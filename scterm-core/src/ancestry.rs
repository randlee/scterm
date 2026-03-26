//! Session ancestry helpers for `scterm`.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the ancestry helper contract."
)]

use std::path::{Path, PathBuf};

use crate::{ScError, SessionPath};

/// A parsed colon-delimited ancestry chain.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AncestryChain {
    paths: Vec<SessionPath>,
}

impl AncestryChain {
    /// Creates an empty ancestry chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a colon-delimited ancestry value.
    ///
    /// # Errors
    /// Returns [`ScError`] when any segment is not a valid absolute session
    /// path.
    pub fn parse(value: &str) -> Result<Self, ScError> {
        if value.is_empty() {
            return Ok(Self::new());
        }

        let paths = value
            .split(':')
            .map(|segment| SessionPath::new(PathBuf::from(segment)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { paths })
    }

    /// Returns the number of ancestry segments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Returns whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Returns the ancestry paths in outermost-first order.
    #[must_use]
    pub fn paths(&self) -> &[SessionPath] {
        &self.paths
    }

    /// Returns the innermost session path.
    #[must_use]
    pub fn innermost(&self) -> Option<&SessionPath> {
        self.paths.last()
    }

    /// Appends `path` to the ancestry chain.
    pub fn append(&mut self, path: SessionPath) {
        self.paths.push(path);
    }

    /// Builds the environment value for the ancestry chain.
    #[must_use]
    pub fn build_env_value(&self) -> String {
        self.paths
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Renders the chain for the `current` command.
    #[must_use]
    pub fn render_human(&self) -> String {
        self.paths
            .iter()
            .map(|path| {
                path.file_name()
                    .map_or_else(|| path.as_path().display().to_string(), ToOwned::to_owned)
            })
            .collect::<Vec<_>>()
            .join(" > ")
    }

    /// Returns whether `target` appears exactly in the chain.
    #[must_use]
    pub fn contains_path(&self, target: &SessionPath) -> bool {
        self.paths.iter().any(|path| path == target)
    }

    /// Checks whether `target` would cause a self-attach loop.
    ///
    /// # Errors
    /// Returns [`ScError`] when `target` is already present in the chain.
    pub fn ensure_not_self_attach(&self, target: &SessionPath) -> Result<(), ScError> {
        if self.contains_path(target) {
            return Err(ScError::self_attach_loop(target.as_path()));
        }

        Ok(())
    }
}

/// Derives the ancestry environment variable name from a program path or basename.
///
/// The final path segment of `program` is used when `program` contains path
/// separators. ASCII letters and digits are preserved, all other characters are
/// normalized to `_`, and the `_SESSION` suffix is appended.
///
/// # Examples
/// ```
/// use scterm_core::session_env_var_name;
///
/// assert_eq!(session_env_var_name("scterm"), "SCTERM_SESSION");
/// assert_eq!(session_env_var_name("/opt/bin/ssh2incus-atch"), "SSH2INCUS_ATCH_SESSION");
/// ```
#[must_use]
pub fn session_env_var_name(program: &str) -> String {
    let base = Path::new(program)
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or(program);

    let mut env_name = base
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    env_name.push_str("_SESSION");
    env_name
}

#[cfg(test)]
mod tests {
    use super::{session_env_var_name, AncestryChain};
    use crate::SessionPath;

    #[test]
    fn env_var_name_tracks_binary_basename() {
        assert_eq!(session_env_var_name("scterm"), "SCTERM_SESSION");
        assert_eq!(
            session_env_var_name("/run/scterm/bin/ssh2incus-atch"),
            "SSH2INCUS_ATCH_SESSION"
        );
    }

    #[test]
    fn ancestry_round_trips_and_renders_human_output() {
        let chain = AncestryChain::parse("/s/outer:/s/inner").expect("parse chain");
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.render_human(), "outer > inner");
        assert_eq!(chain.build_env_value(), "/s/outer:/s/inner");
    }

    #[test]
    fn self_attach_detection_requires_exact_path_match() {
        let base = std::env::temp_dir().join("scterm-ancestry-self-attach");
        let chain = AncestryChain::parse(&format!(
            "{}:{}",
            base.join("a").display(),
            base.join("b").display()
        ))
        .expect("parse chain");
        let target = SessionPath::new(base.join("b")).expect("session path");
        let distinct = SessionPath::new(base.join("c")).expect("session path");

        assert!(chain.ensure_not_self_attach(&target).is_err());
        assert!(chain.ensure_not_self_attach(&distinct).is_ok());
    }

    #[test]
    fn append_tracks_the_innermost_session() {
        let mut chain = AncestryChain::new();
        let base = std::env::temp_dir().join("scterm-ancestry-append");
        let outer = SessionPath::new(base.join("outer")).expect("outer path");
        let inner = SessionPath::new(base.join("inner")).expect("inner path");

        chain.append(outer);
        chain.append(inner.clone());

        assert_eq!(chain.innermost(), Some(&inner));
    }
}
