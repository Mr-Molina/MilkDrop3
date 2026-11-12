//! Shared plumbing for the OS loopback backends (PulseAudio, WASAPI, ScreenCaptureKit).
//!
//! Each backend runs its capture on a thread it does not join: [`spawn_capture`] starts
//! that thread, waits (bounded) for the backend to open, and returns a [`RingSource`].
//! The capture side pushes float PCM into a [`CaptureRing`] that keeps the most recent
//! [`RING_FRAMES`] frames; [`RingSource::next_frame`] waits on a condvar for fresh audio
//! (honouring its timeout) and converts the newest samples with [`to_frame`]. Dropping
//! the source sets the stop flag and returns at once: the capture thread notices the
//! flag and releases the OS stream on its own, so a stalled driver cannot hang shutdown.

use crate::{to_frame, AudioError, AudioSource, FrameStatus};
use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Ring capacity in sample frames (one sample per channel).
pub(crate) const RING_FRAMES: usize = 8192;
/// Upper bound for a backend to open its stream before `spawn_capture` gives up.
pub(crate) const OPEN_TIMEOUT: Duration = Duration::from_secs(10);

struct State {
    /// Interleaved samples, newest at the back, at most `RING_FRAMES` frames.
    samples: VecDeque<f32>,
    channels: u16,
    rate: u32,
    fresh: bool,
    stop: bool,
    failed: bool,
}

/// Most recent captured audio, shared between a capture thread and a [`RingSource`].
pub(crate) struct CaptureRing {
    state: Mutex<State>,
    cv: Condvar,
}

impl CaptureRing {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                samples: VecDeque::with_capacity(RING_FRAMES * 2),
                channels: 2,
                rate: 44_100,
                fresh: false,
                stop: false,
                failed: false,
            }),
            cv: Condvar::new(),
        })
    }

    /// Appends interleaved samples in the given format. A format change discards the
    /// older samples. Returns `false` once the ring is stopped (the producer should quit).
    pub(crate) fn push(&self, interleaved: &[f32], channels: u16, rate: u32) -> bool {
        let Ok(mut st) = self.state.lock() else {
            return false;
        };
        if st.stop {
            return false;
        }
        let channels = channels.max(1);
        if st.channels != channels || st.rate != rate {
            st.samples.clear();
            st.channels = channels;
            st.rate = rate;
        }
        let whole = interleaved.len() - interleaved.len() % usize::from(channels);
        if whole == 0 {
            return true;
        }
        st.samples.extend(&interleaved[..whole]);
        let excess = st
            .samples
            .len()
            .saturating_sub(RING_FRAMES * usize::from(channels));
        st.samples.drain(..excess);
        st.fresh = true;
        drop(st);
        self.cv.notify_all();
        true
    }

    /// Marks the capture as failed; `next_frame` then reports `Ended`.
    pub(crate) fn fail(&self) {
        if let Ok(mut st) = self.state.lock() {
            st.failed = true;
        }
        self.cv.notify_all();
    }

    /// Asks the capture side to stop and wakes every waiter.
    pub(crate) fn stop(&self) {
        if let Ok(mut st) = self.state.lock() {
            st.stop = true;
        }
        self.cv.notify_all();
    }

    /// True once [`CaptureRing::stop`] was called (or the lock is poisoned).
    #[cfg_attr(windows, allow(dead_code))] // used by the Linux and macOS backends
    pub(crate) fn is_stopped(&self) -> bool {
        self.state.lock().map_or(true, |st| st.stop)
    }

    /// Blocks the calling (capture-owner) thread until [`CaptureRing::stop`] is called.
    #[cfg_attr(target_os = "linux", allow(dead_code))] // used by the Windows and macOS backends
    pub(crate) fn wait_for_stop(&self) {
        let Ok(mut st) = self.state.lock() else {
            return;
        };
        while !st.stop {
            st = match self.cv.wait(st) {
                Ok(g) => g,
                Err(_) => return,
            };
        }
    }

    fn next_frame(
        &self,
        left: &mut [u8; 576],
        right: &mut [u8; 576],
        timeout: Duration,
    ) -> FrameStatus {
        let now = Instant::now();
        let deadline = now
            .checked_add(timeout)
            .unwrap_or_else(|| now + Duration::from_secs(86_400 * 365));
        let zero = |left: &mut [u8; 576], right: &mut [u8; 576], status| {
            left.fill(0);
            right.fill(0);
            status
        };
        let Ok(mut st) = self.state.lock() else {
            return zero(left, right, FrameStatus::Ended);
        };
        loop {
            if st.fresh {
                st.fresh = false;
                let (channels, rate) = (st.channels, st.rate);
                let want = window_frames(rate) * usize::from(channels);
                let skip = st.samples.len().saturating_sub(want);
                let tail: Vec<f32> = st.samples.iter().skip(skip).copied().collect();
                drop(st);
                to_frame(&tail, channels, rate, left, right);
                return FrameStatus::Data;
            }
            if st.failed {
                return zero(left, right, FrameStatus::Ended);
            }
            let now = Instant::now();
            if now >= deadline {
                return zero(left, right, FrameStatus::Silence);
            }
            st = match self.cv.wait_timeout(st, deadline - now) {
                Ok(g) => g.0,
                Err(_) => return zero(left, right, FrameStatus::Ended),
            };
        }
    }
}

