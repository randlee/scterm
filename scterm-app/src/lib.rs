//! Application orchestration for `scterm`.
//!
//! This crate owns session startup, master-session behavior, attach flow,
//! persistent history policy, and structured logging integration.

pub mod cli;

mod atm;
mod attach;
mod logging;
mod master;
mod storage;

#[doc(inline)]
pub use attach::{AttachSession, LiveAttachment};
#[doc(inline)]
pub use cli::run_cli;
#[doc(inline)]
pub use logging::AppLogger;
#[doc(inline)]
pub use master::{
    BroadcastSummary, InputSource, MasterConfig, MasterSession, NoopOutputObserver, OutputObserver,
    SessionLauncher, StartedSession,
};
#[doc(inline)]
pub use storage::{attached_state, log_path_for_session, PersistentLog};
