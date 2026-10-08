//! Rust-first UI: ordinary Rust functions, state, conditionals and typed closures.
//!
//! The native host calls `view(&state)` initially and after dispatched actions.
//! This is whole-tree composition, not dependency-tracked signals. Do not perform
//! I/O or mutate state in view bodies. Callback captures must be owned (`'static`).
//!
//! ```
//! use volt_ui_kit::ui::{column, prelude::*};
//! #[derive(Default)]
//! struct App { count: usize }
//! fn view(app: &App) -> Element<App> {
//!     column![
//!         text(format!("Count: {}", app.count)).font_size(24.),
//!         button("Add").key("add").on_click(|app: &mut App| app.count += 1),
//!     ].padding(24., Edge::All).gap(12., Axis::Both).into()
//! }
//! let (document, handlers) = view(&App::default()).build()?.into_parts();
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
//! Directional spacing uses logical pixels; these are property setters (last
//! write to an edge/axis wins), not accumulating wrapper layers:
//! ```
//! use volt_ui_kit::ui::{column, prelude::*};
//! let screen: Element<()> = column![
//!     text("Title"),
//!     row![text("Left"), text("Right")].gap(12., Axis::Horizontal),
//! ].padding(24., Edge::Horizontal).padding(16., Edge::Vertical).padding(8., Edge::Top)
//!  .gap(10., Axis::Vertical).into();
//! screen.build()?;
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
//! Adaptive direction belongs to `stack!`, not the fixed-direction aliases:
//! ```compile_fail
//! use volt_ui_kit::ui::prelude::*;
//! vstack![text::<()>("Fixed vertical")].direction(Direction::Horizontal);
//! ```
//! Responsive properties resolve during layout without composing new callbacks:
//! ```
//! use volt_ui_kit::ui::prelude::*;
//! let screen: Element<()> = stack![
//!     text("Navigation"),
//!     zstack![text("Back"), badge("Front")].height(80.),
//! ].direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
//!  .padding(responsive(12.).at(Breakpoint::Md, 24.), Edge::Horizontal)
//!  .gap(responsive(8.).at(Breakpoint::Md, 16.), Axis::Both).into();
//! screen.build()?;
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
//!
//! Button callbacks cannot accept a boolean change value:
//! ```compile_fail
//! use volt_ui_kit::ui::{column, prelude::*};
//! button("Add").on_click(|_: &mut (), _: bool| {});
//! ```
//! Checkable callbacks require the proposed boolean value:
//! ```compile_fail
//! use volt_ui_kit::ui::{column, prelude::*};
//! toggle("Preview", false).on_change(|_: &mut ()| {});
//! ```
//! Component-only modifiers remain typed:
//! ```compile_fail
//! use volt_ui_kit::ui::{column, prelude::*};
//! text::<()>("Title").on_click(|_: &mut ()| {});
//! ```
//! Gaps select an axis, not an outer edge:
//! ```compile_fail
//! use volt_ui_kit::ui::prelude::*;
//! vstack![text::<()>("Title")].gap(12., Edge::Top);
//! ```
//! Familiar stack aliases and content placement work inside any container:
//! ```
//! use volt_ui_kit::ui::prelude::*;
//! let screen: Element<()> = panel![
//!     vstack![
//!         text("Centered content").font_weight(700),
//!         hstack![text("Left"), spacer(), badge("Right").rounded(Rounding::Full)]
//!             .width(220.).gap(12., Axis::Horizontal),
//!         divider(),
//!     ].gap(12., Axis::Vertical).flow(Flow::Center),
//! ].height(240.).padding(20.5, Edge::Horizontal).flow(Flow::Center).rounded(4).into();
//! screen.build()?;
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
use crate::declarative::{
    document::{FontSlant, MAX_DEPTH, MAX_NODES},
    Bindings, Document, DocumentError, Handlers, Kind, Node,
};
pub use crate::declarative::{
    responsive, Align, Breakpoint, Direction, Flow, Responsive, Rounding, Tone,
};

/// Inner edges selected by `.padding(value, edge)`; left/right are absolute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    All,
    Horizontal,
    Vertical,
    Top,
    Right,
    Bottom,
    Left,
}
/// Inter-child gap axes; edges are deliberately not accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Both,
    Horizontal,
    Vertical,
}

/// Explicit normal/italic font selection. The host resolves available families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
}

