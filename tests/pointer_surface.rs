#![cfg(feature = "rust-ui")]
//! Pointer surfaces (`on_pointer`) hear presses with their modifiers and
//! click count, drags while captured, hover and leave, wheel and pinch; a
//! control inside the surface still takes its own press.
use volt_ui_kit::{
    declarative::{
        Bindings, Handlers, InputModifiers, Pointer, PointerPhase, Runtime, TextMetrics,
    },
    ui::prelude::*,
    Viewport,
};

#[derive(Default)]
struct App {
    seen: Vec<Pointer>,
    clicks: usize,
    list_scrolled: bool,
}

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, s: f32, _: f32) -> [f32; 2] {
        [t.len() as f32 * s / 2., s]
    }
}

/// A 300 × 200 surface at (50, 50) holding a button at its top left, and a
/// scrolling list beside it.
fn view(_: &App) -> Element<App> {
    let surface = zstack![button("Inside")
        .key("inside")
        .width(60.)
        .height(30.)
        .on_click(|a: &mut App| a.clicks += 1)]
    .key("surface")
    .flow(Flow::TopLeft)
    .width(300.)
    .height(200.)
    .on_pointer(|a: &mut App, p| a.seen.push(p));
    let list = vstack((0..40).map(|i| -> Element<App> {
        text(format!("Row {i}"))
            .key(format!("row-{i}"))
            .height(20.)
            .into()
    }))
    .key("list")
    .width(100.)
    .height(200.)
    .scroll(Scroll::Vertical)
    .on_height(|a: &mut App, _| a.list_scrolled = false);
    hstack![surface, list]
        .key("root")
        .width(500.)
        .height(300.)
        .padding(50., Edge::All)
        .align(Align::Start)
        .into()
}

fn mount(a: &App) -> (Runtime, Handlers<'static, App>) {
    let (d, h) = view(a).build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(500., 300., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    (r, h)
}

fn shift() -> InputModifiers {
    InputModifiers {
        shift: true,
        ..Default::default()
    }
}

#[test]
fn press_drag_release_carry_position_modifiers_and_clicks() {
    let mut app = App::default();
    let (mut r, mut h) = mount(&app);
    let out = r
        .pointer_down_outcome_with_clicks(200., 150., shift(), 2, &mut h, &mut app)
        .unwrap();
    assert!(out.activated);
    let press = app.seen[0];
    assert_eq!(press.phase, PointerPhase::Press);
    assert_eq!(press.at, [150., 100.]);
    assert!(press.modifiers.shift);
    assert_eq!(press.clicks, 2);

    // A modifier change mid-drag arrives with the next drag event.
    r.set_pointer_modifiers(InputModifiers::default());
    r.pointer_move_outcome(230., 140., &mut h, &mut app)
        .unwrap();
    let drag = *app.seen.last().unwrap();
    assert_eq!(drag.phase, PointerPhase::Drag);
    assert_eq!(drag.offset, [30., -10.]);
    assert_eq!(drag.at, [180., 90.]);
    assert!(!drag.modifiers.shift);

    // Released outside the surface: still the surface's release.
    let out = r.pointer_up_outcome(480., 290., &mut h, &mut app).unwrap();
    assert!(out.activated);
    let release = *app.seen.last().unwrap();
    assert_eq!(release.phase, PointerPhase::Release);
    assert_eq!(release.offset, [280., 140.]);
    assert!(!r.pointer_dragging());
}

#[test]
fn a_control_inside_the_surface_keeps_its_press() {
    let mut app = App::default();
    let (mut r, mut h) = mount(&app);
    r.pointer_down_outcome_with_clicks(70., 60., InputModifiers::default(), 1, &mut h, &mut app)
        .unwrap();
    r.pointer_up_outcome(70., 60., &mut h, &mut app).unwrap();
    assert_eq!(app.clicks, 1);
    assert!(app.seen.iter().all(|p| p.phase != PointerPhase::Press));
}

#[test]
fn hover_then_leave() {
    let mut app = App::default();
    let (mut r, mut h) = mount(&app);
    let out = r
        .pointer_move_outcome(250., 200., &mut h, &mut app)
        .unwrap();
    assert!(out.activated);
    let hover = *app.seen.last().unwrap();
    assert_eq!((hover.phase, hover.at), (PointerPhase::Hover, [200., 150.]));
    // Over the list: the surface is told it was left.
    r.pointer_move_outcome(400., 100., &mut h, &mut app)
        .unwrap();
    assert_eq!(app.seen.last().unwrap().phase, PointerPhase::Leave);
    let count = app.seen.len();
    r.pointer_move_outcome(410., 110., &mut h, &mut app)
        .unwrap();
    assert_eq!(app.seen.len(), count, "no repeat leave");
    // Back in, then out of the window.
    r.pointer_move_outcome(250., 200., &mut h, &mut app)
        .unwrap();
    r.pointer_leave_outcome(&mut h, &mut app).unwrap();
    assert_eq!(app.seen.last().unwrap().phase, PointerPhase::Leave);
}

#[test]
fn wheel_and_pinch_go_to_the_surface_not_the_page() {
    let mut app = App::default();
    let (mut r, mut h) = mount(&app);
    r.set_pointer_modifiers(InputModifiers {
        meta: true,
        ..Default::default()
    });
    let out = r
        .scroll_outcome(200., 150., [0., 40.], &mut h, &mut app)
        .unwrap();
    assert!(out.activated);
    let wheel = *app.seen.last().unwrap();
    assert_eq!(wheel.phase, PointerPhase::Wheel);
    assert_eq!(wheel.delta, [0., 40.]);
    assert!(wheel.modifiers.meta);

    r.pinch_outcome(200., 150., 0.1, &mut h, &mut app).unwrap();
    let pinch = *app.seen.last().unwrap();
    assert_eq!(pinch.phase, PointerPhase::Pinch);
    assert!((pinch.zoom - 0.1).abs() < 1e-6);

    // Elsewhere the wheel still scrolls content.
    let count = app.seen.len();
    r.scroll_outcome(400., 150., [0., 40.], &mut h, &mut app)
        .unwrap();
    assert_eq!(app.seen.len(), count);
    assert!(r.scroll_offset("list")[1] > 0.);
    // A pinch with no surface under it does nothing.
    let out = r.pinch_outcome(400., 150., 0.1, &mut h, &mut app).unwrap();
    assert!(!out.activated);
}
