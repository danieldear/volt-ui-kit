//! A controlled, virtualized code surface. The application owns the text;
//! it hands the view only the lines around the visible window, with coloured
//! spans and the selections, and receives every edit as a [`CodeInput`].
//! Work per frame depends on the window's size, not the document's.
use super::*;
use crate::declarative::InputModifiers;
use std::sync::Arc;

/// A line and a column on it. Columns count characters (Unicode scalar
/// values) of the line text the application supplied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CodePosition {
    pub line: usize,
    pub column: usize,
}
impl CodePosition {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// One selection: empty (a caret) when `anchor == head`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeSelection {
    pub anchor: CodePosition,
    pub head: CodePosition,
}
impl CodeSelection {
    pub fn caret(at: CodePosition) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }
    fn ordered(&self) -> (CodePosition, CodePosition) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

/// How a [`CodeDecoration`] marks its range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CodeMark {
    /// A tint behind the text, under any selection: a search match.
    #[default]
    Fill,
    /// A line under the text: a diagnostic.
    Underline,
    /// A thin box around the range: the current match.
    Outline,
    /// Every line from the start's to the end's, tinted across the view,
    /// with a bar at the gutter's edge: changed lines. An empty range marks
    /// the boundary above the start's line instead: removed lines.
    Lines,
}

/// A marked range of the document: search matches, diagnostics, an agent's
/// edits. Ranges that end where they start are skipped, except `Lines`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeDecoration {
    pub start: CodePosition,
    pub end: CodePosition,
    pub mark: CodeMark,
    /// A theme token (`"@primary"`), hex, or `"light|dark"`; `Fill` and
    /// `Lines` draw it as given, so give them a translucent colour.
    pub color: String,
}
impl CodeDecoration {
    pub fn new(
        start: CodePosition,
        end: CodePosition,
        mark: CodeMark,
        color: impl Into<String>,
    ) -> Self {
        Self {
            start: start.min(end),
            end: start.max(end),
            mark,
            color: color.into(),
        }
    }
}

/// A line as drawn: its text (tabs already expanded) and coloured byte
/// ranges `(start, end, palette index)`, sorted and disjoint.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeLine {
    pub text: String,
    pub spans: Vec<(u32, u32, u8)>,
}
impl CodeLine {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            spans: vec![],
        }
    }
}

/// Editing keys a focused code view hands to the application with the
/// modifiers held, so it can apply its own platform conventions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeKey {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Enter,
    Tab,
    /// Only sent while there are several selections (to collapse them).
    Escape,
    Undo,
    Redo,
    SelectAll,
}

/// Everything a code view reports. Copy and Cut expect the selected text
/// back from the handler, for the clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodeInput {
    /// Typed text or an input-method commit.
    Insert(String),
    /// Clipboard text pasted with the primary shortcut.
    Paste(String),
    Key {
        key: CodeKey,
        modifiers: InputModifiers,
    },
    /// A press in the text or gutter; `clicks` counts double/triple clicks.
    Press {
        at: CodePosition,
        clicks: u8,
        modifiers: InputModifiers,
    },
    /// The pointer moved with the button held after a press.
    Drag {
        at: CodePosition,
    },
    Copy,
    Cut,
    /// Lines `first..last` are on screen (the application supplies lines
    /// around them). Sent when the window moves outside the supplied lines
    /// or its height changes.
    Visible {
        first: usize,
        last: usize,
    },
    /// With `caret_band`: the primary caret's horizontal span `[x, width]`
    /// in whole window logical points while it is on screen within the
    /// view's bottom band, `None` once it isn't. Sent only when that
    /// changes, so a surface floating over the band can keep clear of the
    /// caret.
    CaretBand(Option<[i32; 2]>),
}

/// Most lines one document may carry; supply a window around the visible one.
pub(crate) const MAX_CODE_LINES: usize = 4096;
/// How far in from the right edge a press counts as on the scrollbar.
const SCROLLBAR_HIT: f32 = 14.;
/// Longest line text accepted; longer lines should be cut by the application.
pub(crate) const MAX_CODE_LINE: usize = 64 * 1024;
/// Most decorations one view may carry; supply those near the window.
pub(crate) const MAX_CODE_DECORATIONS: usize = 10_000;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CodeViewProps {
    pub line_count: usize,
    pub first: usize,
    pub lines: Arc<[CodeLine]>,
    pub palette: Vec<String>,
    pub selections: Vec<CodeSelection>,
    pub primary: usize,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub gutter: bool,
    pub read_only: bool,
    pub highlight_line: bool,
    pub reveal: u32,
    pub scroll_to: Option<(usize, u32)>,
    /// Height of the bottom band watched for the caret (`CodeInput::CaretBand`).
    pub caret_band: Option<f32>,
    pub decorations: Vec<CodeDecoration>,
    /// Palette indices drawn in italic (comments, say).
    pub italic: Vec<u8>,
    /// Columns per indentation level, drawing a guide at each level.
    pub indent_guides: Option<usize>,
    /// Lines marked in the scrollbar's track, with their colours.
    pub scroll_marks: Vec<(usize, String)>,
    pub guide_color: Option<String>,
    pub selection_color: Option<String>,
    pub caret_color: Option<String>,
    /// The caret's shape: 0 a line, 1 a block over the character, 2 an
    /// underline beneath it.
    pub caret_shape: u8,
    pub line_color: Option<String>,
    pub gutter_color: Option<String>,
    pub on_input: Option<String>,
    /// Separate opt-in geometry callback; never a new editing event variant.
    pub on_caret_rect: Option<String>,
}
impl Default for CodeViewProps {
    fn default() -> Self {
        Self {
            line_count: 1,
            first: 0,
            lines: Arc::from(vec![CodeLine::default()]),
            palette: vec![],
            selections: vec![CodeSelection::caret(CodePosition::default())],
            primary: 0,
            font_family: "SF Mono, Menlo, monospace".into(),
            font_size: 13.,
            line_height: 20.,
            gutter: true,
            read_only: false,
            highlight_line: true,
            reveal: 0,
            scroll_to: None,
            caret_band: None,
            decorations: vec![],
            italic: vec![],
            indent_guides: None,
            scroll_marks: vec![],
            guide_color: None,
            selection_color: None,
            caret_color: None,
            caret_shape: 0,
            line_color: None,
            gutter_color: None,
            on_input: None,
            on_caret_rect: None,
        }
    }
}
impl CodeViewProps {
    pub(crate) fn valid(&self) -> bool {
        let finite = |v: f32, lo: f32, hi: f32| v.is_finite() && (lo..=hi).contains(&v);
        self.line_count >= 1
            && self.line_count <= 1 << 31
            && self.lines.len() <= MAX_CODE_LINES
            && self.first.saturating_add(self.lines.len()) <= self.line_count
            && self.palette.len() <= 64
            && !self.selections.is_empty()
            && self.selections.len() <= 10_000
            && self.primary < self.selections.len()
            && finite(self.font_size, 4., 96.)
            && finite(self.line_height, self.font_size, 4. * self.font_size)
            && self.font_family.len() <= 256
            && self.caret_band.is_none_or(|h| finite(h, 0., 4096.))
            && self.decorations.len() <= MAX_CODE_DECORATIONS
            && self
                .decorations
                .iter()
                .all(|d| d.start <= d.end && d.color.len() <= 64)
            && self.italic.len() <= 64
            && self
                .indent_guides
                .is_none_or(|unit| (1..=16).contains(&unit))
            && self.scroll_marks.len() <= MAX_CODE_DECORATIONS
            && self.scroll_marks.iter().all(|(_, c)| c.len() <= 64)
            && self.lines.iter().all(|l| {
                l.text.len() <= MAX_CODE_LINE
                    && l.spans.windows(2).all(|w| w[0].1 <= w[1].0)
                    && l.spans.iter().all(|&(a, b, c)| {
                        a <= b
                            && (b as usize) <= l.text.len()
                            && l.text.is_char_boundary(a as usize)
                            && l.text.is_char_boundary(b as usize)
                            && (c as usize) < self.palette.len().max(1)
                    })
            })
    }
    fn line(&self, index: usize) -> Option<&CodeLine> {
        index
            .checked_sub(self.first)
            .and_then(|i| self.lines.get(i))
    }
}

