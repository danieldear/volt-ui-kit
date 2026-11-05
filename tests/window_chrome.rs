#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, s: f32, w: f32) -> [f32; 2] {
        [(t.len() as f32 * s * 0.5).min(w), s * 1.2]
    }
}
#[test]
fn controls_emit_typed_intents_have_accessible_names_and_do_not_drag() {
    for maximized in [false, true] {
        for scale in [1., 1.25, 2.] {
            let ui: Element<Vec<WindowAction>> = hstack![
                text("Title"),
                spacer(),
                window_controls(maximized, |out: &mut Vec<WindowAction>, a| out.push(a))
            ]
            .height(44.)
            .drag_region(true)
            .into();
            let (doc, mut handlers) = ui.build().unwrap().into_parts();
            let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
            runtime
                .layout(
                    Viewport::new(640. * scale, 120. * scale, scale).unwrap(),
                    &Bindings::default(),
                    &mut Metrics,
                )
                .unwrap();
            let mut out = vec![];
            for (label, intent) in [
                ("Minimize window", WindowAction::Minimize),
                (
                    if maximized {
                        "Restore window"
                    } else {
                        "Maximize window"
                    },
                    WindowAction::ToggleMaximize,
                ),
                ("Close window", WindowAction::Close),
            ] {
                let node = runtime
                    .accessibility_nodes()
                    .into_iter()
                    .find(|n| n.label == label)
                    .unwrap();
                let b = runtime
                    .boxes()
                    .iter()
                    .find(|b| b.id == node.id)
                    .unwrap()
                    .bounds
                    .bounds();
                assert!(!runtime.drag_region_hit(b[0] + b[2] / 2., b[1] + b[3] / 2.));
                assert!(
                    runtime
                        .accessibility_activate(&node.id, &mut handlers, &mut out)
                        .unwrap()
                        .activated
                );
                assert_eq!(out.last(), Some(&intent));
            }
            assert!(runtime.drag_region_hit(200. * scale, 20. * scale));
        }
    }
}
#[test]
fn controls_can_be_reused_with_custom_spacing_and_no_duplicate_keys() {
    let ui: Element<()> = vstack![
        window_controls(false, |_: &mut (), _| {})
            .key("first")
            .gap(12., Axis::Horizontal),
        window_controls(true, |_: &mut (), _| {}).key("second")
    ]
    .into();
    let (doc, handlers) = ui.build().unwrap().into_parts();
    assert!(Runtime::new(doc, &handlers, &Bindings::default()).is_ok());
}

#[test]
fn passive_toast_layers_do_not_block_drag_but_their_buttons_still_do() {
    let ui: Element<()> = zstack![
        hstack![text("Title"), spacer()]
            .height(44.)
            .drag_region(true),
        zstack![button("Toast action")
            .width(80.)
            .height(30.)
            .on_click(|_: &mut ()| {})]
        .scroll_passthrough(true),
    ]
    .into();
    let (doc, handlers) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(640., 44., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    assert!(r.drag_region_hit(200., 20.));
    let b = r
        .boxes()
        .iter()
        .find(|b| b.kind == volt_ui_kit::declarative::Kind::Button)
        .unwrap()
        .bounds
        .bounds();
    assert!(!r.drag_region_hit(b[0] + b[2] / 2., b[1] + b[3] / 2.));
}

#[test]
fn disabled_maximize_has_no_pointer_or_accessibility_activation() {
    let state = WindowControlsState {
        can_maximize: false,
        ..Default::default()
    };
    let ui: Element<Vec<WindowAction>> =
        window_controls(state, |out: &mut Vec<WindowAction>, a| out.push(a)).into();
    let (doc, mut handlers) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(320., 60., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let node = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.label == "Maximize window")
        .unwrap();
    let mut out = vec![];
    assert!(
        !r.accessibility_activate(&node.id, &mut handlers, &mut out)
            .unwrap()
            .activated
    );
    assert!(out.is_empty());
}
