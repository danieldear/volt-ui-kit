#![cfg(feature = "rust-ui")]
//! Display text over the 8 KiB node limit is shortened, not a failed window.
use volt_ui_kit::ui::prelude::*;

#[test]
fn long_display_text_is_shortened_instead_of_failing_the_tree() {
    let long = "é".repeat(20_000);
    let ui: Element<()> = vstack![
        text(long.clone()).key("message"),
        button(long.clone()).key("action").on_click(|_| {}),
    ]
    .key("root")
    .into();
    let (doc, _) = ui
        .build()
        .expect("long text no longer fails the tree")
        .into_parts();
    let shown = format!("{doc:?}");
    assert!(!shown.contains(&long), "the full text was kept");
    assert!(shown.contains("é…"), "the cut is marked with an ellipsis");
}

#[test]
fn text_at_the_limit_is_kept_whole() {
    let exact = "a".repeat(8192);
    let ui: Element<()> = vstack![text(exact.clone()).key("t")].key("root").into();
    let (doc, _) = ui.build().unwrap().into_parts();
    assert!(format!("{doc:?}").contains(&exact));
}

#[test]
fn editable_text_is_never_cut() {
    // Shortening a field would change what the person typed; it keeps its own check.
    let ui: Element<()> = vstack![text_field("x".repeat(20_000)).key("f").on_change(|_, _| {})]
        .key("root")
        .into();
    assert!(ui.build().is_err());
}
