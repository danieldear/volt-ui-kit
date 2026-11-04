//! Linux Wayland blur compatibility for the stable winit 0.30 host.
//!
//! The two unsafe imports below borrow a winit-owned display/surface, never
//! assume ownership or install a dispatcher on winit's objects. A retained
//! `Arc<Window>` plus `OwnedDisplayHandle` outlive every borrowed proxy. Only
//! the private `Imported::new` handle-import function permits unsafe code.
//!
//! A private queue/dispatcher shares libwayland's synchronized socket reads
//! with winit. It blocks on the Wayland socket and a command pipe, not a timer;
//! no roundtrip blocks the UI thread. Drop wakes and joins the dispatcher before
//! the borrowed window can be released. Construct/drop on the window's thread.

use super::{
    backdrop::logical_extent,
    backdrop::protocol::{Desired, Engine, Notify, Shared},
    BackdropStatus, BackdropUnavailable,
};
use std::{
    collections::HashSet,
    io::Write,
    marker::PhantomData,
    os::unix::net::UnixStream,
    rc::Rc,
    sync::OnceLock,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::{protocol::wl_surface, Connection, Proxy};
use winit::{
    event_loop::OwnedDisplayHandle,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle},
    window::{Window, WindowId},
};

fn desired_for_window(window: &Window, enabled: bool) -> Desired {
    let size = window.inner_size();
    let scale = window.scale_factor();
    Desired {
        enabled,
        width: logical_extent(size.width, scale),
        height: logical_extent(size.height, scale),
    }
}
/// A per-window compositor-blur controller reusable by winit 0.30 consumers.
///
/// Keep exactly one controller for a window. Its callback runs on the protocol
/// dispatcher thread: use an event-loop proxy, not application/UI state, there.
/// Call [`Self::update`] after resize/scale/blur changes. The callback is the
/// only status channel; the controller does not log. `Failed` is the worker's
/// last report: drop the controller and create a new one to retry. Drop it on
/// the window's thread; it retains the window and display connection, so it
/// may outlive the event loop. The controller is deliberately not Send/Sync.
/// It never captures the desktop or changes compositor settings.
pub struct Controller {
    shared: Arc<Shared>,
    signal: UnixStream,
    worker: Option<JoinHandle<()>>,
    _window_thread: PhantomData<Rc<()>>,
    _lease: Lease,
}
impl Controller {
    /// Begin asynchronous discovery on a native Wayland window, retaining its
    /// matching `event_loop.owned_display_handle()`. X11 returns an
    /// explicit unsupported status; setup failure is nonfatal to the renderer.
    pub fn new(
        window: Arc<Window>,
        display: OwnedDisplayHandle,
        enabled: bool,
        notify: impl Fn(BackdropStatus) + Send + Sync + 'static,
    ) -> Result<Self, BackdropStatus> {
        let lease = Lease::new(window.id())?;
        let worker_state = Imported::new(window, display)?;
        let window = &worker_state.window;
        let desired = desired_for_window(window, enabled);
        let shared = Arc::new(Shared {
            desired: Mutex::new(desired),
            stopped: AtomicBool::new(false),
        });
        let (signal, receiver) = UnixStream::pair().map_err(|_| BackdropStatus::Failed)?;
        signal
            .set_nonblocking(true)
            .map_err(|_| BackdropStatus::Failed)?;
        receiver
            .set_nonblocking(true)
            .map_err(|_| BackdropStatus::Failed)?;
        let notify: Notify = Arc::new(notify);
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("volt-wayland-blur".into())
            .stack_size(512 * 1024)
            .spawn(move || {
                let mut worker = worker_state;
                let redraw_notify: Notify = Arc::new({
                    let window = worker.window.clone();
                    let notify = notify.clone();
                    move |status| {
                        window.request_redraw();
                        notify(status);
                    }
                });
                let result = worker.engine.run(&receiver, &worker_shared, &redraw_notify);
                if let Err(error) = result {
                    eprintln!(
                        "Volt UI: Wayland backdrop failed ({error}); continuing without blur"
                    );
                    redraw_notify(BackdropStatus::Failed);
                }
                // Drop the notification's Arc before Worker releases its window.
                drop(redraw_notify);
            })
            .map_err(|_| BackdropStatus::Failed)?;
        Ok(Self {
            shared,
            signal,
            worker: Some(worker),
            _window_thread: PhantomData,
            _lease: lease,
        })
    }
    /// Coalesce changes; ordinary opacity/color updates do not wake the worker.
    pub fn update(&self, window: &Window, enabled: bool) {
        let desired = desired_for_window(window, enabled);
        if let Ok(mut current) = self.shared.desired.lock() {
            if *current == desired {
                return;
            }
            *current = desired;
            self.wake();
        }
    }
    fn wake(&self) {
        let _ = (&self.signal).write(&[1]);
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Release);
        self.wake();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

// Field order is a safety invariant, including panic/spawn-failure paths:
// release the guest engine/queue/proxies before the retained winit window.
struct Imported {
    engine: Engine,
    window: Arc<Window>,
    _display: OwnedDisplayHandle,
}
impl Imported {
    #[allow(unsafe_code)]
    fn new(window: Arc<Window>, owned_display: OwnedDisplayHandle) -> Result<Self, BackdropStatus> {
        let retained = owned_display
            .display_handle()
            .map_err(|_| BackdropStatus::Failed)?
            .as_raw();
        let display = window
            .display_handle()
            .map_err(|_| BackdropStatus::Failed)?;
        let surface = window.window_handle().map_err(|_| BackdropStatus::Failed)?;
        let (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(surface)) =
            (display.as_raw(), surface.as_raw())
        else {
            return Err(BackdropStatus::Unsupported(BackdropUnavailable::X11));
        };
        let RawDisplayHandle::Wayland(retained) = retained else {
            return Err(BackdropStatus::Failed);
        };
        if retained.display != display.display {
            return Err(BackdropStatus::Failed);
        }
        // SAFETY: these matching handles come from the same live winit window.
        // The guest backend does not close the display. `window` in this
        // imported owner outlives the engine/queue/proxies. OwnedDisplayHandle
        // retains the matching display even if the original event loop is dropped.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        // SAFETY: borrowed wl_surface is retained by `window`; never destroy it,
        // take over its dispatcher, attach buffers, or change its queue.
        let id = unsafe {
            ObjectId::from_ptr(
                wl_surface::WlSurface::interface(),
                surface.surface.as_ptr().cast(),
            )
        }
        .map_err(|_| BackdropStatus::Failed)?;
        let surface =
            wl_surface::WlSurface::from_id(&connection, id).map_err(|_| BackdropStatus::Failed)?;
        Ok(Self {
            engine: Engine::new(connection, surface),
            window,
            _display: owned_display,
        })
    }
}

// Reject accidental duplicate controllers before they can violate the Wayland
// protocol's one-effect-object-per-surface rule and disconnect the application.
struct Lease(WindowId);
static CONTROLLERS: OnceLock<Mutex<HashSet<WindowId>>> = OnceLock::new();
impl Lease {
    fn new(id: WindowId) -> Result<Self, BackdropStatus> {
        let mut ids = CONTROLLERS
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|_| BackdropStatus::Failed)?;
        if !ids.insert(id) {
            return Err(BackdropStatus::Failed);
        }
        Ok(Self(id))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(ids) = CONTROLLERS.get() {
            if let Ok(mut ids) = ids.lock() {
                ids.remove(&self.0);
            }
        }
    }
}
