#![cfg(feature = "rust-ui")]
//! The zoomable sunburst: angular partition, per-arc hits that follow the
//! ring, a centre that goes back up, and d3's rainbow palette.
use std::f32::consts::{PI, TAU};
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::{prelude::*, rainbow, sunburst_layout},
    Viewport,
};
fn nodes() -> Vec<HierarchyNode> {
    vec![
        HierarchyNode::new("root", "Projects", 100, None),
        HierarchyNode::new("rust", "Rust", 50, Some(0)),
        HierarchyNode::new("target", "Build", 30, Some(1)),
        HierarchyNode::new("deep", "Deep", 30, Some(2)),
        HierarchyNode::new("web", "Web", 50, Some(0)),
    ]
}
#[test]
fn arcs_partition_the_circle_by_value_and_stop_at_the_ring_limit() {
    let arcs = sunburst_layout(&nodes(), 2).unwrap();
    let find = |n: usize| arcs.iter().find(|a| a.node == n).copied();
    let rust = find(1).unwrap();
    let web = find(4).unwrap();
    assert_eq!((rust.depth, rust.start), (1, 0.));
    assert!((rust.end - PI).abs() < 1e-4 && (web.end - TAU).abs() < 1e-4);
    // A child takes its share of the parent's sweep, starting where it starts.
    let target = find(2).unwrap();
    assert_eq!(target.depth, 2);
    assert!((target.end - target.start - PI * 0.6).abs() < 1e-4);
    // Ring 3 isn't shown with two rings.
    assert!(find(3).is_none());
    assert_eq!(sunburst_layout(&nodes(), 3).unwrap().len(), 4);
}
#[test]
fn malformed_trees_are_rejected() {
    assert!(sunburst_layout(&[], 2).is_err());
    let mut n = nodes();
    n[2].parent = Some(3);
    assert!(sunburst_layout(&n, 2).is_err());
    let mut n = nodes();
    n[0].parent = Some(0);
    assert!(sunburst_layout(&n, 2).is_err());
    let bad: Element<()> = sunburst_chart(n).into();
    assert!(bad.build().is_err());
}
#[test]
fn rainbow_matches_d3_endpoints() {
    // d3.interpolateRainbow(0) is rgb(110, 64, 170); 0.5 is rgb(175, 240, 91).
    assert_eq!(rainbow(0.), "#6E40AA");
    assert_eq!(rainbow(0.5), "#AFF05B");
    assert_eq!(rainbow(1.), rainbow(0.));
}

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
#[derive(Default)]
struct App {
    picked: Option<String>,
    up: u32,
    menus: Vec<(String, [f32; 2])>,
}
fn view() -> Element<App> {
    sunburst_chart(nodes())
        .key("rings")
        .size(300.)
        .keyboard_navigation(true)
        .reviewed_keys(["web".into()])
        .on_select(|a: &mut App, key| a.picked = Some(key))
        .on_context_open(|a: &mut App, key, point| a.menus.push((key, point)))
        .on_center("Go up", |a: &mut App| a.up += 1)
        .into()
}
fn mount() -> (Runtime, Handlers<'static, App>) {
    let (d, h) = view().build().unwrap().into_parts();
    let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
    rt.layout(
        Viewport::new(320., 320., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    (rt, h)
}
fn click(rt: &mut Runtime, h: &mut Handlers<'static, App>, app: &mut App, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up_outcome(x, y, h, app).unwrap();
}
#[test]
fn arcs_follow_the_ring_and_the_centre_goes_up() {
    let (mut rt, mut h) = mount();
    let b = rt
        .boxes()
        .iter()
        .find(|b| b.id == "rings")
        .unwrap()
        .bounds
        .bounds();
    let (cx, cy) = (b[0] + 150., b[1] + 150.);
    // Three rings of 50px: hole 0–50, ring 1 50–100, ring 2 100–150.
    let mut app = App::default();
    click(&mut rt, &mut h, &mut app, cx + 75., cy);
    assert_eq!(app.picked.as_deref(), Some("rust"), "right side, ring 1");
    click(&mut rt, &mut h, &mut app, cx - 75., cy);
    assert_eq!(app.picked.as_deref(), Some("web"), "left side, ring 1");
    click(&mut rt, &mut h, &mut app, cx + 125., cy - 20.);
    assert_eq!(app.picked.as_deref(), Some("target"), "ring 2, inside Rust");
    // Rust's empty remainder in ring 2 picks nothing.
    app.picked = None;
    click(&mut rt, &mut h, &mut app, cx + 20., cy + 125.);
    assert_eq!(app.picked, None);
    click(&mut rt, &mut h, &mut app, cx + 4., cy + 4.);
    assert_eq!((app.up, app.picked.as_deref()), (1, None));
}
#[test]
fn every_arc_and_the_centre_have_spoken_labels() {
    let (rt, _) = mount();
    let nodes = rt.accessibility_nodes();
    let label = |id: &str| nodes.iter().find(|n| n.id == id).map(|n| n.label.clone());
    assert_eq!(label("rings-tile-1").as_deref(), Some("Rust — 50"));
    assert_eq!(
        label("rings-tile-4").as_deref(),
        Some("Web — 50 — In review")
    );
    assert_eq!(label("rings-center").as_deref(), Some("Go up"));
}
#[test]
fn menus_open_on_the_arc_and_arrows_move_around_and_out() {
    let (mut rt, mut h) = mount();
    let mut app = App::default();
    let b = rt
        .boxes()
        .iter()
        .find(|b| b.id == "rings")
        .unwrap()
        .bounds
        .bounds();
    let (cx, cy) = (b[0] + 150., b[1] + 150.);
    assert!(
        rt.context_menu_at(cx - 75., cy, &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.menus.last().unwrap(), &("web".into(), [cx - 75., cy]));
    assert!(
        !rt.context_menu_at(cx, cy, &mut h, &mut app)
            .unwrap()
            .activated,
        "not the hole"
    );
    let mut key = |rt: &mut Runtime, app: &mut App, key: InputKey| {
        let event = InputKeyEvent {
            key,
            modifiers: InputModifiers::default(),
            repeat: false,
        };
        assert!(
            !rt.input_key(event, &mut h, app).unwrap().activated,
            "arrows only move focus"
        );
    };
    rt.accessibility_focus("rings-tile-1");
    key(&mut rt, &mut app, InputKey::ArrowRight);
    assert_eq!(rt.focused(), Some("rings-tile-4"), "around the ring");
    key(&mut rt, &mut app, InputKey::ArrowLeft);
    key(&mut rt, &mut app, InputKey::ArrowDown);
    assert_eq!(rt.focused(), Some("rings-tile-2"), "out to a child");
    key(&mut rt, &mut app, InputKey::ArrowUp);
    assert_eq!(rt.focused(), Some("rings-tile-1"), "back to the parent");
    // A keyboard menu opens at the focused arc, on the right of the centre.
    assert!(rt.context_menu_focused(&mut h, &mut app).unwrap().activated);
    let [x, _] = app.menus.last().unwrap().1;
    assert!(x > cx + 20., "{x} vs centre {cx}");
    assert!(app.picked.is_none());
}
#[test]
fn new_data_tweens_the_arcs_once_and_a_new_root_does_not() {
    let grown = || {
        let mut n = nodes();
        n[0].value = 160;
        n[4].value = 110;
        n
    };
    let chart = |data: Vec<HierarchyNode>, revision: u64| -> Element<App> {
        sunburst_chart(data)
            .key("rings")
            .size(300.)
            .animate_from(nodes(), revision)
            .into()
    };
    let (d, h) = chart(grown(), 1).build().unwrap().into_parts();
    let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(320., 320., 1.).unwrap();
    rt.advance_animations(std::time::Duration::from_secs(1));
    rt.layout(vp, &Bindings::default(), &mut Metrics).unwrap();
    assert!(rt.is_animating(), "the arcs tween to the new values");
    rt.advance_animations(std::time::Duration::from_millis(1400));
    assert!(!rt.is_animating(), "and settle");
    // The same revision recomposed (a hover, say) doesn't replay it.
    let (d, h) = chart(grown(), 1).build().unwrap().into_parts();
    rt.replace_document(d, &h, &Bindings::default()).unwrap();
    rt.layout(vp, &Bindings::default(), &mut Metrics).unwrap();
    assert!(!rt.is_animating());
    // Unchanged data, or a different root (a zoom), draws without a tween
    // (the old labels just finish fading out).
    let (d, h) = chart(nodes(), 2).build().unwrap().into_parts();
    rt.replace_document(d, &h, &Bindings::default()).unwrap();
    rt.layout(vp, &Bindings::default(), &mut Metrics).unwrap();
    rt.advance_animations(std::time::Duration::from_millis(1700));
    assert!(!rt.is_animating());
    let mut zoomed = grown();
    zoomed[0].key = "elsewhere".into();
    let (d, h) = chart(zoomed, 3).build().unwrap().into_parts();
    rt.replace_document(d, &h, &Bindings::default()).unwrap();
    rt.layout(vp, &Bindings::default(), &mut Metrics).unwrap();
    rt.advance_animations(std::time::Duration::from_millis(2100));
    assert!(!rt.is_animating());
}
