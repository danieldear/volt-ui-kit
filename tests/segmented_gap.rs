#![cfg(feature = "rust-ui")]
//! Segmented controls and joined button bars keep a caller-chosen gap.
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};

struct App(u8);
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
        [
            (text.chars().count() as f32 * size * 0.5).min(width),
            size * 1.2,
        ]
    }
}
fn spacing(control: Element<App>) -> f32 {
    let (document, handlers) = control.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(600., 200., 1.).unwrap();
    let mut painter = Metrics;
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    let a = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "a")
        .unwrap()
        .bounds
        .bounds();
    let b = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "b")
        .unwrap()
        .bounds
        .bounds();
    b[0] - (a[0] + a[2])
}
/// The same joined path `segmented_control` uses, with keyed segments.
fn segmented() -> SelectionControl<App, u8> {
    tab_bar([
        tab_item(0_u8, "Space").key("a"),
        tab_item(1, "Duplicates").key("b"),
    ])
    .joined(true)
    .key("mode")
    .selected(0)
    .on_select(|app: &mut App, v| app.0 = v)
}

#[test]
fn segmented_gap_defaults_to_a_seam_and_respects_the_caller() {
    let seam = spacing(segmented().into());
    assert!((seam - 1.).abs() < 0.5, "default seam: {seam}");
    let wide = spacing(segmented().gap(8., Axis::Horizontal).into());
    assert!((wide - 8.).abs() < 0.5, "caller gap kept: {wide}");
}
