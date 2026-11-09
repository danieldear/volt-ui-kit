//! Disabled options: shown in the list, but skipped by keyboard and pointer.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

const ZONES: [&str; 6] = [
    "Pacific (UTC−8)",
    "Mountain (UTC−7)",
    "Central (UTC−6)",
    "Eastern (UTC−5)",
    "London (UTC+0)",
    "Tokyo (UTC+9)",
];
/// Mountain time can't be chosen in this example.
const UNAVAILABLE: u8 = 1;

#[derive(Default)]
pub struct App {
    /// Index into `ZONES`.
    pub zone: u8,
    pub open: bool,
}

pub fn view(app: &App) -> Element<App> {
    let items = ZONES.iter().enumerate().map(|(i, name)| {
        let i = i as u8;
        select_item(i, *name).enabled(i != UNAVAILABLE)
    });
    vstack![
        text("Time zone").font_size(14.).font_weight(500),
        select(items)
            .key("zone")
            .selected(app.zone)
            .open(app.open)
            .width(260.)
            .on_select(|app: &mut App, zone| app.zone = zone)
            .on_open_change(|app: &mut App, open| app.open = open),
        text("Mountain (UTC−7) is unavailable.")
            .font_size(13.)
            .color(ColorToken::Muted),
    ]
    .gap(8., Axis::Vertical)
    .into()
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Disabled options", App::default(), view)
        .size(420, 400)
        .run()
}
