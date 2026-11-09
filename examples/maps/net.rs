//! Downloads. The app describes what it needs as jobs; on the desktop a few
//! worker threads fetch them over HTTPS, decode them off the UI thread and
//! post the results back through the app's `UiSender`; in a browser the page
//! fetches the queued jobs and hands the bytes back. Either way `finish`
//! turns the bytes into the app's message. Results carry the provider
//! generation so a late tile from a provider you switched away from is
//! dropped.
use super::mvt;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
#[cfg(feature = "native")]
use std::{sync::mpsc, time::Duration};
use volt_ui_kit::graphics::Image;
#[cfg(feature = "native")]
use volt_ui_kit::native::UiSender;

/// Identifies this example to tile servers, as their usage policies ask. (A
/// browser sends its own, with the page's address as the referrer.)
#[cfg(feature = "native")]
const USER_AGENT: &str =
    "volt-ui-kit-maps-example/0.1 (+https://github.com/danieldear/volt-ui-kit)";
#[cfg(feature = "native")]
const WORKERS: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub source: String,
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone)]
pub enum TileData {
    Vector(Arc<mvt::Tile>),
    Raster(Image),
}

pub enum Job {
    Text {
        what: Fetched,
        url: String,
    },
    Post {
        what: Fetched,
        url: String,
        body: String,
    },
    /// A GET, or a JSON POST when there is a body, with extra headers.
    Http {
        what: Fetched,
        url: String,
        body: Option<String>,
        headers: Vec<(String, String)>,
    },
    Tile {
        key: TileKey,
        url: String,
        vector: bool,
    },
}

/// What a text download was for.
#[derive(Clone, Debug)]
pub enum Fetched {
    Style,
    TileJson(String),
    GoogleSession,
    /// Search request N.
    Search(u64),
    /// Directions request N.
    Route(u64),
}

pub enum Msg {
    Text(u64, Fetched, Result<String, String>),
    Tile(u64, TileKey, Result<TileData, String>),
    #[cfg_attr(not(all(target_os = "macos", feature = "native")), allow(dead_code))]
    Snapshot(u64, u64, Result<Image, String>),
    /// The camera has been still since snapshot request N was made.
    #[cfg_attr(not(all(target_os = "macos", feature = "native")), allow(dead_code))]
    SnapshotDue(u64, u64),
    /// Typing has paused since search request N.
    SearchDue(u64),
    /// Time to widen search request N (after Nominatim's one-second gap).
    SearchWider(u64),
    /// Time to read the latest location fix.
    #[cfg_attr(not(all(target_os = "macos", feature = "native")), allow(dead_code))]
    LocationTick,
    /// Time to compare the system appearance with the map's.
    AppearanceCheck,
    /// The next animation frame, while the camera moves.
    Frame,
    /// Debugging: start a demonstration flight.
    Demo,
    /// Apple's search and directions arrive already parsed.
    #[cfg_attr(not(all(target_os = "macos", feature = "native")), allow(dead_code))]
    Places(u64, Result<Vec<super::services::Place>, String>),
    #[cfg_attr(not(all(target_os = "macos", feature = "native")), allow(dead_code))]
    Routed(u64, Result<super::services::Route, String>),
}

/// Where jobs go: worker threads on the desktop, or a queue that a browser
/// page (or a headless render) works through.
#[derive(Clone)]
pub enum Net {
    #[cfg(feature = "native")]
    Workers(mpsc::Sender<(u64, Job)>),
    Queue(Arc<Mutex<VecDeque<(u64, Job)>>>),
}

impl Job {
    /// What to ask for: the address, a JSON body to POST, extra headers.
    pub fn request(&self) -> (&str, Option<&str>, &[(String, String)]) {
        match self {
            Job::Text { url, .. } | Job::Tile { url, .. } => (url, None, &[]),
            Job::Post { url, body, .. } => (url, Some(body), &[]),
            Job::Http {
                url, body, headers, ..
            } => (url, body.as_deref(), headers),
        }
    }
}

/// One response budget shared by native HTTP, browser streaming and finish.
/// The latter also guards externally supplied/headless download results.
pub fn response_limit(job: &Job) -> usize {
    match job {
        Job::Tile { vector: true, .. } => mvt::MAX_TILE_BYTES,
        Job::Tile { vector: false, .. } => 32 * 1024 * 1024,
        _ => 8 * 1024 * 1024,
    }
}

