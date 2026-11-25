use serde::Deserialize;
/// Version checked by the native host before any pointer-bearing call.
pub const HOST_ABI_VERSION: u32 = 2;

/// Colours the platform resolved for the current appearance and contrast
/// setting, as sRGB RGBA from 0 to 1. On iOS these are UIKit's grouped
/// system colours, so Volt content matches the rest of the phone.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SystemColors {
    /// The screen behind grouped content.
    pub background: [f32; 4],
    /// Rows, cards and fields on that background.
    pub surface: [f32; 4],
    /// Controls and pressed rows on a surface.
    pub raised: [f32; 4],
    pub label: [f32; 4],
    pub secondary_label: [f32; 4],
    /// Hairlines between rows; usually translucent.
    pub separator: [f32; 4],
    /// The app's tint (accent) colour.
    pub tint: [f32; 4],
    pub success: [f32; 4],
    pub warning: [f32; 4],
    pub error: [f32; 4],
}
impl SystemColors {
    fn valid(&self) -> bool {
        [
            self.background,
            self.surface,
            self.raised,
            self.label,
            self.secondary_label,
            self.separator,
            self.tint,
            self.success,
            self.warning,
            self.error,
        ]
        .iter()
        .flatten()
        .all(|c| c.is_finite() && (0. ..=1.).contains(c))
    }
}

/// Validated platform events, independent of UIKit, JNI or a windowing crate.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlatformEvent {
    Resize {
        width: f32,
        height: f32,
        scale: f32,
    },
    Begin {
        x: f32,
        y: f32,
    },
    Move {
        x: f32,
        y: f32,
    },
    End {
        x: f32,
        y: f32,
    },
    /// The finger rested in place: a long press.
    Hold {
        x: f32,
        y: f32,
    },
    Cancel {},
    Activate {
        id: String,
    },
    /// A screen reader's "Show menu": open the long-press menu for `id`.
    Menu {
        id: String,
    },
    Focus {
        id: String,
    },
    /// Typing ended (Done, or the keyboard was put away): nothing is focused.
    Blur {},
    /// The keyboard's Search (or return) key in field `id`.
    Submit {
        id: String,
    },
    Text {
        id: String,
        text: String,
    },
    Scroll {
        id: String,
        dy: f32,
    },
    /// A platform scroll view's position, in points; past an end while bouncing.
    ScrollTo {
        id: String,
        x: f32,
        y: f32,
    },
    /// What the host provides. Sent once after creation; older hosts omit it.
    Host {
        native_scrolling: bool,
        #[serde(default)]
        native_navigation: bool,
        /// The host draws switches and sliders as platform controls.
        #[serde(default)]
        native_controls: bool,
    },
    /// A platform slider moved.
    Value {
        id: String,
        value: f32,
    },
    /// Space a platform scroll view keeps above and below its content: the
    /// bars it scrolls under. `scroll_to` positions then include it.
    ScrollInsets {
        id: String,
        top: f32,
        bottom: f32,
    },
    /// The host's navigation bar ran a control: Back or a bar button.
    Navigation {
        id: String,
    },
    /// A platform scroll view was pulled past its top and let go: its
    /// container's `on_refresh` runs.
    Refresh {
        id: String,
    },
    /// `colors` and `reduce_motion` are optional so older hosts keep working;
    /// motion stays reduced until a host with a frame loop sends `false`.
    Appearance {
        dark: bool,
        #[serde(default)]
        colors: Option<SystemColors>,
        #[serde(default)]
        reduce_motion: Option<bool>,
    },
    Suspend {
        value: bool,
    },
    Draw {},
    /// The screen's safe area in points, when the view runs under the
    /// status bar and home indicator: overlays stay inside it.
    SafeArea {
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
    },
    /// How far the host's own bottom bars reach up the view, in points: the
    /// tab bar and home indicator, or the keyboard when it is taller.
    Bars {
        bottom: f32,
        /// The keyboard alone: sheets rise above it.
        #[serde(default)]
        keyboard: f32,
    },
    /// Changes sent through an `Updater` are waiting: apply them and draw.
    Updates {},
    Back {},
}
impl PlatformEvent {
    /// Shared bounded wire decoder; native adapters must not invent defaults for
    /// missing IDs, wrong types, unknown operations or unknown fields.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > 65_536 {
            return Err("platform event must be 1–65536 bytes".into());
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let finite = match &value {
            Self::Resize {
                width,
                height,
                scale,
            } => [*width, *height, *scale].iter().all(|v| v.is_finite()),
            Self::Begin { x, y }
            | Self::Move { x, y }
            | Self::End { x, y }
            | Self::Hold { x, y } => x.is_finite() && y.is_finite(),
            Self::Scroll { dy, .. } => dy.is_finite() && dy.abs() <= 16_384.,
            Self::Value { value, .. } => value.is_finite(),
            Self::ScrollInsets { top, bottom, .. } => [top, bottom]
                .iter()
                .all(|v| v.is_finite() && (0. ..=8192.).contains(*v)),
            Self::ScrollTo { x, y, .. } => [x, y]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_048_576.),
            Self::Appearance {
                colors: Some(c), ..
            } => c.valid(),
            _ => true,
        };
        if !finite {
            return Err("non-finite or excessive platform geometry".into());
        }
        Ok(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_decoder_rejects_malformed_unbounded_and_unknown_events() {
        for bad in [
            r#"{}"#,
            r#"{"op":"activate"}"#,
            r#"{"op":"draw","extra":true}"#,
            r#"{"op":"back","extra":true}"#,
            r#"{"op":"run_shell","command":"no"}"#,
            r#"{"op":"resize","width":1e99,"height":400,"scale":3}"#,
            r#"{"op":"scroll","id":"list","dy":20000}"#,
            r#"{"op":"scroll_to","id":"list","x":0,"y":1e9}"#,
            r#"{"op":"scroll_to","id":"list","y":10}"#,
            r#"{"op":"host"}"#,
            r#"{"op":"value","id":"volume","value":1e999}"#,
            r#"{"op":"scroll_insets","id":"list","top":-1,"bottom":0}"#,
            r#"{"op":"appearance","dark":"yes"}"#,
            r#"{"op":"appearance","dark":true,"reduce_motion":"no"}"#,
            r#"{"op":"appearance","dark":true,"colors":{"background":[0,0,0,1]}}"#,
        ] {
            assert!(PlatformEvent::decode(bad.as_bytes()).is_err(), "{bad}");
        }
        assert!(PlatformEvent::decode(&vec![b' '; 65_537]).is_err());
        assert!(PlatformEvent::decode(br#"{"op":"appearance","dark":true}"#).is_ok());
        let c = "[0.1,0.2,0.3,1]";
        let colors = |bad: &str| {
            format!(
                r#"{{"op":"appearance","dark":false,"reduce_motion":false,"colors":{{"background":{bad},"surface":{c},"raised":{c},"label":{c},"secondary_label":{c},"separator":{c},"tint":{c},"success":{c},"warning":{c},"error":{c}}}}}"#
            )
        };
        assert!(PlatformEvent::decode(colors(c).as_bytes()).is_ok());
        assert!(PlatformEvent::decode(colors("[2,0,0,1]").as_bytes()).is_err());
        assert!(
            PlatformEvent::decode(r#"{"op":"text","id":"name","text":"Zoë 東京"}"#.as_bytes())
                .is_ok()
        );
    }
}
