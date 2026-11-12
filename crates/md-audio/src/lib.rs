//! Audio sources: the AudioSource trait, WAV and silence sources, and OS loopback backends.
#![deny(unsafe_code)]

mod error;
mod loopback;
mod resample;
mod silence;
mod source;
mod wav;

pub use error::AudioError;
pub use loopback::{default_loopback, list_devices, open_device, DeviceInfo};
pub use resample::to_frame;
pub use silence::SilenceSource;
pub use source::{AudioSource, FrameStatus};
pub use wav::WavSource;
