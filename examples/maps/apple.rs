//! Apple Maps through MapKit snapshots (macOS). Apple offers no tile service
//! for other renderers, so MapKit renders a still image of exactly the area
//! the camera shows, and Volt draws that image. A new snapshot is taken when
//! the camera settles; while you drag, the last one moves with the camera.
use super::{net::Msg, unproject, Camera};
use block2::RcBlock;
use objc2::AnyThread;
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSBitmapImageFileType,
    NSBitmapImageRep,
};
use objc2_core_location::CLLocationCoordinate2D;
use objc2_foundation::{NSDate, NSDictionary, NSError, NSRunLoop, NSSize};
use objc2_map_kit::{
    MKCoordinateRegion, MKCoordinateSpan, MKMapSnapshot, MKMapSnapshotOptions, MKMapSnapshotter,
};
use std::{cell::RefCell, rc::Rc, time::Instant};
use volt_ui_kit::{graphics::Image, native::UiSender};

pub fn snapshot(
    sender: UiSender<Msg>,
    generation: u64,
    request: u64,
    camera: Camera,
    size: [f32; 2],
    dark: bool,
) {
    take(camera, size, dark, move |result| {
        let _ = sender.send(Msg::Snapshot(generation, request, result));
    });
}

/// Headless renders have no app run loop, so run one until MapKit answers.
pub fn take_blocking(camera: Camera, size: [f32; 2], dark: bool) -> Result<Image, String> {
    let slot: Rc<RefCell<Option<Result<Image, String>>>> = Rc::default();
    let out = slot.clone();
    take(camera, size, dark, move |result| {
        *out.borrow_mut() = Some(result)
    });
    let started = Instant::now();
    let run_loop = NSRunLoop::currentRunLoop();
    while slot.borrow().is_none() && started.elapsed().as_secs() < 30 {
        run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.05));
    }
    slot.take()
        .unwrap_or_else(|| Err("MapKit did not answer in 30 s".into()))
}

/// The area the camera shows, as MapKit expects it: a centre and spans in
/// degrees. MapKit throws (and Rust aborts) on a region outside the world,
/// which a far-out zoom asks for, so the area is kept within it.
pub fn region(camera: Camera, size: [f32; 2]) -> Option<MKCoordinateRegion> {
    let world = camera.world();
    let half = [
        f64::from(size[0]) / 2. / world,
        f64::from(size[1]) / 2. / world,
    ];
    let top = (camera.center[1] - half[1]).clamp(0., 1.);
    let bottom = (camera.center[1] + half[1]).clamp(0., 1.);
    let (north, _) = unproject([camera.center[0], top]);
    let (south, _) = unproject([camera.center[0], bottom]);
    let (_, lon) = camera.lat_lon();
    let lat_span = (north - south).clamp(0.000_1, 170.);
    let lon_span = (half[0] * 2. * 360.).clamp(0.000_1, 360.);
    let center_lat = (north + south) / 2.;
    let valid = [lat_span, lon_span, center_lat, lon]
        .iter()
        .all(|v| v.is_finite())
        && center_lat.abs() + lat_span / 2. <= 90.
        && lon.abs() <= 180.;
    valid.then_some(MKCoordinateRegion {
        center: CLLocationCoordinate2D {
            latitude: center_lat,
            longitude: lon,
        },
        span: MKCoordinateSpan {
            latitudeDelta: lat_span,
            longitudeDelta: lon_span,
        },
    })
}

/// Start a snapshot of the camera's view; `done` runs on the main thread.
fn take(
    camera: Camera,
    size: [f32; 2],
    dark: bool,
    done: impl Fn(Result<Image, String>) + 'static,
) {
    let Some(region) = region(camera, size) else {
        done(Err("this view is outside what MapKit can show".into()));
        return;
    };
    if !(size[0] >= 1. && size[1] >= 1. && size[0] <= 8192. && size[1] <= 8192.) {
        done(Err("invalid snapshot size".into()));
        return;
    }
    let options = unsafe { MKMapSnapshotOptions::new() };
    unsafe {
        options.setRegion(region);
        options.setSize(NSSize::new(f64::from(size[0]), f64::from(size[1])));
        let name = if dark {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        };
        options.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
    }
    let snapshotter =
        unsafe { MKMapSnapshotter::initWithOptions(MKMapSnapshotter::alloc(), &options) };
    // The block keeps the snapshotter alive until MapKit calls back.
    let keep = snapshotter.clone();
    let block = RcBlock::new(move |snapshot: *mut MKMapSnapshot, error: *mut NSError| {
        let _alive = &keep;
        let result = guarded(|| match unsafe { snapshot.as_ref() } {
            Some(snapshot) => to_image(snapshot),
            None => Err(error_text(error)),
        });
        done(result);
    });
    unsafe { snapshotter.startWithCompletionHandler(RcBlock::as_ptr(&block)) };
}

