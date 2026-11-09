#![cfg(feature = "rust-ui")]
//! Apps recompose on timers and background results. A press that spans a
//! recomposition still clicks when released over the same control; a press
//! on a control that went away does not.
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
    clicks: u32,
    ticks: u32,
    hide: bool,
}
fn view(app: &App) -> Element<App> {
    let mut items: Vec<Element<App>> = vec![text(format!("Tick {}", app.ticks)).into()];
    if !app.hide {
        items.push(
            button("Add")
                .key("add")
                .on_click(|app: &mut App| app.clicks += 1)
                .into(),
        );
    }
    vstack(items).key("root").into()
}
struct Harness {
    app: App,
    runtime: Runtime,
    handlers: Handlers<'static, App>,
}
impl Harness {
    fn new() -> Self {
        let app = App::default();
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
                Viewport::new(400., 300., 1.).unwrap(),
                &Bindings::default(),
                &mut Metrics,
            )
            .unwrap();
    }
    /// What a background message does: change state, rebuild, lay out.
    fn tick(&mut self) {
        self.app.ticks += 1;
        let (d, handlers) = view(&self.app).build().unwrap().into_parts();
        self.runtime
            .replace_document(d, &handlers, &Bindings::default())
            .unwrap();
        self.handlers = handlers;
        self.layout();
    }
    fn centre(&self, id: &str) -> (f32, f32) {
        let [x, y, w, h] = self
            .runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .bounds
            .bounds();
        (x + w / 2., y + h / 2.)
    }
}

#[test]
fn a_press_spanning_a_recomposition_still_clicks() {
    let mut h = Harness::new();
    let (x, y) = h.centre("add");
    h.runtime
        .pointer_down_outcome(x, y, &mut h.handlers, &mut h.app)
        .unwrap();
    h.tick();
    h.tick();
    let out = h
        .runtime
        .pointer_up_outcome(x, y, &mut h.handlers, &mut h.app)
        .unwrap();
    assert!(out.activated);
    assert_eq!(h.app.clicks, 1);
}

#[test]
fn a_press_on_a_control_that_went_away_does_not_click() {
    let mut h = Harness::new();
    let (x, y) = h.centre("add");
    h.runtime
        .pointer_down_outcome(x, y, &mut h.handlers, &mut h.app)
        .unwrap();
    h.app.hide = true;
    h.tick();
    h.app.hide = false;
    h.tick();
    let out = h
        .runtime
        .pointer_up_outcome(x, y, &mut h.handlers, &mut h.app)
        .unwrap();
    assert!(
        !out.activated,
        "the press ended when its button was removed"
    );
    assert_eq!(h.app.clicks, 0);
}
