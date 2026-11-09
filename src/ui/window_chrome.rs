//! Application-drawn window controls. These emit intent; only a native host
//! performs window operations. Browser previews may handle the same callbacks.
use super::*;
use std::{cell::RefCell, rc::Rc};

/// A window-control intent, independent of winit or a particular platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowAction {
    Minimize,
    ToggleMaximize,
    /// Request a normal close, including the application's unsaved-work guard.
    Close,
}

/// Optional per-control availability. `bool` converts to this state with all
/// controls enabled, preserving the simple `window_controls(maximized, f)` form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowControlsState {
    pub maximized: bool,
    pub can_minimize: bool,
    pub can_maximize: bool,
    pub can_close: bool,
}
impl Default for WindowControlsState {
    fn default() -> Self {
        Self {
            maximized: false,
            can_minimize: true,
            can_maximize: true,
            can_close: true,
        }
    }
}
impl From<bool> for WindowControlsState {
    fn from(maximized: bool) -> Self {
        Self {
            maximized,
            ..Self::default()
        }
    }
}

/// Themed, accessible minimize / maximize-or-restore / close buttons.
/// Place these inside a `.drag_region(true)` header: interactive children are
/// excluded from dragging. Connect to `Application::window_actions` for native
/// behavior. This function does not close a process or perform OS calls itself.
/// The returned row supports ordinary `.gap`, `.padding` and style modifiers.
pub fn window_controls<C: 'static>(
    state: impl Into<WindowControlsState>,
    action: impl FnMut(&mut C, WindowAction) + 'static,
) -> Container<C> {
    let state = state.into();
    let maximized = state.maximized;
    let action = Rc::new(RefCell::new(action));
    let buttons = [
        WindowAction::Minimize,
        WindowAction::ToggleMaximize,
        WindowAction::Close,
    ]
    .into_iter()
    .map(|intent| {
        let action = action.clone();
        let enabled = match intent {
            WindowAction::Minimize => state.can_minimize,
            WindowAction::ToggleMaximize => state.can_maximize,
            WindowAction::Close => state.can_close,
        };
        let label = match intent {
            WindowAction::Minimize => "Minimize window",
            WindowAction::ToggleMaximize if maximized => "Restore window",
            WindowAction::ToggleMaximize => "Maximize window",
            WindowAction::Close => "Close window",
        };
        let glyph = canvas(move |c| {
            let p = VectorPath::new;
            match intent {
                WindowAction::Minimize => c.stroke(
                    p().move_to(3., 8.).line_to(13., 8.),
                    ColorToken::BaseContent,
                    1.25,
                ),
                WindowAction::ToggleMaximize => {
                    if maximized {
                        c.stroke(
                            p().move_to(6., 5.)
                                .line_to(6., 3.)
                                .line_to(13., 3.)
                                .line_to(13., 10.)
                                .line_to(11., 10.),
                            ColorToken::BaseContent,
                            1.25,
                        );
                    }
                    c.stroke(
                        p().move_to(3., 5.)
                            .line_to(11., 5.)
                            .line_to(11., 13.)
                            .line_to(3., 13.)
                            .close(),
                        ColorToken::BaseContent,
                        1.25,
                    );
                }
                WindowAction::Close => {
                    c.stroke(
                        p().move_to(4., 4.).line_to(12., 12.),
                        ColorToken::BaseContent,
                        1.25,
                    );
                    c.stroke(
                        p().move_to(12., 4.).line_to(4., 12.),
                        ColorToken::BaseContent,
                        1.25,
                    );
                }
            }
        })
        .width(16.)
        .height(16.);
        zstack![
            button("")
                .accessible_label(label)
                .enabled(enabled)
                .variant(ButtonVariant::Ghost)
                .width(36.)
                .height(30.)
                .rounded(6.)
                .hover_style(move |s| s.background(if intent == WindowAction::Close {
                    ColorToken::Error.alpha(20)
                } else {
                    ColorToken::BaseContent.alpha(10)
                }))
                .on_click(move |context| (action.borrow_mut())(context, intent)),
            glyph,
        ]
        .width(36.)
        .height(30.)
        .into()
    });
    hstack(buttons)
        .gap(2., Axis::Horizontal)
        .align(Align::Center)
}
