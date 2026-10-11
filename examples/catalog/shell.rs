//! Site chrome: header, sidebar, page frame, landing page, command palette, toasts.
use super::{
    art::{icon, mark, Icon},
    pages::{dialogs, doc, Page, PAGES},
    App, THEMES,
};
use volt_ui_kit::ui::{prelude::*, Element as El};

/// Fully transparent pixels let clicks fall through to the window behind on
/// macOS, so the tint never goes below this.
pub const TINT_MIN: u8 = 10;

const SIDEBAR: f32 = 248.;

pub fn view(app: &App) -> El<App> {
    let page = if app.page == Page::Introduction {
        landing(app)
    } else {
        article(app)
    };
    // Desktop-app layout: the sidebar sits on the (optionally frosted) window
    // material; the content is a solid sheet so text never reads over blur.
    let content = vstack![
        header(app),
        divider().background(ColorToken::Border),
        // Vertical breathing room lives inside the scrolled content: scroll
        // padding would also shrink the clip, cutting content off early.
        vstack![
            spacer().height(32.).flex(0.),
            page,
            spacer().height(64.).flex(0.),
        ]
        .key(format!("page-{}", app.page.slug()))
        .transition(Transition::fade().duration_ms(160))
        .flex(1.)
        .padding(responsive(24.).at(Breakpoint::Xl, 56.), Edge::Horizontal)
        .scroll(Scroll::Vertical),
    ]
    .key("content")
    .flex(1.)
    .align(Align::Stretch)
    .background(ColorToken::Base100)
    .border_color(if app.glass {
        ColorToken::BaseContent.alpha(9)
    } else {
        ColorToken::Border.into()
    })
    .border_width(1.);
    let content: El<App> = if app.glass {
        // A sheet floating inside the material. Inset on every free edge, it
        // never meets the window's own rounded corners, whatever their radius.
        // No rounded clip: the page is padded clear of the corners, and a
        // full-window clip layer would push page fades past the GPU's layer
        // budget into software frames.
        vstack![
            vstack![].key("sheet-drag").height(8.).drag_region(true),
            content
                .rounded(12.)
                .shadow(Shadow::new().color("#0000002E").blur(18.).offset(0., 2.)),
        ]
        .key("sheet-frame")
        .flex(1.)
        .align(Align::Stretch)
        .padding(8., Edge::Right)
        .padding(8., Edge::Bottom)
        .into()
    } else {
        content.into()
    };
    let body = hstack![sidebar(app), content]
        .flex(1.)
        .align(Align::Stretch);
    let mut layers: Vec<El<App>> = vec![body.into()];
    layers.extend(dialogs(app));
    layers.push(palette(app));
    layers.push(toast(app));
    layers.extend(super::lifecycle::overlays(app));
    zstack(layers)
        .key("catalog")
        .align(Align::Stretch)
        .font_family("SF")
        .font_size(14.)
        .color(ColorToken::BaseContent)
        // In glass mode the window clears fully transparent and this fill is
        // the sidebar's tint, so its strength can change live.
        .background(if app.glass {
            ColorToken::Base100.alpha(app.tint)
        } else {
            ColorToken::Base200.into()
        })
        .into()
}

