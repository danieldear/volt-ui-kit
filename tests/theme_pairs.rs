#![cfg(feature = "rust-ui")]
//! Colours may be "light|dark" pairs: the theme in effect picks the half.
use volt_ui_kit::{ui::prelude::*, ColorToken};

struct Doc;

#[test]
fn light_dark_pairs_resolve_by_the_theme_in_effect() {
    let rgb = |r: u8, g: u8, b: u8| [r as f32 / 255., g as f32 / 255., b as f32 / 255., 1.];
    let (light, dark) = (UiTheme::light(), UiTheme::dark());
    assert_eq!(
        light.resolve("#112233|#DDEEFF"),
        Some(rgb(0x11, 0x22, 0x33))
    );
    assert_eq!(dark.resolve("#112233|#DDEEFF"), Some(rgb(0xDD, 0xEE, 0xFF)));
    // Tokens (with opacity) work on either side; spaces around the bar are fine.
    let mut primary = dark.palette.color(ColorToken::Primary);
    primary[3] *= 0.3;
    assert_eq!(dark.resolve("#FFFFFF | @primary/30"), Some(primary));
    assert_eq!(light.resolve("#FFFFFF | @primary/30"), Some([1.; 4]));
    // Only one bar.
    assert_eq!(dark.resolve("#111111|#222222|#333333"), None);
}

#[test]
fn elements_accept_pairs_and_reject_bad_halves() {
    let ok: Element<Doc> = text("x")
        .color("#112233|@primary")
        .background("#FFFFFFE6|@base-100/90")
        .into();
    assert!(ok.build().is_ok());
    for bad in ["#12|#000000", "#000000|nope", "#000000|#111111|#222222"] {
        let e: Element<Doc> = text("x").color(bad).into();
        assert!(e.build().is_err(), "{bad} should be rejected");
    }
}

#[test]
fn rich_text_spans_accept_pairs_too() {
    let ok: Element<Doc> = rich_text([
        span("a").color("#112233|@primary"),
        span("b").background("#D6960029|#FFCB6B2B"),
    ])
    .into();
    assert!(ok.build().is_ok());
    let bad: Element<Doc> = rich_text([span("a").background("#000000|nope")]).into();
    assert!(bad.build().is_err());
}

#[test]
fn a_command_palette_card_can_be_given_an_opaque_surface() {
    let state = CommandPaletteState {
        open: true,
        ..CommandPaletteState::default()
    };
    let palette: Element<Doc> = command_palette([command(1u8, "One")], &state)
        .surface("#FFFFFF|#1A2338")
        .state(|_: &mut Doc| unreachable!())
        .on_run(|_: &mut Doc, _| {})
        .into();
    assert!(palette.build().is_ok());
}

#[test]
fn a_select_list_can_be_given_an_opaque_surface() {
    let open: Element<Doc> = select([select_item(1u8, "One"), select_item(2, "Two")])
        .key("pick")
        .open(true)
        .surface("#FFFFFF|#1A2338")
        .on_select(|_: &mut Doc, _| {})
        .on_open_change(|_: &mut Doc, _| {})
        .into();
    let (document, _) = open.build().expect("an opaque list builds").into_parts();
    assert!(
        format!("{document:?}").contains("#FFFFFF|#1A2338"),
        "the list is painted with it"
    );
}
