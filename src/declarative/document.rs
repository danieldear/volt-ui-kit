use super::{responsive::ResponsiveLayout, Direction};
use super::{EventKind, Handlers};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_DOCUMENT_BYTES: usize = 262_144;
pub(crate) const MAX_NODES: usize = 512;
pub(crate) const MAX_DEPTH: usize = 24;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Panel,
    Row,
    Column,
    Stack,
    #[serde(rename = "zstack")]
    ZStack,
    Text,
    Button,
    Checkbox,
    Toggle,
    Badge,
    Separator,
    Spacer,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Body,
    Muted,
    Accent,
    Title,
}
/// Placement of a container's content group, not a wrapping-flow layout.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}
impl Flow {
    pub(crate) fn fractions(self) -> [f32; 2] {
        match self {
            Self::TopLeft => [0., 0.],
            Self::Top => [0.5, 0.],
            Self::TopRight => [1., 0.],
            Self::Left => [0., 0.5],
            Self::Center => [0.5, 0.5],
            Self::Right => [1., 0.5],
            Self::BottomLeft => [0., 1.],
            Self::Bottom => [0.5, 1.],
            Self::BottomRight => [1., 1.],
        }
    }
}
/// Corner radius in logical pixels, or a pill/circle based on laid-out dimensions.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(from = "RoundingInput")]
pub enum Rounding {
    Radius(f32),
    Full,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum RoundingInput {
    Radius(f32),
    Preset(FullRounding),
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FullRounding {
    Full,
}
impl From<RoundingInput> for Rounding {
    fn from(value: RoundingInput) -> Self {
        match value {
            RoundingInput::Radius(v) => Self::Radius(v),
            RoundingInput::Preset(_) => Self::Full,
        }
    }
}
macro_rules! numeric_rounding {
    ($($number:ty),+ $(,)?) => { $(
        impl From<$number> for Rounding {
            fn from(value: $number) -> Self { Self::Radius(value as f32) }
        }
    )+ };
}
numeric_rounding!(f32, f64, i32, i64, isize, u32, u64, usize, i16, u16, i8, u8);
/// Editable visual overrides. Text color and typography inherit; surfaces do not.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodeStyle {
    pub color: Option<String>,
    pub background: Option<String>,
    pub border_color: Option<String>,
    pub accent_color: Option<String>,
    pub border_width: Option<f32>,
    pub radius: Option<Rounding>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub font_weight: Option<u16>,
    pub font_style: Option<FontSlant>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FontSlant {
    Normal,
    Italic,
}
pub(crate) fn parse_color(value: &str) -> Option<[f32; 4]> {
    let hex = value.strip_prefix('#')?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut color = [1.; 4];
    for (index, channel) in color.iter_mut().enumerate().take(hex.len() / 2) {
        *channel = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()? as f32 / 255.;
    }
    Some(color)
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    #[serde(rename = "type")]
    pub(crate) kind: Kind,
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) children: Vec<Node>,
    #[serde(default)]
    pub(crate) width: Option<f32>,
    #[serde(default)]
    pub(crate) height: Option<f32>,
    #[serde(default)]
    pub(crate) flex: f32,
    #[serde(default)]
    pub(crate) padding: f32,
    #[serde(default)]
    pub(crate) gap: f32,
    #[serde(default)]
    pub(crate) padding_horizontal: Option<f32>,
    #[serde(default)]
    pub(crate) padding_vertical: Option<f32>,
    #[serde(default)]
    pub(crate) padding_top: Option<f32>,
    #[serde(default)]
    pub(crate) padding_right: Option<f32>,
    #[serde(default)]
    pub(crate) padding_bottom: Option<f32>,
    #[serde(default)]
    pub(crate) padding_left: Option<f32>,
    #[serde(default)]
    pub(crate) gap_horizontal: Option<f32>,
    #[serde(default)]
    pub(crate) gap_vertical: Option<f32>,
    #[serde(default)]
    pub(crate) align: Align,
    #[serde(default)]
    pub(crate) flow: Option<Flow>,
    #[serde(default)]
    pub(crate) direction: Direction,
    #[serde(skip)]
    pub(crate) responsive: ResponsiveLayout,
    #[serde(default)]
    pub(crate) text: Option<String>,
    #[serde(default)]
    pub(crate) bind_text: Option<String>,
    #[serde(default)]
    pub(crate) label: Option<String>,
    #[serde(default = "yes")]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) bind_enabled: Option<String>,
    #[serde(default)]
    pub(crate) on_activate: Option<String>,
    #[serde(default)]
    pub(crate) font_size: Option<f32>,
    #[serde(default)]
    pub(crate) tone: Tone,
    #[serde(default)]
    pub(crate) primary: bool,
    #[serde(default)]
    pub(crate) checked: bool,
    #[serde(default)]
    pub(crate) bind_checked: Option<String>,
    #[serde(default)]
    pub(crate) on_toggle_changed: Option<String>,
    #[serde(default)]
    pub(crate) style: NodeStyle,
}
fn yes() -> bool {
    true
}
impl Node {
    #[cfg(feature = "rust-ui")]
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            kind,
            id: String::new(),
            children: vec![],
            width: None,
            height: None,
            flex: 0.,
            padding: 0.,
            gap: 0.,
            padding_horizontal: None,
            padding_vertical: None,
            padding_top: None,
            padding_right: None,
            padding_bottom: None,
            padding_left: None,
            gap_horizontal: None,
            gap_vertical: None,
            align: Align::Start,
            flow: None,
            direction: Direction::Vertical,
            responsive: ResponsiveLayout::default(),
            text: None,
            bind_text: None,
            label: None,
            enabled: true,
            bind_enabled: None,
            on_activate: None,
            font_size: None,
            tone: Tone::Body,
            primary: false,
            checked: false,
            bind_checked: None,
            on_toggle_changed: None,
            style: NodeStyle::default(),
        }
    }
    /// Logical insets in top/right/bottom/left order. JSON precedence is explicit
    /// edge > axis > all; Rust modifiers clear any older affected overrides.
    pub(crate) fn padding_sides(&self) -> [f32; 4] {
        let horizontal = self.padding_horizontal.unwrap_or(self.padding);
        let vertical = self.padding_vertical.unwrap_or(self.padding);
        [
            self.padding_top.unwrap_or(vertical),
            self.padding_right.unwrap_or(horizontal),
            self.padding_bottom.unwrap_or(vertical),
            self.padding_left.unwrap_or(horizontal),
        ]
    }
    pub(crate) fn main_gap(&self, axis: usize) -> f32 {
        if axis == 0 {
            self.gap_horizontal.unwrap_or(self.gap)
        } else {
            self.gap_vertical.unwrap_or(self.gap)
        }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn children(&self) -> &[Node] {
        &self.children
    }
}
#[derive(Clone, Debug)]
pub struct Document {
    root: Node,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDocument {
    schema_version: u32,
    root: Node,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentError(pub String);
impl std::fmt::Display for DocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DocumentError {}
fn error(path: &str, reason: &str) -> DocumentError {
    DocumentError(format!("{path}: {reason}"))
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Bool(bool),
}
/// Named, application-owned values. Documents never evaluate expressions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bindings(BTreeMap<String, Value>);
impl Bindings {
    pub fn set_text(&mut self, name: impl Into<String>, text: impl Into<String>) {
        self.0.insert(name.into(), Value::Text(text.into()));
    }
    pub fn set_bool(&mut self, name: impl Into<String>, value: bool) {
        self.0.insert(name.into(), Value::Bool(value));
    }
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }
}
impl Document {
    pub fn root(&self) -> &Node {
        &self.root
    }
    pub fn parse(source: &str) -> Result<Self, DocumentError> {
        if source.len() > MAX_DOCUMENT_BYTES {
            return Err(error("document", "exceeds 256 KiB limit"));
        }
        let raw: serde_json::Value =
            serde_json::from_str(source).map_err(|e| error("document", &e.to_string()))?;
        check_raw(&raw["root"], "root", 0, &mut 0)?;
        let mut doc: RawDocument =
            serde_json::from_value(raw.clone()).map_err(|e| error("document", &e.to_string()))?;
        if doc.schema_version != 1 {
            return Err(error("schema_version", "only version 1 is supported"));
        }
        // This new component is flexible by default in both frontends. Preserve
        // explicit zero (fixed spacers) and every existing component's defaults.
        fn spacer_defaults(node: &mut Node, raw: &serde_json::Value) {
            if node.kind == Kind::Spacer && raw.get("flex").is_none() {
                node.flex = 1.;
            }
            if let Some(children) = raw.get("children").and_then(|v| v.as_array()) {
                for (child, source) in node.children.iter_mut().zip(children) {
                    spacer_defaults(child, source);
                }
            }
        }
        spacer_defaults(&mut doc.root, &raw["root"]);
        let mut ids = BTreeSet::new();
        let mut count = 0;
        check_node(&doc.root, "root", 0, &mut count, &mut ids)?;
        Ok(Self { root: doc.root })
    }
    /// Constructs the same validated tree as JSON, without serializing Rust UI.
    #[cfg(feature = "rust-ui")]
    pub(crate) fn from_node(root: Node) -> Result<Self, DocumentError> {
        check_node(&root, "root", 0, &mut 0, &mut BTreeSet::new())?;
        Ok(Self { root })
    }
    /// Called before installation and after host binding updates.
    pub fn validate_bindings(&self, values: &Bindings) -> Result<(), DocumentError> {
        fn visit(n: &Node, v: &Bindings) -> Result<(), DocumentError> {
            for (binding, text) in [
                (&n.bind_text, true),
                (&n.bind_enabled, false),
                (&n.bind_checked, false),
            ] {
                if let Some(name) = binding {
                    let valid = match (v.get(name), text) {
                        (Some(Value::Text(value)), true) => value.len() <= 8192,
                        (Some(Value::Bool(_)), false) => true,
                        _ => false,
                    };
                    if !valid {
                        return Err(error(
                            &n.id,
                            &format!("missing, incorrectly typed or oversized binding `{name}`"),
                        ));
                    }
                }
            }
            for child in &n.children {
                visit(child, v)?;
            }
            Ok(())
        }
        visit(&self.root, values)
    }
    pub fn validate_connections<C>(
        &self,
        handlers: &Handlers<'_, C>,
        values: &Bindings,
    ) -> Result<(), DocumentError> {
        fn visit<C>(
            n: &Node,
            handlers: &Handlers<'_, C>,
            values: &Bindings,
        ) -> Result<(), DocumentError> {
            for (name, kind) in [
                (&n.on_activate, EventKind::Activate),
                (&n.on_toggle_changed, EventKind::ToggleChanged),
            ] {
                if let Some(name) = name {
                    handlers
                        .validate(name, kind)
                        .map_err(|e| error(&n.id, &e.to_string()))?;
                }
            }
            for (binding, text) in [
                (&n.bind_text, true),
                (&n.bind_enabled, false),
                (&n.bind_checked, false),
            ] {
                if let Some(name) = binding {
                    let valid = matches!(
                        (values.get(name), text),
                        (Some(Value::Text(_)), true) | (Some(Value::Bool(_)), false)
                    );
                    if !valid {
                        return Err(error(
                            &n.id,
                            &format!("missing or incorrectly typed binding `{name}`"),
                        ));
                    }
                }
            }
            for child in &n.children {
                visit(child, handlers, values)?;
            }
            Ok(())
        }
        self.validate_bindings(values)?;
        visit(&self.root, handlers, values)
    }
}
fn check_raw(
    raw: &serde_json::Value,
    path: &str,
    depth: usize,
    count: &mut usize,
) -> Result<(), DocumentError> {
    *count += 1;
    if depth > MAX_DEPTH || *count > MAX_NODES {
        return Err(error(path, "node count or depth limit exceeded"));
    }
    let obj = raw
        .as_object()
        .ok_or_else(|| error(path, "expected node object"))?;
    let kind = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| error(path, "type must be a string"))?;
    if !matches!(
        kind,
        "panel"
            | "row"
            | "column"
            | "stack"
            | "zstack"
            | "text"
            | "button"
            | "checkbox"
            | "toggle"
            | "badge"
            | "separator"
            | "spacer"
    ) {
        return Err(error(path, &format!("unknown component `{kind}`")));
    }
    for (key, value) in obj {
        let common = matches!(
            key.as_str(),
            "type" | "id" | "width" | "height" | "flex" | "style"
        );
        let allowed = common
            || match kind {
                "panel" | "row" | "column" | "stack" | "zstack" => {
                    matches!(
                        key.as_str(),
                        "children"
                            | "padding"
                            | "gap"
                            | "align"
                            | "flow"
                            | "padding_horizontal"
                            | "padding_vertical"
                            | "padding_top"
                            | "padding_right"
                            | "padding_bottom"
                            | "padding_left"
                            | "gap_horizontal"
                            | "gap_vertical"
                    ) || (kind == "stack" && key == "direction")
                }
                "text" | "badge" => {
                    matches!(key.as_str(), "text" | "bind_text" | "font_size" | "tone")
                }
                "button" => matches!(
                    key.as_str(),
                    "label" | "enabled" | "bind_enabled" | "on_activate" | "primary"
                ),
                "checkbox" | "toggle" => matches!(
                    key.as_str(),
                    "label"
                        | "enabled"
                        | "bind_enabled"
                        | "checked"
                        | "bind_checked"
                        | "on_toggle_changed"
                ),
                _ => false,
            };
        if !allowed {
            return Err(error(&format!("{path}.{key}"), "unsupported property"));
        }
        let valid = match key.as_str() {
            "width" | "height" | "flex" | "padding" | "gap" | "font_size"
            | "padding_horizontal" | "padding_vertical" | "padding_top" | "padding_right"
            | "padding_bottom" | "padding_left" | "gap_horizontal" | "gap_vertical" => {
                value.is_number()
            }
            "enabled" | "primary" | "checked" => value.is_boolean(),
            "children" => value.is_array(),
            "style" => value.is_object(),
            _ => value.is_string(),
        };
        if !valid {
            return Err(error(&format!("{path}.{key}"), "incorrect property type"));
        }
    }
    if matches!(kind, "checkbox" | "toggle")
        && obj.contains_key("checked")
        && obj.contains_key("bind_checked")
    {
        return Err(error(
            path,
            "checked and bind_checked are mutually exclusive",
        ));
    }
    if let Some(style) = obj.get("style") {
        check_style_raw(style, &format!("{path}.style"))?;
    }
    if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
        for (i, child) in children.iter().enumerate() {
            check_raw(child, &format!("{path}.children[{i}]"), depth + 1, count)?;
        }
    }
    Ok(())
}
fn check_style_raw(raw: &serde_json::Value, path: &str) -> Result<(), DocumentError> {
    let obj = raw
        .as_object()
        .ok_or_else(|| error(path, "expected style object"))?;
    for (key, value) in obj {
        let field = format!("{path}.{key}");
        let valid = match key.as_str() {
            "color" | "background" | "border_color" | "accent_color" => {
                value.as_str().and_then(parse_color).is_some()
            }
            "font_family" => value.as_str().is_some_and(|v| {
                !v.trim().is_empty() && v.len() <= 128 && !v.chars().any(char::is_control)
            }),
            "font_size" => value.as_f64().is_some_and(|v| (8. ..=96.).contains(&v)),
            "border_width" => value.as_f64().is_some_and(|v| (0. ..=16.).contains(&v)),
            "radius" => {
                value.as_str() == Some("full")
                    || value.as_f64().is_some_and(|v| (0. ..=256.).contains(&v))
            }
            "font_weight" => value
                .as_u64()
                .is_some_and(|v| (100..=900).contains(&v) && v % 100 == 0),
            "font_style" => value
                .as_str()
                .is_some_and(|v| matches!(v, "normal" | "italic")),
            _ => return Err(error(&field, "unsupported style property")),
        };
        if !valid {
            return Err(error(&field, "invalid style value"));
        }
    }
    Ok(())
}
fn check_style(style: &NodeStyle, path: &str) -> Result<(), DocumentError> {
    for (name, value) in [
        ("color", &style.color),
        ("background", &style.background),
        ("border_color", &style.border_color),
        ("accent_color", &style.accent_color),
    ] {
        if value.as_ref().is_some_and(|v| parse_color(v).is_none()) {
            return Err(error(&format!("{path}.{name}"), "invalid style value"));
        }
    }
    if style
        .font_family
        .as_ref()
        .is_some_and(|v| v.trim().is_empty() || v.len() > 128 || v.chars().any(char::is_control))
    {
        return Err(error(&format!("{path}.font_family"), "invalid style value"));
    }
    for (name, value, min, max) in [
        ("font_size", style.font_size, 8., 96.),
        ("border_width", style.border_width, 0., 16.),
    ] {
        if value.is_some_and(|v| !v.is_finite() || !(min..=max).contains(&v)) {
            return Err(error(&format!("{path}.{name}"), "invalid style value"));
        }
    }
    if let Some(Rounding::Radius(value)) = style.radius {
        if !value.is_finite() || !(0. ..=256.).contains(&value) {
            return Err(error(&format!("{path}.radius"), "invalid style value"));
        }
    }
    if style
        .font_weight
        .is_some_and(|v| !(100..=900).contains(&v) || v % 100 != 0)
    {
        return Err(error(&format!("{path}.font_weight"), "invalid style value"));
    }
    Ok(())
}
fn check_node(
    n: &Node,
    path: &str,
    depth: usize,
    count: &mut usize,
    ids: &mut BTreeSet<String>,
) -> Result<(), DocumentError> {
    *count += 1;
    if *count > MAX_NODES || depth > MAX_DEPTH {
        return Err(error(path, "node count or depth limit exceeded"));
    }
    if !valid_name(&n.id) || !ids.insert(n.id.clone()) {
        return Err(error(path, "invalid or duplicate id"));
    }
    for (name, value, positive) in [
        ("width", n.width, false),
        ("height", n.height, false),
        ("flex", Some(n.flex), false),
        ("padding", Some(n.padding), false),
        ("gap", Some(n.gap), false),
        ("padding_horizontal", n.padding_horizontal, false),
        ("padding_vertical", n.padding_vertical, false),
        ("padding_top", n.padding_top, false),
        ("padding_right", n.padding_right, false),
        ("padding_bottom", n.padding_bottom, false),
        ("padding_left", n.padding_left, false),
        ("gap_horizontal", n.gap_horizontal, false),
        ("gap_vertical", n.gap_vertical, false),
        ("font_size", n.font_size, true),
    ] {
        if let Some(v) = value {
            if !v.is_finite()
                || !(0. ..=16_384.).contains(&v)
                || (positive && !(8. ..=96.).contains(&v))
            {
                return Err(error(path, &format!("invalid {name}")));
            }
        }
    }
    check_style(&n.style, &format!("{path}.style"))?;
    n.responsive.validate(path)?;
    if n.text.as_ref().is_some_and(|v| v.len() > 8192)
        || n.label.as_ref().is_some_and(|v| v.len() > 8192)
    {
        return Err(error(path, "text exceeds 8 KiB limit"));
    }
    if matches!(n.kind, Kind::Text | Kind::Badge) && (n.text.is_some() == n.bind_text.is_some()) {
        return Err(error(
            path,
            "Text/Badge requires exactly one of text or bind_text",
        ));
    }
    if n.kind == Kind::Button && (n.label.is_none() || n.on_activate.is_none()) {
        return Err(error(path, "Button requires label and on_activate"));
    }
    if matches!(n.kind, Kind::Checkbox | Kind::Toggle)
        && (n.label.is_none() || n.on_toggle_changed.is_none())
    {
        return Err(error(
            path,
            "Checkbox/Toggle requires label and on_toggle_changed",
        ));
    }
    for name in [
        &n.bind_text,
        &n.bind_enabled,
        &n.on_activate,
        &n.bind_checked,
        &n.on_toggle_changed,
    ]
    .into_iter()
    .flatten()
    {
        if !valid_name(name) {
            return Err(error(path, "invalid binding or handler name"));
        }
    }
    for (i, child) in n.children.iter().enumerate() {
        check_node(
            child,
            &format!("{path}.children[{i}]"),
            depth + 1,
            count,
            ids,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(n: &str) -> Result<Document, DocumentError> {
        Document::parse(&format!(r#"{{"schema_version":1,"root":{n}}}"#))
    }
    #[test]
    fn strict_properties_and_ids() {
        assert!(parse(r#"{"type":"text","id":"a","text":"ok"}"#).is_ok());
        for n in [
            r#"{"type":"text","id":"a","text":"ok","typo":1}"#,
            r#"{"type":"text","id":"a","text":"ok","children":[]}"#,
            r#"{"type":"column","id":"a","children":[{"type":"column","id":"a"}]}"#,
            r#"{"type":"button","id":"a","label":"Save"}"#,
            r#"{"type":"column","id":"a","padding":-1}"#,
            r#"{"type":"text","id":"a","text":"hi","bind_text":"x"}"#,
        ] {
            assert!(parse(n).is_err(), "{n}");
        }
        assert!(
            Document::parse(r#"{"schema_version":2,"root":{"type":"column","id":"a"}}"#).is_err()
        );
        assert!(Document::parse("{").unwrap_err().0.contains("line"));
    }
    #[test]
    fn bindings_and_signatures_are_validated() {
        let d = parse(r#"{"type":"button","id":"a","label":"Save","on_activate":"save","bind_enabled":"ready"}"#).unwrap();
        let mut h = Handlers::default();
        let mut b = Bindings::default();
        assert!(d.validate_connections(&h, &b).is_err());
        h.on_activate("save", |_: &mut ()| {}).unwrap();
        b.set_text("ready", "wrong");
        assert!(d.validate_connections(&h, &b).is_err());
        b.set_bool("ready", true);
        assert!(d.validate_connections(&h, &b).is_ok());
    }
    #[test]
    fn bounded_depth_and_size() {
        assert!(Document::parse(&" ".repeat(MAX_DOCUMENT_BYTES + 1)).is_err());
        let mut n = serde_json::json!({"type":"column","id":"leaf"});
        for i in 0..26 {
            n = serde_json::json!({"type":"column","id":format!("node{i}"),"children":[n]});
        }
        assert!(
            Document::parse(&serde_json::json!({"schema_version":1,"root":n}).to_string()).is_err()
        );
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn font_and_node_budget() {
        for size in [0.0000001, 0., 7., 97., 16384.] {
            let doc = serde_json::json!({"schema_version":1,"root":{"type":"text","id":"t","text":"Budget","font_size":size}});
            assert!(Document::parse(&doc.to_string()).is_err());
        }
        let children: Vec<_> = (0..513)
            .map(|i| serde_json::json!({"type":"text","id":format!("t{i}"),"text":"x"}))
            .collect();
        assert!(Document::parse(&serde_json::json!({"schema_version":1,"root":{"type":"column","id":"root","children":children}}).to_string()).is_err());
    }
    #[test]
    fn property_errors_have_tree_path() {
        let err=Document::parse(r#"{"schema_version":1,"root":{"type":"column","id":"root","children":[{"type":"text","id":"t","text":"hi","typo":2}]}}"#).unwrap_err();
        assert!(err.0.contains("root.children[0].typo"));
    }
}

#[cfg(test)]
mod style_component_tests {
    use super::*;
    fn parse(node: serde_json::Value) -> Result<Document, DocumentError> {
        Document::parse(&serde_json::json!({"schema_version":1,"root":node}).to_string())
    }
    #[test]
    fn styles_are_strict_and_errors_name_the_field() {
        let valid = serde_json::json!({"type":"text","id":"text","text":"Styled","font_size":24,"style":{"color":"#c0ffee80","background":"#112233","border_color":"#aabbcc","accent_color":"#ff886680","border_width":1.5,"radius":8,"font_family":"Avenir Next","font_size":20,"font_weight":600,"font_style":"italic"}});
        assert!(parse(valid.clone()).is_ok());
        for (name, value) in [
            ("color", serde_json::json!("red")),
            ("color", serde_json::json!("#xyzxyz")),
            ("color", serde_json::json!("#12345")),
            ("color", serde_json::Value::Null),
            ("accent_color", serde_json::json!("amber")),
            ("font_size", serde_json::json!(7.)),
            ("font_size", serde_json::json!(97.)),
            ("font_family", serde_json::json!(" ")),
            ("font_family", serde_json::json!("bad\nfont")),
            ("font_family", serde_json::json!("a".repeat(129))),
            ("font_weight", serde_json::json!(450)),
            ("font_weight", serde_json::json!(1000)),
            ("font_weight", serde_json::json!(400.5)),
            ("font_style", serde_json::json!("oblique")),
            ("border_width", serde_json::json!(-1)),
            ("border_width", serde_json::json!(17)),
            ("radius", serde_json::json!(257)),
            ("unknown", serde_json::json!(true)),
        ] {
            let mut node = valid.clone();
            node["style"][name] = value;
            let err = parse(node).unwrap_err();
            assert!(err.0.contains(&format!("root.style.{name}")), "{err}");
        }
        assert_eq!(
            parse_color("#ff008080"),
            Some([1., 0., 128. / 255., 128. / 255.])
        );
        assert_eq!(
            parse_color("#112233"),
            Some([17. / 255., 34. / 255., 51. / 255., 1.])
        );
    }
    #[test]
    fn checkable_components_validate_typed_bindings_and_handlers() {
        for kind in ["checkbox", "toggle"] {
            let node = serde_json::json!({"type":kind,"id":"check","label":"Checked","bind_checked":"value","on_toggle_changed":"change"});
            let doc = parse(node.clone()).unwrap();
            let mut wrong = Handlers::<()>::default();
            wrong.on_activate("change", |_| {}).unwrap();
            let mut values = Bindings::default();
            values.set_bool("value", false);
            assert!(doc.validate_connections(&wrong, &values).is_err());
            let mut correct = Handlers::<()>::default();
            correct.on_toggle_changed("change", |_, _| {}).unwrap();
            assert!(doc.validate_connections(&correct, &values).is_ok());
            values.set_text("value", "not boolean");
            assert!(doc.validate_connections(&correct, &values).is_err());
            let mut invalid = node.clone();
            invalid["checked"] = serde_json::json!(true);
            assert!(parse(invalid).is_err());
            let mut missing = node;
            missing.as_object_mut().unwrap().remove("on_toggle_changed");
            assert!(parse(missing).is_err());
        }
        assert!(parse(serde_json::json!({"type":"badge","id":"badge","text":"Beta"})).is_ok());
        assert!(parse(serde_json::json!({"type":"separator","id":"separator","style":{"background":"#334455"}})).is_ok());
        assert!(parse(
            serde_json::json!({"type":"badge","id":"badge","on_activate":"x","text":"Beta"})
        )
        .is_err());
        assert!(
            parse(serde_json::json!({"type":"separator","id":"separator","text":"line"})).is_err()
        );
    }
}