fn mode_button(app: &App) -> El<App> {
    let dark = app.mode == ThemeMode::Dark;
    zstack![
        button("")
            .key("mode-toggle")
            .accessible_label(if dark {
                "Use light appearance"
            } else {
                "Use dark appearance"
            })
            .variant(ButtonVariant::Ghost)
            .width(36.)
            .height(36.)
            .on_click(move |app: &mut App| {
                app.mode = if dark {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
            }),
        icon(
            if dark { Icon::Sun } else { Icon::Moon },
            18.,
            ColorToken::BaseContent
        ),
    ]
    .width(36.)
    .height(36.)
    .into()
}

fn header(app: &App) -> El<App> {
    let nav = |label: &'static str, target: Page| -> El<App> {
        let active = match target {
            Page::Introduction => app.page == Page::Introduction,
            Page::Theming => app.page == Page::Theming,
            Page::SignIn => app.page.group() == "Patterns",
            _ => app.page.group() == "Components",
        };
        button(label)
            .key(format!("nav-{}", target.slug()))
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Small)
            .color(if active {
                ColorToken::BaseContent
            } else {
                ColorToken::Muted
            })
            .font_weight(if active { 600 } else { 500 })
            .on_click(move |app: &mut App| app.go(target))
            .into()
    };
    let search = zstack![
        button("Search components…")
            .key("open-palette")
            .variant(ButtonVariant::Outline)
            .label_align(Align::Start)
            .color(ColorToken::Muted)
            .width(240.)
            .height(36.)
            .on_click(|app: &mut App| app.palette_open = true),
        hstack![spacer(), icon(Icon::Search, 16., ColorToken::Muted)]
            .align(Align::Center)
            .padding(12., Edge::Right),
    ]
    .width(240.)
    .height(36.)
    .align(Align::Stretch);
    let themes = menu(
        THEMES[app.theme].0,
        THEMES
            .iter()
            .enumerate()
            .map(|(i, (name, ..))| menu_item(i, *name)),
    )
    .key("theme-menu")
    .accessible_label("Accent color")
    .open(app.theme_menu_open)
    .width(116.)
    .height(36.)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .on_action(|app: &mut App, i| app.theme = i)
    .on_open_change(|app: &mut App, open| app.theme_menu_open = open);
    toolbar![
        hstack![
            nav("Docs", Page::Introduction),
            nav("Components", Page::Button),
            nav("Themes", Page::Theming),
            nav("Patterns", Page::SignIn),
        ]
        .key("header-navigation")
        .accessible_label("Documentation navigation")
        .height(36.)
        .gap(6., Axis::Horizontal)
        .align(Align::Center)
        .scroll(Scroll::Horizontal)
        .scrollbars(Scrollbars::Hidden)
        .flex(1.),
        search,
        themes,
        mode_button(app),
    ]
    .key("site-header")
    .height(56.)
    .gap(6., Axis::Horizontal)
    .padding(16., Edge::Horizontal)
    .drag_region(true)
    .into()
}

fn sidebar(app: &App) -> El<App> {
    let query = app.sidebar_query.trim().to_lowercase();
    let mut items: Vec<El<App>> = vec![text_field(&app.sidebar_query)
        .key("sidebar-filter")
        .accessible_label("Filter component pages")
        .placeholder("Filter…")
        .height(32.)
        .font_size(13.)
        .background(ColorToken::BaseContent.alpha(5))
        .border_color(ColorToken::BaseContent.alpha(10))
        .on_change(|app: &mut App, v| app.sidebar_query = v)
        .into()];
    let mut group = "";
    let mut shown = 0;
    for page in PAGES {
        if !query.is_empty() && !page.title().to_lowercase().contains(&query) {
            continue;
        }
        if page.group() != group {
            group = page.group();
            items.push(
                vstack![text(group)
                    .font_size(12.)
                    .font_weight(600)
                    .color(ColorToken::BaseContent)]
                .key(format!("group-{}", group.replace(' ', "-").to_lowercase()))
                .padding(16., Edge::Top)
                .padding(4., Edge::Bottom)
                .padding(10., Edge::Left)
                .into(),
            );
        }
        shown += 1;
        let active = app.page == page;
        let mut link = button(page.title())
            .key(format!("side-{}", page.slug()))
            .variant(ButtonVariant::Ghost)
            .label_align(Align::Start)
            .height(30.)
            .font_size(13.)
            .flex(1.)
            .on_click(move |app: &mut App| app.go(page));
        link = if active {
            link.background(ColorToken::BaseContent.alpha(if app.glass { 10 } else { 7 }))
                .font_weight(600)
        } else {
            link.color(ColorToken::Muted)
        };
        let row: El<App> = if page.is_new() {
            hstack![
                link,
                badge("New")
                    .font_size(10.)
                    .rounded(Rounding::Full)
                    .background(ColorToken::Primary.alpha(14))
                    .color(ColorToken::Primary),
            ]
            .gap(6., Axis::Horizontal)
            .align(Align::Center)
            .into()
        } else {
            link.into()
        };
        items.push(row);
    }
    if shown == 0 {
        items.push(
            text("No matching pages.")
                .font_size(13.)
                .color(ColorToken::Muted)
                .into(),
        );
    }
    // The traffic lights sit over the sidebar on macOS; elsewhere the brand
    // row starts at the top.
    let mac = cfg!(target_os = "macos");
    let brand = hstack![
        mark(24.),
        text("Volt UI").font_size(15.).font_weight(700),
        badge(concat!(
            env!("CARGO_PKG_VERSION_MAJOR"),
            ".",
            env!("CARGO_PKG_VERSION_MINOR")
        ))
        .font_size(10.)
        .rounded(Rounding::Full)
        .background(ColorToken::BaseContent.alpha(8))
        .color(ColorToken::Muted),
    ]
    .key("brand")
    .height(40.)
    .gap(8., Axis::Horizontal)
    .align(Align::Center)
    .padding(18., Edge::Horizontal);
    vstack![
        vstack![]
            .key("titlebar-space")
            .height(if mac { 48. } else { 8. })
            .drag_region(true),
        brand,
        vstack(items)
            .key("sidebar")
            .accessible_label("Component navigation")
            .flex(1.)
            .padding(12., Edge::Horizontal)
            .padding(8., Edge::Top)
            .padding(16., Edge::Bottom)
            .gap(2., Axis::Vertical)
            .scroll(Scroll::Vertical)
            .scrollbars(Scrollbars::Hidden),
        tint_control(app),
    ]
    .key("sidebar-column")
    .width(SIDEBAR)
    .align(Align::Stretch)
    .into()
}

