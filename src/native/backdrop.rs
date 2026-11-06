//! Requested compositor effects, not proof that pixels have been blurred.
/// Convert physical window extents into Wayland surface-local coordinates.
#[cfg(any(target_os = "linux", test))]
pub(super) fn logical_extent(physical: u32, scale: f64) -> i32 {
    if !scale.is_finite() || scale <= 0. {
        return 0;
    }
    (f64::from(physical) / scale)
        .ceil()
        .clamp(0., f64::from(i32::MAX)) as i32
}

#[cfg(any(target_os = "linux", all(test, unix)))]
pub(crate) mod protocol;

/// The API used to request native background blur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackdropProtocol {
    /// Modern Wayland `ext-background-effect-v1`.
    WaylandBackgroundEffect,
    /// Older Wayland KDE blur protocol.
    WaylandKde,
    /// Public AppKit visual-effect material.
    AppKitMaterial,
    /// Explicit opt-in private macOS compositor request.
    MacosCompositor,
}

/// Why this window cannot currently request blur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackdropUnavailable {
    /// The current winit X11 path has no blur implementation.
    X11,
    /// Neither supported Wayland protocol is advertised.
    NoProtocol,
    /// The compositor does not currently advertise the blur capability.
    CompositorDeclined,
    /// Opaque software presentation cannot show native backdrop effects.
    SoftwareRenderer,
    /// The OS accessibility setting disables translucency.
    ReducedTransparency,
    /// No adapter is implemented for this platform.
    Platform,
    /// Public AppKit underlay currently needs a decorated window.
    UndecoratedWindow,
}

/// Native backdrop request status. `Requested` does not promise visible blur:
/// the desktop compositor owns its algorithm, policy and effect settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackdropStatus {
    /// Blur is not requested.
    #[default]
    Disabled,
    /// Waiting asynchronously for compositor globals/capabilities.
    Discovering,
    /// The effect was requested through the named API.
    Requested(BackdropProtocol),
    /// The effect is unavailable in the current configuration.
    Unsupported(BackdropUnavailable),
    /// Setup or the Wayland connection failed; see the diagnostic message.
    Failed,
}

impl BackdropStatus {
    /// Short truthful text suitable for a settings panel.
    pub fn description(self) -> &'static str {
        match self {
            Self::Disabled => "Backdrop blur is off",
            Self::Discovering => "Checking compositor blur support…",
            Self::Requested(BackdropProtocol::WaylandBackgroundEffect) => {
                "Blur requested via ext-background-effect-v1; compositor controls the result"
            }
            Self::Requested(BackdropProtocol::WaylandKde) => {
                "Blur requested via legacy KDE protocol; compositor controls the result"
            }
            Self::Requested(BackdropProtocol::AppKitMaterial) => {
                "AppKit material requested; macOS adds its system tint"
            }
            Self::Requested(BackdropProtocol::MacosCompositor) => {
                "Compositor blur requested via private macOS API"
            }
            Self::Unsupported(BackdropUnavailable::X11) => {
                "Blur unavailable: the current X11 adapter does not support it"
            }
            Self::Unsupported(BackdropUnavailable::NoProtocol) => {
                "Blur unavailable: compositor advertises no supported blur protocol"
            }
            Self::Unsupported(BackdropUnavailable::CompositorDeclined) => {
                "Blur unavailable: compositor has not enabled its blur capability"
            }
            Self::Unsupported(BackdropUnavailable::SoftwareRenderer) => {
                "Blur unavailable: window is using opaque software presentation"
            }
            Self::Unsupported(BackdropUnavailable::ReducedTransparency) => {
                "Blur unavailable: reduced transparency is enabled"
            }
            Self::Unsupported(BackdropUnavailable::Platform) => "Blur unavailable on this platform",
            Self::Unsupported(BackdropUnavailable::UndecoratedWindow) => {
                "Public material needs an AppKit frame; transparency remains available"
            }
            Self::Failed => "Backdrop setup failed; see terminal diagnostics",
        }
    }
}

/// Pure negotiation policy, independently testable without a display server.
#[cfg(any(target_os = "linux", test))]
pub(super) fn negotiate(
    enabled: bool,
    ready: bool,
    modern: Option<bool>,
    legacy: bool,
) -> BackdropStatus {
    use BackdropProtocol::*;
    use BackdropStatus::*;
    if !enabled {
        Disabled
    } else if !ready {
        Discovering
    } else if modern == Some(true) {
        Requested(WaylandBackgroundEffect)
    } else if legacy {
        Requested(WaylandKde)
    } else if modern == Some(false) {
        Unsupported(BackdropUnavailable::CompositorDeclined)
    } else {
        Unsupported(BackdropUnavailable::NoProtocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_scale_regions_are_surface_local_and_bounded() {
        assert_eq!(logical_extent(1800, 1.25), 1440);
        assert_eq!(logical_extent(1080, 1.25), 864);
        assert_eq!(logical_extent(1245, 1.25), 996);
        assert_eq!(logical_extent(739, 1.25), 592);
        assert_eq!(logical_extent(1800, 1.), 1800);
        assert_eq!(logical_extent(0, 1.25), 0);
        assert_eq!(logical_extent(u32::MAX, 0.001), i32::MAX);
        for scale in [0., -1., f64::NAN, f64::INFINITY] {
            assert_eq!(logical_extent(1800, scale), 0);
        }
    }
    #[test]
    fn modern_only_and_legacy_only_compositors_are_supported() {
        assert_eq!(
            negotiate(true, true, Some(true), false),
            BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect)
        );
        assert_eq!(
            negotiate(true, true, None, true),
            BackdropStatus::Requested(BackdropProtocol::WaylandKde)
        );
        assert_eq!(
            negotiate(true, true, Some(true), true),
            BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect)
        );
    }
    #[test]
    fn revocation_falls_back_or_reports_decline_and_can_recover() {
        assert_eq!(
            negotiate(true, true, Some(false), true),
            BackdropStatus::Requested(BackdropProtocol::WaylandKde)
        );
        assert_eq!(
            negotiate(true, true, Some(false), false),
            BackdropStatus::Unsupported(BackdropUnavailable::CompositorDeclined)
        );
        assert_eq!(
            negotiate(true, true, None, false),
            BackdropStatus::Unsupported(BackdropUnavailable::NoProtocol)
        );
        assert_eq!(
            negotiate(true, true, Some(true), false),
            BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect)
        );
    }
    #[test]
    fn disabled_and_discovery_never_claim_an_active_effect() {
        assert_eq!(
            negotiate(false, true, Some(true), true),
            BackdropStatus::Disabled
        );
        assert_eq!(
            negotiate(true, false, None, false),
            BackdropStatus::Discovering
        );
        for status in [
            BackdropStatus::Discovering,
            BackdropStatus::Requested(BackdropProtocol::WaylandBackgroundEffect),
            BackdropStatus::Unsupported(BackdropUnavailable::NoProtocol),
        ] {
            assert!(!status.description().contains("active"));
            assert!(!status.description().contains("Material"));
        }
    }
}
