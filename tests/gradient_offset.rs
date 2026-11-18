#![cfg(feature = "native")]
//! A gradient drawn away from where its batch was recorded keeps its colours
//! with it: the colour picker's hue strip sits beside a preview, inside a
//! row, and must run red to red across its own width.
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Viewport,
};

const W: f32 = 300.;
const H: f32 = 320.;

fn picker() -> Element<()> {
    // Indented, as in a settings row, so every part is drawn away from 0.
    hstack([
        vstack(Vec::<Element<()>>::new())
            .key("indent")
            .width(50.)
            .height(10.)
            .into(),
        color_picker("#A3AECA")
            .key("p")
            .area(true)
            .width(240.)
            .on_change(|_: &mut (), _| {})
            .into(),
    ])
    .key("row")
    .align(Align::Start)
    .into()
}

#[test]
fn the_hue_strip_runs_through_every_hue_across_its_own_width() {
    let viewport = Viewport::new(W, H, 1.).unwrap();
    let mut painter = RasterPainter::new(viewport);
    let (document, handlers) = picker().build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    let strip = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "p-hue")
        .unwrap()
        .bounds
        .bounds();
    let batches = runtime.paint_ui(&mut painter, &UiTheme::light());
    let frame = painter.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
    let pixels: Vec<[u8; 3]> = frame
        .pixels()
        .iter()
        .map(|c| [c.red(), c.green(), c.blue()])
        .collect();
    // The track is 10 tall, 2 below the strip's top.
    let y = (strip[1] + 7.) as usize;
    let at = |t: f32| pixels[y * W as usize + (strip[0] + strip[2] * t) as usize];
    let [r, g, b] = at(0.5);
    assert!(
        r < 60 && g > 200 && b > 200,
        "the middle of the strip is cyan, got {:?}",
        [r, g, b]
    );
    let [r, g, b] = at(2. / 3.);
    assert!(
        r < 60 && g < 60 && b > 200,
        "two thirds along is blue, got {:?}",
        [r, g, b]
    );
    let [r, g, b] = at(1. / 6.);
    assert!(
        r > 200 && g > 200 && b < 60,
        "a sixth along is yellow, got {:?}",
        [r, g, b]
    );
}
