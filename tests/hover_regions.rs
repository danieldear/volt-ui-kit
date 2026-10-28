#![cfg(feature = "native")]
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
    escapes: Vec<&'static str>,
}

fn view(_: &State) -> Element<State> {
    let panel_a = hstack([button("Inside")
        .key("inside")
        .width(60.)
        .height(30.)
        .on_click(|_: &mut State| {})
        .into()])
    .key("panel")
    .width(200.)
    .height(100.)
    .on_hover(|s: &mut State, inside| s.events.push(("panel", inside)))
    .on_focus_within(|s: &mut State, inside| s.events.push(("focus", inside)))
    .on_escape(|s: &mut State| s.escapes.push("panel"));
    let other = button("Elsewhere")
        .key("elsewhere")
        .width(80.)
        .height(30.)
        .on_click(|_: &mut State| {});
    let under = vstack([spacer().height(1.).into()])
        .key("under")
        .on_escape(|s: &mut State| s.escapes.push("under"));
    vstack([
        under.into(),
        panel_a.into(),
        other.into(),
        spacer().height(70.).into(),
    ])
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
    let [x, y, w, h] = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "panel")
        .unwrap()
        .bounds
        .bounds();
    // Over the panel's own button: inside the region.
    assert!(runtime
        .dispatch_hover_regions(Some([x + 20., y + 15.]), &mut handlers, &mut state)
        .unwrap());
    // Moving within the region reports nothing new.
    assert!(!runtime
        .dispatch_hover_regions(Some([x + w - 10., y + h - 10.]), &mut handlers, &mut state)
        .unwrap());
    // Below the panel: left.
    assert!(runtime
        .dispatch_hover_regions(Some([x + 150., y + h + 40.]), &mut handlers, &mut state)
        .unwrap());
    // Back in, then the pointer leaves the window.
    runtime
        .dispatch_hover_regions(Some([x + 10., y + 10.]), &mut handlers, &mut state)
        .unwrap();
    runtime
        .dispatch_hover_regions(None, &mut handlers, &mut state)
        .unwrap();
    assert_eq!(
        state.events,
        vec![
            ("panel", true),
            ("panel", false),
            ("panel", true),
            ("panel", false)
        ]
    );
}

#[test]
fn focus_within_and_escape_reach_the_right_container() {
    let mut state = State::default();
    let (d, mut handlers) = view(&state).build().unwrap().into_parts();
    let bindings = Bindings::default();
    let mut runtime = Runtime::new(d, &handlers, &bindings).unwrap();
    let viewport = Viewport::new(400., 300., 1.).unwrap();
    let mut painter = volt_ui_kit::native::RasterPainter::new(viewport);
    runtime.layout(viewport, &bindings, &mut painter).unwrap();
    let centre = |id: &str| {
        let b = runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .bounds
            .bounds();
        [b[0] + b[2] / 2., b[1] + b[3] / 2.]
    };
    let (inside, outside) = (centre("inside"), centre("elsewhere"));
    // Focus on the panel's button: inside the region.
    runtime.pointer_down(inside[0], inside[1]);
    assert!(runtime
        .dispatch_focus_regions(&mut handlers, &mut state)
        .unwrap());
    // Unchanged focus reports nothing.
    assert!(!runtime
        .dispatch_focus_regions(&mut handlers, &mut state)
        .unwrap());
    // Focus elsewhere: left.
    runtime.pointer_down(outside[0], outside[1]);
    assert!(runtime
        .dispatch_focus_regions(&mut handlers, &mut state)
        .unwrap());
    assert_eq!(state.events, vec![("focus", true), ("focus", false)]);
    // An unused Escape goes to the last on_escape container in the document.
    assert!(runtime.dispatch_escape(&mut handlers, &mut state).unwrap());
    assert_eq!(state.escapes, vec!["panel"]);
}
