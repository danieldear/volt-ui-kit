//! Search and directions, from whichever provider draws the map: open
//! servers (Photon search, FOSSGIS OSRM routing) for the open maps, the
//! Mapbox and Google APIs with your keys, and MapKit for Apple (apple.rs).
//! Every backend fills in the same `Place` and `Route`.
use super::net::{Fetched, Job};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    /// The address or area, for the second line of a result.
    pub detail: String,
    /// What it is ("Museum", "Address"), when the service says.
    pub kind: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Drive,
    Walk,
    Cycle,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Drive, Mode::Walk, Mode::Cycle];
    pub fn label(self) -> &'static str {
        match self {
            Mode::Drive => "Drive",
            Mode::Walk => "Walk",
            Mode::Cycle => "Cycle",
        }
    }
}

// "U-turn" is what the manoeuvre is called.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Depart,
    Straight,
    SlightLeft,
    Left,
    SharpLeft,
    SlightRight,
    Right,
    SharpRight,
    UTurn,
    Roundabout,
    Arrive,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub text: String,
    /// Metres until the next step.
    pub distance: f64,
    pub turn: Turn,
    /// Where the manoeuvre happens.
    pub at: (f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub distance: f64,
    pub duration: f64,
    /// The route as (lat, lon) points.
    pub line: Vec<(f64, f64)>,
    pub steps: Vec<Step>,
}

/// Where search and directions come from.
#[derive(Clone, Debug, PartialEq)]
pub enum Backend {
    Open,
    Mapbox(String),
    Google(String),
    Apple,
}

impl Backend {
    /// Cycling directions, where the backend has them.
    pub fn modes(&self) -> &'static [Mode] {
        &Mode::ALL
    }
}

/// Percent-encode a query for a URL.
pub fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `area` (west, south, east, north) keeps open searches local; without
/// it they search the world, nearest first.
pub fn search_job(
    backend: &Backend,
    seq: u64,
    query: &str,
    near: (f64, f64),
    area: Option<[f64; 4]>,
) -> Option<Job> {
    let (lat, lon) = near;
    let q = encode(query);
    let what = Fetched::Search(seq);
    Some(match backend {
        Backend::Open => {
            let bbox = area
                .map(|[w, s, e, n]| format!("&bbox={w:.4},{s:.4},{e:.4},{n:.4}"))
                .unwrap_or_default();
            Job::Http {
                what,
                url: format!("https://photon.komoot.io/api/?q={q}&lat={lat:.5}&lon={lon:.5}&limit=20&lang=en{bbox}"),
                body: None,
                headers: Vec::new(),
            }
        }
        Backend::Mapbox(token) => Job::Http {
            what,
            url: format!(
                "https://api.mapbox.com/search/searchbox/v1/forward?q={q}&proximity={lon:.5},{lat:.5}&limit=8&access_token={token}"
            ),
            body: None,
            headers: Vec::new(),
        },
        Backend::Google(key) => Job::Http {
            what,
            url: "https://places.googleapis.com/v1/places:searchText".into(),
            body: Some(
                json!({
                    "textQuery": query,
                    "maxResultCount": 8,
                    "locationBias": {"circle": {"center": {"latitude": lat, "longitude": lon}, "radius": 50000.0}},
                })
                .to_string(),
            ),
            headers: vec![
                ("X-Goog-Api-Key".into(), key.clone()),
                (
                    "X-Goog-FieldMask".into(),
                    "places.displayName,places.formattedAddress,places.location,places.primaryTypeDisplayName".into(),
                ),
            ],
        },
        Backend::Apple => return None,
    })
}

