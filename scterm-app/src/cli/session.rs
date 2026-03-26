use super::{CliError, EXIT_GENERAL, EXIT_NOT_FOUND, EXIT_NO_TTY, EXIT_SELF_ATTACH, EXIT_STALE};
use anyhow::Result;
use scterm_core::{session_env_var_name, AncestryChain, SessionPath, WindowSize};
use scterm_unix::{SocketTransport, UnixError, UnixSocketTransport};
use std::env;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

pub(super) fn current_ancestry_chain(program: &str) -> Result<AncestryChain> {
    let env_name = session_env_var_name(program);
    match env::var(&env_name) {
        Ok(value) => AncestryChain::parse(&value).map_err(anyhow::Error::from),
        Err(env::VarError::NotPresent) => Ok(AncestryChain::new()),
        Err(error) => Err(anyhow::Error::new(error)),
    }
}

pub(super) fn ensure_not_self_attach(program: &str, path: &SessionPath) -> Result<(), CliError> {
    current_ancestry_chain(program)
        .map_err(runtime_error)?
        .ensure_not_self_attach(path)
        .map_err(domain_error)
}

pub(super) fn require_tty(program: &str) -> Result<(), CliError> {
    if io::stdin().is_terminal() {
        Ok(())
    } else {
        Err(CliError::new(
            EXIT_NO_TTY,
            format!("attaching to a session requires a terminal ({program})"),
        ))
    }
}

pub(super) fn current_session_path(program: &str) -> Result<SessionPath, CliError> {
    current_ancestry_chain(program)
        .map_err(runtime_error)?
        .innermost()
        .cloned()
        .ok_or_else(|| CliError::usage("No session was specified."))
}

pub(super) fn resolve_session_path(program: &str, session: &str) -> Result<SessionPath, CliError> {
    let path = if session.contains('/') {
        let path = PathBuf::from(session);
        if path.is_absolute() {
            path
        } else {
            env::current_dir()
                .map_err(|error| CliError::new(EXIT_GENERAL, error.to_string()))?
                .join(path)
        }
    } else {
        default_session_dir(program).join(session)
    };
    SessionPath::new(path).map_err(domain_error)
}

pub(super) fn default_session_dir(program: &str) -> PathBuf {
    if let Some(home) = usable_home_dir() {
        return home.join(".cache").join(binary_name(program));
    }
    std::env::temp_dir().join(format!(
        ".{}-{}",
        binary_name(program),
        nix::unistd::geteuid()
    ))
}

fn usable_home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty() && home != "/")
        .map(PathBuf::from)
}

pub(super) fn app_log_root_for(path: &SessionPath) -> PathBuf {
    path.as_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".scterm-logs")
}

pub(super) fn session_label(path: &SessionPath) -> String {
    path.file_name()
        .map_or_else(|| path.to_string(), ToOwned::to_owned)
}

fn binary_name(program: &str) -> String {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionState {
    Absent,
    Live,
    Stale,
}

pub(super) fn session_state(path: &SessionPath) -> Result<SessionState, CliError> {
    let transport = UnixSocketTransport;
    match transport.connect(path) {
        Ok(_) => Ok(SessionState::Live),
        Err(error) if error.is_absent_session() => Ok(SessionState::Absent),
        Err(error) if error.is_stale_socket() => Ok(SessionState::Stale),
        Err(error) => Err(unix_error(error)),
    }
}

pub(super) fn current_window_size() -> WindowSize {
    WindowSize::new(24, 80, 0, 0)
}

pub(super) fn domain_error(error: impl Into<anyhow::Error>) -> CliError {
    let error = error.into();
    let Some(error) = error.downcast_ref::<scterm_core::ScError>() else {
        return CliError::new(EXIT_GENERAL, error.to_string());
    };
    if error.is_session_not_found() {
        CliError::new(EXIT_NOT_FOUND, error.to_string())
    } else if error.is_stale_socket() {
        CliError::new(EXIT_STALE, error.to_string())
    } else if error.is_self_attach_loop() {
        CliError::new(EXIT_SELF_ATTACH, error.to_string())
    } else if error.is_no_tty() {
        CliError::new(EXIT_NO_TTY, error.to_string())
    } else {
        CliError::new(EXIT_GENERAL, error.to_string())
    }
}

pub(super) fn unix_error(error: UnixError) -> CliError {
    match error {
        UnixError::AbsentSession { path, .. } => CliError::new(
            EXIT_NOT_FOUND,
            format!("session '{}' does not exist", path.display()),
        ),
        UnixError::StaleSocket { path, .. } => CliError::new(
            EXIT_STALE,
            format!("session '{}' is not running", path.display()),
        ),
        other => CliError::new(EXIT_GENERAL, other.to_string()),
    }
}

pub(super) fn runtime_error(error: impl std::fmt::Display) -> CliError {
    CliError::new(EXIT_GENERAL, error.to_string())
}
