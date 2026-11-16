//! Window identity and backend reporting, independent of native decoration policy.
use super::{BackdropStatus, WindowChrome};

/// Backend actually used for this OS window (not inferred from desktop name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowBackend {
    Wayland,
    X11,
    AppKit,
    Win32,
    Other,
}
impl WindowBackend {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
            Self::AppKit => "AppKit",
            Self::Win32 => "Win32",
            Self::Other => "Other",
        }
    }
}
/// Optional Linux backend constraint. Auto preserves winit's normal selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinuxBackend {
    #[default]
    Auto,
    Wayland,
    X11,
}
impl LinuxBackend {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "wayland" => Ok(Self::Wayland),
            "x11" => Ok(Self::X11),
            _ => Err("VOLT_UI_BACKEND must be auto, wayland or x11".into()),
        }
    }
}
/// Actual host state. A requested blur protocol is not proof of visible blur.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowDiagnostics {
    pub backend: WindowBackend,
    pub app_id: String,
    pub renderer: &'static str,
    pub chrome: WindowChrome,
    pub backdrop: BackdropStatus,
    pub scale_factor: f64,
}

pub(crate) fn validate_app_id(id: &str) -> Result<(), String> {
    if id.len() > 255
        || !id.contains('.')
        || !id.split('.').all(|part| {
            !part.is_empty()
                && (part.as_bytes()[0].is_ascii_alphabetic() || part.starts_with('_'))
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
    {
        return Err("app_id must be a reverse-DNS ASCII identifier, e.g. com.example.my_app (letters, digits and underscores; no empty segments)".into());
    }
    Ok(())
}
pub(crate) fn default_app_id(title: &str) -> String {
    // Stable development identity without exposing a document/window title in logs.
    // Not a security hash; distribution should always set an explicit app_id.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in title.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("dev.volt.app.app_{hash:016x}")
}
pub(crate) fn backend_preference(
    explicit: Option<LinuxBackend>,
    environment: Option<&str>,
) -> Result<LinuxBackend, String> {
    let preference = explicit
        .map(Ok)
        .unwrap_or_else(|| LinuxBackend::parse(environment.unwrap_or("auto")))?;
    if !cfg!(target_os = "linux") && preference != LinuxBackend::Auto {
        return Err("Wayland/X11 backend selection is only supported on Linux".into());
    }
    Ok(preference)
}
pub(crate) fn event_loop_builder<T>(
    preference: LinuxBackend,
) -> winit::event_loop::EventLoopBuilder<T> {
    let builder = winit::event_loop::EventLoop::<T>::with_user_event();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::{wayland::EventLoopBuilderExtWayland, x11::EventLoopBuilderExtX11};
        let mut builder = builder;
        match preference {
            LinuxBackend::Auto => {}
            LinuxBackend::Wayland => {
                builder.with_wayland();
            }
            LinuxBackend::X11 => {
                builder.with_x11();
            }
        }
        builder
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = preference;
        builder
    }
}
pub(crate) fn attributes(
    attributes: winit::window::WindowAttributes,
    id: &str,
) -> winit::window::WindowAttributes {
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::WindowAttributesExtWayland;
        attributes.with_name(id, id)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = id;
        attributes
    }
}
pub(crate) fn actual_backend(window: &winit::window::Window) -> WindowBackend {
    use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle as R};
    match window.display_handle().map(|h| h.as_raw()) {
        Ok(R::Wayland(_)) => WindowBackend::Wayland,
        Ok(R::Xlib(_) | R::Xcb(_)) => WindowBackend::X11,
        Ok(R::AppKit(_)) => WindowBackend::AppKit,
        Ok(R::Windows(_)) => WindowBackend::Win32,
        _ => WindowBackend::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_safe_for_desktop_filenames_and_do_not_contain_titles() {
        for id in ["dev.volt.catalog", "com.example.my_app", "io.test.App2"] {
            assert!(validate_app_id(id).is_ok());
        }
        for id in [
            "",
            "catalog",
            "../bad",
            "com..example",
            "com.2bad",
            "com.example\nExec=bad",
            "com.example app",
        ] {
            assert!(validate_app_id(id).is_err(), "{id:?}");
        }
        let id = default_app_id("My app / \"unsafe\" 🦀");
        assert!(validate_app_id(&id).is_ok());
        assert!(id.starts_with("dev.volt.app."));
        assert!(!id.contains("unsafe"));
        assert_eq!(id, default_app_id("My app / \"unsafe\" 🦀"));
        assert!(validate_app_id(&default_app_id("🦀")).is_ok());
    }
    #[test]
    fn explicit_backend_never_silently_turns_into_auto() {
        assert_eq!(
            LinuxBackend::parse("wayland").unwrap(),
            LinuxBackend::Wayland
        );
        assert_eq!(LinuxBackend::parse("x11").unwrap(), LinuxBackend::X11);
        assert_eq!(LinuxBackend::parse("auto").unwrap(), LinuxBackend::Auto);
        assert!(LinuxBackend::parse("vulkan").is_err());
    }
}
