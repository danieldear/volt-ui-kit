#![cfg(feature = "rust-ui")]
use std::time::Duration;
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Painter, Theme, Viewport,
};
#[derive(Default)]
struct Recorder;
impl TextMetrics for Recorder {
    fn measure(&mut self, _: &str, _: f32, _: f32) -> [f32; 2] {
        [40., 20.]
    }
}
impl Painter for Recorder {
    type Shape = f32;
    type Glyph = (String, [f32; 4]);
    fn viewport(&self) -> Viewport {
        Viewport::new(400., 300., 1.).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<f32>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<f32>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(
        &mut self,
        _: &mut Vec<Self::Glyph>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: [f32; 4],
    ) {
    }
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<Self::Glyph>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for Recorder {
    fn ui_text(
        &mut self,
        out: &mut Vec<Self::Glyph>,
        text: &str,
        bounds: [f32; 4],
        _: f32,
        _: [f32; 4],
    ) {
        out.push((text.into(), bounds));
    }
    fn ui_layer_begin(&mut self, out: &mut Vec<f32>, opacity: f32) {
        out.push(opacity);
    }
}
fn mount(ui: Element<usize>, scale: f32) -> (Runtime, Handlers<'static, usize>) {
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(400. * scale, 300. * scale, scale).unwrap(),
        &Bindings::default(),
        &mut Recorder,
    )
    .unwrap();
    (r, h)
}
fn replace(r: &mut Runtime, h: &mut Handlers<'static, usize>, ui: Element<usize>, scale: f32) {
    let (doc, next) = ui.build().unwrap().into_parts();
    r.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    *h = next;
    r.layout(
        Viewport::new(400. * scale, 300. * scale, scale).unwrap(),
        &Bindings::default(),
        &mut Recorder,
    )
    .unwrap();
}
fn frame(r: &Runtime) -> (Vec<f32>, Vec<(String, [f32; 4])>) {
    let frame = r.paint(&mut Recorder, &Theme::dark());
    (
        frame
            .iter()
            .flat_map(|b| b.shapes.iter().copied())
            .collect(),
        frame.into_iter().flat_map(|b| b.glyphs).collect(),
    )
}
fn at(r: &Runtime, name: &str) -> [f32; 4] {
    frame(r)
        .1
        .into_iter()
        .find(|(text, _)| text == name)
        .unwrap()
        .1
}
#[test]
fn delayed_state_animation_keeps_scheduler_alive_and_settles_after_delay_plus_duration() {
    let (mut r, _) = mount(
        button("Hover")
            .key("hover")
            .width(100.)
            .height(40.)
            .hover_style(|s| s.opacity(0.2))
            .animate_on(
                Interaction::Hover,
                Animation::linear().delay_ms(200).duration_ms(100),
            )
            .on_click(|_: &mut usize| {})
            .into(),
        1.,
    );
    frame(&r);
    r.pointer_move(50., 20.);
    frame(&r);
    r.advance_animations(Duration::from_millis(150));
    assert!(r.is_animating());
    assert!(frame(&r).0.is_empty());
    r.advance_animations(Duration::from_millis(250));
    assert!((frame(&r).0[0] - 0.6).abs() < 0.001);
    assert!(r.is_animating());
    assert!(r.advance_animations(Duration::from_millis(300)));
    assert!(!r.is_animating());
    assert!(!r.advance_animations(Duration::from_millis(301)));
}
fn replay(revision: u64) -> Element<usize> {
    text("Replay")
        .key("replay")
        .transition(
            Transition::slide(0., 20.)
                .duration_ms(200)
                .easing(Easing::Linear),
        )
        .transition_revision(revision)
        .into()
}
#[test]
fn replay_revision_is_stable_and_does_not_relayout_or_reidentify_controls() {
    let (mut r, mut h) = mount(replay(0), 1.);
    r.advance_animations(Duration::from_millis(200));
    assert!(!r.is_animating());
    let settled = at(&r, "Replay");
    replace(&mut r, &mut h, replay(0), 1.);
    assert!(!r.is_animating());
    replace(&mut r, &mut h, replay(1), 1.);
    assert!(r.is_animating());
    assert!((at(&r, "Replay")[1] - settled[1] - 20.).abs() < 0.01);
    r.advance_animations(Duration::from_millis(300));
    let halfway = at(&r, "Replay");
    replace(&mut r, &mut h, replay(1), 1.);
    assert_eq!(at(&r, "Replay"), halfway);
    r.advance_animations(Duration::from_millis(400));
    assert_eq!(at(&r, "Replay"), settled);
    assert!(!r.is_animating());
}
#[test]
fn spring_overshoots_position_but_not_opacity_and_reduced_motion_stops_it() {
    for scale in [1., 2.] {
        let ui = text("Spring")
            .key("spring")
            .transition(Transition::slide(0., 40.).animation(Animation::spring().duration_ms(500)));
        let (mut r, _) = mount(ui.into(), scale);
        let bounds = r.boxes()[0].bounds.bounds();
        let start = at(&r, "Spring")[1];
        let mut overshot = false;
        for time in (20..500).step_by(20) {
            r.advance_animations(Duration::from_millis(time));
            let (layers, _) = frame(&r);
            assert!(layers.iter().all(|v| (0. ..=1.).contains(v)));
            if at(&r, "Spring")[1] < start - 40. * scale - 0.001 {
                overshot = true;
            }
            assert!(r.is_animating());
            assert_eq!(r.boxes()[0].bounds.bounds(), bounds);
        }
        assert!(overshot);
        r.set_reduced_motion(true);
        assert!(!r.is_animating());
        assert_eq!(at(&r, "Spring")[1], start - 40. * scale);
    }
}
#[test]
fn stagger_uses_full_clock_and_overshooting_bezier_does_not_prune_early() {
    let ui = vstack![
        text("A")
            .key("a")
            .transition(Transition::fade().duration_ms(100)),
        text("B")
            .key("b")
            .transition(Transition::fade().duration_ms(100).stagger(2, 100))
    ];
    let (mut r, _) = mount(ui.into(), 1.);
    r.advance_animations(Duration::from_millis(150));
    assert!(r.is_animating());
    assert!(frame(&r).0.contains(&0.));
    r.advance_animations(Duration::from_millis(300));
    assert!(!r.is_animating());
    let (mut r, _) = mount(
        text("Overshoot")
            .key("overshoot")
            .transition(
                Transition::fade()
                    .duration_ms(500)
                    .easing(Easing::CubicBezier(0.2, 2., 0.8, 2.)),
            )
            .into(),
        1.,
    );
    r.advance_animations(Duration::from_millis(250));
    assert!(r.is_animating());
    assert!(frame(&r).0.iter().all(|v| (0. ..=1.).contains(v)));
    r.advance_animations(Duration::from_millis(500));
    assert!(!r.is_animating());
}
#[test]
fn removed_slide_moves_out_and_cannot_dispatch_even_if_it_is_still_painted() {
    for scale in [1., 2.] {
        let ui = vstack![button("Delete")
            .key("delete")
            .height(40.)
            .width(100.)
            .transition(
                Transition::slide(0., 20.)
                    .duration_ms(200)
                    .easing(Easing::Linear)
            )
            .on_click(|n: &mut usize| *n += 1)];
        let (mut r, mut h) = mount(ui.into(), scale);
        r.advance_animations(Duration::from_millis(200));
        frame(&r);
        let old = at(&r, "Delete");
        replace(
            &mut r,
            &mut h,
            vstack![text("Gone").key("gone")].into(),
            scale,
        );
        r.advance_animations(Duration::from_millis(300));
        let painted = at(&r, "Delete");
        assert!((painted[1] - old[1] - 10. * scale).abs() < 0.01);
        let mut count = 0;
        assert!(
            !r.accessibility_activate("delete", &mut h, &mut count)
                .unwrap()
                .activated
        );
        assert_eq!(count, 0);
        r.advance_animations(Duration::from_millis(400));
        assert!(!r.is_animating());
        assert!(!frame(&r).1.iter().any(|(t, _)| t == "Delete"));
    }
}
#[test]
fn invalid_spring_stagger_and_zero_duration_delay_are_handled() {
    for ui in [
        text("Bad")
            .animation(Animation::spring_with(
                Spring::new().damping_ratio(f32::NAN),
            ))
            .into(),
        text("Bad")
            .transition(Transition::fade().stagger(usize::MAX, 1000))
            .into(),
    ] {
        let ui: Element<usize> = ui;
        assert!(ui.build().is_err());
    }
    let (mut r, _) = mount(
        text("Instant")
            .transition(Transition::fade().duration_ms(0).delay_ms(100))
            .into(),
        1.,
    );
    assert!(r.is_animating());
    r.advance_animations(Duration::from_millis(99));
    assert_eq!(frame(&r).0, vec![0.]);
    r.advance_animations(Duration::from_millis(100));
    assert!(!r.is_animating());
    assert!(frame(&r).0.is_empty());
}

#[test]
fn offset_retargets_back_to_zero_without_snapping_and_keeps_focus_and_handlers() {
    let view = |offset| -> Element<usize> {
        button("Move")
            .key("move")
            .width(100.)
            .height(40.)
            .offset(offset, 0.)
            .animation(Animation::linear().duration_ms(200))
            .on_click(|n: &mut usize| *n += 1)
            .into()
    };
    let (mut r, mut h) = mount(view(0.), 1.);
    frame(&r);
    assert!(r.accessibility_focus("move"));
    let original = at(&r, "Move");
    replace(&mut r, &mut h, view(80.), 1.);
    frame(&r);
    r.advance_animations(Duration::from_millis(200));
    assert!((at(&r, "Move")[0] - original[0] - 80.).abs() < 0.01);
    replace(&mut r, &mut h, view(0.), 1.);
    assert!((at(&r, "Move")[0] - original[0] - 80.).abs() < 0.01);
    r.advance_animations(Duration::from_millis(300));
    assert!((at(&r, "Move")[0] - original[0] - 40.).abs() < 0.01);
    assert_eq!(r.focused(), Some("move"));
    let mut count = 0;
    assert!(
        r.accessibility_activate("move", &mut h, &mut count)
            .unwrap()
            .activated
    );
    assert_eq!(count, 1);
    r.advance_animations(Duration::from_millis(400));
    assert_eq!(at(&r, "Move")[0], original[0]);
    assert!(!r.is_animating());
}

#[test]
fn replay_does_not_reset_editor_selection_text_or_keyboard_focus() {
    let view = |revision| -> Element<usize> {
        vstack![text_field("Keep my draft")
            .key("draft")
            .on_change(|n: &mut usize, _| *n += 1)]
        .key("editor-panel")
        .transition(Transition::fade().duration_ms(200))
        .transition_revision(revision)
        .into()
    };
    let (mut r, mut h) = mount(view(0), 1.);
    r.settle_animations();
    assert!(r.accessibility_focus("draft"));
    assert!(r.editor_select_all());
    let selection = r.editor_selection("draft");
    let text = r.editor_text("draft");
    replace(&mut r, &mut h, view(1), 1.);
    assert_eq!(r.focused(), Some("draft"));
    assert_eq!(r.editor_selection("draft"), selection);
    assert_eq!(r.editor_text("draft"), text);
    r.set_reduced_motion(true);
    assert!(!r.is_animating());
}

#[test]
fn delayed_only_scenes_can_sleep_until_the_first_useful_frame() {
    let (mut r, _) = mount(
        text("Wait")
            .key("wait")
            .transition(Transition::fade().duration_ms(200).delay_ms(1000))
            .into(),
        1.,
    );
    assert_eq!(r.animation_wait(), Some(Duration::from_millis(1000)));
    r.advance_animations(Duration::from_millis(400));
    assert_eq!(r.animation_wait(), Some(Duration::from_millis(600)));
    r.advance_animations(Duration::from_millis(1000));
    assert_eq!(r.animation_wait(), Some(Duration::ZERO));
    r.advance_animations(Duration::from_millis(1200));
    assert_eq!(r.animation_wait(), None);
    assert!(!r.is_animating());
    let (mut r, _) = mount(replay(0), 1.);
    r.set_reduced_motion(true);
    assert_eq!(r.animation_wait(), None);
}

#[test]
fn a_completed_entrance_can_present_its_endpoint_before_the_next_stagger() {
    let (mut r, _) = mount(
        vstack![
            text("First")
                .key("first")
                .transition(Transition::fade().duration_ms(100)),
            text("Second")
                .key("second")
                .transition(Transition::fade().duration_ms(100).delay_ms(500)),
        ]
        .into(),
        1.,
    );
    assert_eq!(r.animation_wait(), Some(Duration::ZERO));
    assert!(r.advance_animations(Duration::from_millis(100)));
    assert_eq!(r.animation_wait(), Some(Duration::from_millis(400)));
    assert!(r.is_animating());
    r.advance_animations(Duration::from_millis(500));
    assert_eq!(r.animation_wait(), Some(Duration::ZERO));
    r.advance_animations(Duration::from_millis(600));
    assert_eq!(r.animation_wait(), None);
}

#[test]
fn themed_offset_paints_do_not_retarget_the_same_animation_forever() {
    for theme in [UiTheme::light(), UiTheme::dark()] {
        let view = |moved| -> Element<usize> {
            panel![text("Themed motion").color(ColorToken::Primary)]
                .key("themed-motion")
                .background(if moved {
                    ColorToken::Primary.alpha(13)
                } else {
                    ColorToken::Base100.into()
                })
                .border_color(ColorToken::Primary)
                .offset(if moved { 64. } else { 0. }, 0.)
                .animation(Animation::spring().duration_ms(200))
                .into()
        };
        for scale in [1., 2.] {
            let (mut r, mut h) = mount(view(false), scale);
            r.paint_ui(&mut Recorder, &theme);
            for (moved, start) in [(true, 0), (false, 800)] {
                replace(&mut r, &mut h, view(moved), scale);
                for time in (start..=start + 800).step_by(16) {
                    r.advance_animations(Duration::from_millis(time));
                    r.paint_ui(&mut Recorder, &theme);
                }
                assert_eq!(
                    r.animation_wait(),
                    None,
                    "paint passes must agree on colors at scale {scale}, moved={moved}"
                );
            }
        }
    }
}