/// Per-view state the runtime keeps across recompositions, by element id.
#[derive(Clone, Debug, Default)]
pub(crate) struct CodeViewState {
    /// Scroll offset in logical points.
    offset: [f32; 2],
    reported: Option<(usize, usize)>,
    /// The last caret band report, in whole logical points.
    band: Option<Option<[i32; 2]>>,
    /// Outer None means no paint has measured this optional geometry yet.
    painted_caret: Option<Option<[f32; 4]>>,
    reported_caret: Option<Option<[f32; 4]>>,
    reveal: u32,
    scroll: u32,
    preedit: String,
    preedit_cursor: Option<(usize, usize)>,
    dragging: bool,
    /// While the scrollbar's thumb is held: where in it (physical, from its
    /// top) the pointer took hold.
    thumb: Option<f32>,
    /// Character advance (logical) measured for the font at the last paint.
    advance: Option<f32>,
    /// Caret stops (logical x per character boundary) of painted lines that
    /// aren't plain ASCII, for exact hit-testing.
    stops: BTreeMap<usize, Vec<f32>>,
    stop_texts: BTreeMap<usize, String>,
    stop_font: Option<(String, u32, u32)>,
}

/// Geometry of one view in physical pixels.
struct Frame {
    bounds: [f32; 4],
    gutter: f32,
    text_x: f32,
    line_height: f32,
    advance: f32,
    scale: f32,
}
impl Frame {
    fn text_width(&self) -> f32 {
        (self.bounds[0] + self.bounds[2] - self.text_x).max(0.)
    }
}

const TEXT_INSET: f32 = 12.;

fn digits(n: usize) -> usize {
    n.max(1).ilog10() as usize + 1
}