/// Sidebar material strength, shown only for translucent windows.
fn tint_control(app: &App) -> El<App> {
    if !app.glass {
        return spacer().height(0.).flex(0.).into();
    }
    vstack![
        hstack![
            text("Sidebar tint")
                .font_size(12.)
                .font_weight(600)
                .color(ColorToken::Muted),
            spacer(),
            text(format!("{}%", app.tint))
                .font_size(12.)
                .color(ColorToken::Muted),
        ]
        .align(Align::Center),
        slider(app.tint as f32)
            .key("sidebar-tint")
            .accessible_label("Sidebar tint")
            .range(TINT_MIN as f32..=100.)
            .step(1.)
            .height(24.)
            .on_change(|app: &mut App, v| app.tint = v.round() as u8),
    ]
    .key("tint")
    .gap(6., Axis::Vertical)
    .padding(18., Edge::Horizontal)
    .padding(12., Edge::Vertical)
    .into()
}

fn landing(app: &App) -> El<App> {
    let pages: Vec<Page> = PAGES
        .into_iter()
        .filter(|p| *p != Page::Introduction)
        .collect();
    let card = |page: Page| -> El<App> {
        let key = format!("card-{}", page.slug());
        zstack![
            button("")
                .key(key.clone())
                .accessible_label(format!("Open {}", page.title()))
                .variant(ButtonVariant::Outline)
                .rounded(12.)
                .hover_style(|s| s
                    .background(ColorToken::Base200)
                    .border_color(ColorToken::Muted))
                .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(140))
                .on_click(move |app: &mut App| app.go(page)),
            vstack![
                hstack![
                    text(page.title()).font_size(15.).font_weight(600),
                    spacer(),
                    if page.is_new() {
                        El::from(
                            badge("New")
                                .font_size(10.)
                                .rounded(Rounding::Full)
                                .background(ColorToken::Primary.alpha(14))
                                .color(ColorToken::Primary),
                        )
                    } else {
                        spacer().width(0.).flex(0.).into()
                    },
                ]
                .align(Align::Center),
                text(page.summary()).font_size(13.).color(ColorToken::Muted),
            ]
            .padding(16., Edge::All)
            .gap(6., Axis::Vertical),
        ]
        .key(format!("{key}-layer"))
        .flow(Flow::TopLeft)
        .align(Align::Stretch)
        .flex(1.)
        .height(104.)
        .into()
    };
    let rows: Vec<El<App>> = pages
        .chunks(3)
        .map(|chunk| {
            let mut cells: Vec<El<App>> = chunk.iter().map(|p| card(*p)).collect();
            while cells.len() < 3 {
                cells.push(spacer().into());
            }
            stack(cells)
                .direction(
                    responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal),
                )
                .gap(12., Axis::Both)
                .into()
        })
        .collect();
    let _ = app;
    vstack![
        vstack![
            badge("New: vector GPU text, screen readers and damage tracking")
                .rounded(Rounding::Full)
                .font_size(12.)
                .background(ColorToken::Base200),
            text("Beautiful native components for Rust.").font_size(44.).font_weight(700),
            text("Declare your interface as ordinary functions of your state. Buttons, dialogs, tables and trees with real keyboard, screen-reader and GPU support. No webview.")
                .font_size(18.)
                .color(ColorToken::Muted),
            hstack![
                button("Browse components").key("hero-browse").variant(ButtonVariant::Primary).size(ControlSize::Large).on_click(|app: &mut App| app.go(Page::Button)),
                button("Theming").key("hero-theming").variant(ButtonVariant::Outline).size(ControlSize::Large).on_click(|app: &mut App| app.go(Page::Theming)),
            ]
            .gap(10., Axis::Horizontal),
        ]
        .gap(16., Axis::Vertical)
        .padding(24., Edge::Bottom),
        text("Components").font_size(22.).font_weight(700),
        vstack(rows).gap(12., Axis::Vertical),
    ]
    .gap(20., Axis::Vertical)
    .into()
}

