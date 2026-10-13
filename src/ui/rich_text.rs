//! Rich text: one paragraph whose runs differ in weight, slant, font, colour,
//! background or decoration, laid out and wrapped as a single text.

use super::*;
use crate::declarative::extras::TextSpan;

/// A run of [`rich_text`]. Unset properties inherit the text's own style.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    text: String,
    style: TextSpan,
}
/// A run of rich text with the given string.
pub fn span(text: impl Into<String>) -> Span {
    Span {
        text: text.into(),
        style: TextSpan::default(),
    }
}
impl From<&str> for Span {
    fn from(text: &str) -> Self {
        span(text)
    }
}
impl From<String> for Span {
    fn from(text: String) -> Self {
        span(text)
    }
}
impl Span {
    /// Weight 700.
    pub fn bold(self) -> Self {
        self.weight(700)
    }
    /// 100–900 in steps of 100.
    pub fn weight(mut self, weight: u16) -> Self {
        self.style.weight = Some(weight);
        self
    }
    pub fn italic(mut self) -> Self {
        self.style.italic = Some(true);
        self
    }
    /// A font family by name, or `"monospace"`, `"serif"`, `"sans-serif"`.
    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.style.family = Some(family.into());
        self
    }
    /// Inline code: monospace at 88% size on a subtle rounded background.
    pub fn code(self) -> Self {
        self.family("monospace")
            .size(0.88)
            .background(ColorToken::BaseContent.alpha(8))
    }
    /// Font size relative to the text (0.5–2.0); line spacing is unchanged.
    pub fn size(mut self, scale: f32) -> Self {
        self.style.scale = Some((scale.clamp(0.5, 2.) * 100.).round() as u16);
        self
    }
    /// Make the run a link. Clicks report `target` to the text's
    /// [`Text::on_link`] callback; links default to the primary colour and
    /// underline on hover.
    pub fn link(mut self, target: impl Into<String>) -> Self {
        self.style.link = Some(target.into());
        self
    }
    /// Text colour: hex or a theme token such as `ColorToken::Primary`.
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.style.color = Some(color.into());
        self
    }
    /// A rounded background behind the run, such as a highlight.
    pub fn background(mut self, color: impl Into<String>) -> Self {
        self.style.background = Some(color.into());
        self
    }
    pub fn underline(mut self) -> Self {
        self.style.underline = true;
        self
    }
    pub fn strikethrough(mut self) -> Self {
        self.style.strikethrough = true;
        self
    }
    fn styled(&self) -> bool {
        self.style != TextSpan::default()
    }
}

/// One paragraph made of differently styled runs. It wraps, aligns and
/// truncates as a whole, like [`text`], and takes the same modifiers:
///
/// ```
/// use volt_ui_kit::ui::prelude::*;
/// let line: Element<()> = rich_text([
///     span("Run "),
///     span("cargo test").code(),
///     span(" before you "),
///     span("push").bold(),
///     span(", or CI will fail.").color(ColorToken::Error),
/// ])
/// .font_size(15.)
/// .into();
/// line.build()?;
/// # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
/// ```
pub fn rich_text<C>(spans: impl IntoIterator<Item = impl Into<Span>>) -> Text<C> {
    let mut value = String::new();
    let mut runs = Vec::new();
    for s in spans {
        let s: Span = s.into();
        if s.text.is_empty() {
            continue;
        }
        let start = value.len();
        value.push_str(&s.text);
        if s.styled() {
            let mut style = s.style;
            if style.link.is_some() && style.color.is_none() {
                style.color = Some(ColorToken::Primary.into());
            }
            runs.push(TextSpan {
                start,
                end: value.len(),
                ..style
            });
        }
    }
    let mut element = text(value);
    element.0.node.ext.spans = runs;
    element
}

impl<C> Text<C> {
    /// Called with a link's target when one of this text's link runs is
    /// clicked. See [`Span::link`].
    pub fn on_link(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.0.callback = Some(super::Callback::Link(Box::new(f)));
        self
    }
}
