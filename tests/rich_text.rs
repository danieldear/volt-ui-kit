#![cfg(feature = "native")]
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics, TextRun, TextStyle},
    native::RasterPainter,
    ui::prelude::*,
    Painter, UiTheme, Viewport,
};

fn layout(ui: Element<()>, size: [f32; 2]) -> (Runtime, RasterPainter) {
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(size[0], size[1], 1.).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    (r, p)
}
fn bounds(r: &Runtime, key: &str) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == key)
        .unwrap()
        .bounds
        .bounds()
}
fn style(size: f32) -> TextStyle {
    TextStyle {
        size,
        ..TextStyle::default()
    }
}

#[test]
fn spans_build_one_text_and_reject_invalid_styles() {
    let ok: Element<()> = rich_text([span("Hello "), span(""), span("world").bold().italic()])
        .key("t")
        .into();
    ok.build().unwrap();
    for bad in [
        span("x").weight(450),
        span("x").color("not-a-colour"),
        span("x").background("#12"),
        span("x").family(""),
    ] {
        let ui: Element<()> = rich_text([span("a "), bad]).into();
        assert!(ui.build().is_err());
    }
}

#[test]
fn font_runs_change_measurement_and_colour_runs_do_not_reshape() {
    let mut p = RasterPainter::new(Viewport::new(800., 200., 1.).unwrap());
    let text = "The quick brown fox jumps";
    let plain = p.measure_styled(text, &style(16.), 1_000_000.);
    let run = |r: TextRun| TextStyle {
        runs: vec![TextRun {
            start: 0,
            end: text.len(),
            ..r
        }],
        ..style(16.)
    };
    let bold = p.measure_styled(
        text,
        &run(TextRun {
            weight: Some(800),
            ..TextRun::default()
        }),
        1_000_000.,
    );
    assert!(bold[0] > plain[0], "bold {bold:?} vs plain {plain:?}");
    let mono = p.measure_styled(
        text,
        &run(TextRun {
            family: Some("monospace".into()),
            ..TextRun::default()
        }),
        1_000_000.,
    );
    assert_ne!(mono[0], plain[0]);
    let misses = p.text_cache_misses;
    let colored = p.measure_styled(
        text,
        &run(TextRun {
            color: Some([1., 0., 0., 1.]),
            ..TextRun::default()
        }),
        1_000_000.,
    );
    assert_eq!(colored, plain);
    assert_eq!(
        p.text_cache_misses, misses,
        "colour-only runs reuse the plain shape"
    );
}

#[test]
fn run_colours_draw_only_in_their_ranges() {
    let ui: Element<()> = vstack![rich_text([
        span("MMMMMM").color("#FF0000"),
        span("MMMMMM").color("#0000FF"),
    ])
    .key("t")
    .font_size(28.)]
    .padding(10., Edge::All)
    .into();
    let (r, mut p) = layout(ui, [400., 80.]);
    let [x, y, w, h] = bounds(&r, "t");
    let batches = r.paint_ui(&mut p, &UiTheme::dark());
    let pix = p.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
    // x extents of clearly red and clearly blue ink.
    let (mut red, mut blue) = ([u32::MAX, 0], [u32::MAX, 0]);
    for py in y as u32..(y + h) as u32 {
        for px in x as u32..(x + w) as u32 {
            let c = pix.pixel(px, py).unwrap();
            if c.red() > 120 && c.blue() < 40 {
                red = [red[0].min(px), red[1].max(px)];
            }
            if c.blue() > 120 && c.red() < 40 {
                blue = [blue[0].min(px), blue[1].max(px)];
            }
        }
    }
    assert!(
        red[0] < red[1] && blue[0] < blue[1],
        "both runs drew: {red:?} {blue:?}"
    );
    assert!(
        red[1] < blue[0],
        "red run left of blue run: {red:?} {blue:?}"
    );
}

