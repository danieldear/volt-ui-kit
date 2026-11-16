#![cfg(feature = "native")]
//! A one-pixel separator (an iOS hairline) paints on a surface at 3x, even
//! when the layout above it puts it at a fractional pixel position.
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::{prelude::*, Palette, UiTheme},
    Viewport,
};

fn render(e: Element<()>) -> (tiny_skia::Pixmap, Runtime) {
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(300., 300., 3.).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    let mut palette = Palette::dark();
    palette.base[1] = [0.11, 0.11, 0.12, 1.];
    palette.border = [0.33, 0.33, 0.35, 0.6];
    let ui = UiTheme::from_palette(palette);
    let batches = r.paint_ui(&mut p, &ui);
    let pixels = p.rasterize(&batches, [0., 0., 0., 1.]).unwrap().clone();
    (pixels, r)
}

#[test]
fn a_default_separator_uses_the_palette_border() {
    let e: Element<()> = vstack![
        vstack![].height(17.),
        divider().key("line"),
        vstack![].height(40.),
    ]
    .background(ColorToken::Base200)
    .width(100.)
    .into();
    let (pixels, r) = render(e);
    let [x, y, _, _] = r
        .boxes()
        .iter()
        .find(|b| b.id == "line")
        .unwrap()
        .bounds
        .bounds();
    let line = pixels.pixel(x as u32 + 10, y as u32 + 1).unwrap();
    // The 60% border over the surface, not the surface and not another colour.
    assert!((55..=70).contains(&line.red()), "{}", line.red());
    assert!(line.blue() >= line.red());
}
#[test]
fn a_hairline_separator_paints_at_a_fractional_position() {
    let e: Element<()> = vstack![
        // 17.13 points tall: the separator starts mid-pixel.
        vstack![].key("above").height(17.13),
        vstack![divider()
            .key("line")
            .height(1. / 3.)
            .color(ColorToken::Border)]
        .padding(16., Edge::Left)
        .align(Align::Stretch),
        vstack![].height(40.),
    ]
    .background(ColorToken::Base200)
    .width(100.)
    .into();
    let (pixels, r) = render(e);
    let line = r.boxes().iter().find(|b| b.id == "line").unwrap();
    let [x, y, w, h] = line.bounds.bounds();
    assert!(w > 100. && h > 0.9, "laid out: {:?}", line.bounds.bounds());
    let surface = pixels.pixel(x as u32 + 10, y as u32 + 20).unwrap();
    let hairline = pixels.pixel(x as u32 + 10, y.ceil() as u32).unwrap();
    assert!(
        hairline.red() > surface.red() + 15,
        "hairline {} vs surface {}",
        hairline.red(),
        surface.red()
    );
}
