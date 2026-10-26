#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
#[derive(Clone, Debug, PartialEq, Eq)]
enum Page {
    Home,
    Inbox,
    Settings,
}
#[test]
fn history_branches_bounds_and_replace() {
    let mut history = NavigationHistory::with_capacity(Page::Home, 3).unwrap();
    assert!(!history.back());
    assert!(!history.push(Page::Home));
    assert!(history.push(Page::Inbox));
    assert!(history.push(Page::Settings));
    assert!(history.back());
    assert_eq!(history.current(), &Page::Inbox);
    assert!(history.can_go_forward());
    assert!(history.push(Page::Home));
    assert!(!history.can_go_forward());
    history.push(Page::Settings);
    assert_eq!(
        history.snapshot().entries,
        [Page::Inbox, Page::Home, Page::Settings]
    );
    assert!(history.replace(Page::Inbox));
    assert_eq!(history.current(), &Page::Inbox);
    history.reset(Page::Home);
    assert!(!history.can_go_back());
    assert!(NavigationHistory::with_capacity(Page::Home, 0).is_err());
    assert!(NavigationHistory::with_capacity(Page::Home, 1025).is_err());
}
#[test]
fn history_restore_is_validated_and_atomic() {
    let mut history = NavigationHistory::with_capacity(Page::Home, 2).unwrap();
    history.push(Page::Inbox);
    let before = history.snapshot();
    for invalid in [
        NavigationSnapshot {
            entries: vec![],
            index: 0,
        },
        NavigationSnapshot {
            entries: vec![Page::Home],
            index: 1,
        },
        NavigationSnapshot {
            entries: vec![Page::Home, Page::Inbox, Page::Settings],
            index: 0,
        },
    ] {
        assert!(history.restore(invalid).is_err());
        assert_eq!(history.snapshot(), before);
    }
    history.back();
    assert!(history.forward());
    history
        .restore(NavigationSnapshot {
            entries: vec![Page::Settings],
            index: 0,
        })
        .unwrap();
    assert_eq!(history.current(), &Page::Settings);
}
#[test]
fn web_urls_are_conservative_and_never_commands() {
    for valid in [
        "https://example.com",
        "HTTPS://example.com/path?q=ok%20yes#anchor",
        "http://localhost:8080/",
        "https://[::1]:443/",
    ] {
        assert!(ExternalUrl::parse(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "data:text/html,test",
        "mailto:a@b.test",
        "https://",
        "https://user:pass@example.com",
        "https://a\\b/",
        "https://example.com/\n",
        "https://example.com/%0a",
        "https://example.com/%",
        "https://example.com:65536",
        "https://example.com:bad",
        "https://[invalid]",
        "https://a..b",
        "https://example.com/ has space",
        "https://example.com/<script>",
    ] {
        assert!(ExternalUrl::parse(invalid).is_err(), "{invalid}");
    }
    assert!(ExternalUrl::parse(format!("https://example.com/{}", "a".repeat(4096))).is_err());
}
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, max: f32) -> [f32; 2] {
        [(text.len() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
struct App {
    history: NavigationHistory<Page>,
    opened: Vec<String>,
}
fn app() -> App {
    App {
        history: NavigationHistory::new(Page::Home),
        opened: vec![],
    }
}
fn mount(ui: Element<App>) -> (Runtime, Handlers<'static, App>) {
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(600., 400., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    (r, h)
}
fn links(disabled: bool) -> Element<App> {
    vstack![
        navigation_link("Inbox", Page::Inbox)
            .key("inbox")
            .on_navigate(|a: &mut App, route| {
                a.history.push(route);
            }),
        link("Documentation", "https://example.com/docs")
            .key("docs")
            .on_open(|a: &mut App, url| a.opened.push(url.as_str().into())),
        link("Disabled", "https://example.com")
            .key("disabled")
            .enabled(!disabled)
            .on_open(|a: &mut App, url| a.opened.push(url.as_str().into())),
    ]
    .into()
}
#[test]
fn links_focus_activate_and_disabled_guard_without_build_side_effects() {
    let (mut r, mut h) = mount(links(true));
    let mut a = app();
    assert!(a.opened.is_empty());
    assert_eq!(a.history.current(), &Page::Home);
    assert_eq!(r.accessibility_links().len(), 3);
    r.focus_next(false);
    assert_eq!(r.focused(), Some("inbox"));
    assert!(
        r.accessibility_activate("inbox", &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(a.history.current(), &Page::Inbox);
    r.focus_next(false);
    assert_eq!(r.focused(), Some("docs"));
    assert!(
        r.accessibility_activate("docs", &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(a.opened, ["https://example.com/docs"]);
    assert!(
        !r.accessibility_activate("disabled", &mut h, &mut a)
            .unwrap()
            .activated
    );
    r.focus_next(false);
    assert_eq!(r.focused(), Some("inbox"));
}
#[test]
fn links_require_key_handler_label_and_safe_target() {
    let invalid: Vec<Element<App>> = vec![
        link("Docs", "https://example.com").key("docs").into(),
        link("Docs", "https://example.com")
            .on_open(|_: &mut App, _| {})
            .into(),
        link("", "https://example.com")
            .key("docs")
            .on_open(|_: &mut App, _| {})
            .into(),
        link("Bad", "file:///tmp")
            .key("docs")
            .on_open(|_: &mut App, _| {})
            .into(),
        navigation_link("Inbox", Page::Inbox).key("inbox").into(),
    ];
    for ui in invalid {
        assert!(ui.build().is_err());
    }
}
#[test]
fn navigation_stack_mounts_only_current_route_and_removed_links_cannot_activate() {
    let mut a = app();
    let view = |history: &NavigationHistory<Page>| -> Element<App> {
        navigation_stack(history, |route| match route {
            Page::Home => links(false),
            _ => text("Other page").key("other").into(),
        })
        .key("stack")
        .into()
    };
    let (mut r, _) = mount(view(&a.history));
    assert_eq!(r.accessibility_links().len(), 3);
    a.history.push(Page::Settings);
    let (doc, next) = view(&a.history).build().unwrap().into_parts();
    r.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    let mut h = next;
    r.layout(
        Viewport::new(600., 400., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    assert!(r.accessibility_links().is_empty());
    assert!(
        !r.accessibility_activate("docs", &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(a.opened.is_empty());
}

#[test]
fn keyboard_press_release_and_pointer_activation_use_the_same_handler() {
    let (mut r, mut h) = mount(links(true));
    let mut a = app();
    r.focus_next(false);
    r.focus_next(false);
    assert_eq!(r.focused(), Some("docs"));
    assert!(r.key_down());
    assert!(a.opened.is_empty());
    assert!(r.key_up_outcome(&mut h, &mut a).unwrap().activated);
    assert_eq!(a.opened.len(), 1);
    let [x, y, w, height] = r
        .boxes()
        .iter()
        .find(|b| b.id == "docs")
        .unwrap()
        .bounds
        .bounds();
    r.pointer_down(x + w / 2., y + height / 2.);
    assert!(
        r.pointer_up_outcome(x + w / 2., y + height / 2., &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(a.opened.len(), 2);
}
#[test]
fn modal_scope_hides_and_blocks_background_links() {
    let ui = vstack![
        links(false),
        dialog(button("Close").key("close").on_click(|_: &mut App| {}))
            .key("modal")
            .open(true)
            .on_dismiss(|_: &mut App| {})
    ];
    let (mut r, mut h) = mount(ui.into());
    let mut a = app();
    assert!(r.accessibility_links().is_empty());
    assert!(
        !r.accessibility_activate("docs", &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(a.opened.is_empty());
}
#[test]
fn history_capacity_one_and_snapshot_round_trip() {
    let mut history = NavigationHistory::with_capacity("home".to_owned(), 1).unwrap();
    history.push("inbox".to_owned());
    assert!(!history.can_go_back());
    let json = serde_json::to_string(&history.snapshot()).unwrap();
    let checkpoint = serde_json::from_str(&json).unwrap();
    let mut restored = NavigationHistory::new("root".to_owned());
    restored.restore(checkpoint).unwrap();
    assert_eq!(restored.current(), "inbox");
}

impl volt_ui_kit::Painter for Metrics {
    type Shape = ([f32; 2], [f32; 2], f32, [f32; 4]);
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(600., 400., 1.).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<Self::Shape>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<Self::Shape>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(&mut self, _: &mut Vec<()>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<()>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
    fn card_line(
        &self,
        out: &mut Vec<Self::Shape>,
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: [f32; 4],
    ) {
        out.push((a, b, width, color));
    }
}
impl volt_ui_kit::declarative::DocumentPainter for Metrics {
    fn ui_text(&mut self, _: &mut Vec<()>, _: &str, _: [f32; 4], _: f32, _: [f32; 4]) {}
    fn ui_text_lines(
        &mut self,
        text: &str,
        bounds: [f32; 4],
        style: &volt_ui_kit::declarative::TextStyle,
    ) -> Vec<[f32; 3]> {
        vec![[
            bounds[0],
            bounds[1] + style.size,
            (text.len() as f32 * style.size * 0.5).min(bounds[2]),
        ]]
    }
}
#[test]
fn link_underline_is_painted_at_label_not_across_the_hit_target() {
    for (underline, strike, count) in [(true, false, 1), (false, false, 0), (true, true, 2)] {
        let (r, _) = mount(
            link("Docs", "https://example.com")
                .key("link")
                .width(300.)
                .underline(underline)
                .strikethrough(strike)
                .on_open(|_: &mut App, _| {})
                .into(),
        );
        let mut p = Metrics;
        let frame = r.paint(&mut p, &volt_ui_kit::Theme::dark());
        let lines: Vec<_> = frame.into_iter().flat_map(|batch| batch.shapes).collect();
        assert_eq!(lines.len(), count);
        for (a, b, width, _) in lines {
            assert!((a[0] - 10.).abs() < 0.01);
            assert!((b[0] - a[0] - 28.).abs() < 0.01);
            assert_eq!(width, 1.);
        }
    }
}

#[test]
fn revisiting_current_route_preserves_forward_and_replace_does_not_prune() {
    let mut history = NavigationHistory::new(Page::Home);
    history.push(Page::Inbox);
    history.push(Page::Settings);
    history.back();
    assert!(!history.push(Page::Inbox));
    assert!(history.can_go_forward());
    assert!(history.replace(Page::Home));
    assert!(history.forward());
    assert_eq!(history.current(), &Page::Settings);
    assert_eq!(history.index(), 2);
    assert_eq!(history.entries().len(), 3);
    let default = NavigationHistory::<String>::default();
    assert_eq!(default.current(), "");
}
