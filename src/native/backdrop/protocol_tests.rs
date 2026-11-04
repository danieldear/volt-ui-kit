//! Wire-level mock-compositor tests, with no display server/GPU or personal data.
use super::*;
use crate::native::BackdropUnavailable;
use std::io::Write;
use std::thread::JoinHandle;
use std::{sync::mpsc, thread};
use wayland_protocols::ext::background_effect::v1::server::{
    ext_background_effect_manager_v1 as sm, ext_background_effect_surface_v1 as se,
};
use wayland_protocols_plasma::blur::server::org_kde_kwin_blur as sb;
use wayland_protocols_plasma::blur::server::org_kde_kwin_blur_manager as sk;
use wayland_server::protocol::{wl_compositor as sc, wl_region as sr, wl_surface as ss};
use wayland_server::{Client, DataInit, Display, DisplayHandle, GlobalDispatch, New, Resource};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct Audit {
    regions: Vec<Vec<[i32; 4]>>,
    creates: usize,
    destroys: usize,
    surface_commits: usize,
    legacy_creates: usize,
    legacy_commits: usize,
    legacy_unsets: usize,
    legacy_releases: usize,
    legacy_regions: Vec<Vec<[i32; 4]>>,
}
#[derive(Default, Clone)]
struct Region(Arc<Mutex<Vec<[i32; 4]>>>);
struct Server {
    audit: Arc<Mutex<Audit>>,
    managers: Vec<sm::ExtBackgroundEffectManagerV1>,
    blur: bool,
    /// Withhold the capabilities event on bind (a misbehaving compositor).
    silent_bind: bool,
}
impl GlobalDispatch<sc::WlCompositor, ()> for Server {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<sc::WlCompositor>,
        _: &(),
        init: &mut DataInit<'_, Self>,
    ) {
        init.init(resource, ());
    }
}
impl wayland_server::Dispatch<sc::WlCompositor, ()> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &sc::WlCompositor,
        request: sc::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        match request {
            sc::Request::CreateSurface { id } => {
                init.init(id, ());
            }
            sc::Request::CreateRegion { id } => {
                init.init(id, Region::default());
            }
            _ => (),
        }
    }
}
impl wayland_server::Dispatch<sr::WlRegion, Region> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &sr::WlRegion,
        request: sr::Request,
        data: &Region,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let sr::Request::Add {
            x,
            y,
            width,
            height,
        } = request
        {
            data.0.lock().unwrap().push([x, y, width, height]);
        }
    }
}
impl wayland_server::Dispatch<ss::WlSurface, ()> for Server {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ss::WlSurface,
        request: ss::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let ss::Request::Commit = request {
            state.audit.lock().unwrap().surface_commits += 1;
        }
    }
}
impl GlobalDispatch<sm::ExtBackgroundEffectManagerV1, ()> for Server {
    fn bind(
        state: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<sm::ExtBackgroundEffectManagerV1>,
        _: &(),
        init: &mut DataInit<'_, Self>,
    ) {
        let manager = init.init(resource, ());
        if !state.silent_bind {
            manager.capabilities(if state.blur {
                sm::Capability::Blur
            } else {
                sm::Capability::empty()
            });
        }
        state.managers.push(manager);
    }
}
impl wayland_server::Dispatch<sm::ExtBackgroundEffectManagerV1, ()> for Server {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &sm::ExtBackgroundEffectManagerV1,
        request: sm::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let sm::Request::GetBackgroundEffect { id, .. } = request {
            init.init(id, ());
            state.audit.lock().unwrap().creates += 1;
        }
    }
}
impl wayland_server::Dispatch<se::ExtBackgroundEffectSurfaceV1, ()> for Server {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &se::ExtBackgroundEffectSurfaceV1,
        request: se::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            se::Request::SetBlurRegion {
                region: Some(region),
            } => {
                let rectangles = region.data::<Region>().unwrap().0.lock().unwrap().clone();
                state.audit.lock().unwrap().regions.push(rectangles);
            }
            se::Request::Destroy => state.audit.lock().unwrap().destroys += 1,
            _ => (),
        }
    }
}
impl GlobalDispatch<sk::OrgKdeKwinBlurManager, ()> for Server {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<sk::OrgKdeKwinBlurManager>,
        _: &(),
        init: &mut DataInit<'_, Self>,
    ) {
        init.init(resource, ());
    }
}
impl wayland_server::Dispatch<sk::OrgKdeKwinBlurManager, ()> for Server {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &sk::OrgKdeKwinBlurManager,
        request: sk::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        match request {
            sk::Request::Create { id, .. } => {
                init.init(id, ());
                state.audit.lock().unwrap().legacy_creates += 1;
            }
            sk::Request::Unset { .. } => state.audit.lock().unwrap().legacy_unsets += 1,
            _ => (),
        }
    }
}
impl
    wayland_server::Dispatch<
        wayland_protocols_plasma::blur::server::org_kde_kwin_blur::OrgKdeKwinBlur,
        (),
    > for Server
{
    fn request(
        state: &mut Self,
        _: &Client,
        _: &wayland_protocols_plasma::blur::server::org_kde_kwin_blur::OrgKdeKwinBlur,
        request: wayland_protocols_plasma::blur::server::org_kde_kwin_blur::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            sb::Request::SetRegion {
                region: Some(region),
            } => {
                let rectangles = region.data::<Region>().unwrap().0.lock().unwrap().clone();
                state.audit.lock().unwrap().legacy_regions.push(rectangles);
            }
            sb::Request::Commit => state.audit.lock().unwrap().legacy_commits += 1,
            sb::Request::Release => state.audit.lock().unwrap().legacy_releases += 1,
            _ => (),
        }
    }
}

