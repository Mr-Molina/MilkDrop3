//! Error type shared by every audio source and backend.

/// Errors from opening or running an audio source.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    /// No sound server (PulseAudio/PipeWire) is reachable.
    #[error("no audio server is reachable")]
    NoServer,
    /// The OS refused access to the audio device or capture API.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// The named device does not exist.
    #[error("audio device not found: {0}")]
    DeviceNotFound(String),
    /// Any other backend failure (also: no backend on this platform).
    #[error("audio backend error: {0}")]
    Backend(String),
    /// A caller-supplied argument is invalid.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// File I/O failed.
    #[error("audio I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The WAV file could not be decoded.
    #[error("WAV error: {0}")]
    Wav(#[from] hound::Error),
}
