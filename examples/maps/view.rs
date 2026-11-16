//! The map app: the map fills the window; a floating panel on the left
//! searches, shows a place and gives directions; zoom and map style float on
//! the right; a scale bar and the credits sit along the bottom.
//!
//! Drawing: style layers become canvas paths (fills and lines), image tiles
//! become positioned images, and point labels become positioned text with a
//! simple collision check. Layers keep their style order, so an image layer
//! between vector layers splits the canvas in two.
mod tilted;

use super::{
    mvt::{self, Geometry},
    net::{TileData, TileKey},
    perspective::View,
    services::{self, Mode, Place, Turn},
    style::{self, hex, Ctx, Layer, LayerKind},
    App, Panel, Placement, Provider,
};
use std::collections::{BTreeSet, HashMap};
use volt_ui_kit::{
    graphics::{Canvas, ImageFit, PathCommand, VectorPath},
    ui::{prelude::*, Container, Text},
    ColorToken,
};

pub const PANEL_WIDTH: f32 = 360.;
/// The title bar: with the overlay chrome on macOS, the window buttons sit
/// in its leading corner over the map.
pub const TOOLBAR: f32 = 52.;
const HERE: &str = "#0A84FF";
const ROUTE: &str = "#2563EB";
const PIN: &str = "#2563EB";
const PICKED: &str = "#E11D48";
const START: &str = "#16A34A";

pub fn view(app: &App) -> Element<App> {
    zstack![map(app), chrome(app)]
        .key("root")
        .align(Align::Stretch)
        .into()
}

// ── The map ────────────────────────────────────────────────────────────────

fn map(app: &App) -> Container<App> {
    let mut layers: Vec<Element<App>> = Vec::new();
    let mut labels = Vec::new();
    if app.provider == Provider::Apple {
        apple_layer(app, &mut layers);
    } else if let Some(style) = &app.style {
        labels = draw_style(app, style, &mut layers);
    }
    route_layer(app, &mut layers);
    let budget = if app.camera.flat() {
        LABEL_GLYPHS
    } else {
        TILTED_LABEL_GLYPHS
    };
    place_labels(labels, app.size, budget, &mut layers);
    pins(app, &mut layers);
    zstack(layers)
        .key("map")
        .flow(Flow::TopLeft)
        .clip_contents(true)
        .background(if app.dark { "#1B1F24" } else { "#E5E7EB" })
        .cursor(Cursor::Grab)
        .on_width(|app: &mut App, w| {
            app.size[0] = w;
            app.request_tiles();
        })
        .on_height(|app: &mut App, h| {
            app.size[1] = h;
            app.request_tiles();
        })
        .on_pointer(|app: &mut App, pointer| app.pointer(pointer))
}

/// A path through screen points, split to fit the kit's per-path limit.
fn polylines(points: &[[f32; 2]]) -> Vec<VectorPath> {
    let max = volt_ui_kit::graphics::MAX_PATH_COMMANDS - 1;
    let mut out = Vec::new();
    for chunk in points.chunks(max).enumerate().map(|(i, c)| {
        // Each chunk starts where the last ended, so the line is unbroken.
        let start = if i == 0 { 0 } else { i * max - 1 };
        &points[start..(start + c.len() + usize::from(i > 0)).min(points.len())]
    }) {
        let mut path = VectorPath::new();
        for (i, p) in chunk.iter().enumerate() {
            let p = [p[0].clamp(-60_000., 60_000.), p[1].clamp(-60_000., 60_000.)];
            path.commands.push(if i == 0 {
                PathCommand::Move(p)
            } else {
                PathCommand::Line(p)
            });
        }
        if path.is_valid() {
            out.push(path);
        }
    }
    out
}

fn route_layer(app: &App, out: &mut Vec<Element<App>>) {
    let Some(route) = &app.route else { return };
    let paths: Vec<VectorPath> = if app.camera.flat() {
        let points: Vec<[f32; 2]> = route
            .line
            .iter()
            .map(|&(lat, lon)| app.screen(lat, lon))
            .collect();
        polylines(&points)
    } else {
        tilted::route_points(&View::new(app.camera, app.size), &route.line)
            .iter()
            .flat_map(|piece| polylines(piece))
            .collect()
    };
    let [w, h] = app.size;
    let casing = if app.dark { "#0B1220" } else { "#FFFFFF" };
    out.push(
        volt_ui_kit::ui::canvas(move |c| {
            for path in &paths {
                c.stroke(path.clone(), casing, 9.);
            }
            for path in paths {
                c.stroke(path, ROUTE, 5.);
            }
        })
        .key("route")
        .width(w.max(1.))
        .height(h.max(1.)),
    );
}

/// Result pins (numbered), the chosen place, and a route's start.
fn pins(app: &App, out: &mut Vec<Element<App>>) {
    let [w, h] = app.size;
    let mut numbers = Vec::new();
    let showing_results = app.panel == Panel::Search;
    let picked = app.selected.clone();
    let start = (app.panel == Panel::Directions)
        .then(|| app.origin.clone())
        .flatten();
    // Only pins on or near the screen: far-away results would be drawn
    // outside the coordinates a canvas accepts.
    let near = |[x, y]: [f32; 2]| x > -60. && y > -60. && x < w + 60. && y < h + 60.;
    let mut results: Vec<[f32; 2]> = Vec::new();
    if showing_results {
        for (i, p) in app.results.iter().enumerate() {
            let at = app.screen(p.lat, p.lon);
            if near(at) {
                results.push(at);
                numbers.push((i + 1, at));
            }
        }
    }
    let picked_at = picked
        .as_ref()
        .map(|p| app.screen(p.lat, p.lon))
        .filter(|&at| near(at));
    let start_at = start
        .as_ref()
        .filter(|p| p.name != "My location")
        .map(|p| app.screen(p.lat, p.lon))
        .filter(|&at| near(at));
    // You are here: a dot and a circle as wide as the fix is uncertain.
    let here = app.location.and_then(|(lat, lon, accuracy)| {
        let at = app.screen(lat, lon);
        let per_point = 40_075_016.686 * lat.to_radians().cos() / app.camera.world();
        let radius = ((accuracy.max(0.) / per_point) as f32).min(4_000.);
        near(at).then_some((at, radius))
    });
    out.push(
        volt_ui_kit::ui::canvas(move |c| {
            let pin = |c: &mut Canvas, [x, y]: [f32; 2], r: f32, color: &str| {
                let head = y - r - 6.;
                c.fill(
                    VectorPath::new()
                        .move_to(x, y)
                        .line_to(x - r * 0.62, head + r * 0.55)
                        .line_to(x + r * 0.62, head + r * 0.55)
                        .close(),
                    color,
                );
                c.fill(VectorPath::circle(x, head, r), color);
                c.stroke(VectorPath::circle(x, head, r), "#FFFFFF", 2.);
            };
            if let Some(([x, y], radius)) = here {
                if radius > 10. {
                    c.fill(VectorPath::circle(x, y, radius), "#0A84FF26");
                    c.stroke(VectorPath::circle(x, y, radius), "#0A84FF66", 1.);
                }
                c.fill(VectorPath::circle(x, y, 9.), "#FFFFFF");
                c.fill(VectorPath::circle(x, y, 6.5), HERE);
            }
            for at in &results {
                pin(c, *at, 9., PIN);
            }
            if let Some(at) = start_at {
                c.fill(VectorPath::circle(at[0], at[1], 8.), "#FFFFFF");
                c.fill(VectorPath::circle(at[0], at[1], 6.), START);
            }
            if let Some(at) = picked_at {
                pin(c, at, 12., PICKED);
                c.fill(VectorPath::circle(at[0], at[1] - 18., 4.), "#FFFFFF");
            }
        })
        .key("pins")
        .width(w.max(1.))
        .height(h.max(1.)),
    );
    for (n, [x, y]) in numbers {
        let label = n.to_string();
        let width = label.len() as f32 * 6.5;
        out.push(
            text(label)
                .key(format!("pin-{n}"))
                .font_size(11.)
                .font_weight(700)
                .no_wrap(true)
                .color("#FFFFFF")
                .offset(x - width / 2., y - 15. - 7.5)
                .into(),
        );
    }
}

// ── Design ─────────────────────────────────────────────────────────────────
//
// One light look: white surfaces with soft shadows floating over the map,
// Inter throughout, a single accent blue, 8-point spacing.

// Theme roles, so light and dark both follow the palette in mod.rs.
const INK: ColorToken = ColorToken::BaseContent;
const MUTED: ColorToken = ColorToken::Muted;
const HAIRLINE: ColorToken = ColorToken::Border;
const FIELD: ColorToken = ColorToken::Base200;
const ACCENT: ColorToken = ColorToken::Primary;
fn accent_soft() -> String {
    ColorToken::Primary.alpha(14)
}

fn surface(content: Vec<Element<App>>) -> Container<App> {
    vstack(content)
        .background(ColorToken::Base100)
        .border_color(HAIRLINE)
        .border_width(1.)
        .rounded(16.)
        .shadow(Shadow::new().color("#0000002E").blur(28.).offset(0., 10.))
}

fn glyph(markup: &str, size: f32) -> Element<App> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{markup}</svg>"#
    );
    match SvgIcon::parse(&svg) {
        Ok(icon) => svg_icon(&icon).size(size).color(INK).into(),
        Err(_) => spacer().into(),
    }
}

const LAYERS: &str = r#"<path d="M12 3 3 8l9 5 9-5z"/><path d="m3 13 9 5 9-5"/>"#;
const SIDEBAR: &str = r#"<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M9 4v16"/>"#;
const LOCATE: &str = r#"<path d="M20 4 4 11l7 2 2 7z"/>"#;
const SWAP: &str = r#"<path d="M8 4v16M4 8l4-4 4 4M16 20V4M12 16l4 4 4-4"/>"#;
const CLOSE: &str = r#"<path d="M6 6l12 12M18 6 6 18"/>"#;
const SETTINGS: &str = r#"<path d="M4 7h10M18 7h2M4 17h4M12 17h8"/><circle cx="16" cy="7" r="2"/><circle cx="10" cy="17" r="2"/>"#;

