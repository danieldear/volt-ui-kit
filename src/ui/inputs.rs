//! Controlled plain-text inputs with host-owned cursor/undo/IME state.
use super::*;
use crate::declarative::document::EditorOptions;
pub struct TextInput<C>(pub(super) Element<C>, InputCallbacks<C>);
type TextChange<C> = Box<dyn FnMut(&mut C, String)>;
struct InputCallbacks<C> {
    change: Option<TextChange<C>>,
    submit: Option<Click<C>>,
    cancel: Option<Click<C>>,
    selection: Option<SelectionCallback<C>>,
    key: Option<KeyCallback<C>>,
    leading: Option<Icon>,
    trailing: Option<Trailing<C>>,
}
enum Trailing<C> {
    Icon(Icon),
    Action {
        key: String,
        glyph: Icon,
        label: String,
        click: Click<C>,
    },
}
/// Width of an icon slot inside a field, in logical pixels.
const SLOT: f32 = 26.;
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
            cancel: None,
            selection: None,
            key: None,
            leading: None,
            trailing: None,
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
    /// The selection or caret moved (or the text changed): byte range and
    /// the caret's rectangle, for popups that follow the caret.
    pub fn on_selection(
        mut self,
        f: impl FnMut(&mut C, crate::declarative::TextSelection) + 'static,
    ) -> Self {
        self.1.selection = Some(Box::new(f));
        self
    }
    /// Hand `keys` to `f` instead of editing, e.g. Up, Down and Enter while
    /// a completion menu is open. An empty list hands over nothing.
    pub fn on_keys(
        mut self,
        keys: impl IntoIterator<Item = crate::declarative::EditKey>,
        f: impl FnMut(&mut C, crate::declarative::EditKey) + 'static,
    ) -> Self {
        self.0.node.editor.as_mut().unwrap().keys = keys.into_iter().collect();
        self.1.key = Some(Box::new(f));
        self
    }
    /// Select `range` (caret at its end) whenever `request` changes, e.g.
    /// to place the caret after text the application inserted.
    pub fn select(mut self, range: std::ops::Range<usize>, request: u32) -> Self {
        self.0.node.editor.as_mut().unwrap().select = Some((range.start, range.end, request));
        self
    }
    /// Scroll so the line holding byte `at` is at the top whenever
    /// `request` changes, e.g. to follow a preview.
    pub fn scroll_to_offset(mut self, at: usize, request: u32) -> Self {
        self.0.node.editor.as_mut().unwrap().scroll_to = Some((at, request));
        self
    }
    /// Colour byte ranges of the text (syntax highlighting): sorted,
    /// non-overlapping `(start, end, palette index)`. Colours only, so the
    /// layout is unchanged; at most 200,000 ranges.
    pub fn highlights(mut self, palette: Vec<String>, ranges: Vec<(usize, usize, u8)>) -> Self {
        self.0.node.editor.as_mut().unwrap().highlights =
            Some(std::sync::Arc::new(crate::declarative::EditorHighlights {
                palette,
                ranges,
            }));
        self
    }
    /// Escape while the field has focus, e.g. to close a find bar.
    pub fn on_cancel(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.1.cancel = Some(Box::new(f));
        self
    }
    /// Text areas: Tab inserts four spaces instead of moving focus (Shift+Tab
    /// still moves focus), as source editors expect.
    pub fn tab_indents(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().tab_indents = value;
        self
    }
    /// Take keyboard focus when the field first appears (not on every
    /// rebuild), as a find field does when its bar opens.
    pub fn autofocus(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().autofocus = value;
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
    /// Multiline only: Enter submits and Shift+Enter inserts a newline, as
    /// chat composers do. Cmd/Ctrl+Enter submits either way.
    pub fn submit_on_enter(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().enter_submits = value;
        self
    }
    /// A decorative icon inside the field's leading edge (search, mail…).
    pub fn leading_icon(mut self, glyph: Icon) -> Self {
        self.1.leading = Some(glyph);
        self
    }
    /// A decorative icon inside the trailing edge.
    pub fn trailing_icon(mut self, glyph: Icon) -> Self {
        self.1.trailing = Some(Trailing::Icon(glyph));
        self
    }
    /// A small icon button inside the trailing edge: clear, reveal password…
    pub fn trailing_action(
        mut self,
        key: impl Into<String>,
        glyph: Icon,
        label: impl Into<String>,
        click: impl FnMut(&mut C) + 'static,
    ) -> Self {
        self.1.trailing = Some(Trailing::Action {
            key: key.into(),
            glyph,
            label: label.into(),
            click: Box::new(click),
        });
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
impl<C: 'static> From<TextInput<C>> for Element<C> {
    fn from(mut input: TextInput<C>) -> Self {
        input.0.callback = Some(Callback::Text {
            change: input.1.change,
            submit: input.1.submit,
            cancel: input.1.cancel,
            selection: input.1.selection,
            key: input.1.key,
        });
        let (leading, trailing) = (input.1.leading, input.1.trailing);
        if leading.is_none() && trailing.is_none() {
            return input.0;
        }
        // Make room for the slots, then layer the icons over the field.
        if let Some(editor) = input.0.node.editor.as_mut() {
            editor.inset = [
                if leading.is_some() { SLOT } else { 0. },
                if trailing.is_some() { SLOT } else { 0. },
            ];
        }
        let key = input.0.node.id.clone();
        let height = input.0.node.height;
        // The layer takes the field's size so the slots sit inside it.
        let width = input.0.node.width.take();
        let flex = std::mem::take(&mut input.0.node.flex);
        let mut overlay: Vec<Element<C>> = vec![];
        if let Some(glyph) = leading {
            overlay.push(icon(glyph).size(16.).color(ColorToken::Muted).into());
        }
        overlay.push(spacer().into());
        match trailing {
            Some(Trailing::Icon(glyph)) => {
                overlay.push(icon(glyph).size(16.).color(ColorToken::Muted).into())
            }
            Some(Trailing::Action {
                key: action,
                glyph,
                label,
                click,
            }) => {
                let mut b = button("")
                    .key(action.clone())
                    .accessible_label(label)
                    .variant(ButtonVariant::Ghost)
                    .width(24.)
                    .height(24.);
                b.0.callback = Some(Callback::Click(click));
                overlay.push(
                    zstack![b, icon(glyph).size(14.).color(ColorToken::Muted)]
                        .key(format!("{action}-layer"))
                        .width(24.)
                        .height(24.)
                        .flow(Flow::Center)
                        .into(),
                );
            }
            None => {}
        }
        let mut layer = zstack![
            input.0,
            hstack(overlay)
                .key(format!("{key}-slots"))
                .align(Align::Center)
                .padding(10., Edge::Left)
                .padding(6., Edge::Right)
                .scroll_passthrough(true),
        ]
        .key(format!("{key}-adorned"))
        .align(Align::Stretch);
        if let Some(h) = height {
            layer = layer.height(h);
        }
        if let Some(w) = width {
            layer = layer.width(w);
        }
        layer.0.node.flex = flex;
        layer.into()
    }
}
