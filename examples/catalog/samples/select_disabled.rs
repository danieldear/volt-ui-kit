//! Disabled options are skipped by pointer and keyboard.
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
#[derive(Default)]
pub struct App {
    pub zone: u8,
    pub open: bool,
}

pub fn view(app: &App) -> Element<App> {
    let tz = select(
        ZONES
            .iter()
            .enumerate()
            .map(|(i, z)| select_item(i as u8, *z).enabled(i != 1)),
    )
    .key("select-tz")
    .selected(app.zone)
    .open(app.open)
    .width(260.)
    .on_select(|app: &mut App, v| app.zone = v)
    .on_open_change(|app: &mut App, open| app.open = open);
    tz.into()
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Disabled options", App::default(), |app| {
        vstack![view(app)]
            .align(Align::Center)
            .padding(24., Edge::All)
            .fill_width()
            .fill_height()
            .into()
    })
    .size(420, 360)
    .run()
}