fn to_image(snapshot: &MKMapSnapshot) -> Result<Image, String> {
    let image = unsafe { snapshot.image() };
    let tiff = image.TIFFRepresentation().ok_or("no image data")?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff).ok_or("unreadable snapshot")?;
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .ok_or("PNG encoding failed")?;
    volt_ui_kit::native::decode_image(&png.to_vec()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regions_stay_inside_what_mapkit_accepts() {
        for zoom in [1., 1.5, 2., 4., 8., 12., 19.] {
            for lat in [-85., -60., 0., 45., 69.6, 85.] {
                for lon in [-180., -179.9, 0., 18.9, 179.9] {
                    let camera = Camera::at(lat, lon, zoom);
                    let r = region(camera, [1200., 712.]).expect("a valid region");
                    assert!(r.span.latitudeDelta > 0. && r.span.latitudeDelta <= 180.);
                    assert!(r.span.longitudeDelta > 0. && r.span.longitudeDelta <= 360.);
                    assert!(r.center.latitude.abs() + r.span.latitudeDelta / 2. <= 90.);
                    assert!(r.center.longitude.abs() <= 180.);
                }
            }
        }
    }
}

// ── Search and directions (MapKit) ─────────────────────────────────────────

use super::services::{Mode, Place, Route, Step, Turn};
use objc2_core_location::CLLocation;
use objc2_foundation::{NSRange, NSString};
use objc2_map_kit::{
    MKDirections, MKDirectionsRequest, MKDirectionsResponse, MKDirectionsTransportType,
    MKLocalSearch, MKLocalSearchRequest, MKLocalSearchResponse, MKMapItem, MKPolyline,
};

/// Run a MapKit callback's work so that a panic becomes an error message:
/// unwinding out of a block into MapKit would abort the app.
fn guarded<T>(work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
        .unwrap_or_else(|_| Err("MapKit returned something unexpected.".into()))
}

fn error_text(error: *mut NSError) -> String {
    unsafe { error.as_ref() }
        .map(|e| e.localizedDescription().to_string())
        .unwrap_or_else(|| "MapKit gave no answer".into())
}

/// MapKit's local search, biased to the area on screen.
pub fn search(sender: UiSender<Msg>, seq: u64, query: &str, camera: Camera, size: [f32; 2]) {
    let request = unsafe { MKLocalSearchRequest::new() };
    unsafe {
        request.setNaturalLanguageQuery(Some(&NSString::from_str(query)));
        if let Some(r) = region(camera, size) {
            request.setRegion(r);
        }
    }
    let search = unsafe { MKLocalSearch::initWithRequest(MKLocalSearch::alloc(), &request) };
    let keep = search.clone();
    let block = RcBlock::new(
        move |response: *mut MKLocalSearchResponse, error: *mut NSError| {
            let _alive = &keep;
            let result = guarded(|| match unsafe { response.as_ref() } {
                Some(r) => Ok(unsafe { r.mapItems() }
                    .iter()
                    .filter_map(|item| place(&item))
                    .collect()),
                None => Err(error_text(error)),
            });
            let _ = sender.send(Msg::Places(seq, result));
        },
    );
    unsafe { search.startWithCompletionHandler(RcBlock::as_ptr(&block)) };
}

fn place(item: &MKMapItem) -> Option<Place> {
    let c = unsafe { item.location().coordinate() };
    let detail = unsafe { item.address() }
        .map(|a| unsafe { a.fullAddress() }.to_string().replace('\n', ", "))
        .unwrap_or_default();
    let kind = unsafe { item.pointOfInterestCategory() }
        .map(|c| {
            let raw = c.to_string();
            let raw = raw.trim_start_matches("MKPOICategory");
            // "PublicTransport" → "Public transport".
            let mut out = String::new();
            for (i, ch) in raw.chars().enumerate() {
                if i > 0 && ch.is_uppercase() {
                    out.push(' ');
                    out.extend(ch.to_lowercase());
                } else {
                    out.push(ch);
                }
            }
            out
        })
        .unwrap_or_default();
    Some(Place {
        name: unsafe { item.name() }
            .map(|n| n.to_string())
            .unwrap_or_default(),
        detail,
        kind,
        lat: c.latitude,
        lon: c.longitude,
    })
}

