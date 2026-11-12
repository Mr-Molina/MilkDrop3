//! The GPU context and headless device creation.

use crate::mips::MipCache;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::Thread;

/// The device and queue md-render draws with.
///
/// F6: no lifetime — it stores owned clones of the (reference-counted)
/// `wgpu::Device` and `wgpu::Queue`, so the app can own both the device and
/// a renderer. A.3 signatures taking `&wgpu::Device`/`&wgpu::Queue` build
/// one with [`Gpu::new`]. Cloning a `Gpu` is cheap and shares the device
/// and the built-in pipelines it caches (the mip blitter).
#[derive(Clone, Debug)]
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub(crate) mips: Arc<MipCache>,
}

impl Gpu {
    /// A context sharing `device` and `queue` (clones of the handles).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Gpu {
        Gpu {
            device: device.clone(),
            queue: queue.clone(),
            mips: Arc::default(),
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Block until all submitted work has finished.
    pub fn wait_idle(&self) {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll failed");
    }
}

/// Run a future to completion on this thread (wgpu's native futures resolve
/// promptly; this avoids an executor dependency).
pub(crate) fn block_on<F: Future>(fut: F) -> F::Output {
    struct ThreadWaker(Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

/// A device and queue without a surface, for tests and the headless
/// `render` subcommand. Backends follow `WGPU_BACKEND` (default: all).
/// When `WGPU_ADAPTER_NAME` is set, the first adapter whose name contains
/// it (case-insensitive) is used, and `None` is returned if none matches;
/// otherwise wgpu's default choice. `None` when there is no adapter or the
/// device request fails. The device gets the adapter's own limits.
pub fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = match std::env::var("WGPU_ADAPTER_NAME") {
        Ok(want) => {
            let want = want.to_lowercase();
            block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                .into_iter()
                .find(|a| a.get_info().name.to_lowercase().contains(&want))?
        }
        Err(_) => {
            block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?
        }
    };
    block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("md-render headless"),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .ok()
}

/// [`headless_device`] wrapped in a [`Gpu`].
pub fn headless_gpu() -> Option<Gpu> {
    headless_device().map(|(d, q)| Gpu::new(&d, &q))
}

/// For GPU tests: a headless [`Gpu`], or `None` after printing a skip note
/// for `test` when no adapter exists. With `MD_REQUIRE_GPU=1` (CI) a
/// missing adapter panics instead, so GPU tests never pass vacuously.
pub fn gpu_or_skip(test: &str) -> Option<Gpu> {
    let gpu = headless_gpu();
    if gpu.is_none() {
        if std::env::var("MD_REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("{test}: no GPU adapter, but MD_REQUIRE_GPU=1");
        }
        eprintln!("skip {test}: no GPU adapter (set MD_REQUIRE_GPU=1 to fail instead)");
    }
    gpu
}
