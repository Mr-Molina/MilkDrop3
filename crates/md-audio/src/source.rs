//! The `AudioSource` trait and the `TimeoutSource` wrapper.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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

/// How long the worker thread lets the inner source block per call. Bounds how long
/// `Drop` can take, provided the inner source honors its `timeout` argument.
const POLL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy)]
struct Frame {
    status: FrameStatus,
    left: [u8; 576],
    right: [u8; 576],
}

struct Shared {
    slot: Mutex<Slot>,
    cv: Condvar,
}

struct Slot {
    frame: Option<Frame>,
    stop: bool,
}

/// Wraps a source so `next_frame` never blocks past its timeout.
///
/// One worker thread owns the inner source and keeps exactly one frame prefetched in a
/// single slot; it sleeps on a condvar while the slot is full (no busy-waiting, no
/// unbounded backlog). If the caller's timeout expires first, `next_frame` returns
/// [`FrameStatus::Silence`] with both buffers zeroed. A frame that arrives late is not
/// dropped: it stays in the slot and is delivered by the next call. After the inner
/// source reports `Ended` (or panics: the panic is caught, logged with `eprintln!`, and
/// treated as `Ended`), every call returns `Ended`. Dropping the wrapper stops and
/// joins the worker, so no thread leaks; the inner source must honor the timeout it is
/// given (the worker passes 20 ms) for the join to be prompt.
pub struct TimeoutSource {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl TimeoutSource {
    /// Starts the worker thread around `inner`.
    pub fn new<S: AudioSource + 'static>(inner: S) -> Self {
        let shared = Arc::new(Shared {
            slot: Mutex::new(Slot {
                frame: None,
                stop: false,
            }),
            cv: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = std::thread::Builder::new()
            .name("md-audio-timeout".into())
            .spawn(move || worker_loop(inner, &worker_shared))
            .expect("spawn audio timeout worker");
        Self {
            shared,
            worker: Some(worker),
        }
    }
}

fn worker_loop<S: AudioSource>(mut inner: S, shared: &Shared) {
    let mut left = [0u8; 576];
    let mut right = [0u8; 576];
    loop {
        {
            let mut slot = shared.slot.lock().unwrap();
            while slot.frame.is_some() && !slot.stop {
                slot = shared.cv.wait(slot).unwrap();
            }
            if slot.stop {
                return;
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            inner.next_frame(&mut left, &mut right, POLL)
        }));
        let status = match result {
            Ok(status) => status,
            Err(_) => {
                // Surface the failure as a sticky Ended instead of silent Silence forever.
                eprintln!("md-audio: inner audio source panicked; treating it as ended");
                left.fill(0);
                right.fill(0);
                FrameStatus::Ended
            }
        };
        let mut slot = shared.slot.lock().unwrap();
        if slot.stop {
            return;
        }
        slot.frame = Some(Frame {
            status,
            left,
            right,
        });
        shared.cv.notify_all();
        if status == FrameStatus::Ended {
            return;
        }
    }
}

impl AudioSource for TimeoutSource {
    fn next_frame(
        &mut self,
        left: &mut [u8; 576],
        right: &mut [u8; 576],
        timeout: Duration,
    ) -> FrameStatus {
        let now0 = Instant::now();
        // Saturate huge timeouts (e.g. Duration::MAX) to a far-future deadline.
        let deadline = now0
            .checked_add(timeout)
            .unwrap_or_else(|| now0 + Duration::from_secs(86_400 * 365));
        let mut slot = self.shared.slot.lock().unwrap();
        loop {
            if let Some(f) = slot.frame.take() {
                left.copy_from_slice(&f.left);
                right.copy_from_slice(&f.right);
                if f.status == FrameStatus::Ended {
                    // Keep reporting Ended on later calls.
                    slot.frame = Some(f);
                } else {
                    self.shared.cv.notify_all();
                }
                return f.status;
            }
            let now = Instant::now();
            if now >= deadline {
                left.fill(0);
                right.fill(0);
                return FrameStatus::Silence;
            }
            slot = self.shared.cv.wait_timeout(slot, deadline - now).unwrap().0;
        }
    }
}

impl Drop for TimeoutSource {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.shared.slot.lock() {
            slot.stop = true;
        }
        self.shared.cv.notify_all();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