struct Setup {
    compositor: Option<wl_compositor::WlCompositor>,
}
impl Dispatch<wl_registry::WlRegistry, ()> for Setup {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            if interface == "wl_compositor" {
                state.compositor = Some(registry.bind(name, 1, qh, ()));
            }
        }
    }
}
wayland_client::delegate_noop!(Setup: ignore wl_compositor::WlCompositor);
wayland_client::delegate_noop!(Setup: ignore wl_surface::WlSurface);

struct Fixture {
    shared: Arc<Shared>,
    signal: UnixStream,
    statuses: mpsc::Receiver<BackdropStatus>,
    done: mpsc::Receiver<Result<(), String>>,
    worker: Option<JoinHandle<()>>,
    server: Option<JoinHandle<()>>,
    server_stop: Arc<AtomicBool>,
    blur: Arc<AtomicBool>,
    modern: Arc<AtomicBool>,
    /// Ask the server to (re)send current capabilities to every manager.
    announce: Arc<AtomicBool>,
    audit: Arc<Mutex<Audit>>,
}
impl Fixture {
    fn new(modern: bool, legacy: bool) -> Self {
        Self::build(modern, legacy, false, Duration::from_secs(5))
    }
    fn build(modern: bool, legacy: bool, silent_bind: bool, discovery: Duration) -> Self {
        let (client, server) = UnixStream::pair().unwrap();
        let audit = Arc::new(Mutex::new(Audit::default()));
        let server_stop = Arc::new(AtomicBool::new(false));
        let blur = Arc::new(AtomicBool::new(true));
        let modern_flag = Arc::new(AtomicBool::new(modern));
        let announce = Arc::new(AtomicBool::new(false));
        let server_thread = {
            let (audit, stop, blur, modern_flag, announce) = (
                audit.clone(),
                server_stop.clone(),
                blur.clone(),
                modern_flag.clone(),
                announce.clone(),
            );
            thread::spawn(move || {
                let mut display = Display::<Server>::new().unwrap();
                let mut dh = display.handle();
                let _client = dh.insert_client(server, Arc::new(())).unwrap();
                dh.create_global::<Server, sc::WlCompositor, _>(1, ());
                if legacy {
                    dh.create_global::<Server, sk::OrgKdeKwinBlurManager, _>(1, ());
                }
                let mut global = modern.then(|| {
                    dh.create_global::<Server, sm::ExtBackgroundEffectManagerV1, _>(1, ())
                });
                let mut state = Server {
                    audit,
                    managers: vec![],
                    blur: true,
                    silent_bind,
                };
                while !stop.load(Ordering::Acquire) {
                    if global.is_some() != modern_flag.load(Ordering::Acquire) {
                        if let Some(id) = global.take() {
                            dh.remove_global::<Server>(id);
                        } else {
                            global = Some(
                                dh.create_global::<Server, sm::ExtBackgroundEffectManagerV1, _>(
                                    1,
                                    (),
                                ),
                            );
                        }
                    }
                    let current = blur.load(Ordering::Acquire);
                    if state.blur != current || announce.swap(false, Ordering::AcqRel) {
                        state.blur = current;
                        for manager in &state.managers {
                            if manager.is_alive() {
                                manager.capabilities(if current {
                                    sm::Capability::Blur
                                } else {
                                    sm::Capability::empty()
                                });
                            }
                        }
                    }
                    display.dispatch_clients(&mut state).unwrap_or_default();
                    display.flush_clients().unwrap();
                    thread::sleep(Duration::from_millis(1)); // Test server only, not production dispatcher.
                }
            })
        };
        let connection = Connection::from_socket(client).unwrap();
        let mut queue = connection.new_event_queue();
        let registry = connection.display().get_registry(&queue.handle(), ());
        let mut setup = Setup { compositor: None };
        queue.roundtrip(&mut setup).unwrap();
        let surface = setup
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(&queue.handle(), ());
        connection.flush().unwrap();
        let shared = Arc::new(Shared {
            desired: Mutex::new(Desired {
                enabled: true,
                width: 1440,
                height: 864,
            }),
            stopped: AtomicBool::new(false),
        });
        let (signal, receiver) = UnixStream::pair().unwrap();
        receiver.set_nonblocking(true).unwrap();
        let (tx, statuses) = mpsc::channel();
        let (finished, done) = mpsc::channel();
        let worker = {
            let shared = shared.clone();
            thread::spawn(move || {
                let mut engine = Engine::new(connection, surface).with_discovery_timeout(discovery);
                let notify: Notify = Arc::new(move |s| {
                    let _ = tx.send(s);
                });
                let result = engine.run(&receiver, &shared, &notify);
                drop(engine);
                drop(queue);
                drop(registry);
                let _ = finished.send(result);
            })
        };
        Self {
            shared,
            signal,
            statuses,
            done,
            worker: Some(worker),
            server: Some(server_thread),
            server_stop,
            blur,
            modern: modern_flag,
            announce,
            audit,
        }
    }
    fn wait(&self, expected: BackdropStatus) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let actual = self
                .statuses
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|e| {
                    panic!(
                        "waiting for {expected:?}: {e}; audit={:?}",
                        self.audit.lock().unwrap()
                    )
                });
            if actual == expected {
                break;
            }
        }
    }
    fn update(&self, desired: Desired) {
        *self.shared.desired.lock().unwrap() = desired;
        (&self.signal).write_all(&[1]).unwrap();
    }
    fn until(&self, predicate: impl Fn(&Audit) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !predicate(&self.audit.lock().unwrap()) {
            assert!(
                Instant::now() < deadline,
                "mock compositor did not receive expected request"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn stop(&mut self) {
        self.shared.stopped.store(true, Ordering::Release);
        (&self.signal).write_all(&[1]).unwrap();
        assert!(self
            .done
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .is_ok());
        self.worker.take().unwrap().join().unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Release);
        let _ = (&self.signal).write_all(&[1]);
        self.server_stop.store(true, Ordering::Release);
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[test]
fn modern_only_wire_requests_resize_disable_reenable_and_no_idle_redraw() {
    let mut f = Fixture::new(true, false);
    let on = BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect);
    f.wait(on);
    f.until(|a| a.regions == vec![vec![[0, 0, 1440, 864]]]);
    f.update(Desired {
        enabled: true,
        width: 1000,
        height: 500,
    });
    f.wait(on);
    f.until(|a| a.regions.last() == Some(&vec![[0, 0, 1000, 500]]));
    assert_eq!(f.audit.lock().unwrap().creates, 1);
    // Assert observable requests and redraw notifications, not a counter updated
    // after dispatch_clients returns (which races the server's request handler).
    let audit = f.audit.lock().unwrap().clone();
    f.update(Desired {
        enabled: true,
        width: 1000,
        height: 500,
    });
    assert!(matches!(
        f.statuses.recv_timeout(Duration::from_millis(40)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(*f.audit.lock().unwrap(), audit);
    f.update(Desired {
        enabled: false,
        width: 1000,
        height: 500,
    });
    f.wait(BackdropStatus::Disabled);
    f.until(|a| a.destroys == 1);
    f.update(Desired {
        enabled: true,
        width: 1000,
        height: 500,
    });
    f.wait(on);
    f.until(|a| a.creates == 2);
    assert_eq!(
        f.audit.lock().unwrap().surface_commits,
        0,
        "only the host renderer may commit its wl_surface"
    );
    f.stop();
    f.until(|a| a.destroys == 2);
}
#[test]
fn capability_withdrawal_and_restoration_are_observed_without_user_input() {
    let mut f = Fixture::new(true, false);
    let on = BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect);
    f.wait(on);
    f.blur.store(false, Ordering::Release);
    f.wait(BackdropStatus::Unsupported(
        BackdropUnavailable::CompositorDeclined,
    ));
    f.until(|a| a.destroys == 1);
    f.blur.store(true, Ordering::Release);
    f.wait(on);
    f.until(|a| a.creates == 2);
    f.stop();
}
#[test]
fn legacy_only_and_unsupported_compositors_do_not_report_modern_blur() {
    for legacy in [true, false] {
        let mut f = Fixture::new(false, legacy);
        f.wait(if legacy {
            BackdropStatus::Requested(BackdropProtocol::WaylandKde)
        } else {
            BackdropStatus::Unsupported(BackdropUnavailable::NoProtocol)
        });
        if legacy {
            f.until(|a| a.legacy_commits == 1);
            assert_eq!(
                f.audit.lock().unwrap().legacy_regions,
                vec![vec![[0, 0, 1440, 864]]]
            );
        }
        assert_eq!(f.audit.lock().unwrap().creates, 0);
        f.stop();
        if legacy {
            f.until(|a| a.legacy_unsets == 1 && a.legacy_releases == 1);
        }
    }
}
#[test]
fn global_removal_falls_back_and_new_global_is_rebound() {
    let mut f = Fixture::new(true, true);
    let on = BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect);
    f.wait(on);
    assert_eq!(f.audit.lock().unwrap().legacy_creates, 0);
    f.modern.store(false, Ordering::Release);
    f.wait(BackdropStatus::Requested(BackdropProtocol::WaylandKde));
    f.until(|a| a.legacy_commits == 1);
    f.modern.store(true, Ordering::Release);
    f.wait(on);
    f.until(|a| a.legacy_unsets == 1 && a.legacy_releases == 1);
    f.stop();
}

#[test]
fn zero_extent_waits_for_a_visible_surface_without_sending_invalid_regions() {
    let mut f = Fixture::new(true, false);
    let on = BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect);
    f.wait(on);
    f.update(Desired {
        enabled: true,
        width: 0,
        height: 0,
    });
    f.wait(BackdropStatus::Disabled);
    f.until(|a| a.destroys == 1);
    assert!(f
        .audit
        .lock()
        .unwrap()
        .regions
        .iter()
        .flatten()
        .all(|r| r[2] > 0 && r[3] > 0));
    f.update(Desired {
        enabled: true,
        width: 1000,
        height: 500,
    });
    f.wait(on);
    f.until(|a| a.creates == 2);
    f.stop();
}
#[test]
fn server_disconnect_terminates_dispatch_instead_of_spinning() {
    let mut f = Fixture::new(true, false);
    f.wait(BackdropStatus::Requested(
        BackdropProtocol::WaylandBackgroundEffect,
    ));
    f.server_stop.store(true, Ordering::Release);
    f.server.take().unwrap().join().unwrap();
    assert!(f
        .done
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .is_err());
    f.worker.take().unwrap().join().unwrap();
}
#[test]
fn missing_capabilities_fall_back_without_failing_and_a_late_event_recovers() {
    let on = BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect);
    for legacy in [true, false] {
        let mut f = Fixture::build(true, legacy, true, Duration::from_millis(100));
        f.wait(if legacy {
            BackdropStatus::Requested(BackdropProtocol::WaylandKde)
        } else {
            BackdropStatus::Unsupported(BackdropUnavailable::CompositorDeclined)
        });
        assert_eq!(f.audit.lock().unwrap().creates, 0);
        if legacy {
            f.until(|a| a.legacy_commits == 1);
        }
        f.announce.store(true, Ordering::Release);
        f.wait(on);
        f.until(|a| a.creates == 1 && a.regions == vec![vec![[0, 0, 1440, 864]]]);
        if legacy {
            f.until(|a| a.legacy_unsets == 1 && a.legacy_releases == 1);
        }
        f.stop();
    }
}
