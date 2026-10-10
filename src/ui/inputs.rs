//! Controlled plain-text inputs with host-owned cursor/undo/IME state.
use super::*;
use crate::declarative::document::EditorOptions;
pub struct TextInput<C>(pub(super) Element<C>, InputCallbacks<C>);
type TextChange<C> = Box<dyn FnMut(&mut C, String)>;
struct InputCallbacks<C> {
    change: Option<TextChange<C>>,
    submit: Option<Click<C>>,
}
pub fn text_field<C>(value: impl Into<String>) -> TextInput<C> {
    input(value, false)
}
pub fn text_area<C>(value: impl Into<String>) -> TextInput<C> {
    input(value, true)
}
fn input<C>(value: impl Into<String>, multiline: bool) -> TextInput<C> {
    let mut e = Element::new(Kind::Text);
    e.node.text = Some(value.into());
    e.node.height = Some(if multiline { 128. } else { 36. });
    e.node.style.radius = Some(Rounding::Radius(6.));
    e.node.editor = Some(EditorOptions {
        multiline,
        ..Default::default()
    });
    TextInput(
        e,
        InputCallbacks {
            change: None,
            submit: None,
        },
    )
}
impl<C> TextInput<C> {
    pub fn on_change(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.1.change = Some(Box::new(f));
        self
    }
    pub fn on_submit(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.1.submit = Some(Box::new(f));
        self
    }
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        self.0.node.editor.as_mut().unwrap().placeholder = value.into();
        self
    }
    pub fn read_only(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().read_only = value;
        self
    }
    /// Opt-in visual wrapping for multiline input; stored text is unchanged.
    pub fn soft_wrap(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().soft_wrap = value;
        self
    }
    pub fn password(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().password = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.0.node.enabled = value;
        self
    }
}
impl<C> From<TextInput<C>> for Element<C> {
    fn from(mut input: TextInput<C>) -> Self {
        input.0.callback = Some(Callback::Text {
            change: input.1.change,
            submit: input.1.submit,
        });
        input.0
    }
}
