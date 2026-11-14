//! macOS loopback backend: ScreenCaptureKit system audio (macOS 13+).
//!
//! An audio-only `SCStream` (the content filter must name a display, so the main
//! display is used with a 2x2, 1 fps video configuration and no screen output) delivers
//! 48 kHz stereo float buffers, ScreenCaptureKit's native rate (preflight F53);
//! `to_frame` resamples them to 44.1 kHz. Sample buffers arrive on a private dispatch
//! queue and are pushed into the shared capture ring; the stream itself is owned by a
//! detached capture thread (see `capture_ring`) that stops it once the source is
//! dropped.
//!
//! Capture needs the Screen Recording permission. When it is missing, ScreenCaptureKit
//! fails with `SCStreamErrorUserDeclined`, mapped to `AudioError::PermissionDenied`
//! with a message naming System Settings > Privacy & Security > Screen Recording.
//!
//! ScreenCaptureKit captures the mix of all system audio, not individual output
//! devices, so there is exactly one device: [`SYSTEM_DEVICE`]. `open_device` accepts
//! that name (or `"default"`) and returns `AudioError::DeviceNotFound` for any other.
//!
//! The Objective-C calls are `unsafe` in objc2; each function that makes them carries
//! a scoped `#[allow(unsafe_code)]` and every `unsafe` block states why it is sound.

use crate::backend_errors::map_sck_error;
use crate::capture_ring::{decode_f32_le, spawn_capture, CaptureRing, OPEN_TIMEOUT};
use crate::{AudioError, AudioSource, DeviceInfo};
use block2::RcBlock;
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{available, define_class, msg_send, AnyThread, DefinedClass};
use objc2_core_audio_types::{
    kAudioFormatFlagIsBigEndian, kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved,
    kAudioFormatLinearPCM,
};
use objc2_core_media::{
    CMAudioFormatDescriptionGetStreamBasicDescription, CMSampleBuffer, CMTime, CMTimeFlags,
};
use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol};
use objc2_screen_capture_kit::{
    SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamDelegate,
    SCStreamOutput, SCStreamOutputType, SCWindow,
};
use std::ptr::NonNull;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Capture rate requested from ScreenCaptureKit (its native rate).
const RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
/// The single device name this backend exposes.
pub(crate) const SYSTEM_DEVICE: &str = "system";

/// Lists the one capture device: the system audio mix.
pub fn list_devices() -> Result<Vec<DeviceInfo>, AudioError> {
    Ok(vec![DeviceInfo {
        name: SYSTEM_DEVICE.into(),
        description: "System audio (ScreenCaptureKit)".into(),
        is_default: true,
    }])
}

/// Captures the system audio mix.
pub fn default_loopback() -> Result<Box<dyn AudioSource>, AudioError> {
    // ScreenCaptureKit gained audio capture in macOS 13; on older systems the selectors
    // (or the classes) do not exist and calling them would abort the process.
    if !available!(macos = 13.0) {
        return Err(AudioError::Backend(
            "system-audio capture needs macOS 13 or later (ScreenCaptureKit audio)".into(),
        ));
    }
    spawn_capture("md-audio-sck", OPEN_TIMEOUT, start, |capture, ring| {
        ring.wait_for_stop();
        // `Capture::drop` stops the stream. If `spawn_capture` gave up waiting, `run`
        // never gets here, but the guard is dropped all the same, so a stream that
        // started late is stopped too.
        drop(capture);
    })
}

/// Opens [`SYSTEM_DEVICE`] (or `"default"`); any other name is `DeviceNotFound`.
pub fn open_device(name: &str) -> Result<Box<dyn AudioSource>, AudioError> {
    if name == SYSTEM_DEVICE || name == "default" {
        default_loopback()
    } else {
        Err(AudioError::DeviceNotFound(format!(
            "{name} (ScreenCaptureKit captures only \"{SYSTEM_DEVICE}\")"
        )))
    }
}

