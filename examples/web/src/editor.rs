//! Small application-owned code document for browser-host QA, not a production editor.
use super::Demo;
use std::ops::Range;
use volt_ui_kit::{
    declarative::{CodeInput, CodeKey, CodeLine, CodePosition, CodeSelection},
    ui::prelude::*,
};
pub struct Editor {
    pub text: String,
    pub selection: CodeSelection,
    pub changes: u32,
    undo: Vec<(String, CodeSelection)>,
    redo: Vec<(String, CodeSelection)>,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            text: "fn main() {\n    println!(\"Hello, 🌿\");\n}\n".into(),
            selection: CodeSelection::caret(CodePosition::default()),
            changes: 0,
            undo: vec![],
            redo: vec![],
        }
    }
}
impl Editor {
    fn byte(&self, p: CodePosition) -> usize {
        let start = self
            .text
            .split_inclusive('\n')
            .take(p.line)
            .map(str::len)
            .sum::<usize>()
            .min(self.text.len());
        let line = self.text[start..].split('\n').next().unwrap_or("");
        start
            + line
                .char_indices()
                .nth(p.column)
                .map_or(line.len(), |(i, _)| i)
    }
    fn position(&self, byte: usize) -> CodePosition {
        let prefix = &self.text[..byte];
        CodePosition::new(
            prefix.bytes().filter(|&b| b == b'\n').count(),
            prefix.rsplit('\n').next().unwrap_or("").chars().count(),
        )
    }
    fn range(&self) -> Range<usize> {
        let a = self.byte(self.selection.anchor);
        let b = self.byte(self.selection.head);
        a.min(b)..a.max(b)
    }
    fn replace(&mut self, range: Range<usize>, text: &str) {
        if self.text.len() - range.len() + text.len() > 48000 {
            return;
        }
        let lines = self.text.bytes().filter(|&b| b == b'\n').count()
            - self.text[range.clone()]
                .bytes()
                .filter(|&b| b == b'\n')
                .count()
            + text.bytes().filter(|&b| b == b'\n').count()
            + 1;
        if lines > 512 {
            return;
        }
        self.undo.push((self.text.clone(), self.selection));
        if self.undo.len() > 32 {
            self.undo.remove(0);
        }
        self.redo.clear();
        let at = range.start + text.len();
        self.text.replace_range(range, text);
        self.selection = CodeSelection::caret(self.position(at));
        self.changes = self.changes.wrapping_add(1);
    }
    pub fn input(&mut self, input: CodeInput) -> Option<String> {
        match input {
            CodeInput::Insert(s) | CodeInput::Paste(s) => {
                let clean: String = s
                    .replace("\r\n", "\n")
                    .replace('\r', "\n")
                    .replace('\t', "    ")
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\n')
                    .collect();
                self.replace(self.range(), &clean);
            }
            CodeInput::Press { at, modifiers, .. } => {
                if !modifiers.shift {
                    self.selection.anchor = at;
                }
                self.selection.head = at;
            }
            CodeInput::Drag { at } => self.selection.head = at,
            CodeInput::Copy | CodeInput::Cut => {
                let text = self.text[self.range()].to_owned();
                if input == CodeInput::Cut {
                    self.replace(self.range(), "");
                }
                return Some(text);
            }
            CodeInput::Key { key, modifiers } => match key {
                CodeKey::SelectAll => {
                    self.selection = CodeSelection {
                        anchor: CodePosition::default(),
                        head: self.position(self.text.len()),
                    }
                }
                CodeKey::Undo | CodeKey::Redo => {
                    let previous = if key == CodeKey::Undo {
                        self.undo.pop()
                    } else {
                        self.redo.pop()
                    };
                    if let Some((text, selection)) = previous {
                        let current = (std::mem::replace(&mut self.text, text), self.selection);
                        if key == CodeKey::Undo {
                            self.redo.push(current);
                        } else {
                            self.undo.push(current);
                        }
                        self.selection = selection;
                        self.changes = self.changes.wrapping_add(1);
                    }
                }
                CodeKey::Enter => self.replace(self.range(), "\n"),
                CodeKey::Tab => self.replace(self.range(), "    "),
                CodeKey::Backspace | CodeKey::Delete => {
                    let mut range = self.range();
                    if range.is_empty() {
                        if key == CodeKey::Backspace {
                            range.start = self.text[..range.start]
                                .char_indices()
                                .next_back()
                                .map_or(0, |(i, _)| i);
                        } else {
                            range.end += self.text[range.end..]
                                .chars()
                                .next()
                                .map_or(0, char::len_utf8);
                        }
                    }
                    if !range.is_empty() {
                        self.replace(range, "");
                    }
                }
                _ => {
                    let head = self.byte(self.selection.head);
                    let at = match key {
                        CodeKey::Left => self.text[..head]
                            .char_indices()
                            .next_back()
                            .map_or(0, |(i, _)| i),
                        CodeKey::Right => {
                            head + self.text[head..].chars().next().map_or(0, char::len_utf8)
                        }
                        CodeKey::Home => self.text[..head].rfind('\n').map_or(0, |i| i + 1),
                        CodeKey::End => {
                            head + self.text[head..].split('\n').next().unwrap_or("").len()
                        }
                        CodeKey::Up => self.byte(CodePosition::new(
                            self.selection.head.line.saturating_sub(1),
                            self.selection.head.column,
                        )),
                        CodeKey::Down => self.byte(CodePosition::new(
                            self.selection.head.line + 1,
                            self.selection.head.column,
                        )),
                        _ => head,
                    };
                    self.selection.head = self.position(at);
                    if !modifiers.shift {
                        self.selection.anchor = self.selection.head;
                    }
                }
            },
            _ => {}
        }
        None
    }
}
pub fn view(app: &Demo) -> Element<Demo> {
    vstack![
        text("Rust code surface + native IME").font_size(22.).font_weight(600),
        text("The DOM holds only a write-only keyboard/IME sink. Rust owns the document and selections.").font_size(13.).color(ColorToken::Muted),
        checkbox("Read-only editor",app.editor_readonly).key("code-readonly").on_change(|a:&mut Demo,v|a.editor_readonly=v),
        text(format!("{} committed edits",app.editor.changes)).key("code-edits").font_size(12.).color(ColorToken::Muted),
        code_view(app.editor.text.split('\n').count()).key("code")
            .accessible_label("Rust source editor")
            .lines(0,app.editor.text.split('\n').map(CodeLine::plain).collect::<Vec<_>>())
            .selections(vec![app.editor.selection],0)
            .reveal(app.editor.changes)
            .font("JetBrains Mono, monospace",14.,22.)
            .read_only(app.editor_readonly)
            .height(210.).fill_width()
            .on_input(|a:&mut Demo,input|a.editor.input(input)),
    ].gap(12.,Axis::Vertical).into()
}
