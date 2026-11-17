//! Renderer-independent image handles, vector paths and explicit clip geometry.
//! Assets are supplied by the host; this module never reads files or the network.
use std::sync::Arc;

/// Immutable premultiplied RGBA pixels. Clones share storage; at most 16 MP.
#[derive(Clone, Debug)]
pub struct Image(Arc<ImageData>);
#[derive(Debug)]
struct ImageData {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Image {
    /// Input is straight RGBA; conversion happens once, never during painting.
    pub fn from_rgba(width: u32, height: u32, mut pixels: Vec<u8>) -> Result<Self, &'static str> {
        let count = u64::from(width) * u64::from(height);
        if count == 0 || count > 16_777_216 || count * 4 != pixels.len() as u64 {
            return Err("invalid image dimensions/data (16 MP maximum)");
        }
        for p in pixels.as_chunks_mut::<4>().0.iter_mut() {
            for i in 0..3 {
                p[i] = ((u16::from(p[i]) * u16::from(p[3]) + 127) / 255) as u8;
            }
        }
        Ok(Self(Arc::new(ImageData {
            width,
            height,
            pixels,
        })))
    }
    pub fn width(&self) -> u32 {
        self.0.width
    }
    pub fn height(&self) -> u32 {
        self.0.height
    }
    /// A renderer cache is the last owner; no application can reuse this
    /// immutable image identity. Internal only: this is not an eviction hint
    /// for live application-owned images.
    #[cfg(feature = "rendering-gpu")]
    pub(crate) fn is_uniquely_owned(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }

