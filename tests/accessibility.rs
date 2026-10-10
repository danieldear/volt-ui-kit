#![cfg(feature = "rust-ui")]
//! The accessibility snapshot and actions a screen-reader adapter relies on.
use volt_ui_kit::{
    declarative::{AccessRole, AccessValue, Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, w: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(w), size * 1.2]
    }
}
#[derive(Default)]
struct App {
    saves: usize,
    alerts: bool,
    name: String,
    secret: String,
    volume: f32,
    open: bool,
}
fn view(app: &App) -> Element<App> {
    vstack![
        text("Profile"),
        button("Save")
            .key("save")
            .on_click(|app: &mut App| app.saves += 1),
        toggle("Alerts", app.alerts)
            .key("alerts")
            .on_change(|app: &mut App, v| app.alerts = v),
        text_field(&app.name)
            .key("name")
            .placeholder("Your name")
            .on_change(|app: &mut App, v| app.name = v),
        text_field(&app.secret)
            .key("secret")
            .placeholder("Password")
            .password(true)
            .on_change(|app: &mut App, v| app.secret = v),
        slider(app.volume)
            .key("volume")
            .range(0.0..=10.0)
            .step(1.)
            .on_change(|app: &mut App, v| app.volume = v),
        dialog(
            button("Close")
                .key("close")
                .on_click(|app: &mut App| app.open = false)
        )
        .key("modal")
        .open(app.open)
        .on_dismiss(|app: &mut App| app.open = false),
    ]
    .into()
}
fn mount(app: &App) -> (Runtime, Handlers<'static, App>) {
    let (document, handlers) = view(app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(600., 800., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    (runtime, handlers)
}
fn rebuild(app: &App, runtime: &mut Runtime, handlers: &mut Handlers<'static, App>) {
    let (document, next) = view(app).build().unwrap().into_parts();
    runtime
        .replace_document(document, &next, &Bindings::default())
        .unwrap();
    *handlers = next;
    runtime
        .layout(
            Viewport::new(600., 800., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
}

#[test]
fn roles_labels_states_and_hidden_passwords() {
    let app = App {
        secret: "hunter2".into(),
        volume: 3.,
        ..Default::default()
    };
    let (runtime, _) = mount(&app);
    let nodes = runtime.accessibility_nodes();
    let find = |id: &str| {
        nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    assert!(nodes
        .iter()
        .any(|n| n.role == AccessRole::Label && n.label == "Profile"));
    assert_eq!(find("save").role, AccessRole::Button);
    assert_eq!(find("save").label, "Save");
    assert_eq!(find("alerts").role, AccessRole::Switch);
    assert_eq!(find("alerts").checked, Some(false));
    assert_eq!(find("name").role, AccessRole::TextInput);
    assert_eq!(find("name").label, "Your name");
    let secret = find("secret");
    assert!(secret.password);
    assert_eq!(
        secret.value, None,
        "password text and length are never exposed"
    );
    assert_eq!(find("name").value.as_deref(), Some(""));
    assert_eq!(find("volume").role, AccessRole::Slider);
    assert_eq!(find("volume").numeric, Some([3., 0., 10., 1.]));
    // Closed dialogs expose nothing.
    assert!(nodes.iter().all(|n| n.id != "close"));
    assert!(nodes.iter().all(|n| n.bounds[2] > 0. && n.bounds[3] > 0.));
}

#[test]
fn screen_reader_actions_reach_application_state() {
    let mut app = App::default();
    let (mut runtime, mut handlers) = mount(&app);
    assert!(
        runtime
            .accessibility_activate("save", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.saves, 1);
    assert_eq!(runtime.focused(), Some("save"));
    runtime
        .accessibility_activate("alerts", &mut handlers, &mut app)
        .unwrap();
    assert!(app.alerts);
    runtime
        .accessibility_set_value(
            "name",
            AccessValue::Text("Ada".into()),
            &mut handlers,
            &mut app,
        )
        .unwrap();
    assert_eq!(app.name, "Ada");
    rebuild(&app, &mut runtime, &mut handlers);
    runtime
        .accessibility_set_value("volume", AccessValue::Number(6.4), &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.volume, 6., "values snap to the slider step");
    rebuild(&app, &mut runtime, &mut handlers);
    runtime
        .accessibility_step("volume", true, &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.volume, 7.);
    // Focus moves only to reachable, enabled controls.
    assert!(!runtime.accessibility_focus("missing"));
}

#[test]
fn an_open_modal_hides_the_content_behind_it() {
    let mut app = App {
        open: true,
        ..Default::default()
    };
    let (mut runtime, mut handlers) = mount(&app);
    let ids: Vec<_> = runtime
        .accessibility_nodes()
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert!(ids.contains(&"close".to_string()));
    assert!(
        !ids.contains(&"save".to_string()),
        "content behind the modal is inert"
    );
    // Actions outside the modal are refused, like pointer and keyboard input.
    assert!(
        !runtime
            .accessibility_activate("save", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.saves, 0);
}

/// The node budget admits dense real screens (forms, tables, dashboards).
#[test]
fn four_thousand_node_screens_build_and_lay_out() {
    let rows: Vec<Element<App>> = (0..1300)
        .map(|i| {
            hstack![
                text(format!("Row {i}")),
                button("Edit")
                    .key(format!("edit-{i}"))
                    .on_click(|app: &mut App| app.saves += 1),
            ]
            .into()
        })
        .collect();
    let deep = (0..40).fold(Element::from(text("leaf")), |inner, _| {
        vstack![inner].into()
    });
    let ui: Element<App> = vstack![vstack(rows).scroll(Scroll::Vertical).height(600.), deep].into();
    let start = std::time::Instant::now();
    let (document, handlers) = ui
        .build()
        .expect("4k nodes and depth 40 are within limits")
        .into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(800., 900., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    // Rows scrolled out of view are laid out but not kept as visible boxes.
    assert!(runtime.boxes().iter().any(|b| b.id == "edit-0"));
    eprintln!(
        "4k-node build+layout: {:.1}ms",
        start.elapsed().as_secs_f64() * 1000.
    );
    // Past the budget, building fails with an error instead of degrading.
    let too_many: Vec<Element<App>> = (0..4200).map(|i| text(format!("{i}")).into()).collect();
    assert!(Element::<App>::from(vstack(too_many)).build().is_err());
}
