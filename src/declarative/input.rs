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

/// A key an application can take for its own shortcuts through
/// `on_key_press`. Tab stays with focus traversal and Escape with
/// `on_escape`, so keyboard users can always move and dismiss.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PressedKey {
    /// A letter, digit or punctuation key as the keyboard layout names it
    /// without modifiers, in lower case: Shift+Cmd+Z arrives as `'z'` with
    /// `shift`, and Option+R on macOS as `'r'` with `alt`.
    Character(char),
    Backspace,
    Delete,
    Enter,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
}
/// A key press nothing else in the window used: no text editor, menu,
/// dialog, application command or focused control took it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyPress {
    pub key: PressedKey,
    pub modifiers: InputModifiers,
    /// Held down and auto-repeating, e.g. for nudging with an arrow.
    pub repeat: bool,
}
impl KeyPress {
    /// Cmd on macOS, Ctrl elsewhere (see [`InputModifiers::primary`]).
    pub fn primary(self) -> bool {
        self.modifiers.primary()
    }
    /// No Cmd, Ctrl, Alt or Meta held; Shift may be.
    pub fn bare(self) -> bool {
        let m = self.modifiers;
        !(m.control || m.alt || m.meta)
    }
    /// The character key, if this is one.
    pub fn char(self) -> Option<char> {
        match self.key {
            PressedKey::Character(c) => Some(c),
            _ => None,
        }
    }
}
type ActivationCallback<'a, C> = Box<dyn FnMut(&mut C, PointerActivation, u8) + 'a>;
type KeyInputCallback<'a, C> = Box<dyn FnMut(&mut C, InputKeyEvent) -> bool + 'a>;
type KeyPressCallback<'a, C> = Box<dyn FnMut(&mut C, KeyPress) -> bool + 'a>;
type PointerCallback<'a, C> = Box<dyn FnMut(&mut C, Pointer) + 'a>;

/// What a pointer surface (`on_pointer`) is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    /// The primary button went down on the surface (not on a control in it).
    Press,
    /// The button is held and the pointer moved, or a modifier changed.
    Drag,
    /// The button that pressed the surface came up, wherever the pointer is.
    Release,
    /// The pointer moved over the surface with no button held.
    Hover,
    /// The pointer left the surface (or the window) after hovering it.
    Leave,
    /// A scroll wheel or two-finger trackpad scroll over the surface.
    Wheel,
    /// A trackpad pinch over the surface.
    Pinch,
}
/// A pointer event for a surface that does its own hit testing, such as a
/// drawing canvas: where the pointer is, what moved, and which modifiers
/// are held. Positions are logical points inside the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pointer {
    pub phase: PointerPhase,
    /// Where the pointer is, inside the surface.
    pub at: [f32; 2],
    /// Drag and Release: the offset from where the press landed.
    pub offset: [f32; 2],
    /// Wheel: the scroll distance in logical points; positive y scrolls
    /// down (content moves up), as a scroll view's offset grows.
    pub delta: [f32; 2],
    /// Pinch: the change in magnification, e.g. 0.1 for 10% larger.
    pub zoom: f32,
    /// Held now (for a press, held when it went down).
    pub modifiers: InputModifiers,
    /// Press: 1, 2 or 3 for a single, double or triple click.
    pub clicks: u8,
}
impl Pointer {
    pub(crate) fn new(phase: PointerPhase, at: [f32; 2], modifiers: InputModifiers) -> Self {
        Self {
            phase,
            at,
            offset: [0.; 2],
            delta: [0.; 2],
            zoom: 0.,
            modifiers,
            clicks: 0,
        }
    }
}
type CodeInputCallback<'a, C> = Box<dyn FnMut(&mut C, super::CodeInput) -> Option<String> + 'a>;
type CodeCaretCallback<'a, C> = Box<dyn FnMut(&mut C, Option<[f32; 4]>) + 'a>;
type CodeFoldCallback<'a, C> = Box<dyn FnMut(&mut C, usize) + 'a>;

pub(super) enum InputCallback<'a, C> {
    CodeFold(CodeFoldCallback<'a, C>),
    Activate(ActivationCallback<'a, C>),
    Key(KeyInputCallback<'a, C>),
    /// A container's application keys (`on_key_press`).
    KeyPress(KeyPressCallback<'a, C>),
    /// A pointer surface's events (`on_pointer`).
    Pointer(PointerCallback<'a, C>),
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
        mut f: impl FnMut(&mut C, PointerActivation) + 'a,
    ) -> Result<(), HandlerError> {
        self.on_activate_counted(name, move |c, event, _| f(c, event))
    }
    /// Like on_activate_with, plus host-supplied mouse click count. Keyboard
    /// and accessibility activations report zero; pointer adapters default to one.
    pub fn on_activate_counted(
        &mut self,
        name: &str,
        f: impl FnMut(&mut C, PointerActivation, u8) + 'a,
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
    /// A container's application keys: a true result consumes the key and
    /// requests recomposition; false leaves it to the window's defaults.
    pub fn on_key_press(
        &mut self,
        name: &str,
        f: impl FnMut(&mut C, KeyPress) -> bool + 'a,
    ) -> Result<(), HandlerError> {
        self.insert_input(name, InputCallback::KeyPress(Box::new(f)))
    }
    /// A pointer surface's events: presses, drags, hover, wheel and pinch.
    pub fn on_pointer(
        &mut self,
        name: &str,
        f: impl FnMut(&mut C, Pointer) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert_input(name, InputCallback::Pointer(Box::new(f)))
    }
    pub(crate) fn validate_pointer(&self, name: &str) -> Result<(), HandlerError> {
        match self.input_entries.get(name) {
            Some(InputCallback::Pointer(_)) => Ok(()),
            Some(_) => Err(HandlerError::InvalidValue),
            None => Err(HandlerError::Missing(name.into())),
        }
    }
    pub(crate) fn dispatch_pointer(
        &mut self,
        name: &str,
        event: Pointer,
        context: &mut C,
    ) -> Result<(), HandlerError> {
        self.validate_pointer(name)?;
        if let Some(InputCallback::Pointer(f)) = self.input_entries.get_mut(name) {
            f(context, event);
        }
        Ok(())
    }
    pub(crate) fn validate_key_press(&self, name: &str) -> Result<(), HandlerError> {
        match self.input_entries.get(name) {
            Some(InputCallback::KeyPress(_)) => Ok(()),
            Some(_) => Err(HandlerError::InvalidValue),
            None => Err(HandlerError::Missing(name.into())),
        }
    }
    pub(crate) fn dispatch_key_press(
        &mut self,
        name: &str,
        event: KeyPress,
        context: &mut C,
    ) -> Result<bool, HandlerError> {
        self.validate_key_press(name)?;
        if let Some(InputCallback::KeyPress(f)) = self.input_entries.get_mut(name) {
            Ok(f(context, event))
        } else {
            Ok(false)
        }
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
        clicks: u8,
        context: &mut C,
    ) -> Result<(), HandlerError> {
        self.validate_input(name, false)?;
        if let Some(InputCallback::Activate(f)) = self.input_entries.get_mut(name) {
            f(context, event, clicks);
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
