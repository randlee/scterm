//! CLI parsing and command execution for `scterm`.

use scterm_core::LogCap;
use std::time::Duration;

mod args;
mod dispatch;
mod session;

pub use dispatch::run_cli;

pub(super) const EXIT_SUCCESS: i32 = 0;
pub(super) const EXIT_GENERAL: i32 = 1;
pub(super) const EXIT_USAGE: i32 = 2;
pub(super) const EXIT_NOT_FOUND: i32 = 3;
pub(super) const EXIT_STALE: i32 = 4;
pub(super) const EXIT_SELF_ATTACH: i32 = 5;
pub(super) const EXIT_NO_TTY: i32 = 6;
pub(super) const EXIT_EXISTS: i32 = 7;
pub(super) const EXIT_START_TIMEOUT: i32 = 8;
pub(super) const DEFAULT_DETACH_CHAR: u8 = 0x1c;
pub(super) const START_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CliError {
    pub(super) code: i32,
    pub(super) message: String,
    pub(super) show_help: bool,
}

impl CliError {
    pub(super) fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            show_help: false,
        }
    }

    pub(super) fn usage(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_USAGE,
            message: message.into(),
            show_help: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GlobalOptions {
    pub(super) quiet: bool,
    pub(super) log_cap: LogCap,
    pub(super) detach_char: Option<u8>,
    pub(super) atm: bool,
}

impl Default for GlobalOptions {
    fn default() -> Self {
        Self {
            quiet: false,
            log_cap: LogCap::from_bytes(1_048_576),
            detach_char: Some(DEFAULT_DETACH_CHAR),
            atm: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionCommand {
    pub(super) options: GlobalOptions,
    pub(super) session: String,
    pub(super) child_command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Action {
    Help,
    Version,
    List {
        options: GlobalOptions,
    },
    Current,
    Attach(SessionCommand),
    New(SessionCommand),
    Start(SessionCommand),
    Run(SessionCommand),
    Open(SessionCommand),
    Push {
        session: String,
    },
    Kill {
        session: String,
        force: bool,
    },
    Clear {
        session: Option<String>,
        quiet: bool,
    },
    InternalMaster {
        session_path: String,
        log_cap_bytes: u64,
        atm_enabled: bool,
        child_command: Vec<String>,
    },
}
