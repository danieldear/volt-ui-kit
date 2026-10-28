#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Event, EventKind, HandlerError, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
        [
            (text.chars().count() as f32 * size * 0.5).min(width),
            size * 1.2,
        ]
    }
}
#[derive(Default)]
struct App {
    activations: Vec<PointerActivation>,
    keys: Vec<InputKeyEvent>,
    legacy: usize,
    text: String,
}
fn mount(ui: Element<App>) -> (Runtime, Handlers<'static, App>) {
    let (doc, handlers) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    layout(&mut r);
    (r, handlers)
}
fn layout(r: &mut Runtime) {
    r.layout(
        Viewport::new(600., 400., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
}
fn point(r: &Runtime, id: &str) -> [f32; 2] {
    let [x, y, w, h] = r
        .boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds();
    [x + w * 0.5, y + h * 0.5]
}
fn modifiers() -> InputModifiers {
    InputModifiers {
        shift: true,
        control: true,
        alt: false,
        meta: true,
    }
}
fn key(key: InputKey) -> InputKeyEvent {
    InputKeyEvent {
        key,
        modifiers: modifiers(),
        repeat: false,
    }
}
fn control() -> Element<App> {
    button("Item")
        .key("item")
        .on_click_with(|a: &mut App, event| a.activations.push(event))
        .on_input_key(|a: &mut App, event| {
            a.keys.push(event);
            true
        })
        .into()
}

#[test]
fn pointer_press_modifiers_are_preserved_and_legacy_adapter_defaults() {
    let (mut r, mut h) = mount(control());
    let mut a = App::default();
    let [x, y] = point(&r, "item");
    r.pointer_down_outcome_with_modifiers(x, y, modifiers(), &mut h, &mut a)
        .unwrap();
    assert!(
        r.pointer_up_outcome(x, y, &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(
        a.activations,
        [PointerActivation {
            modifiers: modifiers(),
            source: InputSource::Pointer
        }]
    );
    r.pointer_down(x, y);
    r.pointer_up(x, y, &mut h, &mut a).unwrap();
    assert_eq!(a.activations[1].modifiers, InputModifiers::default());
}

#[test]
fn release_outside_and_cancelled_capture_never_dispatch() {
    let (mut r, mut h) = mount(control());
    let mut a = App::default();
    let [x, y] = point(&r, "item");
    r.pointer_down_with_modifiers(x, y, modifiers());
    assert!(
        !r.pointer_up_outcome(700., 500., &mut h, &mut a)
            .unwrap()
            .activated
    );
    r.pointer_down_with_modifiers(x, y, modifiers());
    r.cancel_input();
    assert!(
        !r.pointer_up_outcome(x, y, &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(a.activations.is_empty());
}

#[test]
fn keyboard_and_accessibility_use_distinct_honest_sources() {
    let (mut r, mut h) = mount(control());
    let mut a = App::default();
    assert!(r.focus_next(false));
    assert!(r.key_down());
    assert!(r.key_up_outcome(&mut h, &mut a).unwrap().activated);
    assert!(
        r.accessibility_activate("item", &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(
        a.activations.iter().map(|e| e.source).collect::<Vec<_>>(),
        [InputSource::Keyboard, InputSource::Accessibility]
    );
    assert!(a
        .activations
        .iter()
        .all(|e| e.modifiers == InputModifiers::default()));
}

#[test]
fn rich_container_is_focusable_clickable_and_keeps_children_interactive() {
    let row = panel![
        text("A rich selectable row"),
        button("Open")
            .key("open")
            .on_click(|a: &mut App| a.legacy += 1)
    ]
    .key("row")
    .accessible_label("Archive folder")
    .width(360.)
    .height(100.)
    .on_click_with(|a: &mut App, event| a.activations.push(event));
    let (mut r, mut h) = mount(row.into());
    let mut a = App::default();
    let node = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "row")
        .unwrap();
    assert!(node.clickable);
    assert_eq!(node.label, "Archive folder");
    assert!(
        r.accessibility_activate("row", &mut h, &mut a)
            .unwrap()
            .activated
    );
    let [x, y] = point(&r, "open");
    r.pointer_down_with_modifiers(x, y, modifiers());
    r.pointer_up(x, y, &mut h, &mut a).unwrap();
    assert_eq!(a.legacy, 1);
    assert_eq!(a.activations.len(), 1);
}

#[test]
fn keys_reach_only_opted_in_focused_enabled_control() {
    let ui: Element<App> = vstack![
        control(),
        button("Other")
            .key("other")
            .on_click(|a: &mut App| a.legacy += 1)
    ]
    .into();
    let (mut r, mut h) = mount(ui);
    let mut a = App::default();
    assert!(
        !r.input_key(key(InputKey::ArrowDown), &mut h, &mut a)
            .unwrap()
            .activated
    );
    r.accessibility_focus("item");
    assert!(
        r.input_key(key(InputKey::ArrowDown), &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(a.keys, [key(InputKey::ArrowDown)]);
    r.focus_next(false);
    assert_eq!(r.focused(), Some("other"));
    assert!(
        !r.input_key(key(InputKey::ArrowDown), &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(a.keys.len(), 1);
}

#[test]
fn unhandled_key_does_not_consume_or_change_focus() {
    let ui = button("Pass through")
        .key("pass")
        .on_click(|_: &mut App| {})
        .on_input_key(|_: &mut App, _| false);
    let (mut r, mut h) = mount(ui.into());
    let mut a = App::default();
    r.accessibility_focus("pass");
    let result = r.input_key(key(InputKey::Enter), &mut h, &mut a).unwrap();
    assert!(!result.activated);
    assert!(!result.changed);
    assert_eq!(r.focused(), Some("pass"));
    r.key_down();
    assert!(r.key_up_outcome(&mut h, &mut a).unwrap().activated);
}

#[test]
fn disabled_control_cannot_receive_pointer_keyboard_or_ax() {
    let ui = button("Disabled")
        .key("disabled")
        .enabled(false)
        .on_click_with(|a: &mut App, e| a.activations.push(e))
        .on_input_key(|a: &mut App, e| {
            a.keys.push(e);
            true
        });
    let (mut r, mut h) = mount(ui.into());
    let mut a = App::default();
    let [x, y] = point(&r, "disabled");
    r.pointer_down_with_modifiers(x, y, modifiers());
    r.pointer_up(x, y, &mut h, &mut a).unwrap();
    assert!(!r.accessibility_focus("disabled"));
    assert!(
        !r.accessibility_activate("disabled", &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(
        !r.input_key(key(InputKey::Enter), &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(a.activations.is_empty() && a.keys.is_empty());
}

#[test]
fn opt_in_and_legacy_click_setters_replace_each_other() {
    for new_wins in [false, true] {
        let b = button("Item").key("item");
        let b = if new_wins {
            b.on_click(|a: &mut App| a.legacy += 1)
                .on_click_with(|a: &mut App, e| a.activations.push(e))
        } else {
            b.on_click_with(|a: &mut App, e| a.activations.push(e))
                .on_click(|a: &mut App| a.legacy += 1)
        };
        let (mut r, mut h) = mount(b.into());
        let mut a = App::default();
        r.accessibility_activate("item", &mut h, &mut a).unwrap();
        assert_eq!(a.legacy, usize::from(!new_wins));
        assert_eq!(a.activations.len(), usize::from(new_wins));
    }
}

#[test]
fn editor_keys_and_tab_traversal_remain_separate() {
    let ui: Element<App> = vstack![
        control(),
        text_field("")
            .key("editor")
            .on_change(|a: &mut App, text| a.text = text)
    ]
    .into();
    let (mut r, mut h) = mount(ui);
    let mut a = App::default();
    r.accessibility_focus("editor");
    assert!(
        !r.input_key(key(InputKey::ArrowDown), &mut h, &mut a)
            .unwrap()
            .activated
    );
    r.text_input("abc", &mut h, &mut a).unwrap();
    assert_eq!(a.text, "abc");
    r.focus_next(false);
    assert_eq!(r.focused(), Some("item"));
    r.focus_next(true);
    assert_eq!(r.focused(), Some("editor"));
}

#[test]
fn modal_blocks_background_opt_in_key_and_activation() {
    let ui: Element<App> = zstack![
        control(),
        dialog(button("Accept").key("accept").on_click(|_: &mut App| {}))
            .key("modal")
            .open(true)
            .on_dismiss(|_: &mut App| {})
    ]
    .into();
    let (mut r, mut h) = mount(ui);
    let mut a = App::default();
    assert!(!r.accessibility_focus("item"));
    assert!(
        !r.accessibility_activate("item", &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(
        !r.input_key(key(InputKey::Enter), &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(a.activations.is_empty() && a.keys.is_empty());
}

#[test]
fn removed_pressed_target_cannot_fire_after_recomposition() {
    let (mut r, mut h) = mount(control());
    let mut a = App::default();
    let [x, y] = point(&r, "item");
    r.pointer_down_with_modifiers(x, y, modifiers());
    let (doc, next_h) = Element::from(text::<App>("No target"))
        .build()
        .unwrap()
        .into_parts();
    r.replace_document(doc, &next_h, &Bindings::default())
        .unwrap();
    layout(&mut r);
    // Even an old registry cannot resurrect an unmounted target.
    r.pointer_up(x, y, &mut h, &mut a).unwrap();
    assert!(a.activations.is_empty());
}

fn focused_view(which: usize) -> Element<App> {
    vstack![
        button("A")
            .key("a")
            .focus_when(which == 0)
            .on_click(|_: &mut App| {}),
        button("B")
            .key("b")
            .focus_when(which == 1)
            .on_click(|_: &mut App| {})
    ]
    .into()
}
#[test]
fn declarative_focus_is_rising_edge_and_does_not_steal_tab_focus() {
    let (mut r, _) = mount(focused_view(0));
    assert_eq!(r.focused(), Some("a"));
    r.focus_next(false);
    assert_eq!(r.focused(), Some("b"));
    let (doc, h) = focused_view(0).build().unwrap().into_parts();
    r.replace_document(doc, &h, &Bindings::default()).unwrap();
    layout(&mut r);
    assert_eq!(r.focused(), Some("b"));
    let (doc, h) = focused_view(1).build().unwrap().into_parts();
    r.replace_document(doc, &h, &Bindings::default()).unwrap();
    layout(&mut r);
    assert_eq!(r.focused(), Some("b"));
    let (doc, h) = focused_view(0).build().unwrap().into_parts();
    r.replace_document(doc, &h, &Bindings::default()).unwrap();
    layout(&mut r);
    assert_eq!(r.focused(), Some("a"));
}

#[test]
fn input_and_legacy_handlers_share_duplicate_name_protection() {
    let mut h: Handlers<'_, App> = Handlers::default();
    h.on_activate_with("same", |_, _| {}).unwrap();
    assert!(matches!(
        h.on_activate("same", |_| {}),
        Err(HandlerError::Duplicate(_))
    ));
    assert!(matches!(
        h.on_input_key("same", |_, _| false),
        Err(HandlerError::Duplicate(_))
    ));
    h.on_activate("legacy", |_| {}).unwrap();
    assert!(matches!(
        h.on_input_key("legacy", |_, _| false),
        Err(HandlerError::Duplicate(_))
    ));
}

#[test]
fn old_public_event_kinds_still_allow_an_exhaustive_match() {
    let event = Event::Activate;
    let _ = match event {
        Event::Activate
        | Event::TextChanged(_)
        | Event::ToggleChanged(_)
        | Event::ValueChanged { .. }
        | Event::ContextRequested { .. }
        | Event::WindowChanged { .. }
        | Event::FilesDropped(_)
        | Event::Link(_)
        | Event::SelectionChanged { .. }
        | Event::Key(_) => true,
    };
    let _ = match EventKind::Activate {
        EventKind::Activate
        | EventKind::TextChanged
        | EventKind::ToggleChanged
        | EventKind::ValueChanged
        | EventKind::ContextRequested
        | EventKind::WindowChanged
        | EventKind::FilesDropped
        | EventKind::Link
        | EventKind::SelectionChanged
        | EventKind::Key => true,
    };
}
#[test]
fn a_container_hears_the_keys_its_focused_rows_do_not_use() {
    let ui: Element<App> = vstack([
        button("Row one").key("one").on_click(|a: &mut App| a.legacy += 1).into(),
        button("Row two").key("two").on_click(|a: &mut App| a.legacy += 1).into(),
    ])
    .key("list")
    .on_input_key(|a: &mut App, event| {
        let used = matches!(event.key, InputKey::ArrowDown | InputKey::ArrowUp);
        if used {
            a.keys.push(event);
        }
        used
    })
    .into();
    let (mut r, mut h) = mount(ui);
    let mut app = App::default();
    let [x, y] = point(&r, "one");
    r.pointer_down(x, y);
    assert_eq!(r.focused(), Some("one"));
    let key = |key| InputKeyEvent { key, modifiers: InputModifiers::default(), repeat: false };
    assert!(r.input_key(key(InputKey::ArrowDown), &mut h, &mut app).unwrap().activated, "the list took it");
    assert_eq!(app.keys.len(), 1);
    assert!(!r.input_key(key(InputKey::Escape), &mut h, &mut app).unwrap().activated, "declined: the host keeps it");
    assert_eq!(app.legacy, 0, "no row was activated by the arrow");
}

#[test]
fn code_focus_requests_are_repeatable_without_stealing_focus_on_unchanged_tokens() {
    let ui = |epoch, focus| -> Element<App> {
        vstack([
            code_view(1).key("code").height(120.).focus_when(focus).focus_request(epoch)
                .on_input(|_: &mut App, _| None).into(),
            button("Other").key("other").on_click(|_: &mut App| {}).into(),
        ]).into()
    };
    let (mut r,_)=mount(ui(1,false));assert!(r.code_view_focused());
    for epoch in [1,2,3] {
        let [x,y]=point(&r,"other");r.pointer_down(x,y);assert_eq!(r.focused(),Some("other"));
        let (doc,handlers)=ui(epoch,false).build().unwrap().into_parts();r.replace_document(doc,&handlers,&Bindings::default()).unwrap();layout(&mut r);
        if epoch==1 {assert_eq!(r.focused(),Some("other"),"unchanged token is inert");}else{assert!(r.code_view_focused(),"new token restores the same code view");}
    }
    let [x,y]=point(&r,"other");r.pointer_down(x,y);
    let (doc,handlers)=ui(3,true).build().unwrap().into_parts();r.replace_document(doc,&handlers,&Bindings::default()).unwrap();layout(&mut r);
    assert!(r.code_view_focused(),"focus_when still has independent rising edges");
}
