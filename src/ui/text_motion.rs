//! Animated text uses ordinary Text nodes: no timers or application callbacks.
use super::{text, Text};
use crate::motion::TextEffect;
pub fn typing_text<C>(value: impl Into<String>) -> Text<C> {
    text(value).text_effect(TextEffect::typing())
}
pub fn morphing_text<C>(value: impl Into<String>) -> Text<C> {
    text(value).text_effect(TextEffect::morph())
}
pub fn rolling_number<C>(value: i64) -> Text<C> {
    text(value.to_string())
        .font_family("monospace")
        .no_wrap(true)
        .text_effect(TextEffect::rolling())
}
pub fn shimmer_text<C>(value: impl Into<String>) -> Text<C> {
    text(value).text_effect(TextEffect::shimmer())
}
impl<C> Text<C> {
    /// Typing is a visual reveal, not incremental delivery or secrecy: accessibility
    /// and copy retain the complete target text. Revision changes replay it.
    pub fn text_effect(mut self, effect: TextEffect) -> Self {
        self.0.node.effects.text_effect = Some(effect);
        self
    }
}