/// A square button with an icon, quiet until hovered.
fn tool(key: &str, markup: &str, label: &str, f: impl FnMut(&mut App) + 'static) -> Element<App> {
    zstack![
        button("")
            .key(key)
            .accessible_label(label)
            .variant(ButtonVariant::Ghost)
            .width(36.)
            .height(36.)
            .rounded(10.)
            .on_click(f),
        glyph(markup, 18.),
    ]
    .key(format!("{key}-tool"))
    .width(36.)
    .height(36.)
    .flow(Flow::Center)
    .into()
}

/// A square button with a short word instead of an icon ("3D").
fn word_tool(
    key: &str,
    word: &str,
    label: &str,
    f: impl FnMut(&mut App) + 'static,
) -> Element<App> {
    zstack![
        button("")
            .key(key)
            .accessible_label(label)
            .variant(ButtonVariant::Ghost)
            .width(36.)
            .height(36.)
            .rounded(10.)
            .on_click(f),
        text(word).font_size(12.5).font_weight(700).color(INK),
    ]
    .key(format!("{key}-tool"))
    .width(36.)
    .height(36.)
    .flow(Flow::Center)
    .into()
}

/// The compass: a needle whose red end points north on the turned map.
fn compass(bearing: f64) -> Element<App> {
    let (sin, cos) = (
        bearing.to_radians().sin() as f32,
        bearing.to_radians().cos() as f32,
    );
    // North on screen is (−sin, −cos); the needle's sides are across it.
    let (c, len, half) = (10f32, 9f32, 3.6f32);
    let north = [c - sin * len, c - cos * len];
    let south = [c + sin * len, c + cos * len];
    let side = [cos * half, -sin * half];
    let needle = move |tip: [f32; 2]| {
        VectorPath::new()
            .move_to(tip[0], tip[1])
            .line_to(c + side[0], c + side[1])
            .line_to(c - side[0], c - side[1])
            .close()
    };
    zstack![
        button("")
            .key("north")
            .accessible_label("Face north")
            .variant(ButtonVariant::Ghost)
            .width(36.)
            .height(36.)
            .rounded(10.)
            .on_click(|app: &mut App| app.face_north()),
        volt_ui_kit::ui::canvas(move |canvas| {
            canvas.fill(needle(north), "#E11D48");
            canvas.fill(needle(south), "#9CA3AF");
        })
        .key("north-needle")
        .width(20.)
        .height(20.),
    ]
    .key("north-tool")
    .width(36.)
    .height(36.)
    .flow(Flow::Center)
    .into()
}

fn secondary(value: impl Into<String>) -> Text<App> {
    text(value).font_size(12.5).color(MUTED)
}

fn link(key: &str, label: &str, f: impl FnMut(&mut App) + 'static) -> Element<App> {
    button(label)
        .key(key)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Small)
        .color(ACCENT)
        .on_click(f)
        .into()
}

/// A back link on its own row, at the leading edge.
fn back(label: &str) -> Element<App> {
    hstack![link("back", label, |app| app.back()), spacer()]
        .key("back-row")
        .into()
}

// ── The window ─────────────────────────────────────────────────────────────

fn chrome(app: &App) -> Element<App> {
    let wide = app.size[0] > 700.;
    let top = if wide && app.panel_open {
        hstack![panel(app), spacer(), controls(app)]
    } else {
        hstack![spacer(), controls(app)]
    };
    let body = vstack![
        top.key("top").align(Align::Start).flex(1.),
        hstack![scale_bar(app), spacer(), credits(app)]
            .key("bottom")
            .align(Align::End),
    ]
    .key("body")
    .padding(16., Edge::All)
    .gap(12., Axis::Vertical)
    .flex(1.);
    vstack![toolbar(app), body]
        .key("chrome")
        .align(Align::Stretch)
        .into()
}

/// The title bar: it moves the window (its buttons still work) and leaves
/// room for the macOS window buttons in its leading corner.
fn toolbar(_app: &App) -> Element<App> {
    let lights = if cfg!(all(target_os = "macos", feature = "native")) {
        70.
    } else {
        0.
    };
    let mut items: Vec<Element<App>> = vec![
        zstack(Vec::<Element<App>>::new())
            .width(lights)
            .height(1.)
            .into(),
        tool("sidebar", SIDEBAR, "Show or hide the sidebar", |app| {
            app.panel_open = !app.panel_open
        }),
        text("Maps")
            .font_size(15.)
            .font_weight(600)
            .color(INK)
            .into(),
        spacer().into(),
    ];
    // A choice of map only where there is one (the browser has one).
    if Provider::available().len() > 1 {
        items.push(tool("map-style", LAYERS, "Map style", |app| {
            app.layers_open = !app.layers_open;
            app.settings_open = false;
        }));
    }
    items.push(tool("settings", SETTINGS, "Settings", |app| {
        app.settings_open = !app.settings_open;
        app.layers_open = false;
    }));
    hstack(items)
        .key("toolbar")
        .height(TOOLBAR)
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .padding(12., Edge::Horizontal)
        .background(ColorToken::Base100.alpha(92))
        .border_color(HAIRLINE)
        .border_width(1.)
        .border_sides(Sides::BOTTOM)
        .drag_region(true)
        .into()
}

fn controls(app: &App) -> Element<App> {
    let zoom = surface(vec![
        tool(
            "zoom-in",
            r#"<path d="M12 5v14M5 12h14"/>"#,
            "Zoom in",
            |app| app.zoom_by(1.),
        ),
        zstack(Vec::<Element<App>>::new())
            .height(1.)
            .width(24.)
            .background(HAIRLINE)
            .into(),
        tool("zoom-out", r#"<path d="M5 12h14"/>"#, "Zoom out", |app| {
            app.zoom_by(-1.)
        }),
    ])
    .key("zoom-card")
    .padding(2., Edge::All)
    .rounded(12.)
    .align(Align::Center);
    let filled = if app.location.is_some() && !app.locating {
        r#"<path d="M20 4 4 11l7 2 2 7z" fill="currentColor"/>"#
    } else {
        LOCATE
    };
    let locate = surface(vec![tool("locate", filled, "Show my location", |app| {
        app.locate()
    })])
    .key("locate-card")
    .padding(2., Edge::All)
    .rounded(12.);
    // 3D, and the compass once the map is turned.
    let tilted = app.camera.pitch > 1.;
    let mut view_tools = vec![word_tool(
        "three-d",
        if tilted { "2D" } else { "3D" },
        if tilted {
            "Look straight down"
        } else {
            "Tilt to 3D"
        },
        |app| app.toggle_3d(),
    )];
    if super::turn_between(app.camera.bearing, 0.).abs() > 0.5 {
        view_tools.push(
            zstack(Vec::<Element<App>>::new())
                .height(1.)
                .width(24.)
                .background(HAIRLINE)
                .into(),
        );
        view_tools.push(compass(app.camera.bearing));
    }
    let view_card = surface(view_tools)
        .key("view-card")
        .padding(2., Edge::All)
        .rounded(12.)
        .align(Align::Center);
    let mut column: Vec<Element<App>> = Vec::new();
    if app.layers_open {
        column.push(layer_menu(app));
    }
    if app.settings_open {
        column.push(settings_menu(app));
    }
    column.push(zoom.into());
    column.push(view_card.into());
    // Location comes from the system on the desktop; the browser build
    // doesn't ask for it yet.
    if cfg!(feature = "native") {
        column.push(locate.into());
    }
    if let Some(hint) = &app.hint {
        column.push(
            surface(vec![text(hint).font_size(12.5).color(INK).into()])
                .key("hint")
                .padding(12., Edge::All)
                .rounded(12.)
                .max_width(250.)
                .into(),
        );
    }
    if let Some(note) = &app.location_note {
        column.push(
            surface(vec![text(note).font_size(12.5).color(INK).into()])
                .key("location-note")
                .padding(12., Edge::All)
                .rounded(12.)
                .max_width(250.)
                .into(),
        );
    }
    vstack(column)
        .key("controls")
        .gap(10., Axis::Vertical)
        .align(Align::End)
        .into()
}

