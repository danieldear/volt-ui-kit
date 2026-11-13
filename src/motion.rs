//! Renderer-independent, clock-driven 2D motion. Coordinates are logical pixels.
//! No timers, threads, callbacks, wall clocks or platform dependencies are owned here.
use crate::declarative::{Animation, Easing};
use std::time::Duration;

/// An affine mapping in physical coordinates. Composition is parent * child.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2D(pub [f64; 6]);
impl Default for Affine2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}
impl Affine2D {
    pub const IDENTITY: Self = Self([1., 0., 0., 1., 0., 0.]);
    pub fn point(self, p: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d, x, y] = self.0;
        let [px, py] = p.map(f64::from);
        [(a * px + c * py + x) as f32, (b * px + d * py + y) as f32]
    }
    pub fn then(self, parent: Self) -> Self {
        let [a, b, c, d, x, y] = self.0;
        let [e, f, g, h, u, v] = parent.0;
        Self([
            e * a + g * b,
            f * a + h * b,
            e * c + g * d,
            f * c + h * d,
            e * x + g * y + u,
            f * x + h * y + v,
        ])
    }
    pub fn inverse(self) -> Option<Self> {
        let [a, b, c, d, x, y] = self.0;
        let det = a * d - b * c;
        if !self.0.iter().all(|v| v.is_finite()) || !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        Some(Self([
            d / det,
            -b / det,
            -c / det,
            a / det,
            (c * y - d * x) / det,
            (b * x - a * y) / det,
        ]))
    }
    pub fn max_scale(self) -> f64 {
        let [a, b, c, d, _, _] = self.0;
        let q = a * a + b * b + c * c + d * d;
        let det = a * d - b * c;
        ((q + (q * q - 4. * det * det).max(0.).sqrt()) * 0.5).sqrt()
    }
    pub fn bounds(self, [x, y, w, h]: [f32; 4]) -> [f32; 4] {
        let pts = [[x, y], [x + w, y], [x, y + h], [x + w, y + h]].map(|p| self.point(p));
        let min = pts
            .iter()
            .fold([f32::INFINITY; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
        let max = pts.iter().fold([f32::NEG_INFINITY; 2], |a, p| {
            [a[0].max(p[0]), a[1].max(p[1])]
        });
        [min[0], min[1], max[0] - min[0], max[1] - min[1]]
    }
}
/// Translation, rotation and scale around a fractional box origin (centre by default).
/// Negative scales mirror; a zero scale paints no area and cannot receive input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    pub(crate) translation: [f32; 2],
    pub(crate) scale: [f32; 2],
    pub(crate) rotation: f32,
    pub(crate) origin: [f32; 2],
}
impl Default for Transform2D {
    fn default() -> Self {
        Self::new()
    }
}
impl Transform2D {
    pub const fn new() -> Self {
        Self {
            translation: [0.; 2],
            scale: [1.; 2],
            rotation: 0.,
            origin: [0.5; 2],
        }
    }
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translation = [x, y];
        self
    }
    pub fn scale(mut self, x: f32, y: f32) -> Self {
        self.scale = [x, y];
        self
    }
    pub fn rotate(mut self, degrees: f32) -> Self {
        self.rotation = degrees;
        self
    }
    pub fn origin(mut self, x: f32, y: f32) -> Self {
        self.origin = [x, y];
        self
    }
    pub fn is_valid(self) -> bool {
        self.translation
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 16384.)
            && self.scale.iter().all(|v| v.is_finite() && v.abs() <= 16.)
            && self.rotation.is_finite()
            && self.rotation.abs() <= 360_000.
            && self
                .origin
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v))
    }
    /// `bounds` are logical coordinates; multiply by the DPI scale only here.
    pub fn matrix(self, [x, y, w, h]: [f32; 4], dpi: f32) -> Affine2D {
        let [sx, sy] = self.scale.map(f64::from);
        let (s, c) = f64::from(self.rotation).to_radians().sin_cos();
        let ox = f64::from((x + w * self.origin[0]) * dpi);
        let oy = f64::from((y + h * self.origin[1]) * dpi);
        Affine2D([
            c * sx,
            s * sx,
            -s * sy,
            c * sy,
            ox + f64::from(self.translation[0] * dpi) - c * sx * ox + s * sy * oy,
            oy + f64::from(self.translation[1] * dpi) - s * sx * ox - c * sy * oy,
        ])
    }
    pub fn interpolate(self, to: Self, t: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self {
            translation: std::array::from_fn(|i| mix(self.translation[i], to.translation[i])),
            scale: std::array::from_fn(|i| mix(self.scale[i], to.scale[i])),
            rotation: mix(self.rotation, to.rotation),
            origin: std::array::from_fn(|i| mix(self.origin[i], to.origin[i])),
        }
    }
}
/// A timeline pose. Translation is logical; rotation is degrees, not radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionPose {
    pub translation: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub opacity: f32,
}
impl Default for MotionPose {
    fn default() -> Self {
        Self::new()
    }
}
impl MotionPose {
    pub const fn new() -> Self {
        Self {
            translation: [0.; 2],
            scale: [1.; 2],
            rotation: 0.,
            opacity: 1.,
        }
    }
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translation = [x, y];
        self
    }
    pub fn scale(mut self, x: f32, y: f32) -> Self {
        self.scale = [x, y];
        self
    }
    pub fn rotate(mut self, degrees: f32) -> Self {
        self.rotation = degrees;
        self
    }
    pub fn opacity(mut self, value: f32) -> Self {
        self.opacity = value;
        self
    }
    pub fn transform(self) -> Transform2D {
        Transform2D::new()
            .translate(self.translation[0], self.translation[1])
            .scale(self.scale[0], self.scale[1])
            .rotate(self.rotation)
    }
    fn valid(self) -> bool {
        self.transform().is_valid()
            && self.opacity.is_finite()
            && (0. ..=1.).contains(&self.opacity)
    }
    fn mix(self, to: Self, t: f32) -> Self {
        let m = |a: f32, b: f32| a + (b - a) * t;
        Self {
            translation: std::array::from_fn(|i| m(self.translation[i], to.translation[i])),
            scale: std::array::from_fn(|i| m(self.scale[i], to.scale[i])),
            rotation: m(self.rotation, to.rotation),
            opacity: m(self.opacity, to.opacity).clamp(0., 1.),
        }
    }
}
/// Declarative timeline controls; seeking also pauses at the selected position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TimelinePlayback {
    #[default]
    Play,
    Paused,
    Reverse,
    Seek(f32),
}
/// A bounded sequence of poses. Each keyframe position is a fraction in 0..=1.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub(crate) playback: TimelinePlayback,
    revision: u64,
    duration: Duration,
    frames: Vec<(f32, MotionPose)>,
    easing: Easing,
}
impl Timeline {
    pub fn playback(mut self, value: TimelinePlayback) -> Self {
        self.playback = value;
        self
    }
    pub fn revision(mut self, value: u64) -> Self {
        self.revision = value;
        self
    }
    pub(crate) fn same_sequence(&self, other: &Self) -> bool {
        self.duration == other.duration
            && self.frames == other.frames
            && self.easing == other.easing
            && self.revision == other.revision
    }

