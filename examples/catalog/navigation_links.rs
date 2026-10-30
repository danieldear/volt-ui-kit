//! Small, real navigation/link workflows: no platform side effects on mount.
use super::{pages::Section, App};
use volt_ui_kit::ui::prelude::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Workspace,
    Activity,
    Settings,
}
impl Route {
    fn path(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Activity => "workspace / activity",
            Self::Settings => "workspace / preferences",
        }
    }
}
pub struct State {
    pub history: NavigationHistory<Route>,
    pub captured: Option<ExternalUrl>,
    pub message: String,
}
impl Default for State {
    fn default() -> Self {
        Self {
            history: NavigationHistory::new(Route::Workspace),
            captured: None,
            message: "Links are intercepted here. Nothing opens until you choose Open in browser."
                .into(),
        }
    }
}
fn muted(value: impl Into<String>) -> volt_ui_kit::ui::Text<App> {
    text(value).font_size(13.).color(ColorToken::Muted)
}
fn destination(label: &str, route: Route, current: Route, key: &str) -> Element<App> {
    navigation_link(label, route)
        .key(key)
        .underline(false)
        .height(34.)
        .font_size(13.)
        .background(if route == current {
            ColorToken::Primary.alpha(12)
        } else {
            ColorToken::BaseContent.alpha(0)
        })
        .color(if route == current {
            ColorToken::Primary
        } else {
            ColorToken::Muted
        })
        .on_navigate(|app: &mut App, route| {
            app.routes.history.push(route);
        })
        .into()
}
pub fn history_section(app: &App) -> Section {
    let current = *app.routes.history.current();
    let content=navigation_stack(&app.routes.history,|route| {
        let (eyebrow, title, body)=match route {
            Route::Workspace => ("WORKSPACE", "A place for your next idea.", "Open Activity or Preferences, then use Back and Forward. Your route model stays ordinary Rust."),
            Route::Activity => ("ACTIVITY", "Everything, in context.", "This is a distinct mounted page. The workspace and preferences controls are not hidden under it."),
            Route::Settings => ("PREFERENCES", "Make the space your own.", "Routes can hold document IDs, filters or settings sections. The toolkit does not impose a router or URL format."),
        };
        vstack![
            text(eyebrow).font_size(11.).font_weight(600).color(ColorToken::Primary),
            text(title).font_size(24.).font_weight(600),
            muted(body),
            hstack![badge("Typed routes").font_size(11.), badge("App-owned state").font_size(11.)].gap(8.,Axis::Horizontal),
        ].key(format!("route-{}",current as u8)).gap(14.,Axis::Vertical).padding(22.,Edge::All).min_height(174.).into()
    }).key("route-current");
    let card = vstack![
        hstack![
            text("STUDIO / NAVIGATION")
                .font_size(11.)
                .font_weight(600)
                .color(ColorToken::Muted),
            spacer(),
            badge("LIVE").font_size(10.).color(ColorToken::Primary)
        ]
        .align(Align::Center)
        .padding(18., Edge::Horizontal)
        .height(46.),
        divider(),
        vstack![
            hstack![
                destination("Workspace", Route::Workspace, current, "route-workspace"),
                destination("Activity", Route::Activity, current, "route-activity"),
                destination("Preferences", Route::Settings, current, "route-settings")
            ]
            .gap(4., Axis::Horizontal)
            .fill_width(),
            text(current.path())
                .font_family("SF Mono")
                .font_size(11.)
                .color(ColorToken::Muted),
        ]
        .gap(12., Axis::Vertical)
        .padding(16., Edge::All),
        divider(),
        content,
        divider(),
        hstack![
            button("← Back")
                .key("route-back")
                .variant(ButtonVariant::Ghost)
                .height(32.)
                .font_size(12.)
                .enabled(app.routes.history.can_go_back())
                .on_click(|app: &mut App| {
                    app.routes.history.back();
                }),
            button("Forward →")
                .key("route-forward")
                .variant(ButtonVariant::Ghost)
                .height(32.)
                .font_size(12.)
                .enabled(app.routes.history.can_go_forward())
                .on_click(|app: &mut App| {
                    app.routes.history.forward();
                }),
            spacer(),
            text(format!(
                "{} / {}",
                app.routes.history.snapshot().index + 1,
                app.routes.history.entries().len()
            ))
            .font_family("SF Mono")
            .font_size(11.)
            .color(ColorToken::Muted),
        ]
        .align(Align::Center)
        .gap(4., Axis::Horizontal)
        .padding(12., Edge::All),
    ]
    .width_fraction(1.)
    .max_width(600.)
    .font_family("SF")
    .background(ColorToken::Base100)
    .border_width(1.)
    .border_color(ColorToken::Border)
    .rounded(12.)
    .clip(Clip::Rounded)
    .into();
    Section {
        id:"navigation-history", title:"A route, not a workaround", note:"Typed destinations with bounded Back/Forward history. Only the current route is mounted; your application owns persistent page data.", demo:card,
        code:r#"// App: history: NavigationHistory<Route>
navigation_link("Activity", Route::Activity)
    .key("activity")
    .on_navigate(|app: &mut App, route| { app.history.push(route); });

navigation_stack(&app.history, |route| match route {
    Route::Workspace => workspace_view(app),
    Route::Activity => activity_view(app),
}).key("current-page");
// app.history.back(); app.history.forward();
// app.history.snapshot(); app.history.restore(checkpoint)?;"#,
    }
}
fn capture(app: &mut App, url: ExternalUrl) {
    app.routes.message = format!("Captured {} — no browser was opened.", url.as_str());
    app.routes.captured = Some(url);
}
pub fn links_section(app: &App) -> Section {
    let card=vstack![
        hstack![text("CONNECTED, BY INTENT").font_size(11.).font_weight(600).color(ColorToken::Muted), spacer(), badge("HTTP / HTTPS").font_size(10.)].align(Align::Center),
        text("Links that behave like links.").font_size(24.).font_weight(600),
        muted("Tab to a link and press Enter, or click. A Rust callback decides what happens next."),
        vstack![
            link("Explore Volt UI on GitHub ↗", "https://github.com/danieldear/volt-ui-kit").key("link-repository").on_open(capture),
            link("Read the documentation ↗", "https://danieldear.github.io/volt-ui-kit/").key("link-documentation").on_open(capture),
            link("Unavailable resource", "https://example.com").key("link-disabled").enabled(false).on_open(capture),
        ].gap(4.,Axis::Vertical),
        divider(),
        muted(&app.routes.message).key("link-result"),
        button("Open in browser ↗").key("link-open-browser").variant(ButtonVariant::Outline).height(34.).font_size(12.)
            .enabled(app.routes.captured.is_some() && platform_links_available())
            .on_click(|app: &mut App| {
                if let Some(url)=&app.routes.captured { app.routes.message=match platform_open_url(url) {
                    Ok(()) => "The operating system accepted the browser request.".into(),
                    Err(error) => error.to_string(),
                }; }
            }),
        muted(if platform_links_available() { "Browser dispatch is explicit. No automatic launch, permission prompt or network request on mount." } else { "Browser dispatch is unavailable in this build. Link callbacks and keyboard interaction still work." }).font_size(11.),
    ].width_fraction(1.).max_width(600.)
    .font_family("SF").gap(16.,Axis::Vertical).padding(24.,Edge::All).background(ColorToken::Base100).border_width(1.).border_color(ColorToken::Border).rounded(12.).into();
    Section { id:"navigation-links", title:"External links, without surprises", note:"Theme-aware, keyboard-focusable links with native accessibility semantics. This demo intercepts activation first; browser dispatch is a separate, explicit action.", demo:card,
    code:r#"link("Documentation ↗", "https://example.com/docs")
    .key("documentation")
    .on_open(|app: &mut App, url| {
        // Your policy: capture, confirm, or explicitly dispatch.
        app.status = match volt_ui_kit::native::links::open_url(&url) {
            Ok(()) => "Browser request accepted".into(),
            Err(error) => error.to_string(),
        };
    });
// HTTP(S) only. No credentials, file URLs or executable schemes."# }
}

fn platform_links_available() -> bool {
    #[cfg(feature = "native")]
    {
        volt_ui_kit::native::links::available()
    }
    #[cfg(not(feature = "native"))]
    {
        false
    }
}
fn platform_open_url(url: &ExternalUrl) -> Result<(), String> {
    #[cfg(feature = "native")]
    {
        volt_ui_kit::native::links::open_url(url).map_err(|e| e.to_string())
    }
    #[cfg(not(feature = "native"))]
    {
        let _ = url;
        Err("Native browser dispatch is unavailable in a web preview".into())
    }
}
