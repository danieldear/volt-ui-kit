//! Additive standalone application configuration. No shell or layout is imposed.
use super::{
    messages::{self, Messages},
    window, NativeOptions, SurfaceOptions, UiSender, WindowBackground, WindowChrome,
};
use crate::{ui::Element, Palette, ThemeMode, UiTheme};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RendererPreference {
    /// Prefer GPU when compiled in, with opaque software fallback.
    #[default]
    Auto,
    /// Fail rather than silently falling back when GPU is unavailable.
    GpuRequired,
    Software,
}
impl RendererPreference {
    pub(super) fn policy(self, available: bool) -> Result<(bool, bool), String> {
        match self {
            Self::Auto => Ok((available, false)),
            Self::Software => Ok((false, false)),
            Self::GpuRequired if available => Ok((true, true)),
            Self::GpuRequired => Err("GPU required but native-gpu was not compiled in".into()),
        }
    }
}
pub(super) type CommandCallback<C> = Box<dyn FnMut(&mut C, &str)>;
pub(super) struct HostExtensions<C> {
    pub renderer: Option<RendererPreference>,
    pub desktop: Option<super::system::DesktopConfig>,
    pub command: Option<CommandCallback<C>>,
}
impl<C> Default for HostExtensions<C> {
    fn default() -> Self {
        Self {
            renderer: None,
            desktop: None,
            command: None,
        }
    }
}
type View<C> = Box<dyn Fn(&C) -> Element<C>>;
/// Configuration plus app-owned state/view. Existing `run_ui` APIs remain unchanged.
pub struct Application<C> {
    options: SurfaceOptions,
    context: C,
    view: View<C>,
    theme: Option<window::ThemeFn<C>>,
    messages: Option<Box<dyn Messages<C>>>,
    extensions: HostExtensions<C>,
}
impl<C: 'static> Application<C> {
    pub fn new(
        title: impl Into<String>,
        state: C,
        view: impl Fn(&C) -> Element<C> + 'static,
    ) -> Self {
        Self {
            options: NativeOptions {
                title: title.into(),
                ..NativeOptions::default()
            }
            .with_theme(ThemeMode::System),
            context: state,
            view: Box::new(view),
            theme: None,
            messages: None,
            extensions: HostExtensions {
                renderer: Some(RendererPreference::Auto),
                ..HostExtensions::default()
            },
        }
    }
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.options.window.native.size = [width, height];
        self
    }
    pub fn renderer(mut self, preference: RendererPreference) -> Self {
        self.extensions.renderer = Some(preference);
        self
    }
    pub fn background(mut self, background: WindowBackground) -> Self {
        self.options.background = background;
        self
    }
    pub fn chrome(mut self, chrome: WindowChrome) -> Self {
        self.options.window.chrome = chrome;
        self
    }
    pub fn theme(mut self, mode: ThemeMode) -> Self {
        self.options.theme = mode;
        self
    }
    pub fn palette(mut self, palette: Palette) -> Self {
        self.options.palette = Some(palette);
        self
    }
    pub fn reduced_motion(mut self, value: bool) -> Self {
        self.options.window.reduced_motion = value;
        self
    }
    pub fn theme_with(mut self, theme: impl Fn(&C, ThemeMode) -> UiTheme + 'static) -> Self {
        self.theme = Some(Box::new(theme));
        self
    }
    /// Install bounded background messages before starting the window.
    pub fn messages<M: Send + 'static>(
        mut self,
        initialize: impl FnOnce(&mut C, UiSender<M>),
        update: impl FnMut(&mut C, M) + 'static,
    ) -> Self {
        let (sender, pump) = messages::channel(update);
        initialize(&mut self.context, sender);
        self.messages = Some(pump);
        self
    }
    /// Optional native menus/tray. Requires `native-desktop` on macOS.
    pub fn desktop(
        mut self,
        config: super::system::DesktopConfig,
        command: impl FnMut(&mut C, &str) + 'static,
    ) -> Self {
        self.extensions.desktop = Some(config);
        self.extensions.command = Some(Box::new(command));
        self
    }
    pub fn options(&self) -> &SurfaceOptions {
        &self.options
    }
    pub fn renderer_preference(&self) -> RendererPreference {
        self.extensions.renderer.unwrap_or_default()
    }
    /// Configuration validation only; never invokes the view or creates OS resources.
    pub fn validate(&self) -> Result<(), String> {
        if self.options.native.title.trim().is_empty()
            || self.options.native.title.len() > 1024
            || self.options.native.title.chars().any(char::is_control)
            || self
                .options
                .native
                .size
                .iter()
                .any(|&n| n == 0 || n > 16384)
        {
            return Err("invalid application title or window size".into());
        }
        window::validate_options(&self.options)?;
        self.renderer_preference()
            .policy(cfg!(feature = "native-gpu"))?;
        if let Some(desktop) = &self.extensions.desktop {
            desktop.validate()?;
            if !super::system::capabilities().native_menu {
                return Err("native desktop integration requires native-desktop on macOS".into());
            }
        }
        Ok(())
    }
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        self.validate()?;
        window::run_ui_extended(
            self.options,
            self.context,
            self.view,
            self.theme,
            self.messages,
            self.extensions,
        )
    }
}
