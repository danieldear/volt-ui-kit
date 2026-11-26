//! Mobile components as Volt draws them where the platform doesn't (Android,
//! desktop previews, the web): the tab view, the bottom sheet and the action
//! dock. `VOLT_SHEET=1` opens the sheet, `VOLT_DOCK_MORE=1` the dock's More.
//!
//! `cargo run --example mobile_parts --features native-gpu -- --size 412x860
//! --theme dark --snapshot tabs.png` renders a phone-sized screenshot.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Ideas,
    Mac,
    Inbox,
    Settings,
}
struct Parts {
    section: Section,
    sheet: bool,
    height: usize,
    actions: bool,
    more: bool,
    ideas: usize,
}
fn page(app: &Parts) -> Element<Parts> {
    let title = match app.section {
        Section::Ideas => "Ideas",
        Section::Mac => "Mac",
        Section::Inbox => "Inbox",
        Section::Settings => "Settings",
    };
    vstack![
        text(title).font_size(34.).font_weight(700),
        text("The selected section's page. Tap a tab to switch.")
            .font_size(17.)
            .color(ColorToken::Muted),
        button("Show the sheet")
            .key("open-sheet")
            .primary(true)
            .height(48.)
            .on_click(|a: &mut Parts| a.sheet = true),
    ]
    .key("page")
    .gap(12., Axis::Vertical)
    .padding(20., Edge::Horizontal)
    .padding(24., Edge::Top)
    .flex(1.)
    .into()
}
const IDEAS: [(&str, &str); 8] = [
    ("Offline drafts", "Keep a reply going on the train"),
    ("Plan review", "Ask a second model to critique the plan"),
    ("Release notes", "Write them from the merged pull requests"),
    ("Pairing mode", "Share one session between two people"),
    ("Voice capture", "Talk an idea in while walking"),
    ("Cost line", "What each agent turn costs, as it runs"),
    ("Quiet hours", "Hold notifications overnight"),
    ("Project import", "Start from an existing repository"),
];
fn ideas(app: &Parts) -> Element<Parts> {
    let rows: Vec<Element<Parts>> = IDEAS
        .iter()
        .take(app.ideas)
        .enumerate()
        .map(|(i, (title, line))| {
            vstack![
                text(*title).font_size(17.).font_weight(600),
                text(*line).font_size(15.).color(ColorToken::Muted),
            ]
            .key(format!("idea-{i}"))
            .gap(2., Axis::Vertical)
            .padding(14., Edge::All)
            .rounded(12.)
            .background(ColorToken::Base200)
            .into()
        })
        .collect();
    let list = vstack![
        text("Ideas").font_size(34.).font_weight(700),
        vstack(rows).gap(10., Axis::Vertical),
    ]
    .key("ideas")
    .gap(16., Axis::Vertical)
    .padding(20., Edge::Horizontal)
    .padding(24., Edge::Top)
    .flex(1.)
    .scroll(Scroll::Auto);
    action_dock(list)
        .key("ideas-dock")
        .follows("ideas")
        .primary(
            dock_action("New idea", Icon::Plus)
                .on_press(|a: &mut Parts| a.ideas = (a.ideas + 1).min(IDEAS.len())),
        )
        .secondary(dock_action("Search", Icon::Search).on_press(|_: &mut Parts| {}))
        .more([
            dock_action("Select", Icon::Check).on_press(|_: &mut Parts| {}),
            dock_action("Sort by date", Icon::Calendar).on_press(|_: &mut Parts| {}),
            dock_action("Delete all", Icon::Trash)
                .danger()
                .on_press(|a: &mut Parts| a.ideas = 0),
        ])
        .more_open(app.more)
        .on_more(|a: &mut Parts, open| a.more = open)
        .into()
}
fn view(app: &Parts) -> Element<Parts> {
    let sheet = bottom_sheet(
        vstack![
            text("Pick up where you left off on your Mac, or start something new here.")
                .font_size(17.)
                .color(ColorToken::Muted),
        ]
        .gap(12., Axis::Vertical),
    )
    .key("sheet")
    .title("New idea")
    .detents([SheetDetent::Medium, SheetDetent::Large])
    .detent(app.height)
    .on_detent(|a: &mut Parts, i| a.height = i)
    .open(app.sheet)
    .on_dismiss(|a: &mut Parts| a.sheet = false);
    let sheet = if app.actions {
        sheet
            .cancel("Cancel", |a: &mut Parts| a.sheet = false)
            .done("Start", |a: &mut Parts| a.sheet = false)
    } else {
        sheet
    };
    zstack![
        tab_view(
            app.section,
            [
                tab_destination(Section::Ideas, "Ideas", Icon::Bolt),
                tab_destination(Section::Mac, "Mac", Icon::Command).dot(),
                tab_destination(Section::Inbox, "Inbox", Icon::Inbox).badge("3"),
                tab_destination(Section::Settings, "Settings", Icon::Settings),
            ],
            if app.section == Section::Ideas {
                ideas(app)
            } else {
                page(app)
            },
        )
        .key("tabs")
        .on_select(|a: &mut Parts, s| a.section = s),
        sheet,
    ]
    .flex(1.)
    .font_family("system")
    .into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sheet = std::env::var("VOLT_SHEET").is_ok();
    support::launch(
        "Mobile parts",
        Parts {
            section: Section::Ideas,
            sheet,
            height: 0,
            actions: std::env::var("VOLT_SHEET_ACTIONS").is_ok(),
            more: std::env::var("VOLT_DOCK_MORE").is_ok(),
            ideas: IDEAS.len(),
        },
        view,
    )
}