    pub fn new(milliseconds: u64) -> Self {
        Self {
            duration: Duration::from_millis(milliseconds),
            playback: TimelinePlayback::Play,
            revision: 0,
            frames: vec![],
            easing: Easing::Linear,
        }
    }
    pub fn keyframe(mut self, at: f32, pose: MotionPose) -> Self {
        self.frames.push((at, pose));
        self
    }
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
    pub fn duration(&self) -> Duration {
        self.duration
    }
    pub fn is_valid(&self) -> bool {
        (!matches!(self.playback,TimelinePlayback::Seek(p) if !p.is_finite()||!(0. ..=1.).contains(&p)))
            && self.duration <= Duration::from_secs(60)
            && (2..=128).contains(&self.frames.len())
            && self.frames.first().is_some_and(|f| f.0 == 0.)
            && self.frames.last().is_some_and(|f| f.0 == 1.)
            && self
                .frames
                .iter()
                .all(|(t, p)| t.is_finite() && (0. ..=1.).contains(t) && p.valid())
            && self.frames.windows(2).all(|w| w[0].0 < w[1].0)
            && Animation::linear().easing(self.easing).valid()
    }
    pub fn sample(&self, position: f32) -> Option<MotionPose> {
        if !self.is_valid() || !position.is_finite() {
            return None;
        }
        let p = position.clamp(0., 1.);
        let pair = self.frames.windows(2).find(|w| p <= w[1].0)?;
        let t = (p - pair[0].0) / (pair[1].0 - pair[0].0);
        let t = Animation::linear()
            .duration_ms(1000)
            .easing(self.easing)
            .progress(t * 1000.);
        Some(pair[0].1.mix(pair[1].1, t))
    }
}
/// A caller-clocked playhead. Pause/reverse preserve the current position.
#[derive(Clone, Debug)]
pub struct TimelinePlayer {
    timeline: Timeline,
    position: f64,
    direction: f64,
    last: Duration,
    playing: bool,
}
impl TimelinePlayer {
    pub fn new(timeline: Timeline) -> Result<Self, &'static str> {
        if !timeline.is_valid() {
            return Err("invalid timeline");
        }
        Ok(Self {
            timeline,
            position: 0.,
            direction: 1.,
            last: Duration::ZERO,
            playing: false,
        })
    }
    fn advance(&mut self, now: Duration) {
        if self.playing {
            let duration = self.timeline.duration.as_secs_f64();
            self.position = if duration == 0. {
                if self.direction > 0. {
                    1.
                } else {
                    0.
                }
            } else {
                (self.position
                    + now.saturating_sub(self.last).as_secs_f64() * self.direction / duration)
                    .clamp(0., 1.)
            };
            if (self.direction > 0. && self.position >= 1.)
                || (self.direction < 0. && self.position <= 0.)
            {
                self.playing = false;
            }
        }
        self.last = self.last.max(now);
    }
    pub fn play(&mut self, now: Duration) {
        self.advance(now);
        self.direction = 1.;
        self.playing = self.position < 1.;
    }
    pub fn reverse(&mut self, now: Duration) {
        self.advance(now);
        self.direction = -1.;
        self.playing = self.position > 0.;
    }
    pub fn pause(&mut self, now: Duration) {
        self.advance(now);
        self.playing = false;
    }
    pub fn seek(&mut self, position: f32) {
        if position.is_finite() {
            self.position = f64::from(position.clamp(0., 1.));
            self.playing = false;
        }
    }
    pub fn sample(&mut self, now: Duration) -> MotionPose {
        self.advance(now);
        self.timeline
            .sample(self.position as f32)
            .expect("validated timeline")
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn position(&self) -> f32 {
        self.position as f32
    }
    pub fn finish(&mut self) {
        self.position = if self.direction > 0. { 1. } else { 0. };
        self.playing = false;
    }
}
/// Bounded exponential inertia, sampled analytically rather than integrated per frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inertia {
    min: [f32; 2],
    max: [f32; 2],
    friction: f32,
}
impl Default for Inertia {
    fn default() -> Self {
        Self::new()
    }
}
impl Inertia {
    pub fn new() -> Self {
        Self {
            min: [-16384.; 2],
            max: [16384.; 2],
            friction: 6.,
        }
    }
    pub fn bounds(mut self, min: [f32; 2], max: [f32; 2]) -> Self {
        self.min = min;
        self.max = max;
        self
    }
    pub fn friction(mut self, value: f32) -> Self {
        self.friction = value;
        self
    }
    pub fn release(
        self,
        position: [f32; 2],
        velocity: [f32; 2],
    ) -> Result<InertiaPlayer, &'static str> {
        if !self.friction.is_finite()
            || !(0.1..=100.).contains(&self.friction)
            || !(0..2).all(|i| {
                self.min[i].is_finite()
                    && self.min[i].abs() <= 16384.
                    && self.max[i].is_finite()
                    && self.max[i].abs() <= 16384.
                    && self.min[i] <= self.max[i]
                    && position[i].is_finite()
                    && velocity[i].is_finite()
                    && velocity[i].abs() <= 100_000.
            })
        {
            return Err("invalid inertia");
        }
        Ok(InertiaPlayer {
            config: self,
            origin: std::array::from_fn(|i| position[i].clamp(self.min[i], self.max[i])),
            velocity,
            start: Duration::ZERO,
            active: true,
        })
    }
}
#[derive(Clone, Debug)]
pub struct InertiaPlayer {
    config: Inertia,
    origin: [f32; 2],
    velocity: [f32; 2],
    start: Duration,
    active: bool,
}
impl InertiaPlayer {
    pub fn at(mut self, start: Duration) -> Self {
        self.start = start;
        self
    }
    pub fn sample(&mut self, now: Duration) -> [f32; 2] {
        let t = now.saturating_sub(self.start).as_secs_f64();
        let f = f64::from(self.config.friction);
        let decay = (-f * t).exp();
        let result: [f32; 2] = std::array::from_fn(|i| {
            (f64::from(self.origin[i]) + f64::from(self.velocity[i]) * (1. - decay) / f) as f32
        });
        self.active = (0..2).any(|i| {
            f64::from(self.velocity[i]).abs() * decay > 0.5
                && result[i] > self.config.min[i]
                && result[i] < self.config.max[i]
        });
        std::array::from_fn(|i| result[i].clamp(self.config.min[i], self.config.max[i]))
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
}
/// Pointer movement sampler for application-owned drags. Superseded timestamps
/// are ignored; stale release samples never launch inertia from an old flick.
#[derive(Clone, Debug, Default)]
pub struct DragGesture {
    last: Option<(Duration, [f32; 2])>,
    velocity: [f32; 2],
}
impl DragGesture {
    pub fn begin(&mut self, now: Duration, position: [f32; 2]) {
        self.last = Some((now, position));
        self.velocity = [0.; 2];
    }
    pub fn update(&mut self, now: Duration, position: [f32; 2]) {
        if !position.iter().all(|v| v.is_finite()) {
            return;
        }
        if let Some((last, p)) = self.last {
            if now <= last {
                return;
            }
            let dt = now.saturating_sub(last).as_secs_f32();
            self.velocity = if dt > 0.1 {
                [0.; 2]
            } else {
                std::array::from_fn(|i| ((position[i] - p[i]) / dt).clamp(-100_000., 100_000.))
            };
        }
        self.last = Some((now, position));
    }
    pub fn release(&mut self, now: Duration) -> [f32; 2] {
        let v = if self
            .last
            .is_some_and(|(t, _)| now >= t && now - t <= Duration::from_millis(100))
        {
            self.velocity
        } else {
            [0.; 2]
        };
        self.last = None;
        self.velocity = [0.; 2];
        v
    }
}

/// Declarative drag policy. Bounds are offsets in the element's logical axes.
/// Direct manipulation remains available under reduced motion; momentum does not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragMotion {
    pub(crate) inertia: Inertia,
    pub(crate) momentum: bool,
}
impl Default for DragMotion {
    fn default() -> Self {
        Self::new()
    }
}
impl DragMotion {
    pub fn new() -> Self {
        Self {
            inertia: Inertia::new(),
            momentum: true,
        }
    }
    pub fn bounds(mut self, min: [f32; 2], max: [f32; 2]) -> Self {
        self.inertia = self.inertia.bounds(min, max);
        self
    }
    pub fn momentum(mut self, value: bool) -> Self {
        self.momentum = value;
        self
    }
    pub fn friction(mut self, value: f32) -> Self {
        self.inertia = self.inertia.friction(value);
        self
    }
    pub(crate) fn clamp(self, p: [f32; 2]) -> [f32; 2] {
        std::array::from_fn(|i| p[i].clamp(self.inertia.min[i], self.inertia.max[i]))
    }
    pub(crate) fn valid(self) -> bool {
        self.inertia.release([0.; 2], [0.; 2]).is_ok()
    }
}

