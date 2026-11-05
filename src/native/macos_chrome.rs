//! Narrow public AppKit chrome integration. No raw-pointer dereference, private
//! selectors, view reparenting, button resizing or replacement of winit's delegate.
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSTitlebarSeparatorStyle, NSToolbar, NSWindow, NSWindowButton,
    NSWindowStyleMask, NSWindowToolbarStyle,
};
use objc2_foundation::NSString;
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

/// Called once per window creation, on winit's main-thread resumed lifecycle.
/// NSWindow retains its toolbar; AppKit owns layout during resize/fullscreen.
pub(super) fn install_unified_titlebar(window: &Window) -> Result<(), String> {
    if !objc2::available!(macos = 11.0) {
        eprintln!("Volt UI: unified overlay requires macOS 11; retaining compact native controls");
        return Ok(());
    }
    let mtm = MainThreadMarker::new().ok_or("AppKit chrome requires the main thread")?;
    let ns_window = app_window(window, mtm)?;
    if ns_window.toolbar().is_some() {
        return Err("unified overlay cannot replace an existing native toolbar".into());
    }
    let toolbar = NSToolbar::initWithIdentifier(
        NSToolbar::alloc(mtm),
        &NSString::from_str("dev.volt-ui.overlay"),
    );
    toolbar.setVisible(true);
    ns_window.setToolbarStyle(NSWindowToolbarStyle::Unified);
    ns_window.setToolbar(Some(&toolbar));
    ns_window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
    // Preserve full-size content and a transparent native chrome surface. The
    // toolkit still paints the header; only system window geometry changes.
    ns_window.setTitlebarAppearsTransparent(true);
    Ok(())
}

/// In fullscreen AppKit moves a visible toolbar into its own opaque titlebar
/// window pinned over the content, hiding the toolkit-painted header. The
/// overlay toolbar is empty, so hide it while fullscreen (the titlebar then
/// auto-hides and reveals on hover) and show it again in a normal window.
pub(super) fn sync_fullscreen_toolbar(window: &Window) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Ok(ns_window) = app_window(window, mtm) else {
        return;
    };
    let Some(toolbar) = ns_window.toolbar() else {
        return;
    };
    let fullscreen = ns_window
        .styleMask()
        .contains(NSWindowStyleMask::FullScreen);
    if toolbar.isVisible() == fullscreen {
        toolbar.setVisible(!fullscreen);
    }
}

pub(super) fn app_window(
    window: &Window,
    mtm: MainThreadMarker,
) -> Result<Retained<NSWindow>, String> {
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err("expected AppKit window handle".into());
    };
    // Obtain retained AppKit objects through its safe API, then compare identity
    // with winit's borrowed NSView handle. Never dereference or take ownership of it.
    let app = NSApplication::sharedApplication(mtm);
    app.windows()
        .iter()
        .find(|candidate| {
            candidate.contentView().is_some_and(|view| {
                std::ptr::from_ref(&*view).cast::<std::ffi::c_void>() == handle.ns_view.as_ptr()
            })
        })
        .ok_or_else(|| "winit content view was not found in AppKit windows".into())
}

/// Read-only diagnostics after a frame, only when VOLT_UI_TRACE is enabled.
pub(super) fn trace_geometry(window: &Window) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Ok(ns_window) = app_window(window, mtm) else {
        return;
    };
    // AppKit moves fullscreen controls into its own transient titlebar window.
    // Coordinates there must not be reported relative to the content window.
    if ns_window
        .styleMask()
        .contains(NSWindowStyleMask::FullScreen)
    {
        eprintln!("VOLT_UI_CHROME fullscreen=true geometry=system-managed");
        return;
    }
    for (label, kind) in [
        ("close", NSWindowButton::CloseButton),
        ("minimize", NSWindowButton::MiniaturizeButton),
        ("zoom", NSWindowButton::ZoomButton),
    ] {
        if let Some(button) = ns_window.standardWindowButton(kind) {
            let bounds = button.bounds();
            let rect = button.convertRect_toView(bounds, None);
            let top = ns_window.frame().size.height - rect.origin.y - rect.size.height;
            eprintln!("VOLT_UI_CHROME button={label} left={:.1} top={top:.1} size={:.1}x{:.1} bounds={:.1}x{:.1}",
                rect.origin.x, rect.size.width, rect.size.height, bounds.size.width, bounds.size.height);
        }
    }
}

