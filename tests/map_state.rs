#![cfg(feature = "rust-ui")]
//! A view written for its own state runs unchanged inside a larger app.
use volt_ui_kit::{
    declarative::{Bindings, Handlers, PointerPhase, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}

/// A self-contained screen, as an example program would write it.
#[derive(Default)]
struct Settings {
    clicks: u32,
    alerts: bool,
    name: String,
    fruit: Option<u8>,
    combo: ComboboxState,
}
fn settings(state: &Settings, prefix: &str) -> Element<Settings> {
    vstack![
        button(format!("Clicked {}", state.clicks))
            .key(format!("{prefix}-add"))
            .on_click(|s: &mut Settings| s.clicks += 1),
        toggle("Alerts", state.alerts)
            .key(format!("{prefix}-alerts"))
            .on_change(|s: &mut Settings, on| s.alerts = on),
        text_field(&state.name)
            .key(format!("{prefix}-name"))
            .on_change(|s: &mut Settings, v| s.name = v),
        // A component that already takes a state accessor still works when the
        // whole view is mapped: the two accessors compose.
        combobox([(1u8, "Apple"), (2, "Banana")], &state.combo)
            .key(format!("{prefix}-fruit"))
            .selected(state.fruit)
            .state(|s: &mut Settings| &mut s.combo)
            .on_select(|s: &mut Settings, v| s.fruit = Some(v)),
    ]
    .into()
}

#[derive(Default)]
struct App {
    left: Settings,
    right: Settings,
}
fn view(app: &App) -> Element<App> {
    hstack![
        settings(&app.left, "left").map_state(|app: &mut App| &mut app.left),
        settings(&app.right, "right").map_state(|app: &mut App| &mut app.right),
    ]
    .gap(24., Axis::Horizontal)
    .into()
}

struct Harness {
    app: App,
    runtime: Runtime,
    handlers: Handlers<'static, App>,
}
impl Harness {
    fn new() -> Self {
        let app = App::default();
        let (d, handlers) = view(&app).build().unwrap().into_parts();
        let runtime = Runtime::new(d, &handlers, &Bindings::default()).unwrap();
        let mut h = Self {
            app,
            runtime,
            handlers,
        };
        h.layout();
        h
    }
    fn layout(&mut self) {
        self.runtime
            .layout(
                Viewport::new(900., 900., 1.).unwrap(),
                &Bindings::default(),
                &mut Metrics,
            )
            .unwrap();
    }
    fn rebuild(&mut self) {
        let (d, handlers) = view(&self.app).build().unwrap().into_parts();
        self.runtime
            .replace_document(d, &handlers, &Bindings::default())
            .unwrap();
        self.handlers = handlers;
        self.layout();
    }
    fn press(&mut self, id: &str) -> bool {
        let b = self
            .runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("no {id}"))
            .clone();
        let [x, y, w, h] = b.bounds.bounds();
        let (x, y) = (x + w * 0.5, y + h * 0.5);
        self.runtime.pointer_down(x, y);
        let outcome = self
            .runtime
            .pointer_up_outcome(x, y, &mut self.handlers, &mut self.app)
            .unwrap();
        self.rebuild();
        outcome.activated
    }
    fn click(&mut self, id: &str) {
        assert!(self.press(id), "{id} did not activate");
    }
    fn type_into(&mut self, id: &str, text: &str) {
        self.press(id);
        assert_eq!(self.runtime.focused(), Some(id), "{id} not focused");
        self.runtime
            .text_input(text, &mut self.handlers, &mut self.app)
            .unwrap();
        self.rebuild();
    }
}

#[test]
fn callbacks_update_only_the_mapped_part_of_the_parent_state() {
    let mut h = Harness::new();
    h.click("left-add");
    h.click("left-add");
    h.click("right-alerts");
    h.type_into("left-name", "Ada");
    assert_eq!(h.app.left.clicks, 2);
    assert_eq!(h.app.right.clicks, 0);
    assert!(h.app.right.alerts);
    assert!(!h.app.left.alerts);
    assert_eq!(h.app.left.name, "Ada");
    assert!(h.app.right.name.is_empty());
}

#[test]
fn component_state_accessors_compose_with_a_mapped_view() {
    let mut h = Harness::new();
    h.click("right-fruit-toggle");
    assert!(h.app.right.combo.open, "the right combobox opened");
    assert!(!h.app.left.combo.open, "the left combobox stayed closed");
    // Options are keyed by position: option-1 is Banana, value 2.
    h.click("right-fruit-option-1");
    assert_eq!(h.app.right.fruit, Some(2));
    assert_eq!(h.app.left.fruit, None);
}

/// A surface that does its own hit testing, such as a map.
#[derive(Default)]
struct Surface {
    presses: u32,
}
fn surface(key: &str) -> Element<Surface> {
    zstack(Vec::<Element<Surface>>::new())
        .key(key)
        .width(200.)
        .height(100.)
        .on_pointer(|s: &mut Surface, p| {
            if matches!(p.phase, PointerPhase::Press) {
                s.presses += 1;
            }
        })
        .into()
}

#[derive(Default)]
struct Board {
    left: Surface,
    right: Surface,
}

#[test]
fn pointer_surfaces_hear_presses_inside_a_mapped_view() {
    let mut board = Board::default();
    let view: Element<Board> = hstack![
        surface("left").map_state(|b: &mut Board| &mut b.left),
        surface("right").map_state(|b: &mut Board| &mut b.right),
    ]
    .into();
    let (d, mut handlers) = view.build().unwrap().into_parts();
    let mut runtime = Runtime::new(d, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(500., 200., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    let [x, y, ..] = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "right")
        .unwrap()
        .bounds
        .bounds();
    runtime
        .pointer_down_outcome(x + 10., y + 10., &mut handlers, &mut board)
        .unwrap();
    assert_eq!(board.right.presses, 1, "the right surface heard its press");
    assert_eq!(board.left.presses, 0);
}
