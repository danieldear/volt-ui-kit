use serde::Deserialize;
/// Version checked by the native host before any pointer-bearing call.
pub const HOST_ABI_VERSION: u32 = 1;

/// Validated platform events, independent of UIKit, JNI or a windowing crate.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlatformEvent {
    Resize { width: f32, height: f32, scale: f32 },
    Begin { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    End { x: f32, y: f32 },
    Cancel {},
    Activate { id: String },
    Focus { id: String },
    Text { id: String, text: String },
    Scroll { id: String, dy: f32 },
    Appearance { dark: bool },
    Suspend { value: bool },
    Draw {},
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
            Self::Begin { x, y } | Self::Move { x, y } | Self::End { x, y } => {
                x.is_finite() && y.is_finite()
            }
            Self::Scroll { dy, .. } => dy.is_finite() && dy.abs() <= 16_384.,
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
            r#"{"op":"run_shell","command":"no"}"#,
            r#"{"op":"resize","width":1e99,"height":400,"scale":3}"#,
            r#"{"op":"scroll","id":"list","dy":20000}"#,
            r#"{"op":"appearance","dark":"yes"}"#,
        ] {
            assert!(PlatformEvent::decode(bad.as_bytes()).is_err(), "{bad}");
        }
        assert!(PlatformEvent::decode(&vec![b' '; 65_537]).is_err());
        assert!(
            PlatformEvent::decode(r#"{"op":"text","id":"name","text":"Zoë 東京"}"#.as_bytes())
                .is_ok()
        );
    }
}
