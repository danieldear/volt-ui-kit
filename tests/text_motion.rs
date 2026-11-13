#![cfg(feature = "rust-ui")]
use std::time::Duration;
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Runtime, TextMetrics},
    ui::prelude::*,
    Painter, Theme, Viewport,
};
struct Recorder;
impl Painter for Recorder {
    type Shape = ();
    type Glyph = String;
    fn viewport(&self) -> Viewport {
        Viewport::new(500., 200., 1.).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<()>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<()>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(&mut self, _: &mut Vec<String>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<String>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl TextMetrics for Recorder {
    fn measure(&mut self, text: &str, _: f32, _: f32) -> [f32; 2] {
        [text.chars().count() as f32 * 10., 20.]
    }
}
impl DocumentPainter for Recorder {
    fn ui_text(&mut self, out: &mut Vec<String>, text: &str, _: [f32; 4], _: f32, _: [f32; 4]) {
        out.push(text.into());
    }
}
fn mount(tree: Element<()>) -> Runtime {
    let (d, h) = tree.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(Recorder.viewport(), &Bindings::default(), &mut Recorder)
        .unwrap();
    r
}
fn draws(r: &Runtime) -> Vec<String> {
    r.paint(&mut Recorder, &Theme::dark())
        .into_iter()
        .flat_map(|b| b.glyphs)
        .collect()
}
#[test]
fn typing_uses_graphemes_keeps_full_semantics_and_returns_to_idle() {
    let full = "A👨‍👩‍👧‍👦e\u{301}🇺🇸";
    let mut r = mount(
        typing_text(full)
            .key("label")
            .text_effect(TextEffect::typing().duration_ms(1000))
            .into(),
    );
    r.advance_animations(Duration::from_millis(500));
    assert_eq!(draws(&r), ["A👨‍👩‍👧‍👦"]);
    assert_eq!(
        r.accessibility_nodes()
            .into_iter()
            .find(|n| n.id == "label")
            .unwrap()
            .label,
        full
    );
    r.advance_animations(Duration::from_secs(2));
    assert_eq!(draws(&r), [full]);
    assert_eq!(r.animation_wait(), None);
}
#[test]
fn reduced_motion_displays_full_text_and_stops_shimmer() {
    let mut r = mount(
        vstack![
            typing_text("Hello").key("typing"),
            shimmer_text("Ready").key("shine")
        ]
        .into(),
    );
    assert!(r.is_animating());
    r.set_reduced_motion(true);
    assert_eq!(draws(&r), ["Hello", "Ready"]);
    assert_eq!(r.animation_wait(), None);
}
#[test]
fn morph_and_roll_wait_for_a_change_instead_of_running_on_mount() {
    for tree in [
        morphing_text::<()>("Ready").into(),
        rolling_number::<()>(42).into(),
    ] {
        let r = mount(tree);
        assert!(!r.is_animating());
    }
}

#[test]
fn offscreen_shimmer_and_ambient_sleep_then_resume_after_scrolling() {
    let mut r = mount(
        vstack![
            spacer().height(300.),
            shimmer_text("Below the fold")
                .key("shine")
                .height(40.)
                .ambient(Ambient::float(0., 1.)),
            spacer().height(200.)
        ]
        .key("scroll")
        .height(180.)
        .scroll(Scroll::Vertical)
        .transform(Transform2D::new())
        .into(),
    );
    assert_eq!(
        r.animation_wait(),
        None,
        "offscreen effects must not redraw forever"
    );
    assert!(r.scroll_at(20., 20., [0., 280.]));
    r.layout(Recorder.viewport(), &Bindings::default(), &mut Recorder)
        .unwrap();
    assert!(r.is_animating(), "visible shimmer resumes");
    assert!(r.scroll_at(20., 20., [0., -280.]));
    r.layout(Recorder.viewport(), &Bindings::default(), &mut Recorder)
        .unwrap();
    assert_eq!(r.animation_wait(), None);
}
