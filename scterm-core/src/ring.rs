//! Ring-buffer support for persistent session catch-up.
#![allow(
    clippy::missing_const_for_fn,
    reason = "Const qualification does not materially improve the ring-buffer API."
)]

use std::collections::VecDeque;

use crate::RingSize;

/// A bounded in-memory FIFO byte buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingBuffer {
    capacity: RingSize,
    bytes: VecDeque<u8>,
}

impl RingBuffer {
    /// Creates an empty ring buffer with `capacity`.
    #[must_use]
    pub fn new(capacity: RingSize) -> Self {
        Self {
            capacity,
            bytes: VecDeque::with_capacity(capacity.get()),
        }
    }

    /// Returns the configured byte capacity.
    #[must_use]
    pub fn capacity(&self) -> RingSize {
        self.capacity
    }

    /// Returns the current length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the ring buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Appends `data`, dropping the oldest bytes when the ring overflows.
    pub fn push(&mut self, data: &[u8]) {
        let capacity = self.capacity.get();

        if data.len() >= capacity {
            self.bytes.clear();
            self.bytes
                .extend(data[data.len() - capacity..].iter().copied());
            return;
        }

        let overflow = self.bytes.len() + data.len();
        if overflow > capacity {
            let to_drop = overflow - capacity;
            self.bytes.drain(..to_drop);
        }

        self.bytes.extend(data.iter().copied());
    }

    /// Returns a snapshot of the buffered bytes.
    #[must_use]
    pub fn snapshot(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }

    /// Drains the buffer contents in FIFO order.
    pub fn drain(&mut self) -> Vec<u8> {
        self.bytes.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;
    use crate::RingSize;

    #[test]
    fn push_and_drain_preserve_fifo_order() {
        let mut ring = RingBuffer::new(RingSize::new(8).expect("ring size"));
        ring.push(b"abc");
        ring.push(b"def");

        assert_eq!(ring.snapshot(), b"abcdef");
        assert_eq!(ring.drain(), b"abcdef");
        assert!(ring.is_empty());
    }

    #[test]
    fn overflow_discards_the_oldest_bytes() {
        let mut ring = RingBuffer::new(RingSize::new(5).expect("ring size"));
        ring.push(b"abc");
        ring.push(b"def");

        assert_eq!(ring.snapshot(), b"bcdef");
    }

    #[test]
    fn oversized_push_keeps_the_tail_only() {
        let mut ring = RingBuffer::new(RingSize::new(4).expect("ring size"));
        ring.push(b"012345");

        assert_eq!(ring.snapshot(), b"2345");
    }
}