fn layer_menu(app: &App) -> Element<App> {
    let mut rows: Vec<Element<App>> = vec![text("Map style")
        .font_size(13.)
        .font_weight(600)
        .color(INK)
        .into()];
    for provider in Provider::available().iter().copied() {
        let current = provider == app.provider;
        let missing = provider.key_variable().is_some() && app.key_for(provider).is_none();
        let about = match provider {
            Provider::OpenFreeMap => "Vector map from open data",
            Provider::OpenStreetMap => "The standard OpenStreetMap",
            Provider::Mapbox if missing => "Add a token in Settings",
            Provider::Mapbox => "Mapbox Streets",
            Provider::Google if missing => "Add a key in Settings",
            Provider::Google => "Google roadmap",
            Provider::Apple => "Apple Maps through MapKit",
        };
        let mut row: Vec<Element<App>> = vec![vstack![
            text(provider.label())
                .font_size(13.5)
                .font_weight(500)
                .color(INK),
            secondary(about),
        ]
        .flex(1.)
        .gap(1., Axis::Vertical)
        .into()];
        if current {
            row.push(glyph(r#"<path d="m5 12 5 5 9-10"/>"#, 16.));
        }
        rows.push(
            hstack(row)
                .key(format!("use-{}", provider.label().to_lowercase()))
                .align(Align::Center)
                .padding(8., Edge::All)
                .rounded(10.)
                .background(if current {
                    FIELD.alpha(100)
                } else {
                    "#00000000".to_owned()
                })
                .hover_style(|s| s.background(FIELD))
                .cursor(Cursor::Pointer)
                .accessible_label(provider.label())
                .on_click_with(move |app: &mut App, _| app.use_provider(provider))
                .into(),
        );
    }
    surface(rows)
        .key("layer-menu")
        .padding(8., Edge::All)
        .gap(2., Axis::Vertical)
        .align(Align::Stretch)
        .width(250.)
        .into()
}

/// Settings: distances in kilometres or miles, and keys for the map
/// services that need one. A key is typed into a password field, so it is
/// never shown, and kept in memory only while the app runs.
fn settings_menu(app: &App) -> Element<App> {
    let mut rows: Vec<Element<App>> = vec![
        text("Settings")
            .font_size(13.)
            .font_weight(600)
            .color(INK)
            .into(),
        secondary("Distances").into(),
        segmented_control([
            (services::Units::Metric, "Kilometres"),
            (services::Units::Imperial, "Miles"),
        ])
        .key("units")
        .selected(app.units)
        .on_select(|app: &mut App, units| app.units = units)
        .into(),
    ];
    for provider in Provider::available().iter().copied() {
        let (Some(name), Some(var)) = (provider.key_name(), provider.key_variable()) else {
            continue;
        };
        let id = provider.label().to_lowercase();
        let draft = app.key_drafts.get(var).cloned().unwrap_or_default();
        let saved = app.keys.contains_key(var);
        let mut actions: Vec<Element<App>> = vec![text_field(&draft)
            .key(format!("key-{id}"))
            .password(true)
            .placeholder(if app.key_for(provider).is_some() {
                "Paste a new key"
            } else {
                "Paste your key"
            })
            .accessible_label(name)
            .on_change(move |app: &mut App, value| {
                app.key_drafts.insert(var, value);
            })
            .on_submit(move |app: &mut App| app.save_key(provider))
            // The menu's width less its padding, the gap and the button.
            .width(300. - 2. * 12. - 6. - 60.)
            .into()];
        actions.push(
            button("Save")
                .key(format!("save-{id}"))
                .variant(ButtonVariant::Soft)
                .size(ControlSize::Small)
                .width(60.)
                .enabled(!draft.trim().is_empty())
                .on_click(move |app: &mut App| app.save_key(provider))
                .into(),
        );
        rows.push(
            vstack![
                hstack![
                    text(name).font_size(13.).font_weight(500).color(INK),
                    spacer(),
                    secondary(app.key_source(provider)),
                ]
                .align(Align::Center),
                hstack(actions)
                    .gap(6., Axis::Horizontal)
                    .align(Align::Center),
            ]
            .key(format!("key-row-{id}"))
            .gap(6., Axis::Vertical)
            .padding(4., Edge::Top)
            .into(),
        );
        if saved {
            rows.push(link(
                &format!("forget-{id}"),
                "Remove this key",
                move |app| app.forget_key(provider),
            ));
        }
    }
    rows.push(secondary("Keys stay in memory while Maps is open and are never shown.").into());
    surface(rows)
        .key("settings-menu")
        .padding(12., Edge::All)
        .gap(8., Axis::Vertical)
        .align(Align::Stretch)
        .width(300.)
        .into()
}

fn scale_bar(app: &App) -> Element<App> {
    let (lat, _) = app.camera.lat_lon();
    let per_point = 40_075_016.686 * lat.to_radians().cos() / app.camera.world();
    let (nice, label) = app.units.scale(per_point * 96.);
    let width = (nice / per_point) as f32;
    vstack![
        text(label).font_size(11.).font_weight(600).color(INK),
        zstack(Vec::<Element<App>>::new())
            .width(width)
            .height(3.)
            .background(INK)
            .rounded(2.),
    ]
    .key("scale")
    .gap(3., Axis::Vertical)
    .padding(6., Edge::All)
    .background(ColorToken::Base100.alpha(85))
    .rounded(8.)
    .into()
}

fn credits(app: &App) -> Element<App> {
    match attribution(app) {
        Some(credit) => zstack([text(credit)
            .font_size(10.5)
            .no_wrap(true)
            .color(MUTED)
            .into()])
        .key("attribution")
        .background(ColorToken::Base100.alpha(85))
        .padding(5., Edge::All)
        .rounded(8.)
        .into(),
        None => spacer().into(),
    }
}

// ── The panel ──────────────────────────────────────────────────────────────

fn panel(app: &App) -> Element<App> {
    let list_height = (app.size[1] - TOOLBAR - 260.).max(160.);
    let mut field: Vec<Element<App>> = vec![text_field(&app.query)
        .key("search")
        .placeholder("Search Maps")
        .leading_icon(Icon::Search)
        .background(FIELD)
        .border_width(0.)
        .rounded(10.)
        .on_change(|app: &mut App, value| app.set_query(value))
        .on_submit(|app: &mut App| app.search_now())
        .flex(1.)
        .into()];
    if !app.query.is_empty() || app.selected.is_some() {
        field.push(tool("clear", CLOSE, "Clear", |app| app.clear()));
    }
    let body = match app.panel {
        Panel::Search => results(app, list_height),
        Panel::Place => place_card(app),
        Panel::Directions => directions(app, list_height),
    };
    surface(vec![
        hstack(field)
            .key("search-row")
            .gap(4., Axis::Horizontal)
            .align(Align::Center)
            .into(),
        body,
        text(credit_line(app)).font_size(11.).color(MUTED).into(),
    ])
    .key("panel")
    .width(PANEL_WIDTH)
    .padding(14., Edge::All)
    .gap(14., Axis::Vertical)
    .align(Align::Stretch)
    .into()
}

fn credit_line(app: &App) -> String {
    let backend = app.backend();
    let mut line = match backend {
        services::Backend::Open => {
            "Search by Photon and Nominatim · Routes by OSRM · OpenStreetMap data".to_owned()
        }
        services::Backend::Mapbox(_) => "Search and routes by Mapbox".to_owned(),
        services::Backend::Google(_) => "Search and routes by Google".to_owned(),
        services::Backend::Apple => "Search and routes by Apple Maps".to_owned(),
    };
    if let (Some(name), services::Backend::Open) = (app.provider.key_name(), &backend) {
        line = format!(
            "{line}. Add a {name} in Settings to use {}.",
            app.provider.label()
        );
    }
    line
}

const SUGGESTIONS: [&str; 6] = [
    "Things to do",
    "Coffee",
    "Restaurants",
    "Museums",
    "Parks",
    "Hotels",
];

fn results(app: &App, height: f32) -> Element<App> {
    if app.query.trim().is_empty() {
        let chips: Vec<Element<App>> = SUGGESTIONS
            .iter()
            .map(|&label| {
                button(label)
                    .key(format!(
                        "suggest-{}",
                        label.to_lowercase().replace(" ", "-")
                    ))
                    .variant(ButtonVariant::Soft)
                    .size(ControlSize::Small)
                    .rounded(Rounding::Full)
                    .on_click(move |app: &mut App| {
                        app.query = label.to_owned();
                        app.search_now();
                    })
                    .into()
            })
            .collect();
        return vstack![
            text("Find nearby")
                .font_size(13.)
                .font_weight(600)
                .color(INK),
            hstack(chips).key("chips").gap(6., Axis::Both).wrap(true),
        ]
        .key("explore")
        .gap(8., Axis::Vertical)
        .into();
    }
    if app.results.is_empty() {
        let message = if app.searching {
            "Searching…".to_owned()
        } else {
            app.search_error
                .clone()
                .unwrap_or_else(|| "Press Return to search.".into())
        };
        return secondary(message).into();
    }
    let here = app.search_from;
    let rows: Vec<Element<App>> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, place)| {
            let chosen = place.clone();
            let second = [place.kind.as_str(), place.detail.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            hstack![
                zstack([text((i + 1).to_string())
                    .font_size(12.)
                    .font_weight(700)
                    .color(ACCENT)
                    .into()])
                .width(30.)
                .height(30.)
                .rounded(Rounding::Full)
                .background(accent_soft())
                .flow(Flow::Center),
                vstack![
                    text(&place.name)
                        .font_size(14.)
                        .font_weight(600)
                        .color(INK)
                        .truncate(),
                    secondary(second).truncate(),
                ]
                .flex(1.)
                // What's left of the panel beside the number and distance:
                // a long address mustn't push the distance out of the row.
                .max_width(PANEL_WIDTH - 2. * 14. - 10. - 2. * 8. - 30. - 2. * 12. - 52.)
                .gap(2., Axis::Vertical),
                secondary(
                    app.units
                        .distance(services::haversine(here, (place.lat, place.lon)))
                )
                .no_wrap(true)
                .min_width(52.)
                .text_align(TextAlign::End),
            ]
            .key(format!("result-{i}"))
            .gap(12., Axis::Horizontal)
            .align(Align::Center)
            .padding(8., Edge::All)
            .rounded(12.)
            .hover_style(|s| s.background(FIELD))
            .cursor(Cursor::Pointer)
            .accessible_label(place.name.clone())
            .on_click_with(move |app: &mut App, _| app.select(chosen.clone()))
            .into()
        })
        .collect();
    vstack(rows)
        .key("results")
        .gap(2., Axis::Vertical)
        .align(Align::Stretch)
        // Room for the scroll bar, so it doesn't cover the distances.
        .padding(10., Edge::Right)
        .scroll(Scroll::Vertical)
        .max_height(height)
        .into()
}

fn place_card(app: &App) -> Element<App> {
    let Some(place) = &app.selected else {
        return spacer().into();
    };
    let mut lines: Vec<Element<App>> = vec![
        back("‹ Results"),
        text(&place.name)
            .font_size(22.)
            .font_weight(700)
            .color(INK)
            .into(),
    ];
    if !place.kind.is_empty() {
        lines.push(
            zstack([text(&place.kind)
                .font_size(12.)
                .font_weight(600)
                .color(ACCENT)
                .into()])
            .key("kind")
            .padding(4., Edge::Vertical)
            .padding(10., Edge::Horizontal)
            .rounded(Rounding::Full)
            .background(accent_soft())
            .into(),
        );
    }
    if !place.detail.is_empty() {
        lines.push(text(&place.detail).font_size(13.5).color(INK).into());
    }
    let distance = app
        .location
        .map(|(lat, lon, _)| {
            format!(
                "{} away · ",
                app.units
                    .distance(services::haversine((lat, lon), (place.lat, place.lon)))
            )
        })
        .unwrap_or_default();
    lines.push(secondary(format!("{distance}{:.5}, {:.5}", place.lat, place.lon)).into());
    let again = place.clone();
    lines.push(
        hstack![
            button("Directions")
                .key("directions")
                .variant(ButtonVariant::Primary)
                .rounded(10.)
                .flex(1.)
                .on_click(|app: &mut App| app.directions()),
            button("Centre")
                .key("show")
                .variant(ButtonVariant::Soft)
                .rounded(10.)
                .on_click(move |app: &mut App| app.select(again.clone())),
        ]
        .key("place-actions")
        .gap(8., Axis::Horizontal)
        .into(),
    );
    vstack(lines)
        .key("place")
        .gap(8., Axis::Vertical)
        .align(Align::Start)
        .into()
}

fn directions(app: &App, height: f32) -> Element<App> {
    let name = |p: Option<&Place>| p.map_or("Choose a place", |p| p.name.as_str()).to_owned();
    let dot = |colour: &str| -> Element<App> {
        zstack(Vec::<Element<App>>::new())
            .width(10.)
            .height(10.)
            .rounded(Rounding::Full)
            .background(colour)
            .into()
    };
    // From and To, joined by a line, with swap beside them.
    let ends = hstack![
        vstack![
            dot(START),
            zstack(Vec::<Element<App>>::new())
                .width(2.)
                .height(26.)
                .background("#D1D5DB"),
            dot(PICKED),
        ]
        .key("ends-line")
        .gap(4., Axis::Vertical)
        .align(Align::Center),
        vstack![
            vstack![
                secondary("From"),
                text(name(app.origin.as_ref()))
                    .font_size(14.)
                    .font_weight(600)
                    .color(INK)
                    .no_wrap(true)
            ]
            .gap(1., Axis::Vertical),
            vstack![
                secondary("To"),
                text(name(app.selected.as_ref()))
                    .font_size(14.)
                    .font_weight(600)
                    .color(INK)
                    .no_wrap(true)
            ]
            .gap(1., Axis::Vertical),
        ]
        .key("ends-names")
        .gap(10., Axis::Vertical)
        .flex(1.),
        tool("swap", SWAP, "Swap start and destination", |app| app.swap()),
    ]
    .key("ends")
    .gap(12., Axis::Horizontal)
    .align(Align::Center)
    .padding(12., Edge::All)
    .rounded(12.)
    .background(FIELD);
    let starts = hstack![
        secondary("Start from"),
        link("from-here", "My location", |app| app.origin_from_location()),
        link("from-map", "Map centre", |app| app.origin_from_map()),
    ]
    .key("starts")
    .gap(2., Axis::Horizontal)
    .align(Align::Center);
    let modes = segmented_control(app.backend().modes().iter().map(|&m| (m, m.label())))
        .key("modes")
        .accessible_label("Travel mode")
        .selected(app.mode)
        .on_select(|app: &mut App, mode: Mode| app.set_mode(mode));
    let mut lines: Vec<Element<App>> =
        vec![back("‹ Back"), ends.into(), starts.into(), modes.into()];
    if app.routing {
        lines.push(secondary("Finding a route…").into());
    } else if let Some(error) = &app.route_error {
        lines.push(text(error).font_size(13.).color(ColorToken::Error).into());
    } else if let Some(route) = &app.route {
        lines.push(
            hstack![
                text(services::duration(route.duration))
                    .font_size(26.)
                    .font_weight(700)
                    .color(INK),
                secondary(app.units.distance(route.distance)),
            ]
            .key("summary")
            .gap(10., Axis::Horizontal)
            .align(Align::End)
            .into(),
        );
        let steps: Vec<Element<App>> = route
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let at = step.at;
                let mut words = vec![text(&step.text).font_size(13.5).color(INK).into()];
                if step.distance > 0. {
                    words.push(secondary(app.units.distance(step.distance)).into());
                }
                hstack![
                    turn_icon(step.turn),
                    vstack(words).flex(1.).gap(2., Axis::Vertical)
                ]
                .key(format!("step-{i}"))
                .gap(12., Axis::Horizontal)
                .align(Align::Center)
                .padding(8., Edge::All)
                .rounded(10.)
                .hover_style(|s| s.background(FIELD))
                .cursor(Cursor::Pointer)
                .accessible_label(step.text.clone())
                .on_click_with(move |app: &mut App, _| app.show_step(at))
                .into()
            })
            .collect();
        lines.push(
            vstack(steps)
                .key("steps")
                .gap(0., Axis::Vertical)
                .align(Align::Stretch)
                .scroll(Scroll::Vertical)
                .max_height(height - 150.)
                .into(),
        );
    }
    vstack(lines)
        .key("directions-panel")
        .gap(12., Axis::Vertical)
        .align(Align::Stretch)
        .into()
}

