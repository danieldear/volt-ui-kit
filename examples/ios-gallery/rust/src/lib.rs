//! Volt Gallery: every Volt component on iPhone, one page each.
//!
//! The navigation bar, back button, swipe-back and scrolling are UIKit's; the
//! pages are Volt. Each page ends with what to check on a real device.
use volt_ui_kit_mobile::ui_kit::ui::{prelude::*, Button, Container};
use volt_ui_kit_mobile::{Application, HostContext, Updater};

mod alerts;
mod chat;
mod flows;
mod rows;
mod shop;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    Home,
    Navigation,
    Deeper(u8),
    Buttons,
    Lists,
    Controls,
    TextInput,
    Typography,
    Badges,
    Background,
    Sheets,
    Dock,
    Rows,
    Chat,
    Alerts,
    Flows,
    Shop,
    Product(u8),
    Cart,
    Checkout,
    Ordered,
}
impl Route {
    /// Pages of the Pantry sample shop, which use its brand colours.
    fn in_shop(self) -> bool {
        matches!(
            self,
            Route::Shop | Route::Product(_) | Route::Cart | Route::Checkout | Route::Ordered
        )
    }
    fn title(self) -> String {
        match self {
            Route::Home => "Volt Gallery".into(),
            Route::Navigation => "Navigation".into(),
            Route::Deeper(n) => format!("Level {n}"),
            Route::Buttons => "Buttons".into(),
            Route::Lists => "Lists".into(),
            Route::Controls => "Controls".into(),
            Route::TextInput => "Text input".into(),
            Route::Typography => "Typography".into(),
            Route::Badges => "Badges and cards".into(),
            Route::Background => "Background work".into(),
            Route::Sheets => "Sheets".into(),
            Route::Dock => "Action dock".into(),
            Route::Rows => "Swipe and select".into(),
            Route::Chat => "Chat".into(),
            Route::Alerts => "Alerts".into(),
            Route::Flows => "Flows".into(),
            Route::Shop => "Pantry".into(),
            Route::Product(id) => shop::product(id).name.into(),
            Route::Cart => "Cart".into(),
            Route::Checkout => "Checkout".into(),
            Route::Ordered => "Order placed".into(),
        }
    }
}