pub fn parse_search(backend: &Backend, text: &str) -> Result<Vec<Place>, String> {
    let json: Value = serde_json::from_str(text).map_err(|e| format!("unreadable reply: {e}"))?;
    // Nominatim answers with a list; everything else with an object.
    if let Some(items) = json.as_array() {
        return Ok(items.iter().filter_map(nominatim_place).collect());
    }
    if let Some(message) = json["error"]["message"]
        .as_str()
        .or(json["message"].as_str())
    {
        return Err(message.to_owned());
    }
    let places = match backend {
        Backend::Open => json["features"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| {
                let p = &f["properties"];
                let c = f["geometry"]["coordinates"].as_array()?;
                let street = [p["street"].as_str(), p["housenumber"].as_str()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ");
                let name = p["name"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| street.clone());
                let area: Vec<&str> = [
                    p["city"].as_str(),
                    p["state"].as_str(),
                    p["country"].as_str(),
                ]
                .into_iter()
                .flatten()
                .collect();
                let detail = if street.is_empty() || street == name {
                    area.join(", ")
                } else {
                    format!("{street}, {}", area.join(", "))
                };
                // Roads come with their road class ("tertiary"): call them streets.
                let kind = match p["osm_key"].as_str() {
                    Some("highway") => "Street".to_owned(),
                    _ => words(p["osm_value"].as_str().unwrap_or("")),
                };
                Some(Place {
                    name,
                    detail,
                    kind,
                    lat: c.get(1)?.as_f64()?,
                    lon: c.first()?.as_f64()?,
                })
            })
            .collect(),
        Backend::Mapbox(_) => json["features"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| {
                let p = &f["properties"];
                let c = f["geometry"]["coordinates"].as_array()?;
                let kind = p["poi_category"]
                    .as_array()
                    .and_then(|c| c.first())
                    .and_then(Value::as_str)
                    .or(p["feature_type"].as_str())
                    .unwrap_or("");
                Some(Place {
                    name: p["name"].as_str()?.to_owned(),
                    detail: p["full_address"]
                        .as_str()
                        .or(p["place_formatted"].as_str())
                        .unwrap_or("")
                        .to_owned(),
                    kind: words(kind),
                    lat: c.get(1)?.as_f64()?,
                    lon: c.first()?.as_f64()?,
                })
            })
            .collect(),
        Backend::Google(_) => json["places"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                Some(Place {
                    name: p["displayName"]["text"].as_str()?.to_owned(),
                    detail: p["formattedAddress"].as_str().unwrap_or("").to_owned(),
                    kind: p["primaryTypeDisplayName"]["text"]
                        .as_str()
                        .unwrap_or("")
                        .to_owned(),
                    lat: p["location"]["latitude"].as_f64()?,
                    lon: p["location"]["longitude"].as_f64()?,
                })
            })
            .collect(),
        Backend::Apple => Vec::new(),
    };
    Ok(places)
}

/// A category word ("coffee", "restaurants") as OpenStreetMap names it,
/// for searches that should find every such place nearby rather than
/// places with the word in their name.
pub fn category(query: &str) -> Option<&'static str> {
    let q = query.trim().to_lowercase();
    let q = q
        .trim_end_matches(" nearby")
        .trim_end_matches(" near me")
        .trim_end_matches(" here");
    Some(match q {
        "things to do"
        | "thing to do"
        | "things to see"
        | "what to see"
        | "what to do"
        | "places to visit"
        | "places to see"
        | "sights"
        | "sightseeing"
        | "attraction"
        | "attractions"
        | "tourist attraction"
        | "tourist attractions"
        | "landmarks"
        | "tourism" => "attraction",
        "viewpoint" | "viewpoints" | "view" | "views" | "lookout" | "scenic view" => "viewpoint",
        "coffee" | "coffee shop" | "coffee shops" | "cafe" | "cafes" | "café" | "cafés" => "cafe",
        "restaurant" | "restaurants" | "food" | "dinner" | "lunch" => "restaurant",
        "museum" | "museums" => "museum",
        "park" | "parks" => "park",
        "hotel" | "hotels" => "hotel",
        "pharmacy" | "pharmacies" | "chemist" | "drugstore" => "pharmacy",
        "bar" | "bars" => "bar",
        "pub" | "pubs" => "pub",
        "supermarket" | "supermarkets" | "grocery" | "groceries" => "supermarket",
        "bakery" | "bakeries" => "bakery",
        "fuel" | "gas" | "gas station" | "petrol" | "petrol station" => "fuel",
        "hospital" | "hospitals" => "hospital",
        "atm" | "atms" | "cash machine" => "atm",
        "parking" | "car park" => "parking",
        "library" | "libraries" => "library",
        "cinema" | "cinemas" | "movies" => "cinema",
        _ => return None,
    })
}

