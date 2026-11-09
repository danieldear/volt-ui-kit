//! Background downloads: a few worker threads fetch styles, TileJSON and
//! tiles over HTTPS, decode them off the UI thread and post the results back
//! through the app's `UiSender`. Results carry the provider generation so a
//! late tile from a provider you switched away from is dropped.
use super::mvt;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use volt_ui_kit::{graphics::Image, native::UiSender};

/// Identifies this example to tile servers, as their usage policies ask.
const USER_AGENT: &str =
    "volt-ui-kit-maps-example/0.1 (+https://github.com/danieldear/volt-ui-kit)";
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
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Snapshot(u64, u64, Result<Image, String>),
    /// The camera has been still since snapshot request N was made.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    SnapshotDue(u64, u64),
    /// Typing has paused since search request N.
    SearchDue(u64),
    /// Time to widen search request N (after Nominatim's one-second gap).
    SearchWider(u64),
    /// Time to read the latest location fix.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    LocationTick,
    /// Time to compare the system appearance with the map's.
    AppearanceCheck,
    /// The next animation frame, while the camera moves.
    Frame,
    /// Debugging: start a demonstration flight.
    Demo,
    /// Apple's search and directions arrive already parsed.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Places(u64, Result<Vec<super::services::Place>, String>),
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Routed(u64, Result<super::services::Route, String>),
}

/// Where jobs go: worker threads, or a queue drained in place for headless
/// renders.
#[derive(Clone)]
pub enum Net {
    Workers(mpsc::Sender<(u64, Job)>),
    Queue(Arc<Mutex<std::collections::VecDeque<(u64, Job)>>>),
}

pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

impl Net {
    pub fn queue() -> Self {
        Net::Queue(Arc::default())
    }
    /// Run queued jobs one at a time; results may queue more.
    pub fn next(&self) -> Option<(u64, Job)> {
        match self {
            Net::Queue(q) => q.lock().unwrap().pop_front(),
            Net::Workers(_) => None,
        }
    }
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
            Net::Workers(jobs) => drop(jobs.send((generation, job))),
            Net::Queue(q) => q.lock().unwrap().push_back((generation, job)),
        }
    }
}

pub fn run(agent: &ureq::Agent, generation: u64, job: Job) -> Msg {
    match job {
        Job::Text { what, url } => Msg::Text(generation, what, get(agent, &url).and_then(text)),
        Job::Post { what, url, body } => {
            let response = agent
                .post(&url)
                .header("Content-Type", "application/json")
                .send(body.as_bytes())
                .map_err(|e| e.to_string())
                .and_then(read);
            Msg::Text(generation, what, response.and_then(text))
        }
        Job::Http {
            what,
            url,
            body,
            headers,
        } => {
            let response = match body {
                Some(body) => {
                    let mut request = agent.post(&url).header("Content-Type", "application/json");
                    for (k, v) in &headers {
                        request = request.header(k.as_str(), v.as_str());
                    }
                    request.send(body.as_bytes())
                }
                None => {
                    let mut request = agent.get(&url);
                    for (k, v) in &headers {
                        request = request.header(k.as_str(), v.as_str());
                    }
                    request.call()
                }
            };
            let result = response.map_err(|e| e.to_string()).and_then(read_any);
            Msg::Text(generation, what, result.and_then(text))
        }
        Job::Tile { key, url, vector } => {
            let data = get(agent, &url).and_then(|bytes| {
                if vector {
                    mvt::decode(&bytes).map(|t| TileData::Vector(Arc::new(t)))
                } else {
                    volt_ui_kit::native::decode_image(&bytes)
                        .map(TileData::Raster)
                        .map_err(|e| e.to_string())
                }
            });
            Msg::Tile(generation, key, data)
        }
    }
}

fn get(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())
        .and_then(read)
}

fn read(mut response: ureq::http::Response<ureq::Body>) -> Result<Vec<u8>, String> {
    let status = response.status();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    if status.is_success() {
        Ok(bytes)
    } else {
        // Keep the server's short explanation (bad token, quota, …).
        let detail: String = String::from_utf8_lossy(&bytes).chars().take(160).collect();
        Err(format!("HTTP {}: {}", status.as_u16(), detail.trim()))
    }
}

/// Keep error bodies: search and routing services explain failures in JSON.
fn read_any(mut response: ureq::http::Response<ureq::Body>) -> Result<Vec<u8>, String> {
    let status = response.status();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(8 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    if status.is_success() || bytes.first() == Some(&b'{') {
        Ok(bytes)
    } else {
        Err(format!("HTTP {}", status.as_u16()))
    }
}

fn text(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