type Click<C> = Box<dyn FnMut(&mut C)>;
type Change<C> = Box<dyn FnMut(&mut C, bool)>;
enum Callback<C> {
    Click(Click<C>),
    Change(Change<C>),
}
/// A composed, owned UI description. No callback is run while building it.
pub struct Element<C> {
    node: Node,
    auto_id: bool,
    children: Vec<Element<C>>,
    callback: Option<Callback<C>>,
}
impl<C> Element<C> {
    fn new(kind: Kind) -> Self {
        Self {
            node: Node::new(kind),
            auto_id: true,
            children: vec![],
            callback: None,
        }
    }
}
/// Validated layout tree and its directly connected Rust closures.
pub struct BuiltUi<C> {
    document: Document,
    handlers: Handlers<'static, C>,
}
impl<C> BuiltUi<C> {
    pub fn into_parts(self) -> (Document, Handlers<'static, C>) {
        (self.document, self.handlers)
    }
}
impl<C: 'static> Element<C> {
    /// Lower to the shared runtime directly; no JSON serialization or parsing.
    /// Rejects missing callbacks, duplicate keys and invalid geometry/styles.
    pub fn build(self) -> Result<BuiltUi<C>, DocumentError> {
        fn lower<C: 'static>(
            element: Element<C>,
            path: String,
            depth: usize,
            count: &mut usize,
            handlers: &mut Handlers<'static, C>,
        ) -> Result<Node, DocumentError> {
            *count += 1;
            if depth > MAX_DEPTH || *count > MAX_NODES {
                return Err(DocumentError(format!(
                    "{path}: node count or depth limit exceeded"
                )));
            }
            let Element {
                mut node,
                auto_id,
                children,
                callback,
            } = element;
            if auto_id {
                node.id = path.clone();
            }
            if let Some(callback) = callback {
                let name = format!("rust-action-{}", *count);
                match callback {
                    Callback::Click(f) => {
                        handlers
                            .on_activate(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        node.on_activate = Some(name);
                    }
                    Callback::Change(f) => {
                        handlers
                            .on_toggle_changed(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        node.on_toggle_changed = Some(name);
                    }
                }
            }
            node.children = children
                .into_iter()
                .enumerate()
                .map(|(i, child)| lower(child, format!("{path}-{i}"), depth + 1, count, handlers))
                .collect::<Result<_, _>>()?;
            Ok(node)
        }
        let mut handlers = Handlers::default();
        let root = lower(self, "auto-root".into(), 0, &mut 0, &mut handlers)?;
        let document = Document::from_node(root)?;
        document.validate_connections(&handlers, &Bindings::default())?;
        Ok(BuiltUi { document, handlers })
    }
}

pub struct Text<C>(Element<C>);
pub struct Badge<C>(Element<C>);
pub struct Separator<C>(Element<C>);
pub struct Spacer<C>(Element<C>);
pub struct Button<C>(Element<C>);
pub struct Checkable<C>(Element<C>);
pub struct Container<C>(Element<C>);
/// A stack whose horizontal/vertical direction can change at viewport breakpoints.
pub struct Stack<C>(Element<C>);
macro_rules! element_from {
    ($($name:ident),+ $(,)?) => { $(
        impl<C> From<$name<C>> for Element<C> {
            fn from(value: $name<C>) -> Self { value.0 }
        }
    )+ };
}
element_from!(Text, Badge, Separator, Spacer, Button, Checkable, Container, Stack);