#[test]
fn code_background_sits_behind_its_run_only() {
    let ui: Element<()> = vstack![rich_text([
        span("plain plain "),
        span("code").background("#00FF00"),
        span(" tail tail"),
    ])
    .key("t")
    .font_size(20.)]
    .padding(10., Edge::All)
    .into();
    let (r, mut p) = layout(ui, [500., 60.]);
    let [x, y, w, h] = bounds(&r, "t");
    let batches = r.paint_ui(&mut p, &UiTheme::dark());
    let pix = p.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
    let columns = |test: &dyn Fn(u8, u8, u8) -> bool| -> Vec<u32> {
        (x as u32..(x + w) as u32)
            .filter(|&px| {
                (y as u32..(y + h) as u32).any(|py| {
                    let c = pix.pixel(px, py).unwrap();
                    test(c.red(), c.green(), c.blue())
                })
            })
            .collect()
    };
    let green = columns(&|r, g, _| g > 200 && r < 60);
    let ink = columns(&|r, g, b| r > 120 && g > 120 && b > 120);
    let (first, last) = (*green.first().unwrap(), *green.last().unwrap());
    let (ink_first, ink_last) = (*ink.first().unwrap(), *ink.last().unwrap());
    let span = (ink_last - ink_first) as f32;
    // The chip starts after "plain plain " and ends before " tail tail".
    assert!(
        first as f32 > ink_first as f32 + span * 0.3,
        "chip {first}..{last}, ink {ink_first}..{ink_last}"
    );
    assert!(
        (last as f32) < ink_first as f32 + span * 0.8,
        "chip {first}..{last}, ink {ink_first}..{ink_last}"
    );
    assert!(last - first > 20, "chip is wide enough for its glyphs");
}

#[test]
fn a_wrapped_run_reports_one_rect_per_line() {
    let mut p = RasterPainter::new(Viewport::new(400., 400., 1.).unwrap());
    let text = "one two three four five six seven eight nine ten";
    let s = style(16.);
    let all = p.text_range_rects(text, [10., 10., 120., 300.], &s, 0..text.len());
    assert!(all.len() >= 3, "{all:?}");
    for w in all.windows(2) {
        assert!(w[1][1] > w[0][1], "lines go down: {all:?}");
    }
    // A range inside one word covers only part of its line.
    let start = text.find("three").unwrap();
    let one = p.text_range_rects(text, [10., 10., 120., 300.], &s, start..start + 5);
    assert_eq!(one.len(), 1);
    assert!(one[0][2] < 120.);
}

#[test]
fn truncation_across_runs_with_multibyte_text_never_panics() {
    for width in [20., 45., 80., 140., 260.] {
        for lines in [1u16, 2] {
            let ui: Element<()> = vstack![rich_text([
                span("ééééé ").bold(),
                span("ççççç ççç ").italic().color("#FF0000"),
                span("日本語のテキスト").code(),
                span(" 🙂 end").underline(),
            ])
            .key("t")
            .max_lines(lines)
            .width(width)]
            .into();
            let (r, mut p) = layout(ui, [300., 120.]);
            let batches = r.paint_ui(&mut p, &UiTheme::light());
            p.rasterize(&batches, [1., 1., 1., 1.]).unwrap();
        }
    }
}

#[derive(Default)]
struct Clicks(Vec<String>);

fn link_runtime(
    scroll: bool,
) -> (
    Runtime,
    volt_ui_kit::declarative::Handlers<'static, Clicks>,
    RasterPainter,
) {
    let paragraph = rich_text([
        span("Read the "),
        span("guide").link("docs/guide.md"),
        span(" or visit "),
        span("the website").link("https://example.com"),
        span("."),
    ])
    .key("t")
    .font_size(20.)
    .on_link(|clicks: &mut Clicks, target| clicks.0.push(target));
    let ui: Element<Clicks> = if scroll {
        vstack![spacer().height(300.).flex(0.), paragraph]
            .key("scroller")
            .height(120.)
            .scroll(Scroll::Vertical)
            .into()
    } else {
        vstack![paragraph].padding(10., Edge::All).into()
    };
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(600., 120., 1.).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    (r, h, p)
}
/// Centre of the laid-out range of `needle` in the text box.
fn centre_of(r: &Runtime, p: &mut RasterPainter, needle: &str) -> [f32; 2] {
    let b = r.boxes().iter().find(|b| b.id == "t").unwrap();
    let text = "Read the guide or visit the website.";
    let start = text.find(needle).unwrap();
    let [x, y, w, h, _] = p.text_range_rects(
        text,
        b.bounds.bounds(),
        &b.text_style,
        start..start + needle.len(),
    )[0];
    [x + w / 2., y + h / 2.]
}

