//! Select or combobox? Same field look, different jobs. A select suits a
//! short, fixed list picked by click or arrow keys; a combobox suits a long
//! list you search by typing.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

const SIZES: [&str; 4] = ["Small", "Medium", "Large", "Extra large"];
const COUNTRIES: [&str; 28] = [
    "Argentina", "Australia", "Austria", "Belgium", "Brazil", "Canada", "Chile",
    "Denmark", "Finland", "France", "Germany", "India", "Ireland", "Italy", "Japan",
    "Kenya", "Mexico", "Netherlands", "New Zealand", "Norway", "Portugal",
    "Singapore", "South Korea", "Spain", "Sweden", "Switzerland", "United Kingdom",
    "United States",
];

#[derive(Default)]
pub struct App {
    /// Index into `SIZES`.
    pub size: u8,
    pub size_open: bool,
    /// Index into `COUNTRIES`, once one is chosen.
    pub country: Option<u8>,
    /// The combobox keeps its typed query and open state here.
    pub country_search: ComboboxState,
}

pub fn view(app: &App) -> Element<App> {
    let sizes = SIZES
        .iter()
        .enumerate()
        .map(|(i, name)| select_item(i as u8, *name));
    let countries = COUNTRIES
        .iter()
        .enumerate()
        .map(|(i, name)| (i as u8, *name));
    hstack![
        vstack![
            text("Select: 4 sizes").font_size(13.).color(ColorToken::Muted),
            select(sizes)
                .key("size")
                .selected(app.size)
                .open(app.size_open)
                .width(220.)
                .on_select(|app: &mut App, size| app.size = size)
                .on_open_change(|app: &mut App, open| app.size_open = open),
        ]
        .gap(6., Axis::Vertical),
        vstack![
            text("Combobox: 28 countries")
                .font_size(13.)
                .color(ColorToken::Muted),
            combobox(countries, &app.country_search)
                .key("country")
                .selected(app.country)
                .placeholder("Search countries…")
                .width(240.)
                .state(|app: &mut App| &mut app.country_search)
                .on_select(|app: &mut App, country| app.country = Some(country)),
        ]
        .gap(6., Axis::Vertical),
    ]
    .gap(40., Axis::Horizontal)
    .into()
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Select or combobox", App::default(), view)
        .size(640, 420)
        .run()
}
