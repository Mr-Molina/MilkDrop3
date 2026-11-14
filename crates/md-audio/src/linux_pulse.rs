//! Linux loopback backend: PulseAudio / PipeWire (pipewire-pulse) monitor capture.
//!
//! Device discovery uses libpulse introspection over a standard mainloop; capture uses
//! the simple API on a detached background thread that fills the shared capture ring
//! (`capture_ring`). All bindings are safe Rust over the system libpulse.

use crate::capture_ring::{decode_f32_le, spawn_capture, CaptureRing, OPEN_TIMEOUT};
use crate::{AudioError, AudioSource, DeviceInfo};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::def::BufferAttr;
use libpulse_binding::error::{Code, PAErr};
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use libpulse_binding::operation::{Operation, State as OpState};
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
/// Frames per capture read (~10 ms).
const READ_FRAMES: usize = 441;
/// Upper bound for connecting to / querying the server.
const SERVER_TIMEOUT: Duration = Duration::from_secs(5);

fn map_err(e: PAErr, device: Option<&str>) -> AudioError {
    // libpulse returns negative codes, but the simple API reports `pa_simple_*`'s
    // positive out-parameter unchanged; `Code::try_from` expects the negative form.
    match Code::try_from(PAErr(-e.0.saturating_abs())).ok() {
        Some(Code::ConnectionRefused | Code::ConnectionTerminated) => AudioError::NoServer,
        Some(Code::Access | Code::AuthKey) => {
            AudioError::PermissionDenied("the sound server refused access".into())
        }
        Some(Code::NoEntity) => match device {
            Some(d) => AudioError::DeviceNotFound(d.to_string()),
            None => AudioError::NoServer,
        },
        Some(Code::Timeout) => AudioError::Backend("the sound server timed out".into()),
        _ => AudioError::Backend(format!("PulseAudio error: {e}")),
    }
}

/// A connected introspection session.
struct Server {
    // Field order matters: fields drop in declaration order, and dropping the context
    // calls back into the mainloop's API, so the context must be freed first.
    context: Context,
    mainloop: Mainloop,
}

