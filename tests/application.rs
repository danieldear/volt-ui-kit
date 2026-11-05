#![cfg(feature = "native")]
use volt_ui_kit::native::{Application, RendererPreference, WindowBackground};
use volt_ui_kit::ui::prelude::*;
#[test]
fn application_configuration_is_additive_and_developer_owned() {
    let app = Application::new("Inbox", (), |_| text("Mail").into())
        .renderer(RendererPreference::Software)
        .background(WindowBackground::Blur { opacity: 0.65 });
    assert_eq!(
        app.options().background,
        WindowBackground::Blur { opacity: 0.65 }
    );
    assert_eq!(app.renderer_preference(), RendererPreference::Software);
    assert!(app.validate().is_ok());
    assert_eq!(
        Application::new("App", (), |_| text("Hi").into())
            .options()
            .theme,
        ThemeMode::System
    );
}
#[test]
fn invalid_application_configuration_fails_before_creating_a_window() {
    let app = Application::new("App", (), |_| text("Hi").into())
        .background(WindowBackground::Transparent { opacity: f32::NAN });
    assert!(app.validate().is_err());
    for title in ["", " \t", "bad\nname"] {
        assert!(Application::new(title, (), |_| text("Hi").into())
            .validate()
            .is_err());
    }
    for size in [(0, 400), (400, 0), (16385, 400)] {
        assert!(Application::new("App", (), |_| text("Hi").into())
            .size(size.0, size.1)
            .validate()
            .is_err());
    }
}

#[test]
fn required_renderer_and_unsupported_services_report_missing_capabilities() {
    let app = Application::new("App", (), |_| text("Hi").into())
        .renderer(RendererPreference::GpuRequired);
    assert_eq!(app.validate().is_ok(), cfg!(feature = "native-gpu"));
    let app = Application::new("App", (), |_| text("Hi").into()).desktop(
        volt_ui_kit::native::system::DesktopConfig::new("App"),
        |_, _| {},
    );
    assert_eq!(
        app.validate().is_ok(),
        cfg!(all(target_os = "macos", feature = "native-desktop"))
    );
}

#[test]
fn tray_visibility_is_opt_in_and_does_not_change_existing_options() {
    let app = Application::new("Tray", true, |_| text("Monitor").into())
        .window_visibility_from_state(|hidden| !*hidden);
    assert!(app.validate().is_ok());
    assert_eq!(app.options().theme, ThemeMode::System);
}

#[test]
fn reveal_rejects_relative_paths_before_opening_any_os_ui() {
    assert_eq!(
        volt_ui_kit::native::file_dialog::reveal_file(std::path::Path::new("relative")),
        Err(volt_ui_kit::native::file_dialog::FileDialogError::InvalidOptions)
    );
}

#[cfg(feature = "native")]
#[test]
fn trash_rejects_relative_root_and_parent_traversal_without_os_actions() {
    use volt_ui_kit::native::file_dialog::{move_to_trash, FileDialogError};
    for path in ["relative", "/", "/tmp/../etc"] {
        assert_eq!(
            move_to_trash(std::path::Path::new(path)),
            Err(FileDialogError::InvalidOptions)
        );
    }
}
