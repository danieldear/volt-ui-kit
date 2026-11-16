#![cfg(feature = "rust-ui")]
//! Built-in component motion: on by default, off per component or app-wide.
use std::time::Duration;
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Painter, Theme, Viewport,
};

#[derive(Default)]
struct Recorder;
impl TextMetrics for Recorder {
    fn measure(&mut self, _: &str, _: f32, _: f32) -> [f32; 2] {
        [40., 20.]
    }
}
impl Painter for Recorder {
    /// Layer opacities only: the dialog's fade is what these tests observe.
    type Shape = f32;
    type Glyph = String;
    fn viewport(&self) -> Viewport {
        Viewport::new(600., 400., 1.).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<f32>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<f32>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(&mut self, _: &mut Vec<String>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<String>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for Recorder {
    fn ui_text(&mut self, out: &mut Vec<String>, text: &str, _: [f32; 4], _: f32, _: [f32; 4]) {
        out.push(text.into());
    }
    fn ui_layer_begin(&mut self, out: &mut Vec<f32>, opacity: f32) {
        out.push(opacity);
    }
}

#[derive(Clone, Copy, Default)]
struct App {
    open: bool,
    still: bool,
    unkeyed: bool,
}
fn view(app: &App) -> Element<App> {
    let d = dialog(
        button("OK")
            .key("ok")
            .on_click(|a: &mut App| a.open = false),
    )
    .open(app.open)
    .motion(!app.still)
    .on_dismiss(|a: &mut App| a.open = false);
    let d = if app.unkeyed { d } else { d.key("dlg") };
    vstack![text("Behind").key("behind"), d].key("root").into()
}
fn mount(app: &App) -> (Runtime, Handlers<'static, App>) {
    let (doc, h) = view(app).build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(Recorder.viewport(), &Bindings::default(), &mut Recorder)
        .unwrap();
    (r, h)
}
fn show(r: &mut Runtime, h: &mut Handlers<'static, App>, app: &App) {
    let (doc, next) = view(app).build().unwrap().into_parts();
    r.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    *h = next;
    r.layout(Recorder.viewport(), &Bindings::default(), &mut Recorder)
        .unwrap();
}
fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
/// Whether the frame has a partly transparent layer, and paints `OK`.
fn frame(r: &Runtime) -> (bool, bool) {
    let batches = r.paint(&mut Recorder, &Theme::dark());
    let fading = batches
        .iter()
        .flat_map(|b| b.shapes.iter())
        .any(|o| *o > 0.01 && *o < 0.99);
    let ok = batches
        .iter()
        .flat_map(|b| b.glyphs.iter())
        .any(|t| t == "OK");
    (fading, ok)
}

#[test]
fn dialogs_fade_in_and_out_by_default_with_or_without_a_key() {
    for unkeyed in [false, true] {
        let mut app = App {
            unkeyed,
            ..App::default()
        };
        let (mut r, mut h) = mount(&app);
        r.advance_animations(ms(100));
        app.open = true;
        show(&mut r, &mut h, &app);
        r.advance_animations(ms(160));
        assert_eq!(
            frame(&r),
            (true, true),
            "opening fades in (unkeyed={unkeyed})"
        );
        r.advance_animations(ms(600));
        assert!(!r.is_animating());
        assert_eq!(frame(&r), (false, true), "settles fully opaque");

        app.open = false;
        show(&mut r, &mut h, &app);
        r.advance_animations(ms(660));
        assert_eq!(
            frame(&r),
            (true, true),
            "closing fades out (unkeyed={unkeyed})"
        );
        let mut state = app;
        assert!(
            !r.accessibility_activate("ok", &mut h, &mut state)
                .unwrap()
                .activated,
            "the departing copy is paint-only"
        );
        r.advance_animations(ms(1200));
        assert!(!r.is_animating());
        assert_eq!(frame(&r), (false, false));
    }
}

#[test]
fn motion_off_per_component_or_app_wide_opens_and_closes_instantly() {
    for app_wide in [false, true] {
        let mut app = App {
            still: !app_wide,
            ..App::default()
        };
        let (mut r, mut h) = mount(&app);
        // `Motion::None` reaches the runtime as reduced motion.
        r.set_reduced_motion(app_wide);
        r.advance_animations(ms(100));
        app.open = true;
        show(&mut r, &mut h, &app);
        assert!(!r.is_animating());
        assert_eq!(frame(&r), (false, true));
        app.open = false;
        show(&mut r, &mut h, &app);
        assert!(!r.is_animating());
        assert_eq!(frame(&r), (false, false));
    }
}

/// A dialog's fading exit must paint like the live portal: last, unclipped
/// and not inheriting its original parent's opacity.
#[cfg(feature = "native")]
#[test]
fn a_closing_dialog_fades_out_above_and_outside_a_clipped_translucent_parent() {
    use volt_ui_kit::native::RasterPainter;
    let ui = |open: bool| -> Element<App> {
        vstack![
            text("Original parent"),
            dialog(text("Modal"))
                .key("exit-portal")
                .open(open)
                .background("#FFFFFF")
                .border_width(0.)
                .rounded(0)
                .on_dismiss(|_: &mut App| {}),
        ]
        .width(80.)
        .height(100.)
        .opacity(0.25)
        .clip_contents(true)
        .into()
    };
    let viewport = Viewport::new(600., 400., 1.).unwrap();
    let mut p = RasterPainter::new(viewport);
    let (doc, h) = ui(true).build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(viewport, &Bindings::default(), &mut p).unwrap();
    r.advance_animations(ms(1000));
    let card = r
        .boxes()
        .iter()
        .find(|b| b.id == "exit-portal-content")
        .unwrap();
    let [x, y, w, height] = card.bounds.bounds();
    // Far right of the card: well outside the 80-pixel parent clip.
    let sample = [(x + w - 12.) as u32, (y + height / 2.) as u32];
    assert!(sample[0] > 200);
    let (doc, next) = ui(false).build().unwrap().into_parts();
    r.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    r.layout(viewport, &Bindings::default(), &mut p).unwrap();
    r.advance_animations(ms(1040));
    let batches = r.paint_ui(&mut p, &UiTheme::dark());
    let pixel = p
        .rasterize(&batches, [0., 0., 0., 1.])
        .unwrap()
        .pixel(sample[0], sample[1])
        .unwrap();
    assert!(
        pixel.red() > 80,
        "the exiting card was clipped or dimmed by its original parent: {pixel:?}"
    );
}
