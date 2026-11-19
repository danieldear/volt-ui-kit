#![cfg(feature = "native")]
//! A menu's separator is a hairline in its row, not a filled bar, and a label
//! too long for the menu stays on its one row instead of spilling below it.
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::{prelude::*, Palette, UiTheme},
    Viewport,
};

fn render(e: Element<()>) -> (tiny_skia::Pixmap, Runtime) {
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(400., 400., 2.).unwrap();
    let mut p = RasterPainter::new(vp);
    r.set_reduced_motion(true);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    r.settle_animations();
    let mut palette = Palette::dark();
    palette.border = [0.9, 0.9, 0.9, 1.];
    let ui = UiTheme::from_palette(palette);
    let batches = r.paint_ui(&mut p, &ui);
    let pixels = p.rasterize(&batches, [0., 0., 0., 1.]).unwrap().clone();
    (pixels, r)
}

fn bright(pixels: &tiny_skia::Pixmap, x: f32, y: f32, scale: f32) -> bool {
    pixels
        .pixel((x * scale) as u32, (y * scale) as u32)
        .is_some_and(|p| p.red() > 150)
}

fn find(r: &Runtime, key: &str) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == key)
        .unwrap_or_else(|| panic!("no {key}"))
        .bounds
        .bounds()
}

#[test]
fn a_menu_separator_is_a_hairline_not_a_bar() {
    let e: Element<()> = menu(
        "Actions",
        [
            menu_item((), "Save").key("save"),
            menu_separator(),
            menu_item((), "Export").key("export"),
        ],
    )
    .key("actions")
    .open(true)
    .on_action(|_: &mut (), _| {})
    .on_open_change(|_: &mut (), _| {})
    .into();
    let (pixels, r) = render(e);
    // Bounds are in device pixels at scale 2.
    let save = find(&r, "save");
    let export = find(&r, "export");
    let x = save[0] / 2. + 20.;
    let (top, bottom) = ((save[1] + save[3]) / 2., export[1] / 2.);
    let lit: Vec<f32> = (0..((bottom - top) * 4.) as usize)
        .map(|i| top + i as f32 / 4.)
        .filter(|&y| bright(&pixels, x, y, 2.))
        .collect();
    assert!(
        !lit.is_empty(),
        "no separator line between {top} and {bottom}"
    );
    // One pixel at scale 2 is half a point: a few quarter-point samples, not 9 points.
    assert!(
        lit.len() <= 4,
        "the separator fills {} quarter points: {lit:?}",
        lit.len()
    );
}

#[test]
fn a_long_menu_label_stays_on_one_line() {
    let long = "Search the web for this selection and open the results in a new window";
    let e: Element<()> = menu(
        "Actions",
        [
            menu_item((), long).key("long"),
            menu_item((), "Next").key("next"),
        ],
    )
    .key("actions")
    .open(true)
    .on_action(|_: &mut (), _| {})
    .on_open_change(|_: &mut (), _| {})
    .into();
    let (pixels, r) = render(e);
    let [x, y, w, h] = find(&r, "long").map(|v| v / 2.);
    // Rows have a fixed height, so a label that wrapped would spill onto the
    // rows below it in a wider font. It's cut short instead: one band of ink.
    let inked: Vec<bool> = (0..(h as u32 * 2))
        .map(|j| (0..(w as u32)).any(|i| bright(&pixels, x + i as f32, y + j as f32 / 2., 2.)))
        .collect();
    let lines = inked.windows(2).filter(|p| !p[0] && p[1]).count() + usize::from(inked[0]);
    assert_eq!(lines, 1, "the long label shows {lines} lines");
}
