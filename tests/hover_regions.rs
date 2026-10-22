#![cfg(feature = "rust-ui")]
//! Containers with `on_hover` learn when the pointer enters and leaves them,
//! descendants included, so a floating panel can stay open while in use.
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    ui::prelude::*,
    Viewport,
};

#[derive(Default)]
struct State {
    events: Vec<(&'static str, bool)>,
}

fn view(_: &State) -> Element<State> {
    let panel_a = hstack([button("Inside").key("inside").width(60.).height(30.).on_click(|_: &mut State| {}).into()])
        .key("panel")
        .width(200.)
        .height(100.)
        .on_hover(|s: &mut State, inside| s.events.push(("panel", inside)));
    vstack([panel_a.into(), spacer().height(100.).into()])
        .key("root")
        .width(400.)
        .height(300.)
        .into()
}

#[test]
fn hover_regions_report_entering_and_leaving_once_each() {
    let mut state = State::default();
    let (d, mut handlers) = view(&state).build().unwrap().into_parts();
    let bindings = Bindings::default();
    let mut runtime = Runtime::new(d, &handlers, &bindings).unwrap();
    let viewport = Viewport::new(400., 300., 1.).unwrap();
    let mut painter = volt_ui_kit::native::RasterPainter::new(viewport);
    runtime.layout(viewport, &bindings, &mut painter).unwrap();
    // Over the panel's own button: inside the region.
    assert!(runtime.dispatch_hover_regions(Some([20., 15.]), &mut handlers, &mut state).unwrap());
    // Moving within the region reports nothing new.
    assert!(!runtime.dispatch_hover_regions(Some([150., 80.]), &mut handlers, &mut state).unwrap());
    // Below the panel: left.
    assert!(runtime.dispatch_hover_regions(Some([150., 200.]), &mut handlers, &mut state).unwrap());
    // Back in, then the pointer leaves the window.
    runtime.dispatch_hover_regions(Some([10., 10.]), &mut handlers, &mut state).unwrap();
    runtime.dispatch_hover_regions(None, &mut handlers, &mut state).unwrap();
    assert_eq!(
        state.events,
        vec![("panel", true), ("panel", false), ("panel", true), ("panel", false)]
    );
}