/// Every place of a category inside an area, from OpenStreetMap's own
/// search (Nominatim). Its policy is one request a second and no search as
/// you type, so it is only asked when a search is submitted.
pub fn category_job(seq: u64, phrase: &str, area: [f64; 4]) -> Job {
    let [west, south, east, north] = area;
    Job::Http {
        what: Fetched::Search(seq),
        url: format!(
            "https://nominatim.openstreetmap.org/search?q={}&format=jsonv2&limit=40&viewbox={west:.4},{north:.4},{east:.4},{south:.4}&bounded=1",
            encode(phrase)
        ),
        body: None,
        headers: Vec::new(),
    }
}

fn nominatim_place(item: &Value) -> Option<Place> {
    let display = item["display_name"].as_str().unwrap_or("");
    let mut parts = display.split(", ");
    let first = parts.next().unwrap_or("");
    let name = item["name"]
        .as_str()
        .filter(|n| !n.is_empty())
        .unwrap_or(first);
    let rest: Vec<&str> = parts.filter(|p| *p != name).take(3).collect();
    Some(Place {
        name: name.to_owned(),
        detail: rest.join(", "),
        kind: words(item["type"].as_str().unwrap_or("")),
        lat: item["lat"].as_str()?.parse().ok()?,
        lon: item["lon"].as_str()?.parse().ok()?,
    })
}

/// "fast_food" → "Fast food".
fn words(raw: &str) -> String {
    let text = raw.replace('_', " ");
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if !matches!(raw, "yes" | "") => first.to_uppercase().chain(chars).collect(),
        _ => String::new(),
    }
}

pub fn route_job(backend: &Backend, seq: u64, from: &Place, to: &Place, mode: Mode) -> Option<Job> {
    let what = Fetched::Route(seq);
    let pair = format!(
        "{:.6},{:.6};{:.6},{:.6}",
        from.lon, from.lat, to.lon, to.lat
    );
    Some(match backend {
        Backend::Open => {
            let profile = match mode {
                Mode::Drive => "car",
                Mode::Walk => "foot",
                Mode::Cycle => "bike",
            };
            Job::Http {
                what,
                url: format!(
                    "https://routing.openstreetmap.de/routed-{profile}/route/v1/driving/{pair}?overview=full&geometries=geojson&steps=true"
                ),
                body: None,
                headers: Vec::new(),
            }
        }
        Backend::Mapbox(token) => {
            let profile = match mode {
                Mode::Drive => "driving",
                Mode::Walk => "walking",
                Mode::Cycle => "cycling",
            };
            Job::Http {
                what,
                url: format!(
                    "https://api.mapbox.com/directions/v5/mapbox/{profile}/{pair}?overview=full&geometries=geojson&steps=true&access_token={token}"
                ),
                body: None,
                headers: Vec::new(),
            }
        }
        Backend::Google(key) => {
            let point = |p: &Place| json!({"location": {"latLng": {"latitude": p.lat, "longitude": p.lon}}});
            Job::Http {
                what,
                url: "https://routes.googleapis.com/directions/v2:computeRoutes".into(),
                body: Some(
                    json!({
                        "origin": point(from),
                        "destination": point(to),
                        "travelMode": match mode { Mode::Drive => "DRIVE", Mode::Walk => "WALK", Mode::Cycle => "BICYCLE" },
                        "polylineEncoding": "ENCODED_POLYLINE",
                    })
                    .to_string(),
                ),
                headers: vec![
                    ("X-Goog-Api-Key".into(), key.clone()),
                    (
                        "X-Goog-FieldMask".into(),
                        "routes.duration,routes.distanceMeters,routes.polyline.encodedPolyline,routes.legs.steps.distanceMeters,routes.legs.steps.navigationInstruction,routes.legs.steps.startLocation".into(),
                    ),
                ],
            }
        }
        Backend::Apple => return None,
    })
}

pub fn parse_route(backend: &Backend, text: &str) -> Result<Route, String> {
    let json: Value = serde_json::from_str(text).map_err(|e| format!("unreadable reply: {e}"))?;
    if let Some(message) = json["error"]["message"]
        .as_str()
        .or(json["message"].as_str())
    {
        return Err(message.to_owned());
    }
    match backend {
        Backend::Google(_) => google_route(&json),
        _ => osrm_route(&json),
    }
}

