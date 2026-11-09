//! A test bed for maps drawn by Volt from any provider's style and tiles.
//! The app owns the camera; tiles download in the background and arrive as
//! messages. Apple Maps has no tile service for other renderers, so on macOS
//! its map comes from MapKit snapshots instead.
#[cfg(target_os = "macos")]
pub mod apple;
pub mod mvt;
pub mod net;
pub mod perspective;
pub mod services;
pub mod style;
pub mod view;

use net::{Fetched, Job, Msg, Net, TileData, TileKey};
use services::{Backend, Mode, Place, Route, Units};
use std::collections::{BTreeMap, BTreeSet};
use style::{SourceKind, Style};
use volt_ui_kit::{graphics::Image, native::UiSender, Palette, ThemeMode, UiTheme};

pub use view::view;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    OpenFreeMap,
    OpenStreetMap,
    Mapbox,
    Google,
    Apple,
}

impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::OpenFreeMap,
        Provider::OpenStreetMap,
        Provider::Mapbox,
        Provider::Google,
        Provider::Apple,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Provider::OpenFreeMap => "OpenFreeMap",
            Provider::OpenStreetMap => "OpenStreetMap",
            Provider::Mapbox => "Mapbox",
            Provider::Google => "Google",
            Provider::Apple => "Apple",
        }
    }
    /// What a provider's key is called, for providers that need one.
    pub fn key_name(self) -> Option<&'static str> {
        match self {
            Provider::Mapbox => Some("Mapbox access token"),
            Provider::Google => Some("Google Maps API key"),
            _ => None,
        }
    }
    /// The environment variable a provider's key or token comes from.
    pub fn key_variable(self) -> Option<&'static str> {
        match self {
            Provider::Mapbox => Some("MAPBOX_ACCESS_TOKEN"),
            Provider::Google => Some("GOOGLE_MAPS_API_KEY"),
            _ => None,
        }
    }
}

/// Where the map looks: the centre in Web Mercator units (0..1 across the
/// world), a zoom where the world is 512 × 2^zoom points wide, the compass
/// direction at the top of the map (degrees clockwise from north) and how
/// far the view leans back from straight down (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub center: [f64; 2],
    pub zoom: f64,
    pub bearing: f64,
    pub pitch: f64,
}

impl Camera {
    pub fn at(lat: f64, lon: f64, zoom: f64) -> Self {
        Self {
            center: project(lat, lon),
            zoom,
            bearing: 0.,
            pitch: 0.,
        }
    }
    /// North up and looking straight down: the map is drawn as it always
    /// was, tile by tile in upright boxes.
    pub fn flat(&self) -> bool {
        self.bearing
            .rem_euclid(360.)
            .min(360. - self.bearing.rem_euclid(360.))
            < 1e-6
            && self.pitch < 1e-6
    }
    pub fn world(&self) -> f64 {
        512. * self.zoom.exp2()
    }
    pub fn lat_lon(&self) -> (f64, f64) {
        unproject(self.center)
    }
}

pub fn project(lat: f64, lon: f64) -> [f64; 2] {
    let lat = lat.clamp(-85.0511, 85.0511).to_radians();
    [
        (lon + 180.) / 360.,
        (1. - (lat.tan() + 1. / lat.cos()).ln() / std::f64::consts::PI) / 2.,
    ]
}

pub fn unproject(p: [f64; 2]) -> (f64, f64) {
    let lon = p[0] * 360. - 180.;
    let n = std::f64::consts::PI * (1. - 2. * p[1]);
    (n.sinh().atan().to_degrees(), lon)
}

/// What the side panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Search,
    Place,
    Directions,
}

/// One tile on screen: which tile to show and where its top-left lands.
#[derive(Clone, Debug)]
pub struct Placement {
    pub key: TileKey,
    pub origin: [f64; 2],
    pub size: f64,
}

pub struct App {
    pub provider: Provider,
    pub camera: Camera,
    pub size: [f32; 2],
    /// While dragging: the ground under the pointer when it went down (Web
    /// Mercator), kept under the pointer as it moves.
    pub drag_from: Option<[f64; 2]>,
    /// While Option- or Command-dragging: the camera when it started and
    /// the ground at the middle of the open map, which stays put.
    orbit: Option<(Camera, [f64; 2], [f32; 2])>,
    pub style: Option<Style>,
    pub tiles: BTreeMap<TileKey, TileData>,
    pub pending: BTreeSet<TileKey>,
    pub failed: usize,
    pub status: String,
    pub generation: u64,
    pub net: Option<Net>,
    pub sender: Option<UiSender<Msg>>,
    /// Google's tile URLs need a session created with the key first.
    pub google_session: Option<String>,
    /// Apple: the latest MapKit snapshot and the camera it was taken at.
    pub snapshot: Option<(Image, Camera)>,
    pub snapshot_request: u64,
    /// The map and window are dark.
    pub dark: bool,
    /// The system appearance as the theme last saw it (the theme function
    /// only reads the app, so it notes the mode here).
    pub appearance: std::cell::Cell<bool>,
    // Search, places and directions.
    pub panel: Panel,
    pub query: String,
    pub search_seq: u64,
    pub searching: bool,
    pub results: Vec<Place>,
    pub search_error: Option<String>,
    /// How far the open search has looked: 0 what's in view, 1 the region
    /// around it, 2 anywhere. It widens only while little has turned up.
    pub search_stage: u8,
    /// A category search ("cafe") rather than a search for names.
    pub search_category: Option<&'static str>,
    /// What the nearby search found, kept while a wider one runs.
    search_near: Vec<Place>,
    /// Where the search was made from: results are sorted and measured
    /// from here, even after the map moves to show them.
    pub search_from: (f64, f64),
    /// The place on show, and the destination for directions.
    pub selected: Option<Place>,
    pub origin: Option<Place>,
    pub mode: Mode,
    pub route: Option<Route>,
    pub route_seq: u64,
    pub routing: bool,
    pub route_error: Option<String>,
    pub layers_open: bool,
    pub panel_open: bool,
    /// A short note over the map (why something can't be done right now).
    pub hint: Option<String>,
    pub settings_open: bool,
    pub units: Units,
    /// Keys typed into Settings, by provider; they win over the environment.
    /// Only in memory while the app runs.
    pub keys: BTreeMap<&'static str, String>,
    /// What's typed in each key field, before it's saved.
    pub key_drafts: BTreeMap<&'static str, String>,
    // Current location.
    #[cfg(target_os = "macos")]
    pub locator: Option<apple::Locator>,
    /// Latitude, longitude and accuracy in metres.
    pub location: Option<(f64, f64, f64)>,
    pub locating: bool,
    /// Centre the map on the next fix.
    pub follow: bool,
    pub location_note: Option<String>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    location_ticks: u32,
    // Camera motion.
    pub motion: Option<Motion>,
    /// Tells the frame ticker to send frames while something moves.
    frames: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Recent drag positions, for the glide after letting go.
    drag_trail: Vec<(std::time::Instant, [f32; 2])>,
}

/// How the camera is moving, between frames.
#[derive(Clone, Copy, Debug)]
pub enum Motion {
    /// Ease from one camera to another, pulling out over long distances.
    Fly {
        from: Camera,
        to: Camera,
        start: std::time::Instant,
        ms: f64,
        lift: f64,
    },
    /// Ease the zoom, keeping the place under `anchor` (Web Mercator
    /// `under`) where it is.
    Zoom {
        from: f64,
        to: f64,
        anchor: [f32; 2],
        under: [f64; 2],
        start: std::time::Instant,
        ms: f64,
    },
    /// Turn and tilt to a bearing and pitch, keeping the ground `under` at
    /// the point `anchor` of the map.
    Turn {
        from: Camera,
        bearing: f64,
        pitch: f64,
        anchor: [f32; 2],
        under: [f64; 2],
        start: std::time::Instant,
        ms: f64,
    },
    /// Keep moving after a drag lets go, slowing down. Points per millisecond.
    Glide {
        velocity: [f64; 2],
        last: std::time::Instant,
    },
}

/// Where a flight is at time `t` (0..=1): eased along the way, pulled out
/// in the middle by `lift` zoom levels.
pub fn flight(from: Camera, to: Camera, lift: f64, t: f64) -> Camera {
    let e = ease_in_out(t);
    Camera {
        center: [
            (from.center[0] + (to.center[0] - from.center[0]) * e).rem_euclid(1.),
            from.center[1] + (to.center[1] - from.center[1]) * e,
        ],
        zoom: (from.zoom + (to.zoom - from.zoom) * e - lift * (std::f64::consts::PI * e).sin())
            .max(1.),
        bearing: from.bearing + turn_between(from.bearing, to.bearing) * e,
        pitch: from.pitch + (to.pitch - from.pitch) * e,
    }
}

