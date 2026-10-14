//! Selecting and copying read-only text inside a `.selectable(true)`
//! container: drag across paragraphs, double-click a word, triple-click a
//! paragraph, Shift-click to extend, select all, and copy. Text boxes are
//! taken in document order; a selection is anchored to box keys, so it
//! survives recomposition while those boxes exist.
use super::*;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Default)]
pub(super) struct TextSelectionState {
    anchor: Option<(String, usize)>,
    focus: Option<(String, usize)>,
    dragging: bool,
    /// What the press selected (box, start, end) and its unit: 1 for
    /// characters, 2 for words, 3 for paragraphs. Drags extend by the unit.
    pivot: Option<(String, usize, usize)>,
    unit: u8,
    /// The selected text as of the last change; kept when rows scroll away.
    text: String,
    /// The text last reported to the application.
    reported: Option<String>,
    /// Highlight rectangles per box, relative to the box origin.
    rects: Vec<(String, Vec<[f32; 4]>)>,
}

fn selectable(b: &LayoutBox) -> bool {
    b.kind == Kind::Text && b.style.selectable && b.editor.is_none() && !b.text.is_empty()
}

fn contains(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

/// The word (or run of spaces/punctuation) around byte `at`.
fn word_at(text: &str, at: usize) -> (usize, usize) {
    for (start, word) in text.split_word_bound_indices() {
        let end = start + word.len();
        if at < end || end == text.len() {
            return (start, end);
        }
    }
    (at, at)
}

impl Runtime {
    fn selectable_at(&self, x: f32, y: f32) -> Option<&LayoutBox> {
        self.boxes.iter().rev().find(|b| {
            selectable(b)
                && contains(b.bounds.bounds(), x, y)
                && contains(b.clip.bounds(), x, y)
                && self.scope_allows_id(&b.id)
        })
    }

    /// Whether the last press left nothing captured (no control, link,
    /// editor, scrollbar, divider or slider), so it may select text.
    pub fn press_was_free(&self) -> bool {
        self.capture.is_none()
            && self.link_capture.is_none()
            && self.scrollbar_capture.is_none()
            && self.split_capture.is_none()
            && self.slider_capture.is_none()
            && !self.editors.borrow().values().any(|e| e.dragging())
    }

    /// The selectable box a drag at (x, y) extends into: the one under the
    /// pointer, else the last box starting above it, else the first.
    fn selectable_near(&self, x: f32, y: f32) -> Option<&LayoutBox> {
        if let Some(b) = self.selectable_at(x, y) {
            return Some(b);
        }
        let visible = |b: &&LayoutBox| selectable(b) && b.layout_visible;
        self.boxes
            .iter()
            .rev()
            .filter(visible)
            .find(|b| b.bounds.bounds()[1] <= y)
            .or_else(|| self.boxes.iter().find(visible))
    }

    fn offset_in(b: &LayoutBox, x: f32, y: f32, m: &mut impl TextMetrics) -> usize {
        let r = b.bounds.bounds();
        // Points beyond the box snap to its edges.
        if y >= r[1] + r[3] {
            return b.text.len();
        }
        if y < r[1] {
            return 0;
        }
        let point = [x.clamp(r[0], r[0] + r[2]), y];
        m.text_offset_at(&b.text, r, &b.text_style, point)
            .unwrap_or(0)
            .min(b.text.len())
    }

    /// A left press: start a selection on selectable text (`clicks` 2 for a
    /// word, 3 for the paragraph; `extend` keeps the anchor), or clear it.
    /// Returns whether the selection changed.
    pub fn text_select_press(
        &mut self,
        x: f32,
        y: f32,
        clicks: u8,
        extend: bool,
        m: &mut impl TextMetrics,
    ) -> bool {
        let Some(b) = self.selectable_at(x, y) else {
            return self.clear_text_selection();
        };
        let id = b.id.clone();
        let at = Self::offset_in(b, x, y, m);
        let (start, end) = match clicks {
            0 | 1 => (at, at),
            2 => word_at(&b.text, at),
            _ => (0, b.text.len()),
        };
        let s = &mut self.text_selection;
        if extend && s.anchor.is_some() {
            s.focus = Some((id, at));
            s.unit = 1;
        } else {
            s.anchor = Some((id.clone(), start));
            s.focus = Some((id.clone(), end));
            s.pivot = Some((id, start, end));
            s.unit = clicks.clamp(1, 3);
        }
        s.dragging = true;
        self.refresh_text_selection(m);
        true
    }

    /// Pointer moves while selecting: extend the focus under the pointer.
    pub fn text_select_drag(&mut self, x: f32, y: f32, m: &mut impl TextMetrics) -> bool {
        if !self.text_selection.dragging {
            return false;
        }
        let Some(b) = self.selectable_near(x, y) else {
            return false;
        };
        let at = Self::offset_in(b, x, y, m);
        let s = &self.text_selection;
        let (anchor, focus) = match (s.unit, &s.pivot) {
            // A double- or triple-click drag keeps what the press selected
            // and adds whole words (or paragraphs) toward the pointer.
            (unit @ (2 | 3), Some((pivot, start, end))) => {
                let (from, to) = if unit == 2 {
                    word_at(&b.text, at)
                } else {
                    (0, b.text.len())
                };
                let index = |id: &str| self.boxes.iter().position(|b| b.id == id);
                let before = match (index(&b.id), index(pivot)) {
                    (Some(i), Some(p)) => (i, at) < (p, *start),
                    _ => false,
                };
                if before {
                    ((pivot.clone(), *end), (b.id.clone(), from))
                } else {
                    let to = if b.id == *pivot { to.max(*end) } else { to };
                    ((pivot.clone(), *start), (b.id.clone(), to))
                }
            }
            _ => match &s.anchor {
                Some(anchor) => (anchor.clone(), (b.id.clone(), at)),
                None => return false,
            },
        };
        if s.anchor.as_ref() == Some(&anchor) && s.focus.as_ref() == Some(&focus) {
            return false;
        }
        self.text_selection.anchor = Some(anchor);
        self.text_selection.focus = Some(focus);
        self.refresh_text_selection(m);
        true
    }

    /// The press ended. Returns whether a selection drag was in progress.
    pub fn text_select_release(&mut self) -> bool {
        std::mem::take(&mut self.text_selection.dragging)
    }

    pub fn text_selecting(&self) -> bool {
        self.text_selection.dragging
    }

    pub fn clear_text_selection(&mut self) -> bool {
        let s = &mut self.text_selection;
        let had = s.anchor.is_some();
        s.anchor = None;
        s.focus = None;
        s.pivot = None;
        s.dragging = false;
        s.text.clear();
        s.rects.clear();
        had
    }

    /// Select all selectable text currently laid out.
    pub fn select_all_text(&mut self, m: &mut impl TextMetrics) -> bool {
        let first = self.boxes.iter().find(|b| selectable(b));
        let last = self.boxes.iter().rev().find(|b| selectable(b));
        let (Some(first), Some(last)) = (first, last) else {
            return false;
        };
        self.text_selection.anchor = Some((first.id.clone(), 0));
        self.text_selection.focus = Some((last.id.clone(), last.text.len()));
        self.refresh_text_selection(m);
        true
    }

    /// The selected text, if any.
    pub fn selected_text(&self) -> Option<&str> {
        Some(self.text_selection.text.as_str()).filter(|t| !t.is_empty())
    }

    /// Recompute the selected text and highlight rectangles from the boxes
    /// laid out now. Ends whose boxes are gone keep the last text.
    pub(super) fn refresh_text_selection(&mut self, m: &mut impl TextMetrics) {
        let s = &self.text_selection;
        let (Some(anchor), Some(focus)) = (s.anchor.clone(), s.focus.clone()) else {
            return;
        };
        let index = |id: &str| self.boxes.iter().position(|b| b.id == id);
        let (Some(a), Some(f)) = (index(&anchor.0), index(&focus.0)) else {
            self.text_selection.rects.clear();
            return;
        };
        let ((start_box, start_at), (end_box, end_at)) = if (a, anchor.1) <= (f, focus.1) {
            ((a, anchor.1), (f, focus.1))
        } else {
            ((f, focus.1), (a, anchor.1))
        };
        let mut parts = Vec::new();
        let mut rects = Vec::new();
        for (i, b) in self
            .boxes
            .iter()
            .enumerate()
            .take(end_box + 1)
            .skip(start_box)
        {
            if !selectable(b) {
                continue;
            }
            let from = if i == start_box {
                start_at.min(b.text.len())
            } else {
                0
            };
            let to = if i == end_box {
                end_at.min(b.text.len())
            } else {
                b.text.len()
            };
            if from >= to || !b.text.is_char_boundary(from) || !b.text.is_char_boundary(to) {
                continue;
            }
            parts.push(&b.text[from..to]);
            let r = b.bounds.bounds();
            let boxes: Vec<[f32; 4]> = m
                .text_range_rects(&b.text, r, &b.text_style, from..to)
                .into_iter()
                .map(|[x, y, w, h, _]| [x - r[0], y - r[1], w, h])
                .collect();
            if !boxes.is_empty() {
                rects.push((b.id.clone(), boxes));
            }
        }
        self.text_selection.text = parts.join("\n");
        self.text_selection.rects = rects;
    }

    /// Report a changed selection to the selectable container's
    /// `on_select_text`. Hosts call this after input; returns whether an
    /// application callback ran.
    pub fn dispatch_text_selection<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<bool, DocumentError> {
        let text = self.text_selection.text.clone();
        if self.text_selection.dragging || self.text_selection.reported.as_ref() == Some(&text) {
            return Ok(false);
        }
        let handler = self
            .text_selection
            .anchor
            .as_ref()
            .and_then(|(id, _)| self.boxes.iter().find(|b| b.id == *id))
            .or_else(|| self.boxes.iter().find(|b| b.style.select_handler.is_some()))
            .and_then(|b| b.style.select_handler.clone());
        self.text_selection.reported = Some(text.clone());
        let Some(name) = handler else {
            return Ok(false);
        };
        handlers
            .dispatch(&name, Event::TextChanged(text), ctx)
            .map_err(|e| DocumentError(e.to_string()))?;
        Ok(true)
    }

    /// Selection highlights for box `b`, drawn before its text.
    pub(super) fn paint_text_selection<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
        b: &LayoutBox,
    ) -> Option<PaintBatch<P::Shape, P::Glyph>> {
        let (_, rects) = self
            .text_selection
            .rects
            .iter()
            .find(|(id, _)| *id == b.id)?;
        let mut color = ui.colors().accent;
        color[3] *= 0.28;
        let [bx, by, _, _] = b.bounds.bounds();
        let mut shapes = Vec::new();
        for [x, y, w, h] in rects {
            p.draw_rect(&mut shapes, bx + x, by + y, w.max(1.), *h, color);
        }
        Some(PaintBatch {
            clip: b.clip,
            shapes,
            glyphs: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn words_split_at_unicode_boundaries() {
        assert_eq!(super::word_at("hello brave world", 8), (6, 11));
        assert_eq!(super::word_at("hello", 5), (0, 5));
        assert_eq!(super::word_at("naïve café", 8), (7, 12));
    }
}
