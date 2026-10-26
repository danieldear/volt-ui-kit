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
fn selectable_links_still_click_but_word_selection_and_drag_do_not_activate_them() {
    let ui: Element<Vec<String>> = vstack![
        rich_text([span("linked label").link("reference.rs")])
            .key("link")
            .on_link(|a: &mut Vec<String>, target| a.push(target)),
        text("After the link").key("after"),
        button("Control")
            .key("control")
            .on_click(|_: &mut Vec<String>| {}),
    ]
    .selectable(true)
    .into();
    let (doc, mut handlers) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(400., 200., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let mut targets = Vec::new();
    let a = bounds(&r, "link");
    let y = a[1] + 5.;
    let x = a[0] + 7.;
    for clicks in [1, 2, 3] {
        r.pointer_down(x, y);
        assert!(
            !r.press_was_free(),
            "legacy free-press semantics stay unchanged"
        );
        assert!(r.press_allows_text_selection());
        r.text_select_press(x, y, clicks, false, &mut Metrics);
        r.text_select_release();
        let outcome = r
            .pointer_up_outcome(x, y, &mut handlers, &mut targets)
            .unwrap();
        assert_eq!(outcome.activated, clicks == 1);
        if clicks == 2 {
            assert_eq!(r.selected_text(), Some("linked"));
        }
        if clicks == 3 {
            assert_eq!(r.selected_text(), Some("linked label"));
        }
    }
    // Both endpoints remain in the very same link: a release must not open it.
    r.pointer_down(x, y);
    r.text_select_press(x, y, 1, false, &mut Metrics);
    r.text_select_drag(x + 7. * 4., y, &mut Metrics);
    r.text_select_release();
    assert_eq!(r.selected_text(), Some("inke"));
    assert!(
        !r.pointer_up_outcome(x + 7. * 4., y, &mut handlers, &mut targets)
            .unwrap()
            .activated
    );
    // Selection can extend from a link into the next paragraph as well.
    r.pointer_down(a[0], y);
    r.text_select_press(a[0], y, 1, false, &mut Metrics);
    let b = bounds(&r, "after");
    r.text_select_drag(b[0] + 7. * 5., b[1] + 5., &mut Metrics);
    r.text_select_release();
    assert_eq!(r.selected_text(), Some("linked label\nAfter"));
    assert!(
        !r.pointer_up_outcome(b[0], b[1] + 5., &mut handlers, &mut targets)
            .unwrap()
            .activated
    );
    assert_eq!(targets, ["reference.rs"]);
    // A captured button still owns its input, even inside a selectable parent.
    let c = bounds(&r, "control");
    r.pointer_down(c[0] + 3., c[1] + 3.);
    assert!(!r.press_allows_text_selection());
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

#[test]
fn double_click_drags_extend_by_whole_words() {
    let (doc, h) = view(&App::default()).build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(400., 300., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let b = bounds(&r, "b");
    let y = b[1] + 5.;
    // Double-click inside "paragraph", then the pointer wobbles within it:
    // the whole word stays selected (it used to shrink to the pointer).
    r.text_select_press(b[0] + 7. * 9., y, 2, false, &mut Metrics);
    r.text_select_drag(b[0] + 7. * 11., y, &mut Metrics);
    assert_eq!(r.selected_text(), Some("paragraph"));
    // Dragging back over "Second" takes whole words back to it.
    r.text_select_drag(b[0] + 7. * 2., y, &mut Metrics);
    assert_eq!(r.selected_text(), Some("Second paragraph"));
    r.text_select_release();
    // Triple-click drags extend by whole paragraphs.
    let a = bounds(&r, "a");
    r.text_select_press(a[0] + 7. * 2., a[1] + 5., 3, false, &mut Metrics);
    r.text_select_drag(b[0] + 7. * 3., y, &mut Metrics);
    assert_eq!(r.selected_text(), Some("Hello world\nSecond paragraph"));
    // A single click still extends character by character.
    r.text_select_press(b[0] + 7. * 2., y, 1, false, &mut Metrics);
    r.text_select_drag(b[0] + 7. * 4., y, &mut Metrics);
    assert_eq!(r.selected_text(), Some("co"));
}

#[derive(Default)]
struct ModalApp {
    open: bool,
    nested: bool,
    background_reports: Vec<String>,
    dialog_reports: Vec<String>,
}
fn modal_view(app: &ModalApp) -> Element<ModalApp> {
    zstack![
        vstack![text("BEHIND MODAL").key("background-text")]
            .selectable(true)
            .on_select_text(|a: &mut ModalApp, t| a.background_reports.push(t)),
        dialog(
            vstack![
                text("MODAL CONTENT").key("dialog-text"),
                dialog(text("NESTED CONTENT").key("nested-text"))
                    .key("nested")
                    .open(app.nested)
                    .on_dismiss(|a: &mut ModalApp| a.nested = false),
            ]
            .selectable(true)
            .on_select_text(|a: &mut ModalApp, t| a.dialog_reports.push(t))
        )
        .key("modal")
        .open(app.open)
        .on_dismiss(|a: &mut ModalApp| a.open = false),
    ]
    .into()
}
fn install_modal(
    r: &mut Runtime,
    h: &mut volt_ui_kit::declarative::Handlers<'static, ModalApp>,
    a: &ModalApp,
) {
    let (d, next) = modal_view(a).build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    *h = next;
    r.layout(
        Viewport::new(600., 400., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
}

#[test]
fn select_all_and_drag_stay_in_the_topmost_modal_scope() {
    let mut a = ModalApp {
        open: true,
        ..Default::default()
    };
    let (d, mut h) = modal_view(&a).build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(600., 400., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    assert!(r.select_all_text(&mut Metrics));
    assert_eq!(r.selected_text(), Some("MODAL CONTENT"));
    r.dispatch_text_selection(&mut h, &mut a).unwrap();
    assert!(a.background_reports.is_empty());
    assert_eq!(a.dialog_reports, ["MODAL CONTENT"]);
    let b = bounds(&r, "dialog-text");
    r.text_select_press(b[0], b[1] + 5., 1, false, &mut Metrics);
    r.text_select_drag(0., 0., &mut Metrics);
    r.text_select_release();
    assert!(!r.selected_text().unwrap_or("").contains("BEHIND"));
    a.nested = true;
    install_modal(&mut r, &mut h, &a);
    assert_eq!(
        r.selected_text(),
        None,
        "opening a modal must invalidate outside selection"
    );
    r.select_all_text(&mut Metrics);
    assert_eq!(r.selected_text(), Some("NESTED CONTENT"));
    a.nested = false;
    a.open = false;
    install_modal(&mut r, &mut h, &a);
    assert_eq!(
        r.selected_text(),
        None,
        "removed modal content cannot still be copied"
    );
    r.select_all_text(&mut Metrics);
    assert_eq!(r.selected_text(), Some("BEHIND MODAL"));
}

#[test]
fn old_background_selection_cannot_be_copied_or_dispatched_under_a_modal() {
    let mut a = ModalApp::default();
    let (d, mut h) = modal_view(&a).build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(600., 400., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    r.select_all_text(&mut Metrics);
    a.open = true;
    let (d, next) = modal_view(&a).build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    h = next;
    assert_eq!(
        r.selected_text(),
        None,
        "scope isolation must hold even before relayout"
    );
    r.dispatch_text_selection(&mut h, &mut a).unwrap();
    assert!(a.background_reports.is_empty());
    assert!(a.dialog_reports.iter().all(String::is_empty));
}

#[test]
fn copy_popups_keep_the_authorized_selection_including_inside_a_dialog() {
    fn view(open: bool, in_dialog: bool) -> Element<App> {
        let content: Element<App> = vstack![
            vstack![text("Selected text").key("copy-text")]
                .selectable(true)
                .on_select_text(|a: &mut App, t| a.selected.push(t)),
            button("Menu").key("copy-anchor").on_click(|_: &mut App| {}),
            popover(button("Copy").key("copy-action").on_click(|_: &mut App| {}))
                .key("copy-popup")
                .anchor("copy-anchor")
                .open(open)
                .on_dismiss(|_: &mut App| {}),
        ]
        .into();
        if in_dialog {
            zstack![
                text("Behind dialog").key("behind").selectable(true),
                dialog(content)
                    .key("copy-dialog")
                    .open(true)
                    .on_dismiss(|_: &mut App| {})
            ]
            .into()
        } else {
            content
        }
    }
    for in_dialog in [false, true] {
        let mut app = App::default();
        let (d, mut h) = view(false, in_dialog).build().unwrap().into_parts();
        let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
        let viewport = Viewport::new(600., 400., 1.).unwrap();
        r.layout(viewport, &Bindings::default(), &mut Metrics)
            .unwrap();
        r.select_all_text(&mut Metrics);
        assert_eq!(r.selected_text(), Some("Selected text"));
        r.dispatch_text_selection(&mut h, &mut app).unwrap();
        for open in [true, false] {
            let (d, next) = view(open, in_dialog).build().unwrap().into_parts();
            r.replace_document(d, &next, &Bindings::default()).unwrap();
            h = next;
            assert_eq!(
                r.selected_text(),
                Some("Selected text"),
                "opening a Copy popup must not discard its input"
            );
            r.layout(viewport, &Bindings::default(), &mut Metrics)
                .unwrap();
            assert_eq!(r.selected_text(), Some("Selected text"));
            if open {
                assert!(!r.select_all_text(&mut Metrics));
            }
            assert_eq!(r.selected_text(), Some("Selected text"));
            r.dispatch_text_selection(&mut h, &mut app).unwrap();
        }
        assert_eq!(app.selected, ["Selected text"]);
    }
}

#[test]
fn a_new_root_context_clears_selection_even_when_descendant_keys_are_reused() {
    let build = |root: &str, words: &str| -> Element<App> {
        vstack![text(words).key("same-message")].key(root).selectable(true).into()
    };
    let (d, h) = build("session-one", "First answer").build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let viewport = Viewport::new(400., 200., 1.).unwrap();
    r.layout(viewport, &Bindings::default(), &mut Metrics).unwrap();
    r.select_all_text(&mut Metrics);
    assert_eq!(r.selected_text(), Some("First answer"));
    let (d, next) = build("session-two", "Other answer").build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    r.layout(viewport, &Bindings::default(), &mut Metrics).unwrap();
    assert_eq!(r.selected_text(), None);
}

#[test]
fn a_floating_surface_blocks_selection_of_read_only_text_behind_its_blank_space() {
    let ui: Element<()> = zstack([
        vstack([text("Underlying document text").key("under").into()])
            .selectable(true)
            .width(300.)
            .height(160.)
            .into(),
        hstack([panel([])
            .key("floating")
            .width(120.)
            .height(80.)
            .background("#FFFFFF|#202020")
            .into()]).align(Align::Start).width(300.).height(160.).into(),
    ])
    .align(Align::Start)
    .width(300.)
    .height(160.)
    .into();
    let (doc, handlers) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(300., 160., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let a = bounds(&r, "under");
    assert!(!r.text_select_press(a[0] + 5., a[1] + 5., 3, false, &mut Metrics),"text={a:?}, floating={:?}",bounds(&r,"floating"));
    assert!(
        r.selected_text().is_none(),
        "blank popup must not select hidden document text"
    );
    // Still inside the same paragraph, but beyond the floating surface.
    assert!(r.text_select_press(a[0] + 150., a[1] + 5., 3, false, &mut Metrics));
    r.text_select_release();
    assert_eq!(r.selected_text(), Some("Underlying document text"));
}
