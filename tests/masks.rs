#![cfg(feature = "native")]
//! `Container::mask`: content shows where an alpha mask is opaque or a
//! luminance mask is bright; the mask itself is never painted.
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Viewport,
};

/// A 100 × 40 black window with a white box masked by `mask`.
fn render(mask: Element<()>, mode: MaskMode) -> Vec<[u8; 4]> {
    let content: Element<()> = vstack(Vec::<Element<()>>::new())
        .key("white")
        .width(100.)
        .height(40.)
        .background("#FFFFFF")
        .into();
    let element: Element<()> = zstack![zstack![content]
        .key("masked")
        .flow(Flow::TopLeft)
        .width(100.)
        .height(40.)
        .mask(mask, mode)]
    .key("root")
    .flow(Flow::TopLeft)
    .width(100.)
    .height(40.)
    .background("#000000")
    .into();
    let viewport = Viewport::new(100., 40., 1.).unwrap();
    let mut painter = RasterPainter::new(viewport);
    let (document, handlers) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    let batches = runtime.paint_ui(&mut painter, &UiTheme::light());
    let frame = painter.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
    frame
        .pixels()
        .iter()
        .map(|c| [c.red(), c.green(), c.blue(), c.alpha()])
        .collect()
}

fn at(pixels: &[[u8; 4]], x: usize, y: usize) -> [u8; 4] {
    pixels[y * 100 + x]
}

fn gradient_mask(from: &str, to: &str) -> Element<()> {
    vstack(Vec::<Element<()>>::new())
        .key("mask")
        .width(100.)
        .height(40.)
        .background_gradient(Gradient::linear(
            90.,
            [(0., from.to_owned()), (1., to.to_owned())],
        ))
        .into()
}

#[test]
fn an_alpha_mask_fades_the_content_with_its_opacity() {
    let px = render(gradient_mask("#FF0000FF", "#FF000000"), MaskMode::Alpha);
    let (left, middle, right) = (at(&px, 2, 20), at(&px, 50, 20), at(&px, 97, 20));
    assert!(
        left[0] > 240 && left[1] > 240,
        "opaque mask shows white: {left:?}"
    );
    assert!(
        (100..160).contains(&middle[1]),
        "half mask, half white: {middle:?}"
    );
    assert!(right[1] < 15, "clear mask hides it: {right:?}");
    // The mask's own red never shows.
    assert!(
        px.iter().all(|p| p[0].abs_diff(p[1]) <= 2),
        "no red anywhere"
    );
}

#[test]
fn a_luminance_mask_shows_content_where_it_is_bright() {
    let px = render(gradient_mask("#FFFFFF", "#000000"), MaskMode::Luminance);
    assert!(
        at(&px, 2, 20)[1] > 240,
        "white mask shows: {:?}",
        at(&px, 2, 20)
    );
    assert!(
        at(&px, 97, 20)[1] < 15,
        "black mask hides: {:?}",
        at(&px, 97, 20)
    );
    // Opaque black under alpha would show everything; luminance hides it.
    let alpha = render(gradient_mask("#FFFFFF", "#000000"), MaskMode::Alpha);
    assert!(at(&alpha, 97, 20)[1] > 240);
}

#[test]
fn a_shape_mask_cuts_the_content_to_it_and_is_not_drawn() {
    let circle: Element<()> = vstack(Vec::<Element<()>>::new())
        .key("dot")
        .width(30.)
        .height(30.)
        .rounded(Rounding::Full)
        .background("#00FF00")
        .offset(35., 5.)
        .into();
    let px = render(circle, MaskMode::Alpha);
    assert!(at(&px, 50, 20)[0] > 240, "inside the circle: white");
    assert!(
        at(&px, 36, 6)[0] < 15,
        "corner of its box, outside the circle"
    );
    assert!(at(&px, 10, 20)[0] < 15, "outside the mask");
    assert!(
        px.iter().all(|p| p[1] <= p[0].saturating_add(2)),
        "the mask's green never shows"
    );
}

#[test]
fn only_one_mask_and_it_must_be_last() {
    let mask = || -> Element<()> { vstack(Vec::<Element<()>>::new()).key("m").into() };
    let twice: Element<()> = zstack(Vec::<Element<()>>::new())
        .key("box")
        .mask(mask(), MaskMode::Alpha)
        .mask(vstack(Vec::<Element<()>>::new()).key("m2"), MaskMode::Alpha)
        .into();
    assert!(twice.build().is_err());
    let once: Element<()> = zstack(Vec::<Element<()>>::new())
        .key("box")
        .mask(mask(), MaskMode::Luminance)
        .into();
    assert!(once.build().is_ok());
}
