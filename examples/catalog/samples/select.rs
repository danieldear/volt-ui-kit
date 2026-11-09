//! Select: choose one option from a short list. Escape or a click outside
//! closes it; arrow keys move through the options.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

const FRUITS: [&str; 5] = ["Apple", "Banana", "Blueberry", "Grapes", "Pineapple"];

#[derive(Default)]
pub struct App {
    /// Index into `FRUITS`.
    pub fruit: u8,
    /// The select is controlled: your state decides whether it is open.
    pub open: bool,
}

pub fn view(app: &App) -> Element<App> {
    let items = FRUITS
        .iter()
        .enumerate()
        .map(|(i, name)| select_item(i as u8, *name));
    vstack![
        text("Fruit").font_size(14.).font_weight(500),
        select(items)
            .key("fruit")
            .selected(app.fruit)
            .open(app.open)
            .width(220.)
            .on_select(|app: &mut App, fruit| app.fruit = fruit)
            .on_open_change(|app: &mut App, open| app.open = open),
        text(format!("You chose {}.", FRUITS[app.fruit as usize]))
            .font_size(13.)
            .color(ColorToken::Muted),
    ]
    .gap(8., Axis::Vertical)
    .into()
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Select", App::default(), view)
        .size(420, 360)
        .run()
}