impl Runtime {
    fn code_frame(&self, b: &LayoutBox, props: &CodeViewProps, state: &CodeViewState) -> Frame {
        let s = self.scale;
        let advance = state.advance.unwrap_or(props.font_size * 0.6);
        let gutter = if props.gutter {
            (14. + digits(props.line_count).max(3) as f32 * advance + 18.).max(64.)
        } else {
            0.
        };
        let bounds = b.bounds.bounds();
        Frame {
            bounds,
            gutter: gutter * s,
            text_x: bounds[0] + (gutter + TEXT_INSET) * s,
            line_height: props.line_height * s,
            advance: advance * s,
            scale: s,
        }
    }
    /// The overlay scrollbar, physical: `[x, track top, track length,
    /// thumb length, thumb top]`, or `None` when everything fits.
    fn code_scrollbar(&self, b: &LayoutBox, props: &CodeViewProps, offset_y: f32) -> Option<[f32; 5]> {
        let s = self.scale;
        let [bx, by, bw, bh] = b.bounds.bounds();
        let lh = props.line_height * s;
        let content = props.line_count as f32 * lh + bh - lh;
        if content <= bh * 1.01 {
            return None;
        }
        let track = bh - 12. * s;
        let length = (track * bh / content).clamp(24. * s, track);
        let max = (props.line_count.saturating_sub(1)) as f32 * props.line_height;
        let t = if max > 0. { (offset_y / max).clamp(0., 1.) } else { 0. };
        let top = by + 6. * s;
        Some([bx + bw - SCROLLBAR_HIT * s, top, track, length, top + (track - length) * t])
    }
    /// Moves a held thumb so the pointer keeps its hold at physical `y`.
    fn code_drag_thumb(&self, b: &LayoutBox, props: &CodeViewProps, y: f32, grab: f32) -> bool {
        let mut states = self.code_views.borrow_mut();
        let Some(st) = states.get_mut(&b.id) else {
            return false;
        };
        let Some([_, top, track, length, _]) = self.code_scrollbar(b, props, st.offset[1]) else {
            return false;
        };
        let room = (track - length).max(1.);
        let t = ((y - grab - top) / room).clamp(0., 1.);
        let max = (props.line_count.saturating_sub(1)) as f32 * props.line_height;
        let old = st.offset[1];
        st.offset[1] = (t * max).round();
        st.offset[1] != old
    }
    fn code_max_offset(
        &self,
        b: &LayoutBox,
        props: &CodeViewProps,
        state: &CodeViewState,
    ) -> [f32; 2] {
        let f = self.code_frame(b, props, state);
        let s = self.scale;
        // The last line may scroll up to the top of the view.
        let y = ((props.line_count.saturating_sub(1)) as f32 * props.line_height).max(0.);
        let longest = props
            .lines
            .iter()
            .map(|l| l.text.chars().count())
            .max()
            .unwrap_or(0) as f32;
        let x = (longest * f.advance / s + 48. - f.text_width() / s).max(0.);
        [x, y]
    }
    fn code_visible(
        &self,
        b: &LayoutBox,
        props: &CodeViewProps,
        state: &CodeViewState,
    ) -> (usize, usize) {
        let height = b.bounds.bounds()[3] / self.scale;
        let first = (state.offset[1] / props.line_height).floor().max(0.) as usize;
        let last = ((state.offset[1] + height) / props.line_height)
            .ceil()
            .max(0.) as usize;
        let first = first.min(props.line_count.saturating_sub(1));
        (
            first,
            last.clamp(first + 1, props.line_count.max(first + 1)),
        )
    }
    fn code_box_at(&self, x: f32, y: f32) -> Option<&LayoutBox> {
        // The topmost box that covers the point visibly: transparent layout
        // layers (a row holding a find bar over the view) let presses through.
        let top = self.boxes.iter().rev().find(|b| {
            self.scope_allows_id(&b.id)
                && !self.portal_passthrough(&b.id)
                && b.clip.contains(x, y)
                && b.clip_regions.iter().all(|r| r.contains(x, y))
                && b.content_clip.as_ref().is_none_or(|r| r.contains(x, y))
                && b.bounds.contains_corners(b.radii(self.scale), x, y)
                && self.input_surface(b)
        })?;
        (top.ext.code.is_some() && top.enabled).then_some(top)
    }
    fn focused_code(&self) -> Option<&LayoutBox> {
        let id = self.focused.as_deref()?;
        self.boxes
            .iter()
            .find(|b| b.id == id && b.ext.code.is_some() && b.enabled)
    }
    /// Whether physical `x`, `y` is over a code view's scrollbar, which takes
    /// presses for itself (and shows the arrow, not the I-beam).
    pub(super) fn code_scrollbar_at(&self, x: f32, y: f32) -> bool {
        let held = self.code_views.borrow().values().any(|st| st.thumb.is_some());
        held || self.code_box_at(x, y).is_some_and(|b| {
            let offset = self.code_views.borrow().get(&b.id).map_or(0., |st| st.offset[1]);
            self.code_scrollbar(b, b.ext.code.as_deref().unwrap(), offset).is_some_and(|[left, ..]| x >= left)
        })
    }
    /// Whether a press at physical `x`, `y` lands on a code view.
    pub fn code_view_at(&self, x: f32, y: f32) -> bool {
        self.code_box_at(x, y).is_some()
    }
    /// A code view has keyboard focus: keys, text and input methods go to it.
    pub fn code_view_focused(&self) -> bool {
        self.focused_code().is_some()
    }
    /// Whether the focused code view is read-only.
    pub fn code_view_read_only(&self) -> bool {
        self.focused_code()
            .and_then(|b| b.ext.code.as_ref())
            .is_some_and(|p| p.read_only)
    }
    /// The focused code view has more than one selection.
    pub fn code_view_multiple_selections(&self) -> bool {
        self.focused_code()
            .and_then(|b| b.ext.code.as_ref())
            .is_some_and(|p| p.selections.len() > 1)
    }
    /// The logical scroll offset of a code view (for tests and restoring).
    pub fn code_view_offset(&self, id: &str) -> Option<[f32; 2]> {
        self.code_views.borrow().get(id).map(|s| s.offset)
    }
    fn code_position_at(&self, b: &LayoutBox, x: f32, y: f32) -> CodePosition {
        let props = b.ext.code.as_deref().unwrap();
        let states = self.code_views.borrow();
        let default = CodeViewState::default();
        let state = states.get(&b.id).unwrap_or(&default);
        let f = self.code_frame(b, props, state);
        let s = f.scale;
        let line_f = ((y - f.bounds[1]) / s + state.offset[1]) / props.line_height;
        let line = (line_f.floor().max(0.) as usize).min(props.line_count - 1);
        let x_text = (x - f.text_x) / s + state.offset[0];
        let chars = props.line(line).map_or(0, |l| l.text.chars().count());
        let column = if let Some(stops) = state.stops.get(&line) {
            // Nearest caret stop.
            stops
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1 - x_text).abs().total_cmp(&(b.1 - x_text).abs()))
                .map_or(0, |(i, _)| i)
        } else {
            (x_text / (f.advance / s)).round().max(0.) as usize
        };
        CodePosition::new(line, column.min(chars))
    }
    fn dispatch_code<C>(
        &mut self,
        b_id: &str,
        input: CodeInput,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<(InputOutcome, Option<String>), DocumentError> {
        let Some(name) = self
            .boxes
            .iter()
            .find(|b| b.id == b_id)
            .and_then(|b| b.ext.code.as_ref())
            .and_then(|p| p.on_input.clone())
        else {
            return Ok((InputOutcome::default(), None));
        };
        let reply = handlers
            .dispatch_code_input(&name, input, ctx)
            .map_err(|e| DocumentError(e.to_string()))?;
        Ok((
            InputOutcome {
                changed: true,
                activated: true,
            },
            reply,
        ))
    }
    /// Send `input` to the focused code view. Returns the outcome and, for
    /// Copy and Cut, the text the application put on the clipboard.
    pub fn code_input<C>(
        &mut self,
        input: CodeInput,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<(InputOutcome, Option<String>), DocumentError> {
        let Some(id) = self.focused_code().map(|b| b.id.clone()) else {
            return Ok((InputOutcome::default(), None));
        };
        let read_only = self.code_view_read_only();
        let edits = match &input {
            CodeInput::Insert(_) | CodeInput::Paste(_) | CodeInput::Cut => true,
            CodeInput::Key { key, .. } => matches!(
                key,
                CodeKey::Backspace
                    | CodeKey::Delete
                    | CodeKey::Enter
                    | CodeKey::Tab
                    | CodeKey::Undo
                    | CodeKey::Redo
            ),
            _ => false,
        };
        if read_only && edits && input != CodeInput::Cut {
            return Ok((InputOutcome::default(), None));
        }
        let input = if read_only && input == CodeInput::Cut {
            CodeInput::Copy
        } else {
            input
        };
        self.dispatch_code(&id, input, handlers, ctx)
    }
    /// A press at physical `x`, `y`. Returns `None` when no code view is
    /// there (the host handles the press normally).
    pub fn code_pointer_down<C>(
        &mut self,
        x: f32,
        y: f32,
        clicks: u8,
        modifiers: InputModifiers,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<Option<InputOutcome>, DocumentError> {
        let Some(b) = self.code_box_at(x, y).cloned() else {
            return Ok(None);
        };
        // The scrollbar strip: hold the thumb and drag it, or press the
        // track to bring the thumb under the pointer and keep dragging.
        let props = b.ext.code.as_deref().unwrap();
        let offset = self.code_views.borrow().get(&b.id).map_or(0., |st| st.offset[1]);
        if let Some([left, _, _, length, thumb]) = self.code_scrollbar(&b, props, offset) {
            if x >= left {
                let grab = if (thumb..thumb + length).contains(&y) { y - thumb } else { length / 2. };
                self.code_views.borrow_mut().entry(b.id.clone()).or_default().thumb = Some(grab);
                let changed = self.code_drag_thumb(&b, props, y, grab);
                return Ok(Some(InputOutcome { changed: true, activated: changed }));
            }
        }
        self.ime_cancel();
        let at = self.code_position_at(&b, x, y);
        self.focused = Some(b.id.clone());
        self.focus_visible = false;
        self.keyboard = None;
        self.capture = None;
        self.code_views
            .borrow_mut()
            .entry(b.id.clone())
            .or_default()
            .dragging = true;
        let (outcome, _) = self.dispatch_code(
            &b.id,
            CodeInput::Press {
                at,
                clicks: clicks.max(1),
                modifiers,
            },
            handlers,
            ctx,
        )?;
        Ok(Some(outcome))
    }
    /// The pointer moved. While dragging from a press in a code view, reports
    /// the position under it (scrolling when it leaves the view).
    pub fn code_pointer_drag<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<Option<InputOutcome>, DocumentError> {
        let held = self
            .code_views
            .borrow()
            .iter()
            .find_map(|(id, st)| st.thumb.map(|grab| (id.clone(), grab)));
        if let Some((id, grab)) = held {
            let Some(b) = self.boxes.iter().find(|b| b.id == id && b.ext.code.is_some()).cloned() else {
                return Ok(None);
            };
            let changed = self.code_drag_thumb(&b, b.ext.code.as_deref().unwrap(), y, grab);
            return Ok(Some(InputOutcome { changed, activated: changed }));
        }
        let Some(b) = self.focused_code().cloned() else {
            return Ok(None);
        };
        let dragging = self
            .code_views
            .borrow()
            .get(&b.id)
            .is_some_and(|s| s.dragging);
        if !dragging {
            return Ok(None);
        }
        // Past the top or bottom edge, scroll toward the pointer.
        let [_, by, _, bh] = b.bounds.bounds();
        let edge = if y < by {
            (y - by) / self.scale
        } else if y > by + bh {
            (y - by - bh) / self.scale
        } else {
            0.
        };
        if edge != 0. {
            let props = b.ext.code.as_deref().unwrap();
            let max = {
                let states = self.code_views.borrow();
                self.code_max_offset(&b, props, &states[&b.id])
            };
            let mut states = self.code_views.borrow_mut();
            let st = states.get_mut(&b.id).unwrap();
            st.offset[1] = (st.offset[1] + edge.clamp(-60., 60.)).clamp(0., max[1]);
        }
        let at = self.code_position_at(&b, x, y.clamp(by, by + bh - 1.));
        let (outcome, _) = self.dispatch_code(&b.id, CodeInput::Drag { at }, handlers, ctx)?;
        Ok(Some(outcome))
    }
    /// The button was released: a drag ends.
    pub fn code_pointer_up(&mut self) -> bool {
        let mut changed = false;
        for st in self.code_views.get_mut().values_mut() {
            changed |= std::mem::take(&mut st.dragging);
            changed |= st.thumb.take().is_some();
        }
        changed
    }
    /// Wheel and trackpad scrolling over a code view; consumes what it uses.
    pub(super) fn code_view_scroll_at(&mut self, x: f32, y: f32, delta: &mut [f32; 2]) -> bool {
        let Some(b) = self.code_box_at(x, y).cloned() else {
            return false;
        };
        let props = b.ext.code.as_deref().unwrap();
        let mut states = self.code_views.borrow_mut();
        let st = states.entry(b.id.clone()).or_default();
        let max = self.code_max_offset(&b, props, st);
        let mut changed = false;
        for axis in 0..2 {
            let old = st.offset[axis];
            st.offset[axis] = (old + delta[axis]).clamp(0., max[axis]);
            delta[axis] -= st.offset[axis] - old;
            changed |= st.offset[axis] != old;
        }
        changed
    }
    /// After layout and scrolling: apply scroll and reveal requests, keep
    /// offsets in range, and report visible windows the application must
    /// fill. Returns whether a handler ran.
    pub(super) fn dispatch_code_windows<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<bool, DocumentError> {
        let views: Vec<LayoutBox> = self
            .boxes
            .iter()
            .filter(|b| b.ext.code.is_some() && self.scope_allows_id(&b.id))
            .cloned()
            .collect();
        let mut activated = false;
        for b in views {
            let props = b.ext.code.as_deref().unwrap();
            let mut band = None;
            let report = {
                let mut states = self.code_views.borrow_mut();
                let st = states.entry(b.id.clone()).or_default();
                let f = self.code_frame(&b, props, st);
                let max = self.code_max_offset(&b, props, st);
                let height = f.bounds[3] / f.scale;
                if let Some((line, request)) = props.scroll_to {
                    if request != st.scroll {
                        st.scroll = request;
                        st.offset[1] = line as f32 * props.line_height - height / 3.;
                    }
                }
                if props.reveal != st.reveal {
                    st.reveal = props.reveal;
                    let caret = props.selections[props.primary].head;
                    let lh = props.line_height;
                    let top = caret.line as f32 * lh;
                    if top < st.offset[1] {
                        st.offset[1] = top;
                    } else if top + lh > st.offset[1] + height {
                        st.offset[1] = top + lh - height;
                    }
                    let x = st
                        .stops
                        .get(&caret.line)
                        .and_then(|s| s.get(caret.column).copied())
                        .unwrap_or(caret.column as f32 * f.advance / f.scale);
                    let width = f.text_width() / f.scale;
                    if x < st.offset[0] + 4. {
                        st.offset[0] = (x - 48.).max(0.);
                    } else if x > st.offset[0] + width - 16. {
                        st.offset[0] = x - width + 48.;
                    }
                }
                st.offset[0] = st.offset[0].clamp(0., max[0]);
                st.offset[1] = st.offset[1].clamp(0., max[1]);
                if let Some(height) = props.caret_band {
                    let span = self.code_caret_in_band(&f, props, st, height);
                    let whole = span.map(|[x, w]| [x.round() as i32, w.ceil() as i32]);
                    if st.band != Some(whole) {
                        st.band = Some(whole);
                        band = Some(CodeInput::CaretBand(whole));
                    }
                }
                let (first, last) = self.code_visible(&b, props, st);
                let supplied = props.first..props.first + props.lines.len();
                let covered = supplied.contains(&first) && last <= supplied.end;
                let page_changed = st.reported.is_none_or(|(a, z)| z - a != last - first);
                if (!covered || page_changed) && st.reported != Some((first, last)) {
                    st.reported = Some((first, last));
                    Some(CodeInput::Visible { first, last })
                } else {
                    None
                }
            };
            for input in report.into_iter().chain(band) {
                activated |= self.dispatch_code(&b.id, input, handlers, ctx)?.0.activated;
            }
        }
        Ok(activated | self.dispatch_code_carets(handlers, ctx)?)
    }
    /// Dispatch opt-in primary-caret geometry measured during the latest paint.
    /// Call after `paint_ui` so font shaping, DPI and clipping are current. An
    /// unchanged frame never invokes the callback or requests recomposition.
    pub fn dispatch_code_carets<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<bool, DocumentError> {
        let mut reports = Vec::new();
        {
            let mut states = self.code_views.borrow_mut();
            for b in &self.boxes {
                if !self.scope_allows_id(&b.id) {
                    continue;
                }
                let Some(name) = b.ext.code.as_ref().and_then(|p| p.on_caret_rect.as_ref()) else {
                    continue;
                };
                let Some(st) = states.get_mut(&b.id) else {
                    continue;
                };
                let Some(rect) = st.painted_caret else {
                    continue;
                };
                if st.reported_caret != Some(rect) {
                    st.reported_caret = Some(rect);
                    reports.push((name.clone(), rect));
                }
            }
        }
        let changed = !reports.is_empty();
        for (name, rect) in reports {
            handlers
                .dispatch_code_caret(&name, rect, ctx)
                .map_err(|e| DocumentError(e.to_string()))?;
        }
        Ok(changed)
    }
    /// The primary caret's `[x, width]` in window logical points when it is
    /// on screen within the bottom `height` points of the view.
    fn code_caret_in_band(
        &self,
        f: &Frame,
        props: &CodeViewProps,
        st: &CodeViewState,
        height: f32,
    ) -> Option<[f32; 2]> {
        let head = props.selections[props.primary].head;
        let x = st
            .stops
            .get(&head.line)
            .and_then(|s| s.get(head.column).copied())
            .unwrap_or(head.column as f32 * f.advance / f.scale);
        let left = f.text_x + (x - st.offset[0]) * f.scale;
        let top = f.bounds[1] + (head.line as f32 * props.line_height - st.offset[1]) * f.scale;
        let bottom = f.bounds[1] + f.bounds[3];
        let on_screen = left >= f.text_x
            && left <= f.bounds[0] + f.bounds[2]
            && top + f.line_height > f.bounds[1]
            && top < bottom;
        (on_screen && top + f.line_height > bottom - height * f.scale)
            .then(|| [left / f.scale, (f.advance / f.scale).max(1.)])
    }

    // ---- Input methods ------------------------------------------------------

    pub(super) fn code_ime_preedit(
        &mut self,
        text: &str,
        cursor: Option<(usize, usize)>,
    ) -> Option<bool> {
        let id = self.focused_code()?.id.clone();
        if self.code_view_read_only() {
            return Some(false);
        }
        let mut states = self.code_views.borrow_mut();
        let st = states.entry(id.clone()).or_default();
        let changed = st.preedit != text || st.preedit_cursor != cursor;
        st.preedit = text.into();
        st.preedit_cursor = cursor.filter(|(a, b)| {
            *a <= text.len()
                && *b <= text.len()
                && text.is_char_boundary(*a)
                && text.is_char_boundary(*b)
        });
        drop(states);
        if !text.is_empty() {
            self.ime_target = Some(id);
            self.ime_cancelled = false;
        }
        Some(changed)
    }
    pub(super) fn code_ime_cancel(&mut self) -> bool {
        let mut changed = false;
        for st in self.code_views.get_mut().values_mut() {
            changed |= !st.preedit.is_empty();
            st.preedit.clear();
            st.preedit_cursor = None;
        }
        changed
    }
    pub(super) fn code_ime_active(&self) -> Option<bool> {
        let b = self.focused_code()?;
        Some(
            self.code_views
                .borrow()
                .get(&b.id)
                .is_some_and(|s| !s.preedit.is_empty()),
        )
    }
    /// Where the input method's candidate window should open: the primary
    /// caret, in physical pixels.
    pub(super) fn code_candidate_bounds(&self) -> Option<[f32; 4]> {
        let b = self.focused_code()?;
        let props = b.ext.code.as_deref()?;
        let states = self.code_views.borrow();
        let default = CodeViewState::default();
        let st = states.get(&b.id).unwrap_or(&default);
        let f = self.code_frame(b, props, st);
        let head = props.selections[props.primary].head;
        let x = st
            .stops
            .get(&head.line)
            .and_then(|s| s.get(head.column).copied())
            .unwrap_or(head.column as f32 * f.advance / f.scale);
        Some([
            f.text_x + (x - st.offset[0]) * f.scale,
            f.bounds[1] + (head.line as f32 * props.line_height - st.offset[1]) * f.scale,
            f.scale,
            f.line_height,
        ])
    }

    // ---- Painting ----------------------------------------------------------

    pub(super) fn push_code_view<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
        b: &LayoutBox,
        out: &mut Vec<PaintBatch<P::Shape, P::Glyph>>,
    ) {
        let Some(props) = b.ext.code.as_deref() else {
            return;
        };
        let c = ui.colors();
        let s = self.scale;
        let focused = self.focused.as_deref() == Some(&b.id);
        let mut states = self.code_views.borrow_mut();
        let st = states.entry(b.id.clone()).or_default();
        // Measure the font once per size: every column is one advance wide.
        if props.on_caret_rect.is_some() {
            st.painted_caret = Some(None);
        }
        let mut style = TextStyle {
            family: props.font_family.clone(),
            size: props.font_size * s,
            line_height: props.line_height / props.font_size,
            no_wrap: true,
            ..TextStyle::default()
        };
        let sample = "0000000000";
        let ten = p.line_advances(sample, &style, &[sample.len()]);
        if let Some(w) = ten.first().filter(|w| **w > 0.) {
            st.advance = Some(w / 10. / s);
        }
        let f = self.code_frame(b, props, st);
        let [bx, by, bw, bh] = f.bounds;
        let lh = f.line_height;
        let (first, last) = self.code_visible(b, props, st);
        let offset = st.offset;
        let y_of = |line: usize| by + line as f32 * lh - offset[1] * s;
        let x_scroll = offset[0] * s;
        let resolve = |value: &Option<String>, fallback: [f32; 4]| {
            value
                .as_deref()
                .and_then(|v| ui.resolve(v))
                .unwrap_or(fallback)
        };
        let text_color = b.style.color.unwrap_or(c.text);
        let with_alpha = |mut color: [f32; 4], a: f32| {
            color[3] *= a;
            color
        };
        let selection_color = resolve(&props.selection_color, with_alpha(c.accent, 0.28));
        let caret_color = resolve(&props.caret_color, c.accent);
        let line_color = resolve(&props.line_color, with_alpha(text_color, 0.045));
        let gutter_color = resolve(&props.gutter_color, c.muted);
        // `"light|dark"` entries pick by the text colour: light text means a
        // dark theme, so syntax colours follow System appearance too.
        let dark = 0.2126 * text_color[0] + 0.7152 * text_color[1] + 0.0722 * text_color[2] > 0.5;
        let pick = |v: &str| {
            let v = match v.split_once('|') {
                Some((light, night)) => {
                    if dark {
                        night
                    } else {
                        light
                    }
                }
                None => v,
            };
            ui.resolve(v.trim())
        };
        let palette: Vec<Option<[f32; 4]>> = props.palette.iter().map(|v| pick(v)).collect();
        // Decorations touching the drawn lines, with their colours.
        let marks: Vec<(&CodeDecoration, [f32; 4])> = props
            .decorations
            .iter()
            .filter(|d| d.start.line < last && d.end.line >= first)
            .filter(|d| d.mark == CodeMark::Lines || d.start != d.end)
            .filter_map(|d| Some((d, pick(&d.color)?)))
            .collect();

        // 1. Surface and current-line bands, across gutter and text.
        let mut shapes = Vec::new();
        paint_surface(
            p,
            &mut shapes,
            f.bounds,
            b.radius_spec(s),
            b.style.background.unwrap_or([0.; 4]),
        );
        let caret_lines: BTreeSet<usize> = props
            .selections
            .iter()
            .filter(|sel| sel.anchor.line == sel.head.line)
            .map(|sel| sel.head.line)
            .collect();
        if props.highlight_line {
            for &line in caret_lines.range(first..last) {
                if let Some(r) =
                    Rect::new(bx, y_of(line), bw, lh).and_then(|r| intersection(r, b.clip))
                {
                    let [x, y, w, h] = r.bounds();
                    p.draw_rect(&mut shapes, x, y, w, h, line_color);
                }
            }
        }
        for (d, color) in marks.iter().filter(|(d, _)| d.mark == CodeMark::Lines) {
            // The band as given; the bar at the gutter's edge fully opaque.
            let bar = (2. * s).round().max(1.);
            let bar_x = (bx + f.gutter - 4. * s).max(bx);
            let solid = [color[0], color[1], color[2], 1.];
            let rects = if d.start == d.end {
                // Removed lines: a notch on the boundary and a hairline.
                let y = y_of(d.start.line);
                let thin = s.round().max(1.);
                vec![
                    (Rect::new(bx, y - thin / 2., bw, thin), *color),
                    (Rect::new(bar_x - bar, y - 2. * s, 3. * bar, 4. * s), solid),
                ]
            } else {
                (d.start.line.max(first)..=d.end.line.min(last.saturating_sub(1)))
                    .flat_map(|line| {
                        let y = y_of(line);
                        [
                            (Rect::new(bx, y, bw, lh), *color),
                            (Rect::new(bar_x, y, bar, lh), solid),
                        ]
                    })
                    .collect()
            };
            for (r, color) in rects {
                if let Some(r) = r.and_then(|r| intersection(r, b.clip)) {
                    let [x, y, w, h] = r.bounds();
                    p.draw_rect(&mut shapes, x, y, w, h, color);
                }
            }
        }
        out.push(PaintBatch {
            clip: b.clip,
            shapes,
            glyphs: vec![],
        });
        let text_clip = Rect::new(
            f.text_x - TEXT_INSET * s,
            by,
            (bx + bw - f.text_x + TEXT_INSET * s).max(0.1),
            bh,
        )
        .and_then(|r| intersection(r, b.clip));
        let Some(text_clip) = text_clip else {
            return;
        };

        // Caret stops for this frame's lines: monospace arithmetic for ASCII,
        // shaped advances otherwise (wide and combining characters).
        // Cached positions must follow text/font changes at the same line id.
        // Keeping old cluster advances would misplace both hit tests and anchors.
        let font = (
            props.font_family.clone(),
            props.font_size.to_bits(),
            s.to_bits(),
        );
        if st.stop_font.as_ref() != Some(&font) {
            st.stops.clear();
            st.stop_texts.clear();
            st.stop_font = Some(font);
        }
        let stop_texts = &st.stop_texts;
        st.stops.retain(|line, _| {
            (first..last).contains(line)
                && props
                    .line(*line)
                    .is_some_and(|text| stop_texts.get(line) == Some(&text.text))
        });
        let stops = &st.stops;
        st.stop_texts.retain(|line, _| stops.contains_key(line));
        let primary_head = props.selections[props.primary].head;
        let preedit =
            (focused && !st.preedit.is_empty()).then(|| (st.preedit.clone(), st.preedit_cursor));
        let measure = style.clone();
        let stop_map = &mut st.stops;
        let stop_text_map = &mut st.stop_texts;
        let advance = f.advance;
        let mut x_at = |line: usize, text: &str, column: usize, p: &mut P| -> f32 {
            if text.is_ascii() {
                return column.min(text.len()) as f32 * advance;
            }
            let stops = stop_map.entry(line).or_insert_with(|| {
                stop_text_map.insert(line, text.to_owned());
                let mut offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
                offsets.push(text.len());
                p.line_advances(text, &measure, &offsets)
                    .into_iter()
                    .map(|x| x / s)
                    .collect()
            });
            stops.get(column).or(stops.last()).map_or(0., |x| x * s)
        };

        // One rectangle per drawn line of a range, unclipped; a line break
        // inside the range counts as part of it.
        let mut range_rects =
            |start: CodePosition, end: CodePosition, p: &mut P| -> Vec<[f32; 4]> {
                let mut out = Vec::new();
                for line in start.line.max(first)..=end.line.min(last.saturating_sub(1)) {
                    let text = props.line(line).map_or("", |l| l.text.as_str());
                    let chars = text.chars().count();
                    let a = if line == start.line {
                        start.column.min(chars)
                    } else {
                        0
                    };
                    let z = if line == end.line {
                        end.column.min(chars)
                    } else {
                        chars
                    };
                    let x0 = f.text_x - x_scroll + x_at(line, text, a, p);
                    let mut x1 = f.text_x - x_scroll + x_at(line, text, z, p);
                    if line != end.line {
                        x1 += f.advance * 0.6;
                    }
                    out.push([x0, y_of(line), (x1 - x0).max(0.), lh]);
                }
                out
            };
        let mut fills = Vec::new();
        let mut overlays = Vec::new();
        let thin = s.round().max(1.);
        for (d, color) in &marks {
            for [x, y, w, h] in range_rects(d.start, d.end, p) {
                match d.mark {
                    CodeMark::Fill => fills.push(([x, y, w, h], *color)),
                    CodeMark::Underline => {
                        overlays.push(([x, y + h - 2. * s, w.max(thin), thin], *color))
                    }
                    CodeMark::Outline => {
                        let w = w.max(thin);
                        overlays.push(([x, y, w, thin], *color));
                        overlays.push(([x, y + h - thin, w, thin], *color));
                        overlays.push(([x, y, thin, h], *color));
                        overlays.push(([x + w - thin, y, thin, h], *color));
                    }
                    CodeMark::Lines => {}
                }
            }
        }
        let mut selected = Vec::new();
        for sel in &props.selections {
            let (start, end) = sel.ordered();
            if start != end {
                selected.extend(range_rects(start, end, p));
            }
        }

        // 2. Indent guides, decoration fills, selections and text.
        let mut shapes = Vec::new();
        let mut glyphs = Vec::new();
        if let Some(unit) = props.indent_guides {
            let guide = resolve(&props.guide_color, with_alpha(text_color, 0.08));
            let thin = s.round().max(1.);
            let indent = |line: usize| -> Option<usize> {
                let text = &props.line(line)?.text;
                let spaces = text.len() - text.trim_start_matches(' ').len();
                (spaces < text.len()).then_some(spaces)
            };
            for line in first..last {
                // A blank line takes the indentation of the code after it.
                let depth = (line..last + 64)
                    .take_while(|&l| props.line(l).is_some())
                    .find_map(indent)
                    .unwrap_or(0);
                for level in 0..depth / unit {
                    let x = f.text_x - x_scroll + (level * unit) as f32 * f.advance;
                    if let Some(r) =
                        Rect::new(x, y_of(line), thin, lh).and_then(|r| intersection(r, text_clip))
                    {
                        let [x, y, w, h] = r.bounds();
                        p.draw_rect(&mut shapes, x, y, w, h, guide);
                    }
                }
            }
        }
        let painted = fills
            .into_iter()
            .chain(selected.into_iter().map(|r| (r, selection_color)));
        for ([x, y, w, h], color) in painted {
            if let Some(r) = Rect::new(x, y, w, h).and_then(|r| intersection(r, text_clip)) {
                let [x, y, w, h] = r.bounds();
                p.draw_rect(&mut shapes, x, y, w, h, color);
            }
        }
        for line in first..last {
            let Some(code) = props.line(line) else {
                continue;
            };
            let y = y_of(line);
            let mut text = std::borrow::Cow::Borrowed(code.text.as_str());
            let mut runs: Vec<TextRun> = code
                .spans
                .iter()
                .filter_map(|&(a, z, i)| {
                    let color = palette.get(i as usize).copied().flatten()?;
                    Some(TextRun {
                        start: a as usize,
                        end: z as usize,
                        color: Some(color),
                        italic: props.italic.contains(&i).then_some(true),
                        ..TextRun::default()
                    })
                })
                .collect();
            if let (Some((pre, _)), true) = (&preedit, line == primary_head.line) {
                // Show the composition inline at the caret, underlined.
                let at = code
                    .text
                    .char_indices()
                    .nth(primary_head.column)
                    .map_or(code.text.len(), |(i, _)| i);
                let mut spliced = code.text.clone();
                spliced.insert_str(at, pre);
                for r in &mut runs {
                    if r.start >= at {
                        r.start += pre.len();
                    }
                    if r.end > at {
                        r.end += pre.len();
                    }
                }
                let x0 = f.text_x - x_scroll + x_at(line, &code.text, primary_head.column, p);
                let width = p
                    .line_advances(pre, &style, &[pre.len()])
                    .first()
                    .copied()
                    .unwrap_or(0.);
                if let Some(r) = Rect::new(x0, y + lh - 2. * s, width.max(s), s.max(1.))
                    .and_then(|r| intersection(r, text_clip))
                {
                    let [x, y, w, h] = r.bounds();
                    p.draw_rect(&mut shapes, x, y, w, h, text_color);
                }
                text = std::borrow::Cow::Owned(spliced);
            }
            if text.is_empty() {
                continue;
            }
            style.runs = runs;
            p.ui_text_styled(
                &mut glyphs,
                &text,
                [f.text_x - x_scroll, y, 1_000_000., lh],
                &style,
                text_color,
            );
        }
        style.runs.clear();
        out.push(PaintBatch {
            clip: text_clip,
            shapes,
            glyphs,
        });
        // Underlines and outlines above the text.
        if !overlays.is_empty() {
            let mut shapes = Vec::new();
            for ([x, y, w, h], color) in overlays {
                if let Some(r) = Rect::new(x, y, w, h).and_then(|r| intersection(r, text_clip)) {
                    let [x, y, w, h] = r.bounds();
                    p.draw_rect(&mut shapes, x, y, w, h, color);
                }
            }
            out.push(PaintBatch {
                clip: text_clip,
                shapes,
                glyphs: vec![],
            });
        }

        // Measure the optional anchor with the same shaped caret stops and
        // clipped rectangle as painting, including input-method composition.
        if props.on_caret_rect.is_some() && (first..last).contains(&primary_head.line) {
            if let Some(code) = props.line(primary_head.line) {
                let mut x = f.text_x - x_scroll
                    + x_at(primary_head.line, &code.text, primary_head.column, p);
                if let Some((pre, cursor)) = &preedit {
                    let upto = cursor.map_or(pre.len(), |c| c.1);
                    x += p
                        .line_advances(pre, &style, &[upto])
                        .first()
                        .copied()
                        .unwrap_or(0.);
                }
                let width = (2. * s).round().max(1.);
                let rect = Rect::new(
                    x - width / 2.,
                    y_of(primary_head.line) + 2. * s,
                    width,
                    lh - 4. * s,
                )
                .and_then(|r| intersection(r, text_clip));
                st.painted_caret = Some(
                    rect.filter(|r| {
                        let [x, y, w, h] = r.bounds();
                        b.clip_regions
                            .iter()
                            .all(|region| region.contains(x + w / 2., y + h / 2.))
                    })
                    .map(|r| r.bounds().map(|v| v / s)),
                );
            }
        }

        // 3. Carets above the text, only while focused.
        if focused {
            let mut shapes = Vec::new();
            for sel in &props.selections {
                let head = sel.head;
                if !(first..last).contains(&head.line) {
                    continue;
                }
                let text = props.line(head.line).map_or("", |l| l.text.as_str());
                let mut x = f.text_x - x_scroll + x_at(head.line, text, head.column, p);
                if let (Some((pre, cursor)), true) = (&preedit, head == primary_head) {
                    let upto = cursor.map_or(pre.len(), |c| c.1);
                    x += p
                        .line_advances(pre, &style, &[upto])
                        .first()
                        .copied()
                        .unwrap_or(0.);
                }
                let width = (2. * s).round().max(1.);
                let top = y_of(head.line);
                // A block is translucent so the character under it stays legible.
                let (rect, color) = match props.caret_shape {
                    1 => (Rect::new(x, top + 2. * s, f.advance, lh - 4. * s), [caret_color[0], caret_color[1], caret_color[2], caret_color[3] * 0.45]),
                    2 => (Rect::new(x, top + lh - 2. * s - width, f.advance, width), caret_color),
                    _ => (Rect::new(x - width / 2., top + 2. * s, width, lh - 4. * s), caret_color),
                };
                if let Some(r) = rect.and_then(|r| intersection(r, text_clip)) {
                    let [x, y, w, h] = r.bounds();
                    p.draw_rect(&mut shapes, x, y, w, h, color);
                }
            }
            out.push(PaintBatch {
                clip: text_clip,
                shapes,
                glyphs: vec![],
            });
        }

        // 4. Line numbers, right-aligned in the gutter.
        if props.gutter {
            if let Some(gutter_clip) =
                Rect::new(bx, by, f.gutter, bh).and_then(|r| intersection(r, b.clip))
            {
                let mut glyphs = Vec::new();
                let right = bx + f.gutter - 18. * s;
                let mut number_style = style.clone();
                number_style.size = (props.font_size - 1.).max(4.) * s;
                for line in first..last.min(props.line_count) {
                    let label = (line + 1).to_string();
                    let width = label.len() as f32 * f.advance * (number_style.size / style.size);
                    let color = if caret_lines.contains(&line) {
                        text_color
                    } else {
                        gutter_color
                    };
                    p.ui_text_styled(
                        &mut glyphs,
                        &label,
                        [right - width, y_of(line), width + f.advance, lh],
                        &number_style,
                        color,
                    );
                }
                out.push(PaintBatch {
                    clip: gutter_clip,
                    shapes: vec![],
                    glyphs,
                });
            }
        }

        // 5. An overlay scrollbar when the document is taller than the view,
        // with the marked lines (search matches, say) along its track.
        let content = props.line_count as f32 * lh + bh - lh;
        let scrolls = content > bh * 1.01;
        if scrolls || !props.scroll_marks.is_empty() {
            let track = bh - 12. * s;
            let mut shapes = Vec::new();
            if scrolls {
                let length = (track * bh / content).clamp(24. * s, track);
                let max = (props.line_count.saturating_sub(1)) as f32 * props.line_height;
                let t = if max > 0. {
                    (offset[1] / max).clamp(0., 1.)
                } else {
                    0.
                };
                let top = by + 6. * s + (track - length) * t;
                // Held, it widens and darkens so it reads as grabbed.
                let (w, a) = if st.thumb.is_some() { (9., 0.3) } else { (7., 0.14) };
                p.card_round(
                    &mut shapes,
                    [bx + bw - (4. + w) * s, top, w * s, length],
                    w / 2. * s,
                    with_alpha(text_color, a),
                );
            }
            let rows = props.line_count.max(1) as f32;
            for (line, color) in &props.scroll_marks {
                if let Some(color) = pick(color) {
                    let y = by + 6. * s + track * (*line as f32 / rows).min(1.);
                    p.card_round(
                        &mut shapes,
                        [bx + bw - 12. * s, y, 9. * s, 2. * s],
                        s,
                        color,
                    );
                }
            }
            out.push(PaintBatch {
                clip: b.clip,
                shapes,
                glyphs: vec![],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn props_validation_rejects_bad_windows_and_spans() {
        let ok = CodeViewProps::default();
        assert!(ok.valid());
        let mut bad = CodeViewProps {
            first: 1,
            ..CodeViewProps::default()
        };
        assert!(!bad.valid(), "window past the end");
        bad.first = 0;
        bad.lines = Arc::from(vec![CodeLine {
            text: "é".into(),
            spans: vec![(0, 1, 0)],
        }]);
        bad.palette = vec!["#fff".into()];
        assert!(!bad.valid(), "span inside a character");
        bad.lines = Arc::from(vec![CodeLine {
            text: "ab".into(),
            spans: vec![(0, 2, 0), (1, 2, 0)],
        }]);
        assert!(!bad.valid(), "overlapping spans");
        assert_eq!(digits(1), 1);
        assert_eq!(digits(1000), 4);
    }
}