// Deliberately common visual/layout modifiers, not a stringly property map.
macro_rules! modifiers {
    ($target:ident, [$($field:tt)*], [$($auto:tt)*]) => {
        impl<C> $target<C> {
            /// Stable, globally unique identity. Required on dynamic interactive items.
            /// Must be 1..=128 ASCII letters/digits/underscore/hyphen; checked at build.
            pub fn key(mut self, key: impl Into<String>) -> Self {
                self.$($field)*.id = key.into(); self.$($auto)* = false; self
            }
            pub fn width(mut self, value: f32) -> Self { self.$($field)*.width = Some(value); self }
            pub fn height(mut self, value: f32) -> Self { self.$($field)*.height = Some(value); self }
            pub fn flex(mut self, value: f32) -> Self { self.$($field)*.flex = value; self }
            pub fn color(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.color = Some(value.into()); self }
            pub fn background(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.background = Some(value.into()); self }
            pub fn border_color(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.border_color = Some(value.into()); self }
            pub fn accent_color(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.accent_color = Some(value.into()); self }
            pub fn border_width(mut self, value: f32) -> Self { self.$($field)*.style.border_width = Some(value); self }
            /// Numeric radius compatibility alias; prefer rounded(number/preset).
            pub fn radius(self, value: f32) -> Self { self.rounded(value) }
            /// Numeric logical radius or Rounding::Full (resolved after layout).
            pub fn rounded(mut self, value: impl Into<Rounding>) -> Self {
                self.$($field)*.style.radius = Some(value.into()); self
            }
            pub fn font_family(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.font_family = Some(value.into()); self }
            pub fn font_size(mut self, value: f32) -> Self { self.$($field)*.style.font_size = Some(value); self }
            pub fn font_weight(mut self, value: u16) -> Self { self.$($field)*.style.font_weight = Some(value); self }
            pub fn font_style(mut self, value: FontStyle) -> Self {
                self.$($field)*.style.font_style = Some(match value { FontStyle::Normal => FontSlant::Normal, FontStyle::Italic => FontSlant::Italic }); self
            }
            pub fn italic(self, value: bool) -> Self { self.font_style(if value { FontStyle::Italic } else { FontStyle::Normal }) }
        }
    };
}
modifiers!(Element, [node], [auto_id]);
modifiers!(Text, [0.node], [0.auto_id]);
modifiers!(Badge, [0.node], [0.auto_id]);
modifiers!(Separator, [0.node], [0.auto_id]);
modifiers!(Spacer, [0.node], [0.auto_id]);
modifiers!(Button, [0.node], [0.auto_id]);
modifiers!(Checkable, [0.node], [0.auto_id]);
modifiers!(Container, [0.node], [0.auto_id]);
modifiers!(Stack, [0.node], [0.auto_id]);

