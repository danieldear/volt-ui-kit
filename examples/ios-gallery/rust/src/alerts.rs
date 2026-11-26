//! The Alerts page: a question that needs an answer now. iOS shows UIKit's
//! own alert and action sheet; elsewhere Volt draws Material 3's.
use super::*;

#[derive(Default)]
pub struct Alerts {
    pub sign_out: bool,
    pub share: bool,
    pub said: String,
}

fn row(label: &str, key: &str, open: fn(&mut App)) -> Element<App> {
    list_row(label)
        .key(key)
        .on_press(move |a: &mut App| open(a))
        .into()
}

pub fn page(app: &App) -> Container<App> {
    let s = &app.alerts;
    let mut parts: Vec<Element<App>> = vec![list_section(
        "Ask",
        [
            row("Sign out", "alerts-sign-out", |a| a.alerts.sign_out = true),
            row("Share note", "alerts-share", |a| a.alerts.share = true),
        ],
        "Ask only when an action can't be undone. When it can, do it and offer Undo.",
    )
    .into()];
    if !s.said.is_empty() {
        parts.push(
            text(s.said.clone())
                .key("alerts-said")
                .font_size(15.)
                .color(ColorToken::Muted)
                .into(),
        );
    }
    let said = |what: &'static str| move |a: &mut App| a.alerts.said = what.into();
    vstack![
        super::page("alerts-list", parts),
        alert_dialog("Sign out of Volt?")
            .key("alerts-sign-out-alert")
            .message("Drafts on this phone that haven't synced will be lost.")
            .action(
                prompt_action("Sign out")
                    .destructive()
                    .on_press(said("Signed out."))
            )
            .cancel("Cancel")
            .open(s.sign_out)
            .on_close(|a: &mut App| a.alerts.sign_out = false),
        action_sheet("Groceries")
            .key("alerts-share-sheet")
            .action(prompt_action("Copy link").on_press(said("Link copied.")))
            .action(prompt_action("Send a copy").on_press(said("Copy sent.")))
            .action(
                prompt_action("Delete note")
                    .destructive()
                    .on_press(said("Note deleted."))
            )
            .open(s.share)
            .on_close(|a: &mut App| a.alerts.share = false),
    ]
    .key("alerts-screen")
    .flex(1.)
}
