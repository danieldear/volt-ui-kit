//! Drawing a turned or tilted map. There are no upright tile boxes to cut
//! shapes to: every shape is cut to its tile and to the ground in view in
//! the turned frame, then projected to the screen. Lines keep their width
//! and labels stay upright. Buildings with heights stand up from the ground
//! and are drawn last, the farthest first, so nearer ones cover them.
use super::super::{
    mvt::{self, Geometry},
    net::TileData,
    perspective::{self, clip_polygon, clip_polyline, View},
    style::{self, hex, Ctx, Layer, LayerKind},
    App,
};
use super::{add, drain, label_text, paint_color, paint_num, tiles_for, Batches, Label, Packer};
use std::collections::BTreeSet;
use volt_ui_kit::{
    graphics::{PathCommand, VectorPath},
    ui::prelude::*,
};

/// At most this many buildings, the nearest, are stood up.
const MOST_BUILDINGS: usize = 3_000;

/// One building: its footprint in the turned frame, how high its base and
/// roof are (points), its colour, and which footprint edges get no wall
/// (edges where the tile or the view cut it).
struct Building {
    rings: Vec<Vec<[f64; 2]>>,
    open: Vec<Vec<bool>>,
    base: f64,
    top: f64,
    color: [f32; 4],
    far: f64,
}

/// Where a tile's own coordinates land in the turned frame.
struct Frame<'a> {
    view: &'a View,
    origin: [f64; 2],
    k: f64,
    /// The part of the tile in view, as a box in the tile's own units:
    /// features outside it are skipped before any cutting.
    seen: [f32; 4],
}

/// A feature's extent in its tile's units.
fn bounds(feature: &mvt::Feature) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for p in feature.parts.iter().flatten() {
        b = [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ];
    }
    b
}

fn overlaps(a: [f32; 4], b: [f32; 4], margin: f32) -> bool {
    a[0] <= b[2] + margin && b[0] <= a[2] + margin && a[1] <= b[3] + margin && b[1] <= a[3] + margin
}

impl Frame<'_> {
    fn at(&self, p: [f32; 2]) -> [f64; 2] {
        self.view.turned_from_flat([
            self.origin[0] + f64::from(p[0]) * self.k,
            self.origin[1] + f64::from(p[1]) * self.k,
        ])
    }
}

fn screen(view: &View, points: &[[f64; 2]]) -> Option<Vec<[f32; 2]>> {
    points
        .iter()
        .map(|&r| view.project(r, 0.).map(|s| [s[0] as f32, s[1] as f32]))
        .collect()
}

