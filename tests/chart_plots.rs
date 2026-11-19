#![cfg(feature = "rust-ui")]
//! Line plots with gaps and guides, range charts and timelines: they build,
//! answer a tap with a readout, and draw in once per revision.
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
#[derive(Default)]
struct App {
    day: Option<usize>,
    column: Option<usize>,
    span: Option<usize>,
}
fn hours(v: f32) -> String {
    format!("{:02}:{:02}", (v / 60.) as u32 % 24, v as u32 % 60)
}
fn view(app: &App) -> Element<App> {
    vstack![
        line_plot([
            ("Resting", vec![58., 57., f32::NAN, f32::NAN, 60., 59., 61.]),
            ("Asleep", vec![52., 51., 50., f32::NAN, 54., 53., 52.]),
        ])
        .key("hr")
        .size(400., 220.)
        .x_labels(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"])
        .guide(55., "55")
        .area(true)
        .glow(true)
        .animate(3)
        .readout(true)
        .selected(app.day)
        .on_select(|a: &mut App, i| a.day = Some(i)),
        range_chart([
            ValueRange::new(110., 128.).mid(121.),
            ValueRange::new(112., 131.).mid(119.),
            ValueRange::new(f32::NAN, f32::NAN),
            ValueRange::new(108., 125.),
        ])
        .key("bp")
        .size(400., 180.)
        .x_labels(["Mon", "Tue", "Wed", "Thu"])
        .readout(true)
        .animate(1)
        .selected(app.column)
        .on_select(|a: &mut App, i| a.column = Some(i)),
        timeline_chart(
            [
                ("Awake", "#F59E0B"),
                ("REM", "#A5B4FC"),
                ("Light", "#818CF8"),
                ("Deep", "#4F46E5")
            ],
            [
                (0, 1380., 1395.),
                (2, 1395., 1440.),
                (3, 1440., 1500.),
                (1, 1500., 1530.),
                (2, 1530., 1600.)
            ],
        )
        .key("stages")
        .size(400., 160.)
        .x_labels(["23:00", "00:00", "01:00", "02:00"])
        .value_format(hours)
        .readout(true)
        .selected(app.span)
        .on_select(|a: &mut App, i| a.span = Some(i)),
    ]
    .key("page")
    .gap(16., Axis::Vertical)
    .into()
}
struct Harness {
    app: App,
    rt: Runtime,
    h: Handlers<'static, App>,
}
impl Harness {
    fn new() -> Self {
        let app = App::default();
        let (d, h) = view(&app).build().unwrap().into_parts();
        let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
        rt.layout(
            Viewport::new(600., 900., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
        Self { app, rt, h }
    }
    fn rebuild(&mut self) {
        let (d, h) = view(&self.app).build().unwrap().into_parts();
        self.rt
            .replace_document(d, &h, &Bindings::default())
            .unwrap();
        self.h = h;
        self.rt
            .layout(
                Viewport::new(600., 900., 1.).unwrap(),
                &Bindings::default(),
                &mut Metrics,
            )
            .unwrap();
    }
    fn click(&mut self, id: &str) -> bool {
        let b = self
            .rt
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("no {id}"))
            .bounds
            .bounds();
        let (x, y) = (b[0] + b[2] / 2., b[1] + b[3] / 2.);
        self.rt.pointer_down(x, y);
        let o = self
            .rt
            .pointer_up_outcome(x, y, &mut self.h, &mut self.app)
            .unwrap();
        self.rebuild();
        o.activated
    }
    fn label(&self, id: &str) -> String {
        self.rt
            .accessibility_nodes()
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.label.clone())
            .unwrap_or_else(|| panic!("no label for {id}"))
    }
    fn texts(&self) -> String {
        self.rt
            .accessibility_nodes()
            .iter()
            .map(|n| n.label.clone())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

#[test]
fn a_tap_reads_a_point_and_says_what_is_missing() {
    let mut t = Harness::new();
    assert!(t.texts().contains("Tap the chart to read a value"));
    assert_eq!(t.label("hr-3"), "Thu, Resting: no value, Asleep: no value");
    assert!(t.click("hr-4"));
    assert_eq!(t.app.day, Some(4));
    assert!(
        t.texts().contains("Fri · Resting: 60 · Asleep: 54"),
        "{}",
        t.texts()
    );

    assert_eq!(t.label("bp-2"), "Wed, no value");
    assert!(t.click("bp-0"));
    assert!(
        t.texts().contains("Mon · 110 to 128, middle 121"),
        "{}",
        t.texts()
    );

    assert_eq!(t.label("stages-2"), "Deep, 00:00 to 01:00");
    assert!(t.click("stages-3"));
    assert_eq!(t.app.span, Some(3));
    assert!(t.texts().contains("REM · 01:00 to 01:30"), "{}", t.texts());
}

#[test]
fn plots_stay_within_the_canvas_limits_and_draw_in_as_frames() {
    // Many gaps and a long timeline still fit one canvas each.
    let gappy: Vec<f32> = (0..400)
        .map(|i| if i % 3 == 0 { f32::NAN } else { i as f32 })
        .collect();
    let spans: Vec<(usize, f32, f32)> =
        (0..600).map(|i| (i % 4, i as f32, i as f32 + 1.)).collect();
    let page: Element<()> = vstack![
        line_plot([("A", gappy.clone()), ("B", gappy)])
            .glow(true)
            .area(true)
            .animate(1),
        timeline_chart(
            [
                ("a", "#111111"),
                ("b", "#222222"),
                ("c", "#333333"),
                ("d", "#444444")
            ],
            spans
        )
        .animate(1),
        range_chart((0..200).map(|i| ValueRange::new(i as f32, i as f32 + 5.).mid(i as f32 + 2.)))
            .animate(1),
    ]
    .into();
    let (d, h) = page
        .build()
        .expect("every plot is a valid document")
        .into_parts();
    let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
    rt.layout(
        Viewport::new(600., 900., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
}

#[test]
fn trimmed_first_frames_are_skipped_not_rejected() {
    let mut c = Canvas::default();
    let line = VectorPath::new().move_to(0., 0.).line_to(10., 0.);
    c.stroke_with(line.trimmed(0.), "#ffffff", StrokeStyle::new(2.).round());
    c.fill(VectorPath::new().move_to(5., 5.), "#ffffff");
    assert!(c.paths.is_empty());
    let page: Element<()> = canvas(move |x| *x = c).width(10.).height(10.);
    assert!(page.build().is_ok());
}