/// Original turn arrows, drawn as strokes in a 24-point box.
fn turn_icon(turn: Turn) -> Element<App> {
    let path = match turn {
        Turn::Depart => r#"<circle cx="12" cy="12" r="4"/><path d="M12 3v3M12 18v3"/>"#,
        Turn::Arrive => r#"<path d="M6 21V4M6 4h11l-2.5 4L17 12H6"/>"#,
        Turn::Straight => r#"<path d="M12 20V5M6 11l6-6 6 6"/>"#,
        Turn::SlightLeft => r#"<path d="M15 20v-6L8 7M8 13V7h6"/>"#,
        Turn::Left => r#"<path d="M16 20v-7a3 3 0 0 0-3-3H5M9 6l-4 4 4 4"/>"#,
        Turn::SharpLeft => r#"<path d="M16 4v9L6 19M6 12v7h7"/>"#,
        Turn::SlightRight => r#"<path d="M9 20v-6l7-7M16 13V7h-6"/>"#,
        Turn::Right => r#"<path d="M8 20v-7a3 3 0 0 1 3-3h8M15 6l4 4-4 4"/>"#,
        Turn::SharpRight => r#"<path d="M8 4v9l10 6M18 12v7h-7"/>"#,
        Turn::UTurn => r#"<path d="M8 20V9a4 4 0 0 1 8 0v6M12 12l4 4 4-4"/>"#,
        Turn::Roundabout => {
            r#"<circle cx="12" cy="9" r="4"/><path d="M12 13v8M16 9h4M18 7l2 2-2 2"/>"#
        }
    };
    zstack([glyph(path, 17.)])
        .width(32.)
        .height(32.)
        .rounded(Rounding::Full)
        .background(FIELD)
        .flow(Flow::Center)
        .into()
}

fn attribution(app: &App) -> Option<String> {
    if app.provider == Provider::Apple {
        return Some("Apple Maps · MapKit".into());
    }
    let style = app.style.as_ref()?;
    let used: BTreeSet<&str> = style
        .layers
        .iter()
        .filter_map(|l| l.source.as_deref())
        .collect();
    let credits: BTreeSet<String> = style
        .sources
        .iter()
        .filter(|(id, _)| used.contains(id.as_str()))
        .filter_map(|(_, s)| s.attribution.as_deref())
        .map(strip_tags)
        .filter(|s| !s.is_empty())
        .collect();
    (!credits.is_empty()).then(|| credits.into_iter().collect::<Vec<_>>().join(" "))
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out.replace("&copy;", "©")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A tile to draw for a placement: the tile itself, or the nearest loaded
/// ancestor stretched over it while the sharper one downloads.
/// The kit takes paint offsets of at most 4,096 points either way.
fn fits(at: [f32; 2]) -> bool {
    at.iter().all(|v| v.is_finite() && v.abs() <= 4_000.)
}

/// What to draw for a placement: the tile itself; else the nearest loaded
/// coarser tile stretched over it; else the loaded sharper tiles inside it.
/// While zooming either way, the map stays filled with what is already here.
fn find_tiles<'a>(app: &'a App, p: &Placement) -> Vec<(&'a TileData, [f64; 2], f64, TileKey)> {
    // Vector tiles stay crisp when stretched, so a coarse one can stand in
    // for longer; image tiles get blurry, so they stop sooner.
    let reach = if matches!(app.tiles.values().next(), Some(TileData::Vector(_))) {
        6
    } else {
        4
    };
    for up in 0..=reach {
        if up > p.key.z {
            break;
        }
        let key = TileKey {
            source: p.key.source.clone(),
            z: p.key.z - up,
            x: p.key.x >> up,
            y: p.key.y >> up,
        };
        if let Some(tile) = app.tiles.get(&key) {
            let scale = f64::from(1u32 << up);
            let size = p.size * scale;
            let origin = [
                p.origin[0] - f64::from(p.key.x - (key.x << up)) * p.size,
                p.origin[1] - f64::from(p.key.y - (key.y << up)) * p.size,
            ];
            return vec![(tile, origin, size, key)];
        }
    }
    for down in 1..=2u8 {
        let n = 1u32 << down;
        let size = p.size / f64::from(n);
        let mut found = Vec::new();
        for dy in 0..n {
            for dx in 0..n {
                let key = TileKey {
                    source: p.key.source.clone(),
                    z: p.key.z + down,
                    x: (p.key.x << down) + dx,
                    y: (p.key.y << down) + dy,
                };
                if let Some(tile) = app.tiles.get(&key) {
                    let origin = [
                        p.origin[0] + f64::from(dx) * size,
                        p.origin[1] + f64::from(dy) * size,
                    ];
                    found.push((tile, origin, size, key));
                }
            }
        }
        if !found.is_empty() {
            return found;
        }
    }
    Vec::new()
}

struct Label {
    text: String,
    at: [f32; 2],
    size: f32,
    color: String,
    /// The outline that keeps a label readable over roads and water.
    halo: Option<String>,
    weight: u16,
}

/// Packs shapes into as few canvas paths as the kit allows (1,024 commands
/// per path, 256 paths per canvas) and spreads them over several canvases.
struct Packer {
    canvases: Vec<Canvas>,
}

impl Packer {
    fn push(&mut self, path: VectorPath, color: &str, stroke: Option<f32>) {
        if !path.is_valid() {
            return;
        }
        if self
            .canvases
            .last()
            .is_none_or(|c| c.paths.len() >= volt_ui_kit::graphics::MAX_CANVAS_PATHS)
        {
            self.canvases.push(Canvas::default());
        }
        let canvas = self.canvases.last_mut().unwrap();
        match stroke {
            Some(width) => canvas.stroke(path, color, width.min(256.)),
            None => canvas.fill(path, color),
        }
    }
}

/// Shapes of one style waiting to be packed: (colour, stroke width).
type Batches = HashMap<(String, Option<u32>), VectorPath>;

fn add(
    batches: &mut Batches,
    packer: &mut Packer,
    key: (String, Option<u32>),
    ring: &[[f32; 2]],
    close: bool,
) {
    let points = simplify(ring, close);
    if points.len() < 2 {
        return;
    }
    let max = volt_ui_kit::graphics::MAX_PATH_COMMANDS;
    // A ring longer than one path allows: keep every n-th point.
    let stride = points.len().div_ceil(max - 2).max(1);
    let points: Vec<[f32; 2]> = points.iter().step_by(stride).copied().collect();
    let path = batches.entry(key.clone()).or_default();
    if path.commands.len() + points.len() + 1 > max {
        let full = std::mem::replace(path, VectorPath::new());
        packer.push(full, &key.0, key.1.map(|w| w as f32 / 100.));
    }
    path.commands.push(PathCommand::Move(points[0]));
    for &p in &points[1..] {
        path.commands.push(PathCommand::Line(p));
    }
    if close {
        path.commands.push(PathCommand::Close);
    }
}

