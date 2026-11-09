#![cfg(feature = "rust-ui")]
//! The design-tool colour picker (`color_picker(..).area(true)`): dragging
//! the square, hue and opacity strips, typing hex, and Hsva conversions.
use volt_ui_kit::{
    declarative::{Bindings, Handlers, InputModifiers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};

#[derive(Default)]
struct App {
    colour: Option<Hsva>,
    hex: String,
}

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, s: f32, _: f32) -> [f32; 2] {
        [t.len() as f32 * s / 2., s]
    }
}

fn start() -> Hsva {
    Hsva {
        hue: 220.,
        saturation: 0.5,
        value: 0.5,
        alpha: 1.,
    }
}

fn view(a: &App) -> Element<App> {
    vstack![color_picker(a.hex.clone())
        .key("picker")
        .area(true)
        .alpha(true)
        .width(240.)
        .hsva(a.colour.unwrap_or_else(start))
        .on_hsva(|a: &mut App, c| a.colour = Some(c))
        .on_change(|a: &mut App, hex| a.hex = hex)]
    .key("root")
    .width(300.)
    .height(500.)
    .into()
}

fn mount(a: &App) -> (Runtime, Handlers<'static, App>) {
    let (d, h) = view(a).build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(300., 500., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    (r, h)
}

fn bounds(r: &Runtime, id: &str) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no box {id}"))
        .bounds
        .bounds()
}

fn press(r: &mut Runtime, h: &mut Handlers<'static, App>, a: &mut App, x: f32, y: f32) {
    r.pointer_down_outcome_with_clicks(x, y, InputModifiers::default(), 1, h, a)
        .unwrap();
    r.pointer_up_outcome(x, y, h, a).unwrap();
}

#[test]
fn the_square_sets_saturation_and_brightness() {
    let mut app = App::default();
    let (mut r, mut h) = mount(&app);
    let [x, y, w, hh] = bounds(&r, "picker-area");
    press(&mut r, &mut h, &mut app, x + w * 0.75, y + hh * 0.25);
    let c = app.colour.unwrap();
    assert!((c.saturation - 0.75).abs() < 0.01);
    assert!((c.value - 0.75).abs() < 0.01);
    assert_eq!(c.hue, 220.);
    assert_eq!(app.hex.len(), 9, "alpha pickers report #RRGGBBAA");
}

#[test]
fn hue_and_opacity_strips() {
    let mut app = App::default();
    let (mut r, mut h) = mount(&app);
    let [x, y, w, hh] = bounds(&r, "picker-hue");
    press(&mut r, &mut h, &mut app, x + w * 0.5, y + hh / 2.);
    assert!((app.colour.unwrap().hue - 180.).abs() < 1.);
    let (mut r, mut h) = mount(&app);
    let [x, y, w, hh] = bounds(&r, "picker-alpha");
    press(&mut r, &mut h, &mut app, x + w * 0.25, y + hh / 2.);
    let c = app.colour.unwrap();
    assert!((c.alpha - 0.25).abs() < 0.01);
    assert!(app.hex.ends_with("40"));
}

#[test]
fn hsva_round_trips_and_keeps_hue_for_greys() {
    let blue = Hsva::parse("#2B59C3").unwrap();
    assert_eq!(blue.hex(), "#2B59C3");
    assert!((blue.hue - 222.).abs() < 1.);
    let half = Hsva::parse("#2B59C380").unwrap();
    assert!((half.alpha - 0.5).abs() < 0.01);
    assert_eq!(half.hex_alpha(), "#2B59C380");
    let grey = blue.with_hex("#808080").unwrap();
    assert_eq!(grey.hue, blue.hue, "a grey keeps the hue it had");
    assert_eq!(grey.saturation, 0.);
    assert_eq!(grey.alpha, 1.);
}

#[test]
fn the_plain_picker_takes_its_width_too() {
    let picker = |width: f32| -> Element<App> {
        color_picker("#336699")
            .key("plain")
            .width(width)
            .on_change(|a: &mut App, hex| a.hex = hex)
            .into()
    };
    let (d, h) = picker(400.).build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(600., 600., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    assert_eq!(bounds(&r, "plain")[2], 400.);
    assert!(
        picker(100.).build().is_err(),
        "widths outside 160..=640 are refused"
    );
}
