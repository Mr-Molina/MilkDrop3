//! Pure mappings from OS backend failures to [`AudioError`].
//!
//! They are compiled on every OS so `backend_errors_are_mapped` exercises them in CI on
//! Linux too; only the thin conversions from OS error types are platform-gated.
// Each OS build uses only its own backend's mapping; the tests use all of them.
#![cfg_attr(not(test), allow(dead_code))]

use crate::AudioError;

/// Where a macOS user grants the permission ScreenCaptureKit audio capture needs.
pub(crate) const SCREEN_RECORDING_SETTING: &str =
    "System Settings > Privacy & Security > Screen Recording";

/// `SCStreamErrorDomain`, the NSError domain of ScreenCaptureKit failures.
pub(crate) const SCK_ERROR_DOMAIN: &str = "com.apple.ScreenCaptureKit.SCStreamErrorDomain";

/// Maps a ScreenCaptureKit `NSError` (domain, code, localized description).
pub(crate) fn map_sck_error(domain: &str, code: isize, description: &str) -> AudioError {
    if domain != SCK_ERROR_DOMAIN {
        return AudioError::Backend(format!("{domain} error {code}: {description}"));
    }
    match code {
        // SCStreamErrorUserDeclined, SCStreamErrorMissingEntitlements
        -3801 | -3803 => AudioError::PermissionDenied(format!(
            "macOS refused system-audio capture ({description}); allow this app under \
             {SCREEN_RECORDING_SETTING}, then restart it"
        )),
        // SCStreamErrorNoDisplayList, SCStreamErrorNoCaptureSource
        -3814 | -3815 => AudioError::DeviceNotFound(format!(
            "system audio: ScreenCaptureKit found nothing to capture ({description})"
        )),
        _ => AudioError::Backend(format!("ScreenCaptureKit error {code}: {description}")),
    }
}

/// The WASAPI failure classes md-audio distinguishes (from cpal's `ErrorKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WasapiFailure {
    DeviceNotAvailable,
    PermissionDenied,
    HostUnavailable,
    DeviceBusy,
    UnsupportedConfig,
    Other,
}

/// Maps a WASAPI failure on `device` with cpal's message `detail`.
pub(crate) fn map_wasapi_failure(kind: WasapiFailure, detail: &str, device: &str) -> AudioError {
    match kind {
        WasapiFailure::DeviceNotAvailable => AudioError::DeviceNotFound(device.to_string()),
        WasapiFailure::PermissionDenied => AudioError::PermissionDenied(format!(
            "Windows denied loopback capture of '{device}': {detail}"
        )),
        WasapiFailure::HostUnavailable => AudioError::NoServer,
        WasapiFailure::DeviceBusy => AudioError::Backend(format!(
            "'{device}' is busy (another application may hold it in exclusive mode): {detail}"
        )),
        WasapiFailure::UnsupportedConfig => AudioError::Backend(format!(
            "'{device}' uses an unsupported sample format: {detail}"
        )),
        WasapiFailure::Other => {
            AudioError::Backend(format!("WASAPI error on '{device}': {detail}"))
        }
    }
}

#[cfg(windows)]
impl From<cpal::ErrorKind> for WasapiFailure {
    fn from(kind: cpal::ErrorKind) -> Self {
        use cpal::ErrorKind as K;
        match kind {
            K::DeviceNotAvailable => Self::DeviceNotAvailable,
            K::PermissionDenied => Self::PermissionDenied,
            K::HostUnavailable => Self::HostUnavailable,
            K::DeviceBusy => Self::DeviceBusy,
            K::UnsupportedConfig => Self::UnsupportedConfig,
            _ => Self::Other,
        }
    }
}

/// Maps a cpal (WASAPI host) error on `device`.
#[cfg(windows)]
pub(crate) fn map_cpal_error(e: &cpal::Error, device: &str) -> AudioError {
    map_wasapi_failure(e.kind().into(), &e.to_string(), device)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(e: &AudioError) -> String {
        e.to_string()
    }

    #[test]
    fn backend_errors_are_mapped() {
        // ScreenCaptureKit: TCC denial and missing entitlements name the setting.
        for code in [-3801, -3803] {
            let e = map_sck_error(SCK_ERROR_DOMAIN, code, "The user declined TCCs");
            assert!(matches!(e, AudioError::PermissionDenied(_)), "{e:?}");
            let m = message(&e);
            assert!(m.contains(SCREEN_RECORDING_SETTING), "{m}");
            assert!(m.contains("The user declined TCCs"), "{m}");
        }
        let e = map_sck_error(SCK_ERROR_DOMAIN, -3815, "no source");
        assert!(matches!(e, AudioError::DeviceNotFound(_)), "{e:?}");
        let e = map_sck_error(SCK_ERROR_DOMAIN, -3818, "audio failed");
        assert!(
            matches!(&e, AudioError::Backend(m) if m.contains("audio failed") && m.contains("-3818")),
            "{e:?}"
        );
        // The same code in another domain is not a permission problem.
        let e = map_sck_error("NSOSStatusErrorDomain", -3801, "other");
        assert!(
            matches!(&e, AudioError::Backend(m) if m.contains("NSOSStatusErrorDomain")),
            "{e:?}"
        );

        // WASAPI (through cpal).
        let dev = "Speakers (Realtek)";
        let e = map_wasapi_failure(WasapiFailure::DeviceNotAvailable, "gone", dev);
        assert!(
            matches!(&e, AudioError::DeviceNotFound(d) if d == dev),
            "{e:?}"
        );
        let e = map_wasapi_failure(WasapiFailure::PermissionDenied, "E_ACCESSDENIED", dev);
        assert!(
            matches!(&e, AudioError::PermissionDenied(m) if m.contains(dev) && m.contains("E_ACCESSDENIED")),
            "{e:?}"
        );
        let e = map_wasapi_failure(WasapiFailure::HostUnavailable, "service stopped", dev);
        assert!(matches!(e, AudioError::NoServer), "{e:?}");
        for (kind, needle) in [
            (WasapiFailure::DeviceBusy, "exclusive"),
            (WasapiFailure::UnsupportedConfig, "format"),
            (WasapiFailure::Other, "boom"),
        ] {
            let e = map_wasapi_failure(kind, "boom", dev);
            assert!(
                matches!(&e, AudioError::Backend(m) if m.contains(dev) && m.contains(needle)),
                "{kind:?}: {e:?}"
            );
        }

        #[cfg(windows)]
        {
            use cpal::{Error, ErrorKind};
            let e = map_cpal_error(
                &Error::with_message(ErrorKind::DeviceNotAvailable, "x"),
                dev,
            );
            assert!(
                matches!(&e, AudioError::DeviceNotFound(d) if d == dev),
                "{e:?}"
            );
            let e = map_cpal_error(&Error::with_message(ErrorKind::PermissionDenied, "x"), dev);
            assert!(matches!(e, AudioError::PermissionDenied(_)), "{e:?}");
            let e = map_cpal_error(&Error::new(ErrorKind::HostUnavailable), dev);
            assert!(matches!(e, AudioError::NoServer), "{e:?}");
        }
    }
}
