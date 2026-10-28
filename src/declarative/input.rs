//! Opt-in control input, separate from legacy activation and editor events.
use super::{HandlerError, Handlers};

/// Physical modifier state captured by the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}
impl InputModifiers {
    /// Command on macOS, Control on other supported desktops.
    pub fn primary(self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.control
        }
    }
}

/// Origin of an opt-in activation; keyboard and accessibility are not mouse clicks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSource {
    Pointer,
    Keyboard,
    Accessibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerActivation {
    pub modifiers: InputModifiers,
    pub source: InputSource,
}

/// Restricted navigation/action keys; never arbitrary text, Tab, or application shortcuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKey {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    Enter,
    Space,
    Escape,
    SelectAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputKeyEvent {
    pub key: InputKey,
    pub modifiers: InputModifiers,
    pub repeat: bool,
}

type ActivationCallback<'a, C> = Box<dyn FnMut(&mut C, PointerActivation) + 'a>;
type KeyInputCallback<'a, C> = Box<dyn FnMut(&mut C, InputKeyEvent) -> bool + 'a>;
type CodeInputCallback<'a, C> = Box<dyn FnMut(&mut C, super::CodeInput) -> Option<String> + 'a>;
type CodeCaretCallback<'a, C> = Box<dyn FnMut(&mut C, Option<[f32; 4]>) + 'a>;
type CodeFoldCallback<'a, C> = Box<dyn FnMut(&mut C, usize) + 'a>;

pub(super) enum InputCallback<'a, C> {
    CodeFold(CodeFoldCallback<'a, C>),
    Activate(ActivationCallback<'a, C>),
    Key(KeyInputCallback<'a, C>),
    /// A code view's edits; returns clipboard text for Copy and Cut.
    Code(CodeInputCallback<'a, C>),
    /// Opt-in painted caret geometry, independent of editing events.
    CodeCaret(CodeCaretCallback<'a, C>),
}
impl<'a, C> Handlers<'a, C> {
    fn insert_input(
        &mut self,
        name: &str,
        callback: InputCallback<'a, C>,
    ) -> Result<(), HandlerError> {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(HandlerError::InvalidName);
        }
        if self.entries.contains_key(name) || self.input_entries.contains_key(name) {
            return Err(HandlerError::Duplicate(name.into()));
        }
        self.input_entries.insert(name.into(), callback);
        Ok(())
    }
    pub fn on_activate_with(
        &mut self,
        name: &str,
        f: impl FnMut(&mut C, PointerActivation) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert_input(name, InputCallback::Activate(Box::new(f)))
    }
    /// A false result means unhandled and no state mutation; the host retains its
    /// existing keyboard behavior. True requests recomposition and consumes the key.
    pub fn on_input_key(
        &mut self,
        name: &str,
        f: impl FnMut(&mut C, InputKeyEvent) -> bool + 'a,
    ) -> Result<(), HandlerError> {
        self.insert_input(name, InputCallback::Key(Box::new(f)))
    }
    /// A code view's input handler. Return the selected text for
    /// `CodeInput::Copy` and `CodeInput::Cut`; `None` otherwise.
    pub fn on_code_input(
        &mut self,
        name: &str,
        f: impl FnMut(&mut C, super::CodeInput) -> Option<String> + 'a,
    ) -> Result<(), HandlerError> {
        self.insert_input(name, InputCallback::Code(Box::new(f)))
    }
    pub(crate) fn dispatch_code_input(
        &mut self,
        name: &str,
        input: super::CodeInput,
        context: &mut C,
    ) -> Result<Option<String>, HandlerError> {
        match self.input_entries.get_mut(name) {
            Some(InputCallback::Code(f)) => Ok(f(context, input)),
            Some(_) => Err(HandlerError::InvalidValue),
            None => Err(HandlerError::Missing(name.into())),
        }
    }
    /// Receive the visible primary caret's painted rectangle in window logical
    /// points, or `None` when outside the clipped text viewport. Reports happen
    /// only after painting and only when geometry changes.
    pub fn on_code_caret(
        &mut self,
        name: &str,
        f: impl FnMut(&mut C, Option<[f32; 4]>) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert_input(name, InputCallback::CodeCaret(Box::new(f)))
    }
    /// Optional code-gutter action, separate from editing/selection events.
    pub fn on_code_fold(
        &mut self,
        name: &str,
        f: impl FnMut(&mut C, usize) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert_input(name, InputCallback::CodeFold(Box::new(f)))
    }
    pub(crate) fn dispatch_code_fold(
        &mut self,
        name: &str,
        row: usize,
        context: &mut C,
    ) -> Result<(), HandlerError> {
        match self.input_entries.get_mut(name) {
            Some(InputCallback::CodeFold(f)) => {
                f(context, row);
                Ok(())
            }
            Some(_) => Err(HandlerError::InvalidValue),
            None => Err(HandlerError::Missing(name.into())),
        }
    }
    pub(crate) fn dispatch_code_caret(
        &mut self,
        name: &str,
        rect: Option<[f32; 4]>,
        context: &mut C,
    ) -> Result<(), HandlerError> {
        match self.input_entries.get_mut(name) {
            Some(InputCallback::CodeCaret(f)) => {
                f(context, rect);
                Ok(())
            }
            Some(_) => Err(HandlerError::InvalidValue),
            None => Err(HandlerError::Missing(name.into())),
        }
    }
    pub(crate) fn validate_input(&self, name: &str, key: bool) -> Result<(), HandlerError> {
        match self.input_entries.get(name) {
            Some(InputCallback::Key(_)) if key => Ok(()),
            Some(InputCallback::Activate(_)) if !key => Ok(()),
            Some(_) => Err(HandlerError::InvalidValue),
            None => Err(HandlerError::Missing(name.into())),
        }
    }
    pub(crate) fn dispatch_activation(
        &mut self,
        name: &str,
        event: PointerActivation,
        context: &mut C,
    ) -> Result<(), HandlerError> {
        self.validate_input(name, false)?;
        if let Some(InputCallback::Activate(f)) = self.input_entries.get_mut(name) {
            f(context, event);
        }
        Ok(())
    }
    pub(crate) fn dispatch_input_key(
        &mut self,
        name: &str,
        event: InputKeyEvent,
        context: &mut C,
    ) -> Result<bool, HandlerError> {
        self.validate_input(name, true)?;
        if let Some(InputCallback::Key(f)) = self.input_entries.get_mut(name) {
            Ok(f(context, event))
        } else {
            Ok(false)
        }
    }
}