/// The camera moved (not turned, tilted or zoomed) so the ground `under`
/// (Web Mercator) shows at the point `at` of a map of `size`. Moving the
/// camera moves all the ground under it alike, at any bearing and pitch.
fn under_point(camera: Camera, size: [f32; 2], under: [f64; 2], at: [f32; 2]) -> Camera {
    let view = perspective::View::new(camera, size);
    let Some(r) = view.ground([f64::from(at[0]), f64::from(at[1])]) else {
        return camera;
    };
    let now = view.mercator_from_turned(r);
    let dx = (under[0] - now[0] + 0.5).rem_euclid(1.) - 0.5;
    Camera {
        center: [
            (camera.center[0] + dx).rem_euclid(1.),
            (camera.center[1] + under[1] - now[1]).clamp(0., 1.),
        ],
        ..camera
    }
}

/// The shortest turn from one bearing to another, in degrees (−180..180).
pub fn turn_between(from: f64, to: f64) -> f64 {
    (to - from + 180.).rem_euclid(360.) - 180.
}

fn ease_in_out(t: f64) -> f64 {
    if t < 0.5 {
        4. * t * t * t
    } else {
        1. - (-2. * t + 2.).powi(3) / 2.
    }
}

fn ease_out(t: f64) -> f64 {
    1. - (1. - t).powi(3)
}

impl App {
    pub fn new() -> Self {
        Self {
            provider: Provider::OpenFreeMap,
            // Tromsø, on Northbound's route.
            camera: Camera::at(69.6496, 18.956, 11.),
            size: [0., 0.],
            drag_from: None,
            orbit: None,
            style: None,
            tiles: BTreeMap::new(),
            pending: BTreeSet::new(),
            failed: 0,
            status: "Starting…".into(),
            generation: 0,
            net: None,
            sender: None,
            google_session: None,
            snapshot: None,
            snapshot_request: 0,
            dark: system_is_dark(),
            appearance: std::cell::Cell::new(system_is_dark()),
            panel: Panel::Search,
            query: String::new(),
            search_seq: 0,
            searching: false,
            results: Vec::new(),
            search_error: None,
            search_stage: 0,
            search_category: None,
            search_near: Vec::new(),
            search_from: (0., 0.),
            selected: None,
            origin: None,
            mode: Mode::Drive,
            route: None,
            route_seq: 0,
            routing: false,
            route_error: None,
            layers_open: false,
            panel_open: true,
            hint: None,
            settings_open: false,
            units: system_units(),
            keys: BTreeMap::new(),
            key_drafts: BTreeMap::new(),
            #[cfg(target_os = "macos")]
            locator: None,
            location: None,
            locating: false,
            follow: false,
            location_note: None,
            location_ticks: 0,
            motion: None,
            frames: Default::default(),
            drag_trail: Vec::new(),
        }
    }

    // ── Camera motion ──────────────────────────────────────────────────

    fn set_motion(&mut self, motion: Option<Motion>) {
        self.motion = motion;
        self.frames
            .store(motion.is_some(), std::sync::atomic::Ordering::Relaxed);
    }

    /// Move the camera there: smoothly in a window, at once headless.
    pub fn fly_to(&mut self, to: Camera) {
        if self.sender.is_none() {
            self.camera = to;
            self.request_tiles();
            return;
        }
        let from = self.camera;
        let mut dx = to.center[0] - from.center[0];
        if dx > 0.5 {
            dx -= 1.;
        } else if dx < -0.5 {
            dx += 1.;
        }
        let dy = to.center[1] - from.center[1];
        // How far it is on screen at the start; far trips pull out first.
        let far =
            (dx.hypot(dy) * from.world()) / f64::from(self.size[0].hypot(self.size[1]).max(1.));
        let lift = if far > 1. {
            far.log2().min(6.) * 0.9 + 0.5
        } else {
            0.
        };
        let ms = 420. + lift * 120.;
        let target = Camera {
            center: [from.center[0] + dx, to.center[1]],
            ..to
        };
        self.prefetch_flight(from, target, lift);
        self.set_motion(Some(Motion::Fly {
            from,
            to: target,
            start: std::time::Instant::now(),
            ms,
            lift,
        }));
    }

    /// Ease the zoom around a point in the map.
    pub fn zoom_smoothly(&mut self, anchor: [f32; 2], zoom: f64) {
        let to = zoom.clamp(1., 19.);
        if self.sender.is_none() {
            self.zoom_at(anchor, to);
            return;
        }
        let under = self.ground_at(anchor);
        self.set_motion(Some(Motion::Zoom {
            from: self.camera.zoom,
            to,
            anchor,
            under,
            start: std::time::Instant::now(),
            ms: 260.,
        }));
    }

    /// One frame of motion.
    fn frame(&mut self) {
        let Some(motion) = self.motion else {
            self.set_motion(None);
            return;
        };
        let now = std::time::Instant::now();
        if std::env::var_os("MAPS_DEBUG").is_some() {
            let since = match motion {
                Motion::Fly { start, .. }
                | Motion::Zoom { start, .. }
                | Motion::Turn { start, .. } => now.duration_since(start).as_millis(),
                Motion::Glide { last, .. } => now.duration_since(last).as_millis(),
            };
            eprintln!("frame {} ms in, zoom {:.2}", since, self.camera.zoom);
        }
        let done;
        match motion {
            Motion::Fly {
                from,
                to,
                start,
                ms,
                lift,
            } => {
                let t = (now.duration_since(start).as_secs_f64() * 1000. / ms).min(1.);
                self.camera = flight(from, to, lift, t);
                if t >= 1. {
                    self.camera.bearing = to.bearing.rem_euclid(360.);
                }
                done = t >= 1.;
            }
            Motion::Zoom {
                from,
                to,
                anchor,
                under,
                start,
                ms,
            } => {
                let t = (now.duration_since(start).as_secs_f64() * 1000. / ms).min(1.);
                self.camera.zoom = from + (to - from) * ease_out(t);
                self.keep_under(under, anchor);
                done = t >= 1.;
            }
            Motion::Turn {
                from,
                bearing,
                pitch,
                anchor,
                under,
                start,
                ms,
            } => {
                let t = (now.duration_since(start).as_secs_f64() * 1000. / ms).min(1.);
                let e = ease_in_out(t);
                self.camera.bearing = from.bearing + turn_between(from.bearing, bearing) * e;
                self.camera.pitch = from.pitch + (pitch - from.pitch) * e;
                self.keep_under(under, anchor);
                done = t >= 1.;
            }
            Motion::Glide { velocity, last } => {
                let dt = now.duration_since(last).as_secs_f64() * 1000.;
                // The ground a step behind the middle moves to the middle.
                let middle = [self.size[0] / 2., self.size[1] / 2.];
                let behind = self.ground_at([
                    middle[0] - (velocity[0] * dt) as f32,
                    middle[1] - (velocity[1] * dt) as f32,
                ]);
                self.keep_under(behind, middle);
                let slow = 0.9f64.powf(dt / 16.);
                let velocity = [velocity[0] * slow, velocity[1] * slow];
                done = velocity[0].hypot(velocity[1]) < 0.03;
                self.motion = Some(Motion::Glide {
                    velocity,
                    last: now,
                });
            }
        }
        if done {
            self.set_motion(None);
            self.request_tiles();
        } else if self.provider != Provider::Apple {
            // Apple snapshots wait for the camera to settle.
            self.request_tiles();
        }
    }

    /// A camera centred on a place in the part of the map the panel leaves open.
    fn centred(&self, lat: f64, lon: f64, zoom: f64) -> Camera {
        let place = project(lat, lon);
        let camera = Camera {
            center: place,
            zoom,
            ..self.camera
        };
        let [x, y, w, h] = self.open_area();
        under_point(camera, self.size, place, [x + w / 2., y + h / 2.])
    }

    /// The ground (Web Mercator) under a point of the map.
    fn ground_at(&self, at: [f32; 2]) -> [f64; 2] {
        let view = perspective::View::new(self.camera, self.size);
        match view.ground([f64::from(at[0]), f64::from(at[1])]) {
            Some(r) => view.mercator_from_turned(r),
            None => self.camera.center,
        }
    }

    /// Move the camera so the ground `under` (Web Mercator) sits at `at`.
    fn keep_under(&mut self, under: [f64; 2], at: [f32; 2]) {
        self.camera = under_point(self.camera, self.size, under, at);
    }

    /// Vector maps can turn and tilt; image tiles and MapKit snapshots stay
    /// upright for now.
    pub fn can_tilt(&self) -> bool {
        matches!(self.provider, Provider::OpenFreeMap | Provider::Mapbox)
            && self
                .style
                .as_ref()
                .is_some_and(|s| s.sources.values().any(|src| src.kind == SourceKind::Vector))
    }

