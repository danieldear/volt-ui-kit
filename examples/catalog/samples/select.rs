//! Select and combobox recipes. Run this file with `--compare` to see both.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

pub const FRUITS: [&str; 5] = ["Apple", "Banana", "Blueberry", "Grapes", "Pineapple"];
#[derive(Default)]
pub struct App {
    pub fruit: u8,
    pub open: bool,
    pub country: Option<u8>,
    pub country_search: ComboboxState,
}

pub fn view(app: &App) -> Element<App> {
    select(
        FRUITS
            .iter()
            .enumerate()
            .map(|(i, f)| select_item(i as u8, *f)),
    )
    .key("select-fruit")
    .selected(app.fruit)
    .open(app.open)
    .width(220.)
    .on_select(|app: &mut App, v| app.fruit = v)
    .on_open_change(|app: &mut App, open| app.open = open)
    .into()
}

pub fn comparison_view(app: &App) -> Element<App> {
    let countries = [
        "Argentina",
        "Australia",
        "Austria",
        "Belgium",
        "Brazil",
        "Canada",
        "Chile",
        "Denmark",
        "Finland",
        "France",
        "Germany",
        "India",
        "Ireland",
        "Italy",
        "Japan",
        "Kenya",
        "Mexico",
        "Netherlands",
        "New Zealand",
        "Norway",
        "Portugal",
        "Singapore",
        "South Korea",
        "Spain",
        "Sweden",
        "Switzerland",
        "United Kingdom",
        "United States",
    ];
    hstack![
        vstack![
            muted("Select · 5 fruits"),
            text(FRUITS[app.fruit as usize])
                .font_size(14.)
                .font_weight(500),
            muted("(the first example above)").font_size(12.),
        ]
        .gap(6., Axis::Vertical)
        .width(220.),
        vstack![
            muted("Combobox · 28 countries"),
            combobox(
                countries.iter().enumerate().map(|(i, c)| (i as u8, *c)),
                &app.country_search,
            )
            .key("select-country")
            .selected(app.country)
            .placeholder("Search countries…")
            .width(240.)
            .state(|app: &mut App| &mut app.country_search)
            .on_select(|app: &mut App, v| app.country = Some(v)),
        ]
        .gap(6., Axis::Vertical),
    ]
    .gap(40., Axis::Horizontal)
    .into()
}
fn muted(value: impl Into<String>) -> volt_ui_kit::ui::Text<App> {
    text(value).font_size(13.).color(ColorToken::Muted)
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let compare = std::env::args().any(|arg| {
        matches!(
            arg.as_str(),
            "--compare" | "select-vs-combobox" | "select_vs_combobox"
        )
    });
    Application::new("Select and combobox", App::default(), move |app| {
        let demo = if compare {
            vstack![view(app), comparison_view(app)]
                .gap(24., Axis::Vertical)
                .into()
        } else {
            view(app)
        };
        vstack![demo]
            .align(Align::Center)
            .padding(24., Edge::All)
            .fill_width()
            .fill_height()
            .into()
    })
    .size(640, 400)
    .run()
}
