#![cfg(feature = "rust-ui")]
//! Drag grips must survive the recompositions their callbacks trigger.
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
#[derive(Default)]
struct App {
    events: Vec<(u32, [f32; 2])>,
    generation: u32,
    starts: usize,
    hidden: bool,
    remove: bool,
    root: u32,
    other_kind: bool,
    modal: bool,
    extra_control: bool,
}
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, s: f32, _: f32) -> [f32; 2] {
        [t.len() as f32 * s / 2., s]
    }
}
fn view(a: &App) -> Element<App> {
    let generation = a.generation;
    let grip: Element<App> = if a.other_kind {
        hstack([])
            .key("grip")
            .width(40.)
            .height(40.)
            .visible(!a.hidden)
            .on_drag_start(|a: &mut App| a.starts += 1)
            .on_drag(move |a: &mut App, d| a.events.push((generation, d)))
            .into()
    } else {
        panel([])
            .key("grip")
            .width(40.)
            .height(40.)
            .visible(!a.hidden)
            .on_drag_start(|a: &mut App| a.starts += 1)
            .on_drag(move |a: &mut App, d| a.events.push((generation, d)))
            .into()
    };
    let mut children = vec![];
    if a.extra_control {
        children.push(
            button("Extra callback")
                .key("before")
                .on_click(|_: &mut App| {})
                .into(),
        );
    }
    if !a.remove {
        children.push(vstack([grip]).into());
    }
    if a.modal {
        children.push(
            dialog(text("New modal"))
                .key("modal")
                .open(true)
                .on_dismiss(|_: &mut App| {})
                .into(),
        );
    }
    zstack(children)
        .key(format!("root-{}", a.root))
        .width(300.)
        .height(200.)
        .into()
}
fn mount(a: &App) -> (Runtime, volt_ui_kit::declarative::Handlers<'static, App>) {
    let (d, h) = view(a).build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(300., 200., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    (r, h)
}
fn rebuild(r: &mut Runtime, h: &mut volt_ui_kit::declarative::Handlers<'static, App>, a: &App) {
    let (d, next) = view(a).build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    *h = next;
    r.layout(
        Viewport::new(300., 200., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
}
fn press(
    r: &mut Runtime,
    h: &mut volt_ui_kit::declarative::Handlers<'static, App>,
    a: &mut App,
) -> [f32; 2] {
    let b = r
        .boxes()
        .iter()
        .find(|b| b.id == "grip")
        .unwrap()
        .bounds
        .bounds();
    let p = [b[0] + 20., b[1] + 20.];
    assert!(r.pointer_down_outcome(p[0], p[1], h, a).unwrap().activated);
    p
}
#[test]
fn drag_survives_each_rebuild_uses_the_new_handler_and_stops_after_release() {
    let mut a = App::default();
    let (mut r, mut h) = mount(&a);
    let p = press(&mut r, &mut h, &mut a);
    for i in 1..=3 {
        a.generation = i;
        a.extra_control = i % 2 == 1;
        rebuild(&mut r, &mut h, &a);
        assert!(
            r.pointer_move_outcome(p[0] - i as f32 * 10., p[1] + i as f32 * 20., &mut h, &mut a)
                .unwrap()
                .activated
        );
        assert_eq!(
            a.events.last(),
            Some(&(i, [-(i as f32) * 10., i as f32 * 20.]))
        );
    }
    r.pointer_up_outcome(p[0] - 30., p[1] + 60., &mut h, &mut a)
        .unwrap();
    let n = a.events.len();
    rebuild(&mut r, &mut h, &a);
    r.pointer_move_outcome(p[0] - 40., p[1] + 80., &mut h, &mut a)
        .unwrap();
    assert_eq!(a.events.len(), n);
}
#[test]
fn replacing_drag_target_scope_or_enabling_a_modal_cancels_capture() {
    for change in 0..5 {
        let mut a = App::default();
        let (mut r, mut h) = mount(&a);
        let p = press(&mut r, &mut h, &mut a);
        match change {
            0 => a.hidden = true,
            1 => a.remove = true,
            2 => a.root = 1,
            3 => a.modal = true,
            _ => a.other_kind = true,
        };
        rebuild(&mut r, &mut h, &a);
        let n = a.events.len();
        r.pointer_move_outcome(p[0] + 30., p[1] + 30., &mut h, &mut a)
            .unwrap();
        assert_eq!(a.events.len(), n, "cancel case {change}");
    }
}

#[test]
fn dragging_owns_the_press_and_cancel_or_viewport_changes_end_it() {
    for cause in 0..3 {
        let mut a = App::default();
        let (mut r, mut h) = mount(&a);
        let p = press(&mut r, &mut h, &mut a);
        assert!(!r.press_allows_text_selection());
        if cause == 0 {
            r.cancel_input();
        } else {
            r.layout(
                Viewport::new(320., 200., if cause == 1 { 1. } else { 2. }).unwrap(),
                &Bindings::default(),
                &mut Metrics,
            )
            .unwrap();
        }
        let n = a.events.len();
        r.pointer_move_outcome(p[0] + 10., p[1] + 10., &mut h, &mut a)
            .unwrap();
        assert_eq!(a.events.len(), n);
    }
}

#[test]
fn a_transparent_grip_claims_the_native_press_before_a_code_view() {
    use volt_ui_kit::declarative::{CodeLine, InputModifiers};
    let tree: Element<App> = zstack([
        code_view(1)
            .key("code")
            .lines(
                0,
                vec![CodeLine {
                    text: "source".into(),
                    spans: vec![],
                }],
            )
            .width(300.)
            .height(200.)
            .on_input(|a: &mut App, _| {
                a.events.push((99, [0., 0.]));
                None
            })
            .into(),
        panel([])
            .key("grip")
            .width(40.)
            .height(40.)
            .background("#00000000")
            .on_drag(|a: &mut App, d| a.events.push((0, d)))
            .into(),
    ])
    .width(300.)
    .height(200.)
    .into();
    let (d, mut h) = tree.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(300., 200., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let mut a = App::default();
    let b = r
        .boxes()
        .iter()
        .find(|b| b.id == "grip")
        .unwrap()
        .bounds
        .bounds();
    let p = [b[0] + 2., b[1] + 2.];
    assert!(r
        .code_pointer_down(p[0], p[1], 1, InputModifiers::default(), &mut h, &mut a)
        .unwrap()
        .is_none());
    assert!(
        r.pointer_down_outcome(p[0], p[1], &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(!r.press_allows_text_selection());
    r.pointer_move_outcome(p[0] - 30., p[1] + 60., &mut h, &mut a)
        .unwrap();
    assert_eq!(a.events, vec![(0, [0., 0.]), (0, [-30., 60.])]);
}
#[test]
fn invalid_pointer_coordinates_cancel_a_drag_instead_of_sending_nan_sizes() {
    for position in [[f32::NAN, 0.], [0., f32::INFINITY]] {
        let mut a = App::default();
        let (mut r, mut h) = mount(&a);
        let p = press(&mut r, &mut h, &mut a);
        let n = a.events.len();
        assert!(
            !r.pointer_move_outcome(position[0], position[1], &mut h, &mut a)
                .unwrap()
                .activated
        );
        r.pointer_move_outcome(p[0] + 20., p[1] + 20., &mut h, &mut a)
            .unwrap();
        assert_eq!(a.events.len(), n);
    }
}

#[test]
fn drag_start_is_once_per_press_not_each_return_to_zero_offset() {
    let mut a = App::default();
    let (mut r, mut h) = mount(&a);
    let p = press(&mut r, &mut h, &mut a);
    assert_eq!(a.starts, 1);
    for dx in [30., 0., -30., 0.] {
        rebuild(&mut r, &mut h, &a);
        r.pointer_move_outcome(p[0] + dx, p[1], &mut h, &mut a)
            .unwrap();
        assert_eq!(a.starts, 1);
        assert_eq!(a.events.last().unwrap().1, [dx, 0.]);
    }
    r.pointer_up_outcome(p[0], p[1], &mut h, &mut a).unwrap();
    press(&mut r, &mut h, &mut a);
    assert_eq!(a.starts, 2);
}
