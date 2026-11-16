use crate::{protocol::SystemColors, PlatformEvent};
use serde_json::{json, Value};
#[cfg(any(target_os = "ios", target_os = "android"))]
use volt_ui_kit::rendering::{PresentStatus, SurfaceRenderer};
use volt_ui_kit::{mobile::MobileView, ui::Element, Palette, ThemeMode, UiTheme};

/// Platform resources supplied once by a host. No UIKit/Android types leak in.
#[derive(Clone, Debug)]
pub struct HostContext {
    pub font_family: String,
}
/// A Rust-owned mobile application. Rendering policy and themes belong to the app.
type BackNavigation<C> = (fn(&C) -> bool, fn(&mut C));
pub struct Application<C: 'static> {
    ui: MobileView<C>,
    mode: ThemeMode,
    /// The app's own theme; `None` follows the platform's system colours.
    theme: Option<fn(&C, ThemeMode) -> UiTheme>,
    /// Adjusts the platform's colours, e.g. a brand accent on some pages.
    from_system: Option<fn(&C, ThemeMode, Palette) -> UiTheme>,
    system: Option<SystemColors>,
    appearance: Option<fn(&mut C, ThemeMode)>,
    suspended: bool,
    allow_software: bool,
    scale: f32,
    back: Option<BackNavigation<C>>,
}
fn fallback_theme(mode: ThemeMode) -> UiTheme {
    UiTheme::from_palette(if mode == ThemeMode::Dark {
        Palette::zinc_dark()
    } else {
        Palette::zinc_light()
    })
}
/// The platform's own colours as a Volt palette: grouped backgrounds, label
/// colours, hairline separators and the app tint.
pub fn system_palette(c: &SystemColors, mode: ThemeMode) -> Palette {
    let white = [1., 1., 1., 1.];
    let mut palette = fallback_theme(mode).palette;
    palette.name = "System".into();
    palette.base = [c.background, c.surface, c.raised];
    palette.content = c.label;
    palette.muted = c.secondary_label;
    palette.border = c.separator;
    palette.primary = c.tint;
    palette.on_primary = white;
    palette.accent = c.tint;
    palette.on_accent = white;
    palette.info = c.tint;
    palette.on_info = white;
    palette.success = c.success;
    palette.on_success = white;
    palette.warning = c.warning;
    palette.on_warning = white;
    palette.error = c.error;
    palette.on_error = white;
    palette
}
impl<C: 'static> Application<C> {
    pub fn new(host: HostContext, state: C, view: fn(&C) -> Element<C>) -> Result<Self, String> {
        Ok(Self {
            ui: MobileView::new_with_font(state, view, host.font_family)?,
            mode: ThemeMode::Light,
            theme: None,
            from_system: None,
            system: None,
            appearance: None,
            suspended: false,
            allow_software: false,
            scale: 1.,
            back: None,
        })
    }
    /// Replace the platform's system colours with the app's own theme.
    pub fn theme_with(mut self, theme: fn(&C, ThemeMode) -> UiTheme) -> Self {
        self.theme = Some(theme);
        self
    }
    /// Start from the platform's colours and adjust them: a brand accent,
    /// or a different palette on some pages. The function receives the system
    /// palette (or a neutral one before the host reports it).
    pub fn theme_from_system(mut self, theme: fn(&C, ThemeMode, Palette) -> UiTheme) -> Self {
        self.from_system = Some(theme);
        self
    }
    fn current_theme(&self) -> UiTheme {
        if let Some(theme) = self.theme {
            return theme(self.ui.context(), self.mode);
        }
        let system = match &self.system {
            Some(colors) => UiTheme::from_palette(system_palette(colors, self.mode)),
            None => fallback_theme(self.mode),
        };
        match self.from_system {
            Some(adjust) => adjust(self.ui.context(), self.mode, system.palette),
            None => system,
        }
    }
    /// Whether the host should keep drawing frames.
    pub fn is_animating(&self) -> bool {
        !self.suspended && self.ui.is_animating()
    }
    /// Optional state observer, e.g. for custom literal light/dark styles.
    pub fn on_appearance(mut self, observer: fn(&mut C, ThemeMode)) -> Self {
        self.appearance = Some(observer);
        self
    }
    /// Explicitly allow software drawing + GPU presentation on restricted adapters
    /// such as iOS Simulator. The drawing mode is always reported to diagnostics.
    pub fn allow_software_fallback(mut self, allow: bool) -> Self {
        self.allow_software = allow;
        self
    }
    /// Optional platform back navigation; Android falls back to Activity back otherwise.
    pub fn back_with(mut self, can_go_back: fn(&C) -> bool, action: fn(&mut C)) -> Self {
        self.back = Some((can_go_back, action));
        self
    }
    pub fn state(&self) -> &C {
        self.ui.context()
    }
    pub fn mobile_view(&self) -> &MobileView<C> {
        &self.ui
    }
    pub fn handle(&mut self, event: PlatformEvent) -> Result<(), String> {
        match event {
            PlatformEvent::Resize {
                width,
                height,
                scale,
            } => {
                self.ui.resize(width, height, scale)?;
                self.scale = scale;
            }
            PlatformEvent::Begin { x, y } => self.ui.touch_begin([x, y])?,
            PlatformEvent::Move { x, y } => self.ui.touch_move([x, y])?,
            PlatformEvent::End { x, y } => self.ui.touch_end([x, y])?,
            PlatformEvent::Cancel {} => self.ui.cancel_touch(),
            PlatformEvent::Activate { id } => {
                self.ui.activate(&id)?;
            }
            PlatformEvent::Focus { id } => {
                self.ui.focus_text(&id)?;
            }
            PlatformEvent::Text { id, text } => {
                if self
                    .ui
                    .accessibility_nodes()
                    .iter()
                    .any(|n| n.id == id && n.password)
                {
                    return Err(
                        "native secure-field editing is not implemented; existing value preserved"
                            .into(),
                    );
                }
                self.ui.set_text(&id, &text)?;
            }
            PlatformEvent::Scroll { id, dy } => {
                self.ui.scroll(&id, [0., dy])?;
            }
            PlatformEvent::ScrollTo { id, x, y } => {
                self.ui.scroll_to(&id, [x, y])?;
            }
            PlatformEvent::Host {
                native_scrolling,
                native_navigation,
                native_controls,
            } => {
                self.ui.set_native_scrolling(native_scrolling)?;
                self.ui.set_native_navigation(native_navigation)?;
                self.ui.set_native_controls(native_controls);
            }
            PlatformEvent::Value { id, value } => {
                self.ui.set_value(&id, value)?;
            }
            PlatformEvent::ScrollInsets { id, top, bottom } => {
                self.ui.set_scroll_insets(&id, [top, 0., bottom, 0.])?;
            }
            PlatformEvent::Navigation { id } => {
                self.ui.activate_navigation(&id)?;
            }
            PlatformEvent::Appearance {
                dark,
                colors,
                reduce_motion,
            } => {
                self.mode = if dark {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                };
                if colors.is_some() {
                    self.system = colors;
                }
                if let Some(reduced) = reduce_motion {
                    self.ui.set_reduced_motion(reduced);
                }
                if let Some(observer) = self.appearance {
                    let mode = self.mode;
                    self.ui.update(|s| observer(s, mode))?;
                }
            }
            PlatformEvent::Suspend { value } => {
                self.suspended = value;
                self.ui.set_suspended(value);
            }
            PlatformEvent::Back {} => {
                if let Some((can, action)) = self.back {
                    if !self.suspended && can(self.ui.context()) {
                        self.ui.update(action)?;
                    }
                }
            }
            PlatformEvent::Draw {} => {}
        }
        Ok(())
    }
    pub fn background(&self) -> [f32; 4] {
        self.current_theme().palette.base[0]
    }
    /// Platform-neutral semantics used by native accessibility/input adapters.
    pub fn snapshot(&self) -> Value {
        let clips: std::collections::HashMap<_, _> = self
            .ui
            .runtime()
            .boxes()
            .iter()
            .map(|b| (b.id.as_str(), b.clip.bounds()))
            .collect();
        let nodes=self.ui.accessibility_nodes().into_iter().map(|n| {
            let clip=clips.get(n.id.as_str()).copied().unwrap_or([0.;4]);
            let [x,y,w,h]=n.bounds;
            let left=x.max(clip[0]);let top=y.max(clip[1]);
            let visible=[left,top,((x+w).min(clip[0]+clip[2])-left).max(0.),((y+h).min(clip[1]+clip[3])-top).max(0.)];
            use volt_ui_kit::declarative::AccessRole;
            let multiline=n.role==AccessRole::MultilineTextInput;
            let kind=match n.role {AccessRole::TextInput|AccessRole::MultilineTextInput=>"field",AccessRole::ScrollView=>"scroll",AccessRole::Switch=>"switch",AccessRole::Slider=>"slider",AccessRole::Button=>"button",_ if n.clickable=>"button",_=>"label"};
            json!({"id":n.id,"label":n.label,"value":n.value,"enabled":n.enabled,"kind":kind,"checked":n.checked,"numeric":n.numeric,"multiline":multiline,
                "password":n.password,"readOnly":n.read_only || n.password,"bounds":n.bounds.map(|v|v/self.scale),"visibleBounds":visible.map(|v|v/self.scale)})
        }).collect::<Vec<_>>();
        // Scroll views for a platform scroll view to drive: the visible frame,
        // the content size and offset, in points.
        let scroll_views = self
            .ui
            .runtime()
            .boxes()
            .iter()
            .filter_map(|b| {
                let axes = b.scroll?.axes();
                let [x, y, w, h] = b.bounds.bounds().map(|v| v / self.scale);
                let [cx, cy, cw, ch] = b.clip.bounds().map(|v| v / self.scale);
                let (left, top) = (x.max(cx), y.max(cy));
                let frame = [
                    left,
                    top,
                    ((x + w).min(cx + cw) - left).max(0.),
                    ((y + h).min(cy + ch) - top).max(0.),
                ];
                // The content alone; the host adds its own insets.
                let [it, ir, ib, il] = self.ui.runtime().scroll_insets(&b.id);
                let content = [
                    (w + b.scroll_max[0] - if axes[0] { il + ir } else { 0. }).max(0.),
                    (h + b.scroll_max[1] - if axes[1] { it + ib } else { 0. }).max(0.),
                ];
                Some(
                    json!({"id": b.id, "frame": frame, "content": content, "insets": [it, ib],
                    "offset": b.scroll_offset, "axes": axes}),
                )
            })
            .collect::<Vec<_>>();
        // The outermost navigation bar, for a host that draws its own.
        let navigation = self.ui.runtime().navigation_bars().into_iter().next().map(|(id, bar)| {
            json!({"id": id, "title": bar.title, "largeTitle": bar.large_title, "depth": bar.depth,
                "back": bar.back, "buttons": bar.buttons.iter().map(|(id, label)| json!({"id": id, "label": label})).collect::<Vec<_>>()})
        });
        let theme = self.current_theme();
        json!({"navigation": navigation, "scrollViews": scroll_views, "canGoBack":self.back.is_some_and(|(can,_)|can(self.ui.context())),"animating":self.is_animating(),"nodes":nodes,"dark":self.mode==ThemeMode::Dark,"background":theme.palette.base[0],
            "fieldBackground":theme.palette.base[1],"foreground":theme.palette.content,"accent":theme.palette.primary})
    }
}
#[cfg(any(target_os = "ios", target_os = "android"))]
pub(crate) trait HostedApplication {
    fn handle(&mut self, event: PlatformEvent) -> Result<(), String>;
    fn draw(&mut self, renderer: &mut SurfaceRenderer) -> Result<PresentStatus, String>;
    fn snapshot(&self) -> Value;
    fn suspended(&self) -> bool;
    fn allow_software(&self) -> bool;
}
#[cfg(any(target_os = "ios", target_os = "android"))]
impl<C: 'static> HostedApplication for Application<C> {
    fn handle(&mut self, e: PlatformEvent) -> Result<(), String> {
        self.handle(e)
    }
    fn draw(&mut self, r: &mut SurfaceRenderer) -> Result<PresentStatus, String> {
        let theme = self.current_theme();
        self.ui.present(r, &theme)
    }
    fn snapshot(&self) -> Value {
        self.snapshot()
    }
    fn suspended(&self) -> bool {
        self.suspended
    }
    fn allow_software(&self) -> bool {
        self.allow_software
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit::ui::prelude::*;
    #[derive(Default)]
    struct State {
        clicks: usize,
        name: String,
        dark: bool,
    }
    fn view(s: &State) -> Element<State> {
        vstack![
            button(format!("Count {}", s.clicks))
                .key("count")
                .on_click(|s: &mut State| s.clicks += 1),
            text_field(&s.name)
                .key("name")
                .accessible_label("Name")
                .on_change(|s: &mut State, v| s.name = v),
            text_field("keep secret")
                .key("secret")
                .password(true)
                .on_change(|_: &mut State, _| {}),
            text_field("locked")
                .key("locked")
                .read_only(true)
                .on_change(|_: &mut State, _| {}),
            vstack(
                (0..30)
                    .map(|i| text(format!("Item {i}"))
                        .key(format!("item-{i}"))
                        .height(40.)
                        .into())
                    .collect::<Vec<_>>()
            )
            .key("list")
            .flex(1.)
            .scroll(Scroll::Vertical),
        ]
        .into()
    }
    fn app() -> Application<State> {
        Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            State::default(),
            view,
        )
        .unwrap()
    }
    #[test]
    fn typed_callbacks_unicode_native_values_and_suspension_share_one_state() {
        let mut a = app();
        a.handle(PlatformEvent::Activate { id: "count".into() })
            .unwrap();
        a.handle(PlatformEvent::Text {
            id: "name".into(),
            text: "Zoë 東京".into(),
        })
        .unwrap();
        assert_eq!(a.state().clicks, 1);
        assert_eq!(a.state().name, "Zoë 東京");
        a.handle(PlatformEvent::Suspend { value: true }).unwrap();
        a.handle(PlatformEvent::Activate { id: "count".into() })
            .unwrap();
        a.handle(PlatformEvent::Text {
            id: "name".into(),
            text: "ignored".into(),
        })
        .unwrap();
        assert_eq!(a.state().clicks, 1);
        assert_eq!(a.state().name, "Zoë 東京");
        a.handle(PlatformEvent::Suspend { value: false }).unwrap();
        a.handle(PlatformEvent::Activate { id: "count".into() })
            .unwrap();
        assert_eq!(a.state().clicks, 2);
    }
    #[test]
    fn host_font_survives_recomposition_and_app_override_wins() {
        let mut a = app();
        a.handle(PlatformEvent::Activate { id: "count".into() })
            .unwrap();
        assert_eq!(
            a.mobile_view()
                .runtime()
                .boxes()
                .iter()
                .find(|b| b.id == "count")
                .unwrap()
                .text_style
                .family,
            "serif"
        );
        fn own(s: &State) -> Element<State> {
            view(s).font_family("monospace")
        }
        let a = Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            State::default(),
            own,
        )
        .unwrap();
        assert_eq!(
            a.mobile_view()
                .runtime()
                .boxes()
                .iter()
                .find(|b| b.id == "count")
                .unwrap()
                .text_style
                .family,
            "monospace"
        );
    }
    #[test]
    fn system_appearance_and_custom_theme_are_app_controlled() {
        let mut a = app()
            .on_appearance(|s, m| s.dark = m == ThemeMode::Dark)
            .theme_with(|_, _| {
                let mut t = UiTheme::light();
                t.palette.base[0] = [0.1, 0.2, 0.3, 1.];
                t
            });
        a.handle(PlatformEvent::Appearance {
            dark: true,
            colors: Some(colors()),
            reduce_motion: None,
        })
        .unwrap();
        assert!(a.state().dark);
        // The app's own theme wins over the system colours.
        assert_eq!(a.background(), [0.1, 0.2, 0.3, 1.]);
        assert_eq!(a.snapshot()["dark"], true);
    }
    fn colors() -> SystemColors {
        let gray = |v: f32| [v, v, v, 1.];
        SystemColors {
            background: gray(0.),
            surface: gray(0.11),
            raised: gray(0.17),
            label: gray(1.),
            secondary_label: [0.92, 0.92, 0.96, 0.6],
            separator: [0.33, 0.33, 0.35, 0.6],
            tint: [0.04, 0.52, 1., 1.],
            success: [0.19, 0.82, 0.35, 1.],
            warning: [1., 0.62, 0.04, 1.],
            error: [1., 0.27, 0.23, 1.],
        }
    }
    #[test]
    fn an_app_can_adjust_the_system_colours() {
        let mut a = app().theme_from_system(|s: &State, _, mut palette| {
            if s.clicks > 0 {
                palette.primary = [0.05, 0.49, 0.4, 1.];
            }
            UiTheme::from_palette(palette)
        });
        a.handle(PlatformEvent::Appearance {
            dark: false,
            colors: Some(colors()),
            reduce_motion: None,
        })
        .unwrap();
        assert_eq!(a.snapshot()["accent"][2], 1.);
        a.handle(PlatformEvent::Activate { id: "count".into() })
            .unwrap();
        assert_eq!(a.snapshot()["accent"][1], 0.49f32);
        // The system's surfaces stay.
        assert_eq!(a.background(), [0., 0., 0., 1.]);
    }
    #[test]
    fn without_an_app_theme_the_system_colours_apply() {
        let mut a = app();
        assert_eq!(a.background(), Palette::zinc_light().base[0]);
        a.handle(PlatformEvent::Appearance {
            dark: true,
            colors: Some(colors()),
            reduce_motion: Some(false),
        })
        .unwrap();
        assert_eq!(a.background(), [0., 0., 0., 1.]);
        let snapshot = a.snapshot();
        assert_eq!(snapshot["accent"][2], 1.);
        assert_eq!(snapshot["fieldBackground"][0], 0.11f32);
        // An appearance change without colours keeps the last ones.
        a.handle(PlatformEvent::Appearance {
            dark: false,
            colors: None,
            reduce_motion: None,
        })
        .unwrap();
        assert_eq!(a.background(), [0., 0., 0., 1.]);
        assert_eq!(snapshot["animating"], false);
    }
    #[test]
    fn invalid_resize_is_atomic_and_scrolling_clips_semantics() {
        let mut a = app();
        a.handle(PlatformEvent::Resize {
            width: 390.,
            height: 500.,
            scale: 3.,
        })
        .unwrap();
        let before = a.snapshot();
        assert!(a
            .handle(PlatformEvent::Resize {
                width: 390.,
                height: 500.,
                scale: 0.1
            })
            .is_err());
        assert_eq!(a.snapshot(), before);
        a.handle(PlatformEvent::Scroll {
            id: "list".into(),
            dy: 16384.,
        })
        .unwrap();
        let snapshot = a.snapshot();
        let old = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "item-0")
            .unwrap();
        assert_eq!(old["visibleBounds"][3], 0.);
        let last = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "item-29")
            .unwrap();
        assert!(last["visibleBounds"][3].as_f64().unwrap() > 0.);
    }
    #[test]
    fn a_native_scroll_view_owns_scrolling_and_can_bounce() {
        let mut a = app();
        a.handle(PlatformEvent::Resize {
            width: 390.,
            height: 500.,
            scale: 3.,
        })
        .unwrap();
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: false,
            native_controls: false,
        })
        .unwrap();
        let item = |a: &Application<State>| {
            a.snapshot()["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == "item-5")
                .unwrap()["bounds"][1]
                .as_f64()
                .unwrap()
        };
        let list = a.snapshot()["scrollViews"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == "list")
            .unwrap()
            .clone();
        assert_eq!(list["axes"], json!([false, true]));
        let height = list["frame"][3].as_f64().unwrap();
        assert!(list["content"][1].as_f64().unwrap() > height);
        let y = item(&a);
        // A drag no longer scrolls: the platform scroll view does.
        a.handle(PlatformEvent::Begin { x: 100., y: 400. }).unwrap();
        a.handle(PlatformEvent::Move { x: 100., y: 300. }).unwrap();
        a.handle(PlatformEvent::Cancel {}).unwrap();
        assert_eq!(item(&a), y);
        a.handle(PlatformEvent::ScrollTo {
            id: "list".into(),
            x: 0.,
            y: 100.,
        })
        .unwrap();
        assert!((item(&a) - (y - 100.)).abs() < 0.01);
        // Pulled 30 points past the top: content sits 30 points lower,
        // while the stored offset stays at the start.
        a.handle(PlatformEvent::ScrollTo {
            id: "list".into(),
            x: 0.,
            y: -30.,
        })
        .unwrap();
        assert!((item(&a) - (y + 30.)).abs() < 0.01);
        assert_eq!(a.mobile_view().runtime().scroll_offset("list"), [0., 0.]);
        a.handle(PlatformEvent::ScrollTo {
            id: "list".into(),
            x: 0.,
            y: 0.,
        })
        .unwrap();
        assert!((item(&a) - y).abs() < 0.01);
        // A bar above and the home indicator below: content starts 100
        // points lower and scrolls 134 points further.
        let max = |a: &Application<State>| {
            a.mobile_view()
                .runtime()
                .boxes()
                .iter()
                .find(|b| b.id == "list")
                .unwrap()
                .scroll_max[1]
        };
        let before = max(&a);
        a.handle(PlatformEvent::ScrollInsets {
            id: "list".into(),
            top: 100.,
            bottom: 34.,
        })
        .unwrap();
        assert!((item(&a) - (y + 100.)).abs() < 0.01);
        assert!((max(&a) - before - 134.).abs() < 0.01);
        let content = |a: &Application<State>| {
            a.snapshot()["scrollViews"][0]["content"][1]
                .as_f64()
                .unwrap()
        };
        assert!((content(&a) - list["content"][1].as_f64().unwrap()).abs() < 0.01);
        a.handle(PlatformEvent::ScrollTo {
            id: "list".into(),
            x: 0.,
            y: 100.,
        })
        .unwrap();
        assert!((item(&a) - y).abs() < 0.01);
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Route {
        Home,
        Detail,
    }
    struct Pages {
        history: NavigationHistory<Route>,
        edits: usize,
    }
    fn pages(p: &Pages) -> Element<Pages> {
        navigation_stack(&p.history, |route| match route {
            Route::Home => vstack![button("Open").key("open").on_click(|p: &mut Pages| {
                p.history.push(Route::Detail);
            })]
            .navigation_title("Components")
            .large_title(true)
            .into(),
            Route::Detail => vstack![text("Detail").key("detail")]
                .navigation_title("Buttons")
                .bar_button("Edit", |p: &mut Pages| p.edits += 1)
                .into(),
        })
        .key("nav")
        .on_back(|p: &mut Pages| {
            p.history.back();
        })
        .into()
    }
    #[test]
    fn a_native_navigation_bar_is_described_and_runs_its_controls() {
        let mut a = Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            Pages {
                history: NavigationHistory::new(Route::Home),
                edits: 0,
            },
            pages,
        )
        .unwrap();
        let ids = |a: &Application<Pages>| -> Vec<String> {
            a.snapshot()["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n["id"].as_str().unwrap().to_owned())
                .collect()
        };
        // Volt draws the bar until the host says it draws its own.
        assert!(ids(&a).iter().any(|id| id == "nav-large-title"));
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        assert!(!ids(&a).iter().any(|id| id.starts_with("nav-")));
        let bar = a.snapshot()["navigation"].clone();
        assert_eq!(bar["title"], "Components");
        assert_eq!(bar["largeTitle"], true);
        assert_eq!(bar["depth"], 0);
        assert!(bar["back"].is_null());
        a.handle(PlatformEvent::Activate { id: "open".into() })
            .unwrap();
        let bar = a.snapshot()["navigation"].clone();
        assert_eq!(bar["title"], "Buttons");
        assert_eq!(bar["depth"], 1);
        assert_eq!(bar["back"], "nav-back");
        assert_eq!(bar["buttons"][0]["label"], "Edit");
        let edit = bar["buttons"][0]["id"].as_str().unwrap().to_owned();
        a.handle(PlatformEvent::Navigation { id: edit }).unwrap();
        assert_eq!(a.state().edits, 1);
        // Only navigation controls run this way.
        a.handle(PlatformEvent::Navigation {
            id: "detail".into(),
        })
        .unwrap();
        a.handle(PlatformEvent::Navigation {
            id: "nav-back".into(),
        })
        .unwrap();
        assert!(matches!(a.state().history.current(), Route::Home));
        assert_eq!(a.snapshot()["navigation"]["depth"], 0);
    }
    #[test]
    fn secure_and_read_only_fields_do_not_leak_or_clear_existing_values() {
        let mut a = app();
        let snapshot = a.snapshot();
        let node = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "secret")
            .unwrap();
        assert!(node["value"].is_null());
        assert_eq!(node["readOnly"], true);
        assert!(a
            .handle(PlatformEvent::Text {
                id: "secret".into(),
                text: "".into()
            })
            .is_err());
        a.handle(PlatformEvent::Text {
            id: "locked".into(),
            text: "".into(),
        })
        .unwrap();
        let snapshot = a.snapshot();
        let node = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "locked")
            .unwrap();
        assert_eq!(node["value"], "locked");
    }

    #[test]
    fn platform_back_is_opt_in_and_suspended_back_cannot_mutate_state() {
        let mut a = app().back_with(|s| s.clicks > 0, |s| s.clicks -= 1);
        assert_eq!(a.snapshot()["canGoBack"], false);
        a.handle(PlatformEvent::Activate { id: "count".into() })
            .unwrap();
        assert_eq!(a.snapshot()["canGoBack"], true);
        a.handle(PlatformEvent::Suspend { value: true }).unwrap();
        a.handle(PlatformEvent::Back {}).unwrap();
        assert_eq!(a.state().clicks, 1);
        a.handle(PlatformEvent::Suspend { value: false }).unwrap();
        a.handle(PlatformEvent::Back {}).unwrap();
        assert_eq!(a.state().clicks, 0);
        assert_eq!(a.snapshot()["canGoBack"], false);
    }
}
