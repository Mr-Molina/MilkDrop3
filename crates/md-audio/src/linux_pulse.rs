//! Linux loopback backend: PulseAudio / PipeWire (pipewire-pulse) monitor capture.
//!
//! Device discovery uses libpulse introspection over a standard mainloop; capture uses
//! the simple API on a background thread that fills a ring buffer of the most recent
//! [`RING_FRAMES`] stereo frames. All bindings are safe Rust over the system libpulse.

use crate::{to_frame, AudioError, AudioSource, DeviceInfo, FrameStatus};
use libpulse_binding::context::{Context, FlagSet, State};
use libpulse_binding::def::BufferAttr;
use libpulse_binding::error::{Code, PAErr};
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use libpulse_binding::operation::{Operation, State as OpState};
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
/// Ring capacity in stereo frames.
const RING_FRAMES: usize = 8192;
/// Frames per capture read (~10 ms).
const READ_FRAMES: usize = 441;
/// Upper bound for connecting to / querying the server.
const SERVER_TIMEOUT: Duration = Duration::from_secs(5);

fn map_err(e: PAErr, device: Option<&str>) -> AudioError {
    match Code::try_from(e).ok() {
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
    mainloop: Mainloop,
    context: Context,
}

impl Server {
    fn connect() -> Result<Self, AudioError> {
        let mut mainloop = Mainloop::new()
            .ok_or_else(|| AudioError::Backend("cannot create PulseAudio mainloop".into()))?;
        let mut context = Context::new(&mainloop, "milkdrop")
            .ok_or_else(|| AudioError::Backend("cannot create PulseAudio context".into()))?;
        context.connect(None, FlagSet::NOFLAGS, None).map_err(|e| {
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
    let simple = Simple::new(
        None,
        "milkdrop",
        Direction::Record,
        Some(name),
        "visualizer capture",
        &spec,
        None,
        Some(&attr),
    )
    .map_err(|e| map_err(e, Some(name)))?;
    Ok(Box::new(PulseSource::start(simple)))
}

struct Ring {
    /// Interleaved stereo samples, newest at the back, at most `RING_FRAMES` frames.
    samples: VecDeque<f32>,
    fresh: bool,
    stop: bool,
    failed: bool,
}

struct Shared {
    ring: Mutex<Ring>,
    cv: Condvar,
}

/// Capture source backed by a PulseAudio record stream.
struct PulseSource {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl PulseSource {
    fn start(simple: Simple) -> Self {
        let shared = Arc::new(Shared {
            ring: Mutex::new(Ring {
                samples: VecDeque::with_capacity(RING_FRAMES * 2),
                fresh: false,
                stop: false,
                failed: false,
            }),
            cv: Condvar::new(),
        });
        let s = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("md-audio-pulse".into())
            .spawn(move || capture_loop(simple, &s))
            .expect("spawn pulse capture thread");
        Self {
            shared,
            thread: Some(thread),
        }
    }
}

fn capture_loop(simple: Simple, shared: &Shared) {
    let mut bytes = vec![0u8; READ_FRAMES * 8];
    loop {
        if shared.ring.lock().map_or(true, |r| r.stop) {
            return;
        }
        let res = simple.read(&mut bytes);
        let Ok(mut ring) = shared.ring.lock() else {
            return;
        };
        if ring.stop {
            return;
        }
        if let Err(e) = res {
            eprintln!("md-audio: PulseAudio capture failed: {e}");
            ring.failed = true;
            shared.cv.notify_all();
            return;
        }
        for c in bytes.chunks_exact(4) {
            ring.samples
                .push_back(f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        }
        let cap = RING_FRAMES * 2;
        let excess = ring.samples.len().saturating_sub(cap);
        ring.samples.drain(..excess);
        ring.fresh = true;
        shared.cv.notify_all();
    }
}

impl AudioSource for PulseSource {
    fn next_frame(
        &mut self,
        left: &mut [u8; 576],
        right: &mut [u8; 576],
        timeout: Duration,
    ) -> FrameStatus {
        let now = Instant::now();
        let deadline = now
            .checked_add(timeout)
            .unwrap_or_else(|| now + Duration::from_secs(86_400 * 365));
        let Ok(mut ring) = self.shared.ring.lock() else {
            left.fill(0);
            right.fill(0);
            return FrameStatus::Ended;
        };
        loop {
            if ring.fresh {
                ring.fresh = false;
                let skip = ring.samples.len().saturating_sub(576 * 2);
                let tail: Vec<f32> = ring.samples.iter().skip(skip).copied().collect();
                drop(ring);
                to_frame(&tail, CHANNELS, RATE, left, right);
                return FrameStatus::Data;
            }
            if ring.failed {
                left.fill(0);
                right.fill(0);
                return FrameStatus::Ended;
            }
            let now = Instant::now();
            if now >= deadline {
                left.fill(0);
                right.fill(0);
                return FrameStatus::Silence;
            }
            ring = match self.shared.cv.wait_timeout(ring, deadline - now) {
                Ok(g) => g.0,
                Err(_) => {
                    left.fill(0);
                    right.fill(0);
                    return FrameStatus::Ended;
                }
            };
        }
    }
}

impl Drop for PulseSource {
    fn drop(&mut self) {
        if let Ok(mut r) = self.shared.ring.lock() {
            r.stop = true;
        }
        self.shared.cv.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
