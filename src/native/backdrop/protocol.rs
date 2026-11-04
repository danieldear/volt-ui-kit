//! Safe protocol engine; also exercised against an in-process mock compositor.
//! Requests are double buffered. Never commit a foreign wl_surface here: the
//! host renderer owns buffer/commit sequencing. Notify the UI to present after
//! a region change, so its next normal surface commit applies the effect.
use super::{negotiate, BackdropProtocol, BackdropStatus};
use rustix::event::{poll, PollFd, PollFlags};
use std::{
    io::Read,
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use wayland_client::{
    protocol::{wl_callback, wl_compositor, wl_registry, wl_surface},
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::{self as effects, ExtBackgroundEffectManagerV1},
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use wayland_protocols_plasma::blur::client::{
    org_kde_kwin_blur::OrgKdeKwinBlur, org_kde_kwin_blur_manager::OrgKdeKwinBlurManager,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Desired {
    pub enabled: bool,
    pub width: i32,
    pub height: i32,
}
pub(crate) struct Shared {
    pub desired: Mutex<Desired>,
    pub stopped: AtomicBool,
}
pub(crate) type Notify = Arc<dyn Fn(BackdropStatus) + Send + Sync>;
struct ProtocolState {
    surface: wl_surface::WlSurface,
    compositor: Option<(u32, wl_compositor::WlCompositor)>,
    manager: Option<(u32, ExtBackgroundEffectManagerV1)>,
    modern_capability: Option<bool>,
    legacy_global: Option<(u32, OrgKdeKwinBlurManager)>,
    legacy_effect: Option<OrgKdeKwinBlur>,
    ready: bool,
    effect: Option<ExtBackgroundEffectSurfaceV1>,
    applied: Option<Desired>,
}
pub(crate) struct Engine {
    queue: EventQueue<ProtocolState>,
    registry: wl_registry::WlRegistry,
    connection: Connection,
    state: ProtocolState,
    discovery_timeout: Duration,
}
impl Engine {
    pub(crate) fn new(connection: Connection, surface: wl_surface::WlSurface) -> Self {
        let queue = connection.new_event_queue();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        connection.display().sync(&qh, ());
        Self {
            queue,
            registry,
            connection,
            state: ProtocolState {
                surface,
                compositor: None,
                manager: None,
                modern_capability: None,
                legacy_global: None,
                legacy_effect: None,
                ready: false,
                effect: None,
                applied: None,
            },
            discovery_timeout: Duration::from_secs(5),
        }
    }
    #[cfg(test)]
    fn with_discovery_timeout(mut self, timeout: Duration) -> Self {
        self.discovery_timeout = timeout;
        self
    }
    pub(crate) fn run(
        &mut self,
        commands: &UnixStream,
        shared: &Shared,
        notify: &Notify,
    ) -> Result<(), String> {
        let mut discovery_started = None;
        let mut last_status = None;
        loop {
            self.queue
                .dispatch_pending(&mut self.state)
                .map_err(|e| e.to_string())?;
            if shared.stopped.load(Ordering::Acquire) {
                return Ok(());
            }
            let desired = *shared
                .desired
                .lock()
                .map_err(|_| "backdrop command lock poisoned")?;
            let discovering = !self.state.ready
                || (self.state.manager.is_some() && self.state.modern_capability.is_none());
            let deadline = if discovering {
                *discovery_started.get_or_insert_with(|| Instant::now() + self.discovery_timeout)
            } else {
                discovery_started = None;
                Instant::now()
            };
            if discovering && Instant::now() >= deadline {
                if !self.state.ready {
                    return Err("compositor did not answer the initial sync".into());
                }
                // A manager that never reports capabilities is treated as
                // declining blur, so the legacy fallback still applies. A late
                // capabilities event overwrites this and is honoured.
                self.state.modern_capability = Some(false);
                discovery_started = None;
                continue;
            }
            let status = negotiate(
                desired.enabled && desired.width > 0 && desired.height > 0,
                !discovering,
                self.state.modern_capability,
                self.state.legacy_global.is_some(),
            );
            let mut request_changed = false;
            let modern =
                status == BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect);
            let kde = status == BackdropStatus::Requested(BackdropProtocol::WaylandKde);
            // Remove the previous protocol before creating a replacement.
            if !modern && self.state.clear_modern() {
                request_changed = true;
            }
            if !kde && self.state.clear_legacy() {
                request_changed = true;
            }
            if (modern || kde) && desired.width > 0 && desired.height > 0 {
                if modern && self.state.effect.is_none() {
                    let (_, manager) = self
                        .state
                        .manager
                        .as_ref()
                        .ok_or("missing background-effect manager")?;
                    self.state.effect = Some(manager.get_background_effect(
                        &self.state.surface,
                        &self.queue.handle(),
                        (),
                    ));
                    self.state.applied = None;
                }
                if kde && self.state.legacy_effect.is_none() {
                    let (_, manager) = self
                        .state
                        .legacy_global
                        .as_ref()
                        .ok_or("missing KDE blur manager")?;
                    self.state.legacy_effect =
                        Some(manager.create(&self.state.surface, &self.queue.handle(), ()));
                    self.state.applied = None;
                }
                if self.state.applied != Some(desired) {
                    let (_, compositor) = self
                        .state
                        .compositor
                        .as_ref()
                        .ok_or("missing Wayland compositor")?;
                    let region = compositor.create_region(&self.queue.handle(), ());
                    region.add(0, 0, desired.width, desired.height);
                    if modern {
                        self.state
                            .effect
                            .as_ref()
                            .unwrap()
                            .set_blur_region(Some(&region));
                    } else {
                        let effect = self.state.legacy_effect.as_ref().unwrap();
                        effect.set_region(Some(&region));
                        effect.commit();
                    }
                    region.destroy();
                    self.state.applied = Some(desired);
                    request_changed = true;
                }
            }
            // Flush before describing the request as submitted. WouldBlock is
            // handled by waiting for writability, not busy retrying on the UI.
            let blocked = match self.connection.flush() {
                Ok(()) => false,
                Err(wayland_backend::client::WaylandError::Io(e))
                    if e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    true
                }
                Err(e) => return Err(e.to_string()),
            };
            // Status goes only through `notify`; callers decide whether to log.
            if !blocked && (last_status != Some(status) || request_changed) {
                notify(status);
                last_status = Some(status);
            }
            let Some(guard) = self.connection.prepare_read() else {
                continue;
            };
            let mut fds = [
                PollFd::new(
                    &self.connection,
                    PollFlags::IN
                        | if blocked {
                            PollFlags::OUT
                        } else {
                            PollFlags::empty()
                        },
                ),
                PollFd::new(commands, PollFlags::IN),
            ];
            let remaining = if discovering {
                Some(
                    rustix::event::Timespec::try_from(
                        deadline.saturating_duration_since(Instant::now()),
                    )
                    .map_err(|e| e.to_string())?,
                )
            } else {
                None
            };
            match poll(&mut fds, remaining.as_ref()) {
                Ok(_) => (),
                Err(rustix::io::Errno::INTR) => {
                    drop(guard);
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            }
            let socket = fds[0].revents();
            if socket.contains(PollFlags::NVAL) {
                return Err("invalid Wayland socket".into());
            }
            let command = fds[1].revents();
            if !command.is_empty() {
                drop(guard);
                let mut bytes = [0; 64];
                loop {
                    match (&*commands).read(&mut bytes) {
                        Ok(0) => return Ok(()),
                        Ok(_) => (),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e.to_string()),
                    }
                }
            } else if socket.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR) {
                match guard.read() {
                    Ok(_) => (),
                    Err(wayland_backend::client::WaylandError::Io(e))
                        if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e.to_string()),
                }
            } else {
                drop(guard);
            }
        }
    }
    pub(crate) fn cleanup(&mut self) {
        self.state.clear_modern();
        self.state.clear_legacy();
        if let Some((_, manager)) = self.state.manager.take() {
            manager.destroy();
        }
        if let Some((_, compositor)) = self.state.compositor.take() {
            drop(compositor);
        }
        // wl_registry has no destructor request. The guest backend releases its
        // local proxy on drop and never destroys the borrowed wl_surface.
        let _ = &self.registry;
        let _ = self.connection.flush();
    }
}
impl Dispatch<wl_registry::WlRegistry, ()> for ProtocolState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "ext_background_effect_manager_v1" if state.manager.is_none() => {
                    state.manager = Some((name, registry.bind(name, version.min(1), qh, ())));
                    state.modern_capability = None;
                }
                "wl_compositor" if state.compositor.is_none() => {
                    state.compositor = Some((name, registry.bind(name, version.min(1), qh, ())))
                }
                "org_kde_kwin_blur_manager" if state.legacy_global.is_none() => {
                    state.legacy_global = Some((name, registry.bind(name, version.min(1), qh, ())))
                }
                _ => (),
            },
            wl_registry::Event::GlobalRemove { name } => {
                if state.manager.as_ref().is_some_and(|(id, _)| *id == name) {
                    state.clear_modern();
                    state.manager.take().unwrap().1.destroy();
                    state.modern_capability = None;
                }
                if state
                    .legacy_global
                    .as_ref()
                    .is_some_and(|(id, _)| *id == name)
                {
                    state.clear_legacy();
                    state.legacy_global = None;
                }
                if state.compositor.as_ref().is_some_and(|(id, _)| *id == name) {
                    state.compositor.take();
                }
            }
            _ => (),
        }
    }
}
impl Dispatch<ExtBackgroundEffectManagerV1, ()> for ProtocolState {
    fn event(
        state: &mut Self,
        manager: &ExtBackgroundEffectManagerV1,
        event: effects::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if !state
            .manager
            .as_ref()
            .is_some_and(|(_, active)| active.id() == manager.id())
        {
            return;
        }
        if let effects::Event::Capabilities { flags } = event {
            let flags = match flags {
                WEnum::Value(v) => v.bits(),
                WEnum::Unknown(v) => v,
            };
            state.modern_capability = Some(flags & effects::Capability::Blur.bits() != 0);
        }
    }
}
impl Dispatch<wl_callback::WlCallback, ()> for ProtocolState {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.ready = true;
    }
}
wayland_client::delegate_noop!(ProtocolState: ignore wl_compositor::WlCompositor);
wayland_client::delegate_noop!(ProtocolState: ignore wayland_client::protocol::wl_region::WlRegion);
wayland_client::delegate_noop!(ProtocolState: ignore ExtBackgroundEffectSurfaceV1);

impl Drop for Engine {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;

impl ProtocolState {
    fn clear_modern(&mut self) -> bool {
        if let Some(effect) = self.effect.take() {
            effect.destroy();
            self.applied = None;
            true
        } else {
            false
        }
    }
    fn clear_legacy(&mut self) -> bool {
        if let Some(effect) = self.legacy_effect.take() {
            if let Some((_, manager)) = &self.legacy_global {
                manager.unset(&self.surface);
            }
            effect.release();
            self.applied = None;
            true
        } else {
            false
        }
    }
}
wayland_client::delegate_noop!(ProtocolState: ignore OrgKdeKwinBlurManager);
wayland_client::delegate_noop!(ProtocolState: ignore OrgKdeKwinBlur);
