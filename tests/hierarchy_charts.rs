#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Document, DocumentPainter, Handlers, Runtime, TextMetrics},
    ui::{icicle_layout, prelude::*, treemap_layout},
    Painter, UiTheme, Viewport,
};
fn nodes() -> Vec<HierarchyNode> {
    vec![
        HierarchyNode::new("root", "Projects", 100, None),
        HierarchyNode::new("rust", "Rust", 60, Some(0)),
        HierarchyNode::new("target", "Build", 40, Some(1)),
        HierarchyNode::new("src", "Source", 20, Some(1)),
        HierarchyNode::new("web", "Web", 40, Some(0)),
    ]
}
#[test]
fn icicle_preserves_parent_width_and_depth() {
    let r = icicle_layout(&nodes(), 1000., 300., 3).unwrap();
    let a = r.iter().find(|r| r.node == 1).unwrap();
    let b = r.iter().find(|r| r.node == 2).unwrap();
    assert_eq!(a.width, 600.);
    assert_eq!(b.width, 400.);
    assert_eq!(b.y, 200.);
    assert!(b.x >= a.x && b.x + b.width <= a.x + a.width + 0.01);
}
#[test]
fn treemap_area_matches_values_without_overlap() {
    for count in [1, 2, 5, 30, 100] {
        let mut n = vec![HierarchyNode::new("root", "Root", (1..=count).sum(), None)];
        for i in 1..=count {
            n.push(HierarchyNode::new(i.to_string(), i.to_string(), i, Some(0)));
        }
        let r = treemap_layout(&n, 720., 280.).unwrap();
        let total = r.iter().map(|r| r.width * r.height).sum::<f32>();
        assert!((total - 720. * 280.).abs() < 1.);
        for a in &r {
            assert!(
                a.x >= -0.01 && a.y >= -0.01 && a.x + a.width <= 720.01 && a.y + a.height <= 280.01
            );
            for b in &r {
                if a.node == b.node {
                    continue;
                }
                let overlap = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
                let y = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
                assert!(overlap < 0.01 || y < 0.01);
            }
        }
    }
}
#[test]
fn malformed_and_nonfinite_data_rejected() {
    assert!(icicle_layout(&nodes(), f32::NAN, 200., 4).is_err());
    let mut n = nodes();
    n[1].parent = Some(2);
    assert!(treemap_layout(&n, 600., 200.).is_err());
    let mut n = nodes();
    n[1].key = "root".into();
    assert!(icicle_layout(&n, 600., 200., 3).is_err());
    let mut n = nodes();
    n[0].value = 1;
    assert!(treemap_layout(&n, 600., 200.).is_err());
}
#[test]
fn zero_and_unrepresented_weights_never_inflated() {
    let n = vec![
        HierarchyNode::new("r", "Root", 100, None),
        HierarchyNode::new("a", "A", 20, Some(0)),
        HierarchyNode::new("z", "Zero", 0, Some(0)),
    ];
    let r = treemap_layout(&n, 100., 100.).unwrap();
    assert_eq!(r.len(), 1);
    assert!((r[0].width * r[0].height - 2000.).abs() < 0.01);
    assert!(
        icicle_layout(&[HierarchyNode::new("z", "Z", 0, None)], 100., 100., 2)
            .unwrap()
            .is_empty()
    );
}
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, s: f32, max: f32) -> [f32; 2] {
        [(t.len() as f32 * s * 0.5).min(max), s * 1.2]
    }
}
#[test]
fn both_charts_dispatch_pointer_and_keyboard_with_passive_overlays() {
    for treemap in [false, true] {
        let builder = if treemap {
            treemap_chart(nodes())
        } else {
            icicle_chart(nodes())
        };
        let chart: Element<String> = builder
            .key("map")
            .size(600., 240.)
            .levels(3)
            .on_select(|a: &mut String, k| *a = k)
            .into();
        let (d, mut h) = chart.build().unwrap().into_parts();
        let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
        rt.layout(
            Viewport::new(650., 300., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
        let b = rt
            .boxes()
            .iter()
            .find(|b| b.id == "map-tile-1")
            .unwrap()
            .bounds
            .bounds();
        let (x, y) = (b[0] + b[2] / 2., b[1] + b[3] / 2.);
        let mut app = String::new();
        rt.pointer_down(x, y);
        rt.pointer_up_outcome(x, y, &mut h, &mut app).unwrap();
        assert_eq!(app, "rust");
        rt.focus_next(false);
        rt.key_down();
        rt.key_up(&mut h, &mut app).unwrap();
        assert!(!app.is_empty());
    }
}

#[derive(Default)]
struct ChartApp {
    picks: Vec<String>,
    menus: Vec<(String, [f32; 2])>,
}
fn chart() -> HierarchyChart<ChartApp> {
    icicle_chart(nodes())
        .key("chart")
        .size(600., 240.)
        .levels(3)
        .on_select(|a: &mut ChartApp, key| a.picks.push(key))
}
fn mount_chart(ui: Element<ChartApp>, scale: f32) -> (Runtime, Handlers<'static, ChartApp>) {
    let (document, handlers) = ui.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(700. * scale, 500. * scale, scale).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    (runtime, handlers)
}
fn bounds(runtime: &Runtime, id: &str) -> [f32; 4] {
    runtime
        .boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds()
}
fn center(runtime: &Runtime, id: &str) -> [f32; 2] {
    let [x, y, w, h] = bounds(runtime, id);
    [x + w / 2., y + h / 2.]
}
fn arrow(
    runtime: &mut Runtime,
    handlers: &mut Handlers<'_, ChartApp>,
    app: &mut ChartApp,
    key: InputKey,
) {
    let result = runtime
        .input_key(
            InputKeyEvent {
                key,
                modifiers: InputModifiers::default(),
                repeat: false,
            },
            handlers,
            app,
        )
        .unwrap();
    assert!(
        !result.activated,
        "roving focus must not select or request application recomposition"
    );
}
#[test]
fn multiple_selection_and_review_have_distinct_accessible_states() {
    let (mut runtime, mut handlers) = mount_chart(
        chart()
            .selected_keys(["rust".into(), "web".into(), "rust".into()])
            .reviewed_keys(["target".into()])
            .into(),
        1.,
    );
    let access = runtime.accessibility_nodes();
    assert_eq!(runtime.accessibility_multiselectable_ids(), ["chart"]);
    for (index, selected) in [(0, false), (1, true), (2, false), (3, false), (4, true)] {
        let node = access
            .iter()
            .find(|n| n.id == format!("chart-tile-{index}"))
            .unwrap();
        assert_eq!(node.selected, Some(selected));
        assert_eq!(
            node.checked, None,
            "review status must never impersonate a checkbox"
        );
        assert_eq!(node.label.contains("In review"), index == 2);
    }
    assert!(runtime.boxes().iter().any(|b| b.id == "chart-review-2"));
    let [x, y] = center(&runtime, "chart-review-2");
    let mut app = ChartApp::default();
    runtime.pointer_down(x, y);
    assert!(
        runtime
            .pointer_up_outcome(x, y, &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(
        app.picks,
        ["target"],
        "passive diamond must not intercept its tile"
    );
}
#[test]
fn legacy_selected_replaces_multi_selection_and_empty_selection_is_allowed() {
    for (builder, expected) in [
        (
            chart()
                .selected_keys(["rust".into(), "web".into()])
                .selected("target"),
            vec!["chart-tile-2"],
        ),
        (
            chart().selected("rust").selected_keys(Vec::<String>::new()),
            vec![],
        ),
    ] {
        let (runtime, _) = mount_chart(builder.into(), 1.);
        let selected: Vec<_> = runtime
            .accessibility_nodes()
            .into_iter()
            .filter(|n| n.selected == Some(true))
            .map(|n| n.id)
            .collect();
        assert_eq!(selected, expected);
    }
}
#[test]
fn hidden_root_fills_visible_bands_but_preserves_inclusive_weights() {
    let (runtime, _) = mount_chart(chart().hide_root(true).levels(2).into(), 1.);
    assert!(!runtime.boxes().iter().any(|b| b.id == "chart-tile-0"));
    assert_eq!(bounds(&runtime, "chart-tile-1"), [0., 0., 358., 118.]);
    assert_eq!(bounds(&runtime, "chart-tile-2"), [0., 120., 238., 118.]);
    assert_eq!(bounds(&runtime, "chart-tile-4"), [360., 0., 238., 118.]);
    let sparse = vec![
        HierarchyNode::new("root", "Root", 100, None),
        HierarchyNode::new("child", "Child", 20, Some(0)),
    ];
    let (runtime, _) = mount_chart(
        icicle_chart(sparse)
            .key("chart")
            .size(600., 240.)
            .levels(1)
            .hide_root(true)
            .on_select(|_: &mut ChartApp, _| {})
            .into(),
        1.,
    );
    assert_eq!(bounds(&runtime, "chart-tile-1"), [0., 0., 118., 238.]);
    let (runtime, _) = mount_chart(
        icicle_chart(vec![HierarchyNode::new("root", "Root", 1, None)])
            .hide_root(true)
            .on_select(|_: &mut ChartApp, _| {})
            .into(),
        1.,
    );
    assert!(!runtime.accessibility_nodes().iter().any(|n| n.clickable));
}
#[test]
fn hide_root_does_not_change_treemap_layout() {
    let make = |hide| {
        treemap_chart(nodes())
            .key("chart")
            .size(600., 240.)
            .hide_root(hide)
            .on_select(|_: &mut ChartApp, _| {})
            .into()
    };
    let (normal, _) = mount_chart(make(false), 1.);
    let (hidden, _) = mount_chart(make(true), 1.);
    let rectangles = |runtime: &Runtime| {
        runtime
            .boxes()
            .iter()
            .filter(|b| b.id.starts_with("chart-tile-"))
            .map(|b| (b.id.clone(), b.bounds.bounds()))
            .collect::<Vec<_>>()
    };
    assert_eq!(rectangles(&normal), rectangles(&hidden));
}
#[test]
fn roving_chart_has_one_tab_stop_arrows_move_focus_and_enter_selects() {
    let ui = vstack![
        button("Before")
            .key("before")
            .on_click(|_: &mut ChartApp| {}),
        chart().keyboard_navigation(true).selected("rust"),
        button("After").key("after").on_click(|_: &mut ChartApp| {})
    ]
    .into();
    let (mut runtime, mut handlers) = mount_chart(ui, 1.);
    let mut app = ChartApp::default();
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("before"));
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("chart-tile-1"));
    for (key, target) in [
        (InputKey::ArrowRight, "chart-tile-4"),
        (InputKey::Home, "chart-tile-1"),
        (InputKey::End, "chart-tile-4"),
        (InputKey::ArrowLeft, "chart-tile-1"),
        (InputKey::ArrowDown, "chart-tile-2"),
        (InputKey::ArrowRight, "chart-tile-3"),
        (InputKey::ArrowUp, "chart-tile-1"),
        (InputKey::ArrowUp, "chart-tile-0"),
        (InputKey::ArrowLeft, "chart-tile-0"),
        (InputKey::ArrowDown, "chart-tile-1"),
    ] {
        arrow(&mut runtime, &mut handlers, &mut app, key);
        assert_eq!(runtime.focused(), Some(target));
        assert!(app.picks.is_empty());
    }
    runtime.key_down();
    runtime.key_up(&mut handlers, &mut app).unwrap();
    assert_eq!(app.picks, ["rust"]);
    arrow(&mut runtime, &mut handlers, &mut app, InputKey::ArrowRight);
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("after"));
    runtime.focus_next(true);
    assert_eq!(
        runtime.focused(),
        Some("chart-tile-4"),
        "return to remembered tile, not declared entry"
    );
    runtime.focus_next(true);
    assert_eq!(runtime.focused(), Some("before"));
}
#[test]
fn opt_out_keeps_legacy_tab_order_and_roving_remembers_identity_after_reordering() {
    let (mut legacy, _) = mount_chart(chart().into(), 1.);
    for index in [0, 1, 2, 3, 4, 0] {
        legacy.focus_next(false);
        assert_eq!(
            legacy.focused(),
            Some(format!("chart-tile-{index}").as_str())
        );
    }
    let (mut runtime, _) =
        mount_chart(chart().keyboard_navigation(true).selected("web").into(), 1.);
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("chart-tile-4"));
    let reordered = vec![
        HierarchyNode::new("root", "Projects", 100, None),
        HierarchyNode::new("web", "Web", 40, Some(0)),
        HierarchyNode::new("rust", "Rust", 60, Some(0)),
        HierarchyNode::new("target", "Build", 40, Some(2)),
        HierarchyNode::new("src", "Source", 20, Some(2)),
    ];
    let ui: Element<ChartApp> = icicle_chart(reordered)
        .key("chart")
        .size(600., 240.)
        .keyboard_navigation(true)
        .on_select(|a: &mut ChartApp, key| a.picks.push(key))
        .into();
    let (document, mut handlers) = ui.build().unwrap().into_parts();
    runtime
        .replace_document(document, &handlers, &Bindings::default())
        .unwrap();
    runtime
        .layout(
            Viewport::new(700., 500., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    assert_eq!(
        runtime.focused(),
        Some("chart-tile-1"),
        "Web keeps focus despite index changing"
    );
    let mut app = ChartApp::default();
    runtime.key_down();
    runtime.key_up(&mut handlers, &mut app).unwrap();
    assert_eq!(app.picks, ["web"]);
    // Replacing that identity with a new node at the same tile ID must not steal focus.
    let ui: Element<ChartApp> = icicle_chart(vec![
        HierarchyNode::new("root", "Projects", 100, None),
        HierarchyNode::new("different", "Different", 100, Some(0)),
    ])
    .key("chart")
    .size(600., 240.)
    .keyboard_navigation(true)
    .on_select(|_: &mut ChartApp, _| {})
    .into();
    let (document, handlers) = ui.build().unwrap().into_parts();
    runtime
        .replace_document(document, &handlers, &Bindings::default())
        .unwrap();
    runtime
        .layout(
            Viewport::new(700., 500., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    assert_eq!(runtime.focused(), None);
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("chart-tile-0"));
}
#[test]
fn hidden_root_roving_accessibility_uses_visible_parent_and_sibling_metadata() {
    let (mut runtime, mut handlers) =
        mount_chart(chart().hide_root(true).keyboard_navigation(true).into(), 1.);
    let access = runtime.accessibility_nodes();
    let root = access.iter().position(|n| n.id == "chart").unwrap();
    let rust = access.iter().position(|n| n.id == "chart-tile-1").unwrap();
    let target = access.iter().find(|n| n.id == "chart-tile-2").unwrap();
    assert_eq!(
        access[root].role,
        volt_ui_kit::declarative::AccessRole::Tree
    );
    assert_eq!(access[rust].parent, Some(root));
    assert_eq!(access[rust].level, Some(0));
    assert_eq!(access[rust].position_in_set, Some(0));
    assert_eq!(access[rust].size_of_set, Some(2));
    assert_eq!(target.parent, Some(rust));
    assert_eq!(target.level, Some(1));
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("chart-tile-1"));
    arrow(
        &mut runtime,
        &mut handlers,
        &mut ChartApp::default(),
        InputKey::ArrowUp,
    );
    assert_eq!(
        runtime.focused(),
        Some("chart-tile-1"),
        "hidden root is never a focus target"
    );
    let [x, y, w, h] = bounds(&runtime, "chart-tile-1");
    assert!(x >= 4. && y >= 4. && x + w <= 596. && y + h <= 236.);
}
#[test]
fn pointer_and_keyboard_context_requests_use_node_and_window_logical_coordinates() {
    for scale in [1., 2.] {
        let ui = vstack![chart()
            .keyboard_navigation(true)
            .reviewed_keys(["target".into()])
            .on_context_open(|a: &mut ChartApp, key, point| a.menus.push((key, point)))]
        .padding(20., Edge::All)
        .into();
        let (mut runtime, mut handlers) = mount_chart(ui, scale);
        let mut app = ChartApp::default();
        for index in [0, 1, 2, 4] {
            let point = center(&runtime, &format!("chart-tile-{index}"));
            assert!(
                runtime
                    .context_menu_at(point[0], point[1], &mut handlers, &mut app)
                    .unwrap()
                    .activated
            );
            assert_eq!(
                app.menus.last().unwrap(),
                &(
                    nodes()[index].key.clone(),
                    [point[0] / scale, point[1] / scale]
                )
            );
        }
        let marker = center(&runtime, "chart-review-2");
        assert!(
            runtime
                .context_menu_at(marker[0], marker[1], &mut handlers, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.menus.last().unwrap().0, "target");
        assert!(
            !runtime
                .context_menu_at(1., 1., &mut handlers, &mut app)
                .unwrap()
                .activated
        );
        assert!(
            !runtime
                .context_menu_at(f32::NAN, 0., &mut handlers, &mut app)
                .unwrap()
                .activated
        );
        runtime.accessibility_focus("chart-tile-0");
        arrow(&mut runtime, &mut handlers, &mut app, InputKey::ArrowDown);
        let point = center(&runtime, "chart-tile-1");
        assert!(
            runtime
                .context_menu_focused(&mut handlers, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(
            app.menus.last().unwrap(),
            &("rust".into(), [point[0] / scale, point[1] / scale])
        );
        assert!(
            app.picks.is_empty(),
            "opening context never selects or activates a node"
        );
    }
}
#[test]
fn legacy_document_buttons_still_default_to_tab_stops_and_opt_out_remains_pointer_accessible() {
    let document = Document::parse(r#"{"schema_version":1,"root":{"type":"column","id":"root","children":[{"type":"button","id":"a","label":"A","on_activate":"pick"},{"type":"button","id":"b","label":"B","on_activate":"pick"}]}}"#).unwrap();
    let mut handlers = Handlers::default();
    handlers.on_activate("pick", |_: &mut ChartApp| {}).unwrap();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(700., 500., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("a"));
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("b"));
    let (mut runtime, mut handlers) = mount_chart(
        vstack![
            button("Skipped")
                .key("skip")
                .tab_stop(false)
                .on_click(|a: &mut ChartApp| a.picks.push("skip".into())),
            button("Tab").key("tab").on_click(|_: &mut ChartApp| {})
        ]
        .into(),
        1.,
    );
    runtime.focus_next(false);
    assert_eq!(runtime.focused(), Some("tab"));
    let [x, y] = center(&runtime, "skip");
    let mut app = ChartApp::default();
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y, &mut handlers, &mut app).unwrap();
    assert_eq!(app.picks, ["skip"]);
    runtime.accessibility_focus("skip");
    assert_eq!(runtime.focused(), Some("skip"));
}
#[test]
fn invalid_chart_state_is_bounded_and_never_silently_truncated() {
    for builder in [
        chart().selected_keys(std::iter::repeat_n("root".to_string(), 4097)),
        chart().reviewed_keys([String::new()]),
        chart().node_colors([("root".into(), "not-a-color".into())]),
        chart().keyboard_navigation(true).size(11., 50.),
        chart().hide_root(true).size(f32::NAN, 20.),
    ] {
        let element: Element<ChartApp> = builder.into();
        assert!(element.build().is_err());
    }
}

type RecordedShape = ([f32; 4], [f32; 4]);
type RecordedGlyph = (String, [f32; 4]);
type RecordedBatch = (Vec<RecordedShape>, Vec<RecordedGlyph>);
struct PaintRecorder;
impl TextMetrics for PaintRecorder {
    fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
        Metrics.measure(text, size, width)
    }
}
impl Painter for PaintRecorder {
    type Shape = RecordedShape;
    type Glyph = RecordedGlyph;
    fn viewport(&self) -> Viewport {
        Viewport::new(700., 500., 1.).unwrap()
    }
    fn draw_rect(
        &self,
        out: &mut Vec<Self::Shape>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) {
        out.push(([x, y, w, h], color));
    }
    fn card_round(&self, out: &mut Vec<Self::Shape>, rect: [f32; 4], _: f32, color: [f32; 4]) {
        out.push((rect, color));
    }
    fn card_triangle(&self, _: &mut Vec<Self::Shape>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(
        &mut self,
        out: &mut Vec<Self::Glyph>,
        text: &str,
        _: f32,
        _: f32,
        _: f32,
        color: [f32; 4],
    ) {
        out.push((text.into(), color));
    }
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<Self::Glyph>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for PaintRecorder {
    fn ui_text(
        &mut self,
        out: &mut Vec<Self::Glyph>,
        text: &str,
        _: [f32; 4],
        _: f32,
        color: [f32; 4],
    ) {
        out.push((text.into(), color));
    }
}
#[test]
fn node_color_override_is_exact_and_default_palette_and_layout_are_unchanged() {
    let paint = |builder: HierarchyChart<ChartApp>| {
        let (runtime, _) = mount_chart(builder.into(), 1.);
        runtime
            .paint_ui(&mut PaintRecorder, &UiTheme::dark())
            .into_iter()
            .map(|b| (b.shapes, b.glyphs))
            .collect::<Vec<_>>()
    };
    let default = paint(chart());
    assert_eq!(
        default,
        paint(
            chart()
                .selected_keys([])
                .reviewed_keys([])
                .node_colors([])
                .hide_root(false)
                .keyboard_navigation(false)
        )
    );
    let custom = paint(chart().node_colors([("rust".into(), "#FFFFFF".into())]));
    assert!(custom.iter().flat_map(|b| &b.0).any(|(_, c)| *c == [1.; 4]));
    let glyph_color = |batches: &[RecordedBatch], label: &str| {
        batches
            .iter()
            .flat_map(|b| &b.1)
            .find(|(text, _)| text.starts_with(label))
            .unwrap()
            .1
    };
    assert!(
        glyph_color(&custom, "Rust")[0] < 0.1,
        "light custom fill requires a contrasting foreground"
    );
    assert_eq!(
        glyph_color(&custom, "Build"),
        glyph_color(&default, "Build"),
        "override must not leak into descendants"
    );
}

#[test]
fn context_target_is_passive_and_does_not_change_selection_or_geometry() {
    let make = |target| {
        let c = chart().selected("web");
        let c = if target { c.context_target("rust") } else { c };
        mount_chart(c.into(), 1.)
    };
    let (plain, _) = make(false);
    let (mut marked, mut handlers) = make(true);
    for index in 0..5 {
        assert_eq!(
            bounds(&plain, &format!("chart-tile-{index}")),
            bounds(&marked, &format!("chart-tile-{index}"))
        );
    }
    let access = marked.accessibility_nodes();
    assert_eq!(
        access
            .iter()
            .find(|n| n.id == "chart-tile-1")
            .unwrap()
            .selected,
        Some(false)
    );
    assert_eq!(
        access
            .iter()
            .find(|n| n.id == "chart-tile-4")
            .unwrap()
            .selected,
        Some(true)
    );
    let [x, y] = center(&marked, "chart-tile-1");
    let mut app = ChartApp::default();
    marked.pointer_down(x, y);
    marked
        .pointer_up_outcome(x, y, &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.picks, ["rust"]);
    let invalid: Element<ChartApp> = chart().context_target("").into();
    assert!(invalid.build().is_err());
}

#[test]
fn chart_hover_links_are_opt_in_paint_only_and_keep_review_selection_and_context() {
    for treemap in [false, true] {
        let make = |enabled| -> Element<ChartApp> {
            let chart = if treemap {
                treemap_chart(nodes())
                    .key("chart")
                    .size(600., 240.)
                    .on_select(|a: &mut ChartApp, key| a.picks.push(key))
            } else {
                chart()
            };
            vstack![
                chart
                    .hide_root(true)
                    .hover_linked(enabled)
                    .selected("web")
                    .reviewed_keys(["rust".into()])
                    .context_target("rust"),
                button("List row")
                    .on_click(|_: &mut ChartApp| {})
                    .key("peer")
                    .height(32.)
                    .hover_link("rust")
                    .background("#123456")
                    .hover_style(|s| s.background("#ABCDEF")),
            ]
            .into()
        };
        let (mut linked, mut h) = mount_chart(make(true), 1.);
        let initial_access = linked.accessibility_nodes();
        let initial_focus = linked.focused().map(str::to_owned);
        let before = linked.paint_ui(&mut PaintRecorder, &UiTheme::dark());
        let [x, y] = center(&linked, "chart-tile-1");
        let mut app = ChartApp::default();
        let outcome = linked.pointer_move_outcome(x, y, &mut h, &mut app).unwrap();
        assert!(outcome.changed && !outcome.activated);
        let after = linked.paint_ui(&mut PaintRecorder, &UiTheme::dark());
        assert!(after
            .iter()
            .flat_map(|b| &b.shapes)
            .any(|(_, c)| *c == [171. / 255., 205. / 255., 239. / 255., 1.]));
        assert!(!before
            .iter()
            .flat_map(|b| &b.shapes)
            .any(|(_, c)| *c == [171. / 255., 205. / 255., 239. / 255., 1.]));
        assert!(app.picks.is_empty() && app.menus.is_empty());
        assert_eq!(initial_focus.as_deref(), linked.focused());
        assert_eq!(initial_access, linked.accessibility_nodes());
        let [x, y] = center(&linked, "peer");
        linked.pointer_move(x, y);
        let row_hover = linked.paint_ui(&mut PaintRecorder, &UiTheme::dark());
        assert_ne!(
            before.iter().flat_map(|b| &b.shapes).collect::<Vec<_>>(),
            row_hover.iter().flat_map(|b| &b.shapes).collect::<Vec<_>>()
        );
        linked.pointer_leave();
        let cleared = linked.paint_ui(&mut PaintRecorder, &UiTheme::dark());
        assert_eq!(
            before.iter().flat_map(|b| &b.shapes).collect::<Vec<_>>(),
            cleared.iter().flat_map(|b| &b.shapes).collect::<Vec<_>>()
        );
        let (mut legacy, _) = mount_chart(make(false), 1.);
        let [x, y] = center(&legacy, "chart-tile-1");
        legacy.pointer_move(x, y);
        assert!(!legacy
            .paint_ui(&mut PaintRecorder, &UiTheme::dark())
            .iter()
            .flat_map(|b| b.shapes.iter())
            .any(|(_, c)| *c == [171. / 255., 205. / 255., 239. / 255., 1.]));
    }
}