/// A finished download as the app's message: the HTTP status and body, or
/// why there was none.
pub fn finish(generation: u64, job: Job, response: Result<(u16, Vec<u8>), String>) -> Msg {
    let response = response.and_then(|(status, bytes)| {
        if bytes.len() > response_limit(&job) {
            Err("map response exceeds size limit".into())
        } else {
            Ok((status, bytes))
        }
    });
    // Search and routing services explain failures in JSON: keep those.
    let explains = matches!(job, Job::Http { .. });
    let bytes = response.and_then(|(status, bytes)| {
        if (200..300).contains(&status) || (explains && bytes.first() == Some(&b'{')) {
            Ok(bytes)
        } else if explains {
            Err(format!("HTTP {status}"))
        } else {
            // Keep the server's short explanation (bad token, quota, …).
            let detail: String = String::from_utf8_lossy(&bytes).chars().take(160).collect();
            Err(format!("HTTP {status}: {}", detail.trim()))
        }
    });
    match job {
        Job::Text { what, .. } | Job::Post { what, .. } | Job::Http { what, .. } => {
            Msg::Text(generation, what, bytes.and_then(text))
        }
        Job::Tile { key, vector, .. } => {
            let data = bytes.and_then(|bytes| {
                if vector {
                    mvt::decode(&bytes).map(|t| TileData::Vector(Arc::new(t)))
                } else {
                    image(&bytes).map(TileData::Raster)
                }
            });
            Msg::Tile(generation, key, data)
        }
    }
}

#[cfg(feature = "native")]
fn image(bytes: &[u8]) -> Result<Image, String> {
    volt_ui_kit::native::decode_image(bytes).map_err(|e| e.to_string())
}

#[cfg(not(feature = "native"))]
fn image(_: &[u8]) -> Result<Image, String> {
    Err("Image tiles are drawn on the desktop only.".into())
}

impl Net {
    pub fn queue() -> Self {
        Net::Queue(Arc::default())
    }
    /// The next queued job, oldest first.
    pub fn next(&self) -> Option<(u64, Job)> {
        match self {
            Net::Queue(q) => q.lock().unwrap().pop_front(),
            #[cfg(feature = "native")]
            Net::Workers(_) => None,
        }
    }
    #[cfg(feature = "native")]
    pub fn start(sender: UiSender<Msg>) -> Self {
        let (jobs, queue) = mpsc::channel::<(u64, Job)>();
        let queue = Arc::new(Mutex::new(queue));
        let agent = agent();
        for _ in 0..WORKERS {
            let (queue, agent, sender) = (queue.clone(), agent.clone(), sender.clone());
            std::thread::spawn(move || loop {
                let Ok((generation, job)) = queue.lock().unwrap().recv() else {
                    return;
                };
                let message = run(&agent, generation, job);
                if sender.send(message).is_err() {
                    return;
                }
            });
        }
        Net::Workers(jobs)
    }
    pub fn send(&self, generation: u64, job: Job) {
        match self {
            #[cfg(feature = "native")]
            Net::Workers(jobs) => drop(jobs.send((generation, job))),
            Net::Queue(q) => q.lock().unwrap().push_back((generation, job)),
        }
    }
}

#[cfg(feature = "native")]
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// Fetch one job on this thread.
#[cfg(feature = "native")]
pub fn run(agent: &ureq::Agent, generation: u64, job: Job) -> Msg {
    let (url, body, headers) = job.request();
    let response = match body {
        Some(body) => {
            let mut request = agent.post(url).header("Content-Type", "application/json");
            for (k, v) in headers {
                request = request.header(k.as_str(), v.as_str());
            }
            request.send(body.as_bytes())
        }
        None => {
            let mut request = agent.get(url);
            for (k, v) in headers {
                request = request.header(k.as_str(), v.as_str());
            }
            request.call()
        }
    };
    let limit = response_limit(&job) as u64;
    let response = response
        .map_err(|e| e.to_string())
        .and_then(|mut response| {
            let status = response.status().as_u16();
            response
                .body_mut()
                .with_config()
                .limit(limit)
                .read_to_vec()
                .map(|bytes| (status, bytes))
                .map_err(|e| e.to_string())
        });
    finish(generation, job, response)
}

fn text(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_response_bytes_are_limited_before_text_or_tile_decode() {
        let job = Job::Text {
            what: Fetched::Style,
            url: "https://fixture.invalid/style".into(),
        };
        let limit = response_limit(&job);
        assert!(matches!(
            finish(0, job, Ok((200, vec![b'a'; limit + 1]))),
            Msg::Text(_, _, Err(_))
        ));
    }
}
