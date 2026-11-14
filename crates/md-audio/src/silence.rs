//! A source that is always silent.

use crate::{AudioSource, FrameStatus};
use std::time::Duration;

/// Returns an all-zero frame with [`FrameStatus::Silence`] immediately, every call.
#[derive(Debug, Default, Clone, Copy)]
pub struct SilenceSource;

impl AudioSource for SilenceSource {
    fn next_frame(
        &mut self,
        left: &mut [u8; 576],
        right: &mut [u8; 576],
        _timeout: Duration,
    ) -> FrameStatus {
        left.fill(0);
        right.fill(0);
        FrameStatus::Silence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_zero() {
        let (mut l, mut r) = ([7u8; 576], [9u8; 576]);
        let s = SilenceSource.next_frame(&mut l, &mut r, Duration::ZERO);
        assert_eq!(s, FrameStatus::Silence);
        assert!(l.iter().chain(r.iter()).all(|&b| b == 0));
    }
}
