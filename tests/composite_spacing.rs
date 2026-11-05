#![cfg(feature = "rust-ui")]
//! Root/content spacing must be opt-in and must not change control behavior.
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::{prelude::*, RadioGroup},
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, max: f32) -> [f32; 2] {
        [
            (text.chars().count() as f32 * size * 0.5).min(max),
            size * 1.2,
        ]
    }
}
#[derive(Default)]
struct App {
    picked: u8,
    number: f64,
    open: bool,
    combo: ComboboxState,
    code: String,
    dismissed: Vec<u64>,
}
fn mount(ui: Element<App>, width: f32, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (doc, handlers) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(width * scale, 900. * scale, scale).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    (r, handlers)
}
fn rect(r: &Runtime, key: &str) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .bounds
        .bounds()
}
fn click(r: &mut Runtime, h: &mut Handlers<'_, App>, a: &mut App, key: &str) {
    let [x, y, w, height] = rect(r, key);
    r.pointer_down(x + w / 2., y + height / 2.);
    assert!(
        r.pointer_up_outcome(x + w / 2., y + height / 2., h, a)
            .unwrap()
            .activated
    );
}
fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 0.05, "{a} != {b}");
}
fn radio() -> RadioGroup<App, u8> {
    radio_group([(1, "One"), (2, "Two")])
        .key("radio")
        .on_select(|a: &mut App, v| a.picked = v)
}
#[test]
fn radio_default_gap_is_preserved_and_explicit_zero_custom_and_directional_padding_work() {
    for scale in [1., 2.] {
        for (gap, expected) in [(None, 4.), (Some(0.), 0.), (Some(17.), 17.)] {
            let mut builder = radio();
            if let Some(gap) = gap {
                builder = builder.gap(gap, Axis::Vertical);
            }
            let (mut r, mut h) = mount(
                builder
                    .padding(12., [Edge::Top, Edge::Left])
                    .padding(5., Edge::Left)
                    .into(),
                600.,
                scale,
            );
            let a = rect(&r, "radio-0-row");
            let b = rect(&r, "radio-1-row");
            assert_close(b[1] - a[1] - a[3], expected * scale);
            assert_close(a[0] - rect(&r, "radio")[0], 5. * scale);
            assert_close(a[1] - rect(&r, "radio")[1], 12. * scale);
            let mut app = App::default();
            click(&mut r, &mut h, &mut app, "radio-1");
            assert_eq!(app.picked, 2);
        }
    }
}
#[test]
fn form_field_responsive_gap_and_zero_padding_replace_constructor_defaults() {
    for scale in [1., 2.] {
        for (width, gap) in [(600., 3.), (900., 21.)] {
            let field = form_field(
                "Label",
                text_field("").key("input").on_change(|_: &mut App, _| {}),
            )
            .key("field")
            .padding(0., Edge::All)
            .gap(responsive(3.).at(Breakpoint::Md, 21.), Axis::Vertical);
            let (r, _) = mount(field.into(), width, scale);
            let label = rect(&r, "field-label");
            let input = rect(&r, "input");
            assert_close(input[1] - label[1] - label[3], gap * scale);
            assert_close(input[0], rect(&r, "field")[0]);
        }
    }
}
#[test]
fn number_input_spacing_changes_geometry_not_increment_behavior() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(
            number_input(2.)
                .key("qty")
                .width(220.)
                .range(0., 10.)
                .gap(13., Axis::Horizontal)
                .padding(7., Edge::Horizontal)
                .on_change(|a: &mut App, v| a.number = v)
                .into(),
            600.,
            scale,
        );
        let dec = rect(&r, "qty-dec-layer");
        let field = rect(&r, "qty-field");
        assert_close(field[0] - dec[0] - dec[2], 13. * scale);
        let mut app = App::default();
        click(&mut r, &mut h, &mut app, "qty-inc");
        assert_eq!(app.number, 3.);
    }
}
#[test]
fn collapsible_and_accordion_spacing_preserve_toggling_and_defaults() {
    for scale in [1., 2.] {
        for gap in [None, Some(0.), Some(14.)] {
            let mut c = collapsible("Details", text("Body").key("body"))
                .key("details")
                .open(true)
                .on_toggle(|a: &mut App, v| a.open = v);
            if let Some(gap) = gap {
                c = c.gap(gap, Axis::Vertical);
            }
            let (mut r, mut h) = mount(c.padding(6., Edge::All).into(), 600., scale);
            let head = rect(&r, "details-trigger");
            let body = rect(&r, "details-content");
            assert_close(body[1] - head[1] - head[3], gap.unwrap_or(0.) * scale);
            let mut app = App {
                open: true,
                ..Default::default()
            };
            click(&mut r, &mut h, &mut app, "details-trigger");
            assert!(!app.open);
        }
        let (r, _) = mount(
            accordion([("A", text("a").into()), ("B", text("b").into())])
                .key("acc")
                .gap(11., Axis::Vertical)
                .padding(0., Edge::All)
                .on_change(|_: &mut App, _| {})
                .into(),
            600.,
            scale,
        );
        let a = rect(&r, "acc-0");
        let b = rect(&r, "acc-1");
        assert_close(b[1] - a[1] - a[3], 11. * scale);
    }
}
#[test]
fn drawer_and_command_palette_spacing_style_the_card_not_the_modal_backdrop() {
    let (r, _) = mount(
        drawer(text("Body").key("drawer-body-copy"))
            .key("drawer")
            .open(true)
            .title("Title")
            .gap(3., Axis::Vertical)
            .padding(0., Edge::All)
            .on_dismiss(|_: &mut App| {})
            .into(),
        600.,
        1.,
    );
    let card = rect(&r, "drawer-content");
    let header = rect(&r, "drawer-head");
    assert_close(card[0], header[0]);
    assert_close(card[1], header[1]);
    let state = CommandPaletteState {
        open: true,
        ..Default::default()
    };
    #[derive(Default)]
    struct PaletteApp {
        state: CommandPaletteState,
    }
    let ui: Element<PaletteApp> = command_palette([command(1, "Run")], &state)
        .key("palette")
        .padding(0., Edge::All)
        .gap(0., Axis::Vertical)
        .state(|a: &mut PaletteApp| &mut a.state)
        .on_run(|_: &mut PaletteApp, _| {})
        .into();
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(800., 900., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let card = rect(&r, "palette-content");
    let field = rect(&r, "palette-field");
    assert_close(card[0], field[0]);
    assert_close(card[1], field[1]);
    assert!(
        card[1] >= 90.,
        "modal viewport inset must not be overwritten"
    );
}
#[test]
fn toaster_gap_is_between_cards_and_padding_is_viewport_inset() {
    for scale in [1., 2.] {
        let toasts = [Toast::new(1, "One"), Toast::new(2, "Two")];
        for (gap, pad) in [(8., 16.), (0., 0.), (19., 7.)] {
            let (mut r, mut h) = mount(
                toaster(&toasts)
                    .key("toasts")
                    .position(ToastPosition::TopCenter)
                    .gap(gap, Axis::Vertical)
                    .padding(pad, Edge::All)
                    .on_dismiss(|a: &mut App, id| a.dismissed.push(id))
                    .into(),
                600.,
                scale,
            );
            let a = rect(&r, "toasts-1");
            let b = rect(&r, "toasts-2");
            assert_close(b[1] - a[1] - a[3], gap * scale);
            assert_close(a[1], pad * scale);
            let mut app = App::default();
            click(&mut r, &mut h, &mut app, "toasts-2-close");
            assert_eq!(app.dismissed, [2]);
        }
    }
}
#[test]
fn otp_gap_recomputes_overlay_width_without_changing_digit_box_size() {
    for scale in [1., 2.] {
        for (gap, expected) in [(None, 8.), (Some(0.), 0.), (Some(15.), 15.)] {
            let mut otp = otp_input("12", 4)
                .key("otp")
                .on_change(|a: &mut App, v| a.code = v);
            if let Some(gap) = gap {
                otp = otp.digit_gap(gap);
            }
            let (r, _) = mount(otp.into(), 600., scale);
            let a = rect(&r, "otp-box-0");
            let b = rect(&r, "otp-box-1");
            assert_close(a[2], 42. * scale);
            assert_close(b[0] - a[0] - a[2], expected * scale);
            assert_close(rect(&r, "otp")[2], (4. * 42. + 3. * expected) * scale);
            assert_close(rect(&r, "otp-field")[2], rect(&r, "otp")[2]);
        }
    }
}
#[test]
fn invalid_spacing_is_rejected_instead_of_silently_clamped() {
    for value in [-1., f32::NAN, f32::INFINITY] {
        let ui: Element<App> = radio().gap(value, Axis::Vertical).into();
        assert!(ui.build().is_err());
        let ui: Element<App> = rating(2).key("rating").padding(value, Edge::Left).into();
        assert!(ui.build().is_err());
        let ui: Element<App> = otp_input("", 4)
            .key("otp")
            .digit_gap(value)
            .on_change(|_: &mut App, _| {})
            .into();
        assert!(ui.build().is_err());
        let state = ComboboxState::default();
        let ui: Element<App> = combobox([(1, "A")], &state)
            .key("combo")
            .option_gap(value)
            .state(|a: &mut App| &mut a.combo)
            .on_select(|_: &mut App, _| {})
            .into();
        assert!(ui.build().is_err());
    }
}

#[test]
fn all_root_spacing_builders_accept_custom_rules_and_color_channels_stay_inside_padding() {
    for scale in [1., 2.] {
        let (r, _) = mount(
            color_picker("#3B82F6")
                .key("color")
                .padding(15., Edge::Horizontal)
                .gap(0., Axis::Vertical)
                .on_change(|_: &mut App, _| {})
                .into(),
            600.,
            scale,
        );
        let outer = rect(&r, "color");
        for channel in ["color-H-row", "color-S-row", "color-L-row"] {
            let row = rect(&r, channel);
            assert_close(row[0], outer[0] + 15. * scale);
            assert!(row[0] + row[2] <= outer[0] + outer[2] - 15. * scale + 0.05);
        }
        #[derive(Default)]
        struct TagsApp {
            state: TagInputState,
        }
        let state = TagInputState::default();
        let ui: Element<TagsApp> = tag_input(&["one".into(), "two".into()], &state)
            .key("tags")
            .padding(0., Edge::All)
            .gap(0., Axis::Both)
            .state(|a: &mut TagsApp| &mut a.state)
            .on_change(|_: &mut TagsApp, _| {})
            .into();
        ui.build().unwrap();
        let ui: Element<App> = rating(2)
            .key("rating")
            .gap(0., Axis::Horizontal)
            .padding(4., Edge::All)
            .into();
        ui.build().unwrap();
        let ui: Element<App> = drop_zone()
            .key("drop")
            .gap(0., Axis::Vertical)
            .padding(0., Edge::All)
            .on_drop(|_: &mut App, _| {})
            .into();
        ui.build().unwrap();
    }
}

#[test]
fn combobox_option_gap_changes_only_popup_rows_and_selection_still_works() {
    for scale in [1., 2.] {
        for (gap, expected) in [(None, 2.), (Some(0.), 0.), (Some(12.), 12.)] {
            let state = ComboboxState {
                open: true,
                ..Default::default()
            };
            let mut combo = combobox([(1u8, "One"), (2, "Two")], &state)
                .key("combo")
                .state(|a: &mut App| &mut a.combo)
                .on_select(|a: &mut App, v| a.picked = v);
            if let Some(gap) = gap {
                combo = combo.option_gap(gap);
            }
            let (mut r, mut h) = mount(combo.into(), 600., scale);
            let a = rect(&r, "combo-option-0-row");
            let b = rect(&r, "combo-option-1-row");
            assert_close(b[1] - a[1] - a[3], expected * scale);
            let mut app = App {
                combo: state,
                ..Default::default()
            };
            click(&mut r, &mut h, &mut app, "combo-option-1");
            assert_eq!(app.picked, 2);
            assert!(!app.combo.open);
        }
    }
}