/// MapKit directions between two places.
pub fn directions(sender: UiSender<Msg>, seq: u64, from: &Place, to: &Place, mode: Mode) {
    let item = |p: &Place| {
        let location =
            unsafe { CLLocation::initWithLatitude_longitude(CLLocation::alloc(), p.lat, p.lon) };
        unsafe { MKMapItem::initWithLocation_address(MKMapItem::alloc(), &location, None) }
    };
    let request = unsafe { MKDirectionsRequest::new() };
    unsafe {
        request.setSource(Some(&item(from)));
        request.setDestination(Some(&item(to)));
        request.setTransportType(match mode {
            Mode::Drive => MKDirectionsTransportType::Automobile,
            Mode::Walk => MKDirectionsTransportType::Walking,
            Mode::Cycle => MKDirectionsTransportType::Cycling,
        });
    }
    let directions = unsafe { MKDirections::initWithRequest(MKDirections::alloc(), &request) };
    let keep = directions.clone();
    let block = RcBlock::new(
        move |response: *mut MKDirectionsResponse, error: *mut NSError| {
            let _alive = &keep;
            let result = guarded(|| match unsafe { response.as_ref() } {
                Some(r) => match unsafe { r.routes() }.firstObject() {
                    Some(route) => {
                        let polyline = unsafe { route.polyline() };
                        let line = coordinates(&polyline);
                        let mut steps: Vec<Step> = unsafe { route.steps() }
                            .iter()
                            .filter_map(|s| {
                                // MapKit can leave a step's text out (nil);
                                // the generated getter would panic on that.
                                let text: Option<Retained<NSString>> =
                                    unsafe { objc2::msg_send![&*s, instructions] };
                                let text = text.map(|t| t.to_string()).unwrap_or_default();
                                let polyline = unsafe { s.polyline() };
                                let at = coordinates(&polyline).first().copied()?;
                                (!text.is_empty()).then(|| Step {
                                    turn: turn_from(&text),
                                    text,
                                    distance: unsafe { s.distance() },
                                    at,
                                })
                            })
                            .collect();
                        if let Some(first) = steps.first_mut() {
                            if first.turn == Turn::Straight {
                                first.turn = Turn::Depart;
                            }
                        }
                        Ok(Route {
                            distance: unsafe { route.distance() },
                            duration: unsafe { route.expectedTravelTime() },
                            line,
                            steps,
                        })
                    }
                    None => Err("No route between these places.".into()),
                },
                None => Err(error_text(error)),
            });
            let _ = sender.send(Msg::Routed(seq, result));
        },
    );
    unsafe { directions.calculateDirectionsWithCompletionHandler(RcBlock::as_ptr(&block)) };
}

fn coordinates(line: &MKPolyline) -> Vec<(f64, f64)> {
    let count = unsafe { line.pointCount() };
    if count == 0 {
        return Vec::new();
    }
    let mut points = vec![
        CLLocationCoordinate2D {
            latitude: 0.,
            longitude: 0.
        };
        count
    ];
    unsafe {
        line.getCoordinates_range(
            std::ptr::NonNull::new(points.as_mut_ptr()).unwrap(),
            NSRange::new(0, count),
        )
    };
    points.iter().map(|c| (c.latitude, c.longitude)).collect()
}

/// MapKit gives instruction text, not manoeuvre types; read the turn from it.
fn turn_from(text: &str) -> Turn {
    let t = text.to_lowercase();
    if t.contains("destination") || t.starts_with("arrive") {
        Turn::Arrive
    } else if t.contains("roundabout") {
        Turn::Roundabout
    } else if t.contains("u-turn") {
        Turn::UTurn
    } else if t.contains("sharp left") {
        Turn::SharpLeft
    } else if t.contains("sharp right") {
        Turn::SharpRight
    } else if t.contains("slight left") || t.contains("keep left") || t.contains("bear left") {
        Turn::SlightLeft
    } else if t.contains("slight right") || t.contains("keep right") || t.contains("bear right") {
        Turn::SlightRight
    } else if t.contains("left") {
        Turn::Left
    } else if t.contains("right") {
        Turn::Right
    } else {
        Turn::Straight
    }
}

// ── Current location (Core Location) ───────────────────────────────────────

use objc2::rc::Retained;
use objc2_core_location::{CLAuthorizationStatus, CLLocationManager};

/// Where the person is, as far as macOS will say.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Located {
    /// Waiting for permission or for a first fix.
    Waiting,
    /// Latitude, longitude and accuracy in metres.
    At(f64, f64, f64),
    Denied,
}

/// Asks once for permission, then keeps the latest fix. The app reads it on
/// a timer, so no delegate object is needed.
pub struct Locator {
    manager: Retained<CLLocationManager>,
}

impl Locator {
    pub fn start() -> Self {
        let manager = unsafe { CLLocationManager::new() };
        unsafe {
            manager.requestWhenInUseAuthorization();
            manager.startUpdatingLocation();
        }
        Self { manager }
    }

    pub fn read(&self) -> Located {
        let status = unsafe { self.manager.authorizationStatus() };
        if status == CLAuthorizationStatus::Denied || status == CLAuthorizationStatus::Restricted {
            return Located::Denied;
        }
        match unsafe { self.manager.location() } {
            Some(fix) => {
                let c = unsafe { fix.coordinate() };
                Located::At(c.latitude, c.longitude, unsafe { fix.horizontalAccuracy() })
            }
            None => Located::Waiting,
        }
    }
}

impl Drop for Locator {
    fn drop(&mut self) {
        unsafe { self.manager.stopUpdatingLocation() };
    }
}
