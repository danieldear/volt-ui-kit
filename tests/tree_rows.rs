#![cfg(feature = "rust-ui")]
//! Tree rows with icons, a marker in the disclosure slot, trailing status
//! icons, dimmed rows and a multiple selection.
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
fn mount(ui: Element<u32>) -> Runtime {
    let (d, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(400., 400., 1.).unwrap(),
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
fn tree(trailing: usize, many: Vec<u32>) -> TreeView<u32, u32> {
    let mut photo = tree_node(3, "Photo")
        .key("photo")
        .icon(icon(Icon::Image).size(14.).key("photo-icon"))
        .marker(icon(Icon::ArrowRight).size(12.).key("photo-marker"))
        .muted(true);
    for i in 0..trailing {
        photo = photo.trailing(icon(Icon::Lock).size(14.).key(format!("photo-lock-{i}")));
    }
    tree_view([tree_node(1, "Card")
        .key("card")
        .icon(icon(Icon::Folder).size(14.).key("card-icon"))
        .children([
            tree_node(2, "Mask")
                .key("mask")
                .icon(icon(Icon::Star).size(14.).key("mask-icon")),
            photo,
        ])])
    .key("tree")
    .width(300.)
    .expanded([1])
    .selected(Some(2))
    .selected_many(many)
    .on_select(|a: &mut u32, v| *a = v)
    .on_expand(|_: &mut u32, _, _| {})
}

#[test]
fn rows_carry_an_icon_a_marker_and_trailing_status_icons() {
    let r = mount(tree(1, vec![3]).into());
    let (icon, marker, lock) = (
        b(&r, "photo-icon"),
        b(&r, "photo-marker"),
        b(&r, "photo-lock-0"),
    );
    let row = b(&r, "photo");
    // Marker in the disclosure slot, then the icon, then the trailing icon
    // at the row's far end.
    assert!(
        marker[0] < icon[0],
        "marker {marker:?} before icon {icon:?}"
    );
    assert!(lock[0] > icon[0] + 40., "lock {lock:?} after the label");
    assert!(
        (row[0] + row[2]) - (lock[0] + lock[2]) < 12.,
        "lock at the end: {row:?} {lock:?}"
    );
    // Rows at one depth line their icons up, trailing icons or not.
    assert!(
        (b(&r, "mask-icon")[0] - icon[0]).abs() < 0.5,
        "aligned icons"
    );
    // The parent row keeps its disclosure; its icon still shows.
    assert!(b(&r, "card-icon")[0] > b(&r, "card")[0] + 24.);
    // Both the current row and the other selected row read as selected.
    let selected: Vec<String> = r
        .accessibility_nodes()
        .into_iter()
        .filter(|n| n.selected == Some(true))
        .map(|n| n.id)
        .collect();
    assert!(selected.contains(&"mask".to_owned()) && selected.contains(&"photo".to_owned()));
    assert!(!selected.contains(&"card".to_owned()));
}

#[test]
fn too_many_trailing_icons_or_unknown_selections_are_rejected() {
    let e: Element<u32> = tree(4, vec![]).into();
    assert!(e.build().is_err());
    let e: Element<u32> = tree(3, vec![9]).into();
    assert!(e.build().is_err());
    let e: Element<u32> = tree(3, vec![1, 3]).into();
    assert!(e.build().is_ok());
}
