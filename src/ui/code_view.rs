//! `code_view`: a controlled, virtualized code surface for editors.
//!
//! The application keeps the document (a rope, say) and passes the view the
//! lines around the visible window with coloured spans, plus every
//! selection. The view draws the gutter, text, selections and carets, scrolls
//! smoothly, shows input-method composition at the caret, and reports each
//! key, character, click, drag and clipboard request as a [`CodeInput`].
//! It never holds the whole file, so very large files cost no more per frame
//! than small ones.
use super::*;
use crate::declarative::{CodeInput, CodeLine, CodeSelection, CodeViewProps};
use std::sync::Arc;

type CodeCallback<C> = Box<dyn FnMut(&mut C, CodeInput) -> Option<String>>;

pub struct CodeView<C>(pub(super) Element<C>, Option<CodeCallback<C>>);

/// A code view over a document of `line_count` lines (at least one). It
/// needs a stable `.key(...)`, lines from `.lines(...)` and an `.on_input`.
pub fn code_view<C>(line_count: usize) -> CodeView<C> {
    let mut e = Element::new(Kind::Panel);
    e.node.ext.code = Some(Arc::new(CodeViewProps {
        line_count: line_count.max(1),
        lines: Arc::from(Vec::new()),
        ..CodeViewProps::default()
    }));
    e.node.ext.cursor = Some(Cursor::Text);
    e.node.access_role = Some(crate::declarative::AccessRole::MultilineTextInput);
    CodeView(e, None)
}
impl<C> CodeView<C> {
    fn props(&mut self) -> &mut CodeViewProps {
        Arc::make_mut(self.0.node.ext.code.as_mut().expect("code view props"))
    }
    /// The supplied lines, starting at line `first`. Supply the visible
    /// window plus some lines above and below it (see `CodeInput::Visible`).
    pub fn lines(mut self, first: usize, lines: impl Into<Arc<[CodeLine]>>) -> Self {
        let p = self.props();
        p.first = first;
        p.lines = lines.into();
        self
    }
    /// Span colours by palette index: theme tokens (`"@primary"`) or hex.
    /// `"#9A2A8A|#C3A6FF"` gives a light and a dark colour, chosen by the
    /// theme in effect, so System appearance needs no recomposition.
    pub fn palette(mut self, colors: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.props().palette = colors.into_iter().map(Into::into).collect();
        self
    }
    /// Every selection (carets are empty selections); `primary` is the one
    /// the view keeps on screen and the input method follows.
    pub fn selections(mut self, list: Vec<CodeSelection>, primary: usize) -> Self {
        let p = self.props();
        if !list.is_empty() {
            p.primary = primary.min(list.len() - 1);
            p.selections = list;
        }
        self
    }
    /// Font family (a comma-separated fallback list), size and line height,
    /// in logical points. Defaults: SF Mono 13 on 20.
    pub fn font(mut self, family: impl Into<String>, size: f32, line_height: f32) -> Self {
        let p = self.props();
        p.font_family = family.into();
        p.font_size = size;
        p.line_height = line_height;
        self
    }
    /// Line numbers on the left (default on).
    pub fn gutter(mut self, value: bool) -> Self {
        self.props().gutter = value;
        self
    }
    /// Read-only views still select, scroll and copy.
    pub fn read_only(mut self, value: bool) -> Self {
        self.props().read_only = value;
        self
    }
    /// Shade the lines holding a caret (default on).
    pub fn highlight_line(mut self, value: bool) -> Self {
        self.props().highlight_line = value;
        self
    }
    /// Scroll just enough to show the primary caret whenever `request`
    /// changes, e.g. after each key that moved it.
    pub fn reveal(mut self, request: u32) -> Self {
        self.props().reveal = request;
        self
    }
    /// Put `line` a third of the way down whenever `request` changes
    /// (go to line, find).
    pub fn scroll_to(mut self, line: usize, request: u32) -> Self {
        self.props().scroll_to = Some((line, request));
        self
    }
    /// Watch the bottom `height` points of the view for the caret and report
    /// `CodeInput::CaretBand` as it enters or leaves: for a surface that
    /// floats over that band and must keep clear of the caret.
    pub fn caret_band(mut self, height: f32) -> Self {
        self.props().caret_band = Some(height);
        self
    }
    pub fn selection_color(mut self, value: impl Into<String>) -> Self {
        self.props().selection_color = Some(value.into());
        self
    }
    pub fn caret_color(mut self, value: impl Into<String>) -> Self {
        self.props().caret_color = Some(value.into());
        self
    }
    /// The current-line band.
    pub fn line_color(mut self, value: impl Into<String>) -> Self {
        self.props().line_color = Some(value.into());
        self
    }
    /// Line numbers (the current line uses the text colour).
    pub fn gutter_color(mut self, value: impl Into<String>) -> Self {
        self.props().gutter_color = Some(value.into());
        self
    }
    /// Take keyboard focus when `value` becomes true (a rising edge), e.g.
    /// when a find bar closes or a file opens.
    pub fn focus_when(mut self, value: bool) -> Self {
        self.0.node.input_focus = value;
        self
    }
    /// Every edit, movement, click and clipboard request. Return the
    /// selected text for `CodeInput::Copy` and `CodeInput::Cut`.
    pub fn on_input(
        mut self,
        f: impl FnMut(&mut C, CodeInput) -> Option<String> + 'static,
    ) -> Self {
        self.1 = Some(Box::new(f));
        self
    }
}
impl<C: 'static> From<CodeView<C>> for Element<C> {
    fn from(mut view: CodeView<C>) -> Self {
        let error = if view.0.auto_id {
            Some("code_view requires a stable .key(...)")
        } else if view.1.is_none() {
            Some("code_view requires on_input")
        } else if !view.0.node.ext.code.as_ref().is_some_and(|c| c.valid()) {
            Some("code_view lines, spans, selections or font are out of range")
        } else {
            None
        };
        if let Some(message) = error {
            view.0.validation_error = Some(message.into());
        }
        if let Some(f) = view.1.take() {
            view.0.callback = Some(Callback::Code(f));
        }
        view.0
    }
}
