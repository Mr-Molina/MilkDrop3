//! Windows loopback backend: WASAPI through cpal.
//!
//! cpal's WASAPI host turns an input stream built on an *output* (render) endpoint into
//! a loopback capture (`AUDCLNT_STREAMFLAGS_LOOPBACK`, cpal 0.18
//! `host/wasapi/device.rs::build_input_stream_raw_inner`), so no `unsafe` is needed
//! here. The stream uses the endpoint's shared-mode mix format (normally 32-bit float at
//! 44.1 or 48 kHz); `to_frame` resamples to 44.1 kHz. The stream lives on a detached
//! capture thread (see `capture_ring`); its callback pushes into the shared ring.
//!
//! Device names are cpal `DeviceId` strings (`wasapi:{endpoint id}`); `open_device`
//! also accepts a device's friendly name. Opening a capture (input) endpoint by name
//! records that endpoint directly instead of looping back.

use crate::backend_errors::map_cpal_error;
use crate::capture_ring::{spawn_capture, CaptureRing, OPEN_TIMEOUT};
use crate::{AudioError, AudioSource, DeviceInfo};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, DeviceId, ErrorKind, SampleFormat, SizedSample, Stream, StreamConfig};
use std::str::FromStr;
use std::sync::Arc;

fn host() -> cpal::Host {
    cpal::default_host()
}

fn label(device: &Device) -> String {
    device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "unknown audio device".into())
}

fn id_string(device: &Device) -> Option<String> {
    device.id().ok().map(|id| id.to_string())
}

/// Lists render endpoints (each can be looped back); `is_default` marks the default one.
pub fn list_devices() -> Result<Vec<DeviceInfo>, AudioError> {
    let host = host();
    let default = host.default_output_device().and_then(|d| id_string(&d));
    let devices = host
        .output_devices()
        .map_err(|e| map_cpal_error(&e, "output devices"))?;
    Ok(devices
        .filter_map(|d| {
            let name = id_string(&d)?;
            Some(DeviceInfo {
                is_default: default.as_deref() == Some(name.as_str()),
                description: label(&d),
                name,
            })
        })
        .collect())
}

/// Loops back the default render endpoint, following default-device changes.
pub fn default_loopback() -> Result<Box<dyn AudioSource>, AudioError> {
    open(None)
}

/// Loops back the endpoint with this id (or friendly name).
pub fn open_device(name: &str) -> Result<Box<dyn AudioSource>, AudioError> {
    open(Some(name.to_string()))
}

fn find_device(host: &cpal::Host, name: &str) -> Result<Device, AudioError> {
    if let Some(d) = DeviceId::from_str(name)
        .ok()
        .and_then(|id| host.device_by_id(&id))
    {
        return Ok(d);
    }
    host.devices()
        .map_err(|e| map_cpal_error(&e, name))?
        .find(|d| label(d) == name)
        .ok_or_else(|| AudioError::DeviceNotFound(name.to_string()))
}

fn open(name: Option<String>) -> Result<Box<dyn AudioSource>, AudioError> {
    spawn_capture(
        "md-audio-wasapi",
        OPEN_TIMEOUT,
        move |ring| open_stream(ring, name.as_deref()),
        |stream, ring| {
            ring.wait_for_stop();
            drop(stream);
        },
    )
}

fn open_stream(ring: &Arc<CaptureRing>, name: Option<&str>) -> Result<Stream, AudioError> {
    let host = host();
    let device = match name {
        Some(n) => find_device(&host, n)?,
        None => host
            .default_output_device()
            .ok_or_else(|| AudioError::DeviceNotFound("default output device".into()))?,
    };
    let label = label(&device);
    // Render endpoints report their mix format as the output config; capture endpoints
    // (only reachable through `open_device`) as the input config.
    let supported = if device.supports_output() {
        device.default_output_config()
    } else {
        device.default_input_config()
    }
    .map_err(|e| map_cpal_error(&e, &label))?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, &config, ring, &label, |s| s),
        SampleFormat::I16 => {
            build::<i16>(&device, &config, ring, &label, |s| f32::from(s) / 32_768.0)
        }
        SampleFormat::I32 => build::<i32>(&device, &config, ring, &label, |s| {
            (f64::from(s) / 2_147_483_648.0) as f32
        }),
        other => Err(AudioError::Backend(format!(
            "'{label}' uses an unsupported sample format: {other}"
        ))),
    }?;
    stream.play().map_err(|e| map_cpal_error(&e, &label))?;
    Ok(stream)
}

fn build<T: SizedSample + 'static>(
    device: &Device,
    config: &StreamConfig,
    ring: &Arc<CaptureRing>,
    label: &str,
    convert: fn(T) -> f32,
) -> Result<Stream, AudioError> {
    let (channels, rate) = (config.channels, config.sample_rate);
    let data_ring = Arc::clone(ring);
    let err_ring = Arc::clone(ring);
    let err_label = label.to_string();
    let mut scratch: Vec<f32> = Vec::new();
    device
        .build_input_stream::<T, _, _>(
            *config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                scratch.clear();
                scratch.extend(data.iter().map(|&s| convert(s)));
                data_ring.push(&scratch, channels, rate);
            },
            move |e: cpal::Error| match e.kind() {
                // The default-device stream was rerouted and keeps running.
                ErrorKind::DeviceChanged | ErrorKind::RealtimeDenied => {}
                ErrorKind::DeviceNotAvailable
                | ErrorKind::StreamInvalidated
                | ErrorKind::HostUnavailable => {
                    eprintln!("md-audio: WASAPI capture of '{err_label}' stopped: {e}");
                    err_ring.fail();
                }
                _ => eprintln!("md-audio: WASAPI capture of '{err_label}': {e}"),
            },
            None,
        )
        .map_err(|e| map_cpal_error(&e, label))
}
