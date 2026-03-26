//! Client-to-master packet types for `scterm`.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification is not part of the packet API contract."
)]

use std::fmt;

use crate::ScError;

/// The fixed payload size of a client-to-master packet.
pub const WINDOW_SIZE_BYTES: usize = 8;

/// The serialized size of a client-to-master packet.
pub const PACKET_SIZE: usize = 2 + WINDOW_SIZE_BYTES;

const MSG_PUSH: u8 = 0;
const MSG_ATTACH: u8 = 1;
const MSG_DETACH: u8 = 2;
const MSG_WINCH: u8 = 3;
const MSG_REDRAW: u8 = 4;
const MSG_KILL: u8 = 5;

/// A terminal window size payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowSize {
    rows: u16,
    cols: u16,
    xpixel: u16,
    ypixel: u16,
}

impl WindowSize {
    /// Creates a new window size.
    #[must_use]
    pub fn new(rows: u16, cols: u16, xpixel: u16, ypixel: u16) -> Self {
        Self {
            rows,
            cols,
            xpixel,
            ypixel,
        }
    }

    /// Returns the row count.
    #[must_use]
    pub fn rows(self) -> u16 {
        self.rows
    }

    /// Returns the column count.
    #[must_use]
    pub fn cols(self) -> u16 {
        self.cols
    }

    /// Returns the horizontal pixel count.
    #[must_use]
    pub fn xpixel(self) -> u16 {
        self.xpixel
    }

    /// Returns the vertical pixel count.
    #[must_use]
    pub fn ypixel(self) -> u16 {
        self.ypixel
    }

    #[must_use]
    fn to_payload(self) -> [u8; WINDOW_SIZE_BYTES] {
        let mut payload = [0_u8; WINDOW_SIZE_BYTES];
        payload[0..2].copy_from_slice(&self.rows.to_ne_bytes());
        payload[2..4].copy_from_slice(&self.cols.to_ne_bytes());
        payload[4..6].copy_from_slice(&self.xpixel.to_ne_bytes());
        payload[6..8].copy_from_slice(&self.ypixel.to_ne_bytes());
        payload
    }

    #[must_use]
    fn from_payload(payload: [u8; WINDOW_SIZE_BYTES]) -> Self {
        Self {
            rows: u16::from_ne_bytes([payload[0], payload[1]]),
            cols: u16::from_ne_bytes([payload[2], payload[3]]),
            xpixel: u16::from_ne_bytes([payload[4], payload[5]]),
            ypixel: u16::from_ne_bytes([payload[6], payload[7]]),
        }
    }
}

/// A validated push payload.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PushData {
    len: u8,
    payload: [u8; WINDOW_SIZE_BYTES],
}

impl PushData {
    /// Creates a validated push payload.
    ///
    /// # Errors
    /// Returns [`ScError`] when `data` is longer than eight bytes.
    pub fn new(data: &[u8]) -> Result<Self, ScError> {
        let len = u8::try_from(data.len())
            .map_err(|_| ScError::invalid_packet("push length overflow"))?;
        if usize::from(len) > WINDOW_SIZE_BYTES {
            return Err(ScError::invalid_packet("push payload exceeds 8 bytes"));
        }

        let mut payload = [0_u8; WINDOW_SIZE_BYTES];
        payload[..usize::from(len)].copy_from_slice(data);

        Ok(Self { len, payload })
    }

    /// Returns the payload bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.payload[..usize::from(self.len)]
    }
}

/// An attach request packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttachRequest {
    skip_ring_replay: bool,
}

impl AttachRequest {
    /// Creates an attach request.
    #[must_use]
    pub fn new(skip_ring_replay: bool) -> Self {
        Self { skip_ring_replay }
    }

    /// Returns whether ring replay should be skipped.
    #[must_use]
    pub fn skip_ring_replay(self) -> bool {
        self.skip_ring_replay
    }
}

/// A redraw request packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RedrawRequest {
    method: RedrawMethod,
    size: WindowSize,
}

