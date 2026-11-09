//! Button states: disabled, a toggle that animates between variants, a
//! disclosure chevron and a start-aligned label.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
pub struct App {
    pub liked: bool,
    /// Whether the "Sort by date" menu would be open.
    pub sort_open: bool,
    pub aligned_presses: u32,
}

pub fn view(app: &App) -> Element<App> {
    hstack![
        button("Disabled")
            .key("state-disabled")
            .variant(ButtonVariant::Primary)
            .enabled(false)
            .on_click(|_: &mut App| {}),
        // The variant changes with state; the animation eases between them.
        button(if app.liked { "Liked" } else { "Like" })
            .key("state-like")
            .variant(if app.liked {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Outline
            })
            .animation(Animation::ease_out().duration_ms(180))
            .on_click(|app: &mut App| app.liked = !app.liked),
        // A trailing chevron for buttons that open a menu or popup.
        button("Sort by date")
            .key("state-disclosure")
            .disclosure(app.sort_open)
            .width(150.)
            .variant(ButtonVariant::Outline)
            .on_click(|app: &mut App| app.sort_open = !app.sort_open),
        button(match app.aligned_presses {
            0 => "Left aligned".to_owned(),
            n => format!("Pressed {n}×"),
        })
        .key("state-align")
        .label_align(Align::Start)
        .width(150.)
        .on_click(|app: &mut App| app.aligned_presses += 1),
    ]
    .gap(12., Axis::Horizontal)
    .align(Align::Center)
    .into()
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Button states", App::default(), view)
        .size(720, 200)
        .run()
}