    /// Premultiplied RGBA, suitable for renderer upload without decoding.
    pub fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }
    pub fn background(self) -> BackgroundImage {
        BackgroundImage {
            image: self,
            fit: ImageFit::Cover,
            position: ImagePosition::Center,
            opacity: 1.,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    #[default]
    Cover,
    Contain,
    Stretch,
    Tile,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImagePosition {
    #[default]
    Center,
    Top,
    Right,
    Bottom,
    Left,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
impl ImagePosition {
    pub fn fractions(self) -> [f32; 2] {
        match self {
            Self::Center => [0.5, 0.5],
            Self::Top => [0.5, 0.],
            Self::Right => [1., 0.5],
            Self::Bottom => [0.5, 1.],
            Self::Left => [0., 0.5],
            Self::TopLeft => [0., 0.],
            Self::TopRight => [1., 0.],
            Self::BottomLeft => [0., 1.],
            Self::BottomRight => [1., 1.],
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundImage {
    pub image: Image,
    pub fit: ImageFit,
    pub position: ImagePosition,
    pub opacity: f32,
}
impl From<Image> for BackgroundImage {
    fn from(value: Image) -> Self {
        value.background()
    }
}
impl BackgroundImage {
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
    pub fn position(mut self, position: ImagePosition) -> Self {
        self.position = position;
        self
    }
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum PathCommand {
    Move([f32; 2]),
    Line([f32; 2]),
    Quad([f32; 2], [f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    Close,
}
/// Most commands one [`VectorPath`] may hold.
pub const MAX_PATH_COMMANDS: usize = 1024;
/// Most paths one [`Canvas`] may hold.
pub const MAX_CANVAS_PATHS: usize = 256;

/// Paths use local logical coordinates. Fills/clips use the even-odd rule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorPath {
    pub commands: Vec<PathCommand>,
}
impl VectorPath {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCommand::Move([x, y]));
        self
    }
    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCommand::Line([x, y]));
        self
    }
    pub fn quad_to(mut self, control: [f32; 2], end: [f32; 2]) -> Self {
        self.commands.push(PathCommand::Quad(control, end));
        self
    }
    pub fn cubic_to(mut self, a: [f32; 2], b: [f32; 2], end: [f32; 2]) -> Self {
        self.commands.push(PathCommand::Cubic(a, b, end));
        self
    }
    pub fn close(mut self) -> Self {
        self.commands.push(PathCommand::Close);
        self
    }
    pub fn rectangle(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new()
            .move_to(x, y)
            .line_to(x + w, y)
            .line_to(x + w, y + h)
            .line_to(x, y + h)
            .close()
    }
    /// A rectangle with rounded corners; the radius is capped at half the
    /// shorter side (a capsule).
    pub fn rounded_rectangle(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Self {
        let r = radius.max(0.).min(w * 0.5).min(h * 0.5);
        if !(r > 0.) {
            return Self::rectangle(x, y, w, h);
        }
        let k = 0.552_284_8 * r;
        Self::new()
            .move_to(x + r, y)
            .line_to(x + w - r, y)
            .cubic_to([x + w - r + k, y], [x + w, y + r - k], [x + w, y + r])
            .line_to(x + w, y + h - r)
            .cubic_to(
                [x + w, y + h - r + k],
                [x + w - r + k, y + h],
                [x + w - r, y + h],
            )
            .line_to(x + r, y + h)
            .cubic_to([x + r - k, y + h], [x, y + h - r + k], [x, y + h - r])
            .line_to(x, y + r)
            .cubic_to([x, y + r - k], [x + r - k, y], [x + r, y])
            .close()
    }
    pub fn circle(x: f32, y: f32, r: f32) -> Self {
        Self::ellipse(x, y, r, r)
    }
    pub fn ellipse(x: f32, y: f32, rx: f32, ry: f32) -> Self {
        let k = 0.552_284_8;
        Self::new()
            .move_to(x + rx, y)
            .cubic_to([x + rx, y + k * ry], [x + k * rx, y + ry], [x, y + ry])
            .cubic_to([x - k * rx, y + ry], [x - rx, y + k * ry], [x - rx, y])
            .cubic_to([x - rx, y - k * ry], [x - k * rx, y - ry], [x, y - ry])
            .cubic_to([x + k * rx, y - ry], [x + rx, y - k * ry], [x + rx, y])
            .close()
    }
    /// An arc of a circle as an open path, for stroking (gauges, rings,
    /// progress). Angles are degrees, 0° at 12 o'clock and clockwise, as a
    /// clock or a gauge reads; a negative `sweep` runs anticlockwise. Sweeps
    /// beyond a full turn draw one full turn.
    pub fn arc(cx: f32, cy: f32, radius: f32, start: f32, sweep: f32) -> Self {
        let mut path = Self::new();
        let first = polar(cx, cy, radius, start);
        path = path.move_to(first[0], first[1]);
        arc_segments(path, cx, cy, radius, start, sweep.clamp(-360., 360.))
    }
    /// A ring segment between `inner` and `outer` radii, as a closed shape to
    /// fill (donut and sunburst slices, gauge tracks); `inner` 0 gives a pie
    /// slice. Angles as for [`VectorPath::arc`].
    pub fn sector(cx: f32, cy: f32, outer: f32, inner: f32, start: f32, sweep: f32) -> Self {
        let sweep = sweep.clamp(-360., 360.);
        let mut path = Self::arc(cx, cy, outer, start, sweep);
        if inner > 0. {
            let back = polar(cx, cy, inner, start + sweep);
            path = path.line_to(back[0], back[1]);
            path = arc_segments(path, cx, cy, inner, start + sweep, -sweep);
        } else {
            path = path.line_to(cx, cy);
        }
        path.close()
    }
    /// The box the path's points span, control points included:
    /// `[x, y, width, height]`. `None` for an empty path.
    pub fn bounds(&self) -> Option<[f32; 4]> {
        let mut points = self.commands.iter().flat_map(|c| match c {
            PathCommand::Move(p) | PathCommand::Line(p) => vec![*p],
            PathCommand::Quad(a, b) => vec![*a, *b],
            PathCommand::Cubic(a, b, c) => vec![*a, *b, *c],
            PathCommand::Close => vec![],
        });
        let first = points.next()?;
        let [mut x0, mut y0, mut x1, mut y1] = [first[0], first[1], first[0], first[1]];
        for [x, y] in points {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        Some([x0, y0, x1 - x0, y1 - y0])
    }
    /// The path's length along its segments (curves measured on a flattened
    /// copy), in its own units.
    pub fn length(&self) -> f32 {
        segments(self).iter().map(|s| s.length).sum()
    }
    /// The first `fraction` (0..=1) of the path by length: a line that draws
    /// itself in is `path.trimmed(t)` for each animation frame. Curves are
    /// cut at the matching point, so the shape doesn't change as it grows.
    pub fn trimmed(&self, fraction: f32) -> Self {
        let fraction = if fraction.is_finite() {
            fraction.clamp(0., 1.)
        } else {
            0.
        };
        if fraction >= 1. {
            return self.clone();
        }
        if fraction <= 0. {
            // Only the start: no drawing command, so nothing is painted (not
            // even a round cap's dot).
            return Self {
                commands: self.commands.first().cloned().into_iter().collect(),
            };
        }
        let segs = segments(self);
        let mut left = fraction * segs.iter().map(|s| s.length).sum::<f32>();
        let mut out = Self::new();
        for seg in segs {
            match &seg.command {
                PathCommand::Move(p) => out.commands.push(PathCommand::Move(*p)),
                _ if left >= seg.length => {
                    out.commands.push(seg.command.clone());
                    left -= seg.length;
                }
                command => {
                    let t = seg.parameter_at(left);
                    out.commands.push(match command {
                        PathCommand::Line(p) => PathCommand::Line(lerp(seg.from, *p, t)),
                        PathCommand::Close => PathCommand::Line(lerp(seg.from, seg.start, t)),
                        PathCommand::Quad(c, e) => {
                            let (c1, e1) = split_quad(seg.from, *c, *e, t);
                            PathCommand::Quad(c1, e1)
                        }
                        PathCommand::Cubic(a, b, e) => {
                            let (a1, b1, e1) = split_cubic(seg.from, *a, *b, *e, t);
                            PathCommand::Cubic(a1, b1, e1)
                        }
                        PathCommand::Move(_) => unreachable!("handled above"),
                    });
                    break;
                }
            }
        }
        out
    }
    /// Parse SVG path data (the `d` attribute): M L H V C S Q T A Z, absolute
    /// and relative, with implicit repeats. Arcs become cubic curves. Use it
    /// to draw icons and logos exported from design tools.
    pub fn from_svg(d: &str) -> Result<Self, String> {
        svg::parse(d)
    }
    /// Affine matrix `[a, b, c, d, tx, ty]`. Can translate/scale/rotate paths in Rust.
    pub fn transformed(mut self, m: [f32; 6]) -> Self {
        let point = |p: &mut [f32; 2]| {
            let [x, y] = *p;
            *p = [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
        };
        for c in &mut self.commands {
            match c {
                PathCommand::Move(p) | PathCommand::Line(p) => point(p),
                PathCommand::Quad(a, b) => {
                    point(a);
                    point(b)
                }
                PathCommand::Cubic(a, b, c) => {
                    point(a);
                    point(b);
                    point(c)
                }
                PathCommand::Close => {}
            }
        }
        self
    }
    pub fn is_valid(&self) -> bool {
        let valid = |p: &[f32; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 65_536.);
        !self.commands.is_empty()
            && self.commands.iter().any(|c| {
                matches!(
                    c,
                    PathCommand::Line(_) | PathCommand::Quad(..) | PathCommand::Cubic(..)
                )
            })
            && self.commands.len() <= MAX_PATH_COMMANDS
            && matches!(self.commands[0], PathCommand::Move(_))
            && self.commands.iter().all(|c| match c {
                PathCommand::Move(p) | PathCommand::Line(p) => valid(p),
                PathCommand::Quad(a, b) => valid(a) && valid(b),
                PathCommand::Cubic(a, b, c) => valid(a) && valid(b) && valid(c),
                PathCommand::Close => true,
            })
    }
    /// Even-odd interior. Curves are flattened at bounded resolution for input.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let mut inside = false;
        let mut prev = [0.; 2];
        let mut start = prev;
        let mut open = false;
        let edge = |a: [f32; 2], b: [f32; 2], inside: &mut bool| {
            if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
                *inside = !*inside;
            }
        };
        for c in &self.commands {
            match *c {
                PathCommand::Move(p) => {
                    if open {
                        edge(prev, start, &mut inside)
                    };
                    prev = p;
                    start = p;
                    open = true
                }
                PathCommand::Line(p) => {
                    edge(prev, p, &mut inside);
                    prev = p
                }
                PathCommand::Quad(a, b) => {
                    let from = prev;
                    for i in 1..=32 {
                        let t = i as f32 / 32.;
                        let u = 1. - t;
                        let p = [
                            u * u * from[0] + 2. * u * t * a[0] + t * t * b[0],
                            u * u * from[1] + 2. * u * t * a[1] + t * t * b[1],
                        ];
                        edge(prev, p, &mut inside);
                        prev = p
                    }
                }
                PathCommand::Cubic(a, b, c) => {
                    let from = prev;
                    for i in 1..=32 {
                        let t = i as f32 / 32.;
                        let u = 1. - t;
                        let p = [
                            u * u * u * from[0]
                                + 3. * u * u * t * a[0]
                                + 3. * u * t * t * b[0]
                                + t * t * t * c[0],
                            u * u * u * from[1]
                                + 3. * u * u * t * a[1]
                                + 3. * u * t * t * b[1]
                                + t * t * t * c[1],
                        ];
                        edge(prev, p, &mut inside);
                        prev = p
                    }
                }
                PathCommand::Close => {
                    edge(prev, start, &mut inside);
                    prev = start;
                    open = false
                }
            }
        }
        if open {
            edge(prev, start, &mut inside)
        };
        inside
    }
}
/// Logical-pixel dial geometry shared by gauges and rotary sliders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialStyle {
    pub(crate) start: f32,
    pub(crate) sweep: f32,
    pub(crate) stroke: f32,
    pub(crate) ticks: u16,
}
impl Default for DialStyle {
    fn default() -> Self {
        Self::new()
    }
}
impl DialStyle {
    pub const fn new() -> Self {
        Self {
            start: 135.,
            sweep: 270.,
            stroke: 5.,
            ticks: 11,
        }
    }
    pub fn start_angle(mut self, degrees: f32) -> Self {
        self.start = degrees;
        self
    }
    pub fn sweep(mut self, degrees: f32) -> Self {
        self.sweep = degrees;
        self
    }
    pub fn stroke_width(mut self, logical_pixels: f32) -> Self {
        self.stroke = logical_pixels;
        self
    }
    pub fn ticks(mut self, count: u16) -> Self {
        self.ticks = count;
        self
    }
    #[cfg(feature = "declarative")]
    pub(crate) fn valid(self) -> bool {
        self.start.is_finite()
            && self.start.abs() <= 3600.
            && self.sweep.is_finite()
            && (1. ..=360.).contains(&self.sweep)
            && self.stroke.is_finite()
            && (0.1..=32.).contains(&self.stroke)
            && self.ticks <= 64
    }
}
/// Isolated subtree colour compositing against the content painted below it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    ColorBurn,
    Difference,
    /// Porter-Duff subtraction of source alpha from the destination.
    DestinationOut,
    /// Keep only non-overlapping alpha coverage.
    Xor,
}
/// How a mask element cuts the content of the container it masks
/// (see `Container::mask`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MaskMode {
    /// Content shows where the mask is opaque: a gradient mask fades it.
    Alpha,
    /// Content shows where the mask is bright: white shows, black and
    /// transparent hide (0.2126 R + 0.7152 G + 0.0722 B, times alpha).
    Luminance,
}
/// A point on a circle; 0° at 12 o'clock, clockwise.
fn polar(cx: f32, cy: f32, r: f32, degrees: f32) -> [f32; 2] {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [cx + r * sin, cy - r * cos]
}
/// Cubic arcs of at most 90° each, continuing `path` from the arc's start.
fn arc_segments(
    mut path: VectorPath,
    cx: f32,
    cy: f32,
    r: f32,
    start: f32,
    sweep: f32,
) -> VectorPath {
    let pieces = (sweep.abs() / 90.).ceil().max(1.) as usize;
    let step = sweep / pieces as f32;
    let k = 4. / 3. * (step.to_radians() / 4.).tan() * r;
    for i in 0..pieces {
        let (a0, a1) = (start + step * i as f32, start + step * (i + 1) as f32);
        let (p0, p1) = (polar(cx, cy, r, a0), polar(cx, cy, r, a1));
        // Tangents, clockwise-positive: d/dθ of (sin θ, −cos θ) is (cos θ, sin θ).
        let (s0, c0) = a0.to_radians().sin_cos();
        let (s1, c1) = a1.to_radians().sin_cos();
        path = path.cubic_to(
            [p0[0] + k * c0, p0[1] + k * s0],
            [p1[0] - k * c1, p1[1] - k * s1],
            p1,
        );
    }
    path
}
/// One drawable piece of a path with its start point and length.
struct Segment {
    command: PathCommand,
    from: [f32; 2],
    /// The subpath's start, for `Close`.
    start: [f32; 2],
    length: f32,
    /// Cumulative lengths of a flattened copy, for curves (parameter lookup).
    steps: Vec<f32>,
}
const FLATTEN: usize = 24;
impl Segment {
    /// The curve parameter at `distance` along the segment.
    fn parameter_at(&self, distance: f32) -> f32 {
        if self.length <= 0. {
            return 0.;
        }
        if self.steps.is_empty() {
            return (distance / self.length).clamp(0., 1.);
        }
        let i = self
            .steps
            .iter()
            .position(|d| *d >= distance)
            .unwrap_or(FLATTEN - 1);
        let before = if i == 0 { 0. } else { self.steps[i - 1] };
        let span = (self.steps[i] - before).max(f32::EPSILON);
        ((i as f32 + (distance - before) / span) / FLATTEN as f32).clamp(0., 1.)
    }
}
fn lerp(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}
fn split_quad(p0: [f32; 2], c: [f32; 2], e: [f32; 2], t: f32) -> ([f32; 2], [f32; 2]) {
    let a = lerp(p0, c, t);
    let b = lerp(c, e, t);
    (a, lerp(a, b, t))
}
fn split_cubic(
    p0: [f32; 2],
    a: [f32; 2],
    b: [f32; 2],
    e: [f32; 2],
    t: f32,
) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let (ab, bc, cd) = (lerp(p0, a, t), lerp(a, b, t), lerp(b, e, t));
    let (abc, bcd) = (lerp(ab, bc, t), lerp(bc, cd, t));
    (ab, abc, lerp(abc, bcd, t))
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}
fn segments(path: &VectorPath) -> Vec<Segment> {
    let mut out = Vec::new();
    let (mut at, mut start) = ([0.; 2], [0.; 2]);
    for command in &path.commands {
        let command = command.clone();
        let (end, length, steps) = match command {
            PathCommand::Move(p) => {
                start = p;
                (p, 0., Vec::new())
            }
            PathCommand::Line(p) => (p, distance(at, p), Vec::new()),
            PathCommand::Close => (start, distance(at, start), Vec::new()),
            PathCommand::Quad(c, e) => {
                let point = |t: f32| lerp(lerp(at, c, t), lerp(c, e, t), t);
                flatten(point, e)
            }
            PathCommand::Cubic(a, b, e) => {
                let point = |t: f32| {
                    let (ab, bc, cd) = (lerp(at, a, t), lerp(a, b, t), lerp(b, e, t));
                    lerp(lerp(ab, bc, t), lerp(bc, cd, t), t)
                };
                flatten(point, e)
            }
        };
        out.push(Segment {
            command,
            from: at,
            start,
            length,
            steps,
        });
        at = end;
    }
    out
}
/// A curve's end, its flattened length and the cumulative step lengths.
fn flatten(point: impl Fn(f32) -> [f32; 2], end: [f32; 2]) -> ([f32; 2], f32, Vec<f32>) {
    let mut steps = Vec::with_capacity(FLATTEN);
    let (mut prev, mut total) = (point(0.), 0.);
    for i in 1..=FLATTEN {
        let p = point(i as f32 / FLATTEN as f32);
        total += distance(prev, p);
        steps.push(total);
        prev = p;
    }
    (end, total, steps)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Clip {
    Bounds,
    Rounded,
    Circle,
    Path(VectorPath),
}
/// Resolved physical-pixel clip shared by rendering and interaction.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipRegion {
    Rounded { bounds: [f32; 4], radii: [f32; 4] },
    Circle { bounds: [f32; 4] },
    Path(VectorPath),
}
impl ClipRegion {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        match self {
            Self::Rounded { bounds, radii } => {
                crate::Rect::new(bounds[0], bounds[1], bounds[2], bounds[3])
                    .is_some_and(|r| r.contains_corners(*radii, x, y))
            }
            Self::Circle {
                bounds: [bx, by, w, h],
            } => {
                let r = w.min(*h) * 0.5;
                (x - (bx + w * 0.5)).hypot(y - (by + h * 0.5)) < r
            }
            Self::Path(p) => p.contains(x, y),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DrawPath {
    pub path: VectorPath,
    pub color: String,
    pub stroke: Option<f32>,
    /// Gradient paint and stroke caps, joins and dashes, when set
    /// ([`Canvas::stroke_with`], [`Canvas::fill_gradient`], [`Canvas::stroke_gradient`]).
    pub style: Option<Box<PathStyle>>,
}
/// How a canvas path is painted beyond a plain colour and width.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathStyle {
    /// Paint with this gradient instead of the colour. It is laid over the
    /// path's bounds as a CSS gradient is over a box: `Gradient::linear(90., ..)`
    /// runs from the path's left edge to its right.
    pub gradient: Option<crate::declarative::Gradient>,
    /// Stroke with these caps, joins and dashes instead of a plain width.
    pub stroke: Option<StrokeStyle>,
}
/// Where an open stroke's ends stop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineCap {
    /// Flat, at the end point.
    #[default]
    Butt,
    /// A half circle past the end point.
    Round,
    /// A half square past the end point.
    Square,
}
/// How a stroke turns a corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}
/// A stroke beyond a plain width: caps, joins and dashes, in logical pixels.
///
/// ```
/// use volt_ui_kit::graphics::StrokeStyle;
/// let dotted = StrokeStyle::new(3.).round().dash([0., 8.]);
/// assert!(dotted.is_valid());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    /// On/off lengths, repeating; empty for a solid stroke. A zero "on"
    /// length with round caps draws dots.
    pub dash: Vec<f32>,
}
impl StrokeStyle {
    pub fn new(width: f32) -> Self {
        Self {
            width,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dash: Vec::new(),
        }
    }
    pub fn cap(mut self, cap: LineCap) -> Self {
        self.cap = cap;
        self
    }
    pub fn join(mut self, join: LineJoin) -> Self {
        self.join = join;
        self
    }
    /// Round caps and joins: soft line ends, smooth corners.
    pub fn round(self) -> Self {
        self.cap(LineCap::Round).join(LineJoin::Round)
    }
    /// On/off lengths, repeating. An odd count repeats once more, as SVG does.
    pub fn dash(mut self, pattern: impl IntoIterator<Item = f32>) -> Self {
        self.dash = pattern.into_iter().collect();
        if self.dash.len() % 2 == 1 {
            self.dash.extend(self.dash.clone());
        }
        self
    }
    /// A width the renderers accept, and a dash pattern with some length.
    pub fn is_valid(&self) -> bool {
        self.width.is_finite()
            && self.width > 0.
            && self.width <= 256.
            && self.dash.len() <= 32
            && self
                .dash
                .iter()
                .all(|d| d.is_finite() && *d >= 0. && *d <= 65_536.)
            && (self.dash.is_empty() || self.dash.iter().sum::<f32>() > 0.)
    }
    /// The same stroke at `scale` (logical to physical pixels).
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            width: self.width * scale,
            dash: self.dash.iter().map(|d| d * scale).collect(),
            ..self.clone()
        }
    }
}
/// A retained list built by an ordinary Rust closure when the view composes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    pub paths: Vec<DrawPath>,
}
/// A short drawing sequence played at paint time (see `ui::canvas_frames`):
/// no recomposition or layout per frame. It restarts only when `revision`
/// changes for the same element id.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasFrames {
    pub frames: std::sync::Arc<[Canvas]>,
    pub frame_ms: u32,
    pub revision: u64,
    /// Play each revision once per key: coming back to it (after navigating
    /// away) shows the last frame at once (`ui::canvas_frames_once`).
    pub once: bool,
}
impl CanvasFrames {
    /// How long the sequence plays.
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(u64::from(self.frame_ms) * self.frames.len() as u64)
    }
    /// The frame to show `elapsed` after it started; the last one after.
    pub fn at(&self, elapsed: std::time::Duration) -> Option<&Canvas> {
        let ms = elapsed.as_millis() as u64;
        let i = (ms / u64::from(self.frame_ms.max(1))) as usize;
        self.frames.get(i.min(self.frames.len().saturating_sub(1)))
    }
}
impl Canvas {
    pub fn fill(&mut self, path: VectorPath, color: impl Into<String>) {
        self.push(DrawPath {
            path,
            color: color.into(),
            stroke: None,
            style: None,
        });
    }
    pub fn stroke(&mut self, path: VectorPath, color: impl Into<String>, width: f32) {
        self.push(DrawPath {
            path,
            color: color.into(),
            stroke: Some(width),
            style: None,
        });
    }
    /// Stroke with caps, joins and dashes: `StrokeStyle::new(2.).round()`.
    pub fn stroke_with(&mut self, path: VectorPath, color: impl Into<String>, style: StrokeStyle) {
        self.push(DrawPath {
            path,
            color: color.into(),
            stroke: Some(style.width),
            style: Some(Box::new(PathStyle {
                gradient: None,
                stroke: Some(style),
            })),
        });
    }
    /// Fill with a gradient laid over the path's bounds.
    pub fn fill_gradient(&mut self, path: VectorPath, gradient: crate::declarative::Gradient) {
        let color = gradient.first_color();
        self.push(DrawPath {
            path,
            color,
            stroke: None,
            style: Some(Box::new(PathStyle {
                gradient: Some(gradient),
                stroke: None,
            })),
        });
    }
    /// Stroke with a gradient laid over the path's bounds: a line that
    /// shades from one colour to another along its run.
    pub fn stroke_gradient(
        &mut self,
        path: VectorPath,
        gradient: crate::declarative::Gradient,
        style: StrokeStyle,
    ) {
        let color = gradient.first_color();
        self.push(DrawPath {
            path,
            color,
            stroke: Some(style.width),
            style: Some(Box::new(PathStyle {
                gradient: Some(gradient),
                stroke: Some(style),
            })),
        });
    }
    /// Adds a path unless it draws nothing: an empty path, or one that only
    /// moves (a draw-in's first frame), is skipped rather than rejected.
    fn push(&mut self, path: DrawPath) {
        let draws = path.path.commands.iter().any(|c| {
            matches!(
                c,
                PathCommand::Line(_) | PathCommand::Quad(..) | PathCommand::Cubic(..)
            )
        });
        if draws {
            self.paths.push(path);
        }
    }
    /// A soft glow under a stroke: wider, fainter copies beneath it, then
    /// the stroke itself. `color` is a hex colour (`#RRGGBB`) or a token;
    /// `strength` 0..=1 sets how bright the halo is.
    pub fn glow_stroke(
        &mut self,
        path: VectorPath,
        color: &str,
        style: StrokeStyle,
        strength: f32,
    ) {
        let strength = if strength.is_finite() {
            strength.clamp(0., 1.)
        } else {
            0.
        };
        for (spread, alpha) in [(4.5, 0.10), (2.8, 0.16), (1.7, 0.26)] {
            let halo = StrokeStyle {
                width: (style.width * spread).min(256.),
                cap: LineCap::Round,
                join: LineJoin::Round,
                ..style.clone()
            };
            self.stroke_with(path.clone(), faded(color, alpha * strength), halo);
        }
        self.stroke_with(path, color, style);
    }
}
/// `color` at `alpha` (0..=1): an alpha byte on hex colours, `/NN` on tokens.
pub(crate) fn faded(color: &str, alpha: f32) -> String {
    let percent = (alpha.clamp(0., 1.) * 100.).round() as u8;
    if color.starts_with('@') {
        format!("{color}/{percent}")
    } else if color.len() == 7 && color.starts_with('#') {
        format!("{color}{:02X}", (alpha.clamp(0., 1.) * 255.).round() as u8)
    } else {
        color.to_owned()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_handles_share_storage_and_premultiply_once() {
        let image = Image::from_rgba(1, 1, vec![255, 128, 0, 128]).unwrap();
        assert_eq!(image.pixels(), [128, 64, 0, 128]);
        assert_eq!(image, image.clone());
        assert!(Image::from_rgba(u32::MAX, u32::MAX, vec![]).is_err());
        assert!(Image::from_rgba(2, 2, vec![0; 4]).is_err());
    }
    #[test]
    fn paths_transform_even_odd_holes_and_validate_finite_commands() {
        let mut path = VectorPath::rectangle(0., 0., 100., 100.);
        path.commands
            .extend(VectorPath::rectangle(20., 20., 60., 60.).commands);
        assert!(path.contains(10., 10.));
        assert!(!path.contains(50., 50.));
        let path = path.transformed([2., 0., 0., 2., 10., 20.]);
        assert!(path.contains(30., 40.));
        assert!(!path.contains(110., 120.));
        assert!(!VectorPath::new().move_to(0., 0.).is_valid());
        assert!(!VectorPath::circle(0., 0., f32::NAN).is_valid());
        assert!(VectorPath::circle(50., 50., 25.).contains(50., 50.));
    }
}

mod svg {
    use super::{PathCommand, VectorPath};
    struct Lexer<'a> {
        s: &'a [u8],
        i: usize,
    }
    impl Lexer<'_> {
        fn skip(&mut self) {
            while self.i < self.s.len()
                && matches!(self.s[self.i], b' ' | b',' | b'\t' | b'\n' | b'\r')
            {
                self.i += 1;
            }
        }
        fn command(&mut self) -> Option<u8> {
            self.skip();
            let c = *self.s.get(self.i)?;
            if c.is_ascii_alphabetic() {
                self.i += 1;
                Some(c)
            } else {
                None
            }
        }
        fn at_number(&mut self) -> bool {
            self.skip();
            self.s
                .get(self.i)
                .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.'))
        }
        fn number(&mut self) -> Result<f32, String> {
            self.skip();
            let start = self.i;
            let mut seen_dot = false;
            let mut seen_exp = false;
            if matches!(self.s.get(self.i), Some(b'-' | b'+')) {
                self.i += 1;
            }
            while let Some(&c) = self.s.get(self.i) {
                match c {
                    b'0'..=b'9' => self.i += 1,
                    b'.' if !seen_dot && !seen_exp => {
                        seen_dot = true;
                        self.i += 1
                    }
                    b'e' | b'E' if !seen_exp => {
                        seen_exp = true;
                        self.i += 1;
                        if matches!(self.s.get(self.i), Some(b'-' | b'+')) {
                            self.i += 1;
                        }
                    }
                    _ => break,
                }
            }
            std::str::from_utf8(&self.s[start..self.i])
                .ok()
                .and_then(|t| t.parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("invalid number at byte {start}"))
        }
        /// Arc flags may be written without separators ("a1 1 0 01 5 5").
        fn flag(&mut self) -> Result<bool, String> {
            self.skip();
            match self.s.get(self.i) {
                Some(b'0') => {
                    self.i += 1;
                    Ok(false)
                }
                Some(b'1') => {
                    self.i += 1;
                    Ok(true)
                }
                _ => Err(format!("invalid arc flag at byte {}", self.i)),
            }
        }
    }
    /// SVG arc (endpoint form) to cubic segments of at most 90 degrees.
    #[allow(clippy::too_many_arguments)]
    fn arc(
        out: &mut Vec<PathCommand>,
        from: [f32; 2],
        mut rx: f32,
        mut ry: f32,
        rotation: f32,
        large: bool,
        sweep: bool,
        to: [f32; 2],
    ) {
        if from == to {
            return;
        }
        rx = rx.abs();
        ry = ry.abs();
        if rx == 0. || ry == 0. {
            out.push(PathCommand::Line(to));
            return;
        }
        let (sin, cos) = rotation.to_radians().sin_cos();
        let dx = (from[0] - to[0]) * 0.5;
        let dy = (from[1] - to[1]) * 0.5;
        let x1 = cos * dx + sin * dy;
        let y1 = -sin * dx + cos * dy;
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        if lambda > 1. {
            rx *= lambda.sqrt();
            ry *= lambda.sqrt();
        }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
        let mut coef = (num / den).max(0.).sqrt();
        if large == sweep {
            coef = -coef;
        }
        let cx1 = coef * rx * y1 / ry;
        let cy1 = -coef * ry * x1 / rx;
        let cx = cos * cx1 - sin * cy1 + (from[0] + to[0]) * 0.5;
        let cy = sin * cx1 + cos * cy1 + (from[1] + to[1]) * 0.5;
        let angle =
            |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
        let start = angle(1., 0., (x1 - cx1) / rx, (y1 - cy1) / ry);
        let mut delta = angle(
            (x1 - cx1) / rx,
            (y1 - cy1) / ry,
            (-x1 - cx1) / rx,
            (-y1 - cy1) / ry,
        );
        if !sweep && delta > 0. {
            delta -= std::f32::consts::TAU;
        } else if sweep && delta < 0. {
            delta += std::f32::consts::TAU;
        }
        let segments = (delta.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.) as usize;
        let step = delta / segments as f32;
        let k = 4. / 3. * (step / 4.).tan();
        let point = |t: f32| {
            let (st, ct) = t.sin_cos();
            [
                cx + rx * ct * cos - ry * st * sin,
                cy + rx * ct * sin + ry * st * cos,
            ]
        };
        let derivative = |t: f32| {
            let (st, ct) = t.sin_cos();
            [
                -rx * st * cos - ry * ct * sin,
                -rx * st * sin + ry * ct * cos,
            ]
        };
        for i in 0..segments {
            let (t0, t1) = (start + step * i as f32, start + step * (i + 1) as f32);
            let (p0, p1) = (point(t0), point(t1));
            let (d0, d1) = (derivative(t0), derivative(t1));
            let end = if i + 1 == segments { to } else { p1 };
            out.push(PathCommand::Cubic(
                [p0[0] + k * d0[0], p0[1] + k * d0[1]],
                [p1[0] - k * d1[0], p1[1] - k * d1[1]],
                end,
            ));
        }
    }
    pub(super) fn parse(d: &str) -> Result<VectorPath, String> {
        if d.len() > 1 << 20 {
            return Err("path data larger than 1 MiB".into());
        }
        let mut lx = Lexer {
            s: d.as_bytes(),
            i: 0,
        };
        let mut out = Vec::new();
        let (mut cur, mut start) = ([0_f32; 2], [0_f32; 2]);
        // Reflection points for S/s and T/t.
        let (mut last_cubic, mut last_quad): (Option<[f32; 2]>, Option<[f32; 2]>) = (None, None);
        let mut cmd = None;
        loop {
            lx.skip();
            if lx.i >= lx.s.len() {
                break;
            }
            if let Some(c) = lx.command() {
                cmd = Some(c);
            } else if cmd.is_none() {
                return Err("path data must start with a command".into());
            }
            let c = cmd.unwrap();
            let rel = c.is_ascii_lowercase();
            let base = if rel { cur } else { [0.; 2] };
            let pt = |lx: &mut Lexer| -> Result<[f32; 2], String> {
                Ok([lx.number()? + base[0], lx.number()? + base[1]])
            };
            let (mut next_cubic, mut next_quad) = (None, None);
            match c.to_ascii_uppercase() {
                b'M' => {
                    cur = pt(&mut lx)?;
                    start = cur;
                    out.push(PathCommand::Move(cur));
                    // Extra pairs after a move are implicit line-tos.
                    cmd = Some(if rel { b'l' } else { b'L' });
                }
                b'L' => {
                    cur = pt(&mut lx)?;
                    out.push(PathCommand::Line(cur));
                }
                b'H' => {
                    cur[0] = lx.number()? + if rel { cur[0] } else { 0. };
                    out.push(PathCommand::Line(cur));
                }
                b'V' => {
                    cur[1] = lx.number()? + if rel { cur[1] } else { 0. };
                    out.push(PathCommand::Line(cur));
                }
                b'C' => {
                    let (a, b, e) = (pt(&mut lx)?, pt(&mut lx)?, pt(&mut lx)?);
                    out.push(PathCommand::Cubic(a, b, e));
                    next_cubic = Some(b);
                    cur = e;
                }
                b'S' => {
                    let a = last_cubic.map_or(cur, |p| [2. * cur[0] - p[0], 2. * cur[1] - p[1]]);
                    let (b, e) = (pt(&mut lx)?, pt(&mut lx)?);
                    out.push(PathCommand::Cubic(a, b, e));
                    next_cubic = Some(b);
                    cur = e;
                }
                b'Q' => {
                    let (a, e) = (pt(&mut lx)?, pt(&mut lx)?);
                    out.push(PathCommand::Quad(a, e));
                    next_quad = Some(a);
                    cur = e;
                }
                b'T' => {
                    let a = last_quad.map_or(cur, |p| [2. * cur[0] - p[0], 2. * cur[1] - p[1]]);
                    let e = pt(&mut lx)?;
                    out.push(PathCommand::Quad(a, e));
                    next_quad = Some(a);
                    cur = e;
                }
                b'A' => {
                    let (rx, ry, rot) = (lx.number()?, lx.number()?, lx.number()?);
                    let (large, sweep) = (lx.flag()?, lx.flag()?);
                    let e = pt(&mut lx)?;
                    arc(&mut out, cur, rx, ry, rot, large, sweep, e);
                    cur = e;
                }
                b'Z' => {
                    out.push(PathCommand::Close);
                    cur = start;
                    // Z takes no arguments; a following number is an error.
                    if lx.at_number() {
                        return Err("unexpected number after Z".into());
                    }
                    cmd = None;
                }
                other => return Err(format!("unsupported path command '{}'", other as char)),
            }
            last_cubic = next_cubic;
            last_quad = next_quad;
            if out.len() > 100_000 {
                return Err("path has too many segments".into());
            }
            // A command letter must follow unless numbers repeat the command.
            if cmd.is_some() && !lx.at_number() {
                lx.skip();
                if lx.i < lx.s.len() && !lx.s[lx.i].is_ascii_alphabetic() {
                    return Err(format!("unexpected input at byte {}", lx.i));
                }
            }
        }
        let path = VectorPath { commands: out };
        if path.is_valid() {
            Ok(path)
        } else {
            Err("path coordinates out of range".into())
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn parses_commands_relative_forms_repeats_and_arcs() {
            let p = parse("M10 10 h20 v20 H10 Z m5 5 l1 1 2 2").unwrap();
            assert_eq!(p.commands[0], PathCommand::Move([10., 10.]));
            assert_eq!(p.commands[1], PathCommand::Line([30., 10.]));
            assert_eq!(p.commands[2], PathCommand::Line([30., 30.]));
            assert_eq!(p.commands[4], PathCommand::Close);
            // After Z the pen returns to the subpath start; m is relative to it.
            assert_eq!(p.commands[5], PathCommand::Move([15., 15.]));
            assert_eq!(p.commands[7], PathCommand::Line([18., 18.]));
            // Smooth curves reflect the previous control point.
            let s = parse("M0 0 C10 0 20 10 20 20 S30 40 40 40").unwrap();
            assert_eq!(
                s.commands[2],
                PathCommand::Cubic([20., 30.], [30., 40.], [40., 40.])
            );
            // A half-circle arc ends exactly at its endpoint in two quarter cubics.
            let a = parse("M0 10 A10 10 0 0 1 20 10").unwrap();
            assert_eq!(a.commands.len(), 3);
            let PathCommand::Cubic(_, _, end) = a.commands[2] else {
                panic!()
            };
            assert_eq!(end, [20., 10.]);
            // Compact flags and exponents, as minifiers write them.
            assert!(parse("M0 0a5 5 0 1010 0l1e1,2e0z").is_ok());
        }
        #[test]
        fn rejects_malformed_data() {
            for bad in ["10 10", "M10", "M0 0 X1 2", "M0 0 Z 5", "M0 0 L1e999 0"] {
                assert!(parse(bad).is_err(), "{bad}");
            }
        }
    }
}
