#![cfg(feature = "rust-ui")]
use std::{cell::Cell, rc::Rc};
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App {
    selected: Option<usize>,
    window: std::ops::Range<usize>,
    expanded: Vec<usize>,
    sort: Option<(usize, SortDirection)>,
}
#[test]
fn virtual_list_builds_only_a_bounded_window_of_ten_thousand_records() {
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let ui: Element<App> = virtual_list(10_000, 32., move |index| {
        count.set(count.get() + 1);
        data_item(index, format!("Record {index}")).key(format!("record-{index}"))
    })
    .key("records")
    .window(500..516)
    .selected(Some(505))
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_window_change(|app: &mut App, window| app.window = window)
    .into();
    let (document, _) = ui.build().unwrap().into_parts();
    assert_eq!(calls.get(), 16);
    assert_eq!(document.root().children().len(), 16);
}
#[test]
fn typed_table_sorting_reorders_rows_without_changing_stable_keys() {
    let ui: Element<App> = table(
        [table_column(0, "Name"), table_column(1, "Age")],
        [
            table_row(1, ["Beta", "20"]).key("beta"),
            table_row(2, ["Alpha", "40"]).key("alpha"),
        ],
    )
    .key("table")
    .sort(0, SortDirection::Ascending)
    .selected(Some(1))
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_sort(|app: &mut App, col, direction| app.sort = Some((col, direction)))
    .into();
    ui.build().unwrap();
}
#[test]
fn tree_builds_only_expanded_branches() {
    let ui: Element<App> = tree_view([tree_node(0, "Root")
        .key("root")
        .children([tree_node(1, "Child").key("child")])])
    .key("tree")
    .expanded([])
    .selected(None)
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_expand(|app: &mut App, id, open| {
        if open {
            app.expanded.push(id)
        }
    })
    .into();
    let (document, _) = ui.build().unwrap().into_parts();
    fn has(n: &volt_ui_kit::declarative::Node, id: &str) -> bool {
        n.id() == id || n.children().iter().any(|n| has(n, id))
    }
    assert!(!has(document.root(), "child"));
}

