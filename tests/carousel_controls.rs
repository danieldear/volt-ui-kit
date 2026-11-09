#![cfg(feature = "rust-ui")]
//! A carousel of interactive slides: named slides, with the buttons below.
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}

#[derive(Default)]
struct App {
    slide: usize,
    below: bool,
    named: bool,
    clicks: u32,
}
fn view(app: &App) -> Element<App> {
    let slides = ["Mail", "Maps", "Music"].map(|name| {
        button(format!("{name}: {}", app.clicks))
            .key(format!("open-{}", name.to_lowercase()))
            .on_click(|app: &mut App| app.clicks += 1)
            .into()
    });
    let mut deck = carousel(slides, app.slide)
        .key("apps")
        .height(300.)
        .controls_below(app.below)
        .on_change(|app: &mut App, i| app.slide = i);
    if app.named {
        deck = deck.labels(["Mail", "Maps", "Music"]);
    }
    vstack![deck].key("root").into()
}
struct Harness {
    app: App,
    runtime: Runtime,
    handlers: Handlers<'static, App>,
}
impl Harness {
    fn new(app: App) -> Self {
        let (d, handlers) = view(&app).build().unwrap().into_parts();
        let runtime = Runtime::new(d, &handlers, &Bindings::default()).unwrap();
        let mut h = Self {
            app,
            runtime,
            handlers,
        };
        h.layout();
        h
    }
    fn layout(&mut self) {
        self.runtime
            .layout(
                Viewport::new(800., 600., 1.).unwrap(),
                &Bindings::default(),
                &mut Metrics,
            )
            .unwrap();
    }
    fn bounds(&self, id: &str) -> [f32; 4] {
        self.runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("no {id}"))
            .bounds
            .bounds()
    }
    fn has(&self, id: &str) -> bool {
        self.runtime.boxes().iter().any(|b| b.id == id)
    }
    fn click(&mut self, id: &str) {
        let [x, y, w, h] = self.bounds(id);
        let (x, y) = (x + w * 0.5, y + h * 0.5);
        self.runtime.pointer_down(x, y);
        let outcome = self
            .runtime
            .pointer_up_outcome(x, y, &mut self.handlers, &mut self.app)
            .unwrap();
        assert!(outcome.activated, "{id} did not activate");
        let (d, handlers) = view(&self.app).build().unwrap().into_parts();
        self.runtime
            .replace_document(d, &handlers, &Bindings::default())
            .unwrap();
        self.handlers = handlers;
        self.layout();
    }
}

#[test]
fn controls_below_keep_the_slide_clear() {
    // By default the arrows sit over the slide's edges.
    let h = Harness::new(App::default());
    assert!(h.has("apps-arrows") && !h.has("apps-controls"));
    let [_, stage_y, _, stage_h] = h.bounds("apps-stage");
    let [_, next_y, _, _] = h.bounds("apps-next");
    assert!(next_y < stage_y + stage_h, "edge arrows overlap the slide");

    // Below, they share a row with the markers under the whole slide.
    let mut h = Harness::new(App {
        below: true,
        ..App::default()
    });
    assert!(h.has("apps-controls") && !h.has("apps-arrows"));
    let [_, stage_y, _, stage_h] = h.bounds("apps-stage");
    for id in ["apps-prev", "apps-dot-0", "apps-next"] {
        assert!(
            h.bounds(id)[1] >= stage_y + stage_h,
            "{id} is over the slide"
        );
    }
    // The slide stays fully interactive, and the buttons still page.
    h.click("open-mail");
    assert_eq!(h.app.clicks, 1);
    h.click("apps-next");
    assert_eq!(h.app.slide, 1);
    assert!(h.has("open-maps") && !h.has("open-mail"));
    h.click("apps-prev");
    h.click("apps-prev");
    assert_eq!(h.app.slide, 2, "wraps around");
}

#[test]
fn named_slides_replace_the_dots() {
    let mut h = Harness::new(App {
        below: true,
        named: true,
        ..App::default()
    });
    // A named marker is a text button, wider than a dot.
    assert!(h.bounds("apps-dot-1")[2] > 18.);
    h.click("apps-dot-2");
    assert_eq!(h.app.slide, 2);
    assert!(h.has("open-music") && h.has("apps-slide-2"));

    // Names must cover every slide.
    let wrong: Element<()> = carousel([text("One").into(), text("Two").into()], 0)
        .key("c")
        .labels(["One"])
        .on_change(|_, _| {})
        .into();
    assert!(wrong.build().is_err());
}