pub(super) fn draw_run(
    app: &App,
    view: &View,
    run: &[&Layer],
    runs: &mut usize,
    out: &mut Vec<Element<App>>,
    labels: &mut Vec<Label>,
) {
    if run.is_empty() {
        return;
    }
    *runs += 1;
    let visible = view.visible(32.);
    if visible.len() < 3 {
        return;
    }
    // Styles without buildings in 3D (the dark one) stand their flat
    // building footprints up instead, from the same heights.
    let raise_fills = !app
        .style
        .as_ref()
        .is_some_and(|s| s.layers.iter().any(|l| l.kind == LayerKind::Extrusion));
    let (lat, _) = app.camera.lat_lon();
    let metres_per_point = 40_075_016.686 * lat.to_radians().cos() / app.camera.world();
    // Footprints are cut a little below the map's bottom edge: nearer than
    // that, tall walls would lean in from behind the camera.
    let near = view
        .ground([view.size[0] / 2., view.size[1] + 64.])
        .map_or(f64::MAX, |g| g[1]);
    let mut packer = Packer {
        canvases: Vec::new(),
    };
    let mut buildings: Vec<Building> = Vec::new();
    let sources: BTreeSet<&str> = run.iter().filter_map(|l| l.source.as_deref()).collect();
    for source in sources {
        for (_, origin, size, data) in tiles_for(app, source) {
            let TileData::Vector(tile) = data else {
                continue;
            };
            let square = [
                origin,
                [origin[0] + size, origin[1]],
                [origin[0] + size, origin[1] + size],
                [origin[0], origin[1] + size],
            ]
            .map(|f| view.turned_from_flat(f));
            let clip = clip_polygon(&square, &visible);
            if clip.len() < 3 {
                continue;
            }
            for layer in run.iter().filter(|l| l.source.as_deref() == Some(source)) {
                let Some(data) = layer
                    .source_layer
                    .as_deref()
                    .and_then(|n| tile.layers.get(n))
                else {
                    continue;
                };
                let k = size / f64::from(data.extent);
                let mut frame = Frame {
                    view,
                    origin,
                    k,
                    seen: [0.; 4],
                };
                let mut seen = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                for &r in &clip {
                    let l = local_of(&frame, r);
                    seen = [
                        seen[0].min(l[0] as f32),
                        seen[1].min(l[1] as f32),
                        seen[2].max(l[0] as f32),
                        seen[3].max(l[1] as f32),
                    ];
                }
                frame.seen = seen;
                let raised = layer.kind == LayerKind::Extrusion
                    || (raise_fills
                        && layer.kind == LayerKind::Fill
                        && layer.source_layer.as_deref() == Some("building"));
                if raised {
                    stand_up(
                        layer,
                        data,
                        app.camera.zoom,
                        &frame,
                        metres_per_point,
                        near,
                        &mut buildings,
                    );
                    if layer.kind == LayerKind::Extrusion {
                        continue;
                    }
                }
                let mut batches = Batches::new();
                shapes(
                    layer,
                    data,
                    app.camera.zoom,
                    &frame,
                    &clip,
                    &visible,
                    &mut batches,
                    &mut packer,
                    labels,
                );
                drain(&mut batches, &mut packer);
            }
        }
    }
    draw_buildings(view, buildings, &mut packer);
    if super::super::platform::debug() {
        let paths: usize = packer.canvases.iter().map(|c| c.paths.len()).sum();
        let commands: usize = packer
            .canvases
            .iter()
            .flat_map(|c| &c.paths)
            .map(|p| p.path.commands.len())
            .sum();
        eprintln!(
            "tilted run {runs}: {} canvases, {paths} paths, {commands} commands",
            packer.canvases.len()
        );
    }
    let [w, h] = app.size;
    for (i, paths) in packer.canvases.into_iter().enumerate() {
        out.push(
            volt_ui_kit::ui::canvas(move |c| *c = paths)
                .key(format!("run{runs}-tilted-{i}"))
                .width(w.max(1.))
                .height(h.max(1.)),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn shapes(
    layer: &Layer,
    data: &mvt::Layer,
    zoom: f64,
    frame: &Frame,
    clip: &[[f64; 2]],
    visible: &[[f64; 2]],
    batches: &mut Batches,
    packer: &mut Packer,
    labels: &mut Vec<Label>,
) {
    let view = frame.view;
    // Room for wide lines and labels just outside the view.
    let margin = (48. / frame.k) as f32;
    for feature in &data.features {
        if !overlaps(bounds(feature), frame.seen, margin) {
            continue;
        }
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
            (LayerKind::Fill, Geometry::Polygon) => {
                if layer.paint.contains_key("fill-pattern")
                    && !layer.paint.contains_key("fill-color")
                {
                    continue;
                }
                let color = paint_color(layer, "fill-color", &ctx, "#000000");
                let opacity = paint_num(layer, "fill-opacity", &ctx, 1.) as f32;
                let key = (hex(color, opacity), None);
                for ring in &feature.parts {
                    let ring: Vec<[f64; 2]> = ring.iter().map(|&p| frame.at(p)).collect();
                    let ring = clip_polygon(&ring, clip);
                    if ring.len() < 3 {
                        continue;
                    }
                    if let Some(points) = screen(view, &ring) {
                        add(batches, packer, key.clone(), &points, true);
                    }
                }
            }
            (LayerKind::Line, Geometry::Line | Geometry::Polygon) => {
                let color = paint_color(layer, "line-color", &ctx, "#000000");
                let opacity = paint_num(layer, "line-opacity", &ctx, 1.) as f32;
                let width = paint_num(layer, "line-width", &ctx, 1.);
                let gap = paint_num(layer, "line-gap-width", &ctx, 0.);
                let width = if gap > 0. { gap + 2. * width } else { width } as f32;
                if width <= 0. || opacity <= 0. {
                    continue;
                }
                let key = (hex(color, opacity), Some((width * 100.) as u32));
                for part in &feature.parts {
                    let part: Vec<[f64; 2]> = part.iter().map(|&p| frame.at(p)).collect();
                    for piece in clip_polyline(&part, clip) {
                        if let Some(points) = screen(view, &piece) {
                            add(batches, packer, key.clone(), &points, false);
                        }
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
                let Some(&point) = feature.parts.first().and_then(|p| p.first()) else {
                    continue;
                };
                let r = frame.at(point);
                if !perspective::contains(visible, r) {
                    continue;
                }
                // Far labels shrink with the ground; the farthest are left out.
                let s = view.scale(r);
                if s < 0.45 {
                    continue;
                }
                let Some(at) = view.project(r, 0.) else {
                    continue;
                };
                let Some(text) = label_text(layer, &ctx) else {
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
                let bold = layer
                    .layout
                    .get("text-font")
                    .map(|j| style::eval(j, &ctx).text().to_lowercase().contains("bold"))
                    .unwrap_or(false);
                labels.push(Label {
                    text,
                    at: [at[0] as f32, at[1] as f32],
                    size: size * (s as f32).clamp(0.8, 1.1),
                    color: hex(color, paint_num(layer, "text-opacity", &ctx, 1.) as f32),
                    halo,
                    weight: if bold { 600 } else { 500 },
                });
            }
            _ => {}
        }
    }
}

/// Buildings of one tile layer, with their heights from the style (or,
/// for raised footprints, from the tile's own `render_height`).
fn stand_up(
    layer: &Layer,
    data: &mvt::Layer,
    zoom: f64,
    frame: &Frame,
    metres_per_point: f64,
    near: f64,
    out: &mut Vec<Building>,
) {
    let extent = f64::from(data.extent);
    let eye = frame.view.eye();
    // Everything nearer than `near` is cut off: one convex outline.
    let keep = [[-1e7, -1e7], [1e7, -1e7], [1e7, near], [-1e7, near]];
    // Tall buildings just outside the view can lean into it.
    let margin = (240. / frame.k) as f32;
    for feature in &data.features {
        if feature.kind != Geometry::Polygon || !overlaps(bounds(feature), frame.seen, margin) {
            continue;
        }
        let ctx = Ctx {
            zoom,
            feature: Some(feature),
        };
        if let Some(f) = &layer.filter {
            if !style::filter(f, &ctx) {
                continue;
            }
        }
        let own = |key: &str| style::eval(&serde_json::json!(["get", key]), &ctx).num();
        let (height, base, color) = if layer.kind == LayerKind::Extrusion {
            (
                paint_num(
                    layer,
                    "fill-extrusion-height",
                    &ctx,
                    own("render_height").unwrap_or(0.),
                ),
                paint_num(
                    layer,
                    "fill-extrusion-base",
                    &ctx,
                    own("render_min_height").unwrap_or(0.),
                ),
                paint_color(layer, "fill-extrusion-color", &ctx, "#D9D4CC"),
            )
        } else {
            (
                own("render_height").unwrap_or(0.),
                own("render_min_height").unwrap_or(0.),
                paint_color(layer, "fill-color", &ctx, "#D9D4CC"),
            )
        };
        if height <= base || height <= 0. {
            continue;
        }
        // The first ring's winding marks outer rings; the other winding
        // marks holes (courtyards) in the building before it.
        let Some(first) = feature.parts.first() else {
            continue;
        };
        let outer = perspective::area(
            &first
                .iter()
                .map(|p| [f64::from(p[0]), f64::from(p[1])])
                .collect::<Vec<_>>(),
        )
        .signum();
        let mut current: Option<Building> = None;
        for ring in &feature.parts {
            if ring.len() < 3 {
                continue;
            }
            let local: Vec<[f64; 2]> = ring
                .iter()
                .map(|p| [f64::from(p[0]), f64::from(p[1])])
                .collect();
            let hole = perspective::area(&local).signum() != outer;
            if !hole {
                if let Some(done) = current.take() {
                    out.push(done);
                }
            }
            let turned: Vec<[f64; 2]> = ring.iter().map(|&p| frame.at(p)).collect();
            let cut = clip_polygon(&turned, &keep);
            if cut.len() < 3 {
                continue;
            }
            // Edges on a tile's border (or its buffer's) belong to a building
            // that goes on in the next tile; edges on the near cut are where
            // the view ends. Neither gets a wall.
            let border = |a: [f64; 2], b: [f64; 2]| {
                let on = |v: f64| v <= 0.5 || v >= extent - 0.5;
                let la = local_of(frame, a);
                let lb = local_of(frame, b);
                ((la[0] - lb[0]).abs() < 0.5 && on(la[0]) && on(lb[0]))
                    || ((la[1] - lb[1]).abs() < 0.5 && on(la[1]) && on(lb[1]))
                    || ((a[1] - near).abs() < 1e-6 && (b[1] - near).abs() < 1e-6)
            };
            let open: Vec<bool> = (0..cut.len())
                .map(|i| border(cut[i], cut[(i + 1) % cut.len()]))
                .collect();
            match &mut current {
                Some(b) if hole => {
                    b.rings.push(cut);
                    b.open.push(open);
                }
                _ => {
                    let centre = cut.iter().fold([0., 0.], |s, p| [s[0] + p[0], s[1] + p[1]]);
                    let centre = [centre[0] / cut.len() as f64, centre[1] / cut.len() as f64];
                    current = Some(Building {
                        rings: vec![cut],
                        open: vec![open],
                        base: base.max(0.) / metres_per_point,
                        top: height / metres_per_point,
                        color,
                        far: (centre[0] - eye[0]).hypot(centre[1] - eye[1]),
                    });
                }
            }
        }
        if let Some(done) = current {
            out.push(done);
        }
    }
}

/// Back from the turned frame to a tile's own coordinates (to tell which
/// footprint edges lie on the tile's border).
fn local_of(frame: &Frame, r: [f64; 2]) -> [f64; 2] {
    let d = frame.view.unturn(r);
    let f = [
        d[0] + frame.view.size[0] / 2.,
        d[1] + frame.view.size[1] / 2.,
    ];
    [
        (f[0] - frame.origin[0]) / frame.k,
        (f[1] - frame.origin[1]) / frame.k,
    ]
}

/// Light comes from the left and a little towards the viewer: walls facing
/// it are lighter. Four shades, so a building needs at most five paths.
const SHADES: [f32; 4] = [0.6, 0.7, 0.8, 0.9];

fn shade_of(normal: [f64; 2]) -> usize {
    let light = [-0.8, 0.6];
    let lit = (normal[0] * light[0] + normal[1] * light[1]).clamp(-1., 1.);
    (((lit + 1.) / 2. * 4.) as usize).min(3)
}

fn tint(color: [f32; 4], dark: bool, shade: f32) -> String {
    let [r, g, b, _] = color;
    if dark {
        // Dark maps: dark roofs, walls a little lighter so they read.
        let lift = (1. - shade) * 0.35;
        hex(
            [
                r + (1. - r) * lift,
                g + (1. - g) * lift,
                b + (1. - b) * lift,
                1.,
            ],
            1.,
        )
    } else {
        hex([r * shade, g * shade, b * shade, 1.], 1.)
    }
}

fn draw_buildings(view: &View, mut buildings: Vec<Building>, packer: &mut Packer) {
    if buildings.is_empty() {
        return;
    }
    // The nearest few thousand, then drawn from the farthest.
    buildings.sort_by(|a, b| a.far.total_cmp(&b.far));
    buildings.truncate(MOST_BUILDINGS);
    let eye = view.eye();
    for b in buildings.iter().rev() {
        let [r, g, bl, _] = b.color;
        let dark = 0.2126 * r + 0.7152 * g + 0.0722 * bl < 0.2;
        let roof_color = if dark {
            let lift = |v: f32| v.max(0.12);
            [lift(r), lift(g), lift(bl), 1.]
        } else {
            b.color
        };
        let mut walls: [Vec<[[f32; 2]; 4]>; 4] = Default::default();
        let mut roof: Vec<Vec<[f32; 2]>> = Vec::new();
        let mut hidden = false;
        for (ring, open) in b.rings.iter().zip(&b.open) {
            let sign = perspective::area(ring).signum();
            let hole = b
                .rings
                .first()
                .is_some_and(|first| !std::ptr::eq(first, ring));
            for i in 0..ring.len() {
                if open[i] {
                    continue;
                }
                let (a, c) = (ring[i], ring[(i + 1) % ring.len()]);
                let e = [c[0] - a[0], c[1] - a[1]];
                let len = e[0].hypot(e[1]);
                if len < 1e-9 {
                    continue;
                }
                // Away from the ring's inside; for a courtyard, into it.
                let flip = if hole { -sign } else { sign };
                let normal = [flip * e[1] / len, -flip * e[0] / len];
                if (eye[0] - a[0]) * normal[0] + (eye[1] - a[1]) * normal[1] <= 0. {
                    continue;
                }
                let corners = [
                    view.project(a, b.base),
                    view.project(c, b.base),
                    view.project(c, b.top),
                    view.project(a, b.top),
                ];
                let [Some(p0), Some(p1), Some(p2), Some(p3)] = corners else {
                    hidden = true;
                    break;
                };
                // A wall seen edge-on or from far away is a sliver: the roof
                // and its neighbours cover for it.
                let quad = [p0, p1, p2, p3];
                let twice_area = (0..4)
                    .map(|j| {
                        let (u, v) = (quad[j], quad[(j + 1) % 4]);
                        u[0] * v[1] - v[0] * u[1]
                    })
                    .sum::<f64>()
                    .abs();
                if twice_area < 6. {
                    continue;
                }
                walls[shade_of(normal)].push(quad.map(|p| [p[0] as f32, p[1] as f32]));
            }
            match ring
                .iter()
                .map(|&p| view.project(p, b.top).map(|s| [s[0] as f32, s[1] as f32]))
                .collect::<Option<Vec<_>>>()
            {
                Some(points) => roof.push(points),
                None => hidden = true,
            }
        }
        if hidden || roof.is_empty() {
            continue;
        }
        // Too small to see.
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in roof.iter().flatten() {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        if x1 - x0 < 3. && y1 - y0 < 3. {
            continue;
        }
        for (k, quads) in walls.iter().enumerate() {
            let color = tint(roof_color, dark, SHADES[k]);
            let mut path = VectorPath::new();
            for quad in quads {
                if path.commands.len() + 5 > volt_ui_kit::graphics::MAX_PATH_COMMANDS {
                    packer.push(
                        std::mem::replace(&mut path, VectorPath::new()),
                        &color,
                        None,
                    );
                }
                path.commands.push(PathCommand::Move(quad[0]));
                for &p in &quad[1..] {
                    path.commands.push(PathCommand::Line(p));
                }
                path.commands.push(PathCommand::Close);
            }
            packer.push(path, &color, None);
        }
        let color = hex(roof_color, 1.);
        let mut path = VectorPath::new();
        for ring in roof {
            let ring = super::simplify(&ring, true);
            if ring.len() < 3 {
                continue;
            }
            let stride = ring
                .len()
                .div_ceil(volt_ui_kit::graphics::MAX_PATH_COMMANDS - 2)
                .max(1);
            let ring: Vec<[f32; 2]> = ring.into_iter().step_by(stride).collect();
            if path.commands.len() + ring.len() + 1 > volt_ui_kit::graphics::MAX_PATH_COMMANDS {
                packer.push(
                    std::mem::replace(&mut path, VectorPath::new()),
                    &color,
                    None,
                );
            }
            path.commands.push(PathCommand::Move(ring[0]));
            for &p in &ring[1..] {
                path.commands.push(PathCommand::Line(p));
            }
            path.commands.push(PathCommand::Close);
        }
        packer.push(path, &color, None);
    }
}

/// A route on a turned or tilted map: cut to the ground in view, then
/// projected.
pub(super) fn route_points(view: &View, line: &[(f64, f64)]) -> Vec<Vec<[f32; 2]>> {
    let turned: Vec<[f64; 2]> = line
        .iter()
        .map(|&(lat, lon)| view.turned_from_mercator(super::super::project(lat, lon)))
        .collect();
    clip_polyline(&turned, &view.visible(64.))
        .into_iter()
        .filter_map(|piece| screen(view, &piece))
        .collect()
}
