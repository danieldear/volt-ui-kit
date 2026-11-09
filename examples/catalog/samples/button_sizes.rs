//! Button sizes: 28, 36 and 44 pixels tall, with matching type.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
pub struct App {
    /// The size pressed last.
    pub pressed: Option<&'static str>,
}

pub fn view(app: &App) -> Element<App> {
    let sizes = [
        ("Small", ControlSize::Small),
        ("Medium", ControlSize::Medium),
        ("Large", ControlSize::Large),
    ];
    let buttons: Vec<Element<App>> = sizes
        .into_iter()
        .map(|(label, size)| {
            button(label)
                .key(format!("size-{}", label.to_lowercase()))
                .size(size)
                .variant(ButtonVariant::Outline)
                .on_click(move |app: &mut App| app.pressed = Some(label))
                .into()
        })
        .collect();
    let status = match app.pressed {
        Some(label) => format!("You pressed {label}."),
        None => "Press any button.".to_owned(),
    };
    vstack![
        hstack(buttons).gap(12., Axis::Horizontal).align(Align::Center),
        text(status).font_size(13.).color(ColorToken::Muted),
    ]
    .gap(16., Axis::Vertical)
    .align(Align::Center)
    .into()
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Button sizes", App::default(), view)
        .size(560, 240)
        .run()
}