/// Change only the toolbar/style owned by this host. Never replace a delegate,
/// content view or Metal layer while switching native/custom decorations.
pub(super) fn apply_chrome(window: &Window, chrome: super::WindowChrome) -> Result<(), String> {
    use objc2_app_kit::NSWindowTitleVisibility;
    let mtm = MainThreadMarker::new().ok_or("AppKit chrome requires the main thread")?;
    let ns_window = app_window(window, mtm)?;
    if let Some(toolbar) = ns_window.toolbar() {
        if toolbar.identifier().to_string() == "dev.volt-ui.overlay" {
            ns_window.setToolbar(None);
        }
    }
    let overlay = matches!(
        chrome,
        super::WindowChrome::Overlay | super::WindowChrome::CompactOverlay
    );
    window.set_decorations(!matches!(
        chrome,
        super::WindowChrome::Custom | super::WindowChrome::Borderless
    ));
    let mut mask = ns_window.styleMask();
    mask.set(NSWindowStyleMask::FullSizeContentView, overlay);
    ns_window.setStyleMask(mask);
    ns_window.setTitlebarAppearsTransparent(overlay);
    ns_window.setTitleVisibility(if overlay {
        NSWindowTitleVisibility::Hidden
    } else {
        NSWindowTitleVisibility::Visible
    });
    if chrome == super::WindowChrome::Overlay {
        install_unified_titlebar(window)?;
    }
    Ok(())
}

/// Native geometry for borderless zoom, without winit's temporary style-mask
/// mutation in is_maximized(). Screen coordinates here are AppKit points.
#[derive(Default)]
pub(super) struct CustomZoom {
    restore: Option<objc2_foundation::NSRect>,
}
fn same_frame(a: objc2_foundation::NSRect, b: objc2_foundation::NSRect) -> bool {
    [
        a.origin.x - b.origin.x,
        a.origin.y - b.origin.y,
        a.size.width - b.size.width,
        a.size.height - b.size.height,
    ]
    .iter()
    .all(|v| v.abs() <= 1.)
}
pub(super) fn maximized(window: &Window) -> Result<bool, String> {
    let mtm = MainThreadMarker::new().ok_or("AppKit zoom requires main thread")?;
    let native = app_window(window, mtm)?;
    if window.is_decorated() {
        return Ok(native.isZoomed());
    }
    Ok(native
        .screen()
        .is_some_and(|screen| same_frame(native.frame(), screen.visibleFrame())))
}
pub(super) fn toggle_custom_zoom(window: &Window, cache: &mut CustomZoom) -> Result<(), String> {
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    let mtm = MainThreadMarker::new().ok_or("AppKit zoom requires main thread")?;
    let native = app_window(window, mtm)?;
    let visible = native
        .screen()
        .ok_or("window has no screen")?
        .visibleFrame();
    let current = native.frame();
    let target = if same_frame(current, visible) {
        cache.restore.take().unwrap_or_else(|| {
            let w = (visible.size.width * 0.75)
                .max(320.)
                .min(visible.size.width);
            let h = (visible.size.height * 0.75)
                .max(240.)
                .min(visible.size.height);
            NSRect::new(
                NSPoint::new(
                    visible.origin.x + (visible.size.width - w) / 2.,
                    visible.origin.y + (visible.size.height - h) / 2.,
                ),
                NSSize::new(w, h),
            )
        })
    } else {
        cache.restore = Some(current);
        visible
    };
    native.setFrame_display(target, true);
    Ok(())
}
#[cfg(test)]
mod zoom_tests {
    use super::*;
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    #[test]
    fn zoom_state_requires_matching_size_and_origin_with_point_rounding_tolerance() {
        let r = |x, y, w, h| NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
        assert!(same_frame(r(0., 24., 1440., 900.), r(0., 24., 1440., 900.)));
        assert!(same_frame(
            r(0.5, 24., 1439.5, 900.),
            r(0., 24., 1440., 900.)
        ));
        assert!(!same_frame(
            r(10., 24., 1440., 900.),
            r(0., 24., 1440., 900.)
        ));
        assert!(!same_frame(
            r(0., 24., 1100., 800.),
            r(0., 24., 1440., 900.)
        ));
    }
}
