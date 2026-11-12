use md_audio::{AudioSource, FrameStatus, SilenceSource, TimeoutSource};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Never produces: sleeps for the timeout it is given, then reports Silence.
struct BlockingSource {
    _token: Arc<()>,
}

impl AudioSource for BlockingSource {
    fn next_frame(&mut self, l: &mut [u8; 576], r: &mut [u8; 576], t: Duration) -> FrameStatus {
        std::thread::sleep(t);
        l.fill(0);
        r.fill(0);
        FrameStatus::Silence
    }
}

#[test]
fn source_timeout_yields_silence_frame() {
    let mut s = TimeoutSource::new(BlockingSource {
        _token: Arc::new(()),
    });
    let (mut l, mut r) = ([9u8; 576], [9u8; 576]);
    let t0 = Instant::now();
    let st = s.next_frame(&mut l, &mut r, Duration::from_millis(20));
    assert_eq!(st, FrameStatus::Silence);
    assert!(
        t0.elapsed() < Duration::from_millis(200),
        "{:?}",
        t0.elapsed()
    );
    assert!(l.iter().chain(r.iter()).all(|&b| b == 0));
}

/// Produces one Data frame after 100 ms, then never again.
struct LateSource {
    sent: bool,
}

impl AudioSource for LateSource {
    fn next_frame(&mut self, l: &mut [u8; 576], r: &mut [u8; 576], t: Duration) -> FrameStatus {
        if self.sent {
            std::thread::sleep(t);
            return FrameStatus::Silence;
        }
        std::thread::sleep(Duration::from_millis(100));
        self.sent = true;
        l.fill(11);
        r.fill(22);
        FrameStatus::Data
    }
}

#[test]
fn late_frame_is_delivered_by_next_call_not_dropped() {
    let mut s = TimeoutSource::new(LateSource { sent: false });
    let (mut l, mut r) = ([9u8; 576], [9u8; 576]);
    assert_eq!(
        s.next_frame(&mut l, &mut r, Duration::from_millis(10)),
        FrameStatus::Silence
    );
    assert!(l.iter().all(|&b| b == 0));
    assert_eq!(
        s.next_frame(&mut l, &mut r, Duration::from_secs(5)),
        FrameStatus::Data
    );
    assert!(l.iter().all(|&b| b == 11) && r.iter().all(|&b| b == 22));
}

#[test]
fn drop_joins_worker_thread() {
    let token = Arc::new(());
    let s = TimeoutSource::new(BlockingSource {
        _token: Arc::clone(&token),
    });
    assert_eq!(Arc::strong_count(&token), 2);
    drop(s);
    // The inner source (and its token) lived on the worker thread and is gone.
    assert_eq!(Arc::strong_count(&token), 1);
}

struct Counting(Arc<AtomicUsize>);

impl AudioSource for Counting {
    fn next_frame(&mut self, l: &mut [u8; 576], r: &mut [u8; 576], t: Duration) -> FrameStatus {
        self.0.fetch_add(1, Ordering::SeqCst);
        SilenceSource.next_frame(l, r, t)
    }
}

#[test]
fn does_not_busy_wait_on_instant_inner() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut s = TimeoutSource::new(Counting(Arc::clone(&calls)));
    let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
    s.next_frame(&mut l, &mut r, Duration::from_secs(1));
    std::thread::sleep(Duration::from_millis(150));
    // One consumed frame plus at most one prefetched.
    assert!(
        calls.load(Ordering::SeqCst) <= 3,
        "{}",
        calls.load(Ordering::SeqCst)
    );
}

struct EndsAfterOne(bool);

impl AudioSource for EndsAfterOne {
    fn next_frame(&mut self, l: &mut [u8; 576], r: &mut [u8; 576], _t: Duration) -> FrameStatus {
        l.fill(1);
        r.fill(1);
        if self.0 {
            l.fill(0);
            r.fill(0);
            FrameStatus::Ended
        } else {
            self.0 = true;
            FrameStatus::Data
        }
    }
}

#[test]
fn ended_is_sticky() {
    let mut s = TimeoutSource::new(EndsAfterOne(false));
    let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
    let t = Duration::from_secs(5);
    assert_eq!(s.next_frame(&mut l, &mut r, t), FrameStatus::Data);
    assert_eq!(s.next_frame(&mut l, &mut r, t), FrameStatus::Ended);
    assert_eq!(s.next_frame(&mut l, &mut r, t), FrameStatus::Ended);
}
