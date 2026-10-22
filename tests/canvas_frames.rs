#![cfg(feature = "rust-ui")]
//! Frame sequences play at paint time, once per revision, then rest.
use std::time::Duration;
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics},
    ui::{prelude::*, CanvasFrames},
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, s: f32, max: f32) -> [f32; 2] {
        [(t.len() as f32 * s * 0.5).min(max), s * 1.2]
    }
}
fn frames(revision: u64) -> Element<()> {
    let frame = |w: f32| {
        let mut c = Canvas::default();
        c.fill(VectorPath::rectangle(0., 0., w, 10.), "#ff0000");
        c
    };
    vstack![
        canvas_frames(vec![frame(10.), frame(20.), frame(30.)], 30, revision)
            .key("tween")
            .width(40.)
            .height(10.)
    ]
    .key("page")
    .into()
}
fn layout(rt: &mut Runtime) {
    rt.layout(
        Viewport::new(100., 40., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
}
#[test]
fn frames_play_once_per_revision_then_rest() {
    let (d, h) = frames(1).build().unwrap().into_parts();
    let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
    rt.advance_animations(Duration::from_millis(1000));
    layout(&mut rt);
    assert!(rt.is_animating(), "a new sequence plays");
    rt.advance_animations(Duration::from_millis(1050));
    assert!(rt.is_animating(), "still in its 90 ms");
    rt.advance_animations(Duration::from_millis(1100));
    assert!(
        !rt.is_animating(),
        "done: the last frame stays, no more frames"
    );
    // Recomposing the same revision doesn't replay it; a new one does.
    let (d, h) = frames(1).build().unwrap().into_parts();
    rt.replace_document(d, &h, &Bindings::default()).unwrap();
    layout(&mut rt);
    assert!(!rt.is_animating());
    let (d, h) = frames(2).build().unwrap().into_parts();
    rt.replace_document(d, &h, &Bindings::default()).unwrap();
    layout(&mut rt);
    assert!(rt.is_animating(), "new data, new tween");
    // Reduced motion shows the final frame at once.
    rt.set_reduced_motion(true);
    assert!(!rt.is_animating());
}
#[test]
fn a_frame_is_chosen_by_time_and_the_last_one_holds() {
    let mut c = Canvas::default();
    c.fill(VectorPath::rectangle(0., 0., 1., 1.), "#000000");
    let f = CanvasFrames {
        frames: vec![Canvas::default(), c.clone()].into(),
        frame_ms: 30,
        revision: 0,
    };
    assert_eq!(f.duration(), Duration::from_millis(60));
    assert!(f.at(Duration::from_millis(10)).unwrap().paths.is_empty());
    assert_eq!(f.at(Duration::from_millis(45)), Some(&c));
    assert_eq!(f.at(Duration::from_secs(9)), Some(&c));
}