use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, TextMetrics},
    Painter, Viewport,
};
struct Metrics(f32);
impl TextMetrics for Metrics {
    fn measure(&mut self, s: &str, size: f32, width: f32) -> [f32; 2] {
        [(s.len() as f32 * size * 0.5).min(width), size * 1.2]
    }
}
impl Painter for Metrics {
    type Shape = ();
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(600. * self.0, 600. * self.0, self.0).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<()>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<()>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(&mut self, _: &mut Vec<()>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<()>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for Metrics {
    fn ui_text(&mut self, _: &mut Vec<()>, _: &str, _: [f32; 4], _: f32, _: [f32; 4]) {}
}
fn mount(ui: Element<App>, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (d, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    (r, h)
}
fn replace(r: &mut Runtime, h: &mut Handlers<'static, App>, ui: Element<App>, scale: f32) {
    let (d, next) = ui.build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    *h = next;
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
}
fn click(r: &mut Runtime, h: &mut Handlers<'static, App>, app: &mut App, id: &str) -> bool {
    let [x, y, w, height] = r
        .boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds();
    r.pointer_down_outcome(x + w / 2., y + height / 2., h, app)
        .unwrap();
    r.pointer_up_outcome(x + w / 2., y + height / 2., h, app)
        .unwrap()
        .activated
}
fn list(app: &App) -> Element<App> {
    virtual_list(10_000, 32., |id| {
        data_item(id, format!("Record {id}"))
            .key(format!("r-{id}"))
            .enabled(id != 2)
    })
    .key("records")
    .window(app.window.clone())
    .selected(app.selected)
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_window_change(|app: &mut App, window| app.window = window)
    .into()
}
#[test]
fn virtual_wheel_updates_the_mounted_window_not_the_entire_model() {
    for scale in [1., 2.] {
        let mut app = App {
            window: 0..15,
            ..App::default()
        };
        let (mut r, mut h) = mount(list(&app), scale);
        assert_eq!(r.document().root().children().len(), 15);
        assert!(
            r.boxes()
                .iter()
                .find(|b| b.id == "records")
                .unwrap()
                .scroll_max[1]
                > 300_000.
        );
        assert!(
            r.scroll_outcome(100. * scale, 100. * scale, [0., 1600.], &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert!(app.window.start >= 48);
        assert!(app.window.len() <= 96);
        replace(&mut r, &mut h, list(&app), scale);
        assert!(r.document().root().children().len() <= 96);
        assert!(r.boxes().iter().any(|b| b.id == "r-50"));
        assert!(click(&mut r, &mut h, &mut app, "r-50"));
        assert_eq!(app.selected, Some(50));
        assert!(!r.boxes().iter().any(|b| b.id == "r-0"));
    }
}
#[test]
fn virtual_list_accepts_a_factory_borrowing_view_local_data() {
    let labels = ["One", "Two", "Three"];
    let ui: Element<App> = virtual_list(labels.len(), 32., |i| {
        data_item(i, labels[i]).key(format!("borrow-{i}"))
    })
    .key("borrow")
    .selected(None)
    .on_select(|_: &mut App, _| {})
    .on_window_change(|_, _| {})
    .into();
    ui.build().unwrap();
    assert_eq!(labels.len(), 3);
}
#[test]
fn virtual_invalid_windows_fail_before_calling_the_factory() {
    for window in [std::ops::Range { start: 20, end: 10 }, 0..97, 99..101] {
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        let ui: Element<App> = virtual_list(100, 32., move |i| {
            count.set(count.get() + 1);
            data_item(i, "Row").key(format!("invalid-{i}"))
        })
        .key("invalid")
        .window(window)
        .on_select(|_: &mut App, _| {})
        .on_window_change(|_, _| {})
        .into();
        assert!(ui.build().is_err());
        assert_eq!(calls.get(), 0);
    }
}
#[test]
fn virtual_empty_disabled_rows_and_selection_reorder_are_safe() {
    let mut app = App {
        window: 0..5,
        ..App::default()
    };
    let (mut r, mut h) = mount(list(&app), 1.);
    assert!(!click(&mut r, &mut h, &mut app, "r-2"));
    assert_eq!(app.selected, None);
    assert!(click(&mut r, &mut h, &mut app, "r-1"));
    assert_eq!(app.selected, Some(1));
    let ui: Element<App> = virtual_list(0, 32., |i| data_item(i, "Never").key("never"))
        .key("empty")
        .on_select(|_: &mut App, _| {})
        .on_window_change(|_, _| {})
        .into();
    assert!(ui
        .build()
        .unwrap()
        .into_parts()
        .0
        .root()
        .children()
        .is_empty());
}
fn sorted_table(direction: SortDirection) -> Element<App> {
    table(
        [table_column(0, "Name"), table_column(1, "Age")],
        [
            table_row(1, ["Beta", "20"]).key("beta"),
            table_row(2, ["Alpha", "40"]).key("alpha"),
        ],
    )
    .key("table")
    .sort(0, direction)
    .selected(Some(1))
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_sort(|app: &mut App, col, direction| app.sort = Some((col, direction)))
    .into()
}
#[test]
fn table_really_sorts_and_headers_emit_typed_sort_direction() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(sorted_table(SortDirection::Ascending), scale);
        assert_eq!(
            r.document().root().children()[1].children()[0].id(),
            "alpha"
        );
        let mut app = App::default();
        assert!(click(&mut r, &mut h, &mut app, "table-column-0"));
        assert_eq!(app.sort, Some((0, SortDirection::Descending)));
        replace(
            &mut r,
            &mut h,
            sorted_table(SortDirection::Descending),
            scale,
        );
        assert_eq!(r.document().root().children()[1].children()[0].id(), "beta");
        assert!(click(&mut r, &mut h, &mut app, "alpha"));
        assert_eq!(app.selected, Some(2));
    }
}
#[test]
fn table_typed_comparator_can_sort_numbers_not_display_strings() {
    let ui: Element<App> = table(
        [table_column(0, "Count")],
        [
            table_row(20, ["20"]).key("twenty"),
            table_row(3, ["3"]).key("three"),
        ],
    )
    .key("numbers")
    .sort(0, SortDirection::Ascending)
    .sort_by(|_, a, b| a.cmp(b))
    .on_select(|_: &mut App, _| {})
    .on_sort(|_, _, _| {})
    .into();
    let (d, _) = ui.build().unwrap().into_parts();
    assert_eq!(d.root().children()[1].children()[0].id(), "three");
}
#[test]
fn table_rejects_wrong_cells_duplicates_missing_callback_and_node_budget() {
    let make = || {
        table::<App, _, _>([table_column(0, "A")], [table_row(1, ["A"]).key("one")]).key("table")
    };
    assert!(Element::from(make()).build().is_err());
    let ui: Element<App> = table(
        (0..8).map(|i| table_column(i, "C")),
        (0..96).map(|i| {
            table_row(i, ["1", "2", "3", "4", "5", "6", "7", "8"]).key(format!("row-{i}"))
        }),
    )
    .key("big")
    .on_select(|_: &mut App, _| {})
    .on_sort(|_, _, _| {})
    .into();
    assert!(ui.build().is_err());
    let ui: Element<App> = table(
        [table_column(0, "A")],
        [table_row(1, ["A", "B"]).key("one")],
    )
    .key("bad")
    .on_select(|_: &mut App, _| {})
    .on_sort(|_, _, _| {})
    .into();
    assert!(ui.build().is_err());
}
fn tree(app: &App) -> Element<App> {
    tree_view([
        tree_node(0, "Root")
            .key("root")
            .children([tree_node(1, "Child").key("child")]),
        tree_node(2, "Disabled").key("disabled").enabled(false),
    ])
    .key("tree")
    .expanded(app.expanded.clone())
    .selected(app.selected)
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_expand(|app: &mut App, id, open| {
        if open {
            app.expanded.push(id);
        } else {
            app.expanded.retain(|value| *value != id);
        }
    })
    .into()
}
#[test]
fn tree_expand_select_and_collapse_mounts_only_live_rows() {
    for scale in [1., 2.] {
        let mut app = App::default();
        let (mut r, mut h) = mount(tree(&app), scale);
        assert!(!r.boxes().iter().any(|b| b.id == "child"));
        assert!(click(&mut r, &mut h, &mut app, "root-expand"));
        replace(&mut r, &mut h, tree(&app), scale);
        assert!(click(&mut r, &mut h, &mut app, "child"));
        assert_eq!(app.selected, Some(1));
        assert!(!click(&mut r, &mut h, &mut app, "disabled"));
        assert!(click(&mut r, &mut h, &mut app, "root-expand"));
        replace(&mut r, &mut h, tree(&app), scale);
        assert!(!r.boxes().iter().any(|b| b.id == "child"));
        assert_eq!(app.selected, Some(1));
    }
}
#[test]
fn tree_duplicate_values_and_oversized_models_are_rejected() {
    let ui: Element<App> =
        tree_view([tree_node(0, "Root")
            .key("root")
            .children([tree_node(0, "Duplicate").key("child")])])
        .key("tree")
        .on_select(|_: &mut App, _| {})
        .on_expand(|_, _, _| {})
        .into();
    assert!(ui.build().is_err());
    let ui: Element<App> =
        tree_view((0..129).map(|id| tree_node(id, "Row").key(format!("node-{id}"))))
            .key("big")
            .on_select(|_: &mut App, _| {})
            .on_expand(|_, _, _| {})
            .into();
    assert!(ui.build().is_err());
}
