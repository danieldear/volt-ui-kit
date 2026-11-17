#![cfg(feature = "rust-ui")]
//! Canvas drawing beyond plain fills: arcs and ring segments, partial paths
//! for draw-ins, stroke caps/joins/dashes and gradient paint.
use volt_ui_kit::ui::prelude::*;

fn end_point(path: &VectorPath) -> [f32; 2] {
    use volt_ui_kit::graphics::PathCommand;
    match path.commands.last().unwrap() {
        PathCommand::Move(p) | PathCommand::Line(p) => *p,
        PathCommand::Quad(_, p) | PathCommand::Cubic(_, _, p) => *p,
        PathCommand::Close => panic!("open path expected"),
    }
}

fn near(a: [f32; 2], b: [f32; 2], tolerance: f32) -> bool {
    (a[0] - b[0]).abs() <= tolerance && (a[1] - b[1]).abs() <= tolerance
}

#[test]
fn arcs_read_like_a_clock_and_sectors_are_rings() {
    // 0° is 12 o'clock and angles run clockwise.
    let quarter = VectorPath::arc(50., 50., 40., 0., 90.);
    assert!(quarter.is_valid());
    assert_eq!(
        quarter
            .bounds()
            .map(|b| near([b[0], b[1]], [50., 10.], 0.01)),
        Some(true)
    );
    assert!(near(end_point(&quarter), [90., 50.], 0.01));
    // Anticlockwise from 3 o'clock to 12.
    assert!(near(
        end_point(&VectorPath::arc(50., 50., 40., 90., -90.)),
        [50., 10.],
        0.01
    ));
    // A full circle measures 2πr to within a hair (cubic arcs).
    let circle = VectorPath::arc(0., 0., 100., 0., 720.);
    assert!(
        (circle.length() - std::f32::consts::TAU * 100.).abs() < 0.5,
        "{}",
        circle.length()
    );

    let ring = VectorPath::sector(50., 50., 40., 25., 0., 180.);
    assert!(ring.contains(83., 50.), "inside the band on the right");
    assert!(!ring.contains(50., 50.), "the hole");
    assert!(!ring.contains(17., 50.), "the left half isn't swept");
    let pie = VectorPath::sector(50., 50., 40., 0., 0., 90.);
    assert!(pie.contains(55., 45.));
    assert!(!pie.contains(45., 55.));
}

#[test]
fn trimmed_paths_draw_in_by_length() {
    let l = VectorPath::new()
        .move_to(0., 0.)
        .line_to(100., 0.)
        .line_to(100., 100.);
    assert_eq!(l.length(), 200.);
    assert_eq!(end_point(&l.trimmed(0.25)), [50., 0.]);
    assert_eq!(end_point(&l.trimmed(0.75)), [100., 50.]);
    assert_eq!(l.trimmed(1.), l);
    assert_eq!(
        l.trimmed(0.).commands.len(),
        1,
        "only the start: nothing drawn"
    );
    assert_eq!(
        l.trimmed(f32::NAN).commands.len(),
        1,
        "nothing drawn, not a panic"
    );
    // A curve is cut at the matching point, so the drawn part keeps its shape.
    let arc = VectorPath::arc(0., 0., 50., 0., 180.);
    let half = arc.trimmed(0.5);
    assert!(
        near(end_point(&half), [50., 0.], 0.6),
        "{:?}",
        end_point(&half)
    );
    assert!((half.length() - arc.length() / 2.).abs() < 0.5);
}

