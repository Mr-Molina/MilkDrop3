//! OS loopback entry points, dispatched to the per-OS backend: PulseAudio monitor
//! sources on Linux (`linux_pulse`), WASAPI loopback on Windows (`windows_wasapi`) and
//! ScreenCaptureKit system audio on macOS (`macos_sck`). Other platforms get a stub that
//! reports `AudioError::Backend`.

use crate::{AudioError, AudioSource};

/// One selectable capture device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Backend-specific identifier, accepted by [`open_device`].
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// True when this is the system default loopback/monitor device.
    pub is_default: bool,
}

#[cfg(target_os = "linux")]
use crate::linux_pulse as platform;

#[cfg(windows)]
use crate::windows_wasapi as platform;

#[cfg(target_os = "macos")]
use crate::macos_sck as platform;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod platform {
    use super::*;

    fn unavailable<T>() -> Result<T, AudioError> {
        Err(AudioError::Backend(
            "no loopback backend on this platform".into(),
        ))
    }
    pub fn default_loopback() -> Result<Box<dyn AudioSource>, AudioError> {
        unavailable()
    }
    pub fn list_devices() -> Result<Vec<DeviceInfo>, AudioError> {
        unavailable()
    }
    pub fn open_device(_name: &str) -> Result<Box<dyn AudioSource>, AudioError> {
        unavailable()
    }
}

/// Opens the system's default loopback (what-you-hear) capture.
pub fn default_loopback() -> Result<Box<dyn AudioSource>, AudioError> {
    platform::default_loopback()
}

/// Lists capture devices usable with [`open_device`].
pub fn list_devices() -> Result<Vec<DeviceInfo>, AudioError> {
    platform::list_devices()
}

/// Opens a capture device by [`DeviceInfo::name`].
pub fn open_device(name: &str) -> Result<Box<dyn AudioSource>, AudioError> {
    platform::open_device(name)
}

#[cfg(all(test, not(any(target_os = "linux", target_os = "macos", windows))))]
mod tests {
    use super::*;

    #[test]
    fn stubs_report_backend_error() {
        assert!(matches!(list_devices(), Err(AudioError::Backend(_))));
        assert!(matches!(open_device("x"), Err(AudioError::Backend(_))));
        assert!(matches!(default_loopback(), Err(AudioError::Backend(_))));
    }
}