impl RedrawRequest {
    /// Creates a redraw request.
    #[must_use]
    pub fn new(method: RedrawMethod, size: WindowSize) -> Self {
        Self { method, size }
    }

    /// Returns the redraw method.
    #[must_use]
    pub fn method(self) -> RedrawMethod {
        self.method
    }

    /// Returns the window size payload.
    #[must_use]
    pub fn size(self) -> WindowSize {
        self.size
    }
}

/// A kill request packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KillRequest {
    signal: u8,
}

impl KillRequest {
    /// Creates a kill request.
    #[must_use]
    pub fn new(signal: u8) -> Self {
        Self { signal }
    }

    /// Returns the requested signal byte.
    #[must_use]
    pub fn signal(self) -> u8 {
        self.signal
    }
}

/// A redraw method selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RedrawMethod {
    /// Auto-select redraw behavior.
    Unspecified = 0,
    /// Do not force a redraw.
    None = 1,
    /// Trigger redraw by writing `Ctrl-L`.
    CtrlL = 2,
    /// Trigger redraw by replaying `SIGWINCH`.
    Winch = 3,
}

impl TryFrom<u8> for RedrawMethod {
    type Error = ScError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unspecified),
            1 => Ok(Self::None),
            2 => Ok(Self::CtrlL),
            3 => Ok(Self::Winch),
            _ => Err(ScError::invalid_packet(format!(
                "invalid redraw method {value}"
            ))),
        }
    }
}

impl From<RedrawMethod> for u8 {
    fn from(value: RedrawMethod) -> Self {
        value as Self
    }
}

/// A clear method selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClearMethod {
    /// Auto-select clear behavior.
    Unspecified = 0,
    /// Do not clear the terminal.
    None = 1,
    /// Move to the screen bottom before attaching.
    Move = 2,
}

impl TryFrom<u8> for ClearMethod {
    type Error = ScError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unspecified),
            1 => Ok(Self::None),
            2 => Ok(Self::Move),
            _ => Err(ScError::invalid_packet(format!(
                "invalid clear method {value}"
            ))),
        }
    }
}

impl From<ClearMethod> for u8 {
    fn from(value: ClearMethod) -> Self {
        value as Self
    }
}

/// A client-to-master packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    /// Writes raw bytes into the PTY.
    Push(PushData),
    /// Attaches a client to the master.
    Attach(AttachRequest),
    /// Detaches a client from the master.
    Detach,
    /// Forwards a terminal size change.
    Winch(WindowSize),
    /// Requests a redraw.
    Redraw(RedrawRequest),
    /// Requests that the master deliver a signal.
    Kill(KillRequest),
}

impl Packet {
    /// Serializes the packet into the fixed 10-byte wire format.
    #[must_use]
    pub fn encode(&self) -> [u8; PACKET_SIZE] {
        let mut bytes = [0_u8; PACKET_SIZE];

        match self {
            Self::Push(data) => {
                bytes[0] = MSG_PUSH;
                bytes[1] = data.len;
                bytes[2..].copy_from_slice(&data.payload);
            }
            Self::Attach(request) => {
                bytes[0] = MSG_ATTACH;
                bytes[1] = u8::from(request.skip_ring_replay);
            }
            Self::Detach => {
                bytes[0] = MSG_DETACH;
            }
            Self::Winch(size) => {
                bytes[0] = MSG_WINCH;
                bytes[2..].copy_from_slice(&size.to_payload());
            }
            Self::Redraw(request) => {
                bytes[0] = MSG_REDRAW;
                bytes[1] = u8::from(request.method);
                bytes[2..].copy_from_slice(&request.size.to_payload());
            }
            Self::Kill(request) => {
                bytes[0] = MSG_KILL;
                bytes[1] = request.signal;
            }
        }

        bytes
    }

