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
