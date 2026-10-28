#![cfg(feature = "native")]
//! `selectable_list(..).plain(true)`: flat rows that still select and activate.
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Kind, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Viewport,
};

struct App {
    selection: SelectionModel<u8>,
    opened: Option<u8>,
}
fn list(app: &App, plain: bool) -> Element<App> {
    selectable_list(
        [
            selectable_row(1, "One", text("One")).key("one"),
            selectable_row(2, "Two", text("Two")).key("two"),
        ],
        &app.selection,
    )
    .key("rows")
    .accessible_label("Rows")
    .plain(plain)
    .on_selection_change(|app: &mut App, action| {
        app.selection.apply(action);
    })
    .on_activate(|app: &mut App, id| app.opened = Some(id))
    .into()
}
fn mount(ui: Element<App>) -> (Runtime, Handlers<'static, App>) {
    let (document, handlers) = ui.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(400., 300., 1.).unwrap();
    let mut painter = RasterPainter::new(viewport);
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    (runtime, handlers)
}

#[test]
fn plain_rows_have_no_card_surface_but_still_select_and_activate() {
    let mut app = App {
        selection: SelectionModel::new(),
        opened: None,
    };
    app.selection.reconcile(&[1, 2]).unwrap();
    let (cards, _) = mount(list(&app, false));
    let kind = |r: &Runtime, id: &str| r.boxes().iter().find(|b| b.id == id).unwrap().kind;
    assert_eq!(
        kind(&cards, "one"),
        Kind::Panel,
        "default rows keep their card surface"
    );
    let (mut runtime, mut handlers) = mount(list(&app, true));
    assert_ne!(kind(&runtime, "one"), Kind::Panel, "plain rows are flat");
    let b = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "two")
        .unwrap()
        .bounds
        .bounds();
    let (x, y) = (b[0] + b[2] / 2., b[1] + b[3] / 2.);
    runtime.pointer_down(x, y);
    runtime
        .pointer_up_outcome(x, y, &mut handlers, &mut app)
        .unwrap();
    assert!(app.selection.contains(&2));
    let enter = InputKeyEvent {
        key: InputKey::Enter,
        modifiers: InputModifiers::default(),
        repeat: false,
    };
    runtime.input_key(enter, &mut handlers, &mut app).unwrap();
    assert_eq!(app.opened, Some(2));
}
