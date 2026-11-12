//! The `AudioSource` trait and the `TimeoutSource` wrapper.

use std::time::Duration;

/// Result of one `next_frame` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus {
    /// Both buffers hold a fresh frame.
    Data,
    /// No audio was available; both buffers are zeroed.
    Silence,
    /// The source is finished; both buffers are zeroed.
    Ended,
}

/// A producer of 576-sample signed-8-bit-pattern frames (silence is 0).
pub trait AudioSource: Send {
    /// Fills `left`/`right`, waiting at most about `timeout` for audio.
    fn next_frame(
        &mut self,
        left: &mut [u8; 576],
        right: &mut [u8; 576],
        timeout: Duration,
    ) -> FrameStatus;
}

impl AudioSource for Box<dyn AudioSource> {
    fn next_frame(
        &mut self,
        left: &mut [u8; 576],
        right: &mut [u8; 576],
        timeout: Duration,
    ) -> FrameStatus {
        (**self).next_frame(left, right, timeout)
    }
}