/// OSRM's format, which FOSSGIS and Mapbox Directions both return. Mapbox
/// adds instruction text; for OSRM it is written from the manoeuvre.
fn osrm_route(json: &Value) -> Result<Route, String> {
    if json["code"].as_str().is_some_and(|c| c != "Ok") {
        return Err(match json["code"].as_str() {
            Some("NoRoute") => "No route between these places.".into(),
            Some(code) => format!("Routing failed ({code})."),
            None => "Routing failed.".into(),
        });
    }
    let route = &json["routes"][0];
    let line = route["geometry"]["coordinates"]
        .as_array()
        .ok_or("no route in the reply")?
        .iter()
        .filter_map(|c| Some((c.get(1)?.as_f64()?, c.get(0)?.as_f64()?)))
        .collect();
    let mut steps = Vec::new();
    for leg in route["legs"].as_array().into_iter().flatten() {
        for s in leg["steps"].as_array().into_iter().flatten() {
            let m = &s["maneuver"];
            let kind = m["type"].as_str().unwrap_or("");
            let modifier = m["modifier"].as_str().unwrap_or("");
            let turn = osrm_turn(kind, modifier);
            let name = s["name"].as_str().unwrap_or("");
            // The first step's modifier is relative to nothing; use the compass.
            let modifier = if kind == "depart" {
                m["bearing_after"].as_f64().map_or("", compass)
            } else {
                modifier
            };
            let text = m["instruction"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| osrm_text(kind, modifier, name, turn));
            let at = m["location"]
                .as_array()
                .and_then(|c| Some((c.get(1)?.as_f64()?, c.first()?.as_f64()?)));
            steps.push(Step {
                text,
                distance: s["distance"].as_f64().unwrap_or(0.),
                turn,
                at: at.unwrap_or((0., 0.)),
            });
        }
    }
    Ok(Route {
        distance: route["distance"].as_f64().unwrap_or(0.),
        duration: route["duration"].as_f64().unwrap_or(0.),
        line,
        steps,
    })
}

fn compass(bearing: f64) -> &'static str {
    const NAMES: [&str; 8] = [
        "north",
        "northeast",
        "east",
        "southeast",
        "south",
        "southwest",
        "west",
        "northwest",
    ];
    NAMES[((bearing.rem_euclid(360.) + 22.5) / 45.) as usize % 8]
}

fn osrm_turn(kind: &str, modifier: &str) -> Turn {
    match kind {
        "depart" => Turn::Depart,
        "arrive" => Turn::Arrive,
        "roundabout" | "rotary" | "roundabout turn" | "exit roundabout" | "exit rotary" => {
            Turn::Roundabout
        }
        _ => match modifier {
            "slight left" => Turn::SlightLeft,
            "left" => Turn::Left,
            "sharp left" => Turn::SharpLeft,
            "slight right" => Turn::SlightRight,
            "right" => Turn::Right,
            "sharp right" => Turn::SharpRight,
            "uturn" => Turn::UTurn,
            _ => Turn::Straight,
        },
    }
}

fn osrm_text(kind: &str, modifier: &str, name: &str, turn: Turn) -> String {
    let onto = if name.is_empty() {
        String::new()
    } else {
        format!(" onto {name}")
    };
    let on = if name.is_empty() {
        String::new()
    } else {
        format!(" on {name}")
    };
    match turn {
        Turn::Depart => format!(
            "Head {}{on}",
            if modifier.is_empty() { "out" } else { modifier }
        ),
        Turn::Arrive => "Arrive at your destination".into(),
        Turn::Roundabout => format!("At the roundabout, take the exit{onto}"),
        Turn::UTurn => format!("Make a U-turn{onto}"),
        Turn::Straight if kind == "new name" || kind == "continue" => format!("Continue{onto}"),
        Turn::Straight => format!("Go straight{onto}"),
        _ => {
            let verb = if kind == "fork" {
                "Keep"
            } else if kind == "merge" {
                "Merge"
            } else {
                "Turn"
            };
            format!("{verb} {modifier}{onto}")
        }
    }
}

