#![cfg(feature = "rust-ui")]
use std::collections::BTreeSet;
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, _: f32) -> [f32; 2] {
        [text.len() as f32 * size * 0.5, size * 1.2]
    }
}

const ROWS: usize = 2000;
const GAP: f32 = 4.;
/// Real row heights differ from the 40px estimate in both directions.
fn height(i: usize) -> f32 {
    20. + (i % 7) as f32 * 13.
}

#[derive(Default)]
struct App {
    window: std::ops::Range<usize>,
    jump: (usize, u32),
}

fn view(app: &App) -> Element<App> {
    vstack![lazy_list(ROWS, |i| vstack![]
        .key(format!("row-{i}"))
        .height(height(i))
        .into())
    .key("list")
    .estimate(40.)
    .window(app.window.clone())
    .overscan(2)
    .scroll_to_row(app.jump.0, app.jump.1)
    .on_window_change(|app: &mut App, window| app.window = window)
    .gap(GAP)
    .flex(1.)]
    .into()
}

fn viewport() -> Viewport {
    Viewport::new(400., 300., 1.).unwrap()
}

fn mount(app: &App) -> (Runtime, Handlers<'static, App>) {
    let (doc, h) = view(app).build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(viewport(), &Bindings::default(), &mut Metrics)
        .unwrap();
    (r, h)
}

/// Lay out, then apply window changes until the list is stable, as the
/// native host does after layout.
fn settle(r: &mut Runtime, h: &mut Handlers<'static, App>, app: &mut App) {
    for _ in 0..12 {
        r.layout(viewport(), &Bindings::default(), &mut Metrics)
            .unwrap();
        if !r.dispatch_virtual_windows(h, app).unwrap() {
            return;
        }
        let (doc, next) = view(app).build().unwrap().into_parts();
        r.replace_document(doc, &next, &Bindings::default())
            .unwrap();
        *h = next;
    }
    panic!("window did not settle");
}

fn bounds(r: &Runtime, id: &str) -> Option<[f32; 4]> {
    r.boxes()
        .iter()
        .find(|b| b.id == id)
        .map(|b| b.bounds.bounds())
}

#[test]
fn scrolling_reaches_every_row_and_ends_exactly_at_the_last() {
    let mut app = App {
        window: 0..20,
        ..App::default()
    };
    let (mut r, mut h) = mount(&app);
    settle(&mut r, &mut h, &mut app);
    let mut seen = BTreeSet::new();
    let mut most_boxes = 0;
    for _ in 0..2000 {
        for b in r.boxes() {
            if let Some(i) = b.id.strip_prefix("row-") {
                seen.insert(i.parse::<usize>().unwrap());
            }
        }
        most_boxes = most_boxes.max(r.boxes().len());
        if !r.scroll_at(200., 150., [0., 120.]) {
            break;
        }
        settle(&mut r, &mut h, &mut app);
    }
    assert_eq!(seen.len(), ROWS, "every row was built at some point");
    assert!(most_boxes < 300, "only nearby rows are built: {most_boxes}");
    let list = bounds(&r, "list").unwrap();
    let last = bounds(&r, &format!("row-{}", ROWS - 1)).unwrap();
    assert!(
        (last[1] + last[3] - (list[1] + list[3])).abs() < 1.,
        "the last row ends at the bottom: {last:?} in {list:?}"
    );
    assert_eq!(last[3], height(ROWS - 1), "rows keep their own heights");
}

#[test]
fn jumping_to_an_unbuilt_row_puts_it_at_the_top() {
    let mut app = App {
        window: 0..20,
        ..App::default()
    };
    let (mut r, mut h) = mount(&app);
    settle(&mut r, &mut h, &mut app);
    for (n, row) in [(1, 1500), (2, 37), (3, 1999 - 20)].into_iter() {
        app.jump = (row, n);
        let (doc, next) = view(&app).build().unwrap().into_parts();
        r.replace_document(doc, &next, &Bindings::default())
            .unwrap();
        h = next;
        settle(&mut r, &mut h, &mut app);
        let list = bounds(&r, "list").unwrap();
        let target = bounds(&r, &format!("row-{row}"))
            .unwrap_or_else(|| panic!("row {row} is built after the jump"));
        assert!(
            (target[1] - list[1]).abs() < 1.,
            "row {row} at the top: {target:?} vs list {list:?}"
        );
        assert!(app.window.contains(&row));
    }
}

#[test]
fn lists_need_a_key_and_a_window_callback() {
    let unkeyed: Element<App> = lazy_list(3, |i| vstack![].key(format!("r{i}")).into())
        .on_window_change(|_: &mut App, _| {})
        .into();
    assert!(unkeyed.build().is_err());
    let no_callback: Element<App> = lazy_list(3, |i| vstack![].key(format!("r{i}")).into())
        .key("list")
        .into();
    assert!(no_callback.build().is_err());
    let unkeyed_rows: Element<App> = lazy_list(3, |_| vstack![].into())
        .key("list")
        .on_window_change(|_: &mut App, _| {})
        .into();
    assert!(unkeyed_rows.build().is_err());
}

#[test]
fn invalid_lazy_counts_are_errors_before_allocation_or_row_construction() {
    for count in [usize::MAX, 1_000_001] {
        let result = std::panic::catch_unwind(|| {
            let ui: Element<()> = lazy_list(count, |_| panic!("invalid list built a row"))
                .key("list")
                .on_window_change(|_, _| {})
                .into();
            ui.build().is_err()
        });
        assert_eq!(
            result.ok(),
            Some(true),
            "count {count} must be rejected, not panic"
        );
        let ui: Element<()> = lazy_list(count, |_| panic!("invalid count built a row"))
            .key("list")
            .on_window_change(|_, _| {})
            .estimates(std::iter::from_fn(|| -> Option<f32> {
                panic!("invalid count consumed estimates")
            }))
            .into();
        assert!(ui.build().is_err());
    }
}

#[test]
fn invalid_estimates_do_not_construct_rows_or_consume_unbounded_iterators() {
    use std::{cell::Cell, rc::Rc};
    for estimate in [f32::NAN, f32::INFINITY, 0., 100_001.] {
        let ui: Element<()> = lazy_list(3, |_| panic!("invalid estimate built a row"))
            .key("list")
            .estimate(estimate)
            .on_window_change(|_, _| {})
            .into();
        assert!(ui.build().is_err());
    }
    let reads = Rc::new(Cell::new(0));
    let read = reads.clone();
    let ui: Element<()> = lazy_list(3, |_| panic!("excess estimates built a row"))
        .key("list")
        .on_window_change(|_, _| {})
        .estimates((0..100).map(move |_| {
            read.set(read.get() + 1);
            24.
        }))
        .into();
    assert!(ui.build().is_err());
    assert!(
        reads.get() <= 4,
        "read {} estimates for three rows",
        reads.get()
    );
    let ui: Element<()> = lazy_list(3, |i| text("row").key(format!("r{i}")).into())
        .key("list")
        .on_window_change(|_, _| {})
        .estimates(std::iter::repeat(24.))
        .into();
    assert!(
        ui.build().is_err(),
        "infinite input is bounded and rejected"
    );
}

#[test]
fn empty_and_maximum_sized_lazy_lists_remain_valid() {
    for count in [0, 1_000_000] {
        let ui: Element<()> = lazy_list(count, |i| text("row").key(format!("r{i}")).into())
            .key("list")
            .on_window_change(|_, _| {})
            .into();
        assert!(ui.build().is_ok());
    }
}
