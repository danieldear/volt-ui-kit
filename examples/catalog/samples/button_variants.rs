//! Button variants: the same builders used by the Catalog.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
pub struct App {}

pub fn view(_app: &App) -> Element<App> {
    line(vec![
        button("Primary")
            .key("b-primary")
            .variant(ButtonVariant::Primary)
            .on_click(noop)
            .into(),
        button("Neutral").key("b-neutral").on_click(noop).into(),
        button("Soft")
            .key("b-soft")
            .variant(ButtonVariant::Soft)
            .on_click(noop)
            .into(),
        button("Outline")
            .key("b-outline")
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
        button("Ghost")
            .key("b-ghost")
            .variant(ButtonVariant::Ghost)
            .on_click(noop)
            .into(),
        button("Delete")
            .key("b-danger")
            .variant(ButtonVariant::Danger)
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
    Application::new("Button variants", App::default(), |app| {
        vstack![view(app)]
            .align(Align::Center)
            .padding(24., Edge::All)
            .fill_width()
            .fill_height()
            .into()
    })
    .size(760, 200)
    .run()
}