    /// Deserializes a packet from the fixed 10-byte wire format.
    ///
    /// # Errors
    /// Returns [`ScError`] when the type byte or packet payload is invalid.
    pub fn decode(bytes: [u8; PACKET_SIZE]) -> Result<Self, ScError> {
        let mut payload = [0_u8; WINDOW_SIZE_BYTES];
        payload.copy_from_slice(&bytes[2..]);

        match bytes[0] {
            MSG_PUSH => Ok(Self::Push(PushData {
                len: validate_push_len(bytes[1])?,
                payload,
            })),
            MSG_ATTACH => Ok(Self::Attach(AttachRequest::new(bytes[1] != 0))),
            MSG_DETACH => Ok(Self::Detach),
            MSG_WINCH => Ok(Self::Winch(WindowSize::from_payload(payload))),
            MSG_REDRAW => Ok(Self::Redraw(RedrawRequest::new(
                RedrawMethod::try_from(bytes[1])?,
                WindowSize::from_payload(payload),
            ))),
            MSG_KILL => Ok(Self::Kill(KillRequest::new(bytes[1]))),
            kind => Err(ScError::invalid_packet(format!(
                "unknown packet type {kind}"
            ))),
        }
    }
}

fn validate_push_len(len: u8) -> Result<u8, ScError> {
    if usize::from(len) > WINDOW_SIZE_BYTES {
        return Err(ScError::invalid_packet(format!(
            "push length {len} exceeds {WINDOW_SIZE_BYTES}"
        )));
    }

    Ok(len)
}

impl fmt::Display for Packet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Push(data) => write!(formatter, "Push({} bytes)", data.as_slice().len()),
            Self::Attach(request) => write!(
                formatter,
                "Attach(skip_ring_replay={})",
                request.skip_ring_replay()
            ),
            Self::Detach => formatter.write_str("Detach"),
            Self::Winch(size) => write!(formatter, "Winch({}x{})", size.rows(), size.cols()),
            Self::Redraw(request) => write!(
                formatter,
                "Redraw(method={:?}, {}x{})",
                request.method(),
                request.size().rows(),
                request.size().cols()
            ),
            Self::Kill(request) => write!(formatter, "Kill(signal={})", request.signal()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttachRequest, ClearMethod, KillRequest, Packet, PushData, RedrawMethod, RedrawRequest,
        WindowSize, PACKET_SIZE,
    };

    #[test]
    fn packet_round_trip_covers_all_variants() {
        let size = WindowSize::new(24, 80, 0, 0);
        let packets = [
            Packet::Push(PushData::new(b"hello").expect("push payload")),
            Packet::Attach(AttachRequest::new(true)),
            Packet::Detach,
            Packet::Winch(size),
            Packet::Redraw(RedrawRequest::new(RedrawMethod::Winch, size)),
            Packet::Kill(KillRequest::new(15)),
        ];

        for packet in packets {
            let encoded = packet.encode();
            assert_eq!(encoded.len(), PACKET_SIZE);
            let decoded = Packet::decode(encoded).expect("decode packet");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn packet_rejects_unknown_type_and_invalid_push_len() {
        let mut bytes = [0_u8; PACKET_SIZE];
        bytes[0] = 9;
        assert!(Packet::decode(bytes).is_err());

        let mut bytes = [0_u8; PACKET_SIZE];
        bytes[0] = 0;
        bytes[1] = 9;
        assert!(Packet::decode(bytes).is_err());
    }

    #[test]
    fn enum_value_mappings_match_the_protocol() {
        assert_eq!(u8::from(RedrawMethod::Unspecified), 0);
        assert_eq!(u8::from(RedrawMethod::None), 1);
        assert_eq!(u8::from(RedrawMethod::CtrlL), 2);
        assert_eq!(u8::from(RedrawMethod::Winch), 3);

        assert_eq!(u8::from(ClearMethod::Unspecified), 0);
        assert_eq!(u8::from(ClearMethod::None), 1);
        assert_eq!(u8::from(ClearMethod::Move), 2);
    }
}
