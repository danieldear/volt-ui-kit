//! The landing page's live apps: whole example apps as slides of the kit's
//! own carousel, in one canvas. Each app keeps its state, callbacks and
//! palette; `map_state` points its callbacks at its part of the showcase.
use crate::{northbound, relay};
use volt_ui_kit::{ui::prelude::*, ThemeMode, UiTheme};

/// The size each app lays out at, and the whole canvas with the carousel's
/// controls underneath. The page's frame keeps this aspect ratio.
pub const WIDTH: f32 = 1040.;
pub const APP_HEIGHT: f32 = 700.;
pub const HEIGHT: f32 = 760.;
const NAMES: [&str; 2] = ["Relay", "Northbound"];

pub struct Showcase {
    pub slide: usize,
    pub relay: relay::App,
    pub northbound: northbound::App,
}
impl Default for Showcase {
    fn default() -> Self {
        Self::new()
    }
}
impl Showcase {
    pub fn new() -> Self {
        Self {
            slide: 0,
            relay: crate::gallery_relay(),
            northbound: crate::gallery_northbound(),
        }
    }
    /// The palette of the app on show; the carousel's controls follow it.
    pub fn theme(&self, system: ThemeMode) -> UiTheme {
        match self.slide {
            0 => relay::theme(&self.relay, system),
            _ => northbound::theme(&self.northbound, system),
        }
    }
}

pub fn view(app: &Showcase) -> Element<Showcase> {
    // Only the slide on show is built; the others are empty until chosen.
    let slides = (0..NAMES.len()).map(|slide| match slide {
        _ if slide != app.slide => spacer().into(),
        0 => relay::view(&app.relay).map_state(|s: &mut Showcase| &mut s.relay),
        _ => northbound::view(&app.northbound).map_state(|s: &mut Showcase| &mut s.northbound),
    });
    vstack![carousel(slides, app.slide)
        .key("showcase")
        .labels(NAMES)
        .controls_below(true)
        .height(APP_HEIGHT)
        .on_change(|s: &mut Showcase, slide| s.slide = slide)]
    .key("showcase-root")
    .align(Align::Stretch)
    .into()
}
