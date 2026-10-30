#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
#[derive(Default)]
struct App {
    clicks: usize,
}
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
fn layout(r: &mut Runtime, scale: f32) {
    r.layout(
        Viewport::new(300. * scale, 160. * scale, scale).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
}
fn mount(e: Element<App>, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    layout(&mut r, scale);
    (r, h)
}
fn scroll(hidden: bool) -> Element<App> {
    vstack((0..12).map(|i| {
        Element::from(
            button(format!("Action {i}"))
                .key(format!("action-{i}"))
                .height(30.)
                .enabled(i != 5)
                .on_click(|a: &mut App| a.clicks += 1),
        )
    }))
    .key("scroll")
    .height(100.)
    .width(240.)
    .gap(4., Axis::Vertical)
    .scroll(Scroll::Vertical)
    .scrollbars(if hidden {
        Scrollbars::Hidden
    } else {
        Scrollbars::Auto
    })
    .into()
}
#[test]
fn keyboard_navigation_reveals_every_offscreen_control_and_skips_disabled() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scroll(false), scale);
        let mut a = App::default();
        for i in (0..12).filter(|i| *i != 5) {
            r.focus_next(false);
            layout(&mut r, scale);
            assert_eq!(r.focused(), Some(format!("action-{i}").as_str()));
            let b = r
                .boxes()
                .iter()
                .find(|b| b.id == format!("action-{i}"))
                .unwrap();
            let [x, y, w, h] = b.bounds.bounds();
            let [cx, cy, cw, ch] = b.clip.bounds();
            assert!(x >= cx && y >= cy && x + w <= cx + cw + 0.1 && y + h <= cy + ch + 0.1);
        }
        assert!(r.scroll_offset("scroll")[1] > 0.);
        r.key_down();
        r.key_up_outcome(&mut h, &mut a).unwrap();
        assert_eq!(a.clicks, 1);
        r.focus_next(false);
        layout(&mut r, scale);
        assert_eq!(r.focused(), Some("action-0"));
        assert_eq!(r.scroll_offset("scroll")[1], 0.);
    }
}
#[test]
fn visible_scrollbar_drag_has_capture_and_hidden_bars_do_not_intercept() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scroll(false), scale);
        let mut a = App::default();
        r.pointer_down_outcome(235. * scale, 12. * scale, &mut h, &mut a)
            .unwrap();
        r.pointer_move_outcome(235. * scale, 80. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert!(r.scroll_offset("scroll")[1] > 100.);
        let old = r.scroll_offset("scroll")[1];
        r.cancel_input();
        r.pointer_move_outcome(235. * scale, 20. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("scroll")[1], old);
        assert_eq!(a.clicks, 0);
        let (mut r, mut h) = mount(scroll(true), scale);
        r.pointer_down_outcome(235. * scale, 12. * scale, &mut h, &mut a)
            .unwrap();
        r.pointer_move_outcome(235. * scale, 80. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("scroll")[1], 0.);
    }
}
#[test]
fn keyboard_focus_reveals_nested_scroll_ancestors() {
    for scale in [1., 2.] {
        let e: Element<App> = vstack![
            spacer().height(160.).flex(0.),
            vstack((0..8).map(|i| {
                Element::from(
                    button("Nested")
                        .key(format!("nested-{i}"))
                        .height(30.)
                        .on_click(|a: &mut App| a.clicks += 1),
                )
            }))
            .key("inner")
            .height(90.)
            .scroll(Scroll::Vertical),
            spacer().height(200.).flex(0.)
        ]
        .key("outer")
        .height(130.)
        .scroll(Scroll::Vertical)
        .into();
        let (mut r, _) = mount(e, scale);
        for i in 0..8 {
            r.focus_next(false);
            layout(&mut r, scale);
            assert_eq!(r.focused(), Some(format!("nested-{i}").as_str()));
            let b = r
                .boxes()
                .iter()
                .find(|b| b.id == format!("nested-{i}"))
                .unwrap();
            let [x, y, w, h] = b.bounds.bounds();
            let [cx, cy, cw, ch] = b.clip.bounds();
            assert!(x >= cx && y >= cy && x + w <= cx + cw + 0.1 && y + h <= cy + ch + 0.1);
        }
        assert!(r.scroll_offset("outer")[1] > 0.);
        assert!(r.scroll_offset("inner")[1] > 0.);
    }
}
#[test]
fn horizontal_keyboard_reveal_and_scrollbar_drag_work_at_both_scales() {
    for scale in [1., 2.] {
        let e: Element<App> = hstack((0..6).map(|i| {
            Element::from(
                button("Action")
                    .key(format!("h-{i}"))
                    .width(80.)
                    .height(30.)
                    .on_click(|a: &mut App| a.clicks += 1),
            )
        }))
        .key("horizontal")
        .width(240.)
        .height(50.)
        .scroll(Scroll::Horizontal)
        .into();
        let (mut r, mut h) = mount(e, scale);
        let mut a = App::default();
        for i in 0..6 {
            r.focus_next(false);
            layout(&mut r, scale);
            assert_eq!(r.focused(), Some(format!("h-{i}").as_str()));
        }
        assert!(r.scroll_offset("horizontal")[0] > 200.);
        assert!(
            r.pointer_down_outcome(210. * scale, 45. * scale, &mut h, &mut a)
                .unwrap()
                .changed
        );
        r.pointer_move_outcome(35. * scale, 45. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert!(r.scroll_offset("horizontal")[0] < 50.);
        r.pointer_up_outcome(35. * scale, 45. * scale, &mut h, &mut a)
            .unwrap();
        let old = r.scroll_offset("horizontal");
        r.pointer_move_outcome(210. * scale, 45. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("horizontal"), old);
    }
}
#[test]
fn scrollbar_capture_survives_compatible_recomposition_but_resize_and_tab_disarm_it() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scroll(false), scale);
        let mut a = App::default();
        r.pointer_down_outcome(235. * scale, 12. * scale, &mut h, &mut a)
            .unwrap();
        let (d, new_h) = scroll(false).build().unwrap().into_parts();
        r.replace_document(d, &new_h, &Bindings::default()).unwrap();
        h = new_h;
        layout(&mut r, scale);
        r.pointer_move_outcome(235. * scale, 80. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert!(r.scroll_offset("scroll")[1] > 100.);
        r.focus_next(false);
        layout(&mut r, scale);
        let old = r.scroll_offset("scroll");
        r.pointer_move_outcome(235. * scale, 20. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("scroll"), old);
        r.pointer_down_outcome(235. * scale, 12. * scale, &mut h, &mut a)
            .unwrap();
        r.invalidate_layout();
        r.layout(
            Viewport::new(310. * scale, 160. * scale, scale).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
        let old = r.scroll_offset("scroll");
        r.pointer_move_outcome(235. * scale, 80. * scale, &mut h, &mut a)
            .unwrap();
        assert_eq!(r.scroll_offset("scroll"), old);
    }
}
#[test]
fn offscreen_controls_do_not_create_invisible_click_targets() {
    let (mut r, mut h) = mount(scroll(false), 1.);
    let mut a = App::default();
    assert!(!r.boxes().iter().any(|b| b.id == "action-10"));
    r.pointer_down_outcome(30., 145., &mut h, &mut a).unwrap();
    assert!(
        !r.pointer_up_outcome(30., 145., &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(a.clicks, 0);
}
#[test]
fn keyboard_activation_disarms_a_held_scrollbar_gesture() {
    let (mut r, mut h) = mount(scroll(false), 1.);
    let mut a = App::default();
    r.focus_next(false);
    layout(&mut r, 1.);
    r.pointer_down_outcome(235., 12., &mut h, &mut a).unwrap();
    assert!(r.key_down());
    r.key_up_outcome(&mut h, &mut a).unwrap();
    let old = r.scroll_offset("scroll");
    r.pointer_move_outcome(235., 80., &mut h, &mut a).unwrap();
    layout(&mut r, 1.);
    assert_eq!(r.scroll_offset("scroll"), old);
    assert_eq!(a.clicks, 1);
}
#[test]
fn typing_into_a_mounted_offscreen_focused_editor_reveals_it_and_dispatches() {
    for scale in [1., 2.] {
        let ui: Element<App> = vstack![
            text_field("text")
                .key("field")
                .height(36.)
                .on_change(|a: &mut App, _| a.clicks += 1),
            spacer().height(400.).flex(0.)
        ]
        .key("scroller")
        .height(100.)
        .scroll(Scroll::Vertical)
        .into();
        let (mut r, mut h) = mount(ui, scale);
        let mut app = App::default();
        r.focus_next(false);
        assert_eq!(r.focused(), Some("field"));
        assert!(r.scroll_at(20. * scale, 20. * scale, [0., 200.]));
        layout(&mut r, scale);
        assert!(!r.boxes().iter().any(|b| b.id == "field"));
        assert!(r.editor_candidate_bounds().is_none());
        assert!(r.text_input("!", &mut h, &mut app).unwrap().activated);
        layout(&mut r, scale);
        assert_eq!(app.clicks, 1);
        assert!(r.boxes().iter().any(|b| b.id == "field"));
        assert_eq!(r.scroll_offset("scroller")[1], 0.);
    }
}

fn transcript(lines: usize) -> Element<App> {
    vstack((0..lines).map(|i| {
        Element::from(
            text(format!("Line {i}"))
                .key(format!("line-{i}"))
                .height(20.),
        )
    }))
    .key("log")
    .height(100.)
    .width(240.)
    .scroll(Scroll::Vertical)
    .anchor_end(true)
    .into()
}
fn grow(r: &mut Runtime, lines: usize) {
    let (d, h) = transcript(lines).build().unwrap().into_parts();
    r.replace_document(d, &h, &Bindings::default()).unwrap();
    layout(r, 1.);
}
#[test]
fn end_anchored_scroll_follows_growth_until_the_reader_scrolls_away() {
    let (mut r, _) = mount(transcript(10), 1.);
    // New views start at the end: 10 × 20 − 100.
    assert_eq!(r.scroll_offset("log")[1], 100.);
    grow(&mut r, 12);
    assert_eq!(r.scroll_offset("log")[1], 140.);
    // Scrolling up releases the anchor; growth no longer moves the view.
    assert!(r.scroll_at(20., 20., [0., -60.]));
    layout(&mut r, 1.);
    grow(&mut r, 15);
    assert_eq!(r.scroll_offset("log")[1], 80.);
    // Returning to the end re-engages it.
    assert!(r.scroll_at(20., 20., [0., 1000.]));
    layout(&mut r, 1.);
    assert_eq!(r.scroll_offset("log")[1], 200.);
    grow(&mut r, 20);
    assert_eq!(r.scroll_offset("log")[1], 300.);
}
#[test]
fn keys_scroll_a_line_or_a_page_of_the_view_at_the_point() {
    for scale in [1., 2.] {
        let ui: Element<App> = vstack![spacer().height(1000.).flex(0.)]
            .key("page")
            .height(140.)
            .scroll(Scroll::Vertical)
            .into();
        let (r, _) = mount(ui, scale);
        // A line, and the view's height less a line.
        assert_eq!(r.key_scroll_steps(20. * scale, 20. * scale), [40., 100.]);
        // Outside any scroll view a page is ten lines.
        assert_eq!(r.key_scroll_steps(20. * scale, 150. * scale), [40., 400.]);
    }
}

#[test]
fn transparent_overlay_wrappers_do_not_block_scrollbar_drags_but_painted_surfaces_do() {
    for scale in [1., 2.] {
        for blocked in [false, true] {
            let layer = vstack([spacer().into()])
                .width(300.)
                .height(160.)
                .background(if blocked { "#202020" } else { "#00000000" });
            let (mut r, mut h) = mount(zstack([scroll(false), layer.into()]).into(), scale);
            let mut a = App::default();
            let b = r.boxes().iter().find(|b| b.id == "scroll").unwrap();
            let [bx, by, bw, bh] = b.bounds.bounds();
            let track = bh - 12. * scale;
            let length = (track * bh / (bh + b.scroll_max[1]))
                .max(16. * scale)
                .min(track);
            let (x, y) = (bx + bw - 4. * scale, by + 6. * scale + length / 2.);
            r.pointer_down_outcome(x, y, &mut h, &mut a).unwrap();
            r.pointer_move_outcome(x, y + 54. * scale, &mut h, &mut a)
                .unwrap();
            assert_eq!(r.scroll_offset("scroll")[1] > 0., !blocked);
            r.pointer_up_outcome(x, y + 54. * scale, &mut h, &mut a)
                .unwrap();
        }
    }
}
fn keyboard_owner(with_row: bool, handler: bool) -> Element<App> {
    let owner = vstack(if with_row {
        vec![button("row")
            .key("row")
            .height(30.)
            .on_click(|_: &mut App| {})
            .into()]
    } else {
        vec![]
    })
    .key("keyboard-owner")
    .width(240.)
    .height(100.);
    if handler {
        owner
            .on_input_key(|a: &mut App, _| {
                a.clicks += 1;
                true
            })
            .into()
    } else {
        owner.into()
    }
}
#[test]
fn removing_a_virtualized_focused_child_retains_its_surviving_keyboard_owner() {
    for enabled in [false, true] {
        let (mut r, mut h) = mount(keyboard_owner(true, true), 1.);
        let mut a = App::default();
        r.pointer_down_outcome(20., 15., &mut h, &mut a).unwrap();
        r.pointer_up_outcome(20., 15., &mut h, &mut a).unwrap();
        assert_eq!(r.focused(), Some("row"));
        let (d, next) = keyboard_owner(false, enabled).build().unwrap().into_parts();
        r.replace_document(d, &next, &Bindings::default()).unwrap();
        h = next;
        layout(&mut r, 1.);
        assert_eq!(r.focused(), enabled.then_some("keyboard-owner"));
        let outcome = r
            .input_key(
                InputKeyEvent {
                    key: InputKey::ArrowDown,
                    modifiers: Default::default(),
                    repeat: true,
                },
                &mut h,
                &mut a,
            )
            .unwrap();
        assert_eq!(outcome.activated, enabled);
    }
}

#[test]
fn explicit_text_field_focus_epochs_do_not_steal_focus_on_ordinary_recomposition() {
    let fields = |epoch| -> Element<App> {
        vstack([
            text_area("draft")
                .on_change(|_: &mut App, _| {})
                .key("reply")
                .height(50.)
                .focus_request(epoch)
                .into(),
            button("Elsewhere")
                .key("elsewhere")
                .height(30.)
                .on_click(|_: &mut App| {})
                .into(),
        ])
        .width(240.)
        .height(100.)
        .into()
    };
    let (mut r, _) = mount(fields(1), 1.);
    assert_eq!(r.focused(), Some("reply"));
    r.focus_next(false);
    assert_eq!(r.focused(), Some("elsewhere"));
    let (d, h) = fields(1).build().unwrap().into_parts();
    r.replace_document(d, &h, &Bindings::default()).unwrap();
    layout(&mut r, 1.);
    assert_eq!(r.focused(), Some("elsewhere"));
    let (d, h) = fields(2).build().unwrap().into_parts();
    r.replace_document(d, &h, &Bindings::default()).unwrap();
    layout(&mut r, 1.);
    assert_eq!(r.focused(), Some("reply"));
    assert_eq!(r.editor_text("reply").as_deref(), Some("draft"));
}

fn counted_button(key: &str) -> Element<App> {
    button("row")
        .key(key)
        .width(120.)
        .height(30.)
        .on_click_counted(|a: &mut App, event, count| {
            assert_eq!(event.source, InputSource::Pointer);
            a.clicks = a.clicks * 10 + usize::from(count);
        })
        .into()
}
fn counted_click(
    r: &mut Runtime,
    h: &mut Handlers<'static, App>,
    a: &mut App,
    x: f32,
    count: u8,
    mods: InputModifiers,
) {
    r.pointer_down_outcome_with_clicks(x, 15., mods, count, h, a)
        .unwrap();
    r.pointer_up_outcome(x, 15., h, a).unwrap();
}
#[test]
fn counted_pointer_clicks_survive_recomposition_but_require_same_target_and_modifiers() {
    let fields = || -> Element<App> {
        hstack([counted_button("a"), counted_button("b")])
            .width(240.)
            .height(30.)
            .into()
    };
    let (mut r, mut h) = mount(fields(), 1.);
    let mut a = App::default();
    counted_click(&mut r, &mut h, &mut a, 20., 1, Default::default());
    let (d, next) = fields().build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    h = next;
    layout(&mut r, 1.);
    counted_click(&mut r, &mut h, &mut a, 20., 2, Default::default());
    assert_eq!(a.clicks, 12);
    a.clicks = 0;
    counted_click(&mut r, &mut h, &mut a, 160., 2, Default::default());
    assert_eq!(a.clicks, 1, "different row is not a double click");
    counted_click(
        &mut r,
        &mut h,
        &mut a,
        160.,
        2,
        InputModifiers {
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(a.clicks, 11);
    r.pointer_down_outcome_with_clicks(160., 15., Default::default(), 1, &mut h, &mut a)
        .unwrap();
    r.pointer_up_outcome(400., 150., &mut h, &mut a).unwrap();
    counted_click(&mut r, &mut h, &mut a, 160., 2, Default::default());
    assert_eq!(
        a.clicks, 111,
        "cancelled release must disarm double activation"
    );
}
fn reveal_list(row: usize, request: u32, minimal: bool) -> Element<App> {
    let list = lazy_list(100, |i| {
        button(format!("Row {i}"))
            .key(format!("reveal-row-{i}"))
            .width(500.)
            .height(20.)
            .on_click(|_: &mut App| {})
            .into()
    })
    .key("reveal-list")
    .width(500.)
    .height(100.)
    .estimate(20.)
    .gap(0.)
    .window(0..100)
    .on_window_change(|_: &mut App, _| {});
    let list: Element<App> = if minimal {
        list.reveal_row(row, request).into()
    } else {
        list.scroll_to_row(row, request).into()
    };
    vstack([list])
        .key("reveal-horizontal")
        .width(240.)
        .height(100.)
        .scroll(Scroll::Horizontal)
        .into()
}
#[test]
fn minimal_lazy_row_reveal_keeps_visible_rows_and_horizontal_offset_in_both_scales() {
    for scale in [1., 2.] {
        let (mut r, _) = mount(reveal_list(0, 0, true), scale);
        for (row, request, expected) in [(3, 1, 0.), (5, 2, 20.), (4, 3, 20.), (0, 4, 0.)] {
            let (d, h) = reveal_list(row, request, true)
                .build()
                .unwrap()
                .into_parts();
            r.replace_document(d, &h, &Bindings::default()).unwrap();
            layout(&mut r, scale);
            assert!((r.scroll_offset("reveal-list")[1] - expected).abs() < 0.1);
        }
        assert!(r.scroll_at(100. * scale, 40. * scale, [50., 250.]));
        layout(&mut r, scale);
        let before = r.scroll_offset("reveal-list");
        let horizontal = r.scroll_offset("reveal-horizontal");
        assert!(horizontal[0] > 0.);
        let (d, h) = reveal_list(13, 5, true).build().unwrap().into_parts();
        r.replace_document(d, &h, &Bindings::default()).unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("reveal-list"), before);
        assert_eq!(r.scroll_offset("reveal-horizontal"), horizontal);
        let (d, h) = reveal_list(13, 6, false).build().unwrap().into_parts();
        r.replace_document(d, &h, &Bindings::default()).unwrap();
        layout(&mut r, scale);
        assert_eq!(
            r.scroll_offset("reveal-list"),
            [0., 260.],
            "legacy explicit top alignment unchanged"
        );
    }
}
