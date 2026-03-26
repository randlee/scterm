//! Core domain types and validation for `scterm`.
//!
//! This crate contains the runtime-agnostic building blocks used by the
//! higher-level `scterm` crates.

mod ancestry;
mod error;
mod packet;
mod ring;
mod state;
mod types;

#[doc(inline)]
pub use ancestry::{session_env_var_name, AncestryChain};
#[doc(inline)]
pub use error::ScError;
#[doc(inline)]
pub use packet::{
    AttachRequest, ClearMethod, KillRequest, Packet, PushData, RedrawMethod, RedrawRequest,
    WindowSize, PACKET_SIZE, WINDOW_SIZE_BYTES,
};
#[doc(inline)]
pub use ring::RingBuffer;
#[doc(inline)]
pub use state::{
    AttachClient, BoundSocket, ClientReady, Connecting, Detached, Live, LogReplaying, PtyReady,
    Resolved, RingReplaying, Running, Session, Stale,
};
#[doc(inline)]
pub use types::{LogCap, RingSize, SessionName, SessionPath};