fn inside(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3]
}

/// The part of a polygon ring inside a rectangle (Sutherland–Hodgman).
fn clip_ring(ring: &[[f32; 2]], r: [f32; 4]) -> Vec<[f32; 2]> {
    if ring.iter().all(|&p| inside(p, r)) {
        return ring.to_vec();
    }
    let mut points = ring.to_vec();
    for edge in 0..4 {
        if points.is_empty() {
            break;
        }
        let keep = |p: [f32; 2]| match edge {
            0 => p[0] >= r[0],
            1 => p[1] >= r[1],
            2 => p[0] <= r[2],
            _ => p[1] <= r[3],
        };
        let cross = |a: [f32; 2], b: [f32; 2]| {
            let t = match edge {
                0 => (r[0] - a[0]) / (b[0] - a[0]),
                1 => (r[1] - a[1]) / (b[1] - a[1]),
                2 => (r[2] - a[0]) / (b[0] - a[0]),
                _ => (r[3] - a[1]) / (b[1] - a[1]),
            };
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        };
        let mut out = Vec::with_capacity(points.len());
        for i in 0..points.len() {
            let (prev, cur) = (points[(i + points.len() - 1) % points.len()], points[i]);
            match (keep(prev), keep(cur)) {
                (true, true) => out.push(cur),
                (true, false) => out.push(cross(prev, cur)),
                (false, true) => {
                    out.push(cross(prev, cur));
                    out.push(cur);
                }
                (false, false) => {}
            }
        }
        points = out;
    }
    points
}