impl Server {
    fn connect() -> Result<Self, AudioError> {
        let mut mainloop = Mainloop::new()
            .ok_or_else(|| AudioError::Backend("cannot create PulseAudio mainloop".into()))?;
        let mut context = Context::new(&mainloop, "milkdrop")
            .ok_or_else(|| AudioError::Backend("cannot create PulseAudio context".into()))?;
        context
            .connect(None, FlagSet::NOAUTOSPAWN, None)
            .map_err(|e| {
                match Code::try_from(e).ok() {
                    // libpulse reports an unreachable server socket with codes such as Access.
                    Some(
                        Code::Access
                        | Code::ConnectionRefused
                        | Code::ConnectionTerminated
                        | Code::NoEntity,
                    ) => AudioError::NoServer,
                    _ => map_err(e, None),
                }
            })?;
        let deadline = Instant::now() + SERVER_TIMEOUT;
        let mut authorized = false;
        loop {
            iterate(&mut mainloop)?;
            match context.get_state() {
                State::Ready => break,
                State::Failed | State::Terminated if !authorized => {
                    // libpulse reports a failed socket connect with misleading codes
                    // (e.g. Access); a failure before authorization means no server.
                    return Err(AudioError::NoServer);
                }
                State::Failed | State::Terminated => {
                    return Err(map_err(context.errno(), None));
                }
                State::Authorizing | State::SettingName => authorized = true,
                _ => {}
            }
            if Instant::now() >= deadline {
                return Err(AudioError::Backend(
                    "timed out connecting to PulseAudio".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(Self { mainloop, context })
    }

    fn wait<T: ?Sized>(&mut self, op: &Operation<T>) -> Result<(), AudioError> {
        let deadline = Instant::now() + SERVER_TIMEOUT;
        while op.get_state() == OpState::Running {
            iterate(&mut self.mainloop)?;
            if Instant::now() >= deadline {
                return Err(AudioError::Backend("PulseAudio query timed out".into()));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        match op.get_state() {
            OpState::Done => Ok(()),
            _ => Err(map_err(self.context.errno(), None)),
        }
    }

    fn default_sink(&mut self) -> Result<Option<String>, AudioError> {
        let out = Rc::new(RefCell::new(None));
        let o = Rc::clone(&out);
        let op = self.context.introspect().get_server_info(move |info| {
            *o.borrow_mut() = info.default_sink_name.as_ref().map(|s| s.to_string());
        });
        self.wait(&op)?;
        let name = out.borrow().clone();
        Ok(name)
    }

    fn monitors(&mut self) -> Result<Vec<(String, String)>, AudioError> {
        use libpulse_binding::callbacks::ListResult;
        let out = Rc::new(RefCell::new(Vec::new()));
        let o = Rc::clone(&out);
        let op = self.context.introspect().get_source_info_list(move |r| {
            if let ListResult::Item(i) = r {
                if i.monitor_of_sink.is_some() {
                    if let Some(n) = i.name.as_ref() {
                        let d = i
                            .description
                            .as_ref()
                            .map_or_else(String::new, |d| d.to_string());
                        o.borrow_mut().push((n.to_string(), d));
                    }
                }
            }
        });
        self.wait(&op)?;
        let v = out.borrow().clone();
        Ok(v)
    }
}

fn iterate(mainloop: &mut Mainloop) -> Result<(), AudioError> {
    match mainloop.iterate(false) {
        IterateResult::Success(_) => Ok(()),
        IterateResult::Quit(_) | IterateResult::Err(_) => {
            Err(AudioError::Backend("PulseAudio mainloop stopped".into()))
        }
    }
}

/// Lists monitor sources; `is_default` marks the default sink's monitor.
pub fn list_devices() -> Result<Vec<DeviceInfo>, AudioError> {
    let mut server = Server::connect()?;
    let default = server.default_sink()?.map(|s| format!("{s}.monitor"));
    Ok(server
        .monitors()?
        .into_iter()
        .map(|(name, description)| DeviceInfo {
            is_default: default.as_deref() == Some(name.as_str()),
            name,
            description,
        })
        .collect())
}

/// Opens the default sink's monitor.
pub fn default_loopback() -> Result<Box<dyn AudioSource>, AudioError> {
    let sink = {
        let mut server = Server::connect()?;
        server.default_sink()?
    };
    let sink = sink.ok_or_else(|| AudioError::DeviceNotFound("default sink".into()))?;
    open_source(&format!("{sink}.monitor"))
}

/// Records from the named source.
pub fn open_device(name: &str) -> Result<Box<dyn AudioSource>, AudioError> {
    // The simple API cannot tell "no server" from other failures, so probe first.
    drop(Server::connect()?);
    open_source(name)
}

fn open_source(name: &str) -> Result<Box<dyn AudioSource>, AudioError> {
    let spec = Spec {
        format: Format::F32le,
        channels: CHANNELS as u8,
        rate: RATE,
    };
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: (READ_FRAMES * 8) as u32,
    };
    let device = name.to_string();
    spawn_capture(
        "md-audio-pulse",
        OPEN_TIMEOUT,
        move |_| {
            Simple::new(
                None,
                "milkdrop",
                Direction::Record,
                Some(&device),
                "visualizer capture",
                &spec,
                None,
                Some(&attr),
            )
            .map_err(|e| map_err(e, Some(&device)))
        },
        capture_loop,
    )
}

/// Reads the record stream into the ring until the source is dropped. `Simple::read`
/// has no timeout, so after a drop this thread exits (and frees the stream) when the
/// pending read returns or fails; nothing ever joins it.
fn capture_loop(simple: Simple, ring: &CaptureRing) {
    let mut bytes = vec![0u8; READ_FRAMES * 8];
    while !ring.is_stopped() {
        if let Err(e) = simple.read(&mut bytes) {
            if !ring.is_stopped() {
                eprintln!("md-audio: PulseAudio capture failed: {e}");
                ring.fail();
            }
            return;
        }
        if !ring.push(&decode_f32_le(&bytes, CHANNELS, false), CHANNELS, RATE) {
            return;
        }
    }
}