#[test]
fn stroke_styles_validate_and_repeat_odd_dashes() {
    let dots = StrokeStyle::new(3.).round().dash([0., 8.]);
    assert!(dots.is_valid());
    assert_eq!((dots.cap, dots.join), (LineCap::Round, LineJoin::Round));
    assert_eq!(
        StrokeStyle::new(2.).dash([4., 2., 1.]).dash,
        [4., 2., 1., 4., 2., 1.]
    );
    assert!(!StrokeStyle::new(0.).is_valid());
    assert!(!StrokeStyle::new(2.).dash([0., 0.]).is_valid());
    assert!(!StrokeStyle::new(f32::NAN).is_valid());
    assert_eq!(
        StrokeStyle::new(2.).dash([4., 2.]).scaled(2.).dash,
        [8., 4.]
    );

    let mut c = Canvas::default();
    c.fill_gradient(
        VectorPath::rectangle(0., 0., 10., 10.),
        Gradient::linear(90., [(0., "#ff0000"), (1., "#0000ff")]),
    );
    c.glow_stroke(
        VectorPath::new().move_to(0., 0.).line_to(10., 0.),
        "#4C9BFF",
        StrokeStyle::new(2.),
        1.,
    );
    // The gradient remembers its first colour for painters without gradients;
    // the glow is three halos under the line.
    assert_eq!(c.paths[0].color, "#ff0000");
    assert_eq!(c.paths.len(), 5);
    assert_eq!(c.paths[1].color, "#4C9BFF1A");
    assert_eq!(
        c.paths[4]
            .style
            .as_ref()
            .unwrap()
            .stroke
            .as_ref()
            .unwrap()
            .width,
        2.
    );
}

#[cfg(feature = "native")]
mod pixels {
    use volt_ui_kit::{
        declarative::{Bindings, Runtime},
        native::RasterPainter,
        ui::prelude::*,
        Theme, Viewport,
    };

    fn render(draw: impl FnOnce(&mut Canvas)) -> impl Fn(f32, f32) -> [u8; 4] {
        let viewport = Viewport::new(100., 40., 1.).unwrap();
        let mut painter = RasterPainter::new(viewport);
        let mut c = Canvas::default();
        draw(&mut c);
        let element: Element<()> = vstack![canvas(move |x| *x = c).width(100.).height(40.)]
            .background("#000000")
            .into();
        let (document, handlers) = element.build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        let batches = runtime.paint(&mut painter, &Theme::dark());
        let frame = painter.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
        let width = frame.width() as usize;
        let pixels: Vec<[u8; 4]> = frame
            .pixels()
            .iter()
            .map(|c| [c.red(), c.green(), c.blue(), c.alpha()])
            .collect();
        move |x, y| pixels[y as usize * width + x as usize]
    }

    #[test]
    fn dashes_leave_gaps_and_round_caps_reach_past_the_end() {
        let at = render(|c| {
            c.stroke_with(
                VectorPath::new().move_to(10., 10.).line_to(90., 10.),
                "#ffffff",
                StrokeStyle::new(4.).dash([10., 10.]),
            );
            c.stroke_with(
                VectorPath::new().move_to(10., 30.).line_to(50., 30.),
                "#ffffff",
                StrokeStyle::new(8.).round(),
            );
        });
        assert!(at(15., 10.)[0] > 200, "on");
        assert!(at(25., 10.)[0] < 40, "off");
        assert!(at(35., 10.)[0] > 200, "on again");
        assert!(at(52., 30.)[0] > 150, "the round cap reaches past x = 50");
    }

    #[test]
    fn gradients_paint_fills_and_strokes_across_the_path() {
        let at = render(|c| {
            c.fill_gradient(
                VectorPath::rectangle(10., 2., 80., 16.),
                Gradient::linear(90., [(0., "#ff0000"), (1., "#0000ff")]),
            );
            c.stroke_gradient(
                VectorPath::new().move_to(10., 30.).line_to(90., 30.),
                Gradient::linear(90., [(0., "#00ff00"), (1., "#ff00ff")]),
                StrokeStyle::new(6.),
            );
        });
        let (left, right) = (at(12., 10.), at(88., 10.));
        assert!(left[0] > 200 && left[2] < 60, "{left:?}");
        assert!(right[2] > 200 && right[0] < 60, "{right:?}");
        let (start, end) = (at(12., 30.), at(88., 30.));
        assert!(start[1] > 200 && start[0] < 60, "{start:?}");
        assert!(end[0] > 200 && end[1] < 60, "{end:?}");
    }
}
