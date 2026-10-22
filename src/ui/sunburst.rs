//! Zoomable sunburst: a hierarchy as rings around its root, after d3's
//! zoomable sunburst. The centre names the root; each ring is one level.
//! The drawing is one retained canvas; every arc is also a sector-clipped
//! button, so hover, tooltips, focus and assistive technology work per arc.
use super::*;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    f32::consts::TAU,
    rc::Rc,
};

type Pick<C> = Rc<RefCell<dyn FnMut(&mut C, String)>>;
type Menu<C> = Rc<RefCell<dyn FnMut(&mut C, String, [f32; 2])>>;
type Back<C> = Rc<RefCell<dyn FnMut(&mut C)>>;

/// One visible arc: `node` indexes the input, `depth` is its ring (1 = inner).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunburstArc {
    pub node: usize,
    pub depth: usize,
    /// Clockwise from 12 o'clock, in radians.
    pub start: f32,
    pub end: f32,
}

/// Partition `nodes` (root first, parents before children) into angular arcs
/// for rings `1..=rings`. A child's sweep is its share of its parent's value.
pub fn sunburst_layout(nodes: &[HierarchyNode], rings: usize) -> Result<Vec<SunburstArc>, String> {
    let Some(root) = nodes.first() else {
        return Err("sunburst needs a root node".into());
    };
    if root.parent.is_some() {
        return Err("the first sunburst node must be the root".into());
    }
    let n = nodes.len();
    let mut depth = vec![0_usize; n];
    let mut children: Vec<Vec<usize>> = vec![vec![]; n];
    for (i, node) in nodes.iter().enumerate().skip(1) {
        match node.parent {
            Some(p) if p < i => {
                depth[i] = depth[p] + 1;
                children[p].push(i);
            }
            _ => return Err(format!("node {i} needs an earlier parent")),
        }
    }
    let (mut start, mut end) = (vec![0_f32; n], vec![0_f32; n]);
    end[0] = TAU;
    let mut out = vec![];
    for i in 0..n {
        let span = end[i] - start[i];
        let total = nodes[i].value.max(1) as f64;
        let mut cursor = start[i];
        for &c in &children[i] {
            let sweep = (nodes[c].value as f64 / total) as f32 * span;
            start[c] = cursor;
            end[c] = (cursor + sweep).min(end[i]);
            cursor = end[c];
            if depth[c] <= rings && end[c] - start[c] > 1e-4 {
                out.push(SunburstArc {
                    node: c,
                    depth: depth[c],
                    start: start[c],
                    end: end[c],
                });
            }
        }
    }
    Ok(out)
}

/// d3's `interpolateRainbow` (a cyclical cubehelix rainbow) at `t` in 0..1.
pub fn rainbow(t: f32) -> String {
    let t = t - t.floor();
    let ts = (t - 0.5).abs();
    let (h, s, l) = (360. * t - 100., 1.5 - 1.5 * ts, 0.8 - 0.9 * ts);
    let h = (h + 120.).to_radians();
    let a = s * l * (1. - l);
    let (cosh, sinh) = (h.cos(), h.sin());
    let channel = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}",
        channel(l + a * (-0.14861 * cosh + 1.78277 * sinh)),
        channel(l + a * (-0.29227 * cosh - 0.90649 * sinh)),
        channel(l + a * (1.97294 * cosh)),
    )
}

/// An annular sector: clockwise from 12 o'clock, `a0..a1` radians.
fn sector(cx: f32, cy: f32, outer: f32, inner: f32, a0: f32, a1: f32) -> VectorPath {
    let p = |rad: f32, a: f32| [cx + rad * a.sin(), cy - rad * a.cos()];
    let large = u8::from(a1 - a0 > std::f32::consts::PI);
    let (o0, o1, i1, i0) = (p(outer, a0), p(outer, a1), p(inner, a1), p(inner, a0));
    let d = format!(
        "M{} {} A{outer} {outer} 0 {large} 1 {} {} L{} {} A{inner} {inner} 0 {large} 0 {} {} Z",
        o0[0], o0[1], o1[0], o1[1], i1[0], i1[1], i0[0], i0[1]
    );
    VectorPath::from_svg(&d).unwrap_or_default()
}

fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The box around a sector, `[x, y, w, h]` in whole pixels: its four corners
/// plus any 12/3/6/9 o'clock extremes the outer arc passes.
fn sector_bounds(cx: f32, cy: f32, outer: f32, inner: f32, a0: f32, a1: f32) -> [f32; 4] {
    use std::f32::consts::FRAC_PI_2;
    let p = |rad: f32, a: f32| [cx + rad * a.sin(), cy - rad * a.cos()];
    let mut points = vec![p(outer, a0), p(outer, a1), p(inner, a0), p(inner, a1)];
    let mut k = (a0 / FRAC_PI_2).ceil() * FRAC_PI_2;
    while k <= a1 {
        points.push(p(outer, k));
        k += FRAC_PI_2;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for [x, y] in points {
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    let (x0, y0) = (x0.floor().max(0.), y0.floor().max(0.));
    [x0, y0, (x1.ceil() - x0).max(1.), (y1.ceil() - y0).max(1.)]
}
/// A dashed outline of a sector, as short arc and line segments (the canvas
/// has no dash pattern): marks what an open menu acts on.
fn dashed_sector(cx: f32, cy: f32, outer: f32, inner: f32, a0: f32, a1: f32) -> VectorPath {
    let p = |rad: f32, a: f32| [cx + rad * a.sin(), cy - rad * a.cos()];
    let (dash, gap) = (5., 4.);
    let mut d = String::new();
    for rad in [outer, inner] {
        if rad <= 0. {
            continue;
        }
        let mut a = a0;
        while a < a1 {
            let b = (a + dash / rad).min(a1);
            let (s, e) = (p(rad, a), p(rad, b));
            d += &format!("M{} {} A{rad} {rad} 0 0 1 {} {} ", s[0], s[1], e[0], e[1]);
            a = b + gap / rad;
        }
    }
    for a in [a0, a1] {
        let mut r = inner;
        while r < outer {
            let q = (r + dash).min(outer);
            let (s, e) = (p(r, a), p(q, a));
            d += &format!("M{} {} L{} {} ", s[0], s[1], e[0], e[1]);
            r = q + gap;
        }
    }
    VectorPath::from_svg(d.trim()).unwrap_or_default()
}

/// Builder for [`sunburst_chart`].
pub struct SunburstChart<C> {
    nodes: Vec<HierarchyNode>,
    size: f32,
    rings: usize,
    key: String,
    selected: Vec<String>,
    reviewed: Vec<String>,
    context_target: Option<String>,
    colors: Vec<(String, String)>,
    labels: bool,
    keyboard: bool,
    hover_linked: bool,
    format: Rc<dyn Fn(u64) -> String>,
    pick: Option<Pick<C>>,
    context: Option<Menu<C>>,
    back: Option<(String, Back<C>)>,
    previous: Option<(Vec<HierarchyNode>, u64)>,
}

/// A zoomable sunburst over the same nodes as [`icicle_chart`]: root first,
/// parents before children, each `value` inclusive of its children.
///
/// The centre circle names the root and, with [`SunburstChart::on_center`],
/// goes back up. Arcs call [`SunburstChart::on_select`] with their key. Each
/// top-level branch has its own rainbow hue; leaf arcs are lighter than
/// branches; [`SunburstChart::selected_keys`] arcs are drawn solid and outlined.
/// When the root changes (a zoom), the rings fade to the new focus.
///
/// Arcs use the icicle's ids (`{key}-tile-{node}`) and spoken labels
/// ("Name — size"), and support the same menus, hover links and keyboard
/// navigation, so an app can offer either view of the same data.
///
/// ```
/// use volt_ui_kit::ui::prelude::*;
/// let rings: Element<()> = sunburst_chart([
///     HierarchyNode::new("root", "Home", 100, None),
///     HierarchyNode::new("a", "Photos", 70, Some(0)),
///     HierarchyNode::new("b", "Music", 30, Some(0)),
/// ])
/// .key("usage")
/// .size(240.)
/// .on_select(|_: &mut (), key| println!("{key}"))
/// .into();
/// rings.build().unwrap();
/// ```
pub fn sunburst_chart<C>(nodes: impl IntoIterator<Item = HierarchyNode>) -> SunburstChart<C> {
    SunburstChart {
        nodes: nodes.into_iter().take(4097).collect(),
        size: 240.,
        rings: 2,
        key: "sunburst".into(),
        selected: vec![],
        reviewed: vec![],
        context_target: None,
        colors: vec![],
        labels: true,
        keyboard: false,
        hover_linked: false,
        format: Rc::new(|v| v.to_string()),
        pick: None,
        context: None,
        back: None,
        previous: None,
    }
}
impl<C> SunburstChart<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }
    /// Diameter in logical pixels.
    pub fn size(mut self, diameter: f32) -> Self {
        self.size = diameter.max(48.);
        self
    }
    /// Levels shown around the centre (1–4; d3's default view shows 2).
    pub fn rings(mut self, rings: usize) -> Self {
        self.rings = rings.clamp(1, 4);
        self
    }
    pub fn selected_keys(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        self.selected = keys.into_iter().collect();
        self
    }
    /// Arcs the app has set aside: outlined in the accent colour and spoken
    /// as "In review".
    pub fn reviewed_keys(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        self.reviewed = keys.into_iter().collect();
        self
    }
    /// The arc an open menu acts on: a dashed outline, never a selection.
    pub fn context_target(mut self, key: impl Into<String>) -> Self {
        self.context_target = Some(key.into());
        self
    }
    /// Override the colour of specific nodes (and their descendants), as
    /// `#RRGGBB` or a theme token like `@muted`.
    pub fn node_colors(mut self, colors: impl IntoIterator<Item = (String, String)>) -> Self {
        self.colors = colors.into_iter().collect();
        self
    }
    /// Arc labels where they fit (on by default).
    pub fn labels(mut self, show: bool) -> Self {
        self.labels = show;
        self
    }
    /// One roving Tab stop: Left/Right visit siblings around the ring, Up the
    /// parent arc, Down its first child; Enter keeps `on_select` semantics.
    pub fn keyboard_navigation(mut self, enabled: bool) -> Self {
        self.keyboard = enabled;
        self
    }
    /// Paint-only hover links keyed by node key, as on [`HierarchyChart`].
    pub fn hover_linked(mut self, enabled: bool) -> Self {
        self.hover_linked = enabled;
        self
    }
    pub fn value_format(mut self, format: impl Fn(u64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }
    pub fn on_select(mut self, pick: impl FnMut(&mut C, String) + 'static) -> Self {
        self.pick = Some(Rc::new(RefCell::new(pick)));
        self
    }
    /// Request an app-owned menu for an arc without selecting it, at
    /// window-logical coordinates (as [`HierarchyChart::on_context_open`]).
    pub fn on_context_open(mut self, f: impl FnMut(&mut C, String, [f32; 2]) + 'static) -> Self {
        self.context = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Animate from the nodes shown before (same root): arcs tween to their
    /// new angles over ~360 ms at paint time, once per `revision`. Bump the
    /// revision when the data changes, not on every recomposition.
    pub fn animate_from(mut self, previous: Vec<HierarchyNode>, revision: u64) -> Self {
        self.previous = Some((previous, revision));
        self
    }
    /// Make the centre a button, e.g. "Go up to Downloads", that zooms out.
    pub fn on_center(
        mut self,
        label: impl Into<String>,
        back: impl FnMut(&mut C) + 'static,
    ) -> Self {
        self.back = Some((label.into(), Rc::new(RefCell::new(back))));
        self
    }
}
impl<C: 'static> From<SunburstChart<C>> for Element<C> {
    fn from(chart: SunburstChart<C>) -> Self {
        use crate::declarative::{document::RovingNavigation, AccessRole};
        let arcs = match sunburst_layout(&chart.nodes, chart.rings) {
            Ok(arcs) => arcs,
            Err(e) => {
                let mut invalid: Element<C> = vstack![].key(chart.key).into();
                invalid.validation_error = Some(e);
                return invalid;
            }
        };
        let size = chart.size;
        let r = size / 2.;
        let ring = r / (chart.rings + 1) as f32;
        let nodes = &chart.nodes;
        let n = nodes.len();
        // Colour by top-level branch, d3-style; overrides inherit downwards.
        let branches: Vec<usize> = (1..n).filter(|&i| nodes[i].parent == Some(0)).collect();
        let overrides: HashMap<&str, &str> = chart
            .colors
            .iter()
            .map(|(k, c)| (k.as_str(), c.as_str()))
            .collect();
        let mut color = vec![String::new(); n];
        let mut has_children = vec![false; n];
        for i in 1..n {
            let p = nodes[i].parent.unwrap_or(0);
            has_children[p] = true;
            color[i] = match overrides.get(nodes[i].key.as_str()) {
                Some(c) => (*c).to_string(),
                None if p == 0 => {
                    let rank = branches.iter().position(|&b| b == i).unwrap_or(0);
                    rainbow(rank as f32 / (branches.len() + 1) as f32)
                }
                None => color[p].clone(),
            };
        }
        let paint = |i: usize, solid: bool| -> String {
            let base = &color[i];
            let percent: u8 = if solid {
                92
            } else if has_children[i] {
                66
            } else {
                44
            };
            if base.starts_with('@') {
                format!("{base}/{percent}")
            } else {
                format!("{base}{:02X}", u32::from(percent) * 255 / 100)
            }
        };
        let is = |keys: &[String], i: usize| keys.iter().any(|k| *k == nodes[i].key);
        // Geometry per arc: d3's hairline padding, 1px between rings.
        let shape_of = |depth: usize, start: f32, end: f32| {
            let pad = ((end - start) / 2.).min(0.004);
            let inner = ring * depth as f32;
            let outer = (ring * (depth + 1) as f32 - 1.).max(inner);
            (inner, outer, start + pad, end - pad)
        };
        let geometry: Vec<(f32, f32, f32, f32)> = arcs
            .iter()
            .map(|a| shape_of(a.depth, a.start, a.end))
            .collect();
        // One drawing of arcs at given angles (node, depth, start, end).
        let draw = |angles: &[(usize, usize, f32, f32)]| -> Canvas {
            let mut c = Canvas::default();
            let mut outlines: Vec<(VectorPath, String, f32)> = vec![];
            for &(node, depth, start, end) in angles {
                let (inner, outer, a0, a1) = shape_of(depth, start, end);
                if a1 - a0 <= 1e-4 {
                    continue;
                }
                let shape = sector(r, r, outer, inner, a0, a1);
                let selected = is(&chart.selected, node);
                c.fill(shape.clone(), paint(node, selected));
                if chart.context_target.as_deref() == Some(nodes[node].key.as_str()) {
                    let dashes = dashed_sector(r, r, outer - 1., inner + 1., a0, a1);
                    outlines.push((dashes, ColorToken::BaseContent.into(), 1.5));
                } else if selected {
                    outlines.push((shape, ColorToken::BaseContent.into(), 1.5));
                } else if is(&chart.reviewed, node) {
                    outlines.push((shape, ColorToken::Primary.into(), 1.5));
                }
            }
            for (path, color, width) in outlines {
                c.stroke(path, color, width);
            }
            c
        };
        let target: Vec<(usize, usize, f32, f32)> = arcs
            .iter()
            .map(|a| (a.node, a.depth, a.start, a.end))
            .collect();
        // Animated from the previous data, d3-style: each arc tweens from
        // its old angles (new ones grow from their start), eased in-out.
        let tween = chart.previous.as_ref().and_then(|(before, revision)| {
            if before.first().map(|n| &n.key) != Some(&nodes[0].key) {
                return None;
            }
            let old: HashMap<&str, (f32, f32)> = sunburst_layout(before, chart.rings)
                .ok()?
                .iter()
                .map(|a| (before[a.node].key.as_str(), (a.start, a.end)))
                .collect();
            let from: Vec<(f32, f32)> = arcs
                .iter()
                .map(|a| {
                    old.get(nodes[a.node].key.as_str())
                        .copied()
                        .unwrap_or((a.start, a.start))
                })
                .collect();
            let moved = arcs
                .iter()
                .zip(&from)
                .any(|(a, f)| (a.start - f.0).abs() > 1e-4 || (a.end - f.1).abs() > 1e-4);
            moved.then_some((from, *revision))
        });
        let animated = tween.is_some();
        let drawing = match tween {
            Some((from, revision)) => {
                const FRAMES: usize = 12;
                let ease = |t: f32| {
                    if t < 0.5 {
                        4. * t * t * t
                    } else {
                        1. - (-2. * t + 2.).powi(3) / 2.
                    }
                };
                let frames = (1..=FRAMES)
                    .map(|f| {
                        let t = ease(f as f32 / FRAMES as f32);
                        let angles: Vec<_> = target
                            .iter()
                            .zip(&from)
                            .map(|(&(node, depth, start, end), &(s0, e0))| {
                                (node, depth, s0 + (start - s0) * t, e0 + (end - e0) * t)
                            })
                            .collect();
                        draw(&angles)
                    })
                    .collect();
                canvas_frames(frames, 30, revision)
            }
            None => {
                let still = draw(&target);
                canvas(move |c| *c = still)
            }
        }
        .key(format!("{}-drawing", chart.key))
        .width(size)
        .height(size);
        let mut layers: Vec<Element<C>> = vec![vstack![drawing]
            .key(format!(
                "{}-rings-{:x}",
                chart.key,
                stable_hash(&nodes[0].key)
            ))
            .width(size)
            .height(size)
            .transition(Transition::fade().duration_ms(180))
            .scroll_passthrough(true)
            .into()];
        // Labels where an arc has room, centred on its midpoint. When the
        // arcs tween, the labels cross-fade to their new places.
        if chart.labels {
            let mut labels: Vec<Element<C>> = vec![];
            for (a, &(inner, outer, a0, a1)) in arcs.iter().zip(&geometry) {
                let (mid_r, mid_a) = ((inner + outer) / 2., (a0 + a1) / 2.);
                let (length, thickness) = ((a1 - a0) * mid_r, outer - inner);
                let (c, s) = (mid_a.cos().abs(), mid_a.sin().abs());
                let across = length * c + thickness * s;
                let tall = thickness * c + length * s;
                if across < 34. || tall < 13. {
                    continue;
                }
                labels.push(
                    text(nodes[a.node].label.clone())
                        .key(format!("{}-label-{}", chart.key, a.node))
                        .font_size(10.)
                        .font_weight(500)
                        .color(ColorToken::BaseContent)
                        .truncate()
                        .max_width((across * 0.82).min(110.))
                        .offset(mid_r * mid_a.sin(), -mid_r * mid_a.cos())
                        .scroll_passthrough(true)
                        .into(),
                );
            }
            let revision = chart.previous.as_ref().map_or(0, |p| p.1);
            let layer = zstack(labels)
                .key(format!("{}-labels-{revision:x}", chart.key))
                .width(size)
                .height(size)
                .scroll_passthrough(true);
            layers.push(if animated {
                layer.transition(Transition::fade().duration_ms(260)).into()
            } else {
                layer.into()
            });
        }
        // Hit targets: a transparent button per arc, clipped to its sector.
        // Slivers too thin to point at are drawn but not interactive.
        let tile_id = |node: usize| format!("{}-tile-{node}", chart.key);
        let hit: Vec<usize> = (0..arcs.len())
            .filter(|&i| (geometry[i].3 - geometry[i].2) * geometry[i].1 >= 2.)
            .collect();
        let visible: HashSet<usize> = hit.iter().map(|&i| arcs[i].node).collect();
        let mut siblings: HashMap<usize, Vec<usize>> = HashMap::new();
        for &i in &hit {
            let node = arcs[i].node;
            siblings
                .entry(nodes[node].parent.unwrap_or(0))
                .or_default()
                .push(node);
        }
        let entry = hit
            .iter()
            .map(|&i| arcs[i].node)
            .find(|&node| is(&chart.selected, node))
            .or_else(|| hit.first().map(|&i| arcs[i].node));
        let group = format!("{}:{}{}", chart.key.len(), chart.key, nodes[0].key);
        for &i in &hit {
            let (a, (inner, outer, a0, a1)) = (arcs[i], geometry[i]);
            let node = &nodes[a.node];
            let reviewed = is(&chart.reviewed, a.node);
            let label = format!(
                "{} — {}{}",
                node.label,
                (chart.format)(node.value),
                if reviewed { " — In review" } else { "" }
            );
            // Each target covers only its arc's box, so tooltips, focus and
            // keyboard-opened menus sit at the arc, not the chart's centre.
            let [bx, by, bw, bh] = sector_bounds(r, r, outer, inner, a0, a1);
            let mut button = button("")
                .key(tile_id(a.node))
                .accessible_label(&label)
                .variant(ButtonVariant::Ghost)
                .width(bw)
                .height(bh)
                .rounded(0.)
                .border_width(0.)
                .background("#00000000")
                .hover_style(|s| s.background(ColorToken::BaseContent.alpha(12)));
            button.0.node.access_selected = Some(is(&chart.selected, a.node));
            if chart.hover_linked {
                button = button.hover_link(&node.key);
            }
            if chart.keyboard {
                let parent = node.parent.unwrap_or(0);
                let ring_mates = siblings.get(&parent).map(Vec::as_slice).unwrap_or(&[]);
                let position = ring_mates.iter().position(|&s| s == a.node).unwrap_or(0);
                let first_child = siblings.get(&a.node).and_then(|c| c.first()).copied();
                let up = node.parent.filter(|p| visible.contains(p));
                button = button
                    .tab_stop(entry == Some(a.node))
                    .focus_style(|s| s.background(ColorToken::BaseContent.alpha(18)));
                button.0.node.access_role = Some(AccessRole::TreeItem);
                button.0.node.access_parent =
                    Some(up.map(tile_id).unwrap_or_else(|| chart.key.clone()));
                button.0.node.access_expanded = first_child.map(|_| true);
                button.0.node.access_position = Some(position);
                button.0.node.access_set_size = Some(ring_mates.len());
                button.0.node.access_level = Some(a.depth - 1);
                button.0.node.roving = Some(RovingNavigation {
                    group: group.clone(),
                    member: node.key.clone(),
                    left: position.checked_sub(1).map(|p| tile_id(ring_mates[p])),
                    right: ring_mates.get(position + 1).copied().map(tile_id),
                    parent: up.map(tile_id),
                    child: first_child.map(tile_id),
                    first: ring_mates.first().copied().map(tile_id),
                    last: ring_mates.last().copied().map(tile_id),
                });
            }
            button = match chart.pick.clone() {
                Some(f) => {
                    let value = node.key.clone();
                    button
                        .cursor(Cursor::Pointer)
                        .on_click(move |app: &mut C| (f.borrow_mut())(app, value.clone()))
                }
                None => button.on_click(|_: &mut C| {}),
            };
            let mut tile: Element<C> = tooltip(label, button)
                .key(format!("{}-tip-{}", chart.key, a.node))
                .into();
            if let Some(open) = chart.context.clone() {
                let value = node.key.clone();
                let mut menu = zstack(vec![tile])
                    .key(format!("{}-context-{}", chart.key, a.node))
                    .width(bw)
                    .height(bh);
                menu.0.callback = Some(Callback::Context(Box::new(move |app, position| {
                    (open.borrow_mut())(app, value.clone(), position)
                })));
                tile = menu.into();
            }
            let mut slice: Element<C> = zstack(vec![tile])
                .key(format!("{}-slice-{}", chart.key, a.node))
                .width(bw)
                .height(bh)
                .clip(Clip::Path(sector(r - bx, r - by, outer, inner, a0, a1)))
                .into();
            if chart.hover_linked {
                slice.node.hover_link = Some(node.key.clone());
            }
            layers.push(
                vstack![slice]
                    .padding(bx, Edge::Left)
                    .padding(by, Edge::Top)
                    .width(size)
                    .height(size)
                    .flow(Flow::TopLeft)
                    .scroll_passthrough(true)
                    .into(),
            );
        }
        // The centre: the root's name and size; optionally a way back up.
        let hole = ring * 2. - 2.;
        let center_text = vstack![
            text(nodes[0].label.clone())
                .font_size(12.)
                .font_weight(600)
                .truncate()
                .max_width(hole * 0.8),
            text((chart.format)(nodes[0].value))
                .font_size(11.)
                .color(ColorToken::Muted)
        ]
        .gap(1., Axis::Vertical)
        .align(Align::Center)
        .width(hole)
        .height(hole)
        .flow(Flow::Center)
        .scroll_passthrough(true);
        if let Some((label, back)) = chart.back {
            layers.push(
                zstack![button("")
                    .key(format!("{}-center", chart.key))
                    .accessible_label(label)
                    .variant(ButtonVariant::Ghost)
                    .width(hole)
                    .height(hole)
                    .rounded(Rounding::Full)
                    .background("#00000000")
                    .hover_style(|s| s.background(ColorToken::BaseContent.alpha(7)))
                    .cursor(Cursor::Pointer)
                    .on_click(move |app: &mut C| (back.borrow_mut())(app))]
                .width(hole)
                .height(hole)
                .clip(Clip::Circle)
                .into(),
            );
        }
        layers.push(center_text.into());
        let mut chart_element = zstack(layers)
            .key(chart.key)
            .width(size)
            .height(size)
            .min_width(size)
            .min_height(size);
        if chart.keyboard {
            chart_element.0.node.access_role = Some(AccessRole::Tree);
        }
        chart_element.into()
    }
}