/// Input frames [`to_frame`] reads for one 576-sample window at `rate`: positions
/// `(n-1) - k * rate/44100` for `k` in `0..576`, plus one for interpolation.
fn window_frames(rate: u32) -> usize {
    let span = 575.0 * f64::from(rate) / 44_100.0;
    span.ceil() as usize + 2
}

/// [`AudioSource`] reading a [`CaptureRing`]; dropping it stops the capture thread.
pub(crate) struct RingSource {
    ring: Arc<CaptureRing>,
}

impl AudioSource for RingSource {
    fn next_frame(
        &mut self,
        left: &mut [u8; 576],
        right: &mut [u8; 576],
        timeout: Duration,
    ) -> FrameStatus {
        self.ring.next_frame(left, right, timeout)
    }
}

impl Drop for RingSource {
    /// Signals the capture thread and detaches it instead of joining (see module docs).
    fn drop(&mut self) {
        self.ring.stop();
    }
}

/// Starts a detached capture thread named `name`.
///
/// On that thread `open` creates the OS stream (returning a value that keeps it alive);
/// its result is handed back here, waiting at most `open_timeout`. On success `run`
/// then owns the stream for the rest of the capture; it must return soon after
/// [`CaptureRing::is_stopped`] becomes true (callback-driven backends simply call
/// [`CaptureRing::wait_for_stop`] and drop the stream). On timeout the thread is told to
/// stop and detached, and a `Backend` error is returned.
pub(crate) fn spawn_capture<G, O, R>(
    name: &str,
    open_timeout: Duration,
    open: O,
    run: R,
) -> Result<Box<dyn AudioSource>, AudioError>
where
    O: FnOnce(&Arc<CaptureRing>) -> Result<G, AudioError> + Send + 'static,
    R: FnOnce(G, &CaptureRing) + Send + 'static,
{
    let ring = CaptureRing::new();
    let thread_ring = Arc::clone(&ring);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    // The JoinHandle is dropped: the thread is detached by design (see module docs).
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || match open(&thread_ring) {
            Ok(guard) => {
                if ready_tx.send(Ok(())).is_ok() {
                    run(guard, &thread_ring);
                }
                // Otherwise the caller already gave up: `guard` is dropped here, which
                // must release (and stop) the late-opened stream.
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        })
        .map_err(|e| AudioError::Backend(format!("cannot start the capture thread: {e}")))?;
    match ready_rx.recv_timeout(open_timeout) {
        Ok(Ok(())) => Ok(Box::new(RingSource { ring })),
        Ok(Err(e)) => Err(e),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            ring.stop();
            Err(AudioError::Backend(format!(
                "timed out after {open_timeout:?} opening the audio capture"
            )))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AudioError::Backend(
            "the capture thread exited while opening the stream".into(),
        )),
    }
}

