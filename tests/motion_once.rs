#![cfg(feature = "native")]
//! Entrances that don't fade out over the next page (`enter_only`), and
//! entrances and draw-ins that play once per key (`once`,
//! `canvas_frames_once`): coming back shows them in place.
use std::time::Duration;
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Painter, Theme, Viewport,
};

fn setup(element: Element<()>) -> (Runtime, RasterPainter) {
    let viewport = Viewport::new(100., 80., 1.).unwrap();
    let mut painter = RasterPainter::new(viewport);
    let (document, handlers) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    (runtime, painter)
}
fn replace(runtime: &mut Runtime, painter: &mut RasterPainter, element: Element<()>) {
    let (document, handlers) = element.build().unwrap().into_parts();
    runtime
        .replace_document(document, &handlers, &Bindings::default())
        .unwrap();
    runtime
        .layout(painter.viewport(), &Bindings::default(), painter)
        .unwrap();
}
fn red(runtime: &Runtime, painter: &mut RasterPainter) -> u8 {
    let batches = runtime.paint(painter, &Theme::dark());
    let frame = painter.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
    frame.pixels()[20 * frame.width() as usize + 30].red()
}
fn card(present: bool, transition: Transition) -> Element<()> {
    let children = if present {
        vec![panel![]
            .key("card")
            .width(60.)
            .height(40.)
            .background("#ff0000")
            .border_width(0.)
            .transition(transition.duration_ms(100))
            .into()]
    } else {
        vec![]
    };
    vstack(children).key("page").into()
}

#[test]
fn enter_only_leaves_at_once_and_plain_fades_out() {
    for (transition, lingers) in [
        (Transition::fade(), true),
        (Transition::fade().enter_only(), false),
    ] {
        let (mut rt, mut p) = setup(card(true, transition));
        red(&rt, &mut p);
        rt.advance_animations(Duration::from_millis(150));
        assert_eq!(red(&rt, &mut p), 255);
        replace(&mut rt, &mut p, card(false, transition));
        rt.advance_animations(Duration::from_millis(30));
        assert_eq!(red(&rt, &mut p) > 100, lingers, "{transition:?}");
    }
}

#[test]
fn once_shows_a_returning_card_in_place() {
    for (transition, fades_again) in [
        (Transition::fade(), true),
        (Transition::fade().once(), false),
    ] {
        let (mut rt, mut p) = setup(card(true, transition));
        assert!(red(&rt, &mut p) < 30, "the first entrance plays");
        rt.advance_animations(Duration::from_millis(150));
        replace(&mut rt, &mut p, card(false, transition));
        rt.advance_animations(Duration::from_millis(300));
        replace(&mut rt, &mut p, card(true, transition));
        assert_eq!(red(&rt, &mut p) < 30, fades_again, "{transition:?}");
    }
}

fn drawing(present: bool, once: bool, revision: u64) -> Element<()> {
    let frame = |on: bool| {
        let mut c = Canvas::default();
        if on {
            c.fill(VectorPath::rectangle(0., 0., 60., 40.), "#ff0000");
        } else {
            c.fill(VectorPath::rectangle(0., 0., 1., 1.), "#000000");
        }
        c
    };
    let frames = vec![frame(false), frame(false), frame(true)];
    let children = if present {
        let c: Element<()> = if once {
            canvas_frames_once(frames, 40, revision)
        } else {
            canvas_frames(frames, 40, revision)
        };
        vec![c.key("drawing").width(60.).height(40.)]
    } else {
        vec![]
    };
    vstack(children).key("page").into()
}

#[test]
fn canvas_frames_once_finish_at_once_when_they_come_back() {
    for once in [false, true] {
        let (mut rt, mut p) = setup(drawing(true, once, 1));
        assert!(red(&rt, &mut p) < 30, "the first draw-in plays");
        rt.advance_animations(Duration::from_millis(200));
        assert_eq!(red(&rt, &mut p), 255);
        replace(&mut rt, &mut p, drawing(false, once, 1));
        replace(&mut rt, &mut p, drawing(true, once, 1));
        assert_eq!(red(&rt, &mut p) == 255, once, "once: {once}");
        // New data plays again either way.
        rt.advance_animations(Duration::from_millis(200));
        replace(&mut rt, &mut p, drawing(true, once, 2));
        assert!(red(&rt, &mut p) < 30, "a new revision plays (once: {once})");
    }
}