    /// Turn and tilt smoothly, keeping the middle of the open map in place.
    pub fn turn_to(&mut self, bearing: f64, pitch: f64) {
        let [x, y, w, h] = self.open_area();
        let anchor = [x + w / 2., y + h / 2.];
        let under = self.ground_at(anchor);
        let pitch = pitch.clamp(0., perspective::MAX_PITCH);
        if self.sender.is_none() {
            self.camera.bearing = bearing.rem_euclid(360.);
            self.camera.pitch = pitch;
            self.keep_under(under, anchor);
            self.request_tiles();
            return;
        }
        let ms = 300.
            + turn_between(self.camera.bearing, bearing).abs() * 1.2
            + (pitch - self.camera.pitch).abs() * 3.;
        self.set_motion(Some(Motion::Turn {
            from: self.camera,
            bearing,
            pitch,
            anchor,
            under,
            start: std::time::Instant::now(),
            ms: ms.min(900.),
        }));
    }

    /// Point north again (the compass button).
    pub fn face_north(&mut self) {
        self.turn_to(0., self.camera.pitch);
    }

    /// The 3D button: lean back to see buildings, or look straight down.
    pub fn toggle_3d(&mut self) {
        if self.camera.pitch > 1. {
            self.turn_to(self.camera.bearing, 0.);
        } else if self.can_tilt() {
            self.turn_to(self.camera.bearing, 55.);
            // Buildings have heights from about zoom 15.
            if self.camera.zoom < 15. {
                let [x, y, w, h] = self.open_area();
                self.zoom_smoothly([x + w / 2., y + h / 2.], 16.);
            }
        } else {
            self.hint = Some("3D needs a vector map. Choose OpenFreeMap under Map style.".into());
        }
    }

    // ── Current location ───────────────────────────────────────────────

