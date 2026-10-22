#![cfg(feature = "rust-ui")]
//! Spacing remains caller-owned; constructor constants are only defaults.
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, s: f32, w: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * s * 0.5).min(w), s * 1.2]
    }
}
fn mount(ui: Element<usize>, scale: f32, width: f32) -> Runtime {
    let (d, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(width * scale, 800. * scale, scale).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    r
}
fn b(r: &Runtime, id: &str) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .bounds
        .bounds()
}
fn gap(r: &Runtime, a: &str, z: &str, axis: usize) -> f32 {
    let a = b(r, a);
    let z = b(r, z);
    z[axis] - a[axis] - a[axis + 2]
}
fn tabstrip() -> SelectionControl<usize, u8> {
    tab_bar([tab_item(0, "One").key("one"), tab_item(1, "Two").key("two")])
        .key("strip")
        .on_select(|a: &mut usize, v| *a = v as usize)
}
#[test]
fn tabs_toolbar_and_joined_bar_gaps_keep_defaults_and_explicit_zero() {
    for scale in [1., 2.] {
        for value in [0., 2., 13.] {
            let r = mount(tabstrip().gap(value, Axis::Horizontal).into(), scale, 700.);
            assert!((gap(&r, "one", "two", 0) / scale - value).abs() < 0.01);
            let r = mount(
                tabstrip()
                    .variant(TabVariant::Contained)
                    .gap(value, Axis::Horizontal)
                    .into(),
                scale,
                700.,
            );
            assert!((gap(&r, "one", "two", 0) / scale - value).abs() < 0.01);
            let r = mount(
                tabstrip().joined(true).gap(value, Axis::Horizontal).into(),
                scale,
                700.,
            );
            assert!((gap(&r, "one", "two", 0) / scale - value).abs() < 0.01);
        }
        let r = mount(tabstrip().into(), scale, 700.);
        assert!((gap(&r, "one", "two", 0) / scale - 2.).abs() < 0.01);
        // An irrelevant cross-axis override must not destroy the joined seam.
        let r = mount(
            tabstrip().joined(true).gap(30., Axis::Vertical).into(),
            scale,
            700.,
        );
        assert!((gap(&r, "one", "two", 0) / scale - 1.).abs() < 0.01);
        let r = mount(
            toolbar![
                button("A").key("a").on_click(|_: &mut usize| {}),
                button("B").key("z").on_click(|_: &mut usize| {})
            ]
            .gap(17., Axis::Horizontal)
            .into(),
            scale,
            700.,
        );
        assert!((gap(&r, "a", "z", 0) / scale - 17.).abs() < 0.01);
    }
}
#[test]
fn nested_tabs_distinguish_strip_gap_from_content_gap_and_support_responsive_zero() {
    for width in [600., 1000.] {
        let ui = tabs([
            tab(0, "One", text("Body").key("body")).key("one"),
            tab(1, "Two", text("Unused")).key("two"),
        ])
        .key("tabs")
        .variant(TabVariant::Contained)
        .tab_gap(responsive(0.).at(Breakpoint::Md, 11.))
        .tab_padding(0., Edge::All)
        .gap(19., Axis::Vertical)
        .on_select(|_: &mut usize, _| {});
        let r = mount(ui.into(), 1., width);
        assert!((gap(&r, "one", "two", 0) - if width < 768. { 0. } else { 11. }).abs() < 0.01);
        assert!((gap(&r, "tabs-strip", "body", 1) - 19.).abs() < 0.01);
    }
}
fn make_menu() -> Menu<usize, u8> {
    menu(
        "Actions",
        [
            menu_item(0, "One").key("one").icon(Icon::Copy),
            menu_item(1, "Two").key("two"),
        ],
    )
    .key("menu")
    .open(true)
    .on_action(|a: &mut usize, v| *a = v as usize)
    .on_open_change(|_: &mut usize, _| {})
}
#[test]
fn menu_popup_spacing_has_real_geometry_and_preserves_legacy_defaults() {
    for scale in [1., 2.] {
        for (padding, gap_v, height) in [(6., 2., 34.), (0., 0., 34.), (10., 7., 42.)] {
            let r = mount(
                make_menu()
                    .popup_padding(padding)
                    .item_gap(gap_v)
                    .item_height(height)
                    .popup_max_height(480.)
                    .into(),
                scale,
                700.,
            );
            let one = b(&r, "one");
            let two = b(&r, "two");
            let card = b(&r, "menu-popup-content");
            assert!((one[3] / scale - height).abs() < 0.01);
            assert!((two[3] / scale - height).abs() < 0.01);
            // Decorated row wraps the button, but both have identical row bounds.
            assert!((gap(&r, "one", "two", 1) / scale - gap_v).abs() < 0.01);
            assert!((one[0] / scale - card[0] / scale - padding).abs() < 0.01);
            assert!(
                (card[3] / scale - (2. * padding + 2. * height + gap_v)).abs() < 0.01,
                "card={card:?} padding={padding} gap={gap_v} height={height} scale={scale}"
            );
        }
    }
}
#[test]
fn invalid_spacing_is_rejected_instead_of_clamped_or_hidden() {
    for v in [f32::NAN, f32::INFINITY, -1.] {
        let e: Element<usize> = make_menu().item_gap(v).into();
        assert!(e.build().is_err());
        let e: Element<usize> = make_menu().popup_padding(v).into();
        assert!(e.build().is_err());
        let e: Element<usize> = make_menu().popup_max_height(v).into();
        assert!(e.build().is_err());
        let e: Element<usize> = tabstrip().gap(v, Axis::Horizontal).into();
        assert!(e.build().is_err());
    }
    let e: Element<usize> = make_menu().item_height(10.).into();
    assert!(e.build().is_err());
}
#[test]
fn dialog_popover_tooltip_padding_targets_cards_not_portals() {
    let r = mount(
        dialog(text("Body").key("body"))
            .title("Header")
            .key("dialog")
            .open(true)
            .padding(0., Edge::All)
            .gap(9., Axis::Vertical)
            .on_dismiss(|_: &mut usize| {})
            .into(),
        1.,
        700.,
    );
    let c = b(&r, "dialog-content");
    let a = r
        .boxes()
        .iter()
        .find(|b| b.text() == "Header")
        .unwrap()
        .bounds
        .bounds();
    assert!((a[0] - c[0]).abs() < 0.01 && (a[1] - c[1]).abs() < 0.01);
    let body = b(&r, "body");
    assert!((body[1] - a[1] - a[3] - 9.).abs() < 0.01);
    let ui: Element<usize> = popover(text("Body").key("body"))
        .key("pop")
        .anchor("anchor")
        .open(true)
        .padding(0., Edge::All)
        .gap(0., Axis::Both)
        .on_dismiss(|_: &mut usize| {})
        .into();
    let r = mount(
        vstack![
            button("anchor").key("anchor").on_click(|_: &mut usize| {}),
            ui
        ]
        .into(),
        1.,
        700.,
    );
    assert!(r.boxes().iter().any(|b| b.id == "body"));
    let e: Element<usize> = tooltip("Help", text("Target"))
        .key("tip")
        .padding(0., Edge::All)
        .gap(0., Axis::Both)
        .into();
    assert!(e.build().is_ok());
}
fn table_ui() -> Table<usize, u8, u8> {
    table(
        [table_column(0, "Name").sortable(false)],
        [table_row(0, ["A"]).key("row")],
    )
    .key("table")
    .on_select(|a: &mut usize, v| *a = v as usize)
}
#[test]
fn table_and_tree_density_setters_preserve_selection_targets_and_widths() {
    for scale in [1., 2.] {
        let r = mount(
            table_ui()
                .width(300.)
                .padding(4., Edge::All)
                .row_height(32.)
                .header_height(48.)
                .cell_padding(6.)
                .into(),
            scale,
            700.,
        );
        assert_eq!(b(&r, "row")[3] / scale, 32.);
        assert_eq!(b(&r, "row")[2] / scale, 292.);
        let r = mount(
            tree_view([tree_node(0, "Root")
                .key("root")
                .children([tree_node(1, "Child").key("child")])])
            .key("tree")
            .expanded([0])
            .padding(0., Edge::All)
            .gap(7., Axis::Vertical)
            .row_height(30.)
            .indent_width(12.)
            .on_select(|_: &mut usize, _| {})
            .on_expand(|_: &mut usize, _, _| {})
            .into(),
            scale,
            700.,
        );
        assert_eq!(b(&r, "root")[3] / scale, 30.);
        assert_eq!(b(&r, "child")[3] / scale, 30.);
        assert_eq!(b(&r, "root")[2] / scale, 360.);
        assert!((gap(&r, "root", "child", 1) / scale - 7.).abs() < 0.01);
        let texts = |label: &str| {
            r.boxes()
                .iter()
                .find(|b| b.text() == label)
                .unwrap()
                .bounds
                .bounds()[0]
        };
        assert!(((texts("Child") - texts("Root")) / scale - 12.).abs() < 0.01);
    }
}