/// Decorative text presentation. Semantic/copy text always remains the full value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextEffect {
    pub(crate) kind: TextEffectKind,
    pub(crate) animation: Animation,
    pub(crate) revision: u64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TextEffectKind {
    Typing,
    Morph,
    Rolling,
    Shimmer,
}
impl TextEffect {
    fn new(kind: TextEffectKind) -> Self {
        Self {
            kind,
            animation: Animation::ease_out().duration_ms(420),
            revision: 0,
        }
    }
    pub fn typing() -> Self {
        Self {
            animation: Animation::linear().duration_ms(1200),
            ..Self::new(TextEffectKind::Typing)
        }
    }
    pub fn morph() -> Self {
        Self::new(TextEffectKind::Morph)
    }
    pub fn rolling() -> Self {
        Self::new(TextEffectKind::Rolling)
    }
    pub fn shimmer() -> Self {
        Self {
            animation: Animation::linear().duration_ms(1800),
            ..Self::new(TextEffectKind::Shimmer)
        }
    }
    pub fn animation(mut self, value: Animation) -> Self {
        self.animation = value;
        self
    }
    pub fn duration_ms(mut self, value: u64) -> Self {
        self.animation = self.animation.duration_ms(value);
        self
    }
    pub fn revision(mut self, value: u64) -> Self {
        self.revision = value;
        self
    }
    pub(crate) fn valid(self) -> bool {
        self.animation.valid()
            && (self.kind != TextEffectKind::Shimmer || self.animation.duration >= 200)
    }
}