fn google_route(json: &Value) -> Result<Route, String> {
    let route = json["routes"]
        .get(0)
        .ok_or("No route between these places.")?;
    let line = decode_polyline(route["polyline"]["encodedPolyline"].as_str().unwrap_or(""));
    let mut steps = Vec::new();
    for leg in route["legs"].as_array().into_iter().flatten() {
        for s in leg["steps"].as_array().into_iter().flatten() {
            let n = &s["navigationInstruction"];
            let turn = match n["maneuver"].as_str().unwrap_or("") {
                "DEPART" => Turn::Depart,
                "TURN_SLIGHT_LEFT" | "FORK_LEFT" | "RAMP_LEFT" => Turn::SlightLeft,
                "TURN_LEFT" => Turn::Left,
                "TURN_SHARP_LEFT" => Turn::SharpLeft,
                "TURN_SLIGHT_RIGHT" | "FORK_RIGHT" | "RAMP_RIGHT" => Turn::SlightRight,
                "TURN_RIGHT" => Turn::Right,
                "TURN_SHARP_RIGHT" => Turn::SharpRight,
                "UTURN_LEFT" | "UTURN_RIGHT" => Turn::UTurn,
                "ROUNDABOUT_LEFT" | "ROUNDABOUT_RIGHT" => Turn::Roundabout,
                _ => Turn::Straight,
            };
            let at = &s["startLocation"]["latLng"];
            steps.push(Step {
                text: n["instructions"].as_str().unwrap_or("Continue").to_owned(),
                distance: s["distanceMeters"].as_f64().unwrap_or(0.),
                turn,
                at: (
                    at["latitude"].as_f64().unwrap_or(0.),
                    at["longitude"].as_f64().unwrap_or(0.),
                ),
            });
        }
    }
    if let Some(&end) = line.last() {
        steps.push(Step {
            text: "Arrive at your destination".into(),
            distance: 0.,
            turn: Turn::Arrive,
            at: end,
        });
    }
    let duration = route["duration"]
        .as_str()
        .and_then(|d| d.trim_end_matches('s').parse().ok())
        .unwrap_or(0.);
    Ok(Route {
        distance: route["distanceMeters"].as_f64().unwrap_or(0.),
        duration,
        line,
        steps,
    })
}

/// Google's encoded polyline format (precision 5).
pub fn decode_polyline(encoded: &str) -> Vec<(f64, f64)> {
    let bytes = encoded.as_bytes();
    let (mut i, mut lat, mut lon) = (0usize, 0i64, 0i64);
    let mut out = Vec::new();
    let next = |i: &mut usize| -> Option<i64> {
        let (mut result, mut shift) = (0i64, 0);
        loop {
            let b = i64::from(*bytes.get(*i)?) - 63;
            *i += 1;
            result |= (b & 0x1f) << shift;
            shift += 5;
            if b < 0x20 {
                break;
            }
        }
        Some(if result & 1 != 0 {
            !(result >> 1)
        } else {
            result >> 1
        })
    };
    while i < bytes.len() {
        let (Some(dlat), Some(dlon)) = (next(&mut i), next(&mut i)) else {
            break;
        };
        lat += dlat;
        lon += dlon;
        out.push((lat as f64 / 1e5, lon as f64 / 1e5));
    }
    out
}

/// "850 m", "4.2 km".
pub fn distance(metres: f64) -> String {
    if metres < 1000. {
        format!("{} m", ((metres / 10.).round() * 10.) as i64)
    } else if metres < 100_000. {
        format!("{:.1} km", metres / 1000.)
    } else {
        format!("{} km", (metres / 1000.).round() as i64)
    }
}

/// How distances are written: kilometres and metres, or miles and feet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
    Metric,
    Imperial,
}

impl Units {
    pub fn distance(self, metres: f64) -> String {
        match self {
            Units::Metric => distance(metres),
            Units::Imperial => {
                let miles = metres / 1609.344;
                if miles < 0.1 {
                    format!("{} ft", ((metres / 0.3048 / 10.).round() * 10.) as i64)
                } else if miles < 100. {
                    format!("{miles:.1} mi")
                } else {
                    format!("{} mi", miles.round() as i64)
                }
            }
        }
    }

