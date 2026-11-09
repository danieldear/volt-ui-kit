//! Button variants: six emphasis presets built from theme tokens.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
pub struct App {
    /// The button pressed last.
    pub pressed: Option<&'static str>,
}

pub fn view(app: &App) -> Element<App> {
    let variants = [
        ("Primary", ButtonVariant::Primary),
        ("Neutral", ButtonVariant::Neutral),
        ("Soft", ButtonVariant::Soft),
        ("Outline", ButtonVariant::Outline),
        ("Ghost", ButtonVariant::Ghost),
        ("Delete", ButtonVariant::Danger),
    ];
    let buttons: Vec<Element<App>> = variants
        .into_iter()
        .map(|(label, variant)| {
            button(label)
                .key(format!("variant-{}", label.to_lowercase()))
                .variant(variant)
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
    Application::new("Button variants", App::default(), view)
        .size(720, 240)
        .run()
}
