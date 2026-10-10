//! Volt UI catalog: every component, documented with live previews, code,
//! variations and API tables, in a shadcn/ui-style documentation shell.
//!
//! cargo run --release --features native --example catalog
//! cargo run --release --features native --example catalog -- --page table --theme dark --accent violet
#[path = "catalog/mod.rs"]
mod catalog;
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::{
    native::{WindowBackground, WindowChrome},
    ThemeMode,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = catalog::App::new();
    let mut forwarded = vec!["--size".to_owned(), "1440x920".to_owned()];
    let mut chrome = if cfg!(target_os = "macos") {
        WindowChrome::Overlay
    } else {
        WindowChrome::Native
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--page" => {
                let slug = args.next().ok_or("missing --page slug")?;
                app.page = catalog::PAGES
                    .into_iter()
                    .find(|p| p.slug() == slug)
                    .ok_or_else(|| {
                        let all: Vec<String> = catalog::PAGES.iter().map(|p| p.slug()).collect();
                        format!("unknown page {slug}; one of: {}", all.join(", "))
                    })?;
            }
            "--accent" => {
                let name = args.next().ok_or("missing --accent name")?;
                app.theme = catalog::THEMES
                    .iter()
                    .position(|(n, ..)| n.eq_ignore_ascii_case(&name))
                    .ok_or("--accent must be zinc, blue, violet, rose, green or orange")?;
            }
            "--theme" => {
                let value = args.next().ok_or("missing --theme value")?;
                app.mode = match value.as_str() {
                    "dark" => ThemeMode::Dark,
                    "system" => ThemeMode::System,
                    _ => ThemeMode::Light,
                };
                forwarded.extend(["--theme".to_owned(), value]);
            }
            "--code" => {
                // Show the first section's code (handy for screenshots).
                let doc_first = catalog::first_section_id(&app);
                if let Some(id) = doc_first {
                    app.code.insert(id.to_owned());
                }
            }
            "--palette" => app.palette_open = true,
            "--dialog" => app.dialog_open = true,
            "--native-chrome" => chrome = WindowChrome::Native,
            _ => forwarded.push(arg),
        }
    }
    support::launch_themed_with_messages(
        "Volt UI",
        app,
        catalog::view,
        catalog::theme,
        support::RustLaunchOptions {
            chrome,
            background: WindowBackground::Opaque,
            args: forwarded,
        },
        |app, sender| app.sender = Some(sender),
        catalog::update,
    )
}