fn article(app: &App) -> El<App> {
    let page = app.page;
    let content = doc(page, app);
    let mut parts: Vec<El<App>> = vec![
        hstack![
            text("Docs").font_size(13.).color(ColorToken::Muted),
            text("›").color(ColorToken::Muted),
            text(page.group()).font_size(13.).color(ColorToken::Muted),
            text("›").color(ColorToken::Muted),
            text(page.title()).font_size(13.).font_weight(500),
        ]
        .gap(6., Axis::Horizontal)
        .align(Align::Center)
        .into(),
        hstack![
            text(page.title()).font_size(34.).font_weight(700),
            if page.is_new() {
                El::from(
                    badge("New")
                        .rounded(Rounding::Full)
                        .background(ColorToken::Primary.alpha(14))
                        .color(ColorToken::Primary),
                )
            } else {
                spacer().width(0.).flex(0.).into()
            },
        ]
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .into(),
        text(page.summary())
            .font_size(17.)
            .color(ColorToken::Muted)
            .into(),
    ];
    for section in content.sections {
        let showing_code = app.code.contains(section.id);
        let id = section.id;
        let lines = section.code.lines().count() as f32;
        let switch = tab_bar([tab_item(false, "Preview"), tab_item(true, "Code")])
            .key(format!("{id}-view"))
            .selected(showing_code)
            .on_select(move |app: &mut App, code| {
                if code {
                    app.code.insert(id.to_string());
                } else {
                    app.code.remove(id);
                }
            })
            .height(30.)
            .font_size(12.);
        let body: El<App> = if showing_code {
            // Selectable, read-only source; Cmd/Ctrl+C copies a selection.
            text_area(section.code)
                .key(format!("{id}-code"))
                .read_only(true)
                .on_change(|_: &mut App, _| {})
                .fill_width()
                .height(lines * 19. + 34.)
                .font_family("SF Mono")
                .font_size(13.)
                .color("#E4E4E7")
                .background("#09090B")
                .border_color("#27272A")
                .rounded(10.)
                .into()
        } else {
            vstack![section.demo]
                .key(format!("{id}-preview"))
                .align(Align::Center)
                .padding(40., Edge::Vertical)
                .padding(24., Edge::Horizontal)
                .border_color(ColorToken::Border)
                .border_width(1.)
                .rounded(10.)
                .into()
        };
        parts.push(
            vstack![
                text(section.title).font_size(20.).font_weight(600),
                text(section.note).color(ColorToken::Muted),
                hstack![switch, spacer()],
                body,
            ]
            .key(format!("section-{id}"))
            .gap(10., Axis::Vertical)
            .padding(18., Edge::Top)
            .into(),
        );
    }
    if !content.api.is_empty() {
        let rows = content
            .api
            .iter()
            .enumerate()
            .map(|(i, (name, about))| table_row(i, [*name, *about]).key(format!("api-{i}")));
        parts.push(text("API reference").font_size(20.).font_weight(600).into());
        parts.push(
            table(
                [
                    table_column(0u8, "Method").width(340.).sortable(false),
                    table_column(1, "Description").width(400.).sortable(false),
                ],
                rows,
            )
            .key("api")
            .on_select(|_: &mut App, _| {})
            .font_size(13.)
            .into(),
        );
    }
    let index = PAGES.iter().position(|p| *p == page).unwrap_or(0);
    let prev = index.checked_sub(1).map(|i| PAGES[i]);
    let next = PAGES.get(index + 1).copied();
    let link = |target: Option<Page>, forward: bool| -> El<App> {
        match target {
            Some(target) => button(if forward {
                format!("{} ›", target.title())
            } else {
                format!("‹ {}", target.title())
            })
            .key(if forward { "page-next" } else { "page-prev" })
            .variant(ButtonVariant::Outline)
            .on_click(move |app: &mut App| app.go(target))
            .into(),
            None => spacer().width(0.).flex(0.).into(),
        }
    };
    parts.push(divider().background(ColorToken::Border).into());
    parts.push(hstack![link(prev, false), spacer(), link(next, true)].into());
    vstack(parts).gap(14., Axis::Vertical).into()
}