pub fn text<C>(value: impl Into<String>) -> Text<C> {
    let mut element = Element::new(Kind::Text);
    element.node.text = Some(value.into());
    Text(element)
}
pub fn badge<C>(value: impl Into<String>) -> Badge<C> {
    let mut element = Element::new(Kind::Badge);
    element.node.text = Some(value.into());
    Badge(element)
}
pub fn separator<C>() -> Separator<C> {
    Separator(Element::new(Kind::Separator))
}
/// Horizontal divider; separator() remains an equivalent compatibility name.
pub fn divider<C>() -> Separator<C> {
    separator()
}
/// A noninteractive flexible space along its parent's main layout axis.
/// For fixed space use `.flex(0.).width(...)` / `.height(...)`.
pub fn spacer<C>() -> Spacer<C> {
    let mut element = Element::new(Kind::Spacer);
    element.node.flex = 1.;
    Spacer(element)
}
pub fn button<C>(label: impl Into<String>) -> Button<C> {
    let mut element = Element::new(Kind::Button);
    element.node.label = Some(label.into());
    Button(element)
}
fn checkable<C>(kind: Kind, label: impl Into<String>, checked: bool) -> Checkable<C> {
    let mut element = Element::new(kind);
    element.node.label = Some(label.into());
    element.node.checked = checked;
    Checkable(element)
}
pub fn checkbox<C>(label: impl Into<String>, checked: bool) -> Checkable<C> {
    checkable(Kind::Checkbox, label, checked)
}
pub fn toggle<C>(label: impl Into<String>, checked: bool) -> Checkable<C> {
    checkable(Kind::Toggle, label, checked)
}
impl<C> Text<C> {
    pub fn tone(mut self, value: Tone) -> Self {
        self.0.node.tone = value;
        self
    }
}
impl<C> Badge<C> {
    pub fn tone(mut self, value: Tone) -> Self {
        self.0.node.tone = value;
        self
    }
}
impl<C> Button<C> {
    pub fn enabled(mut self, value: bool) -> Self {
        self.0.node.enabled = value;
        self
    }
    pub fn primary(mut self, value: bool) -> Self {
        self.0.node.primary = value;
        self
    }
    pub fn on_click(mut self, callback: impl FnMut(&mut C) + 'static) -> Self {
        self.0.callback = Some(Callback::Click(Box::new(callback)));
        self
    }
}
impl<C> Checkable<C> {
    pub fn enabled(mut self, value: bool) -> Self {
        self.0.node.enabled = value;
        self
    }
    pub fn on_change(mut self, callback: impl FnMut(&mut C, bool) + 'static) -> Self {
        self.0.callback = Some(Callback::Change(Box::new(callback)));
        self
    }
}
fn container<C>(kind: Kind, children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    let mut element = Element::new(kind);
    element.children = children.into_iter().collect();
    Container(element)
}
pub fn column<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    container(Kind::Column, children)
}
pub fn row<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    container(Kind::Row, children)
}
pub fn panel<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    container(Kind::Panel, children)
}
/// Alias for [`column()`]: a vertically arranged stack of children.
pub fn vstack<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    column(children)
}
/// Alias for [`row()`]: a horizontally arranged stack of children.
pub fn hstack<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    row(children)
}
/// An adaptive stack; defaults to vertical. Fixed stacks do not expose direction.
pub fn stack<C>(children: impl IntoIterator<Item = Element<C>>) -> Stack<C> {
    Stack(container(Kind::Stack, children).0)
}
/// Overlapping layers, centered by default; later children paint/hit in front.
pub fn zstack<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    container(Kind::ZStack, children)
}
impl<C> Stack<C> {
    pub fn direction(mut self, value: impl Into<Responsive<Direction>>) -> Self {
        self.0.node.responsive.direction = Some(value.into());
        self
    }
}
macro_rules! container_modifiers {
    ($target:ident) => {
        impl<C> $target<C> {
            /// Logical inner spacing. The last setter affecting an edge wins,
            /// replacing that edge's entire responsive rule set, not merging it.
            pub fn padding(mut self, value: impl Into<Responsive<f32>>, edge: Edge) -> Self {
                let value = value.into();
                let edges: &[usize] = match edge {
                    Edge::All => &[0, 1, 2, 3],
                    Edge::Horizontal => &[1, 3],
                    Edge::Vertical => &[0, 2],
                    Edge::Top => &[0],
                    Edge::Right => &[1],
                    Edge::Bottom => &[2],
                    Edge::Left => &[3],
                };
                for &edge in edges {
                    self.0.node.responsive.padding[edge] = Some(value.clone());
                }
                self
            }
            /// Inter-child spacing on the resolved main axis. ZStack ignores gaps.
            pub fn gap(mut self, value: impl Into<Responsive<f32>>, axis: Axis) -> Self {
                let value = value.into();
                let axes: &[usize] = match axis {
                    Axis::Both => &[0, 1],
                    Axis::Horizontal => &[0],
                    Axis::Vertical => &[1],
                };
                for &axis in axes {
                    self.0.node.responsive.gap[axis] = Some(value.clone());
                }
                self
            }
            /// Place content in both axes; for ZStack, place each layer independently.
            pub fn flow(mut self, value: Flow) -> Self {
                self.0.node.flow = Some(value);
                self
            }
            pub fn align(mut self, value: Align) -> Self {
                self.0.node.align = value;
                self
            }
        }
    };
}
container_modifiers!(Container);
container_modifiers!(Stack);
/// Compose heterogeneous Rust builders into a vertical layout.
#[macro_export]
macro_rules! column { ($($child:expr),* $(,)?) => { $crate::ui::column(vec![$($crate::ui::Element::from($child)),*]) }; }
/// Compose heterogeneous Rust builders into a horizontal layout.
#[macro_export]
macro_rules! row { ($($child:expr),* $(,)?) => { $crate::ui::row(vec![$($crate::ui::Element::from($child)),*]) }; }
/// Compose heterogeneous Rust builders into a padded surface/vertical layout.
#[macro_export]
macro_rules! panel { ($($child:expr),* $(,)?) => { $crate::ui::panel(vec![$($crate::ui::Element::from($child)),*]) }; }
pub use crate::{column, panel, row};
/// SwiftUI-familiar alias for [`column!`].
#[macro_export]
macro_rules! vstack { ($($child:expr),* $(,)?) => { $crate::column![$($child),*] }; }
/// SwiftUI-familiar alias for [`row!`].
#[macro_export]
macro_rules! hstack { ($($child:expr),* $(,)?) => { $crate::row![$($child),*] }; }
/// Compose heterogeneous children into an adaptive horizontal/vertical stack.
#[macro_export]
macro_rules! stack { ($($child:expr),* $(,)?) => { $crate::ui::stack(vec![$($crate::ui::Element::from($child)),*]) }; }
/// Compose overlapping layers; declaration order is back to front.
#[macro_export]
macro_rules! zstack { ($($child:expr),* $(,)?) => { $crate::ui::zstack(vec![$($crate::ui::Element::from($child)),*]) }; }
pub use crate::{hstack, stack, vstack, zstack};
/// Import component functions, types and layout macros together.
pub mod prelude {
    pub use super::{
        badge, button, checkbox, column, divider, hstack, panel, responsive, row, separator,
        spacer, stack, text, toggle, vstack, zstack, Align, Axis, Breakpoint, Direction, Edge,
        Element, Flow, FontStyle, Responsive, Rounding, Tone,
    };
}
