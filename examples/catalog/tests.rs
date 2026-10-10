use super::*;
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    Viewport,
};

fn mount(app: &App, size: [f32; 2]) -> Runtime {
    let (document, handlers) = view(app)
        .build()
        .unwrap_or_else(|e| panic!("{:?}: {e}", app.page))
        .into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(size[0], size[1], 1.).unwrap();
    runtime
        .layout(
            viewport,
            &Bindings::default(),
            &mut RasterPainter::new(viewport),
        )
        .unwrap();
    runtime
}

#[test]
fn every_page_builds_in_both_modes_with_code_and_overlays() {
    let mut app = App::new();
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        app.mode = mode;
        for page in PAGES {
            app.go(page);
            mount(&app, [1440., 920.]);
            mount(&app, [900., 920.]);
            // Every example also renders its source view.
            for section in pages::doc(page, &app).sections {
                app.code.insert(section.id.to_owned());
            }
            mount(&app, [1440., 920.]);
        }
    }
    app.palette_open = true;
    app.dialog_open = true;
    app.fruit_open = true;
    mount(&app, [1440., 920.]);
}

#[test]
fn slugs_are_unique_and_every_theme_is_valid() {
    let mut slugs: Vec<String> = PAGES.iter().map(|p| p.slug()).collect();
    slugs.sort();
    slugs.dedup();
    assert_eq!(slugs.len(), PAGES.len());
    let mut app = App::new();
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for theme_index in 0..THEMES.len() {
            app.mode = mode;
            app.theme = theme_index;
            assert!(theme(&app, ThemeMode::Light).palette.is_valid());
        }
    }
}

#[test]
fn worker_progress_ignores_stale_generations() {
    let mut app = App::new();
    app.job_running = true;
    update(
        &mut app,
        Message::Progress {
            generation: 0,
            step: 40,
        },
    );
    assert_eq!(app.job_progress, 40);
    app.cancel_job();
    update(
        &mut app,
        Message::Progress {
            generation: 0,
            step: 80,
        },
    );
    assert_eq!(app.job_progress, 40, "cancelled task results are ignored");
}