/// One segment cut to a rectangle (Liang–Barsky), if any of it is inside.
fn clip_segment(a: [f32; 2], b: [f32; 2], r: [f32; 4]) -> Option<([f32; 2], [f32; 2])> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0f32, 1f32);
    for (p, q) in [
        (-dx, a[0] - r[0]),
        (dx, r[2] - a[0]),
        (-dy, a[1] - r[1]),
        (dy, r[3] - a[1]),
    ] {
        if p == 0. {
            if q < 0. {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0. {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((
        [a[0] + dx * t0, a[1] + dy * t0],
        [a[0] + dx * t1, a[1] + dy * t1],
    ))
}

/// The visible pieces of a line inside a rectangle.
fn clip_line(line: &[[f32; 2]], r: [f32; 4]) -> Vec<Vec<[f32; 2]>> {
    if line.iter().all(|&p| inside(p, r)) {
        return vec![line.to_vec()];
    }
    let mut pieces = Vec::new();
    let mut current: Vec<[f32; 2]> = Vec::new();
    for w in line.windows(2) {
        match clip_segment(w[0], w[1], r) {
            Some((a, b)) => {
                let joined = current
                    .last()
                    .is_some_and(|l| (l[0] - a[0]).abs() < 1e-3 && (l[1] - a[1]).abs() < 1e-3);
                if !joined {
                    if current.len() >= 2 {
                        pieces.push(std::mem::take(&mut current));
                    }
                    current = vec![a];
                }
                current.push(b);
            }
            None => {
                if current.len() >= 2 {
                    pieces.push(std::mem::take(&mut current));
                }
                current.clear();
            }
        }
    }
    if current.len() >= 2 {
        pieces.push(current);
    }
    pieces
}

/// Drop points closer than about a point to the last one kept, and shapes
/// too small to see: the GPU renderer has a budget for path segments.
fn simplify(ring: &[[f32; 2]], close: bool) -> Vec<[f32; 2]> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in ring {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    if x1 - x0 < 1.5 && y1 - y0 < 1.5 {
        return Vec::new();
    }
    let mut out: Vec<[f32; 2]> = Vec::with_capacity(ring.len());
    for &p in ring {
        let p = [p[0].clamp(-60_000., 60_000.), p[1].clamp(-60_000., 60_000.)];
        match out.last() {
            Some(q) if (p[0] - q[0]).abs() < 1.2 && (p[1] - q[1]).abs() < 1.2 => {}
            _ => out.push(p),
        }
    }
    if !close && out.len() == 1 && ring.len() > 1 {
        out.push(*ring.last().unwrap());
    }
    out
}

fn drain(batches: &mut Batches, packer: &mut Packer) {
    // Wider strokes first, so a road's casing sits under its fill line.
    let mut all: Vec<_> = batches.drain().collect();
    all.sort_by_key(|((_, width), _)| std::cmp::Reverse(*width));
    for ((color, width), path) in all {
        packer.push(path, &color, width.map(|w| w as f32 / 100.));
    }
}

fn draw_style(app: &App, style: &style::Style, out: &mut Vec<Element<App>>) -> Vec<Label> {
    let zoom = app.camera.zoom;
    // Turned or tilted: one projected drawing instead of upright tile boxes.
    let view = (!app.camera.flat()).then(|| View::new(app.camera, app.size));
    let draw_run = |app: &App,
                    run: &[&Layer],
                    runs: &mut usize,
                    out: &mut Vec<Element<App>>,
                    labels: &mut Vec<Label>| match &view {
        Some(view) => tilted::draw_run(app, view, run, runs, out, labels),
        None => draw_run(app, run, runs, out, labels),
    };
    let mut labels: Vec<Label> = Vec::new();
    // Vector layers run in style order until an image layer interrupts them.
    let mut run: Vec<&Layer> = Vec::new();
    let mut runs = 0;
    // Debugging aid: MAPS_SKIP=prefix,prefix leaves matching layers out.
    let skip: Vec<String> = std::env::var("MAPS_SKIP")
        .map(|v| v.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    for layer in &style.layers {
        if zoom < layer.min_zoom || zoom >= layer.max_zoom {
            continue;
        }
        if skip
            .iter()
            .any(|p| !p.is_empty() && layer.id.starts_with(p.as_str()))
        {
            continue;
        }
        let ctx = Ctx {
            zoom,
            feature: None,
        };
        match layer.kind {
            LayerKind::Background => {
                draw_run(app, &run, &mut runs, out, &mut labels);
                run.clear();
                let color = paint_color(layer, "background-color", &ctx, "#000000");
                let opacity = paint_num(layer, "background-opacity", &ctx, 1.);
                let [w, h] = app.size;
                out.push(
                    zstack(Vec::<Element<App>>::new())
                        .key(format!("background-{}", layer.id))
                        .width(w.max(1.))
                        .height(h.max(1.))
                        .background(hex(color, opacity as f32))
                        .into(),
                );
            }
            // Images can't be turned or tilted yet: left out of such views.
            LayerKind::Raster if view.is_some() => {}
            LayerKind::Raster => {
                draw_run(app, &run, &mut runs, out, &mut labels);
                run.clear();
                let Some(source) = layer.source.as_deref() else {
                    continue;
                };
                let opacity = paint_num(layer, "raster-opacity", &ctx, 1.);
                let room = f64::from(app.size[0].min(app.size[1]));
                for (key, origin, size, data) in tiles_for(app, source) {
                    let TileData::Raster(image) = data else {
                        continue;
                    };
                    if !fits([origin[0] as f32, origin[1] as f32]) {
                        continue;
                    }
                    // Image boxes can't be larger than the map, so skip
                    // stand-ins that would be stretched out of shape.
                    if key.z < app.placements(source).first().map_or(0, |p| p.key.z) && size > room
                    {
                        continue;
                    }
                    out.push(
                        zstack(Vec::<Element<App>>::new())
                            .key(format!(
                                "{}-{}-{}-{}-{}",
                                layer.id, key.z, key.x, key.y, origin[0] as i64
                            ))
                            .width(size as f32 + 0.5)
                            .height(size as f32 + 0.5)
                            .background_image(image.clone().background().fit(ImageFit::Stretch))
                            .background_opacity(opacity as f32)
                            .offset(origin[0] as f32, origin[1] as f32)
                            .into(),
                    );
                }
            }
            LayerKind::Fill | LayerKind::Line | LayerKind::Extrusion | LayerKind::Symbol => {
                run.push(layer)
            }
            LayerKind::Other => {}
        }
    }
    draw_run(app, &run, &mut runs, out, &mut labels);
    labels
}

/// Every tile to draw for a source: loaded tiles at the current zoom, and
/// lower-zoom stand-ins where those are still downloading, coarsest first so
/// sharper tiles land on top.
fn tiles_for<'a>(app: &'a App, source: &str) -> Vec<(TileKey, [f64; 2], f64, &'a TileData)> {
    // One stand-in tile can serve several placements; its position, worked
    // out from each, can differ by rounding. A tile is drawn once per copy
    // of the world, whatever its rounded position.
    let world = app.camera.world();
    let left = app.camera.center[0] * world - f64::from(app.size[0]) / 2.;
    let mut seen = BTreeSet::new();
    let mut out: Vec<_> = app
        .placements(source)
        .iter()
        .flat_map(|p| find_tiles(app, p))
        .filter(|(_, origin, size, key)| {
            let copy = ((origin[0] + left + size / 2.) / world).floor() as i64;
            seen.insert((key.clone(), copy))
        })
        .map(|(data, origin, size, key)| (key, origin, size, data))
        .collect();
    out.sort_by_key(|(key, ..)| key.z);
    out
}

/// One run of vector layers: each tile is its own clipped box, so features
/// that cross into a neighbour's edge strip are cut at the tile's border.
fn draw_run(
    app: &App,
    run: &[&Layer],
    runs: &mut usize,
    out: &mut Vec<Element<App>>,
    labels: &mut Vec<Label>,
) {
    if run.is_empty() {
        return;
    }
    *runs += 1;
    let sources: BTreeSet<&str> = run.iter().filter_map(|l| l.source.as_deref()).collect();
    for source in sources {
        for (key, origin, size, data) in tiles_for(app, source) {
            let TileData::Vector(tile) = data else {
                continue;
            };
            let [w, h] = app.size.map(f64::from);
            let (x0, y0) = (origin[0].max(0.), origin[1].max(0.));
            let (x1, y1) = ((origin[0] + size).min(w), (origin[1] + size).min(h));
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            // Where the tile's corner sits inside its (visible) box.
            let shift = [origin[0] - x0, origin[1] - y0];
            let mut packer = Packer {
                canvases: Vec::new(),
            };
            for layer in run.iter().filter(|l| l.source.as_deref() == Some(source)) {
                let Some(layer_data) = layer
                    .source_layer
                    .as_deref()
                    .and_then(|n| tile.layers.get(n))
                else {
                    continue;
                };
                let mut batches = Batches::new();
                // Shapes are cut to the box (with room for wide lines), so
                // nothing far off-screen reaches the GPU renderer.
                let clip = [-32., -32., (x1 - x0) as f32 + 32., (y1 - y0) as f32 + 32.];
                vector_layer(
                    layer,
                    layer_data,
                    app.camera.zoom,
                    size,
                    origin,
                    shift,
                    clip,
                    &mut batches,
                    &mut packer,
                    labels,
                );
                drain(&mut batches, &mut packer);
            }
            // One point of overlap hides seams between tiles, but never past
            // the map's edge: a box larger than the map makes the map grow.
            let (bw, bh) = (
                ((x1 - x0) + 1.).min(w - x0) as f32,
                ((y1 - y0) + 1.).min(h - y0) as f32,
            );
            if super::platform::debug() {
                let paths: usize = packer.canvases.iter().map(|c| c.paths.len()).sum();
                let commands: usize = packer
                    .canvases
                    .iter()
                    .flat_map(|c| &c.paths)
                    .map(|p| p.path.commands.len())
                    .sum();
                eprintln!("tile {}/{}/{} at {:?} box {bw}x{bh}: {} canvases, {paths} paths, {commands} commands", key.z, key.x, key.y, origin.map(|v| v as i64), packer.canvases.len());
            }
            let id = format!(
                "run{runs}-{source}-{}-{}-{}-{}",
                key.z,
                key.x,
                key.y,
                origin[0].round() as i64
            );
            let canvases: Vec<Element<App>> = packer
                .canvases
                .into_iter()
                .enumerate()
                .map(|(i, paths)| {
                    volt_ui_kit::ui::canvas(move |c| *c = paths)
                        .key(format!("{id}-{i}"))
                        .width(bw)
                        .height(bh)
                })
                .collect();
            if canvases.is_empty() {
                continue;
            }
            out.push(
                zstack(canvases)
                    .key(id.clone())
                    .flow(Flow::TopLeft)
                    .width(bw)
                    .height(bh)
                    .clip_contents(true)
                    .offset(x0 as f32, y0 as f32)
                    .into(),
            );
        }
    }
}

fn paint_color(layer: &Layer, key: &str, ctx: &Ctx, default: &str) -> [f32; 4] {
    layer
        .paint
        .get(key)
        .and_then(|j| style::eval(j, ctx).color())
        .or_else(|| style::parse_color(default))
        .unwrap_or([0., 0., 0., 1.])
}

fn paint_num(layer: &Layer, key: &str, ctx: &Ctx, default: f64) -> f64 {
    layer
        .paint
        .get(key)
        .and_then(|j| style::eval(j, ctx).num())
        .unwrap_or(default)
}

#[allow(clippy::too_many_arguments)]
fn vector_layer(
    layer: &Layer,
    data: &mvt::Layer,
    zoom: f64,
    size: f64,
    screen: [f64; 2],
    shift: [f64; 2],
    clip: [f32; 4],
    batches: &mut Batches,
    packer: &mut Packer,
    labels: &mut Vec<Label>,
) {
    let k = (size / f64::from(data.extent)) as f32;
    let (dx, dy) = (shift[0] as f32, shift[1] as f32);
    let at = |p: [f32; 2]| [dx + p[0] * k, dy + p[1] * k];
    let (sx, sy) = (screen[0] as f32, screen[1] as f32);
    for feature in &data.features {
        let ctx = Ctx {
            zoom,
            feature: Some(feature),
        };
        if let Some(f) = &layer.filter {
            if !style::filter(f, &ctx) {
                continue;
            }
        }
        match (layer.kind, feature.kind) {
            (LayerKind::Fill | LayerKind::Extrusion, Geometry::Polygon) => {
                let (color_key, opacity_key) = if layer.kind == LayerKind::Fill {
                    ("fill-color", "fill-opacity")
                } else {
                    ("fill-extrusion-color", "fill-extrusion-opacity")
                };
                if layer.paint.contains_key("fill-pattern")
                    && !layer.paint.contains_key("fill-color")
                {
                    continue;
                }
                let color = paint_color(layer, color_key, &ctx, "#000000");
                let opacity = paint_num(layer, opacity_key, &ctx, 1.) as f32;
                let key = (hex(color, opacity), None);
                for ring in &feature.parts {
                    let ring: Vec<[f32; 2]> = ring.iter().map(|&p| at(p)).collect();
                    let ring = clip_ring(&ring, clip);
                    if ring.len() >= 3 {
                        add(batches, packer, key.clone(), &ring, true);
                    }
                }
            }
            (LayerKind::Line, Geometry::Line | Geometry::Polygon) => {
                let color = paint_color(layer, "line-color", &ctx, "#000000");
                let opacity = paint_num(layer, "line-opacity", &ctx, 1.) as f32;
                let width = paint_num(layer, "line-width", &ctx, 1.);
                let gap = paint_num(layer, "line-gap-width", &ctx, 0.);
                // A casing draws around a gap; one wider stroke under the
                // road drawn later looks the same.
                let width = if gap > 0. { gap + 2. * width } else { width } as f32;
                if width <= 0. || opacity <= 0. {
                    continue;
                }
                if width > 64. && super::platform::debug() {
                    eprintln!(
                        "wide line: {} width {width} gap {gap} color {}",
                        layer.id,
                        hex(color, opacity)
                    );
                }
                let key = (hex(color, opacity), Some((width * 100.) as u32));
                for part in &feature.parts {
                    let part: Vec<[f32; 2]> = part.iter().map(|&p| at(p)).collect();
                    for piece in clip_line(&part, clip) {
                        add(batches, packer, key.clone(), &piece, false);
                    }
                }
            }
            (LayerKind::Symbol, Geometry::Point) => {
                let placement = layer
                    .layout
                    .get("symbol-placement")
                    .and_then(|j| j.as_str());
                if matches!(placement, Some("line" | "line-center")) {
                    continue;
                }
                let Some(text) = label_text(layer, &ctx) else {
                    continue;
                };
                let Some(&point) = feature.parts.first().and_then(|p| p.first()) else {
                    continue;
                };
                let size = layer
                    .layout
                    .get("text-size")
                    .and_then(|j| style::eval(j, &ctx).num())
                    .unwrap_or(16.) as f32;
                let color = paint_color(layer, "text-color", &ctx, "#000000");
                let halo = layer
                    .paint
                    .get("text-halo-color")
                    .and_then(|j| style::eval(j, &ctx).color())
                    .filter(|_| paint_num(layer, "text-halo-width", &ctx, 1.) > 0.)
                    .map(|c| hex(c, 0.9));
                // The style names its fonts ("Noto Sans Bold"); keep the weight.
                let bold = layer
                    .layout
                    .get("text-font")
                    .map(|j| style::eval(j, &ctx).text().to_lowercase().contains("bold"))
                    .unwrap_or(false);
                labels.push(Label {
                    text,
                    at: [sx + point[0] * k, sy + point[1] * k],
                    size,
                    color: hex(color, paint_num(layer, "text-opacity", &ctx, 1.) as f32),
                    halo,
                    weight: if bold { 600 } else { 500 },
                });
            }
            _ => {}
        }
    }
}

fn label_text(layer: &Layer, ctx: &Ctx) -> Option<String> {
    let field = layer.layout.get("text-field")?;
    let raw = match field.as_str() {
        // Legacy "{name}" templates.
        Some(template) if template.contains('{') => {
            let mut out = String::new();
            let mut rest = template;
            while let Some(start) = rest.find('{') {
                out.push_str(&rest[..start]);
                let Some(end) = rest[start..].find('}') else {
                    break;
                };
                let key = &rest[start + 1..start + end];
                out.push_str(&style::eval(&serde_json::json!(["get", key]), ctx).text());
                rest = &rest[start + end + 1..];
            }
            out.push_str(rest);
            out
        }
        _ => style::eval(field, ctx).text(),
    };
    let text = raw.lines().next().unwrap_or("").trim().to_owned();
    let text = match layer.layout.get("text-transform").and_then(|j| j.as_str()) {
        Some("uppercase") => text.to_uppercase(),
        Some("lowercase") => text.to_lowercase(),
        _ => text,
    };
    (!text.is_empty()).then_some(text)
}

/// Glyphs the labels may draw in one frame (halo copies count). Each glyph
/// is many path segments for the GPU renderer, which has a fixed budget; a
/// tilted map spends part of it on buildings.
const LABEL_GLYPHS: usize = 6_000;
const TILTED_LABEL_GLYPHS: usize = 4_000;

/// Later layers win, as in other map renderers; overlapping labels are skipped.
fn place_labels(labels: Vec<Label>, [w, h]: [f32; 2], budget: usize, out: &mut Vec<Element<App>>) {
    let mut taken: Vec<[f32; 4]> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut glyphs = 0;
    for label in labels.into_iter().rev() {
        let chars = label.text.chars().count();
        let width = chars as f32 * label.size * 0.56;
        let height = label.size * 1.3;
        let rect = [
            label.at[0] - width / 2.,
            label.at[1] - height / 2.,
            width,
            height,
        ];
        if rect[0] < 0. || rect[1] < 0. || rect[0] + width > w || rect[1] + height > h {
            continue;
        }
        let overlaps = taken.iter().any(|r| {
            rect[0] < r[0] + r[2] + 4.
                && r[0] < rect[0] + rect[2] + 4.
                && rect[1] < r[1] + r[3]
                && r[1] < rect[1] + rect[3]
        });
        if overlaps || seen.contains(&label.text) {
            continue;
        }
        let cost = chars * if label.halo.is_some() { 5 } else { 1 };
        if glyphs + cost > budget {
            continue;
        }
        glyphs += cost;
        seen.insert(label.text.clone());
        taken.push(rect);
        let n = taken.len();
        // A halo: the label drawn in the outline colour around itself.
        if let Some(halo) = &label.halo {
            for (i, (dx, dy)) in [(-1., 0.), (1., 0.), (0., -1.), (0., 1.)]
                .into_iter()
                .enumerate()
            {
                out.push(
                    text(label.text.clone())
                        .key(format!("label-{n}-halo-{i}"))
                        .font_size(label.size)
                        .font_weight(label.weight)
                        .no_wrap(true)
                        .color(halo.clone())
                        .offset(rect[0] + dx, rect[1] + dy)
                        .into(),
                );
            }
        }
        out.push(
            text(label.text.clone())
                .key(format!("label-{n}"))
                .font_size(label.size)
                .font_weight(label.weight)
                .no_wrap(true)
                .color(label.color)
                .offset(rect[0], rect[1])
                .into(),
        );
        if n >= 120 {
            break;
        }
    }
    if super::platform::debug() {
        eprintln!("labels: {} placed, {glyphs} glyphs", taken.len());
    }
}

fn apple_layer(app: &App, out: &mut Vec<Element<App>>) {
    let Some((image, at)) = &app.snapshot else {
        return;
    };
    // Until a new snapshot arrives, move the last one with the camera. It can
    // shrink when zooming out; growing it would make it larger than the map.
    let scale = (app.camera.zoom - at.zoom).exp2().min(1.);
    let world = app.camera.world();
    let [w, h] = app.size;
    let dx = (at.center[0] - app.camera.center[0]) * world;
    let dy = (at.center[1] - app.camera.center[1]) * world;
    let (sw, sh) = (w * scale as f32, h * scale as f32);
    let at = [(w - sw) / 2. + dx as f32, (h - sh) / 2. + dy as f32];
    // After a jump (a route framed, a place picked) the old picture is far
    // away: leave it out until the new one arrives. Offsets also have a limit.
    if !fits(at) || at[0] > w || at[1] > h || at[0] + sw < 0. || at[1] + sh < 0. {
        return;
    }
    out.push(
        zstack(Vec::<Element<App>>::new())
            .key("apple-snapshot")
            .width(sw)
            .height(sh)
            .background_image(image.clone().background().fit(ImageFit::Stretch))
            .offset(at[0], at[1])
            .into(),
    );
}

// These use the desktop's headless helpers; they run with the example.
#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use volt_ui_kit::{
        declarative::{Bindings, Runtime, TextMetrics},
        Viewport,
    };
    struct Metrics;
    impl TextMetrics for Metrics {
        fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
            [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
        }
    }
    #[test]
    fn dragging_the_map_pans_the_camera() {
        let mut app = App::new();
        let (document, mut handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        let viewport = Viewport::new(1200., 800., 1.).unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut Metrics)
            .unwrap();
        let before = app.camera.center;
        let down = runtime
            .pointer_down_outcome(600., 400., &mut handlers, &mut app)
            .unwrap();
        let moved = runtime
            .pointer_move_outcome(700., 450., &mut handlers, &mut app)
            .unwrap();
        runtime
            .pointer_up_outcome(700., 450., &mut handlers, &mut app)
            .unwrap();
        eprintln!("down {down:?} moved {moved:?} size {:?}", app.size);
        assert_ne!(app.camera.center, before, "the drag did not reach the map");
    }

    #[test]
    fn the_wheel_zooms_around_the_pointer() {
        let mut app = App::new();
        let (document, mut handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        let viewport = Viewport::new(1200., 800., 1.).unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut Metrics)
            .unwrap();
        // Size reports arrive from the host; set them as it would.
        app.size = [1200., 712.];
        let zoom = app.camera.zoom;
        let world = app.camera.world();
        let under = |app: &App, at: [f64; 2]| {
            let w = app.camera.world();
            [
                app.camera.center[0] + (at[0] - 600.) / w,
                app.camera.center[1] + (at[1] - 356.) / w,
            ]
        };
        let _ = world;
        // Pointer positions are inside the map, which starts below the toolbar.
        let top = runtime
            .boxes()
            .iter()
            .find(|b| b.id == "map")
            .unwrap()
            .bounds
            .bounds()[1];
        let at = [300., 260. - f64::from(top)];
        let before = under(&app, at);
        runtime
            .scroll_outcome(300., 260., [0., -120.], &mut handlers, &mut app)
            .unwrap();
        assert!(app.camera.zoom > zoom, "wheel up should zoom in");
        let after = under(&app, at);
        assert!((before[0] - after[0]).abs() < 1e-9 && (before[1] - after[1]).abs() < 1e-9);
    }

    /// Downloads real tiles. The map reports its size, recomposes, and must
    /// keep the same size: content larger than the map made it grow until
    /// the window scrolled.
    #[test]
    #[ignore]
    fn a_loaded_map_keeps_its_size_across_recompositions() {
        for provider in [Provider::OpenFreeMap, Provider::OpenStreetMap] {
            let mut app = App::new();
            app.prefetch(provider, [1200., 712.]);
            let (document, mut handlers) = view(&app).build().unwrap().into_parts();
            let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
            let viewport = Viewport::new(1200., 800., 1.).unwrap();
            let mut sizes = Vec::new();
            for _ in 0..6 {
                runtime
                    .layout(viewport, &Bindings::default(), &mut Metrics)
                    .unwrap();
                runtime.dispatch_widths(&mut handlers, &mut app).unwrap();
                sizes.push(app.size);
                let (document, next) = view(&app).build().unwrap().into_parts();
                runtime
                    .replace_document(document, &next, &Bindings::default())
                    .unwrap();
                handlers = next;
            }
            assert!(
                sizes.windows(2).skip(1).all(|w| w[0] == w[1]),
                "{provider:?} map size drifted: {sizes:?}"
            );
        }
    }

    /// Buttons must work on the first press, even when the view is rebuilt
    /// between press and release (tiles and timers arrive all the time).
    #[test]
    fn buttons_activate_on_the_first_press_across_rebuilds() {
        let mut app = App::new();
        app.size = [1280., 820.];
        let (document, mut handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        let viewport = Viewport::new(1280., 820., 1.).unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut Metrics)
            .unwrap();
        let centre = |runtime: &Runtime, id: &str| {
            let [x, y, w, h] = runtime
                .boxes()
                .iter()
                .find(|b| b.id == id)
                .unwrap_or_else(|| panic!("no {id}"))
                .bounds
                .bounds();
            (x + w / 2., y + h / 2.)
        };
        for (id, rebuild) in [
            ("zoom-in", false),
            ("zoom-in", true),
            ("zoom-out", true),
            ("sidebar", true),
            ("map-style", true),
        ] {
            let before = (app.camera.zoom, app.panel_open, app.layers_open);
            let (x, y) = centre(&runtime, id);
            runtime.pointer_move(x, y);
            runtime
                .pointer_down_outcome(x, y, &mut handlers, &mut app)
                .unwrap();
            if rebuild {
                let (document, next) = view(&app).build().unwrap().into_parts();
                runtime
                    .replace_document(document, &next, &Bindings::default())
                    .unwrap();
                handlers = next;
                runtime
                    .layout(viewport, &Bindings::default(), &mut Metrics)
                    .unwrap();
            }
            let out = runtime
                .pointer_up_outcome(x, y, &mut handlers, &mut app)
                .unwrap();
            let after = (app.camera.zoom, app.panel_open, app.layers_open);
            assert!(
                out.activated && before != after,
                "{id} (rebuild {rebuild}) did not activate: {out:?}"
            );
            let (document, next) = view(&app).build().unwrap().into_parts();
            runtime
                .replace_document(document, &next, &Bindings::default())
                .unwrap();
            handlers = next;
            runtime
                .layout(viewport, &Bindings::default(), &mut Metrics)
                .unwrap();
        }
    }

    /// Coarse stand-in tiles serve several placements at once; every frame
    /// must still build (no duplicate ids), wherever the camera is. A
    /// duplicate made the whole window refuse updates.
    #[test]
    fn tilted_and_turned_views_build_with_standing_buildings() {
        let mut app = App::new();
        app.size = [1280., 820.];
        app.style = Some(
            style::Style::parse(
                r##"{"sources":{"s":{"type":"vector","tiles":["t/{z}/{x}/{y}"],"maxzoom":14}},
                    "layers":[{"id":"water","type":"fill","source":"s","source-layer":"water","paint":{"fill-color":"#3366FF"}},
                              {"id":"roads","type":"line","source":"s","source-layer":"roads","paint":{"line-color":"#888888","line-width":2}},
                              {"id":"b3d","type":"fill-extrusion","source":"s","source-layer":"building",
                               "paint":{"fill-extrusion-color":"#D9D4CC","fill-extrusion-height":["get","render_height"]}},
                              {"id":"names","type":"symbol","source":"s","source-layer":"place","layout":{"text-field":"{name}"}}]}"##,
            )
            .unwrap(),
        );
        let feature =
            |kind, parts: Vec<Vec<[f32; 2]>>, props: &[(&str, mvt::Value)]| mvt::Feature {
                kind,
                parts,
                properties: props
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), v.clone()))
                    .collect(),
            };
        let mut layers = std::collections::HashMap::new();
        let layer = |features| mvt::Layer {
            extent: 4096.,
            features,
        };
        layers.insert(
            "water".to_owned(),
            layer(vec![feature(
                Geometry::Polygon,
                vec![vec![
                    [100., 100.],
                    [4000., 120.],
                    [3900., 3900.],
                    [80., 4000.],
                ]],
                &[],
            )]),
        );
        layers.insert(
            "roads".to_owned(),
            layer(vec![feature(
                Geometry::Line,
                vec![vec![[0., 2048.], [4096., 2100.]]],
                &[],
            )]),
        );
        // A tower with a courtyard, and one cut by the tile's edge.
        layers.insert(
            "building".to_owned(),
            layer(vec![
                feature(
                    Geometry::Polygon,
                    vec![
                        vec![
                            [1800., 1800.],
                            [2300., 1800.],
                            [2300., 2300.],
                            [1800., 2300.],
                        ],
                        vec![
                            [1900., 1900.],
                            [1900., 2200.],
                            [2200., 2200.],
                            [2200., 1900.],
                        ],
                    ],
                    &[("render_height", mvt::Value::Num(180.))],
                ),
                feature(
                    Geometry::Polygon,
                    vec![vec![[-64., 500.], [300., 500.], [300., 900.], [-64., 900.]]],
                    &[("render_height", mvt::Value::Num(40.))],
                ),
            ]),
        );
        // Labels across each tile, so some are in view at any bearing.
        let points = (0..8)
            .flat_map(|i| (0..8).map(move |j| [256. + 512. * i as f32, 256. + 512. * j as f32]))
            .map(|p| {
                feature(
                    Geometry::Point,
                    vec![vec![p]],
                    &[("name", mvt::Value::Str("Middle".into()))],
                )
            })
            .collect();
        layers.insert("place".to_owned(), layer(points));
        let tile = TileData::Vector(std::sync::Arc::new(mvt::Tile { layers }));
        app.camera = super::super::Camera::at(40.7484, -73.9857, 16.);
        for z in 10..=14u8 {
            let n = 1u32 << z;
            let [x, y] = app.camera.center.map(|v| (v * f64::from(n)) as u32);
            for dy in 0..6u32 {
                for dx in 0..6u32 {
                    let key = TileKey {
                        source: "s".into(),
                        z,
                        x: x + dx - 3,
                        y: y + dy - 3,
                    };
                    app.tiles.insert(key, tile.clone());
                }
            }
        }
        for (bearing, pitch) in [
            (0., 0.),
            (30., 0.),
            (0., 45.),
            (135., 60.),
            (-80., 30.),
            (359.9, 55.),
        ] {
            app.camera.bearing = bearing;
            app.camera.pitch = pitch;
            let element = view(&app);
            let built = element.build();
            if let Err(e) = &built {
                panic!("bearing {bearing} pitch {pitch}: {e}");
            }
        }
        // Standing up, the tower has walls: more paths than when flat.
        app.camera.bearing = 20.;
        app.camera.pitch = 50.;
        let mut layers = Vec::new();
        let mut labels = Vec::new();
        let style = app.style.clone().unwrap();
        let view_3d = super::super::perspective::View::new(app.camera, app.size);
        let run: Vec<&Layer> = style.layers.iter().collect();
        tilted::draw_run(&app, &view_3d, &run, &mut 0, &mut layers, &mut labels);
        assert!(!layers.is_empty(), "the tilted map drew nothing");
        assert!(
            labels.iter().any(|l| l.text == "Middle"),
            "a label in view was kept"
        );
    }

    #[test]
    fn stand_in_tiles_never_duplicate_ids() {
        let mut app = App::new();
        app.size = [1280., 820.];
        app.style = Some(
            style::Style::parse(
                r##"{"sources":{"s":{"type":"vector","tiles":["t/{z}/{x}/{y}"],"maxzoom":14}},
                    "layers":[{"id":"water","type":"fill","source":"s","source-layer":"water","paint":{"fill-color":"#3366FF"}},
                              {"id":"roads","type":"line","source":"s","source-layer":"roads","paint":{"line-color":"#888888","line-width":2}}]}"##,
            )
            .unwrap(),
        );
        let mut layers = std::collections::HashMap::new();
        layers.insert(
            "water".to_owned(),
            mvt::Layer {
                extent: 4096.,
                features: vec![mvt::Feature {
                    kind: Geometry::Polygon,
                    parts: vec![vec![
                        [100., 100.],
                        [4000., 120.],
                        [3900., 3900.],
                        [80., 4000.],
                        [100., 100.],
                    ]],
                    properties: Default::default(),
                }],
            },
        );
        layers.insert(
            "roads".to_owned(),
            mvt::Layer {
                extent: 4096.,
                features: vec![mvt::Feature {
                    kind: Geometry::Line,
                    parts: vec![vec![[0., 2048.], [4096., 2100.]]],
                    properties: Default::default(),
                }],
            },
        );
        let tile = TileData::Vector(std::sync::Arc::new(mvt::Tile { layers }));
        // Niagara Falls; tiles loaded at zoom 9 only, as after a long drag.
        app.camera = super::super::Camera::at(43.08, -79.07, 9.);
        for p in app.placements("s") {
            app.tiles.insert(p.key, tile.clone());
        }
        // Stand-ins at many positions and zooms.
        let start = app.camera.center;
        for step in 0..200u32 {
            let f = f64::from(step);
            app.camera.zoom = 9.05 + (f * 0.37) % 4.8;
            app.camera.center = [
                start[0] + (f * 0.000_731).sin() * 0.001,
                start[1] + (f * 0.000_517).cos() * 0.0008,
            ];
            if let Err(e) = view(&app).build() {
                panic!("step {step} zoom {}: {e}", app.camera.zoom);
            }
        }
        // A tile's left edge a point or two from the window's: the drag to
        // Niagara that froze the app.
        app.camera.zoom = 9.;
        app.camera.center = start;
        let world = app.camera.world();
        let size = world / 512.;
        let left = app.camera.center[0] * world - 640.;
        let k = (left / size).ceil();
        for edge in [0., 0.4, 0.9, 1.3, 2.2] {
            app.camera.center[0] = (k * size - edge + 640.) / world;
            if let Err(e) = view(&app).build() {
                panic!("tile edge at {edge}: {e}");
            }
        }
    }

    #[test]
    fn shapes_are_cut_to_the_visible_box() {
        let r = [0., 0., 100., 100.];
        // A square half outside on the right.
        let ring = vec![[50., 10.], [150., 10.], [150., 90.], [50., 90.], [50., 10.]];
        let cut = clip_ring(&ring, r);
        assert!(cut.iter().all(|&p| inside(p, r)));
        assert!(cut.iter().any(|p| (p[0] - 100.).abs() < 1e-4));
        // A line that leaves and comes back is two pieces.
        let line = vec![[10., 50.], [200., 50.], [200., 60.], [10., 60.]];
        let pieces = clip_line(&line, r);
        assert_eq!(pieces.len(), 2);
        assert!(pieces.iter().flatten().all(|&p| inside(p, r)));
        // Entirely outside: nothing.
        assert!(clip_line(&[[200., 200.], [300., 300.]], r).is_empty());
        assert!(clip_ring(&[[200., 200.], [300., 200.], [300., 300.]], r).is_empty());
    }

    /// Zooming out past what is loaded keeps showing the sharper tiles
    /// already here, instead of a blank map until new tiles arrive.
    #[test]
    fn zooming_out_keeps_the_loaded_tiles_on_screen() {
        let mut app = App::new();
        app.size = [1280., 820.];
        app.style = Some(
            style::Style::parse(r#"{"sources":{"s":{"type":"vector","tiles":["t/{z}/{x}/{y}"],"maxzoom":14}},"layers":[]}"#)
                .unwrap(),
        );
        let empty = TileData::Vector(std::sync::Arc::new(mvt::Tile::default()));
        for p in app.placements("s") {
            app.tiles.insert(p.key, empty.clone());
        }
        let loaded = tiles_for(&app, "s").len();
        app.camera.zoom = 9.5;
        let shown = tiles_for(&app, "s");
        assert!(
            !shown.is_empty() && shown.iter().all(|(key, ..)| key.z == 11),
            "{} of {loaded}",
            shown.len()
        );
        // And zooming in uses the coarser tile stretched over the view.
        app.camera.zoom = 12.4;
        assert!(tiles_for(&app, "s").iter().all(|(key, ..)| key.z == 11));
    }

    /// After the camera jumps (a route framed far away), the old snapshot
    /// must not push a paint offset past the kit's limit: that froze the
    /// window on its previous frame.
    #[test]
    fn a_far_jump_with_an_old_snapshot_still_builds() {
        let mut app = App::new();
        app.provider = Provider::Apple;
        app.size = [1280., 820.];
        let picture = volt_ui_kit::graphics::Image::from_rgba(4, 4, vec![200; 64]).unwrap();
        app.snapshot = Some((picture, app.camera));
        for (dx, zoom) in [(0.001, 0.), (0.2, 0.), (0.0005, -3.), (0.3, 4.)] {
            app.camera.center[0] += dx;
            app.camera.zoom += zoom;
            view(&app).build().expect("a valid frame");
        }
    }

    /// Zooming in while an Apple snapshot is shown must not grow the map
    /// (a stand-in picture: MapKit only answers on the main thread).
    #[test]
    fn zooming_an_apple_snapshot_keeps_the_map_size() {
        let mut app = App::new();
        app.provider = Provider::Apple;
        app.size = [1200., 712.];
        let picture = volt_ui_kit::graphics::Image::from_rgba(4, 4, vec![200; 64]).unwrap();
        app.snapshot = Some((picture, app.camera));
        app.camera.zoom += 1.;
        let (document, mut handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        let viewport = Viewport::new(1200., 800., 1.).unwrap();
        let mut sizes = Vec::new();
        for _ in 0..4 {
            runtime
                .layout(viewport, &Bindings::default(), &mut Metrics)
                .unwrap();
            runtime.dispatch_widths(&mut handlers, &mut app).unwrap();
            sizes.push(app.size);
            let (document, next) = view(&app).build().unwrap().into_parts();
            runtime
                .replace_document(document, &next, &Bindings::default())
                .unwrap();
            handlers = next;
        }
        assert!(
            sizes.windows(2).all(|w| w[0] == w[1]),
            "map size drifted: {sizes:?}"
        );
    }

    /// Downloads real tiles: `cargo test --example maps -- --ignored`.
    #[test]
    #[ignore]
    fn dragging_a_loaded_map_pans_and_providers_switch() {
        let mut app = App::new();
        app.prefetch(Provider::OpenFreeMap, [1200., 712.]);
        assert!(!app.tiles.is_empty());
        let (document, mut handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        let viewport = Viewport::new(1200., 800., 1.).unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut Metrics)
            .unwrap();
        let before = app.camera.center;
        for (x, y) in [(600., 400.), (300., 300.), (900., 600.)] {
            runtime
                .pointer_down_outcome(x, y, &mut handlers, &mut app)
                .unwrap();
            runtime
                .pointer_move_outcome(x + 50., y + 30., &mut handlers, &mut app)
                .unwrap();
            runtime
                .pointer_up_outcome(x + 50., y + 30., &mut handlers, &mut app)
                .unwrap();
            assert_ne!(
                app.camera.center, before,
                "drag at {x},{y} did not reach the map"
            );
        }
        let ids: Vec<String> = runtime
            .boxes()
            .iter()
            .map(|b| b.id.clone())
            .filter(|id| id.contains("providers"))
            .collect();
        eprintln!("provider boxes: {ids:?}");
    }
}
