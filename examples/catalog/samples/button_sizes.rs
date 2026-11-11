//! Button sizes: the same builders used by the Catalog.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
pub struct App {}

pub fn view(_app: &App) -> Element<App> {
    line(vec![
        button("Small")
            .key("b-small")
            .size(ControlSize::Small)
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
        button("Medium")
            .key("b-medium")
            .size(ControlSize::Medium)
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
        button("Large")
            .key("b-large")
            .size(ControlSize::Large)
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
    ])
}

fn line(items: Vec<Element<App>>) -> Element<App> {
    hstack(items)
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .into()
}
fn noop(_: &mut App) {}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Button sizes", App::default(), |app| {
        vstack![view(app)]
            .align(Align::Center)
            .padding(24., Edge::All)
            .fill_width()
            .fill_height()
            .into()
    })
    .size(560, 200)
    .run()
}