#[test]
fn clicking_a_link_reports_its_target_and_plain_text_does_not() {
    let (mut r, mut h, mut p) = link_runtime(false);
    let mut clicks = Clicks::default();
    for (needle, expected) in [
        ("guide", Some("docs/guide.md")),
        ("website", Some("https://example.com")),
        ("Read", None),
    ] {
        let [x, y] = centre_of(&r, &mut p, needle);
        r.pointer_down(x, y);
        let out = r.pointer_up_outcome(x, y, &mut h, &mut clicks).unwrap();
        assert_eq!(out.activated, expected.is_some(), "{needle}");
        if let Some(target) = expected {
            assert_eq!(clicks.0.last().map(String::as_str), Some(target));
        }
    }
    assert_eq!(clicks.0.len(), 2);
    // Press on one link, release on another: no click.
    let [ax, ay] = centre_of(&r, &mut p, "guide");
    let [bx, by] = centre_of(&r, &mut p, "website");
    r.pointer_down(ax, ay);
    assert!(
        !r.pointer_up_outcome(bx, by, &mut h, &mut clicks)
            .unwrap()
            .activated
    );
    assert_eq!(clicks.0.len(), 2);
}

#[test]
fn hovering_a_link_shows_a_pointer_and_an_underline() {
    let (mut r, _h, mut p) = link_runtime(false);
    let [x, y] = centre_of(&r, &mut p, "website");
    let [px, py] = centre_of(&r, &mut p, "Read");
    assert_eq!(r.cursor_at(px, py), Cursor::Default);
    assert_eq!(r.cursor_at(x, y), Cursor::Pointer);
    let ink = |r: &mut Runtime, p: &mut RasterPainter| {
        let batches = r.paint_ui(p, &UiTheme::dark());
        let pix = p.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
        (0..pix.width())
            .flat_map(|x| (0..pix.height()).map(move |y| (x, y)))
            .filter(|&(x, y)| pix.pixel(x, y).unwrap().blue() > 100)
            .count()
    };
    let before = ink(&mut r, &mut p);
    assert!(r.pointer_move(x, y), "hovering a link repaints");
    let hovered = ink(&mut r, &mut p);
    assert!(
        hovered > before,
        "underline adds link-coloured ink: {before} -> {hovered}"
    );
    assert!(r.pointer_move(px, py));
    assert_eq!(ink(&mut r, &mut p), before);
}

#[test]
fn links_follow_their_text_when_it_scrolls() {
    let (mut r, mut h, mut p) = link_runtime(true);
    assert!(r.scroll_at(50., 50., [0., 250.]));
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let [x, y] = centre_of(&r, &mut p, "guide");
    let mut clicks = Clicks::default();
    r.pointer_down(x, y);
    assert!(
        r.pointer_up_outcome(x, y, &mut h, &mut clicks)
            .unwrap()
            .activated
    );
    assert_eq!(clicks.0, ["docs/guide.md"]);
}

#[test]
fn a_sized_run_measures_smaller_without_changing_line_height() {
    let mut p = RasterPainter::new(Viewport::new(800., 200., 1.).unwrap());
    let text = "monospace code sample";
    let style = |scale: Option<f32>| TextStyle {
        runs: vec![TextRun {
            start: 0,
            end: text.len(),
            family: Some("monospace".into()),
            scale,
            ..TextRun::default()
        }],
        ..TextStyle {
            size: 16.,
            ..TextStyle::default()
        }
    };
    let full = p.measure_styled(text, &style(None), 1_000_000.);
    let small = p.measure_styled(text, &style(Some(0.8)), 1_000_000.);
    assert!(small[0] < full[0] * 0.9, "{small:?} vs {full:?}");
    assert_eq!(small[1], full[1], "line height follows the paragraph");
}