    /// A round length (in metres) of at most `most` metres for a scale bar,
    /// and how it's written.
    pub fn scale(self, most: f64) -> (f64, String) {
        let round = |n: f64| {
            let step = 10f64.powf(n.log10().floor());
            [5., 2., 1.]
                .into_iter()
                .map(|k| k * step)
                .find(|&m| m <= n)
                .unwrap_or(step)
        };
        match self {
            Units::Metric if most >= 1000. => {
                let km = round(most / 1000.);
                (km * 1000., format!("{km} km"))
            }
            Units::Metric => {
                let m = round(most);
                (m, format!("{m} m"))
            }
            Units::Imperial if most >= 1609.344 => {
                let mi = round(most / 1609.344);
                (mi * 1609.344, format!("{mi} mi"))
            }
            Units::Imperial => {
                let ft = round(most / 0.3048);
                (ft * 0.3048, format!("{ft} ft"))
            }
        }
    }
}

/// "12 min", "1 h 05 min".
pub fn duration(seconds: f64) -> String {
    let minutes = (seconds / 60.).round() as i64;
    if minutes < 60 {
        format!("{} min", minutes.max(1))
    } else {
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    }
}

/// Great-circle distance in metres.
pub fn haversine(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (
        a.0.to_radians(),
        a.1.to_radians(),
        b.0.to_radians(),
        b.1.to_radians(),
    );
    let h =
        ((la2 - la1) / 2.).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.).sin().powi(2);
    2. * 6_371_000. * h.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polylines_decode_and_osrm_replies_become_routes() {
        // Google's documented example.
        let line = decode_polyline("_p~iF~ps|U_ulLnnqC_mqNvxq`@");
        assert_eq!(line.len(), 3);
        assert!((line[0].0 - 38.5).abs() < 1e-6 && (line[0].1 + 120.2).abs() < 1e-6);
        assert!((line[2].0 - 43.252).abs() < 1e-6 && (line[2].1 + 126.453).abs() < 1e-6);
        let reply = r#"{"code":"Ok","routes":[{"distance":1200,"duration":300,
            "geometry":{"coordinates":[[18.9,69.6],[18.95,69.65]]},
            "legs":[{"steps":[
              {"distance":800,"name":"Storgata","maneuver":{"type":"depart","modifier":"north","location":[18.9,69.6]}},
              {"distance":400,"name":"Sjøgata","maneuver":{"type":"turn","modifier":"left","location":[18.92,69.62]}},
              {"distance":0,"name":"","maneuver":{"type":"arrive","location":[18.95,69.65]}}]}]}]}"#;
        let route = parse_route(&Backend::Open, reply).unwrap();
        assert_eq!(route.line.len(), 2);
        assert_eq!(route.steps[1].text, "Turn left onto Sjøgata");
        assert_eq!(route.steps[1].turn, Turn::Left);
        assert_eq!(route.steps[2].turn, Turn::Arrive);
        assert_eq!(distance(1234.), "1.2 km");
        assert_eq!(Units::Imperial.distance(1234.), "0.8 mi");
        assert_eq!(Units::Imperial.distance(50.), "160 ft");
        assert_eq!(Units::Imperial.scale(400.).1, "1000 ft");
        assert_eq!(Units::Metric.scale(2600.).1, "2 km");
        assert_eq!(duration(3900.), "1 h 05 min");
        assert_eq!(encode("Tromsø museum"), "Troms%C3%B8%20museum");
        assert_eq!(category("Coffee"), Some("cafe"));
        assert_eq!(category("restaurants nearby"), Some("restaurant"));
        assert_eq!(category("Storgata"), None);
        assert_eq!(category("Things to do"), Some("attraction"));
        assert_eq!(category("sights near me"), Some("attraction"));
        let nominatim = r#"[{"name":"Risø Kaffebar","type":"cafe","lat":"69.6489","lon":"18.9560",
            "display_name":"Risø Kaffebar, Fredrik Langes gate, Prostneset, Tromsø, Norway"}]"#;
        let found = parse_search(&Backend::Open, nominatim).unwrap();
        assert_eq!(found[0].name, "Risø Kaffebar");
        assert_eq!(found[0].kind, "Cafe");
        assert_eq!(found[0].detail, "Fredrik Langes gate, Prostneset, Tromsø");
    }
}
