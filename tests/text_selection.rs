#![cfg(feature = "rust-ui")]
//! Selecting and copying read-only text.
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics, TextStyle},
    ui::prelude::*,
    Viewport,
};

/// Fixed-width glyphs, half the font size wide (7 px at the default 14),
/// one line per text.
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, _: f32) -> [f32; 2] {
        [text.chars().count() as f32 * size * 0.5, size * 1.2]
    }
    fn text_offset_at(
        &mut self,
        text: &str,
        r: [f32; 4],
        style: &TextStyle,
        p: [f32; 2],
    ) -> Option<usize> {
        let char_w = style.size * 0.5;
        let n = ((p[0] - r[0]) / char_w).round().max(0.) as usize;
        Some(text.char_indices().nth(n).map_or(text.len(), |(i, _)| i))
    }
    fn text_range_rects(
        &mut self,
        text: &str,
        r: [f32; 4],
        style: &TextStyle,
        range: std::ops::Range<usize>,
    ) -> Vec<[f32; 5]> {
        let char_w = style.size * 0.5;
        let a = text[..range.start].chars().count() as f32;
        let b = text[..range.end].chars().count() as f32;
        vec![[
            r[0] + a * char_w,
            r[1],
            (b - a) * char_w,
            style.size * 1.2,
            r[1] + style.size,
        ]]
    }
}

#[derive(Default)]
struct App {
    selected: Vec<String>,
}

fn view(_: &App) -> Element<App> {
    vstack![
        vstack![
            text("Hello world").key("a"),
            text("•").key("marker").selectable(false),
            text("Second paragraph").key("b"),
        ]
        .key("doc")
        .selectable(true)
        .on_select_text(|app: &mut App, text| app.selected.push(text)),
        text("Chrome label").key("chrome"),
    ]
    .key("root")
    .into()
}

fn bounds(r: &Runtime, id: &str) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds()
}

#[test]
fn drag_selects_across_paragraphs_and_reports_the_text() {
    let mut app = App::default();
    let (doc, mut h) = view(&app).build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let v = Viewport::new(400., 300., 1.).unwrap();
    r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
    let a = bounds(&r, "a");
    let b = bounds(&r, "b");
    // Press at "world", drag into "paragraph".
    assert!(r.text_select_press(a[0] + 7. * 6., a[1] + 5., 1, false, &mut Metrics));
    assert!(r.text_select_drag(b[0] + 7. * 6., b[1] + 5., &mut Metrics));
    assert!(r.text_select_release());
    r.dispatch_text_selection(&mut h, &mut app).unwrap();
    assert_eq!(
        app.selected.last().unwrap(),
        "world\nSecond",
        "the marker is left out"
    );
    assert_eq!(r.selected_text(), Some("world\nSecond"));
    // Reported once until it changes.
    assert!(!r.dispatch_text_selection(&mut h, &mut app).unwrap());
    // Double-click picks a word; Shift-click extends; triple picks the text.
    r.text_select_press(b[0] + 7. * 9., b[1] + 5., 2, false, &mut Metrics);
    r.text_select_release();
    assert_eq!(r.selected_text(), Some("paragraph"));
    r.text_select_press(a[0], a[1] + 5., 1, true, &mut Metrics);
    r.text_select_release();
    assert_eq!(r.selected_text(), Some("Hello world\nSecond "));
    r.text_select_press(a[0] + 6., a[1] + 5., 3, false, &mut Metrics);
    r.text_select_release();
    assert_eq!(r.selected_text(), Some("Hello world"));
    // Text outside a selectable container is not selectable: pressing it clears.
    let chrome = bounds(&r, "chrome");
    assert!(r.text_select_press(chrome[0] + 6., chrome[1] + 5., 1, false, &mut Metrics));
    assert_eq!(r.selected_text(), None);
    r.dispatch_text_selection(&mut h, &mut app).unwrap();
    assert_eq!(app.selected.last().unwrap(), "");
    // Select all takes every selectable text in order.
    assert!(r.select_all_text(&mut Metrics));
    assert_eq!(r.selected_text(), Some("Hello world\nSecond paragraph"));
}

#[test]
fn selection_highlights_paint_behind_the_selected_runs() {
    let app = App::default();
    let (doc, h) = view(&app).build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let v = Viewport::new(400., 300., 1.).unwrap();
    r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
    let a = bounds(&r, "a");
    r.text_select_press(a[0], a[1] + 5., 1, false, &mut Metrics);
    r.text_select_drag(a[0] + 7. * 5., a[1] + 5., &mut Metrics);
    // Relayout (e.g. a resize) keeps the selection and recomputes its rects.
    r.invalidate_layout();
    r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
    assert_eq!(r.selected_text(), Some("Hello"));
}
