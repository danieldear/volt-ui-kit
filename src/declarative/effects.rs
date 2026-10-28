//! Typed, opt-in appearance and time-based motion. Values use logical pixels.
use super::DocumentError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    Hover,
    Pressed,
    Focus,
}
impl Interaction {
    #[cfg(feature = "rust-ui")]
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Hover => 0,
            Self::Pressed => 1,
            Self::Focus => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// CSS `cubic-bezier(x1, y1, x2, y2)`; x values are clamped to 0..=1.
    CubicBezier(f32, f32, f32, f32),
}
/// Solve a CSS cubic-bezier timing curve: find the parameter whose x is `t`,
/// then return its y.
fn cubic_bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let (x1, x2) = (x1.clamp(0., 1.), x2.clamp(0., 1.));
    let curve = |u: f32, a: f32, b: f32| {
        let v = 1. - u;
        3. * v * v * u * a + 3. * v * u * u * b + u * u * u
    };
    let (mut lo, mut hi) = (0_f32, 1_f32);
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        if curve(mid, x1, x2) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    curve((lo + hi) * 0.5, y1, y2)
}
/// A normalized damped-spring timing curve. Duration controls settling time;
/// damping ratio controls bounce (0.4..=2, default 0.8). Retargeting preserves
/// the current painted position, not physical velocity. No background task.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    damping_ratio: f32,
}
impl Default for Spring {
    fn default() -> Self {
        Self::new()
    }
}
impl Spring {
    pub fn new() -> Self {
        Self { damping_ratio: 0.8 }
    }
    pub fn damping_ratio(mut self, ratio: f32) -> Self {
        self.damping_ratio = ratio;
        self
    }
    fn valid(self) -> bool {
        self.damping_ratio.is_finite() && (0.4..=2.).contains(&self.damping_ratio)
    }
    fn sample(self, t: f32) -> f32 {
        let z = self.damping_ratio;
        if (z - 1.).abs() < 0.001 {
            return 1. - (1. + 10. * t) * (-10. * t).exp();
        }
        if z < 1. {
            let w = 10. / z;
            let d = (1. - z * z).sqrt();
            let angle = w * d * t;
            1. - (-10. * t).exp() * (angle.cos() + z / d * angle.sin())
        } else {
            let d = (z * z - 1.).sqrt();
            let a = z - d;
            let b = z + d;
            let w = 10. / a;
            1. - (b * (-a * w * t).exp() - a * (-b * w * t).exp()) / (b - a)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animation {
    pub(crate) duration: u64,
    pub(crate) easing: Easing,
    pub(crate) delay: u64,
    spring: Option<Spring>,
}
impl Animation {
    pub fn linear() -> Self {
        Self {
            duration: 150,
            easing: Easing::Linear,
            delay: 0,
            spring: None,
        }
    }
    /// A subtle, finite damped spring. Only positional offsets overshoot;
    /// opacity, colors and border widths stay within their legal ranges.
    pub fn spring() -> Self {
        Self::spring_with(Spring::new())
    }
    pub fn spring_with(spring: Spring) -> Self {
        Self {
            duration: 420,
            spring: Some(spring),
            ..Self::linear()
        }
    }
    pub fn ease_in() -> Self {
        Self {
            easing: Easing::EaseIn,
            ..Self::linear()
        }
    }
    /// Any curve, e.g. `Easing::CubicBezier(0.16, 1., 0.3, 1.)`.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self.spring = None;
        self
    }
    /// Wait before starting; stagger list items by giving each a larger delay.
    pub fn delay_ms(mut self, milliseconds: u64) -> Self {
        self.delay = milliseconds;
        self
    }
    /// Add `index * interval` to the configured delay. Overflow is retained as
    /// an invalid value and rejected at build, never wrapped into a short delay.
    pub fn stagger(mut self, index: usize, interval_ms: u64) -> Self {
        self.delay = self
            .delay
            .saturating_add((index as u64).saturating_mul(interval_ms));
        self
    }
    pub fn ease_out() -> Self {
        Self {
            easing: Easing::EaseOut,
            ..Self::linear()
        }
    }
    pub fn ease_in_out() -> Self {
        Self {
            easing: Easing::EaseInOut,
            ..Self::linear()
        }
    }
    pub fn duration_ms(mut self, milliseconds: u64) -> Self {
        self.duration = milliseconds;
        self
    }
    pub(crate) fn wait(self, elapsed: std::time::Duration) -> Option<std::time::Duration> {
        let end = std::time::Duration::from_millis(self.delay.saturating_add(self.duration));
        if elapsed >= end {
            None
        } else {
            Some(std::time::Duration::from_millis(self.delay).saturating_sub(elapsed))
        }
    }
    pub(crate) fn finished(self, elapsed: f32) -> bool {
        elapsed >= self.delay.saturating_add(self.duration) as f32
    }
    pub(crate) fn progress(self, elapsed: f32) -> f32 {
        if self.finished(elapsed) {
            return 1.;
        }
        let elapsed = elapsed - self.delay as f32;
        let t = if self.duration == 0 {
            if elapsed >= 0. {
                1.
            } else {
                0.
            }
        } else {
            (elapsed / self.duration as f32).clamp(0., 1.)
        };
        if t <= 0. {
            return 0.;
        }
        if t >= 1. {
            return 1.;
        }
        if let Some(spring) = self.spring {
            return spring.sample(t);
        }
        match self.easing {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1. - (1. - t).powi(3),
            Easing::EaseInOut => t * t * (3. - 2. * t),
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(t, x1, y1, x2, y2),
        }
    }
    pub(crate) fn valid(&self) -> bool {
        self.duration <= 60_000
            && self.delay <= 60_000
            && self.spring.is_none_or(Spring::valid)
            && match self.easing {
                Easing::CubicBezier(a, b, c, d) => [a, b, c, d].iter().all(|v| v.is_finite()),
                _ => true,
            }
    }
}
/// Animate a keyed subtree in and out: a fade, optionally sliding from an
/// offset. Exiting nodes are paint-only, never interactive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition(pub(crate) Animation, pub(crate) [f32; 2]);
impl Transition {
    pub fn fade() -> Self {
        Self(Animation::linear(), [0.; 2])
    }
    /// Fade in while sliding from (`x`, `y`) logical pixels to place; exit
    /// reverses it. `slide(0., 12.)` rises into view.
    pub fn slide(x: f32, y: f32) -> Self {
        Self(Animation::ease_out().duration_ms(220), [x, y])
    }
    pub fn duration_ms(mut self, milliseconds: u64) -> Self {
        self.0.duration = milliseconds;
        self
    }
    pub fn easing(mut self, easing: Easing) -> Self {
        self.0 = self.0.easing(easing);
        self
    }
    /// Use the same timing configuration as state animations, including springs.
    pub fn animation(mut self, animation: Animation) -> Self {
        self.0 = animation;
        self
    }
    pub fn stagger(mut self, index: usize, interval_ms: u64) -> Self {
        self.0 = self.0.stagger(index, interval_ms);
        self
    }
    pub fn delay_ms(mut self, milliseconds: u64) -> Self {
        self.0.delay = milliseconds;
        self
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub(crate) color: String,
    pub(crate) blur: f32,
    pub(crate) spread: f32,
    pub(crate) offset: [f32; 2],
}
impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: "#00000040".into(),
            blur: 8.,
            spread: 0.,
            offset: [0., 2.],
        }
    }
}
impl Shadow {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = color.into();
        self
    }
    pub fn blur(mut self, radius: f32) -> Self {
        self.blur = radius;
        self
    }
    pub fn spread(mut self, pixels: f32) -> Self {
        self.spread = pixels;
        self
    }
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = [x, y];
        self
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Glow(pub(crate) Shadow);
impl Default for Glow {
    fn default() -> Self {
        Self(Shadow::new().color("#60a5fa80").offset(0., 0.))
    }
}
impl Glow {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.0.color = color.into();
        self
    }
    pub fn blur(mut self, radius: f32) -> Self {
        self.0.blur = radius;
        self
    }
    pub fn spread(mut self, pixels: f32) -> Self {
        self.0.spread = pixels;
        self
    }
}
/// A declarative interaction patch; its closure executes once while building.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisualStyle {
    pub(crate) color: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) border_color: Option<String>,
    pub(crate) border_width: Option<f32>,
    pub(crate) opacity: Option<f32>,
    pub(crate) shadow: Option<Shadow>,
    pub(crate) glow: Option<Glow>,
    pub(crate) offset: Option<[f32; 2]>,
}
impl VisualStyle {
    /// Translate the element (logical px) in this state, e.g. a hover lift
    /// `offset(0., -2.)`; animates with the state's animation.
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = Some([x, y]);
        self
    }
    pub fn color(mut self, value: impl Into<String>) -> Self {
        self.color = Some(value.into());
        self
    }
    pub fn background(mut self, value: impl Into<String>) -> Self {
        self.background = Some(value.into());
        self
    }
    pub fn border_color(mut self, value: impl Into<String>) -> Self {
        self.border_color = Some(value.into());
        self
    }
    pub fn border_width(mut self, value: f32) -> Self {
        self.border_width = Some(value);
        self
    }
    pub fn opacity(mut self, value: f32) -> Self {
        self.opacity = Some(value);
        self
    }
    pub fn shadow(mut self, value: Shadow) -> Self {
        self.shadow = Some(value);
        self
    }
    pub fn glow(mut self, value: Glow) -> Self {
        self.glow = Some(value);
        self
    }
}
/// Continuous, decorative paint-time motion: a subtree that drifts on an
/// ellipse, or a canvas that spins about its centre. It never relayouts,
/// recomposes or moves hit targets, and stops under reduced motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    pub(crate) kind: AmbientKind,
    pub(crate) period_ms: u32,
    pub(crate) phase: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AmbientKind {
    Float([f32; 2]),
    Spin,
    /// Opacity breathes between this minimum and 1.
    Pulse(f32),
}
impl Ambient {
    /// Drift up to `x`/`y` logical pixels either side of the laid-out place.
    pub fn float(x: f32, y: f32) -> Self {
        Self {
            kind: AmbientKind::Float([x, y]),
            period_ms: 8000,
            phase: 0.,
        }
    }
    /// Rotate a canvas's drawing once per period (other content is unrotated).
    pub fn spin() -> Self {
        Self {
            kind: AmbientKind::Spin,
            period_ms: 30_000,
            phase: 0.,
        }
    }
    /// Breathe the subtree's opacity between `min` and 1 (skeleton loaders).
    pub fn pulse(min: f32) -> Self {
        Self {
            kind: AmbientKind::Pulse(min),
            period_ms: 1600,
            phase: 0.,
        }
    }
    pub fn period_ms(mut self, milliseconds: u32) -> Self {
        self.period_ms = milliseconds;
        self
    }
    /// Starting point in the cycle, 0..1, so siblings do not move in step.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }
    pub(crate) fn valid(&self) -> bool {
        (200..=600_000).contains(&self.period_ms)
            && self.phase.is_finite()
            && match self.kind {
                AmbientKind::Float(a) => a.iter().all(|v| v.is_finite() && v.abs() <= 256.),
                AmbientKind::Spin => true,
                AmbientKind::Pulse(min) => min.is_finite() && (0. ..=1.).contains(&min),
            }
    }
    /// Cycle position in turns at `now`.
    pub(crate) fn turns(&self, now: std::time::Duration) -> f32 {
        let cycle = (now.as_millis() % u128::from(self.period_ms)) as f32 / self.period_ms as f32;
        (cycle + self.phase).rem_euclid(1.)
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Effects {
    pub scroll_passthrough: bool,
    pub base: VisualStyle,
    pub states: [VisualStyle; 3],
    /// Applied while the element is disabled.
    pub disabled: VisualStyle,
    pub animations: [Option<Animation>; 3],
    pub animation: Option<Animation>,
    pub transition: Option<Transition>,
    pub transition_revision: u64,
    pub ambient: Option<Ambient>,
    pub drag_region: bool,
}
impl Effects {
    pub fn validate(&self, path: &str) -> Result<(), DocumentError> {
        let invalid = || DocumentError(format!("{path}: invalid effect value"));
        if self.ambient.is_some_and(|a| !a.valid()) {
            return Err(invalid());
        }
        for style in std::iter::once(&self.base)
            .chain(&self.states)
            .chain(std::iter::once(&self.disabled))
        {
            for color in [&style.color, &style.background, &style.border_color]
                .into_iter()
                .flatten()
            {
                // Hex, a token, or a "light|dark" pair, as static styles take.
                if !crate::theme::is_color_value(color) {
                    return Err(invalid());
                }
            }
            if style
                .opacity
                .is_some_and(|v| !v.is_finite() || !(0. ..=1.).contains(&v))
                || style
                    .border_width
                    .is_some_and(|v| !v.is_finite() || !(0. ..=16.).contains(&v))
            {
                return Err(invalid());
            }
            for shadow in style.shadow.iter().chain(style.glow.iter().map(|v| &v.0)) {
                if !crate::theme::is_color_value(&shadow.color)
                    || !shadow.blur.is_finite()
                    || !(0. ..=64.).contains(&shadow.blur)
                    || !shadow.spread.is_finite()
                    || !(0. ..=32.).contains(&shadow.spread)
                    || shadow
                        .offset
                        .iter()
                        .any(|v| !v.is_finite() || !(-128. ..=128.).contains(v))
                {
                    return Err(invalid());
                }
            }
        }
        for a in self
            .animations
            .iter()
            .flatten()
            .copied()
            .chain(self.animation)
            .chain(self.transition.map(|v| v.0))
        {
            if a.duration > 10_000 || !a.valid() {
                return Err(invalid());
            }
        }
        let offset_ok = |o: &[f32; 2]| o.iter().all(|v| v.is_finite() && v.abs() <= 512.);
        if std::iter::once(&self.base)
            .chain(&self.states)
            .chain(std::iter::once(&self.disabled))
            .any(|s| s.offset.as_ref().is_some_and(|o| !offset_ok(o)))
            || self.transition.is_some_and(|t| !offset_ok(&t.1))
        {
            return Err(invalid());
        }
        Ok(())
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    #[test]
    fn spring_response_is_bounded_finite_and_finishes_exactly() {
        let animation = Animation::spring().duration_ms(500).delay_ms(80);
        assert_eq!(animation.progress(0.), 0.);
        assert_eq!(animation.progress(80.), 0.);
        assert_eq!(animation.progress(580.), 1.);
        assert_eq!(animation.progress(1000.), 1.);
        assert!(!animation.finished(579.));
        assert!(animation.finished(580.));
        let values: Vec<_> = (0..=500)
            .map(|t| animation.progress(t as f32 + 80.))
            .collect();
        assert!(values.iter().all(|v| v.is_finite() && *v >= 0. && *v < 1.4));
        assert!(values.iter().any(|v| *v > 1.));
        assert!(
            (animation.progress(200.)
                - Animation::spring_with(Spring::new().damping_ratio(1.))
                    .duration_ms(500)
                    .delay_ms(80)
                    .progress(200.))
            .abs()
                > 0.01
        );
        assert_eq!(animation.easing(Easing::Linear).progress(330.), 0.5);
    }
    #[test]
    fn stagger_preserves_base_delay_and_overflow_is_invalid() {
        let a = Animation::linear()
            .duration_ms(100)
            .delay_ms(20)
            .stagger(3, 40);
        assert_eq!(a.delay, 140);
        assert_eq!(a.progress(140.), 0.);
        assert!(!a.finished(239.));
        assert!(a.finished(240.));
        assert!(!Animation::linear().stagger(usize::MAX, u64::MAX).valid());
        assert_eq!(
            Transition::slide(0., 12.)
                .delay_ms(20)
                .stagger(3, 40)
                .0
                .delay,
            140
        );
        for damping in [f32::NAN, f32::INFINITY, 0., -1., 3.] {
            assert!(!Animation::spring_with(Spring::new().damping_ratio(damping)).valid());
        }
    }
}
