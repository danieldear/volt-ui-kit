//! Button states: the same builders used by the Catalog.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
pub struct App {
    pub liked: bool,
}

pub fn view(app: &App) -> Element<App> {
    line(vec![
        button("Disabled")
            .key("b-disabled")
            .variant(ButtonVariant::Primary)
            .enabled(false)
            .on_click(noop)
            .into(),
        button(if app.liked { "Liked" } else { "Like" })
            .key("b-like")
            .variant(if app.liked {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Outline
            })
            .animation(Animation::ease_out().duration_ms(180))
            .on_click(|app: &mut App| app.liked = !app.liked)
            .into(),
        button("Sort by date")
            .key("b-disclosure")
            .disclosure(false)
            .width(150.)
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
        button("Left aligned")
            .key("b-align")
            .label_align(Align::Start)
            .width(150.)
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
    Application::new("Button states", App::default(), |app| {
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
