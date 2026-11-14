//! Audio sources: the AudioSource trait, WAV and silence sources, and OS loopback backends.
#![deny(unsafe_code)]

mod backend_errors;
#[cfg_attr(
    not(any(target_os = "linux", target_os = "macos", windows)),
    allow(dead_code)
)]
mod capture_ring;
mod error;
#[cfg(target_os = "linux")]
mod linux_pulse;
mod loopback;
#[cfg(target_os = "macos")]
mod macos_sck;
mod resample;
mod silence;
mod source;
mod wav;
#[cfg(windows)]
mod windows_wasapi;

pub use error::AudioError;
pub use loopback::{default_loopback, list_devices, open_device, DeviceInfo};
pub use resample::to_frame;
pub use silence::SilenceSource;
pub use source::{AudioSource, FrameStatus, TimeoutSource};
pub use wav::WavSource;
