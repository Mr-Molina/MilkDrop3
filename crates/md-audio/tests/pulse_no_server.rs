//! Own test binary: mutates the process environment before any thread starts.
#![cfg(target_os = "linux")]

use md_audio::AudioError;
use std::time::{Duration, Instant};

#[test]
fn pulse_without_server_returns_no_server() {
    let dir = std::env::temp_dir().join(format!("md-audio-pulse-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let conf = dir.join("client.conf");
    std::fs::write(&conf, "autospawn = no\n").unwrap();
    // This is the only test in the binary, so no other thread reads the environment yet.
    std::env::set_var("PULSE_SERVER", "unix:/nonexistent");
    std::env::set_var("PULSE_CLIENTCONFIG", &conf);

    let t0 = Instant::now();
    let r = md_audio::default_loopback();
    assert!(matches!(r, Err(AudioError::NoServer)), "{:?}", r.err());
    assert!(matches!(
        md_audio::list_devices(),
        Err(AudioError::NoServer)
    ));
    assert!(matches!(
        md_audio::open_device("x.monitor"),
        Err(AudioError::NoServer)
    ));
    assert!(t0.elapsed() < Duration::from_secs(10), "{:?}", t0.elapsed());
    let _ = std::fs::remove_dir_all(&dir);
}