fn ns_error(e: &NSError) -> AudioError {
    map_sck_error(
        &e.domain().to_string(),
        e.code(),
        &e.localizedDescription().to_string(),
    )
}

/// Converts the `NSError *` a completion handler receives (nil on success).
#[allow(unsafe_code)]
fn completion_error(error: *mut NSError) -> Option<AudioError> {
    // SAFETY: ScreenCaptureKit passes either nil or a valid NSError that stays alive for
    // the duration of the completion handler, which is where this is called.
    unsafe { error.as_ref() }.map(ns_error)
}

fn timed_out(what: &str) -> AudioError {
    AudioError::Backend(format!("ScreenCaptureKit did not answer ({what})"))
}

struct OutputIvars {
    ring: Arc<CaptureRing>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Output` does not implement
    // `Drop`. Its methods are called on ScreenCaptureKit's dispatch queues; its only
    // ivar is an `Arc<CaptureRing>`, which is `Send + Sync`.
    #[unsafe(super(NSObject))]
    #[name = "MdAudioScreenCaptureOutput"]
    #[ivars = OutputIvars]
    struct Output;

    unsafe impl NSObjectProtocol for Output {}

    // SAFETY: the method signature matches `SCStreamOutput`'s declaration.
    unsafe impl SCStreamOutput for Output {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn stream_did_output(
            &self,
            _stream: &SCStream,
            buffer: &CMSampleBuffer,
            kind: SCStreamOutputType,
        ) {
            if kind == SCStreamOutputType::Audio {
                push_sample_buffer(&self.ivars().ring, buffer);
            }
        }
    }

    // SAFETY: the method signature matches `SCStreamDelegate`'s declaration.
    unsafe impl SCStreamDelegate for Output {
        #[unsafe(method(stream:didStopWithError:))]
        fn stream_did_stop(&self, _stream: &SCStream, error: &NSError) {
            let ring = &self.ivars().ring;
            if !ring.is_stopped() {
                eprintln!(
                    "md-audio: ScreenCaptureKit capture stopped: {}",
                    ns_error(error)
                );
                ring.fail();
            }
        }
    }
);

impl Output {
    #[allow(unsafe_code)]
    fn new(ring: Arc<CaptureRing>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(OutputIvars { ring });
        // SAFETY: calling NSObject's designated initializer on a fresh allocation.
        unsafe { msg_send![super(this), init] }
    }
}

/// Copies one audio `CMSampleBuffer` into the ring. Buffers that are not 32-bit float
/// linear PCM (ScreenCaptureKit always delivers that) are skipped.
#[allow(unsafe_code)]
fn push_sample_buffer(ring: &CaptureRing, buffer: &CMSampleBuffer) {
    // SAFETY: `buffer` is a valid sample buffer for the duration of the callback; these
    // getters only read it, and the returned objects are retained.
    let (ready, frames, format, block) = unsafe {
        (
            buffer.data_is_ready(),
            buffer.num_samples(),
            buffer.format_description(),
            buffer.data_buffer(),
        )
    };
    let (true, Some(format), Some(block), Ok(frames @ 1..)) =
        (ready, format, block, usize::try_from(frames))
    else {
        return;
    };
    // SAFETY: `format` is a live, retained format description. For audio descriptions
    // the result points into it and stays valid while `format` lives (this scope);
    // otherwise it is null, which `as_ref` turns into `None`.
    let Some(asbd) =
        (unsafe { CMAudioFormatDescriptionGetStreamBasicDescription(&format).as_ref() })
    else {
        return;
    };
    let flags = asbd.mFormatFlags;
    let is_f32_le = asbd.mFormatID == kAudioFormatLinearPCM
        && flags & kAudioFormatFlagIsFloat != 0
        && flags & kAudioFormatFlagIsBigEndian == 0
        && asbd.mBitsPerChannel == 32;
    let channels = u16::try_from(asbd.mChannelsPerFrame).unwrap_or(0);
    if !is_f32_le || channels == 0 || !asbd.mSampleRate.is_finite() || asbd.mSampleRate < 1.0 {
        return;
    }
    let planar = flags & kAudioFormatFlagIsNonInterleaved != 0;
    let rate = asbd.mSampleRate.round() as u32;
    let len = frames * usize::from(channels) * 4;
    // SAFETY: read-only query on a live, retained block buffer.
    let available = unsafe { block.data_length() };
    // Planar data is split into equal planes by position, so any padding would shift the
    // later channels: require the exact size. Interleaved data only needs `len` bytes.
    if available < len || (planar && available != len) {
        return;
    }
    let mut bytes = vec![0u8; len];
    // SAFETY: `bytes` is a writable allocation of exactly `len` bytes (non-zero: frames
    // and channels are both non-zero), and the block buffer holds at least `len` bytes
    // from offset 0 (checked above).
    let status =
        unsafe { block.copy_data_bytes(0, len, NonNull::from(bytes.as_mut_slice()).cast()) };
    if status == 0 {
        // Non-interleaved buffers hold one contiguous plane per channel.
        ring.push(&decode_f32_le(&bytes, channels, planar), channels, rate);
    }
}

/// Moves the shareable-content snapshot from ScreenCaptureKit's completion queue to the
/// capture thread.
struct SendContent(Retained<SCShareableContent>);

// SAFETY: SCShareableContent is an immutable snapshot and not main-thread-only. The
// completion handler runs on one of ScreenCaptureKit's dispatch queues and moves the
// value (never shares it) through a channel to the capture thread, which is then its
// only user. Objective-C retain/release are atomic, so dropping it on either thread is
// sound.
#[allow(unsafe_code)]
unsafe impl Send for SendContent {}

#[allow(unsafe_code)]
fn shareable_content(deadline: Instant) -> Result<Retained<SCShareableContent>, AudioError> {
    let (tx, rx) = mpsc::sync_channel(1);
    // Runs on a ScreenCaptureKit dispatch queue, not on the capture thread; it captures
    // only the `SyncSender` (which is `Send`) and moves results out through it.
    let handler = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            // SAFETY: `content` is nil or a valid object for the duration of the
            // handler; `retain` takes our own (atomically counted) reference before it
            // returns.
            let result = match unsafe { Retained::retain(content) } {
                Some(c) => Ok(SendContent(c)),
                None => Err(completion_error(error).unwrap_or_else(|| {
                    AudioError::Backend("ScreenCaptureKit returned no shareable content".into())
                })),
            };
            let _ = tx.send(result);
        },
    );
    // SAFETY: the handler matches the declared block signature; ScreenCaptureKit copies
    // the block, which may outlive this call and runs on another thread (its captures
    // are `Send`).
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&handler) };
    match rx.recv_timeout(remaining(deadline)) {
        Ok(r) => r.map(|c| c.0),
        Err(_) => Err(timed_out("shareable content")),
    }
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// A started (or starting) stream and its output object. Dropping it, on any path,
/// asks ScreenCaptureKit to stop the stream, so a stream is never released while it is
/// still delivering callbacks to `output`.
struct Capture {
    stream: Retained<SCStream>,
    output: Retained<Output>,
}

impl Drop for Capture {
    /// Requests the stop without waiting. The completion handler holds extra references
    /// to the stream and its output, so both stay alive until ScreenCaptureKit reports
    /// that the stop finished and no further sample callbacks can arrive.
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let keep = (self.stream.clone(), self.output.clone());
        // Runs on a ScreenCaptureKit dispatch queue and releases `keep` there.
        let handler = RcBlock::new(move |_error: *mut NSError| {
            let _alive = &keep;
        });
        // SAFETY: `stream` is a live SCStream (we hold a reference). The handler matches
        // the declared block signature and is copied by ScreenCaptureKit, which may run
        // and release it on another thread: that only drops `Retained` handles, and
        // Objective-C retain/release are atomic. Stopping a stream that never started
        // only reports an error to the handler.
        unsafe { self.stream.stopCaptureWithCompletionHandler(Some(&handler)) };
    }
}

/// Builds and starts the audio-only stream (runs on the capture thread).
#[allow(unsafe_code)]
fn start(ring: &Arc<CaptureRing>) -> Result<Capture, AudioError> {
    // Both waits below share one budget, a little shorter than `spawn_capture`'s, so
    // this thread normally gives up before the caller does. If it still finishes late,
    // the caller has dropped the channel and `Capture::drop` stops the stream.
    let deadline = Instant::now() + OPEN_TIMEOUT.saturating_sub(Duration::from_millis(500));
    let content = shareable_content(deadline)?;
    // SAFETY: `content` is a live snapshot; `displays` only reads it.
    let display = unsafe { content.displays() }.firstObject().ok_or_else(|| {
        AudioError::DeviceNotFound("system audio (ScreenCaptureKit lists no display)".into())
    })?;
    let no_windows = NSArray::<SCWindow>::new();
    // SAFETY: initializing a fresh SCContentFilter allocation with live arguments.
    let filter = unsafe {
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            &display,
            &no_windows,
        )
    };
    // SAFETY: plain property setters on a configuration object this thread owns; the
    // values are within the ranges ScreenCaptureKit documents.
    let config = unsafe {
        let config = SCStreamConfiguration::new();
        config.setCapturesAudio(true);
        config.setSampleRate(RATE as isize);
        config.setChannelCount(CHANNELS as isize);
        config.setExcludesCurrentProcessAudio(true);
        // Audio-only: keep the mandatory video side as cheap as possible.
        config.setWidth(2);
        config.setHeight(2);
        config.setMinimumFrameInterval(CMTime {
            value: 1,
            timescale: 1,
            flags: CMTimeFlags::Valid,
            epoch: 0,
        });
        config
    };
    let output = Output::new(Arc::clone(ring));
    let output_proto = ProtocolObject::<dyn SCStreamOutput>::from_ref(&*output);
    let delegate = ProtocolObject::<dyn SCStreamDelegate>::from_ref(&*output);
    // SAFETY: initializing a fresh SCStream allocation; filter, configuration and
    // delegate are live (the delegate is kept alive by `Capture`).
    let stream = unsafe {
        SCStream::initWithFilter_configuration_delegate(
            SCStream::alloc(),
            &filter,
            &config,
            Some(delegate),
        )
    };
    let queue = DispatchQueue::new("md-audio.screencapturekit", None);
    // SAFETY: `output` outlives the stream's use of it: `Capture` holds it, and its Drop
    // keeps it alive until the stop completes. Its methods run on `queue`, a serial
    // dispatch queue serviced by libdispatch threads (not this thread); the only state
    // they touch is the ivar, a single `Arc<CaptureRing>` whose ring is Mutex/Condvar
    // protected, so running concurrently with the capture thread is sound.
    unsafe {
        stream.addStreamOutput_type_sampleHandlerQueue_error(
            output_proto,
            SCStreamOutputType::Audio,
            Some(&queue),
        )
    }
    .map_err(|e| ns_error(&e))?;

    // From here on every exit path drops `capture`, which stops the stream.
    let capture = Capture { stream, output };
    let (tx, rx) = mpsc::sync_channel(1);
    // Runs on a ScreenCaptureKit dispatch queue; captures only the `Send` sender.
    let handler = RcBlock::new(move |error: *mut NSError| {
        let _ = tx.send(completion_error(error));
    });
    // SAFETY: `capture.stream` is live; the handler matches the declared block signature
    // and is copied by ScreenCaptureKit, which runs it on another thread (its captures
    // are `Send`).
    unsafe {
        capture
            .stream
            .startCaptureWithCompletionHandler(Some(&handler))
    };
    match rx.recv_timeout(remaining(deadline)) {
        Ok(None) => Ok(capture),
        Ok(Some(e)) => Err(e),
        Err(_) => Err(timed_out("start capture")),
    }
}