/// Decodes little-endian `f32` PCM into interleaved samples. With `planar`, the bytes
/// hold one contiguous plane per channel (channel 0 first), as non-interleaved
/// CoreMedia buffers do; otherwise they are already interleaved. Trailing bytes that do
/// not form a whole frame are ignored.
#[cfg_attr(windows, allow(dead_code))] // used by the Linux and macOS backends
pub(crate) fn decode_f32_le(bytes: &[u8], channels: u16, planar: bool) -> Vec<f32> {
    let ch = usize::from(channels.max(1));
    let values: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let frames = values.len() / ch;
    if !planar {
        let mut v = values;
        v.truncate(frames * ch);
        return v;
    }
    (0..frames * ch)
        .map(|i| values[(i % ch) * frames + i / ch])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT: Duration = Duration::from_millis(30);

    fn ramp(frames: usize, channels: u16) -> Vec<f32> {
        (0..frames * channels as usize)
            .map(|i| ((i % 251) as f32 / 125.0) - 1.0)
            .collect()
    }

    fn frame(ring: &CaptureRing, timeout: Duration) -> (FrameStatus, [u8; 576], [u8; 576]) {
        let (mut l, mut r) = ([7u8; 576], [7u8; 576]);
        let s = ring.next_frame(&mut l, &mut r, timeout);
        (s, l, r)
    }

    fn expected(samples: &[f32], channels: u16, rate: u32) -> ([u8; 576], [u8; 576]) {
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        to_frame(samples, channels, rate, &mut l, &mut r);
        (l, r)
    }

    #[test]
    fn empty_ring_times_out_with_zeroed_silence() {
        let ring = CaptureRing::new();
        let t0 = Instant::now();
        let (s, l, r) = frame(&ring, WAIT);
        assert_eq!(s, FrameStatus::Silence);
        assert!(t0.elapsed() >= WAIT);
        assert!(t0.elapsed() < Duration::from_secs(1));
        assert_eq!((l, r), ([0; 576], [0; 576]));
    }

    #[test]
    fn fresh_audio_is_delivered_once_from_the_newest_samples() {
        let ring = CaptureRing::new();
        let pcm = ramp(1000, 2);
        assert!(ring.push(&pcm, 2, 44_100));
        let (s, l, r) = frame(&ring, WAIT);
        assert_eq!(s, FrameStatus::Data);
        assert_eq!((l, r), expected(&pcm, 2, 44_100));
        assert_eq!(frame(&ring, WAIT).0, FrameStatus::Silence);
    }

    #[test]
    fn tail_covers_the_resampling_window_at_48k() {
        let ring = CaptureRing::new();
        let pcm = ramp(2000, 2);
        ring.push(&pcm, 2, 48_000);
        let (s, l, r) = frame(&ring, WAIT);
        assert_eq!(s, FrameStatus::Data);
        assert_eq!((l, r), expected(&pcm, 2, 48_000));
    }

    #[test]
    fn format_change_discards_older_samples() {
        let ring = CaptureRing::new();
        ring.push(&[0.9; 4000], 1, 44_100);
        let pcm = ramp(100, 2);
        ring.push(&pcm, 2, 48_000);
        let (_, l, r) = frame(&ring, WAIT);
        assert_eq!((l, r), expected(&pcm, 2, 48_000));
    }

    #[test]
    fn ring_keeps_at_most_ring_frames() {
        let ring = CaptureRing::new();
        for _ in 0..5 {
            ring.push(&ramp(RING_FRAMES, 2), 2, 44_100);
        }
        let len = ring.state.lock().unwrap().samples.len();
        assert_eq!(len, RING_FRAMES * 2);
    }

    #[test]
    fn waiting_reader_wakes_on_push() {
        let ring = CaptureRing::new();
        let producer = Arc::clone(&ring);
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            producer.push(&ramp(600, 2), 2, 44_100);
        });
        let t0 = Instant::now();
        assert_eq!(frame(&ring, Duration::from_secs(5)).0, FrameStatus::Data);
        assert!(t0.elapsed() < Duration::from_secs(2));
        t.join().unwrap();
    }

    #[test]
    fn failure_ends_the_source_and_stop_rejects_pushes() {
        let ring = CaptureRing::new();
        ring.fail();
        let (s, l, r) = frame(&ring, Duration::from_secs(5));
        assert_eq!(s, FrameStatus::Ended);
        assert_eq!((l, r), ([0; 576], [0; 576]));
        let ring = CaptureRing::new();
        assert!(!ring.is_stopped());
        ring.stop();
        assert!(ring.is_stopped());
        assert!(!ring.push(&[0.5; 2], 2, 44_100));
        ring.wait_for_stop();
    }

    #[test]
    fn spawn_capture_reports_open_errors() {
        let r = spawn_capture(
            "t",
            OPEN_TIMEOUT,
            |_| Err::<(), _>(AudioError::DeviceNotFound("x".into())),
            |_, _| {},
        );
        assert!(matches!(r, Err(AudioError::DeviceNotFound(d)) if d == "x"));
    }

    #[test]
    fn spawn_capture_gives_up_on_a_hanging_open() {
        let t0 = Instant::now();
        let r = spawn_capture(
            "t",
            Duration::from_millis(50),
            |ring| {
                while !ring.is_stopped() {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(())
            },
            |_, _| {},
        );
        assert!(matches!(r, Err(AudioError::Backend(_))));
        assert!(t0.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn late_open_drops_its_guard_without_running() {
        struct Guard(mpsc::Sender<&'static str>);
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = self.0.send("guard dropped");
            }
        }
        let (tx, rx) = mpsc::channel();
        let run_tx = tx.clone();
        let r = spawn_capture(
            "t",
            Duration::from_millis(20),
            move |_| {
                std::thread::sleep(Duration::from_millis(100));
                Ok(Guard(tx))
            },
            move |_, _| {
                let _ = run_tx.send("run");
            },
        );
        assert!(matches!(r, Err(AudioError::Backend(_))));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            "guard dropped"
        );
    }

    #[test]
    fn spawned_capture_streams_and_drop_never_blocks() {
        let (done_tx, done_rx) = mpsc::channel();
        let mut src = spawn_capture(
            "t",
            OPEN_TIMEOUT,
            |ring| {
                ring.push(&ramp(600, 2), 2, 44_100);
                Ok(done_tx)
            },
            |done, ring| {
                ring.wait_for_stop();
                // A slow OS teardown must not delay the caller's drop.
                std::thread::sleep(Duration::from_millis(300));
                let _ = done.send(());
            },
        )
        .unwrap();
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        assert_eq!(
            src.next_frame(&mut l, &mut r, Duration::from_secs(5)),
            FrameStatus::Data
        );
        let t0 = Instant::now();
        drop(src);
        assert!(
            t0.elapsed() < Duration::from_millis(100),
            "{:?}",
            t0.elapsed()
        );
        done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    #[test]
    fn decodes_interleaved_and_planar_f32() {
        let bytes: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .chain([0xAA, 0xBB])
            .collect();
        assert_eq!(
            decode_f32_le(&bytes, 2, false),
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
        assert_eq!(
            decode_f32_le(&bytes, 2, true),
            vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]
        );
        assert!(decode_f32_le(&bytes[..4], 2, true).is_empty());
    }
}
