//! Maps drawn by Volt from different providers' styles and tiles: a test bed
//! for the planned map component.
//!
//! cargo run --release --features native-gpu --example maps
//!
//! Mapbox and Google need your own keys, read from MAPBOX_ACCESS_TOKEN and
//! GOOGLE_MAPS_API_KEY in the environment that runs the example.
#[path = "maps/mod.rs"]
mod maps;
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::native::{WindowBackground, WindowChrome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inter, bundled with the app: the kit uses it as the default face.
    // One file per weight, cut from the repository's variable Inter, since
    // the text engine doesn't pick weights out of a variable font yet.
    for font in [
        include_bytes!("maps/fonts/Inter-Regular.ttf").as_slice(),
        include_bytes!("maps/fonts/Inter-Medium.ttf").as_slice(),
        include_bytes!("maps/fonts/Inter-SemiBold.ttf").as_slice(),
        include_bytes!("maps/fonts/Inter-Bold.ttf").as_slice(),
    ] {
        volt_ui_kit::native::register_font(font.to_vec())?;
    }
    let mut app = maps::App::new();
    let mut args: Vec<String> = ["--size".to_owned(), "1280x820".to_owned()].into();
    let mut rest = std::env::args().skip(1).peekable();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            // --appearance dark|light, before --render: the map and window.
            "--appearance" => {
                let dark = rest.next().as_deref() == Some("dark");
                app.dark = dark;
                app.appearance.set(dark);
                args.extend([
                    "--theme".to_owned(),
                    if dark { "dark" } else { "light" }.to_owned(),
                ]);
            }
            // --at LAT,LON before --render: where the map starts.
            "--at" => {
                let at = rest.next().ok_or("missing --at LAT,LON")?;
                let (lat, lon) = at.split_once(',').ok_or("--at takes LAT,LON")?;
                app.camera.center = maps::project(lat.trim().parse()?, lon.trim().parse()?);
            }
            // --render PROVIDER OUT.png: download, then draw without a window.
            "--render" => {
                let name = rest.next().ok_or("missing --render provider")?;
                let provider = maps::Provider::ALL
                    .into_iter()
                    .find(|p| p.label().eq_ignore_ascii_case(&name))
                    .ok_or("unknown provider")?;
                let out = rest.next().ok_or("missing --render output path")?;
                if let Some(zoom) = rest.next_if(|z| z.parse::<f64>().is_ok()) {
                    app.camera.zoom = zoom.parse()?;
                }
                app.prefetch(provider, [1200., 800.]);
                eprintln!(
                    "{}: {} tiles, {} failed",
                    provider.label(),
                    app.tiles.len(),
                    app.failed
                );
                // Optional: --search TEXT [--route] after the provider.
                let mut extra = Vec::new();
                extra.extend(rest.by_ref());
                let mut words = extra.iter();
                while let Some(arg) = words.next() {
                    match arg.as_str() {
                        "--search" => {
                            app.query = words.next().cloned().unwrap_or_default();
                            app.search_now();
                            app.drain();
                            eprintln!(
                                "search: {} results {:?}",
                                app.results.len(),
                                app.search_error
                            );
                            for place in app.results.iter().take(3) {
                                eprintln!("  {} · {} · {}", place.name, place.kind, place.detail);
                            }
                        }
                        // --settings: with the Settings popover open.
                        "--settings" => app.settings_open = true,
                        // --tilt BEARING,PITCH: a turned and tilted view.
                        "--tilt" => {
                            let value = words.next().cloned().unwrap_or_default();
                            let (bearing, pitch) =
                                value.split_once(',').ok_or("--tilt takes BEARING,PITCH")?;
                            app.camera.bearing = bearing.trim().parse()?;
                            app.camera.pitch = pitch.trim().parse()?;
                            app.request_tiles();
                            app.drain();
                            eprintln!(
                                "tilt: bearing {} pitch {}, {} tiles",
                                app.camera.bearing,
                                app.camera.pitch,
                                app.tiles.len()
                            );
                        }
                        // --flight-at T: mid-way through a flight to Oslo.
                        "--flight-at" => {
                            let t: f64 = words.next().and_then(|t| t.parse().ok()).unwrap_or(0.5);
                            let to = app.oslo();
                            app.flight_at(to, t);
                            eprintln!(
                                "flight at {t}: zoom {:.2}, {} tiles",
                                app.camera.zoom,
                                app.tiles.len()
                            );
                        }
                        "--pick" => {
                            if let Some(place) = app.results.first().cloned() {
                                app.select(place);
                                app.drain();
                            }
                        }
                        "--route" => {
                            if let Some(place) = app.results.first().cloned() {
                                let start = app.camera;
                                app.select(place);
                                app.camera = start;
                                app.directions();
                                app.drain();
                                match &app.route {
                                    Some(r) => eprintln!(
                                        "route: {} · {} · {} steps",
                                        maps::services::duration(r.duration),
                                        maps::services::distance(r.distance),
                                        r.steps.len()
                                    ),
                                    None => eprintln!("route failed: {:?}", app.route_error),
                                }
                            }
                        }
                        other => args.push(other.to_owned()),
                    }
                }
                // How long one view takes to build: the cost of every drag frame.
                let started = std::time::Instant::now();
                for _ in 0..5 {
                    std::hint::black_box(maps::view(&app));
                }
                eprintln!(
                    "view: {:.1} ms",
                    started.elapsed().as_secs_f64() * 1000. / 5.
                );
                args.extend(["--snapshot".to_owned(), out]);
            }
            // --preload PROVIDER: download first, then run (for --bench).
            "--preload" => {
                let name = rest.next().ok_or("missing --preload provider")?;
                let provider = maps::Provider::ALL
                    .into_iter()
                    .find(|p| p.label().eq_ignore_ascii_case(&name))
                    .ok_or("unknown provider")?;
                app.prefetch(provider, [1200., 712.]);
            }
            "--provider" => {
                let name = rest.next().ok_or("missing --provider name")?;
                let provider = maps::Provider::ALL
                    .into_iter()
                    .find(|p| p.label().eq_ignore_ascii_case(&name))
                    .ok_or("unknown provider")?;
                app = app.starting_with(provider);
            }
            _ => args.push(arg),
        }
    }
    support::launch_themed_with_messages(
        "Maps",
        app,
        maps::view,
        maps::theme,
        support::RustLaunchOptions {
            // The title bar is part of the app: the map runs under it and the
            // macOS window buttons sit over its leading corner.
            chrome: if cfg!(all(target_os = "macos", feature = "native")) {
                WindowChrome::Overlay
            } else {
                WindowChrome::Native
            },
            background: WindowBackground::Opaque,
            args,
        },
        |app, sender| app.start(sender),
        maps::App::update,
    )
}