fn palette(app: &App) -> El<App> {
    let query = app.palette_query.trim().to_lowercase();
    let matches: Vec<Page> = PAGES
        .into_iter()
        .filter(|p| {
            query.is_empty()
                || p.title().to_lowercase().contains(&query)
                || p.summary().to_lowercase().contains(&query)
        })
        .take(8)
        .collect();
    let mut results: Vec<El<App>> = matches
        .iter()
        .map(|page| {
            let page = *page;
            button(page.title())
                .key(format!("palette-{}", page.slug()))
                .variant(ButtonVariant::Ghost)
                .label_align(Align::Start)
                .fill_width()
                .height(34.)
                .on_click(move |app: &mut App| app.go(page))
                .into()
        })
        .collect();
    if results.is_empty() {
        results.push(text("No results found.").color(ColorToken::Muted).into());
    }
    let first = matches.first().copied();
    dialog(
        vstack![
            text_field(&app.palette_query)
                .key("palette-query")
                .accessible_label("Find a component page")
                .placeholder("Type a component name…")
                .height(40.)
                .on_change(|app: &mut App, v| app.palette_query = v)
                .on_submit(move |app: &mut App| {
                    if let Some(page) = first {
                        app.go(page);
                    }
                }),
            text(if query.is_empty() { "Pages" } else { "Results" })
                .font_size(12.)
                .font_weight(600)
                .color(ColorToken::Muted),
            vstack(results).gap(2., Axis::Vertical),
        ]
        .gap(10., Axis::Vertical),
    )
    .key("palette")
    .open(app.palette_open)
    .width(480.)
    .rounded(12.)
    .background(ColorToken::Base100)
    .on_dismiss(|app: &mut App| {
        app.palette_open = false;
        app.palette_query.clear();
    })
    .into()
}

fn toast(app: &App) -> El<App> {
    let content: El<App> = match &app.toast {
        Some((id, message)) => hstack![
            icon(Icon::Check, 18., ColorToken::Success),
            text(message.clone()).font_weight(500).flex(1.),
            button("Dismiss")
                .key("toast-dismiss")
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Small)
                .on_click(|app: &mut App| app.toast = None),
        ]
        .key(format!("toast-{id}"))
        .transition(Transition::fade().duration_ms(220))
        .width(360.)
        .padding(14., Edge::All)
        .gap(10., Axis::Horizontal)
        .align(Align::Center)
        .background(ColorToken::Base100)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .rounded(12.)
        .shadow(Shadow::new().color("#0000001F").blur(20.).offset(0., 8.))
        .into(),
        None => spacer().width(0.).height(0.).into(),
    };
    vstack![content]
        .key("toast-layer")
        .flow(Flow::BottomRight)
        .padding(24., Edge::All)
        .scroll_passthrough(true)
        .into()
}
