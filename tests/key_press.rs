#![cfg(feature = "rust-ui")]
//! Application keys (`on_key_press`) reach the right container, and only
//! when no editor, menu or dialog has a claim on them.
use volt_ui_kit::{
    declarative::{Bindings, Handlers, InputModifiers, KeyPress, PressedKey, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};

#[derive(Default)]
struct App {
    /// (container, key) for every press a handler saw.
    heard: Vec<(&'static str, PressedKey)>,
    /// Which keys the canvas claims; others fall through.
    canvas_takes: Vec<PressedKey>,
    field: String,
    modal: bool,
    clicks: usize,
}

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, s: f32, _: f32) -> [f32; 2] {
        [t.len() as f32 * s / 2., s]
    }
}

fn view(a: &App) -> Element<App> {
    let panel = vstack![
        button("Panel button")
            .key("panel-button")
            .on_click(|a: &mut App| a.clicks += 1),
        text_field(&a.field)
            .key("field")
            .accessible_label("Field")
            .on_change(|a: &mut App, v| a.field = v),
    ]
    .key("panel")
    .on_key_press(|a: &mut App, k| {
        a.heard.push(("panel", k.key));
        true
    });
    let canvas = vstack![text("Canvas")]
        .key("canvas")
        .height(80.)
        .on_drag(|_: &mut App, _| {})
        .on_key_press(|a: &mut App, k| {
            a.heard.push(("canvas", k.key));
            a.canvas_takes.contains(&k.key)
        });
    let mut children: Vec<Element<App>> = vec![panel.into(), canvas.into()];
    if a.modal {
        children.push(
            dialog(text("Modal"))
                .key("modal")
                .open(true)
                .on_dismiss(|a: &mut App| a.modal = false)
                .into(),
        );
    }
    zstack![vstack(children).key("columns")]
        .key("root")
        .width(400.)
        .height(300.)
        .into()
}

fn mount(a: &App) -> (Runtime, Handlers<'static, App>) {
    let (d, h) = view(a).build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(400., 300., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    (r, h)
}

fn press(key: PressedKey) -> KeyPress {
    KeyPress {
        key,
        modifiers: InputModifiers::default(),
        repeat: false,
    }
}

#[test]
fn with_nothing_focused_the_last_container_in_the_document_hears_it() {
    let mut a = App {
        canvas_takes: vec![PressedKey::Delete],
        ..App::default()
    };
    let (mut r, mut h) = mount(&a);
    assert!(r.focused().is_none());
    assert!(r
        .key_press(press(PressedKey::Delete), &mut h, &mut a)
        .unwrap());
    assert_eq!(a.heard, [("canvas", PressedKey::Delete)]);
}

#[test]
fn an_unused_key_falls_through_to_the_window() {
    let mut a = App::default();
    let (mut r, mut h) = mount(&a);
    let used = r
        .key_press(press(PressedKey::Character('q')), &mut h, &mut a)
        .unwrap();
    assert!(!used, "false from the handler leaves the key to the host");
    assert_eq!(a.heard, [("canvas", PressedKey::Character('q'))]);
}

#[test]
fn the_container_around_the_focused_control_hears_it_first() {
    let mut a = App::default();
    let (mut r, mut h) = mount(&a);
    assert!(r.focus_next(false));
    assert_eq!(r.focused(), Some("panel-button"));
    assert!(r
        .key_press(press(PressedKey::ArrowLeft), &mut h, &mut a)
        .unwrap());
    assert_eq!(a.heard, [("panel", PressedKey::ArrowLeft)]);
}

#[test]
fn enter_and_space_stay_with_a_focused_control() {
    let mut a = App {
        canvas_takes: vec![PressedKey::Enter],
        ..App::default()
    };
    let (mut r, mut h) = mount(&a);
    assert!(r
        .key_press(press(PressedKey::Enter), &mut h, &mut a)
        .unwrap());
    assert!(r.focus_next(false));
    for key in [PressedKey::Enter, PressedKey::Space] {
        assert!(!r.key_press(press(key), &mut h, &mut a).unwrap());
    }
    assert_eq!(a.heard, [("canvas", PressedKey::Enter)]);
}

#[test]
fn nothing_arrives_while_a_text_field_is_being_edited() {
    let mut a = App::default();
    let (mut r, mut h) = mount(&a);
    r.focus_next(false);
    r.focus_next(false);
    assert_eq!(r.focused(), Some("field"));
    assert!(r.editing_focus());
    for key in [
        PressedKey::Backspace,
        PressedKey::Character('z'),
        PressedKey::ArrowRight,
    ] {
        assert!(!r.key_press(press(key), &mut h, &mut a).unwrap());
    }
    assert!(a.heard.is_empty());
}

#[test]
fn nothing_arrives_while_a_dialog_is_open() {
    let mut a = App {
        modal: true,
        canvas_takes: vec![PressedKey::Delete],
        ..App::default()
    };
    let (mut r, mut h) = mount(&a);
    assert!(!r
        .key_press(press(PressedKey::Delete), &mut h, &mut a)
        .unwrap());
    assert!(a.heard.is_empty());
}

#[test]
fn modifiers_and_repeat_are_passed_through() {
    #[derive(Default)]
    struct Undo {
        seen: Vec<KeyPress>,
    }
    let doc: Element<Undo> = vstack![text("Doc")]
        .key("doc")
        .on_key_press(|a: &mut Undo, k| {
            a.seen.push(k);
            k.primary() && k.char() == Some('z')
        })
        .into();
    let (d, mut h) = doc.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(200., 100., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let mut a = Undo::default();
    let redo = KeyPress {
        key: PressedKey::Character('z'),
        modifiers: InputModifiers {
            shift: true,
            control: true,
            alt: false,
            meta: true,
        },
        repeat: true,
    };
    assert!(r.key_press(redo, &mut h, &mut a).unwrap());
    assert_eq!(a.seen, [redo]);
    assert!(!a.seen[0].bare());
}

#[test]
fn pressing_a_drag_area_ends_text_editing_so_keys_reach_the_app() {
    let mut a = App {
        canvas_takes: vec![PressedKey::Delete],
        ..App::default()
    };
    let (mut r, mut h) = mount(&a);
    r.focus_next(false);
    r.focus_next(false);
    assert!(r.editing_focus());
    let [x, y, w, hgt] = r
        .boxes()
        .iter()
        .find(|b| b.id == "canvas")
        .unwrap()
        .bounds
        .bounds();
    let (px, py) = (x + w / 2., y + hgt / 2.);
    assert!(
        r.pointer_down_outcome(px, py, &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(!r.editing_focus());
    assert_eq!(r.focused(), None);
    assert!(r
        .key_press(press(PressedKey::Delete), &mut h, &mut a)
        .unwrap());
    assert_eq!(a.heard, [("canvas", PressedKey::Delete)]);
}