/// The gallery's sections, each with its own pages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Section {
    Components,
    Shop,
    Work,
}
impl Section {
    fn root(self) -> Route {
        match self {
            Section::Components => Route::Home,
            Section::Shop => Route::Shop,
            Section::Work => Route::Background,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Period {
    Day,
    Week,
    Month,
}

pub struct App {
    /// The open section and its pages; other sections keep theirs in `parked`.
    pub section: Section,
    pub history: NavigationHistory<Route>,
    pub parked: Vec<(Section, NavigationHistory<Route>)>,
    pub editing: bool,
    pub taps: usize,
    pub wifi: bool,
    pub bluetooth: bool,
    pub brightness: f32,
    pub period: Period,
    pub agree: bool,
    pub name: String,
    pub notes: String,
    pub message: String,
    /// The sample shop: product ID to quantity.
    pub cart: std::collections::BTreeMap<u8, u32>,
    pub category: shop::Category,
    pub search: String,
    pub quantity: u32,
    pub window: shop::Window,
    /// Tip in percent.
    pub tip: u32,
    pub ordered: u32,
    /// Sends results of background work back to the UI thread.
    pub updates: Option<Updater<App>>,
    /// Background work: steps received, and whether it is still running.
    pub received: Vec<String>,
    pub working: bool,
    /// The Work page's sample jobs.
    pub approved: bool,
    pub retried: bool,
    /// The Sheets page: open, at which height, and whether closing loses work.
    pub sheet: bool,
    pub sheet_height: usize,
    pub unsaved: bool,
    pub asked: bool,
    /// The Action dock page: how many notes, and whether More is open.
    pub notes_count: usize,
    pub dock_more: bool,
    pub dock_said: String,
    pub dock_query: String,
    pub dock_searching: bool,
    /// The Swipe and select page.
    pub rows: rows::Rows,
    pub chat: chat::Chat,
    pub alerts: alerts::Alerts,
    pub flows: flows::Flows,
}
impl Default for App {
    fn default() -> Self {
        Self {
            section: Section::Components,
            parked: Vec::new(),
            updates: None,
            received: Vec::new(),
            working: false,
            approved: false,
            retried: false,
            sheet: false,
            sheet_height: 0,
            unsaved: false,
            asked: false,
            notes_count: 24,
            dock_more: false,
            dock_said: String::new(),
            dock_query: String::new(),
            dock_searching: false,
            rows: rows::Rows::default(),
            chat: chat::Chat::default(),
            alerts: alerts::Alerts::default(),
            flows: flows::Flows::default(),
            history: NavigationHistory::new(Route::Home),
            editing: false,
            taps: 0,
            wifi: true,
            bluetooth: false,
            brightness: 0.6,
            period: Period::Week,
            agree: false,
            name: String::new(),
            notes: String::new(),
            message: String::new(),
            cart: Default::default(),
            category: shop::Category::All,
            search: String::new(),
            quantity: 1,
            window: shop::Window::Now,
            tip: 15,
            ordered: 0,
        }
    }
}

// Building blocks in the iOS inset grouped style.

/// A tappable row: label, optional value, chevron.
fn link_row(label: &str, value: &str, key: &str, to: Route) -> Element<App> {
    hstack![
        text(label).font_size(17.),
        spacer(),
        text(value).font_size(17.).color(ColorToken::Muted),
        icon(Icon::ChevronRight)
            .size(15.)
            .stroke(2.)
            .color(ColorToken::Muted),
    ]
    .key(key)
    .height(52.)
    .padding(16., Edge::Horizontal)
    .gap(8., Axis::Horizontal)
    .align(Align::Center)
    .accessible_label(label)
    .pressed_style(|s| s.background(ColorToken::Base300))
    .on_click_with(move |a: &mut App, _| {
        a.history.push(to);
    })
    .into()
}
/// A plain row: label and trailing content.
fn row(label: &str, trailing: Element<App>) -> Element<App> {
    hstack![text(label).font_size(17.), spacer(), trailing]
        .height(52.)
        .padding(16., Edge::Horizontal)
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .into()
}
/// Rows on one rounded surface with inset hairlines, a header and a footer.
fn section(title: &str, rows: Vec<Element<App>>, footer: &str) -> Element<App> {
    let mut items = Vec::new();
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            items.push(
                vstack![divider().height(1. / 3.).color(ColorToken::Border)]
                    .padding(16., Edge::Left)
                    .align(Align::Stretch)
                    .into(),
            );
        }
        items.push(r);
    }
    let mut parts: Vec<Element<App>> = Vec::new();
    if !title.is_empty() {
        parts.push(
            hstack![text(title)
                .font_size(15.)
                .font_weight(600)
                .color(ColorToken::Muted)]
            .padding(16., Edge::Horizontal)
            .into(),
        );
    }
    parts.push(
        vstack(items)
            .background(ColorToken::Base200)
            .rounded(26.)
            .clip_contents(true)
            .into(),
    );
    if !footer.is_empty() {
        parts.push(
            hstack![text(footer).font_size(13.).color(ColorToken::Muted)]
                .padding(16., Edge::Horizontal)
                .into(),
        );
    }
    vstack(parts).gap(8., Axis::Vertical).into()
}
/// What to try on the device, at the end of every page.
fn checklist(items: &[&str]) -> Element<App> {
    let rows = items
        .iter()
        .map(|item| {
            hstack![text(*item).font_size(15.)]
                .padding(16., Edge::Horizontal)
                .padding(12., Edge::Vertical)
                .into()
        })
        .collect();
    section("What to check", rows, "")
}
/// A scrolling page with standard margins.
fn page(key: &str, parts: Vec<Element<App>>) -> Container<App> {
    vstack![vstack(parts)
        .gap(28., Axis::Vertical)
        .padding(20., Edge::Vertical)]
    .key(key)
    .padding(16., Edge::Horizontal)
    .flex(1.)
    .scroll(Scroll::Vertical)
}
fn primary(label: impl Into<String>) -> Button<App> {
    button(label)
        .variant(ButtonVariant::Primary)
        .font_size(17.)
        .font_weight(600)
        .height(52.)
        .rounded(26.)
}

// Pages.

fn home(_: &App) -> Container<App> {
    page(
        "home",
        vec![
            section(
                "Native iOS",
                vec![
                    link_row("Navigation", "", "to-navigation", Route::Navigation),
                    link_row("Text input", "", "to-text", Route::TextInput),
                    link_row("Sheets", "", "to-sheets", Route::Sheets),
                    link_row("Action dock", "", "to-dock", Route::Dock),
                    link_row("Swipe and select", "", "to-rows", Route::Rows),
                    link_row("Chat", "", "to-chat", Route::Chat),
                    link_row("Alerts", "", "to-alerts", Route::Alerts),
                    link_row("Flows", "", "to-flows", Route::Flows),
                ],
                "UIKit draws these: the bar, back swipe, keyboard.",
            ),
            section(
                "Controls",
                vec![
                    link_row("Buttons", "", "to-buttons", Route::Buttons),
                    link_row("Switches and sliders", "", "to-controls", Route::Controls),
                ],
                "",
            ),
            section(
                "Content",
                vec![
                    link_row("Lists", "100 rows", "to-lists", Route::Lists),
                    link_row("Typography", "", "to-typography", Route::Typography),
                    link_row("Badges and cards", "", "to-badges", Route::Badges),
                    link_row("Background work", "", "to-background", Route::Background),
                ],
                "Volt draws these with iOS colours and type.",
            ),
            section(
                "Sample apps",
                vec![link_row("Shop", "Pantry", "to-shop", Route::Shop)],
                "A branded grocery shop: grid, product, cart, checkout.",
            ),
        ],
    )
}
fn navigation(app: &App) -> Container<App> {
    page(
        "navigation-page",
        vec![
            section(
                "Go deeper",
                vec![link_row("Level 2", "", "to-level-2", Route::Deeper(2))],
                "Each level is a page in UIKit's stack.",
            ),
            section(
                "Bar button",
                vec![row(
                    "Edit mode",
                    text(if app.editing { "On" } else { "Off" })
                        .key("editing")
                        .font_size(17.)
                        .color(ColorToken::Muted)
                        .into(),
                )],
                "Tap Edit in the bar to switch it.",
            ),
            checklist(&[
                "The title is in the bar; Back shows the previous title.",
                "Swipe from the left edge to go back; stop halfway to cancel.",
                "Edit in the bar flips Edit mode.",
                "Pushing slides the old page left.",
            ]),
        ],
    )
}
fn deeper(n: u8) -> Container<App> {
    let mut rows = Vec::new();
    if n < 5 {
        rows.push(link_row(
            &format!("Level {}", n + 1),
            "",
            &format!("to-level-{}", n + 1),
            Route::Deeper(n + 1),
        ));
    }
    page(
        &format!("level-{n}"),
        vec![
            section("Go deeper", rows, "Up to level 5."),
            primary("Back to start")
                .key(format!("start-{n}"))
                .on_click(|a: &mut App| a.history.reset(Route::Home))
                .into(),
        ],
    )
}
fn buttons(app: &App) -> Container<App> {
    let variant = |label: &str, v: ButtonVariant, key: &str| -> Element<App> {
        button(label)
            .key(key)
            .variant(v)
            .font_size(17.)
            .height(48.)
            .rounded(24.)
            .on_click(|a: &mut App| a.taps += 1)
            .into()
    };
    page(
        "buttons-page",
        vec![
            primary(format!("Tapped {} times", app.taps))
                .key("primary")
                .on_click(|a: &mut App| a.taps += 1)
                .into(),
            vstack![
                variant("Soft", ButtonVariant::Soft, "soft"),
                variant("Neutral", ButtonVariant::Neutral, "neutral"),
                variant("Outline", ButtonVariant::Outline, "outline"),
                variant("Ghost", ButtonVariant::Ghost, "ghost"),
                variant("Delete", ButtonVariant::Danger, "danger"),
                button("Disabled")
                    .key("disabled")
                    .font_size(17.)
                    .height(48.)
                    .rounded(24.)
                    .enabled(false)
                    .on_click(|_: &mut App| {}),
            ]
            .gap(12., Axis::Vertical)
            .into(),
            checklist(&[
                "Each button highlights while your finger is down.",
                "Dragging off a button cancels the tap.",
                "Labels are centred and sharp.",
            ]),
        ],
    )
}
fn lists() -> Container<App> {
    let rows = (1..=100)
        .map(|i| {
            row(
                &format!("Row {i}"),
                text(format!("{}", i * 7 % 60))
                    .font_size(17.)
                    .color(ColorToken::Muted)
                    .into(),
            )
        })
        .collect();
    page(
        "lists-page",
        vec![
            checklist(&[
                "A flick keeps gliding and slows down.",
                "Pulling past the top or bottom bounces.",
                "The scroll bar shows only while scrolling.",
                "Tapping the status bar jumps to the top.",
            ]),
            section("100 rows", rows, ""),
        ],
    )
}
fn controls(app: &App) -> Container<App> {
    page(
        "controls-page",
        vec![
            section(
                "Switches",
                vec![
                    row(
                        "Wi-Fi",
                        toggle("", app.wifi)
                            .key("wifi")
                            .accessible_label("Wi-Fi")
                            .on_change(|a: &mut App, v| a.wifi = v)
                            .into(),
                    ),
                    row(
                        "Bluetooth",
                        toggle("", app.bluetooth)
                            .key("bluetooth")
                            .accessible_label("Bluetooth")
                            .on_change(|a: &mut App, v| a.bluetooth = v)
                            .into(),
                    ),
                ],
                "",
            ),
            section(
                "Slider",
                vec![hstack![slider(app.brightness)
                    .key("brightness")
                    .accessible_label("Brightness")
                    .range(0.0..=1.0)
                    .on_change(|a: &mut App, v| a.brightness = v)
                    .fill_width()
                    .flex(1.)]
                .padding(16., Edge::Horizontal)
                .height(52.)
                .align(Align::Center)
                .into()],
                &format!("Brightness {:.0}%", app.brightness * 100.),
            ),
            section(
                "Segmented control",
                vec![hstack![segmented_control([
                    (Period::Day, "Day"),
                    (Period::Week, "Week"),
                    (Period::Month, "Month"),
                ])
                .key("period")
                .selected(app.period)
                .on_select(|a: &mut App, p| a.period = p)
                .flex(1.)]
                .padding(16., Edge::Horizontal)
                .padding(10., Edge::Vertical)
                .into()],
                "",
            ),
            section(
                "Checkbox",
                vec![hstack![checkbox("I agree to the terms", app.agree)
                    .key("agree")
                    .on_change(|a: &mut App, v| a.agree = v)]
                .padding(16., Edge::Horizontal)
                .height(52.)
                .align(Align::Center)
                .into()],
                "",
            ),
            section(
                "Progress",
                vec![hstack![
                    progress(app.brightness)
                        .key("progress")
                        .fill_width()
                        .flex(1.),
                    spinner().key("spinner"),
                ]
                .padding(16., Edge::Horizontal)
                .height(52.)
                .gap(16., Axis::Horizontal)
                .align(Align::Center)
                .into()],
                "The bar follows the slider.",
            ),
            checklist(&[
                "Switches look and move like iOS switches.",
                "The slider tracks your finger without lag.",
                "Segments switch with a single tap.",
            ]),
        ],
    )
}
fn text_input(app: &App) -> Container<App> {
    page(
        "text-page",
        vec![
            section(
                "Text field",
                vec![hstack![text_field(&app.name)
                    .key("name")
                    .accessible_label("Name")
                    .placeholder("Name")
                    .font_size(17.)
                    .height(44.)
                    .on_change(|a: &mut App, v| a.name = v)
                    .flex(1.)]
                .padding(16., Edge::Horizontal)
                .padding(4., Edge::Vertical)
                .into()],
                if app.name.is_empty() {
                    "Type a name, emoji or another script."
                } else {
                    ""
                },
            ),
            section(
                "Text area",
                vec![hstack![text_area(&app.notes)
                    .key("notes")
                    .accessible_label("Notes")
                    .placeholder("Notes")
                    .font_size(17.)
                    .height(120.)
                    .on_change(|a: &mut App, v| a.notes = v)
                    .flex(1.)]
                .padding(16., Edge::Horizontal)
                .padding(4., Edge::Vertical)
                .into()],
                "Return adds a line; Done closes the keyboard.",
            ),
            primary("Save")
                .key("save")
                .on_click(|a: &mut App| {
                    a.message = if a.name.trim().is_empty() {
                        "Enter a name first.".into()
                    } else {
                        format!("Saved {}.", a.name.trim())
                    };
                })
                .into(),
            hstack![text(&app.message)
                .key("message")
                .font_size(15.)
                .color(ColorToken::Muted)]
            .padding(16., Edge::Horizontal)
            .into(),
            checklist(&[
                "Tapping the field opens the keyboard; the field stays visible.",
                "Done on the keyboard bar closes it.",
                "Emoji and other scripts appear correctly.",
                "Tapping Save while typing saves in one tap.",
            ]),
        ],
    )
}
fn typography() -> Container<App> {
    let styles: [(&str, f32, u16); 11] = [
        ("Large Title", 34., 700),
        ("Title 1", 28., 700),
        ("Title 2", 22., 700),
        ("Title 3", 20., 600),
        ("Headline", 17., 600),
        ("Body", 17., 400),
        ("Callout", 16., 400),
        ("Subheadline", 15., 400),
        ("Footnote", 13., 400),
        ("Caption 1", 12., 400),
        ("Caption 2", 11., 400),
    ];
    let rows = styles
        .iter()
        .map(|(name, size, weight)| {
            hstack![
                text(*name).font_size(*size).font_weight(*weight),
                spacer(),
                text(format!("{size:.0} pt"))
                    .font_size(13.)
                    .color(ColorToken::Muted),
            ]
            .padding(16., Edge::Horizontal)
            .padding(10., Edge::Vertical)
            .align(Align::Center)
            .into()
        })
        .collect();
    page(
        "typography-page",
        vec![
            section("iOS text styles", rows, ""),
            checklist(&[
                "Bold styles are really bold, not regular.",
                "Text is as sharp as in other apps.",
            ]),
        ],
    )
}
fn badges() -> Container<App> {
    let card = |title: &str, body: &str, tag: &str, key: &str| -> Element<App> {
        vstack![
            hstack![
                text(title).font_size(17.).font_weight(600),
                spacer(),
                badge(tag).font_size(12.),
            ]
            .align(Align::Center),
            text(body).font_size(15.).color(ColorToken::Muted),
        ]
        .key(key)
        .gap(6., Axis::Vertical)
        .padding(16., Edge::All)
        .background(ColorToken::Base200)
        .rounded(22.)
        .into()
    };
    page(
        "badges-page",
        vec![
            card(
                "Design review",
                "Thursday, 10:00. Bring the new icons.",
                "New",
                "card-1",
            ),
            card(
                "Ship 0.7",
                "Release notes, store screenshots, TestFlight.",
                "Today",
                "card-2",
            ),
            checklist(&["Cards have even padding and rounded corners."]),
        ],
    )
}

/// Work on another thread sends its results back with an `Updater`.
fn start_work(a: &mut App) {
    let Some(updates) = a.updates.clone() else {
        return;
    };
    a.working = true;
    a.received.clear();
    std::thread::spawn(move || {
        for step in 1..=5 {
            std::thread::sleep(std::time::Duration::from_millis(600));
            let open = updates.update(move |a: &mut App| {
                a.received.push(format!("Result {step} of 5"));
                a.working = step < 5;
            });
            if !open {
                return;
            }
        }
    });
}
fn background(app: &App) -> Container<App> {
    let done = app.received.len();
    let mut cards: Vec<Element<App>> = Vec::new();
    // The worker thread's job, as people would see it.
    if app.working || done > 0 {
        let card = work_card("Import notes").key("work-import");
        cards.push(if app.working {
            card.state(WorkState::Working(Some(done as f32 / 5.)))
                .detail(format!("{done} of 5 notes imported"))
                .into()
        } else {
            card.state(WorkState::Done)
                .detail("5 notes imported")
                .action("Open", |_: &mut App| {})
                .into()
        });
    }
    cards.push(if app.approved {
        work_card("Sync with Mac")
            .key("work-sync")
            .state(WorkState::Done)
            .detail("Queued on the Mac")
            .into()
    } else {
        work_card("Sync with Mac")
            .key("work-sync")
            .state(WorkState::Waiting)
            .detail("Allow this phone to queue work on the Mac")
            .action("Allow", |a: &mut App| a.approved = true)
            .into()
    });
    cards.push(if app.retried {
        work_card("Summarise meeting")
            .key("work-summary")
            .state(WorkState::Working(None))
            .detail("Asking the model again")
            .into()
    } else {
        work_card("Summarise meeting")
            .key("work-summary")
            .state(WorkState::Failed)
            .detail("The model didn't answer. Check the key, then try again.")
            .action("Try again", |a: &mut App| a.retried = true)
            .into()
    });
    let parts = vec![
        primary(if app.working {
            "Importing"
        } else {
            "Import notes"
        })
        .key("start-work")
        .enabled(!app.working)
        .on_click(start_work)
        .into(),
        vstack(cards)
            .key("work-cards")
            .gap(12., Axis::Vertical)
            .into(),
        checklist(&[
            "The import counts up without touching the screen.",
            "Each state has its own colour, icon and words.",
            "The pill shows only while the import runs.",
        ]),
    ];
    let mut screen: Vec<Element<App>> = vec![page("background-page", parts)
        .width_fraction(1.)
        .height_fraction(1.)
        .into()];
    if app.working {
        screen.push(
            activity_pill(format!("Importing {done} of 5"))
                .key("work-pill")
                .into(),
        );
    }
    zstack(screen).key("background-screen").flex(1.)
}

/// Open another section where you left it.
fn open_section(a: &mut App, section: Section) {
    if a.section == section {
        return;
    }
    let mut history = a
        .parked
        .iter()
        .position(|(s, _)| *s == section)
        .map(|i| a.parked.remove(i).1)
        .unwrap_or_else(|| NavigationHistory::new(section.root()));
    std::mem::swap(&mut a.history, &mut history);
    a.parked.push((a.section, history));
    a.section = section;
}
fn dock(app: &App) -> Container<App> {
    let query = app.dock_query.trim().to_lowercase();
    let rows: Vec<Element<App>> = (1..=app.notes_count)
        .filter(|i| query.is_empty() || format!("note {i}").contains(&query))
        .map(|i| {
            row(
                &format!("Note {i}"),
                text(format!("{} min ago", i * 3))
                    .font_size(17.)
                    .color(ColorToken::Muted)
                    .into(),
            )
        })
        .collect();
    let said = if app.dock_said.is_empty() {
        "The page's actions sit at the bottom, in reach of a thumb. Scroll down and the dock moves away; scroll up a little and it's back.".to_owned()
    } else {
        app.dock_said.clone()
    };
    let list = page(
        "dock-list",
        vec![
            text(said)
                .key("dock-said")
                .font_size(15.)
                .color(ColorToken::Muted)
                .into(),
            section(
                &if query.is_empty() {
                    format!("{} notes", app.notes_count)
                } else {
                    format!("Notes matching \"{}\"", app.dock_query.trim())
                },
                rows,
                "",
            ),
        ],
    );
    vstack![action_dock(list)
        .key("dock")
        .follows("dock-list")
        .primary(dock_action("New note", Icon::Plus).on_press(|a: &mut App| {
            a.notes_count += 1;
            a.dock_said = format!("Added note {}.", a.notes_count);
        }))
        .search(
            dock_search(&app.dock_query, app.dock_searching)
                .placeholder("Search notes")
                .on_change(|a: &mut App, q| a.dock_query = q)
                .on_open(|a: &mut App, open| {
                    a.dock_searching = open;
                    if !open {
                        a.dock_query.clear();
                    }
                }),
        )
        .more([
            dock_action("Select", Icon::Check)
                .on_press(|a: &mut App| a.dock_said = "Select mode would start here.".into()),
            dock_action("Sort by date", Icon::Calendar)
                .on_press(|a: &mut App| a.dock_said = "Sorted by date.".into()),
            dock_action("Delete all", Icon::Trash)
                .danger()
                .on_press(|a: &mut App| {
                    a.notes_count = 0;
                    a.dock_said = "Deleted every note.".into();
                }),
        ])
        .more_open(app.dock_more)
        .on_more(|a: &mut App, open| a.dock_more = open)]
    .key("dock-screen")
    .flex(1.)
}
fn sheets(app: &App) -> Container<App> {
    let mut parts = vec![
        primary("Show a sheet")
            .key("show-sheet")
            .on_click(|a: &mut App| {
                a.sheet = true;
                a.asked = false;
            })
            .into(),
        section(
            "Options",
            vec![hstack![
                text("Pretend there are unsaved changes")
                    .font_size(17.)
                    .flex(1.),
                toggle("", app.unsaved)
                    .key("unsaved")
                    .accessible_label("Unsaved changes")
                    .on_change(|a: &mut App, v| a.unsaved = v),
            ]
            .height(52.)
            .padding(16., Edge::Horizontal)
            .align(Align::Center)
            .into()],
            "With unsaved changes, swiping the sheet down asks first.",
        ),
    ];
    if app.asked {
        parts.push(
            text("You swiped with unsaved changes: an app would ask whether to discard them here.")
                .key("asked")
                .font_size(15.)
                .color(ColorToken::Warning)
                .into(),
        );
    }
    parts.push(checklist(&[
        "Only the top corners are rounded, with no border.",
        "Drag the grabber or header up for the full height, down to go back.",
        "Swipe down from half height to close; tap the grabber to change height.",
        "Cancel is on the left, Done on the right.",
    ]));
    let sheet = bottom_sheet(
        vstack![
            text("Name").font_size(15.).color(ColorToken::Muted),
            text_field(&app.name)
                .key("sheet-name")
                .accessible_label("Name")
                .on_change(|a: &mut App, v| a.name = v),
        ]
        .gap(8., Axis::Vertical),
    )
    .key("demo-sheet")
    .title("New list")
    .detents([SheetDetent::Medium, SheetDetent::Large])
    .detent(app.sheet_height)
    .on_detent(|a: &mut App, i| a.sheet_height = i)
    .cancel("Cancel", |a: &mut App| a.sheet = false)
    .done("Create", |a: &mut App| a.sheet = false)
    .done_enabled(!app.name.trim().is_empty())
    .open(app.sheet)
    .on_dismiss(|a: &mut App| a.sheet = false);
    let sheet = if app.unsaved {
        sheet.on_dismiss_attempt(|a: &mut App| a.asked = true)
    } else {
        sheet
    };
    parts.push(sheet.into());
    page("sheets-page", parts)
}
fn view(app: &App) -> Element<App> {
    let in_cart = shop::cart_count(app);
    let shop_tab = tab_destination(Section::Shop, "Shop", Icon::Heart).symbol("bag.fill");
    tab_view(
        app.section,
        [
            tab_destination(Section::Components, "Components", Icon::Grid)
                .symbol("square.grid.2x2.fill"),
            if in_cart > 0 {
                shop_tab.badge(in_cart.to_string())
            } else {
                shop_tab
            },
            tab_destination(Section::Work, "Work", Icon::Bolt).symbol("bolt.fill"),
        ],
        pages(app),
    )
    .key("sections")
    .on_select(open_section)
    // Choosing the open section again goes back to its first page.
    .on_reselect(|a: &mut App, section: Section| a.history.reset(section.root()))
    .into()
}
fn pages(app: &App) -> Element<App> {
    navigation_stack(&app.history, |route| {
        let title = route.title();
        let page = match *route {
            Route::Home => home(app).large_title(true),
            Route::Navigation => navigation(app)
                .bar_button(if app.editing { "Done" } else { "Edit" }, |a: &mut App| {
                    a.editing = !a.editing
                }),
            Route::Deeper(n) => deeper(n),
            Route::Buttons => buttons(app),
            Route::Lists => lists(),
            Route::Controls => controls(app),
            Route::TextInput => text_input(app),
            Route::Typography => typography(),
            Route::Badges => badges(),
            Route::Background => background(app),
            Route::Sheets => sheets(app),
            Route::Dock => dock(app),
            Route::Rows => rows::page(app),
            Route::Chat => chat::page(app),
            Route::Alerts => alerts::page(app),
            Route::Flows => flows::page(app),
            Route::Shop => shop::home(app).large_title(true).bar_button(
                format!("Cart · {}", shop::cart_count(app)),
                |a: &mut App| {
                    a.history.push(Route::Cart);
                },
            ),
            Route::Product(id) => shop::product_page(app, id).bar_button(
                format!("Cart · {}", shop::cart_count(app)),
                |a: &mut App| {
                    a.history.push(Route::Cart);
                },
            ),
            Route::Cart => shop::cart(app),
            Route::Checkout => shop::checkout(app),
            Route::Ordered => shop::ordered(app),
        };
        page.navigation_title(title).into()
    })
    .key("gallery")
    .on_back(|a: &mut App| {
        a.history.back();
    })
    .into()
}

/// `VOLT_GALLERY_PAGE=controls` (or another page) opens that page at launch,
/// for screenshots and checks without tapping through.
fn start_page() -> Option<Route> {
    Some(match std::env::var("VOLT_GALLERY_PAGE").ok()?.as_str() {
        "navigation" => Route::Navigation,
        "buttons" => Route::Buttons,
        "lists" => Route::Lists,
        "controls" => Route::Controls,
        "text" => Route::TextInput,
        "typography" => Route::Typography,
        "badges" => Route::Badges,
        "background" => Route::Background,
        "sheets" => Route::Sheets,
        "dock" => Route::Dock,
        "rows" => Route::Rows,
        "chat" => Route::Chat,
        "alerts" => Route::Alerts,
        "flows" => Route::Flows,
        "shop" => Route::Shop,
        "product" => Route::Product(4),
        "cart" => Route::Cart,
        _ => return None,
    })
}
/// iOS colours everywhere; the sample shop adds its brand colour and, in
/// light mode, white pages with grey fields like most shopping apps.
fn theme(app: &App, mode: ThemeMode, mut palette: Palette) -> UiTheme {
    if app.history.current().in_shop() {
        palette.primary = shop::BRAND_RGBA;
        palette.accent = shop::BRAND_RGBA;
        palette.on_primary = [1., 1., 1., 1.];
        if mode == ThemeMode::Light {
            palette.base.swap(0, 1);
        }
    }
    UiTheme::from_palette(palette)
}
pub fn application(host: HostContext) -> Result<Application<App>, String> {
    let mut app = App::default();
    if let Some(page) = start_page() {
        if page.in_shop() {
            app.history.push(Route::Shop);
            // A filled cart, so the cart page has something to show.
            app.cart.extend([(1, 2), (4, 1), (9, 1)]);
        }
        if page != Route::Shop {
            app.history.push(page);
        }
    }
    Ok(Application::new_with_updater(
        host,
        |updates| App {
            updates: Some(updates),
            ..app
        },
        view,
    )?
    .theme_from_system(theme)
    .allow_software_fallback(true)
    .back_with(
        |a| a.history.can_go_back(),
        |a| {
            a.history.back();
        },
    ))
}
volt_ui_kit_mobile::export_ios_app!(application);

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit_mobile::PlatformEvent;
    fn app() -> Application<App> {
        let mut a = application(HostContext {
            font_family: "sans-serif".into(),
        })
        .unwrap();
        a.handle(PlatformEvent::Resize {
            width: 393.,
            height: 760.,
            scale: 3.,
        })
        .unwrap();
        a
    }
    #[test]
    fn every_page_opens_from_home_and_goes_back() {
        let mut a = app();
        for (row, route) in [
            ("to-navigation", Route::Navigation),
            ("to-text", Route::TextInput),
            ("to-buttons", Route::Buttons),
            ("to-controls", Route::Controls),
            ("to-lists", Route::Lists),
            ("to-typography", Route::Typography),
            ("to-badges", Route::Badges),
        ] {
            a.handle(PlatformEvent::Activate { id: row.into() })
                .unwrap();
            assert_eq!(*a.state().history.current(), route, "{row}");
            a.handle(PlatformEvent::Navigation {
                id: "gallery-back".into(),
            })
            .unwrap();
            assert_eq!(*a.state().history.current(), Route::Home);
        }
    }
    #[test]
    fn the_bar_describes_each_page() {
        let mut a = app();
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        let bar = a.snapshot()["navigation"].clone();
        assert_eq!(bar["title"], "Volt Gallery");
        assert_eq!(bar["largeTitle"], true);
        a.handle(PlatformEvent::Activate {
            id: "to-navigation".into(),
        })
        .unwrap();
        let bar = a.snapshot()["navigation"].clone();
        assert_eq!(bar["title"], "Navigation");
        assert_eq!(bar["buttons"][0]["label"], "Edit");
        let edit = bar["buttons"][0]["id"].as_str().unwrap().to_owned();
        a.handle(PlatformEvent::Navigation { id: edit }).unwrap();
        assert!(a.state().editing);
        assert_eq!(a.snapshot()["navigation"]["buttons"][0]["label"], "Done");
        for level in 2..=5 {
            a.handle(PlatformEvent::Activate {
                id: format!("to-level-{level}"),
            })
            .unwrap();
        }
        assert_eq!(a.snapshot()["navigation"]["depth"], 5);
        a.handle(PlatformEvent::Activate {
            id: "start-5".into(),
        })
        .unwrap();
        assert_eq!(a.snapshot()["navigation"]["depth"], 0);
    }
    #[test]
    fn going_back_returns_to_the_same_scroll_position() {
        let mut a = app();
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        a.handle(PlatformEvent::ScrollInsets {
            id: "home".into(),
            top: 150.,
            bottom: 34.,
        })
        .unwrap();
        a.handle(PlatformEvent::ScrollTo {
            id: "home".into(),
            x: 0.,
            y: 120.,
        })
        .unwrap();
        let home = |a: &Application<App>| {
            a.snapshot()["scrollViews"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == "home")
                .cloned()
        };
        assert_eq!(home(&a).unwrap()["offset"][1], 120.);
        // A row in view, as a tapped row is: returning reveals it.
        a.handle(PlatformEvent::Activate {
            id: "to-text".into(),
        })
        .unwrap();
        assert!(home(&a).is_none());
        a.handle(PlatformEvent::Navigation {
            id: "gallery-back".into(),
        })
        .unwrap();
        // Same offset and the same insets on the first frame back.
        assert_eq!(home(&a).unwrap()["offset"][1], 120.);
        assert_eq!(
            a.mobile_view().runtime().scroll_insets("home"),
            [150., 0., 34., 0.]
        );
    }
    #[test]
    fn a_shopper_fills_the_cart_and_places_an_order() {
        let mut a = app();
        let accent = |a: &Application<App>| a.snapshot()["accent"].clone();
        let system = accent(&a);
        let go = |a: &mut Application<App>, id: &str| {
            a.handle(PlatformEvent::Activate { id: id.into() }).unwrap();
        };
        go(&mut a, "to-shop");
        assert_eq!(*a.state().history.current(), Route::Shop);
        assert_ne!(accent(&a), system, "the shop uses its brand colour");
        assert_eq!(
            a.snapshot()["navigation"]["buttons"][0]["label"],
            "Cart · 0"
        );
        // Quick add from the grid, then the card shows a stepper.
        go(&mut a, "card-1-add");
        go(&mut a, "card-1-plus");
        assert_eq!(a.state().cart[&1], 2);
        assert_eq!(
            a.snapshot()["navigation"]["buttons"][0]["label"],
            "Cart · 2"
        );
        // A product page adds a chosen quantity and returns.
        go(&mut a, "card-4");
        assert_eq!(*a.state().history.current(), Route::Product(4));
        go(&mut a, "qty-plus");
        go(&mut a, "add-to-cart");
        assert_eq!(*a.state().history.current(), Route::Shop);
        assert_eq!(a.state().cart[&4], 2);
        // Search narrows the grid.
        a.handle(PlatformEvent::Text {
            id: "search".into(),
            text: "lime".into(),
        })
        .unwrap();
        let ids: Vec<String> = a.snapshot()["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap().to_owned())
            .collect();
        assert!(ids.iter().any(|id| id == "card-3"));
        assert!(!ids.iter().any(|id| id == "card-1"));
        // Cart, checkout, order.
        let cart = a.snapshot()["navigation"]["buttons"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        a.handle(PlatformEvent::Navigation { id: cart }).unwrap();
        assert_eq!(*a.state().history.current(), Route::Cart);
        // 2 × $4.49 + 2 × $5.49 = $19.96, under the free delivery threshold.
        assert_eq!(shop::price(2 * 449 + 2 * 549), "$19.96");
        go(&mut a, "checkout");
        go(&mut a, "place-order");
        assert_eq!(*a.state().history.current(), Route::Ordered);
        assert_eq!(a.state().ordered, 4);
        assert!(a.state().cart.is_empty());
        go(&mut a, "back-to-shop");
        assert_eq!(a.state().history.entries(), &[Route::Home, Route::Shop]);
        a.handle(PlatformEvent::Navigation {
            id: "gallery-back".into(),
        })
        .unwrap();
        assert_eq!(accent(&a), system, "other pages keep the system colours");
    }
    #[test]
    fn the_text_area_is_an_editable_multiline_field() {
        let mut a = app();
        a.handle(PlatformEvent::Activate {
            id: "to-text".into(),
        })
        .unwrap();
        let notes = a.snapshot()["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "notes")
            .unwrap()
            .clone();
        assert_eq!(notes["kind"], "field");
        assert_eq!(notes["multiline"], true);
        a.handle(PlatformEvent::Focus { id: "notes".into() })
            .unwrap();
        a.handle(PlatformEvent::Text {
            id: "notes".into(),
            text: "Milk\nEggs".into(),
        })
        .unwrap();
        assert_eq!(a.state().notes, "Milk\nEggs");
    }
    #[test]
    fn native_switches_and_sliders_report_and_take_values() {
        let mut a = app();
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        a.handle(PlatformEvent::Activate {
            id: "to-controls".into(),
        })
        .unwrap();
        let node = |a: &Application<App>, id: &str| {
            a.snapshot()["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == id)
                .unwrap()
                .clone()
        };
        let wifi = node(&a, "wifi");
        assert_eq!(wifi["kind"], "switch");
        assert_eq!(wifi["checked"], true);
        a.handle(PlatformEvent::Activate { id: "wifi".into() })
            .unwrap();
        assert!(!a.state().wifi);
        let brightness = node(&a, "brightness");
        assert_eq!(brightness["kind"], "slider");
        assert_eq!(brightness["numeric"][2], 1.);
        a.handle(PlatformEvent::Value {
            id: "brightness".into(),
            value: 0.25,
        })
        .unwrap();
        assert!((a.state().brightness - 0.25).abs() < 0.01);
    }
    #[test]
    fn background_work_reports_back_without_a_touch() {
        let mut a = app();
        a.handle(PlatformEvent::Activate {
            id: "to-background".into(),
        })
        .unwrap();
        a.handle(PlatformEvent::Activate {
            id: "start-work".into(),
        })
        .unwrap();
        assert!(a.state().working);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while a.state().working && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
            a.handle(PlatformEvent::Updates {}).unwrap();
        }
        assert_eq!(a.state().received.len(), 5);
        assert!(!a.state().working);
        let nodes = a.snapshot()["nodes"].to_string();
        assert!(nodes.contains("Import notes, Done") && nodes.contains("5 notes imported"));
        // The pill goes when the work does; the waiting job takes its answer.
        assert!(!nodes.contains("work-pill"));
        a.handle(PlatformEvent::Activate {
            id: "work-sync-action".into(),
        })
        .unwrap();
        assert!(a.state().approved);
    }
    #[test]
    fn each_section_keeps_its_pages() {
        let mut a = app();
        a.handle(PlatformEvent::Activate {
            id: "to-buttons".into(),
        })
        .unwrap();
        assert_eq!(*a.state().history.current(), Route::Buttons);
        a.handle(PlatformEvent::Activate {
            id: "sections-tab-1".into(),
        })
        .unwrap();
        assert_eq!(a.state().section, Section::Shop);
        assert_eq!(*a.state().history.current(), Route::Shop);
        a.handle(PlatformEvent::Activate {
            id: "sections-tab-0".into(),
        })
        .unwrap();
        assert_eq!(
            *a.state().history.current(),
            Route::Buttons,
            "back where you left it"
        );
        // Choosing the open section again goes to its first page.
        a.handle(PlatformEvent::Activate {
            id: "sections-tab-0".into(),
        })
        .unwrap();
        assert_eq!(*a.state().history.current(), Route::Home);
        assert_eq!(a.snapshot()["tabs"]["items"][1]["symbol"], "bag.fill");
    }
    #[test]
    fn select_mode_deletes_the_selection_and_undo_brings_it_back() {
        let mut a = app();
        a.handle(PlatformEvent::Activate {
            id: "to-rows".into(),
        })
        .unwrap();
        assert_eq!(*a.state().history.current(), Route::Rows);
        for id in [
            "rows-dock-more",
            "rows-dock-more-0",
            "note-1-content",
            "note-3-content",
        ] {
            a.handle(PlatformEvent::Activate { id: id.into() }).unwrap();
        }
        assert!(a.state().rows.selecting);
        assert_eq!(a.state().rows.selected.len(), 2);
        a.handle(PlatformEvent::Activate {
            id: "rows-dock-primary".into(),
        })
        .unwrap();
        assert_eq!(a.state().rows.notes.len(), 6);
        assert!(!a.state().rows.selecting);
        a.handle(PlatformEvent::Activate {
            id: "rows-undo-bar-action".into(),
        })
        .unwrap();
        let order: Vec<u32> = a.state().rows.notes.iter().map(|n| n.id).collect();
        assert_eq!(order, (0..8).collect::<Vec<_>>(), "back where they were");
    }
    #[test]
    fn a_project_is_made_in_three_steps() {
        let mut a = app();
        for id in ["to-flows", "flows-due-1", "flows-new"] {
            a.handle(PlatformEvent::Activate { id: id.into() }).unwrap();
        }
        assert_eq!(a.state().flows.due, Some(1));
        assert!(a.state().flows.creating);
        // Next waits for a name.
        a.handle(PlatformEvent::Activate {
            id: "flows-sheet-done".into(),
        })
        .unwrap();
        assert_eq!(a.state().flows.step, 0);
        a.handle(PlatformEvent::Text {
            id: "flows-name".into(),
            text: "Parser".into(),
        })
        .unwrap();
        for id in ["flows-sheet-done", "flows-agent-0", "flows-sheet-done"] {
            a.handle(PlatformEvent::Activate { id: id.into() }).unwrap();
        }
        assert_eq!(a.state().flows.step, 2);
        // Back, then forward again, then Create.
        a.handle(PlatformEvent::Activate {
            id: "flows-sheet-cancel".into(),
        })
        .unwrap();
        assert_eq!(a.state().flows.step, 1);
        for id in ["flows-sheet-done", "flows-sheet-done"] {
            a.handle(PlatformEvent::Activate { id: id.into() }).unwrap();
        }
        assert_eq!(a.state().flows.created, ["Parser"]);
        assert!(!a.state().flows.creating);
    }
    #[test]
    fn the_alerts_page_asks_and_answers() {
        let mut a = app();
        for id in ["to-alerts", "alerts-share-content"] {
            a.handle(PlatformEvent::Activate { id: id.into() }).unwrap();
        }
        assert!(a.state().alerts.share);
        a.handle(PlatformEvent::Activate {
            id: "alerts-share-sheet-action-0".into(),
        })
        .unwrap();
        assert_eq!(
            (a.state().alerts.share, a.state().alerts.said.as_str()),
            (false, "Link copied.")
        );
        a.handle(PlatformEvent::Activate {
            id: "alerts-sign-out-content".into(),
        })
        .unwrap();
        a.handle(PlatformEvent::Activate {
            id: "alerts-sign-out-alert-cancel".into(),
        })
        .unwrap();
        assert!(!a.state().alerts.sign_out);
        assert_eq!(a.state().alerts.said, "Link copied.");
    }
    #[test]
    fn the_chat_sends_streams_and_stops() {
        let mut a = app();
        a.handle(PlatformEvent::Activate {
            id: "to-chat".into(),
        })
        .unwrap();
        assert_eq!(*a.state().history.current(), Route::Chat);
        // Nothing to send yet: Send is off.
        a.handle(PlatformEvent::Activate {
            id: "chat-composer-send".into(),
        })
        .unwrap();
        assert!(a.state().chat.messages.is_empty());
        a.handle(PlatformEvent::Text {
            id: "chat-composer-field".into(),
            text: "How should a composer behave?".into(),
        })
        .unwrap();
        a.handle(PlatformEvent::Activate {
            id: "chat-composer-send".into(),
        })
        .unwrap();
        let chat = &a.state().chat;
        assert_eq!(chat.messages.len(), 2);
        assert!(chat.messages[0].mine && chat.draft.is_empty());
        assert!(chat.replying, "Send became Stop");
        a.handle(PlatformEvent::Activate {
            id: "chat-composer-stop".into(),
        })
        .unwrap();
        assert!(!a.state().chat.replying);
    }
    #[test]
    fn holding_a_note_opens_its_menu() {
        let mut a = app();
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        a.handle(PlatformEvent::Activate {
            id: "to-rows".into(),
        })
        .unwrap();
        let b = a
            .mobile_view()
            .runtime()
            .boxes()
            .iter()
            .find(|b| b.id == "note-1")
            .unwrap()
            .bounds
            .bounds()
            .map(|v| v / 3.);
        let (x, y) = (b[0] + b[2] / 2., b[1] + b[3] / 2.);
        a.handle(PlatformEvent::Begin { x, y }).unwrap();
        a.handle(PlatformEvent::Hold { x, y }).unwrap();
        a.handle(PlatformEvent::End { x, y }).unwrap();
        assert_eq!(a.state().rows.menu, Some(1));
        a.handle(PlatformEvent::Activate {
            id: "note-1-menu-2".into(),
        })
        .unwrap();
        assert!(a.state().rows.selecting);
        assert_eq!(a.state().rows.selected.len(), 1);
        assert_eq!(a.state().rows.menu, None);
    }
    #[test]
    fn the_action_dock_page_runs_its_actions() {
        let mut a = app();
        a.handle(PlatformEvent::Activate {
            id: "to-dock".into(),
        })
        .unwrap();
        assert_eq!(*a.state().history.current(), Route::Dock);
        a.handle(PlatformEvent::Activate {
            id: "dock-primary".into(),
        })
        .unwrap();
        assert_eq!(a.state().notes_count, 25);
        a.handle(PlatformEvent::Activate {
            id: "dock-more".into(),
        })
        .unwrap();
        assert!(a.state().dock_more);
        a.handle(PlatformEvent::Activate {
            id: "dock-more-2".into(),
        })
        .unwrap();
        assert_eq!((a.state().notes_count, a.state().dock_more), (0, false));
    }
}
