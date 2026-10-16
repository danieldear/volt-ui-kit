#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    ui::prelude::*,
};

#[test]
fn navigation_is_controlled_typed_and_group_scoped() {
    let view: Element<u8> = hstack![
        navigation([
            navigation_item(1u8, "Inbox").key("inbox"),
            navigation_item(2, "Sent").key("sent"),
            navigation_item(3, "Archive").key("archive").enabled(false)
        ])
        .key("left")
        .selected(1)
        .on_select(|app: &mut u8, value| *app = value),
        navigation([
            navigation_item(1u8, "Inbox").key("inbox"),
            navigation_item(2, "Sent").key("sent")
        ])
        .key("bottom")
        .placement(NavigationPlacement::Bottom)
        .selected(1)
        .on_select(|app: &mut u8, value| *app = value),
    ]
    .into();
    let (doc, mut handlers) = view.build().unwrap().into_parts();
    let bindings = Bindings::default();
    let mut runtime = Runtime::new(doc, &handlers, &bindings).unwrap();
    struct Metrics;
    impl volt_ui_kit::declarative::TextMetrics for Metrics {
        fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
            [(text.len() as f32 * size * 0.5).min(width), size * 1.2]
        }
    }
    runtime
        .layout(
            volt_ui_kit::Viewport::new(600., 400., 1.).unwrap(),
            &bindings,
            &mut Metrics,
        )
        .unwrap();
    let mut app = 1;
    assert!(
        !runtime
            .accessibility_activate("left-archive", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app, 1);
    runtime
        .accessibility_activate("left-inbox", &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app, 1);
    assert!(
        runtime
            .accessibility_activate("left-sent", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app, 2);
}

#[test]
fn navigation_rejects_duplicate_values_and_unstable_item_keys() {
    let duplicate: Element<()> = navigation([
        navigation_item(1u8, "A").key("a"),
        navigation_item(1, "B").key("b"),
    ])
    .key("nav")
    .on_select(|_: &mut (), _| {})
    .into();
    assert!(duplicate.build().is_err());
    let missing: Element<()> = navigation([navigation_item(1u8, "A")])
        .key("nav")
        .on_select(|_: &mut (), _| {})
        .into();
    assert!(missing.build().is_err());
}

#[test]
fn split_view_collapses_the_sidebar_into_a_drawer_below_lg() {
    #[derive(Default)]
    struct App {
        page: u8,
        open: bool,
    }
    let view = |app: &App| -> Element<App> {
        navigation_split_view(
            |place| {
                navigation([
                    navigation_item(0u8, "Inbox").key("inbox"),
                    navigation_item(1, "Sent").key("sent"),
                ])
                .key(place.key("nav"))
                .selected(app.page)
                .on_select(|app: &mut App, page| app.page = page)
            },
            text("Detail").key("page"),
        )
        .key("app")
        .title("Mail")
        .titlebar_inset(false)
        .drawer_open(app.open)
        .on_drawer_change(|app: &mut App, open| app.open = open)
        .into()
    };
    struct Metrics;
    impl volt_ui_kit::declarative::TextMetrics for Metrics {
        fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
            [(text.len() as f32 * size * 0.5).min(width), size * 1.2]
        }
    }
    let mut app = App::default();
    let mount = |app: &App, width: f32| {
        let (doc, handlers) = view(app).build().unwrap().into_parts();
        let bindings = Bindings::default();
        let mut runtime = Runtime::new(doc, &handlers, &bindings).unwrap();
        runtime
            .layout(
                volt_ui_kit::Viewport::new(width, 700., 1.).unwrap(),
                &bindings,
                &mut Metrics,
            )
            .unwrap();
        (runtime, handlers)
    };
    let shown = |runtime: &Runtime, id: &str| runtime.boxes().iter().any(|b| b.id == id);
    // Wide: the sidebar column at its default width; no menu button.
    let (wide, _) = mount(&app, 1280.);
    let column = wide
        .boxes()
        .iter()
        .find(|b| b.id == "app-sidebar-column")
        .unwrap();
    assert_eq!(column.bounds.bounds()[2], 248.);
    assert!(shown(&wide, "nav-inbox") && !shown(&wide, "app-menu"));
    // Narrow: the column hides and the menu button opens the drawer.
    let (mut narrow, mut handlers) = mount(&app, 800.);
    assert!(!shown(&narrow, "nav-inbox") && shown(&narrow, "app-menu"));
    narrow
        .accessibility_activate("app-menu", &mut handlers, &mut app)
        .unwrap();
    assert!(app.open);
    let (drawer, _) = mount(&app, 800.);
    assert!(shown(&drawer, "nav-drawer-inbox"));
}
