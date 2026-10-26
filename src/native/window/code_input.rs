//! Keys, clicks and the clipboard for a focused code view. Keys arrive with
//! their modifiers so the application applies its own conventions (Option
//! moves by word, Command by line on macOS).
use super::*;
use crate::declarative::{CodeInput, CodeKey};

impl<C: 'static> App<C> {
    /// Route a key press to the focused code view. Returns true when consumed.
    /// Escape stays with overlays unless there are several carets to collapse.
    pub(super) fn code_keyboard(&mut self, event: &winit::event::KeyEvent) -> Result<bool, String> {
        if !self.runtime.code_view_focused() {
            return Ok(false);
        }
        if self.runtime.ime_active() {
            if event.logical_key == Key::Named(NamedKey::Escape) && self.runtime.ime_cancel() {
                self.redraw();
            }
            return Ok(true);
        }
        let modifiers = self.input_modifiers();
        let primary = modifiers.primary();
        let key = match &event.logical_key {
            Key::Character(c) if primary && !modifiers.alt => match c.to_lowercase().as_str() {
                "a" => CodeKey::SelectAll,
                "z" if modifiers.shift => CodeKey::Redo,
                "z" => CodeKey::Undo,
                "y" if !cfg!(target_os = "macos") => CodeKey::Redo,
                "c" => return self.code_copy(false),
                "x" => return self.code_copy(true),
                "v" => {
                    return match self.paste_from_clipboard() {
                        Some(text) => self.code_send(CodeInput::Paste(text)),
                        None => Ok(true),
                    };
                }
                _ => return Ok(false),
            },
            Key::Named(named) => match named {
                NamedKey::ArrowLeft => CodeKey::Left,
                NamedKey::ArrowRight => CodeKey::Right,
                NamedKey::ArrowUp => CodeKey::Up,
                NamedKey::ArrowDown => CodeKey::Down,
                NamedKey::Home => CodeKey::Home,
                NamedKey::End => CodeKey::End,
                NamedKey::PageUp => CodeKey::PageUp,
                NamedKey::PageDown => CodeKey::PageDown,
                NamedKey::Backspace => CodeKey::Backspace,
                NamedKey::Delete => CodeKey::Delete,
                NamedKey::Enter => CodeKey::Enter,
                NamedKey::Tab if !modifiers.control => CodeKey::Tab,
                NamedKey::Escape if self.runtime.code_view_multiple_selections() => CodeKey::Escape,
                NamedKey::Space if !primary && !modifiers.control => {
                    return self.code_send(CodeInput::Insert(" ".into()));
                }
                _ => return Ok(false),
            },
            _ if primary || modifiers.control => return Ok(false),
            _ => {
                return match event
                    .text
                    .as_ref()
                    .filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
                {
                    Some(text) => {
                        let text = text.to_string();
                        self.code_send(CodeInput::Insert(text))
                    }
                    None => Ok(false),
                };
            }
        };
        self.code_send(CodeInput::Key { key, modifiers })
    }
    fn code_send(&mut self, input: CodeInput) -> Result<bool, String> {
        let (outcome, _) = self
            .runtime
            .code_input(input, &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        self.activation_key = None;
        self.after_input(outcome)?;
        self.redraw();
        Ok(true)
    }
    /// Copy the application's selected text; a cut removes it only once the
    /// clipboard has accepted it.
    fn code_copy(&mut self, cut: bool) -> Result<bool, String> {
        let (_, text) = self
            .runtime
            .code_input(CodeInput::Copy, &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        let Some(text) = text.filter(|t| !t.is_empty()) else {
            return Ok(true);
        };
        if !self.copy_to_clipboard(text) || !cut || self.runtime.code_view_read_only() {
            return Ok(true);
        }
        self.code_send(CodeInput::Cut)
    }
    /// A left press: true when a code view took it.
    pub(super) fn code_pointer_down(&mut self, x: f32, y: f32) -> Result<bool, String> {
        if !self.runtime.code_view_at(x, y) {
            return Ok(false);
        }
        let clicks = self.count_click([x, y]);
        let modifiers = self.input_modifiers();
        let outcome = self
            .runtime
            .code_pointer_down(
                x,
                y,
                clicks,
                modifiers,
                &mut self.handlers,
                &mut self.context,
            )
            .map_err(|e| e.to_string())?;
        match outcome {
            Some(outcome) => {
                self.activation_key = None;
                self.after_input(outcome)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
    /// Pointer motion during a drag that began in a code view.
    pub(super) fn code_pointer_drag(&mut self, x: f32, y: f32) -> Result<bool, String> {
        match self
            .runtime
            .code_pointer_drag(x, y, &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?
        {
            Some(outcome) => {
                self.after_input(outcome)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
    pub(super) fn code_pointer_up(&mut self) {
        self.runtime.code_pointer_up();
    }
}
