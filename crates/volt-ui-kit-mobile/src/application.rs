use crate::{
    protocol::SystemColors,
    updates::{Inbox, Updater},
    PlatformEvent,
};
use serde_json::{json, Value};
use std::sync::Arc;
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
    inbox: Arc<Inbox<C>>,
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
        Self::with_inbox(host, state, view, Inbox::new())
    }
    /// Like [`Application::new`], for state that does work off the UI thread:
    /// `state` receives the app's [`Updater`] to keep and hand to that work.
    pub fn new_with_updater(
        host: HostContext,
        state: impl FnOnce(Updater<C>) -> C,
        view: fn(&C) -> Element<C>,
    ) -> Result<Self, String> {
        let inbox = Inbox::new();
        let state = state(inbox.updater());
        Self::with_inbox(host, state, view, inbox)
    }
    fn with_inbox(
        host: HostContext,
        state: C,
        view: fn(&C) -> Element<C>,
        inbox: Arc<Inbox<C>>,
    ) -> Result<Self, String> {
        Ok(Self {
            inbox,
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
    /// Sends state changes from other threads; see [`Updater`].
    pub fn updater(&self) -> Updater<C> {
        self.inbox.updater()
    }
    /// Whether changes from an [`Updater`] are waiting for the next event.
    pub fn has_updates(&self) -> bool {
        self.inbox.has_changes()
    }
    /// The host's way to be woken, from any thread, when changes arrive.
    #[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
    pub(crate) fn set_wake(&self, wake: Option<crate::updates::Wake>) {
        self.inbox.set_wake(wake);
    }
    /// Run queued changes in order, then lay the page out again once.
    fn apply_updates(&mut self) -> Result<(), String> {
        let changes = self.inbox.take();
        if changes.is_empty() {
            return Ok(());
        }
        self.ui.update(|state| {
            for change in changes {
                change(state);
            }
        })
    }
    /// Change the app's state from the UI thread and lay the page out again.
    pub fn update(&mut self, change: impl FnOnce(&mut C)) {
        let _ = self.ui.update(change);
    }
    pub fn state(&self) -> &C {
        self.ui.context()
    }
    pub fn mobile_view(&self) -> &MobileView<C> {
        &self.ui
    }
    pub fn handle(&mut self, event: PlatformEvent) -> Result<(), String> {
        // Changes from other threads land before the event, so a tap acts on
        // the page as it now reads.
        self.apply_updates()?;
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
            PlatformEvent::Refresh { id } => {
                self.ui.refresh(&id)?;
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
            PlatformEvent::SafeArea {
                top,
                right,
                bottom,
                left,
            } => {
                self.ui.set_safe_area([top, right, bottom, left])?;
            }
            PlatformEvent::Bars { bottom, keyboard } => self.ui.set_bars(bottom, keyboard)?,
            PlatformEvent::Draw {} | PlatformEvent::Updates {} => {}
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
                // Pull to refresh: present with `on_refresh`, and whether its
                // work is under way (the host keeps its spinner till then).
                let refresh = self.ui.runtime().refresh_state(&b.id);
                Some(
                    json!({"id": b.id, "frame": frame, "content": content, "insets": [it, ib],
                    "offset": b.scroll_offset, "axes": axes, "refresh": refresh.is_some(),
                    "refreshing": refresh.unwrap_or(false), "inert": !self.ui.runtime().accepts_input(&b.id),
                    "inDraggable": self.ui.runtime().in_draggable(&b.id)}),
                )
            })
            .collect::<Vec<_>>();
        // The outermost navigation bar, for a host that draws its own.
        let navigation = self.ui.runtime().navigation_bars().into_iter().next().map(|(id, bar)| {
            json!({"id": id, "title": bar.title, "largeTitle": bar.large_title, "depth": bar.depth,
                "back": bar.back, "buttons": bar.buttons.iter().map(|(id, label)| json!({"id": id, "label": label})).collect::<Vec<_>>()})
        });
        // The outermost tab view, for a host that draws its own tab bar.
        let tabs = self.ui.runtime().tab_bars().into_iter().next().map(|(id, bar)| {
            json!({"id": id, "selected": bar.selected, "items": bar.items.iter().map(|item| json!({
                "id": item.id, "label": item.label, "symbol": item.symbol, "icon": item.icon, "badge": item.badge,
            })).collect::<Vec<_>>()})
        });
        let theme = self.current_theme();
        // The field that wants the keyboard: hosts open it when this changes.
        let runtime = self.ui.runtime();
        let editing = runtime.focused().filter(|_| runtime.editing_focus());
        json!({"navigation": navigation, "tabs": tabs, "modal": self.ui.runtime().modal_open(), "editing": editing, "scrollViews": scroll_views, "canGoBack":self.back.is_some_and(|(can,_)|can(self.ui.context())),"animating":self.is_animating(),"nodes":nodes,"dark":self.mode==ThemeMode::Dark,"background":theme.palette.base[0],
            "fieldBackground":theme.palette.base[1],"foreground":theme.palette.content,"accent":theme.palette.primary})
    }
}
impl<C: 'static> Drop for Application<C> {
    fn drop(&mut self) {
        self.inbox.close();
    }
}
#[cfg(any(target_os = "ios", target_os = "android"))]
pub(crate) trait HostedApplication {
    fn set_wake(&self, wake: Option<crate::updates::Wake>);
    fn handle(&mut self, event: PlatformEvent) -> Result<(), String>;
    fn draw(&mut self, renderer: &mut SurfaceRenderer) -> Result<PresentStatus, String>;
    fn snapshot(&self) -> Value;
    fn suspended(&self) -> bool;
    fn allow_software(&self) -> bool;
}
#[cfg(any(target_os = "ios", target_os = "android"))]
impl<C: 'static> HostedApplication for Application<C> {
    fn set_wake(&self, wake: Option<crate::updates::Wake>) {
        Application::set_wake(self, wake)
    }
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
        refreshes: usize,
        loading: bool,
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
            .scroll(Scroll::Vertical)
            .on_refresh(|s: &mut State| {
                s.refreshes += 1;
                s.loading = true;
            })
            .refreshing(s.loading),
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
    fn pulling_a_scroll_view_runs_on_refresh_and_reports_the_spinner() {
        let mut a = app();
        a.handle(PlatformEvent::Resize {
            width: 390.,
            height: 500.,
            scale: 3.,
        })
        .unwrap();
        let list = |a: &Application<State>| {
            a.snapshot()["scrollViews"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == "list")
                .cloned()
                .unwrap()
        };
        assert_eq!(
            (list(&a)["refresh"].clone(), list(&a)["refreshing"].clone()),
            (true.into(), false.into())
        );
        let event = PlatformEvent::decode(br#"{"op":"refresh","id":"list"}"#).unwrap();
        a.handle(event).unwrap();
        assert_eq!(a.state().refreshes, 1);
        assert_eq!(
            list(&a)["refreshing"],
            true,
            "the host keeps its spinner while loading"
        );
        // An id without on_refresh does nothing.
        a.handle(PlatformEvent::Refresh { id: "count".into() })
            .unwrap();
        assert_eq!(a.state().refreshes, 1);
        assert!(PlatformEvent::decode(br#"{"op":"refresh"}"#).is_err());
        // While suspended, a pull is ignored.
        a.handle(PlatformEvent::Suspend { value: true }).unwrap();
        a.handle(PlatformEvent::Refresh { id: "list".into() })
            .unwrap();
        assert_eq!(a.state().refreshes, 1);
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
    #[test]
    fn changes_from_other_threads_apply_in_order_before_the_next_event() {
        let mut a = app();
        let updates = a.updater();
        let worker = std::thread::spawn(move || {
            for _ in 0..3 {
                assert!(updates.update(|s: &mut State| s.clicks += 10));
            }
            updates.update(|s: &mut State| s.name = format!("after {}", s.clicks));
        });
        worker.join().unwrap();
        assert!(a.has_updates());
        assert_eq!(a.state().clicks, 0, "nothing runs off the UI thread");
        a.handle(PlatformEvent::Updates {}).unwrap();
        assert!(!a.has_updates());
        assert_eq!(a.state().clicks, 30);
        assert_eq!(a.state().name, "after 30");
        // The page was laid out again with the new state.
        assert!(a.snapshot()["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["label"] == "Count 30"));
        // A tap acts on the page as it now reads.
        a.updater().update(|s: &mut State| s.clicks = 100);
        a.handle(PlatformEvent::Activate { id: "count".into() })
            .unwrap();
        assert_eq!(a.state().clicks, 101);
    }
    #[test]
    fn one_wake_per_batch_and_a_late_wake_still_delivers() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut a = app();
        let updates = a.updater();
        updates.update(|s: &mut State| s.clicks += 1);
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = wakes.clone();
        // Changes queued before the host was ready wake it when it installs.
        a.set_wake(Some(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })));
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        updates.update(|s: &mut State| s.clicks += 1);
        updates.update(|s: &mut State| s.clicks += 1);
        assert_eq!(
            wakes.load(Ordering::SeqCst),
            1,
            "joins the outstanding wake"
        );
        a.handle(PlatformEvent::Updates {}).unwrap();
        assert_eq!(a.state().clicks, 3);
        updates.update(|s: &mut State| s.clicks += 1);
        assert_eq!(wakes.load(Ordering::SeqCst), 2, "a new batch wakes again");
    }
    #[test]
    fn state_can_keep_its_updater_and_changes_stop_after_close() {
        struct Live {
            updates: Updater<Live>,
            text: String,
        }
        fn page(s: &Live) -> Element<Live> {
            text(&s.text).key("text").into()
        }
        let mut a = Application::new_with_updater(
            HostContext {
                font_family: "serif".into(),
            },
            |updates| Live {
                updates,
                text: "waiting".into(),
            },
            page,
        )
        .unwrap();
        let updates = a.state().updates.clone();
        std::thread::spawn(move || updates.update(|s: &mut Live| s.text = "loaded".into()))
            .join()
            .unwrap();
        a.handle(PlatformEvent::Updates {}).unwrap();
        assert_eq!(a.state().text, "loaded");
        let kept = a.updater();
        assert!(kept.is_open());
        drop(a);
        assert!(!kept.is_open());
        assert!(!kept.update(|s: &mut Live| s.text = "lost".into()));
    }
    #[test]
    fn the_updates_event_is_part_of_the_wire_protocol() {
        assert!(matches!(
            PlatformEvent::decode(br#"{"op":"updates"}"#).unwrap(),
            PlatformEvent::Updates {}
        ));
        assert!(PlatformEvent::decode(br#"{"op":"updates","extra":1}"#).is_err());
    }
    #[test]
    fn a_tab_view_is_described_for_a_native_tab_bar_and_runs_its_tabs() {
        #[derive(Clone, Copy, PartialEq)]
        enum Section {
            Inbox,
            Done,
        }
        struct Tabs {
            tab: Section,
            again: usize,
        }
        fn page(s: &Tabs) -> Element<Tabs> {
            tab_view(
                s.tab,
                [
                    tab_destination(Section::Inbox, "Inbox", Icon::Inbox)
                        .symbol("tray.fill")
                        .badge("3"),
                    tab_destination(Section::Done, "Done", Icon::Check).dot(),
                ],
                text(if s.tab == Section::Inbox {
                    "Inbox page"
                } else {
                    "Done page"
                })
                .key("body"),
            )
            .key("tabs")
            .on_select(|s: &mut Tabs, t| s.tab = t)
            .on_reselect(|s: &mut Tabs, _| s.again += 1)
            .into()
        }
        let mut a = Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            Tabs {
                tab: Section::Inbox,
                again: 0,
            },
            page,
        )
        .unwrap();
        a.handle(PlatformEvent::Resize {
            width: 390.,
            height: 800.,
            scale: 3.,
        })
        .unwrap();
        let tabs = &a.snapshot()["tabs"];
        assert_eq!(tabs["selected"], 0);
        assert_eq!(tabs["items"][0]["symbol"], "tray.fill");
        assert_eq!(tabs["items"][0]["badge"], "3");
        assert_eq!(tabs["items"][1]["badge"], "");
        assert_eq!(tabs["items"][1]["label"], "Done");
        // Drawn by Volt until the host says it draws its own bar.
        let drawn = |a: &Application<Tabs>| {
            a.snapshot()["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["id"] == "tabs-tab-1")
        };
        assert!(drawn(&a));
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        assert!(!drawn(&a), "the native tab bar replaces Volt's");
        a.handle(PlatformEvent::Navigation {
            id: "tabs-tab-1".into(),
        })
        .unwrap();
        assert!(a.state().tab == Section::Done);
        assert_eq!(a.snapshot()["tabs"]["selected"], 1);
        a.handle(PlatformEvent::Navigation {
            id: "tabs-tab-1".into(),
        })
        .unwrap();
        assert_eq!(
            a.state().again,
            1,
            "choosing the open tab again reselects it"
        );
    }
    #[test]
    fn the_action_dock_moves_away_while_scrolling_down_and_runs_its_actions() {
        #[derive(Default)]
        struct Dock {
            added: usize,
            more: bool,
            sorted: usize,
        }
        fn page(s: &Dock) -> Element<Dock> {
            let rows: Vec<Element<Dock>> = (0..40)
                .map(|i| {
                    text(format!("Row {i}"))
                        .key(format!("row-{i}"))
                        .height(44.)
                        .into()
                })
                .collect();
            action_dock(vstack(rows).key("list").flex(1.).scroll(Scroll::Auto))
                .key("dock")
                .follows("list")
                .primary(dock_action("Add", Icon::Plus).on_press(|s: &mut Dock| s.added += 1))
                .more([dock_action("Sort", Icon::Calendar).on_press(|s: &mut Dock| s.sorted += 1)])
                .more_open(s.more)
                .on_more(|s: &mut Dock, open| s.more = open)
                .into()
        }
        let mut a = Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            Dock::default(),
            page,
        )
        .unwrap();
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        a.handle(PlatformEvent::Resize {
            width: 390.,
            height: 800.,
            scale: 1.,
        })
        .unwrap();
        let top = |a: &Application<Dock>, id: &str| {
            a.mobile_view()
                .runtime()
                .boxes()
                .iter()
                .find(|b| b.id == id)
                .map(|b| b.bounds.bounds()[1])
                .unwrap()
        };
        let to = |a: &mut Application<Dock>, y: f32| {
            a.handle(PlatformEvent::ScrollTo {
                id: "list".into(),
                x: 0.,
                y,
            })
            .unwrap();
        };
        // 60-point bar, 12 points above the bottom.
        let shown = top(&a, "dock-bar");
        assert!((shown - (800. - 12. - 60.)).abs() < 0.5, "{shown}");
        // The list keeps room at its end so its last row clears the dock.
        let max = a
            .mobile_view()
            .runtime()
            .boxes()
            .iter()
            .find(|b| b.id == "list")
            .unwrap()
            .scroll_max[1];
        assert!((max - (40. * 44. - 800. + 84.)).abs() < 0.5, "{max}");
        // A small move down does nothing; a longer run slides it away.
        to(&mut a, 10.);
        to(&mut a, 20.);
        assert_eq!(top(&a, "dock-bar"), shown);
        to(&mut a, 60.);
        to(&mut a, 200.);
        assert!(a.mobile_view().runtime().scroll_hidden("list"));
        assert!(top(&a, "dock-bar") >= 800., "slid below the screen");
        // Scrolling back up a little brings it back.
        to(&mut a, 185.);
        assert!(!a.mobile_view().runtime().scroll_hidden("list"));
        assert_eq!(top(&a, "dock-bar"), shown);
        // Hidden again, then the end of the list shows it.
        to(&mut a, 400.);
        assert!(a.mobile_view().runtime().scroll_hidden("list"));
        to(&mut a, max);
        assert!(!a.mobile_view().runtime().scroll_hidden("list"));
        // The host's tab bar and home indicator lift it.
        a.handle(PlatformEvent::Bars {
            bottom: 83.,
            keyboard: 0.,
        })
        .unwrap();
        assert!((top(&a, "dock-bar") - (shown - 83.)).abs() < 0.5);
        // The main action runs on a tap.
        let y = top(&a, "dock-primary") + 10.;
        a.handle(PlatformEvent::Begin { x: 195., y }).unwrap();
        a.handle(PlatformEvent::End { x: 195., y }).unwrap();
        assert_eq!(a.state().added, 1);
        // More lists the rest in a sheet; choosing one closes it, then runs it.
        let y = top(&a, "dock-more") + 10.;
        a.handle(PlatformEvent::Begin { x: 360., y }).unwrap();
        a.handle(PlatformEvent::End { x: 360., y }).unwrap();
        assert!(a.state().more);
        let y = top(&a, "dock-more-0") + 10.;
        a.handle(PlatformEvent::Begin { x: 100., y }).unwrap();
        a.handle(PlatformEvent::End { x: 100., y }).unwrap();
        assert_eq!((a.state().more, a.state().sorted), (false, 1));
    }
    #[test]
    fn sheets_and_dock_search_rise_above_the_keyboard() {
        #[derive(Default)]
        struct Page {
            sheet: bool,
            name: String,
            searching: bool,
            query: String,
        }
        fn page(s: &Page) -> Element<Page> {
            let rows: Vec<Element<Page>> = (0..30)
                .map(|i| {
                    text(format!("Row {i}"))
                        .key(format!("row-{i}"))
                        .height(44.)
                        .into()
                })
                .collect();
            zstack![
                action_dock(vstack(rows).key("list").flex(1.).scroll(Scroll::Auto))
                    .key("dock")
                    .follows("list")
                    .search(
                        dock_search(&s.query, s.searching)
                            .on_change(|s: &mut Page, q| s.query = q)
                            .on_open(|s: &mut Page, open| s.searching = open),
                    ),
                bottom_sheet(
                    text_field(&s.name)
                        .key("name")
                        .on_change(|s: &mut Page, v| s.name = v),
                )
                .key("sheet")
                .title("New list")
                .open(s.sheet)
                .on_dismiss(|s: &mut Page| s.sheet = false),
            ]
            .flex(1.)
            .into()
        }
        let mut a = Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            Page::default(),
            page,
        )
        .unwrap();
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        a.handle(PlatformEvent::Resize {
            width: 402.,
            height: 874.,
            scale: 1.,
        })
        .unwrap();
        let bounds = |a: &Application<Page>, id: &str| {
            a.mobile_view()
                .runtime()
                .boxes()
                .iter()
                .find(|b| b.id == id)
                .map(|b| b.bounds.bounds())
        };
        let keyboard = |a: &mut Application<Page>, height: f32| {
            a.handle(PlatformEvent::Bars {
                bottom: height.max(34.),
                keyboard: height,
            })
            .unwrap();
        };
        // Search: the Search button turns the dock into a field that asks
        // for the keyboard, and the dock rides on top of it.
        assert!(bounds(&a, "dock-search-field").is_none());
        a.handle(PlatformEvent::Activate {
            id: "dock-search-open".into(),
        })
        .unwrap();
        assert!(a.state().searching);
        assert_eq!(a.snapshot()["editing"], "dock-search-field");
        keyboard(&mut a, 336.);
        let field = bounds(&a, "dock-search-field").unwrap();
        assert!(field[1] + field[3] <= 874. - 336., "{field:?}");
        assert!(!a.mobile_view().runtime().scroll_hidden("list"));
        a.handle(PlatformEvent::Text {
            id: "dock-search-field".into(),
            text: "Row 2".into(),
        })
        .unwrap();
        assert_eq!(a.state().query, "Row 2");
        a.handle(PlatformEvent::Activate {
            id: "dock-search-close".into(),
        })
        .unwrap();
        assert!(!a.state().searching);
        assert!(bounds(&a, "dock-primary").is_none() && bounds(&a, "dock-search-open").is_some());
        // A sheet's field stays above the keyboard.
        keyboard(&mut a, 0.);
        a.update(|s| s.sheet = true);
        let resting = bounds(&a, "name").unwrap();
        keyboard(&mut a, 336.);
        let lifted = bounds(&a, "name").unwrap();
        assert!(
            lifted[1] + lifted[3] <= 874. - 336.,
            "{resting:?} {lifted:?}"
        );
        keyboard(&mut a, 0.);
        assert_eq!(bounds(&a, "name").unwrap(), resting);
    }
    #[test]
    fn a_dropped_sheet_settles_where_it_belongs() {
        #[derive(Default)]
        struct Sheet {
            open: bool,
            height: usize,
        }
        fn page(s: &Sheet) -> Element<Sheet> {
            zstack![
                text("Page").key("page"),
                bottom_sheet(text("Body").key("body"))
                    .key("sheet")
                    .title("New idea")
                    .detents([SheetDetent::Medium, SheetDetent::Large])
                    .detent(s.height)
                    .on_detent(|s: &mut Sheet, i| s.height = i)
                    .open(s.open)
                    .on_dismiss(|s: &mut Sheet| s.open = false),
            ]
            .flex(1.)
            .into()
        }
        let mut a = Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            Sheet {
                open: true,
                height: 1,
            },
            page,
        )
        .unwrap();
        a.handle(PlatformEvent::Resize {
            width: 390.,
            height: 800.,
            scale: 1.,
        })
        .unwrap();
        let top = |a: &Application<Sheet>| {
            a.mobile_view()
                .runtime()
                .boxes()
                .iter()
                .find(|b| b.id == "sheet-content")
                .unwrap()
                .bounds
                .bounds()[1]
        };
        let settle = |a: &mut Application<Sheet>| {
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(40));
                a.handle(PlatformEvent::Draw {}).unwrap();
            }
        };
        let drag = |a: &mut Application<Sheet>, from: f32, dy: f32| {
            a.handle(PlatformEvent::Begin { x: 195., y: from }).unwrap();
            for i in 1..=6 {
                a.handle(PlatformEvent::Move {
                    x: 195.,
                    y: from + dy * i as f32 / 6.,
                })
                .unwrap();
            }
            a.handle(PlatformEvent::End {
                x: 195.,
                y: from + dy,
            })
            .unwrap();
        };
        settle(&mut a);
        let large = top(&a);
        // A short drag down springs back to full height.
        drag(&mut a, large + 30., 50.);
        assert_eq!(a.state().height, 1);
        settle(&mut a);
        assert_eq!(
            a.mobile_view().runtime().drag_offset("sheet-content"),
            [0., 0.]
        );
        assert_eq!(top(&a), large);
        // A long one steps to half height, and nothing is left over.
        drag(&mut a, large + 30., 190.);
        assert_eq!(a.state().height, 0);
        settle(&mut a);
        assert_eq!(
            a.mobile_view().runtime().drag_offset("sheet-content"),
            [0., 0.]
        );
        assert!(top(&a) > large + 100.);
        // The platform taking the touch mid-drag (a scroll view's pan): the
        // sheet goes back to its place rather than staying under the finger.
        let resting = top(&a);
        a.handle(PlatformEvent::Begin {
            x: 195.,
            y: resting + 30.,
        })
        .unwrap();
        for i in 1..=4 {
            a.handle(PlatformEvent::Move {
                x: 195.,
                y: resting + 30. + 15. * i as f32,
            })
            .unwrap();
        }
        assert!(a.mobile_view().runtime().drag_offset("sheet-content")[1] > 0.);
        a.handle(PlatformEvent::Cancel {}).unwrap();
        settle(&mut a);
        assert_eq!(
            a.mobile_view().runtime().drag_offset("sheet-content"),
            [0., 0.]
        );
        assert!(a.state().open);
    }
    #[test]
    fn a_drag_from_a_full_height_sheets_header_steps_it_down() {
        #[derive(Default)]
        struct Sheet {
            open: bool,
            height: usize,
            name: String,
        }
        fn page(s: &Sheet) -> Element<Sheet> {
            zstack![
                text("Page").key("page"),
                bottom_sheet(vstack![
                    text("Name"),
                    text_field(&s.name)
                        .key("name")
                        .on_change(|s: &mut Sheet, v| s.name = v)
                ])
                .key("sheet")
                .title("New list")
                .detents([SheetDetent::Medium, SheetDetent::Large])
                .detent(s.height)
                .on_detent(|s: &mut Sheet, i| s.height = i)
                .cancel("Cancel", |s: &mut Sheet| s.open = false)
                .done("Create", |s: &mut Sheet| s.open = false)
                .open(s.open)
                .on_dismiss(|s: &mut Sheet| s.open = false),
            ]
            .flex(1.)
            .into()
        }
        let mut a = Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            Sheet {
                open: true,
                height: 1,
                ..Default::default()
            },
            page,
        )
        .unwrap();
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        a.handle(PlatformEvent::Resize {
            width: 402.,
            height: 874.,
            scale: 3.,
        })
        .unwrap();
        a.handle(PlatformEvent::SafeArea {
            top: 62.,
            right: 0.,
            bottom: 34.,
            left: 0.,
        })
        .unwrap();
        let ys = [160f32, 210., 270., 330., 350.];
        a.handle(PlatformEvent::Begin { x: 200., y: ys[0] })
            .unwrap();
        for y in &ys[1..] {
            a.handle(PlatformEvent::Move { x: 200., y: *y }).unwrap();
        }
        a.handle(PlatformEvent::End { x: 200., y: 350. }).unwrap();
        assert_eq!(a.state().height, 0, "a 190-point drag on the header steps down");
    }
    #[test]
    fn a_sheet_follows_a_drag_then_closes_steps_or_stays() {
        #[derive(Default)]
        struct Sheet {
            open: bool,
            height: usize,
            closed: usize,
            asked: usize,
            guard: bool,
        }
        fn page(s: &Sheet) -> Element<Sheet> {
            let sheet = bottom_sheet(text("Body").key("body"))
                .key("sheet")
                .title("New idea")
                .detents([SheetDetent::Medium, SheetDetent::Large])
                .detent(s.height)
                .on_detent(|s: &mut Sheet, i| s.height = i)
                .open(s.open)
                .on_dismiss(|s: &mut Sheet| {
                    s.open = false;
                    s.closed += 1
                });
            let sheet = if s.guard {
                sheet.on_dismiss_attempt(|s: &mut Sheet| s.asked += 1)
            } else {
                sheet
            };
            zstack![text("Page").key("page"), sheet].flex(1.).into()
        }
        let mut a = Application::new(
            HostContext {
                font_family: "serif".into(),
            },
            Sheet {
                open: true,
                ..Sheet::default()
            },
            page,
        )
        .unwrap();
        a.handle(PlatformEvent::Resize {
            width: 390.,
            height: 800.,
            scale: 1.,
        })
        .unwrap();
        let drag = |a: &mut Application<Sheet>, dy: f32| {
            for e in [
                // The header of a sheet at medium height (its top is at 428).
                PlatformEvent::Begin { x: 195., y: 470. },
                PlatformEvent::Move {
                    x: 195.,
                    y: 470. + dy / 2.,
                },
                PlatformEvent::Move {
                    x: 195.,
                    y: 470. + dy,
                },
                PlatformEvent::End {
                    x: 195.,
                    y: 470. + dy,
                },
            ] {
                a.handle(e).unwrap();
            }
        };
        // A short drag springs back and changes nothing.
        drag(&mut a, 30.);
        assert!(a.state().open && a.state().height == 0);
        // Up a long way: the larger height.
        drag(&mut a, -160.);
        assert_eq!(a.state().height, 1);
        // Down from the larger height: back to medium, still open.
        a.handle(PlatformEvent::Begin { x: 195., y: 90. }).unwrap();
        a.handle(PlatformEvent::Move { x: 195., y: 290. }).unwrap();
        a.handle(PlatformEvent::End { x: 195., y: 290. }).unwrap();
        assert_eq!(a.state().height, 0);
        assert!(a.state().open);
        // Down from the smallest height closes it, unless that loses work.
        a.update(|s| s.guard = true);
        drag(&mut a, 200.);
        assert_eq!((a.state().asked, a.state().closed), (1, 0));
        assert!(a.state().open, "the app decides after asking");
        a.update(|s| s.guard = false);
        drag(&mut a, 200.);
        assert_eq!(a.state().closed, 1);
        assert!(!a.state().open);
        // A phone-sized swipe up from the grabber, in many small moves, with
        // the iOS host's scrolling and bars.
        a.update(|s| s.open = true);
        a.handle(PlatformEvent::Host {
            native_scrolling: true,
            native_navigation: true,
            native_controls: true,
        })
        .unwrap();
        a.handle(PlatformEvent::Resize {
            width: 402.,
            height: 874.,
            scale: 3.,
        })
        .unwrap();
        a.handle(PlatformEvent::Begin { x: 201., y: 485. }).unwrap();
        for i in 1..=12 {
            let y = 485. - 235. * i as f32 / 12.;
            a.handle(PlatformEvent::Move { x: 201., y }).unwrap();
        }
        a.handle(PlatformEvent::End { x: 201., y: 250. }).unwrap();
        assert!(a.state().open, "a swipe up never closes the sheet");
        assert_eq!(a.state().height, 1, "a swipe up makes it taller");
        a.update(|s| s.height = 0);
        // The grabber steps through the heights for people who can't drag.
        a.update(|s| s.open = true);
        a.handle(PlatformEvent::Begin { x: 201., y: 475. }).unwrap();
        a.handle(PlatformEvent::End { x: 201., y: 475. }).unwrap();
        assert_eq!(a.state().height, 1, "a tap on the grabber");
    }
}
