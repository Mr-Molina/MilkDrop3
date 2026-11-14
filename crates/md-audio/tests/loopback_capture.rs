//! Real-device capture through the OS loopback backend. CI machines have no audio
//! output, so these are ignored; run them by hand while audio is playing:
//! `cargo test -p md-audio --test loopback_capture -- --ignored`
//! (on macOS the terminal needs the Screen Recording permission).

use md_audio::{AudioSource, FrameStatus};
use std::time::{Duration, Instant};

/// Pulls frames for up to ~5 s, stopping early once three non-silent frames arrived;
/// returns (data frames, whether any sample was non-zero). Some servers deliver audio
/// in large blocks (e.g. a PulseAudio null sink under a high-latency client), so the
/// test does not assume a frame rate.
fn pull(src: &mut dyn AudioSource) -> (usize, bool) {
    let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
    let (mut data, mut nonzero) = (0, false);
    for _ in 0..200 {
        if data >= 3 && nonzero {
            break;
        }
        match src.next_frame(&mut l, &mut r, Duration::from_millis(25)) {
            FrameStatus::Data => {
                data += 1;
                nonzero |= l.iter().chain(&r).any(|&b| b != 0);
            }
            FrameStatus::Silence => {}
            FrameStatus::Ended => panic!("capture ended"),
        }
    }
    (data, nonzero)
}

#[test]
#[ignore = "needs a real output device with audio playing"]
fn default_loopback_captures_audio() {
    let mut src = md_audio::default_loopback().expect("open the default loopback");
    let (data, nonzero) = pull(&mut src);
    assert!(data >= 3, "only {data} data frames");
    assert!(nonzero, "captured only digital silence");
    let t0 = Instant::now();
    drop(src);
    assert!(
        t0.elapsed() < Duration::from_millis(100),
        "{:?}",
        t0.elapsed()
    );
}

#[test]
#[ignore = "needs a real output device with audio playing"]
fn default_device_from_the_list_opens() {
    let devices = md_audio::list_devices().expect("list devices");
    let default = devices
        .iter()
        .find(|d| d.is_default)
        .expect("a default device");
    let mut src = md_audio::open_device(&default.name).expect("open the default device");
    assert!(pull(&mut src).0 >= 3);
    let missing = md_audio::open_device("md-audio-no-such-device");
    assert!(
        matches!(missing, Err(md_audio::AudioError::DeviceNotFound(_))),
        "{:?}",
        missing.err()
    );
}
