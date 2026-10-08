//! Typed, opt-in appearance and time-based motion. Values use logical pixels.
use super::document::parse_color;
use super::DocumentError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    Hover,
    Pressed,
    Focus,
}
impl Interaction {
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Hover => 0,
            Self::Pressed => 1,
            Self::Focus => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    EaseOut,
    EaseInOut,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Animation {
    pub(crate) duration: u64,
    pub(crate) easing: Easing,
}
impl Animation {
    pub fn linear() -> Self {
        Self {
            duration: 150,
            easing: Easing::Linear,
        }
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
    pub(crate) fn progress(self, elapsed: f32) -> f32 {
        let t = if self.duration == 0 {
            1.
        } else {
            (elapsed / self.duration as f32).clamp(0., 1.)
        };
        match self.easing {
            Easing::Linear => t,
            Easing::EaseOut => 1. - (1. - t).powi(3),
            Easing::EaseInOut => t * t * (3. - 2. * t),
        }
    }
}
/// Fade a keyed subtree in/out. Exiting nodes are paint-only, never interactive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition(pub(crate) Animation);
impl Transition {
    pub fn fade() -> Self {
        Self(Animation::linear())
    }
    pub fn duration_ms(mut self, milliseconds: u64) -> Self {
        self.0.duration = milliseconds;
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
#[derive(Clone, Debug, Default)]
pub struct VisualStyle {
    pub(crate) color: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) border_color: Option<String>,
    pub(crate) border_width: Option<f32>,
    pub(crate) opacity: Option<f32>,
    pub(crate) shadow: Option<Shadow>,
    pub(crate) glow: Option<Glow>,
}
impl VisualStyle {
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
#[derive(Clone, Debug, Default)]
pub(crate) struct Effects {
    pub base: VisualStyle,
    pub states: [VisualStyle; 3],
    pub animations: [Option<Animation>; 3],
    pub animation: Option<Animation>,
    pub transition: Option<Transition>,
    pub drag_region: bool,
}
impl Effects {
    pub fn validate(&self, path: &str) -> Result<(), DocumentError> {
        let invalid = || DocumentError(format!("{path}: invalid effect value"));
        for style in std::iter::once(&self.base).chain(&self.states) {
            for color in [&style.color, &style.background, &style.border_color]
                .into_iter()
                .flatten()
            {
                if parse_color(color).is_none() {
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
                if parse_color(&shadow.color).is_none()
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
            if a.duration > 10_000 {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
