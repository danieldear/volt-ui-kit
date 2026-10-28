use crate::PlatformEvent;
use serde_json::{json, Value};
#[cfg(target_os = "ios")]
use volt_ui_kit::rendering::{PresentStatus, SurfaceRenderer};
use volt_ui_kit::{mobile::MobileView, ui::Element, Palette, ThemeMode, UiTheme};

/// Platform resources supplied once by a host. No UIKit/Android types leak in.
#[derive(Clone, Debug)]
pub struct HostContext {
    pub font_family: String,
}
/// A Rust-owned mobile application. Rendering policy and themes belong to the app.
pub struct Application<C: 'static> {
    ui: MobileView<C>,
    mode: ThemeMode,
    theme: fn(&C, ThemeMode) -> UiTheme,
    appearance: Option<fn(&mut C, ThemeMode)>,
    suspended: bool,
    allow_software: bool,
    scale: f32,
}
fn default_theme<C>(_: &C, mode: ThemeMode) -> UiTheme {
    UiTheme::from_palette(if mode == ThemeMode::Dark {
        Palette::zinc_dark()
    } else {
        Palette::zinc_light()
    })
}
impl<C: 'static> Application<C> {
    pub fn new(host: HostContext, state: C, view: fn(&C) -> Element<C>) -> Result<Self, String> {
        Ok(Self {
            ui: MobileView::new_with_font(state, view, host.font_family)?,
            mode: ThemeMode::Light,
            theme: default_theme::<C>,
            appearance: None,
            suspended: false,
            allow_software: false,
            scale: 1.,
        })
    }
    pub fn theme_with(mut self, theme: fn(&C, ThemeMode) -> UiTheme) -> Self {
        self.theme = theme;
        self
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
            PlatformEvent::Appearance { dark } => {
                self.mode = if dark {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                };
                if let Some(observer) = self.appearance {
                    let mode = self.mode;
                    self.ui.update(|s| observer(s, mode))?;
                }
            }
            PlatformEvent::Suspend { value } => {
                self.suspended = value;
                self.ui.set_suspended(value);
            }
            PlatformEvent::Draw {} => {}
        }
        Ok(())
    }
    pub fn background(&self) -> [f32; 4] {
        (self.theme)(self.ui.context(), self.mode).palette.base[0]
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
            let kind=match n.role {AccessRole::TextInput=>"field",AccessRole::ScrollView=>"scroll",AccessRole::Button=>"button",_ if n.clickable=>"button",_=>"label"};
            json!({"id":n.id,"label":n.label,"value":n.value,"enabled":n.enabled,"kind":kind,
                "password":n.password,"readOnly":n.read_only || n.password,"bounds":n.bounds.map(|v|v/self.scale),"visibleBounds":visible.map(|v|v/self.scale)})
        }).collect::<Vec<_>>();
        let theme = (self.theme)(self.ui.context(), self.mode);
        json!({"nodes":nodes,"dark":self.mode==ThemeMode::Dark,"background":theme.palette.base[0],
            "fieldBackground":theme.palette.base[1],"foreground":theme.palette.content,"accent":theme.palette.primary})
    }
}
#[cfg(target_os = "ios")]
pub(crate) trait HostedApplication {
    fn handle(&mut self, event: PlatformEvent) -> Result<(), String>;
    fn draw(&mut self, renderer: &mut SurfaceRenderer) -> Result<PresentStatus, String>;
    fn snapshot(&self) -> Value;
    fn suspended(&self) -> bool;
    fn allow_software(&self) -> bool;
}
#[cfg(target_os = "ios")]
impl<C: 'static> HostedApplication for Application<C> {
    fn handle(&mut self, e: PlatformEvent) -> Result<(), String> {
        self.handle(e)
    }
    fn draw(&mut self, r: &mut SurfaceRenderer) -> Result<PresentStatus, String> {
        let theme = (self.theme)(self.ui.context(), self.mode);
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
        a.handle(PlatformEvent::Appearance { dark: true }).unwrap();
        assert!(a.state().dark);
        assert_eq!(a.background(), [0.1, 0.2, 0.3, 1.]);
        assert_eq!(a.snapshot()["dark"], true);
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
}