    /// Ask macOS where we are (the first time, it asks the person), then
    /// centre the map there when the answer comes.
    pub fn locate(&mut self) {
        self.follow = true;
        if let Some((lat, lon, _)) = self.location {
            self.fly_to(self.centred(lat, lon, self.camera.zoom.max(15.)));
            return;
        }
        #[cfg(target_os = "macos")]
        {
            self.locating = true;
            self.location_note = Some("Finding your location…".into());
            if self.locator.is_none() {
                self.locator = Some(apple::Locator::start());
                // Read the fix a few times a second while waiting, then
                // now and then to keep the dot current.
                if let Some(sender) = self.sender.clone() {
                    std::thread::spawn(move || {
                        // Quickly while waiting for the first fix (and the
                        // permission prompt), then now and then.
                        for tick in 0u32.. {
                            let wait = if tick < 75 { 400 } else { 5_000 };
                            std::thread::sleep(std::time::Duration::from_millis(wait));
                            if sender.send(Msg::LocationTick).is_err() {
                                return;
                            }
                        }
                    });
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.location_note = Some("Location is only wired up on macOS so far.".into());
        }
    }

    fn location_tick(&mut self) {
        #[cfg(target_os = "macos")]
        {
            let Some(locator) = &self.locator else { return };
            self.location_ticks += 1;
            match locator.read() {
                apple::Located::At(lat, lon, accuracy) => {
                    let first = self.location.is_none();
                    self.location = Some((lat, lon, accuracy));
                    self.locating = false;
                    self.location_note = None;
                    if self.follow {
                        self.follow = false;
                        self.fly_to(self.centred(lat, lon, self.camera.zoom.max(15.)));
                    }
                    // Directions that started from the map can start here.
                    if first && self.origin.is_none() {
                        self.origin = Some(self.here());
                    }
                }
                apple::Located::Denied => {
                    self.locating = false;
                    self.follow = false;
                    self.location_note = Some(
                        "Location is off for Maps. Turn it on in System Settings › Privacy & Security › Location Services.".into(),
                    );
                }
                apple::Located::Waiting if self.locating && self.location_ticks > 75 => {
                    self.locating = false;
                    self.location_note =
                        Some("No location yet. Check that Location Services is on.".into());
                }
                apple::Located::Waiting => {}
            }
        }
    }

    fn here(&self) -> Place {
        let (lat, lon, _) = self.location.unwrap_or((0., 0., 0.));
        Place {
            name: "My location".into(),
            detail: format!("{lat:.4}, {lon:.4}"),
            kind: String::new(),
            lat,
            lon,
        }
    }

    /// Start directions here when the location is known.
    pub fn origin_from_location(&mut self) {
        if self.location.is_none() {
            self.locate();
            return;
        }
        self.origin = Some(self.here());
        self.panel = Panel::Directions;
        self.request_route();
    }

    /// Search and directions follow the map. Mapbox and Google without a key
    /// fall back to the open servers.
    pub fn backend(&self) -> Backend {
        match (self.provider, self.key()) {
            (Provider::Mapbox, Some(token)) => Backend::Mapbox(token),
            (Provider::Google, Some(key)) => Backend::Google(key),
            (Provider::Apple, _) if cfg!(target_os = "macos") => Backend::Apple,
            _ => Backend::Open,
        }
    }

    // ── Search ──────────────────────────────────────────────────────────

    /// Typing searches once it pauses, so each keystroke isn't a request.
    pub fn set_query(&mut self, text: String) {
        self.query = text;
        self.search_seq += 1;
        self.panel = Panel::Search;
        if self.query.trim().chars().count() < 2 {
            self.results.clear();
            self.searching = false;
            self.search_error = None;
            return;
        }
        self.searching = true;
        let (Some(sender), seq) = (self.sender.clone(), self.search_seq) else {
            return;
        };
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(350));
            let _ = sender.send(Msg::SearchDue(seq));
        });
    }

    pub fn search_now(&mut self) {
        if self.query.trim().is_empty() {
            return;
        }
        self.search_seq += 1;
        let seq = self.search_seq;
        self.run_search(seq, true);
    }

    /// `submitted`: Return or a suggestion chip, rather than a pause in typing.
    fn run_search(&mut self, seq: u64, submitted: bool) {
        if seq != self.search_seq {
            return;
        }
        self.searching = true;
        self.search_error = None;
        self.panel = Panel::Search;
        self.search_stage = 0;
        self.search_near.clear();
        // From where you are if that's in view, else from what you're
        // looking at: a map dragged to Niagara searches Niagara.
        self.search_from = self
            .location
            .map(|(lat, lon, _)| (lat, lon))
            .filter(|&(lat, lon)| {
                let [px, py] = self.screen(lat, lon);
                let [x, y, w, h] = self.open_area();
                px >= x && py >= y && px <= x + w && py <= y + h
            })
            .unwrap_or_else(|| self.visible_centre());
        self.search_category = submitted.then(|| services::category(&self.query)).flatten();
        self.search_backend(seq);
    }

    /// The area around the part of the map in view, `scale` times its size
    /// and at least `least` metres across: (west, south, east, north).
    fn area(&self, scale: f64, least: f64) -> [f64; 4] {
        let world = self.camera.world();
        let (lat, lon) = self.visible_centre();
        let centre = project(lat, lon);
        let [_, _, w, h] = self.open_area();
        let per_point = 40_075_016.686 * lat.to_radians().cos() / world;
        let floor = least / 2. / per_point / world;
        let half = [
            (f64::from(w.max(400.)) * scale / 2. / world).max(floor),
            (f64::from(h.max(400.)) * scale / 2. / world).max(floor),
        ];
        let (north, west) = unproject([centre[0] - half[0], (centre[1] - half[1]).clamp(0., 1.)]);
        let (south, east) = unproject([centre[0] + half[0], (centre[1] + half[1]).clamp(0., 1.)]);
        [west.max(-180.), south, east.min(180.), north]
    }

    fn search_backend(&mut self, seq: u64) {
        let near = self.visible_centre();
        // Categories: what's in view, then six and thirty times wider.
        // Names: at least 25 km around, then at least 300 km, then anywhere.
        if let (Some(phrase), Backend::Open) = (self.search_category, self.backend()) {
            let area = match self.search_stage {
                0 => self.area(1.2, 2_000.),
                1 => self.area(6., 12_000.),
                _ => self.area(30., 80_000.),
            };
            if let Some(net) = &self.net {
                net.send(self.generation, services::category_job(seq, phrase, area));
            }
            return;
        }
        let area = match self.search_stage {
            0 => Some(self.area(1.5, 25_000.)),
            1 => Some(self.area(8., 300_000.)),
            _ => None,
        };
        match self.backend() {
            #[cfg(target_os = "macos")]
            Backend::Apple => {
                if let Some(sender) = self.sender.clone() {
                    apple::search(sender, seq, self.query.trim(), self.camera, self.size);
                }
            }
            backend => {
                if let (Some(job), Some(net)) = (
                    services::search_job(&backend, seq, self.query.trim(), near, area),
                    &self.net,
                ) {
                    net.send(self.generation, job);
                }
            }
        }
    }

    fn found(&mut self, seq: u64, result: Result<Vec<Place>, String>) {
        if seq != self.search_seq {
            return;
        }
        let category = self.search_category.is_some();
        // Merge what this step found with the nearer steps' finds.
        let result = result.map(|wider| {
            let mut places = std::mem::take(&mut self.search_near);
            for place in wider {
                let same = places.iter().any(|p| {
                    p.name == place.name
                        && (p.lat - place.lat).abs() < 1e-4
                        && (p.lon - place.lon).abs() < 1e-4
                });
                if !same {
                    places.push(place);
                }
            }
            // Categories list the nearest first.
            if category {
                let here = self.search_from;
                places.sort_by(|a, b| {
                    services::haversine(here, (a.lat, a.lon))
                        .total_cmp(&services::haversine(here, (b.lat, b.lon)))
                });
            }
            places
        });
        // Little nearby: look further, keeping what was found. Categories
        // widen while there are under five; names go to the region with
        // under four, and anywhere only when nothing nearer turned up.
        if let (Ok(places), Backend::Open) = (&result, self.backend()) {
            let widen = match self.search_stage {
                0 => places.len() < if category { 5 } else { 4 },
                1 => {
                    if category {
                        places.len() < 5
                    } else {
                        places.is_empty()
                    }
                }
                _ => false,
            };
            if widen {
                self.search_near = places.clone();
                self.search_stage += 1;
                if category {
                    // Nominatim asks for a second between requests.
                    if let Some(sender) = self.sender.clone() {
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(1100));
                            let _ = sender.send(Msg::SearchWider(seq));
                        });
                        return;
                    }
                }
                self.search_backend(seq);
                return;
            }
        }
        self.searching = false;
        match result {
            Ok(places) => {
                self.search_error = places.is_empty().then(|| "No places match.".into());
                self.results = places;
                // Show them all, unless they're already in view.
                let points: Vec<(f64, f64)> = self.results.iter().map(|p| (p.lat, p.lon)).collect();
                let [x, y, w, h] = self.open_area();
                let shown = points.iter().all(|&(lat, lon)| {
                    let [px, py] = self.screen(lat, lon);
                    px >= x + 20. && py >= y + 20. && px <= x + w - 20. && py <= y + h - 20.
                });
                if !points.is_empty() && !shown {
                    self.fit(&points);
                }
            }
            Err(e) => {
                self.results.clear();
                self.search_error = Some(e);
            }
        }
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.search_seq += 1;
        self.route_seq += 1;
        self.results.clear();
        self.search_error = None;
        self.searching = false;
        self.selected = None;
        self.origin = None;
        self.route = None;
        self.routing = false;
        self.route_error = None;
        self.panel = Panel::Search;
    }

    /// Show a place: centre it, close enough to see the street.
    pub fn select(&mut self, place: Place) {
        let zoom = self.camera.zoom.max(15.);
        let to = self.centred(place.lat, place.lon, zoom);
        self.selected = Some(place);
        self.route = None;
        self.route_error = None;
        self.panel = Panel::Place;
        self.fly_to(to);
    }

    /// Back from a place or directions to the results.
    pub fn back(&mut self) {
        self.panel = Panel::Search;
        self.route = None;
        self.route_seq += 1;
        self.routing = false;
    }

    // ── Directions ──────────────────────────────────────────────────────

    pub fn directions(&mut self) {
        if self.selected.is_none() {
            return;
        }
        if self.origin.is_none() {
            if self.location.is_some() {
                self.origin = Some(self.here());
            } else {
                self.origin_from_map();
                return;
            }
        }
        self.panel = Panel::Directions;
        self.request_route();
    }

    /// Start from the middle of the map: there is no location service here.
    pub fn origin_from_map(&mut self) {
        let (lat, lon) = self.visible_centre();
        self.origin = Some(Place {
            name: "Map centre".into(),
            detail: format!("{lat:.4}, {lon:.4}"),
            kind: String::new(),
            lat,
            lon,
        });
        self.panel = Panel::Directions;
        self.request_route();
    }

    pub fn swap(&mut self) {
        if let (Some(from), Some(to)) = (self.origin.take(), self.selected.take()) {
            self.origin = Some(to);
            self.selected = Some(from);
            self.request_route();
        }
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.request_route();
    }

    fn request_route(&mut self) {
        let (Some(from), Some(to)) = (self.origin.clone(), self.selected.clone()) else {
            return;
        };
        self.route_seq += 1;
        self.routing = true;
        self.route_error = None;
        let seq = self.route_seq;
        match self.backend() {
            #[cfg(target_os = "macos")]
            Backend::Apple => {
                if let Some(sender) = self.sender.clone() {
                    apple::directions(sender, seq, &from, &to, self.mode);
                }
            }
            backend => {
                if let (Some(job), Some(net)) = (
                    services::route_job(&backend, seq, &from, &to, self.mode),
                    &self.net,
                ) {
                    net.send(self.generation, job);
                }
            }
        }
    }

    fn routed(&mut self, seq: u64, result: Result<Route, String>) {
        if seq != self.route_seq {
            return;
        }
        self.routing = false;
        match result {
            Ok(route) => {
                let line = route.line.clone();
                self.route = Some(route);
                self.fit(&line);
            }
            Err(e) => {
                self.route = None;
                self.route_error = Some(e);
            }
        }
    }

    pub fn show_step(&mut self, at: (f64, f64)) {
        self.fly_to(self.centred(at.0, at.1, self.camera.zoom.max(16.)));
    }

    /// The part of the map the panel doesn't cover, in map points.
    fn open_area(&self) -> [f32; 4] {
        let left = if self.panel_open && self.size[0] > 700. {
            view::PANEL_WIDTH + 32.
        } else {
            0.
        };
        let top = view::TOOLBAR;
        [
            left,
            top,
            (self.size[0] - left).max(1.),
            (self.size[1] - top).max(1.),
        ]
    }

    fn visible_centre(&self) -> (f64, f64) {
        let [x, y, w, h] = self.open_area();
        self.point_at([x + w / 2., y + h / 2.])
    }

    /// The place at a point in the map.
    pub fn point_at(&self, at: [f32; 2]) -> (f64, f64) {
        if !self.camera.flat() {
            let p = self.ground_at(at);
            return unproject([p[0].rem_euclid(1.), p[1].clamp(0., 1.)]);
        }
        let world = self.camera.world();
        unproject([
            (self.camera.center[0] + f64::from(at[0] - self.size[0] / 2.) / world).rem_euclid(1.),
            (self.camera.center[1] + f64::from(at[1] - self.size[1] / 2.) / world).clamp(0., 1.),
        ])
    }

    /// Where a place lands in the map, in points.
    pub fn screen(&self, lat: f64, lon: f64) -> [f32; 2] {
        let p = project(lat, lon);
        if !self.camera.flat() {
            // Too near the camera to draw (far below the map): off-screen.
            let view = perspective::View::new(self.camera, self.size);
            return view
                .project(view.turned_from_mercator(p), 0.)
                .map_or([-100_000., -100_000.], |s| [s[0] as f32, s[1] as f32]);
        }
        let world = self.camera.world();
        let mut dx = p[0] - self.camera.center[0];
        // Take the copy of the world nearest the camera.
        if dx > 0.5 {
            dx -= 1.;
        } else if dx < -0.5 {
            dx += 1.;
        }
        [
            (dx * world) as f32 + self.size[0] / 2.,
            ((p[1] - self.camera.center[1]) * world) as f32 + self.size[1] / 2.,
        ]
    }

    /// Move the camera so the centre of the open area shows what the map's
    /// centre did, rather than the spot hidden under the panel.
    /// Frame a set of points in the open area.
    fn fit(&mut self, points: &[(f64, f64)]) {
        if points.is_empty() {
            return;
        }
        // Measure along the map's own axes (it may be turned), around the
        // first point so the world's seam doesn't split them.
        let first = project(points[0].0, points[0].1);
        let (cos, sin) = (
            self.camera.bearing.to_radians().cos(),
            self.camera.bearing.to_radians().sin(),
        );
        let turned: Vec<[f64; 2]> = points
            .iter()
            .map(|&(lat, lon)| {
                let p = project(lat, lon);
                let dx = (p[0] - first[0] + 0.5).rem_euclid(1.) - 0.5;
                let dy = p[1] - first[1];
                [dx * cos + dy * sin, -dx * sin + dy * cos]
            })
            .collect();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in &turned {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        let [x, y, w, h] = self.open_area();
        // A tilted map shows more ground at the top than at the bottom;
        // leave it more room down the screen.
        let lean = 1. + self.camera.pitch.to_radians().sin();
        let (fw, fh) = (f64::from(w) - 120., (f64::from(h) - 120.) / lean);
        let span = ((x1 - x0) / fw.max(1.))
            .max((y1 - y0) / fh.max(1.))
            .max(1e-12);
        let (mx, my) = ((x0 + x1) / 2., (y0 + y1) / 2.);
        let middle = [
            (first[0] + mx * cos - my * sin).rem_euclid(1.),
            first[1] + mx * sin + my * cos,
        ];
        let to = Camera {
            center: middle,
            zoom: (1. / (span * 512.)).log2().clamp(1., 16.),
            ..self.camera
        };
        let to = under_point(to, self.size, middle, [x + w / 2., y + h / 2.]);
        self.fly_to(to);
    }

    pub fn start(&mut self, sender: UiSender<Msg>) {
        self.net = Some(Net::start(sender.clone()));
        // Follow the system appearance: once a second, compare the mode the
        // theme last saw with the map's.
        let watch = sender.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if watch.send(Msg::AppearanceCheck).is_err() {
                return;
            }
        });
        let frames = self.frames.clone();
        let ticker = sender.clone();
        std::thread::spawn(move || loop {
            if frames.load(std::sync::atomic::Ordering::Relaxed) {
                if ticker.send(Msg::Frame).is_err() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(16));
            } else {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        });
        // MAPS_DEMO: fly to Oslo three seconds after launch.
        if std::env::var_os("MAPS_DEMO").is_some() {
            let demo = sender.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(3));
                let _ = demo.send(Msg::Demo);
            });
        }
        self.sender = Some(sender);
        self.load();
    }

    fn appearance_check(&mut self) {
        let dark = self.appearance.get();
        if dark != self.dark {
            self.dark = dark;
            // OpenStreetMap only has light tiles; everything else switches.
            if self.provider != Provider::OpenStreetMap {
                self.load();
            }
        }
    }

    /// Choose the provider before the window opens.
    pub fn starting_with(mut self, provider: Provider) -> Self {
        self.provider = provider;
        self
    }

    fn key(&self) -> Option<String> {
        self.key_for(self.provider)
    }

    /// A provider's key: typed into Settings, else from its environment
    /// variable.
    pub fn key_for(&self, provider: Provider) -> Option<String> {
        let var = provider.key_variable()?;
        self.keys
            .get(var)
            .cloned()
            .or_else(|| std::env::var(var).ok())
            .filter(|v| !v.trim().is_empty())
    }

    /// Where a provider's key comes from, for Settings.
    pub fn key_source(&self, provider: Provider) -> &'static str {
        match provider.key_variable() {
            None => "",
            Some(var) if self.keys.get(var).is_some_and(|k| !k.trim().is_empty()) => {
                "Saved while Maps is open"
            }
            Some(var) if std::env::var(var).is_ok_and(|k| !k.trim().is_empty()) => {
                "From the environment"
            }
            Some(_) => "Not set",
        }
    }

    /// Save the key typed for a provider and use it at once.
    pub fn save_key(&mut self, provider: Provider) {
        let Some(var) = provider.key_variable() else {
            return;
        };
        let typed = self
            .key_drafts
            .remove(var)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if typed.is_empty() {
            return;
        }
        self.keys.insert(var, typed);
        self.key_changed(provider);
    }

    pub fn forget_key(&mut self, provider: Provider) {
        let Some(var) = provider.key_variable() else {
            return;
        };
        self.keys.remove(var);
        self.key_drafts.remove(var);
        self.key_changed(provider);
    }

    fn key_changed(&mut self, provider: Provider) {
        if provider != self.provider {
            return;
        }
        self.load();
        if self.panel == Panel::Directions {
            self.request_route();
        } else if !self.query.trim().is_empty() && self.panel == Panel::Search {
            self.search_now();
        }
    }

    /// Switch provider: forget the old style and tiles, fetch the new style.
    pub fn use_provider(&mut self, provider: Provider) {
        self.layers_open = false;
        if provider.key_variable().is_some() && self.key_for(provider).is_none() {
            self.settings_open = true;
            self.hint = Some(format!(
                "Add a {} to use {}.",
                provider.key_name().unwrap_or("key"),
                provider.label()
            ));
            return;
        }
        if provider == self.provider {
            return;
        }
        // Image tiles and snapshots can't be turned or tilted yet.
        if !matches!(provider, Provider::OpenFreeMap | Provider::Mapbox) {
            self.camera.bearing = 0.;
            self.camera.pitch = 0.;
        }
        self.provider = provider;
        self.load();
        // Ask the new provider's services again.
        if self.panel == Panel::Directions {
            self.request_route();
        } else if !self.query.trim().is_empty() && self.panel == Panel::Search {
            self.search_now();
        }
    }

    fn load(&mut self) {
        self.generation += 1;
        self.style = None;
        self.tiles.clear();
        self.pending.clear();
        self.failed = 0;
        self.google_session = None;
        self.snapshot = None;
        let Some(net) = self.net.clone() else { return };
        let g = self.generation;
        if let Some(var) = self.provider.key_variable() {
            if self.key().is_none() {
                self.status = format!(
                    "Add a {} in Settings, or set {var}.",
                    self.provider.key_name().unwrap_or("key")
                );
                return;
            }
        }
        self.status = format!("Loading the {} style…", self.provider.label());
        match self.provider {
            Provider::OpenFreeMap => net.send(
                g,
                Job::Text {
                    what: Fetched::Style,
                    url: format!(
                        "https://tiles.openfreemap.org/styles/{}",
                        if self.dark { "dark" } else { "liberty" }
                    ),
                },
            ),
            Provider::OpenStreetMap => {
                self.apply_style(raster_style(
                    "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
                    256.,
                    19,
                    "© OpenStreetMap contributors",
                ));
            }
            Provider::Mapbox => {
                let token = self.key().unwrap_or_default();
                net.send(
                    g,
                    Job::Text {
                        what: Fetched::Style,
                        url: format!(
                            "https://api.mapbox.com/styles/v1/mapbox/{}?access_token={token}",
                            if self.dark { "dark-v11" } else { "streets-v12" }
                        ),
                    },
                );
            }
            Provider::Google => {
                let key = self.key().unwrap_or_default();
                net.send(
                    g,
                    Job::Post {
                        what: Fetched::GoogleSession,
                        url: format!("https://tile.googleapis.com/v1/createSession?key={key}"),
                        body: if self.dark {
                            // Google styles tiles at session creation.
                            r##"{"mapType":"roadmap","language":"en-US","region":"US","scale":"scaleFactor2x","highDpi":true,
                              "styles":[{"stylers":[{"invert_lightness":true},{"saturation":-70},{"lightness":-8}]},
                                        {"featureType":"water","stylers":[{"color":"#0E1A26"}]}]}"##.into()
                        } else {
                            r#"{"mapType":"roadmap","language":"en-US","region":"US","scale":"scaleFactor2x","highDpi":true}"#.into()
                        },
                    },
                );
            }
            Provider::Apple => {
                self.status = "Apple Maps: MapKit snapshots".into();
                self.request_snapshot();
            }
        }
    }

    fn apply_style(&mut self, text: String) {
        let style = match Style::parse(&text) {
            Ok(style) => style,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        let g = self.generation;
        if let Some(net) = &self.net {
            for (id, source) in &style.sources {
                if source.tiles.is_empty() {
                    if let Some(url) = &source.url {
                        net.send(
                            g,
                            Job::Text {
                                what: Fetched::TileJson(id.clone()),
                                url: self.resolve(url, true),
                            },
                        );
                    }
                }
            }
        }
        self.status = format!("{} layers", style.layers.len());
        self.style = Some(style);
        self.request_tiles();
    }

    /// `mapbox://` URLs become Mapbox API URLs; the token goes on every
    /// Mapbox request. Other providers' URLs are used as they are.
    fn resolve(&self, url: &str, tilejson: bool) -> String {
        if let Some(ids) = url.strip_prefix("mapbox://") {
            let token = self.key().unwrap_or_default();
            return if tilejson {
                format!("https://api.mapbox.com/v4/{ids}.json?secure&access_token={token}")
            } else {
                format!("https://api.mapbox.com/{ids}?access_token={token}")
            };
        }
        if url.contains("api.mapbox.com") && !url.contains("access_token") {
            let token = self.key().unwrap_or_default();
            let join = if url.contains('?') { '&' } else { '?' };
            return format!("{url}{join}access_token={token}");
        }
        url.to_owned()
    }

    pub fn update(&mut self, msg: Msg) {
        if std::env::var_os("MAPS_DEBUG").is_some() {
            match &msg {
                Msg::Text(g, what, r) => eprintln!("msg text g{g} {what:?} ok={}", r.is_ok()),
                Msg::Tile(g, key, r) => eprintln!(
                    "msg tile g{g} {}/{}/{} {:?}",
                    key.z,
                    key.x,
                    key.y,
                    r.as_ref().err()
                ),
                Msg::Snapshot(g, req, r) => eprintln!(
                    "msg snapshot g{g} #{req} {:?}",
                    r.as_ref().map(|i| (i.width(), i.height()))
                ),
                Msg::SnapshotDue(..)
                | Msg::SearchDue(..)
                | Msg::SearchWider(..)
                | Msg::LocationTick
                | Msg::AppearanceCheck
                | Msg::Frame
                | Msg::Demo => {}
                Msg::Places(seq, r) => {
                    eprintln!("msg places #{seq} {:?}", r.as_ref().map(Vec::len))
                }
                Msg::Routed(seq, r) => {
                    eprintln!("msg route #{seq} {:?}", r.as_ref().map(|r| r.steps.len()))
                }
            }
        }
        match msg {
            Msg::Text(g, _, _)
            | Msg::Tile(g, _, _)
            | Msg::Snapshot(g, _, _)
            | Msg::SnapshotDue(g, _)
                if g != self.generation => {}
            Msg::SearchDue(seq) => self.run_search(seq, false),
            Msg::SearchWider(seq) => {
                if seq == self.search_seq {
                    self.search_backend(seq);
                }
            }
            Msg::LocationTick => self.location_tick(),
            Msg::AppearanceCheck => self.appearance_check(),
            Msg::Frame => self.frame(),
            Msg::Demo => {
                eprintln!("demo: flying to Oslo");
                self.fly_to(self.centred(59.9139, 10.7522, 12.));
            }
            Msg::Places(seq, result) => self.found(seq, result),
            Msg::Routed(seq, result) => self.routed(seq, result),
            Msg::Text(_, Fetched::Search(seq), result) => {
                let backend = self.backend();
                self.found(
                    seq,
                    result.and_then(|t| services::parse_search(&backend, &t)),
                );
            }
            Msg::Text(_, Fetched::Route(seq), result) => {
                let backend = self.backend();
                self.routed(
                    seq,
                    result.and_then(|t| services::parse_route(&backend, &t)),
                );
            }
            Msg::SnapshotDue(_, request) => {
                // Only the latest request survives a burst of camera changes.
                #[cfg(target_os = "macos")]
                if request == self.snapshot_request {
                    self.take_snapshot();
                }
                #[cfg(not(target_os = "macos"))]
                let _ = request;
            }
            Msg::Text(_, Fetched::Style, result) => match result {
                Ok(text) => self.apply_style(text),
                Err(e) => self.status = format!("Style failed: {e}"),
            },
            Msg::Text(_, Fetched::TileJson(id), result) => {
                let json = result.and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()));
                match json {
                    Ok(json) => {
                        if let Some(source) =
                            self.style.as_mut().and_then(|s| s.sources.get_mut(&id))
                        {
                            source.apply_tilejson(&json);
                        }
                        self.request_tiles();
                    }
                    Err(e) => self.status = format!("Source {id} failed: {e}"),
                }
            }
            Msg::Text(_, Fetched::GoogleSession, result) => {
                let session = result.and_then(|t| {
                    serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string())?
                        ["session"]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            format!(
                                "no session in reply: {}",
                                t.chars().take(160).collect::<String>()
                            )
                        })
                });
                match session {
                    Ok(session) => {
                        let key = self.key().unwrap_or_default();
                        self.google_session = Some(session.clone());
                        self.apply_style(raster_style(
                            &format!("https://tile.googleapis.com/v1/2dtiles/{{z}}/{{x}}/{{y}}?session={session}&key={key}"),
                            256.,
                            22,
                            "Map data © Google",
                        ));
                    }
                    Err(e) => self.status = format!("Google session failed: {e}"),
                }
            }
            Msg::Tile(_, key, result) => {
                self.pending.remove(&key);
                match result {
                    Ok(data) => {
                        self.tiles.insert(key, data);
                    }
                    Err(e) => {
                        self.failed += 1;
                        self.status = format!("Tile {}/{}/{} failed: {e}", key.z, key.x, key.y);
                    }
                }
            }
            Msg::Snapshot(_, request, result) => {
                if request == self.snapshot_request {
                    match result {
                        Ok(image) => {
                            self.snapshot = Some((image, self.camera));
                            self.status = "Apple Maps: MapKit snapshot".into();
                        }
                        Err(e) => self.status = format!("MapKit: {e}"),
                    }
                }
            }
        }
    }

    /// Tiles of one source that cover the view at the current zoom.
    pub fn placements(&self, source_id: &str) -> Vec<Placement> {
        self.placements_for(source_id, self.camera)
    }

    /// The tiles that cover the view from a given camera.
    pub fn placements_for(&self, source_id: &str, camera: Camera) -> Vec<Placement> {
        let Some(source) = self.style.as_ref().and_then(|s| s.sources.get(source_id)) else {
            return Vec::new();
        };
        if source.tiles.is_empty() || self.size[0] < 1. || self.size[1] < 1. {
            return Vec::new();
        }
        // Vector tiles use the floor zoom. Images use the next level up, so
        // each is drawn at most at its own size: sharp, and never larger than
        // the map.
        let level = camera.zoom + (512. / source.tile_size).log2();
        if !camera.flat() {
            return self.placements_tilted(source_id, source, camera, level.floor());
        }
        let level = if source.kind == SourceKind::Vector {
            level.floor()
        } else {
            level.ceil()
        };
        let z = level.clamp(f64::from(source.min_zoom), f64::from(source.max_zoom)) as u8;
        let n = f64::from(1u32 << z);
        let world = camera.world();
        let size = world / n;
        let left = camera.center[0] * world - f64::from(self.size[0]) / 2.;
        let top = camera.center[1] * world - f64::from(self.size[1]) / 2.;
        let (x0, x1) = (
            (left / size).floor() as i64,
            ((left + f64::from(self.size[0])) / size).floor() as i64,
        );
        let (y0, y1) = (
            (top / size).floor().max(0.) as i64,
            ((top + f64::from(self.size[1])) / size).floor().min(n - 1.) as i64,
        );
        let mut out = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let wrapped = x.rem_euclid(n as i64) as u32;
                out.push(Placement {
                    key: TileKey {
                        source: source_id.to_owned(),
                        z,
                        x: wrapped,
                        y: y as u32,
                    },
                    origin: [x as f64 * size - left, y as f64 * size - top],
                    size,
                });
            }
        }
        out
    }

    /// The tiles under a turned or tilted view: every tile that touches the
    /// ground in view, split while it would be drawn larger than its own
    /// size, so the far side of a tilted map uses coarser (and fewer) tiles.
    /// Origins are where a flat, north-up map would draw each tile.
    fn placements_tilted(
        &self,
        source_id: &str,
        source: &style::Source,
        camera: Camera,
        finest: f64,
    ) -> Vec<Placement> {
        let view = perspective::View::new(camera, self.size);
        let visible = view.visible(32.);
        if visible.len() < 3 {
            return Vec::new();
        }
        let world = camera.world();
        let [w, h] = [f64::from(self.size[0]), f64::from(self.size[1])];
        let left = camera.center[0] * world - w / 2.;
        let top = camera.center[1] * world - h / 2.;
        let flat: Vec<[f64; 2]> = visible
            .iter()
            .map(|&r| {
                let d = view.unturn(r);
                [d[0] + w / 2., d[1] + h / 2.]
            })
            .collect();
        let min_x = flat.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        let max_x = flat.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        let (lo, hi) = (f64::from(source.min_zoom), f64::from(source.max_zoom));
        let finest = finest.clamp(lo, hi);
        let base = camera.zoom + (512. / source.tile_size).log2();
        // Start from each copy of the world in view; x is not wrapped until a
        // tile is chosen.
        let mut stack: Vec<(u8, i64, i64)> = (((left + min_x) / world).floor() as i64
            ..=((left + max_x) / world).floor() as i64)
            .map(|copy| (0u8, copy, 0i64))
            .collect();
        let mut out = Vec::new();
        while let Some((z, x, y)) = stack.pop() {
            let n = 1i64 << z;
            let size = world / n as f64;
            let origin = [x as f64 * size - left, y as f64 * size - top];
            let corners = [
                origin,
                [origin[0] + size, origin[1]],
                [origin[0] + size, origin[1] + size],
                [origin[0], origin[1] + size],
            ]
            .map(|f| view.turned_from_flat(f));
            let part = perspective::clip_polygon(&corners, &visible);
            if part.len() < 3 {
                continue;
            }
            let nearest = part.iter().map(|&r| view.scale(r)).fold(0., f64::max);
            let level = (base + nearest.log2()).floor().clamp(lo, finest);
            if f64::from(z) < level {
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    stack.push((z + 1, x * 2 + dx, y * 2 + dy));
                }
                continue;
            }
            out.push(Placement {
                key: TileKey {
                    source: source_id.to_owned(),
                    z,
                    x: x.rem_euclid(n) as u32,
                    y: y as u32,
                },
                origin,
                size,
            });
            if out.len() >= 400 {
                break;
            }
        }
        // Coarse first, so the near (sharper) tiles are drawn over them.
        out.sort_by_key(|p| p.key.z);
        out
    }

    /// Ask for every visible tile that isn't loaded or on its way.
    pub fn request_tiles(&mut self) {
        if self.provider == Provider::Apple {
            self.request_snapshot();
            return;
        }
        self.request_tiles_for(self.camera);
    }

    /// Ask for the tiles a camera would show (a flight asks for where it is
    /// going before it gets there).
    pub fn request_tiles_for(&mut self, camera: Camera) {
        if self.provider == Provider::Apple {
            return;
        }
        let (Some(style), Some(net)) = (&self.style, &self.net) else {
            return;
        };
        let used: BTreeSet<&str> = style
            .layers
            .iter()
            .filter_map(|l| l.source.as_deref())
            .collect();
        let mut jobs = Vec::new();
        for (id, source) in &style.sources {
            if !used.contains(id.as_str()) || source.tiles.is_empty() {
                continue;
            }
            for p in self.placements_for(id, camera) {
                if self.tiles.contains_key(&p.key) || self.pending.contains(&p.key) {
                    continue;
                }
                let template =
                    &source.tiles[(p.key.x as usize + p.key.y as usize) % source.tiles.len()];
                let url = self.resolve(
                    &template
                        .replace("{z}", &p.key.z.to_string())
                        .replace("{x}", &p.key.x.to_string())
                        .replace("{y}", &p.key.y.to_string())
                        .replace("{ratio}", "@2x"),
                    false,
                );
                jobs.push((p.key, url, source.kind == SourceKind::Vector));
            }
        }
        for (key, url, vector) in jobs {
            self.pending.insert(key.clone());
            net.send(self.generation, Job::Tile { key, url, vector });
        }
    }

    /// Ask for a snapshot once the camera has been still for a moment, so a
    /// scroll or a resize asks MapKit once instead of on every step.
    #[cfg(target_os = "macos")]
    fn request_snapshot(&mut self) {
        let Some(sender) = self.sender.clone() else {
            return;
        };
        self.snapshot_request += 1;
        let (g, request) = (self.generation, self.snapshot_request);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = sender.send(Msg::SnapshotDue(g, request));
        });
    }

    #[cfg(target_os = "macos")]
    fn take_snapshot(&mut self) {
        if std::env::var_os("MAPS_DEBUG").is_some() {
            eprintln!("snapshot taken: size {:?}", self.size);
        }
        if self.size[0] < 1. || self.size[1] < 1. {
            return;
        }
        let Some(sender) = self.sender.clone() else {
            return;
        };
        apple::snapshot(
            sender,
            self.generation,
            self.snapshot_request,
            self.camera,
            self.size,
            self.dark,
        );
    }
    #[cfg(not(target_os = "macos"))]
    fn request_snapshot(&mut self) {
        self.status = "Apple Maps needs MapKit, which only runs on Apple platforms.".into();
    }

    /// Headless: download the style and every visible tile before a
    /// snapshot, without worker threads.
    pub fn prefetch(&mut self, provider: Provider, size: [f32; 2]) {
        let net = Net::queue();
        self.net = Some(net);
        self.size = size;
        self.provider = provider;
        self.load();
        self.drain();
        #[cfg(target_os = "macos")]
        if provider == Provider::Apple {
            let shot = apple::take_blocking(self.camera, size, self.dark);
            self.update(Msg::Snapshot(self.generation, self.snapshot_request, shot));
        }
    }

    /// Headless: set up a flight like `fly_to` (fetching its destination and
    /// the top of its arc), then put the camera at time `t` along it.
    pub fn flight_at(&mut self, to: Camera, t: f64) {
        let from = self.camera;
        let mut dx = to.center[0] - from.center[0];
        if dx > 0.5 {
            dx -= 1.;
        } else if dx < -0.5 {
            dx += 1.;
        }
        let dy = to.center[1] - from.center[1];
        let far =
            (dx.hypot(dy) * from.world()) / f64::from(self.size[0].hypot(self.size[1]).max(1.));
        let lift = if far > 1. {
            far.log2().min(6.) * 0.9 + 0.5
        } else {
            0.
        };
        let to = Camera {
            center: [from.center[0] + dx, to.center[1]],
            ..to
        };
        self.prefetch_flight(from, to, lift);
        self.drain();
        self.camera = flight(from, to, lift, t);
    }

    /// Before a flight: its destination first, then coarse tiles (three
    /// levels below the camera: few, small and quick) along the way, so the
    /// map stays filled while it moves.
    fn prefetch_flight(&mut self, from: Camera, to: Camera, lift: f64) {
        self.request_tiles_for(to);
        if lift > 0. {
            for t in [0.15, 0.3, 0.45, 0.6, 0.75, 0.9] {
                let mut at = flight(from, to, lift, t);
                at.zoom = (at.zoom - 3.).max(1.);
                self.request_tiles_for(at);
            }
        }
    }

    pub fn oslo(&self) -> Camera {
        self.centred(59.9139, 10.7522, 12.)
    }

    /// Headless: run queued downloads (and the ones their results queue).
    pub fn drain(&mut self) {
        let Some(net) = self.net.clone() else { return };
        let agent = net::agent();
        while let Some((g, job)) = net.next() {
            let msg = net::run(&agent, g, job);
            self.update(msg);
        }
    }

    pub fn zoom_by(&mut self, delta: f64) {
        let centre = [self.size[0] / 2., self.size[1] / 2.];
        // Steps land on whole levels, as a zoom button should.
        let to = match self.motion {
            Some(Motion::Zoom { to, .. }) => to + delta,
            _ => (self.camera.zoom + delta).round(),
        };
        self.zoom_smoothly(centre, to);
    }

    /// Zoom so the place under `at` (a point in the map) stays put.
    pub fn zoom_at(&mut self, at: [f32; 2], zoom: f64) {
        let zoom = zoom.clamp(1., 19.);
        let under = self.ground_at(at);
        self.camera.zoom = zoom;
        self.keep_under(under, at);
        self.request_tiles();
    }

    fn click(&mut self, at: [f32; 2]) {
        self.layers_open = false;
        self.settings_open = false;
        let near = self
            .results
            .iter()
            .map(|place| {
                let [x, y] = self.screen(place.lat, place.lon);
                (
                    ((x - at[0]).powi(2) + (y - at[1] + 14.).powi(2)).sqrt(),
                    place,
                )
            })
            .filter(|(d, _)| *d < 18.)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, place)) = near {
            self.select(place.clone());
        }
    }

    /// Drag pans; the wheel, a two-finger scroll and a pinch zoom around the
    /// pointer; a double click zooms in there.
    pub fn pointer(&mut self, p: volt_ui_kit::declarative::Pointer) {
        use volt_ui_kit::declarative::PointerPhase;
        if std::env::var_os("MAPS_DEBUG").is_some()
            && !matches!(p.phase, PointerPhase::Hover | PointerPhase::Drag)
        {
            eprintln!(
                "pointer {:?} at {:?} delta {:?} zoom {} clicks {}",
                p.phase, p.at, p.delta, p.zoom, p.clicks
            );
        }
        if matches!(
            p.phase,
            PointerPhase::Press | PointerPhase::Wheel | PointerPhase::Pinch
        ) {
            // Touching the map stops whatever it was doing.
            self.set_motion(None);
        }
        match p.phase {
            PointerPhase::Press if p.clicks >= 2 => {
                self.drag_from = None;
                self.orbit = None;
                self.zoom_smoothly(p.at, self.camera.zoom.floor() + 1.);
            }
            // Option-, Control- or Command-drag turns (across) and tilts
            // (up and down), as in other map apps.
            PointerPhase::Press if p.modifiers.alt || p.modifiers.control || p.modifiers.meta => {
                self.drag_from = None;
                self.hint = None;
                if self.can_tilt() {
                    let [x, y, w, h] = self.open_area();
                    let anchor = [x + w / 2., y + h / 2.];
                    self.orbit = Some((self.camera, self.ground_at(anchor), anchor));
                } else {
                    self.hint = Some("Turning and tilting need a vector map. Choose OpenFreeMap under Map style.".into());
                }
            }
            PointerPhase::Press => {
                self.orbit = None;
                self.hint = None;
                self.drag_from = Some(self.ground_at(p.at));
                self.drag_trail.clear();
            }
            PointerPhase::Drag if self.orbit.is_some() => {
                if let Some((start, under, anchor)) = self.orbit {
                    self.camera.bearing =
                        (start.bearing - f64::from(p.offset[0]) * 0.4).rem_euclid(360.);
                    self.camera.pitch = (start.pitch - f64::from(p.offset[1]) * 0.3)
                        .clamp(0., perspective::MAX_PITCH);
                    self.keep_under(under, anchor);
                    self.request_tiles();
                }
            }
            PointerPhase::Drag => {
                let now = std::time::Instant::now();
                self.drag_trail.push((now, p.offset));
                self.drag_trail
                    .retain(|(t, _)| now.duration_since(*t).as_millis() < 100);
                if let Some(from) = self.drag_from {
                    self.keep_under(from, p.at);
                    // Fetch what comes into view while dragging; Apple waits
                    // for the release, since each snapshot is a whole image.
                    if self.provider != Provider::Apple {
                        self.request_tiles();
                    }
                }
            }
            PointerPhase::Release if self.orbit.is_some() => {
                self.orbit = None;
                // Close to north and flat snaps there.
                if turn_between(self.camera.bearing, 0.).abs() < 4. {
                    self.camera.bearing = 0.;
                }
                if self.camera.pitch < 2. {
                    self.camera.pitch = 0.;
                }
                self.request_tiles();
            }
            PointerPhase::Release => {
                if self.drag_from.take().is_some() {
                    // A press that barely moved is a click: pick a pin.
                    if p.offset[0].abs() < 4. && p.offset[1].abs() < 4. {
                        self.click(p.at);
                    } else if let (Some(&(t0, a)), Some(&(t1, b))) =
                        (self.drag_trail.first(), self.drag_trail.last())
                    {
                        // Let go while moving: keep gliding.
                        let ms = t1.duration_since(t0).as_secs_f64() * 1000.;
                        let recent = std::time::Instant::now().duration_since(t1).as_millis() < 60;
                        if ms > 4. && recent {
                            let velocity =
                                [f64::from(b[0] - a[0]) / ms, f64::from(b[1] - a[1]) / ms];
                            if velocity[0].hypot(velocity[1]) > 0.25 {
                                self.set_motion(Some(Motion::Glide {
                                    velocity,
                                    last: std::time::Instant::now(),
                                }));
                            }
                        }
                    }
                    self.drag_trail.clear();
                    self.request_tiles();
                }
            }
            PointerPhase::Wheel => {
                // At most half a level per event, so a fast flick (and the
                // momentum after it) stays controllable.
                let step = -f64::from(p.delta[1]) / 240.;
                if step != 0. {
                    self.zoom_at(p.at, self.camera.zoom + step.clamp(-0.5, 0.5));
                }
            }
            PointerPhase::Pinch => {
                let factor = 1. + f64::from(p.zoom);
                if factor > 0. {
                    self.zoom_at(p.at, self.camera.zoom + factor.log2());
                }
            }
            _ => {}
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// A one-layer style for an image tile service.
fn raster_style(url: &str, tile_size: f64, max_zoom: u8, attribution: &str) -> String {
    serde_json::json!({
        "version": 8,
        "sources": {"base": {"type": "raster", "tiles": [url], "tileSize": tile_size,
                             "maxzoom": max_zoom, "attribution": attribution}},
        "layers": [{"id": "base", "type": "raster", "source": "base"}]
    })
    .to_string()
}

fn rgba(v: u32) -> [f32; 4] {
    [
        ((v >> 16) & 255) as f32 / 255.,
        ((v >> 8) & 255) as f32 / 255.,
        (v & 255) as f32 / 255.,
        1.,
    ]
}

/// The window follows the system appearance, and the map with it.
pub fn theme(app: &App, system: ThemeMode) -> UiTheme {
    let dark = system == ThemeMode::Dark;
    app.appearance.set(dark);
    if dark {
        let mut palette = Palette::dark();
        palette.base = [rgba(0x16191E), rgba(0x22262D), rgba(0x2C313A)];
        palette.content = rgba(0xF3F4F6);
        palette.muted = rgba(0x9CA3AF);
        palette.border = rgba(0x30353D);
        UiTheme::from_palette(palette.with_accent(rgba(0x3B82F6)))
    } else {
        let mut palette = Palette::light();
        palette.base = [rgba(0xFFFFFF), rgba(0xF3F4F6), rgba(0xE5E7EB)];
        palette.content = rgba(0x111827);
        palette.muted = rgba(0x6B7280);
        palette.border = rgba(0xE5E7EB);
        UiTheme::from_palette(palette.with_accent(rgba(0x2563EB)))
    }
}

/// The appearance at launch, before the window reports one.
/// Miles where the system measures in inches (macOS), else kilometres.
fn system_units() -> Units {
    let imperial = cfg!(target_os = "macos")
        && std::process::Command::new("/usr/bin/defaults")
            .args(["read", "-g", "AppleMetricUnits"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "0");
    if imperial {
        Units::Imperial
    } else {
        Units::Metric
    }
}

fn system_is_dark() -> bool {
    cfg!(target_os = "macos")
        && std::process::Command::new("/usr/bin/defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "Dark")
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn app() -> App {
        let mut app = App::new();
        app.size = [1280., 820.];
        app
    }

    #[test]
    fn a_flight_lands_on_its_target() {
        let mut app = app();
        let to = Camera::at(59.91, 10.75, 13.);
        app.set_motion(Some(Motion::Fly {
            from: app.camera,
            to,
            start: Instant::now() - Duration::from_secs(5),
            ms: 600.,
            lift: 3.,
        }));
        app.frame();
        assert!(app.motion.is_none());
        assert!((app.camera.zoom - to.zoom).abs() < 1e-9);
        assert!(
            (app.camera.center[0] - to.center[0]).abs() < 1e-9
                && (app.camera.center[1] - to.center[1]).abs() < 1e-9
        );
    }

    #[test]
    fn a_smooth_zoom_keeps_the_anchor_still() {
        let mut app = app();
        let anchor = [300., 250.];
        let world = app.camera.world();
        let under = [
            app.camera.center[0] + f64::from(anchor[0] - 640.) / world,
            app.camera.center[1] + f64::from(anchor[1] - 410.) / world,
        ];
        app.set_motion(Some(Motion::Zoom {
            from: app.camera.zoom,
            to: app.camera.zoom + 1.,
            anchor,
            under,
            start: Instant::now() - Duration::from_millis(100),
            ms: 260.,
        }));
        app.frame();
        let world = app.camera.world();
        let now_under = [
            app.camera.center[0] + f64::from(anchor[0] - 640.) / world,
            app.camera.center[1] + f64::from(anchor[1] - 410.) / world,
        ];
        assert!(app.motion.is_some(), "still easing at 100 of 260 ms");
        assert!((under[0] - now_under[0]).abs() < 1e-12 && (under[1] - now_under[1]).abs() < 1e-12);
    }

    #[test]
    fn turning_and_dragging_keep_their_ground_in_place() {
        let mut app = app();
        app.camera.pitch = 50.;
        app.camera.bearing = 25.;
        let [x, y, w, h] = app.open_area();
        let middle = [x + w / 2., y + h / 2.];
        let before = app.ground_at(middle);
        app.sender = None;
        app.turn_to(140., 30.);
        let after = app.ground_at(middle);
        assert!((before[0] - after[0]).abs() < 1e-9 && (before[1] - after[1]).abs() < 1e-9);
        assert!((app.camera.bearing - 140.).abs() < 1e-9 && (app.camera.pitch - 30.).abs() < 1e-9);
        // A drag on a tilted map keeps the grabbed ground under the pointer.
        let grabbed = app.ground_at([300., 600.]);
        app.keep_under(grabbed, [500., 300.]);
        let now = app.ground_at([500., 300.]);
        assert!((grabbed[0] - now[0]).abs() < 1e-9 && (grabbed[1] - now[1]).abs() < 1e-9);
        // A place framed on a turned, tilted map lands in the open area.
        let to = app.centred(40.7484, -73.9857, 16.);
        app.camera = to;
        let at = app.screen(40.7484, -73.9857);
        assert!(
            (at[0] - middle[0]).abs() < 0.01 && (at[1] - middle[1]).abs() < 0.01,
            "{at:?} vs {middle:?}"
        );
        assert!(
            (turn_between(350., 10.) - 20.).abs() < 1e-9
                && (turn_between(10., 350.) + 20.).abs() < 1e-9
        );
    }

    #[test]
    fn a_glide_slows_to_a_stop() {
        let mut app = app();
        let start = app.camera.center;
        app.set_motion(Some(Motion::Glide {
            velocity: [1.5, 0.],
            last: Instant::now() - Duration::from_millis(16),
        }));
        app.frame();
        assert!(
            app.camera.center[0] < start[0],
            "dragging right moves the map left"
        );
        for _ in 0..400 {
            if app.motion.is_none() {
                break;
            }
            if let Some(Motion::Glide { velocity, .. }) = app.motion {
                app.motion = Some(Motion::Glide {
                    velocity,
                    last: Instant::now() - Duration::from_millis(16),
                });
            }
            app.frame();
        }
        assert!(app.motion.is_none(), "the glide never stopped");
    }
}
