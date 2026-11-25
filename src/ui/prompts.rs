//! Alerts and action sheets: a question that needs an answer now. On iOS the
//! host shows UIKit's own; elsewhere Volt draws them.
//!
//! Ask only when an action can't be undone or costs something. When it can
//! be undone, do it and offer Undo instead.
use super::*;
use crate::declarative::document::OverlayKind;
use crate::declarative::{AccessRole, Prompt as PromptSpec, PromptButton};
use std::{cell::RefCell, rc::Rc};

type Press<C> = Rc<RefCell<dyn FnMut(&mut C)>>;

/// One answer: see [`prompt_action`].
pub struct PromptAction<C> {
    label: String,
    destructive: bool,
    press: Option<Press<C>>,
}
/// An answer in an alert or action sheet. Say what happens: "Delete note",
/// not "OK".
pub fn prompt_action<C>(label: impl Into<String>) -> PromptAction<C> {
    PromptAction {
        label: label.into(),
        destructive: false,
        press: None,
    }
}
impl<C> PromptAction<C> {
    /// Removes or loses something: shown in red.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
    pub fn on_press(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.press = Some(Rc::new(RefCell::new(f)));
        self
    }
}

/// See [`alert_dialog`] and [`action_sheet`].
pub struct Prompt<C> {
    sheet: bool,
    key: Option<String>,
    title: String,
    message: String,
    actions: Vec<PromptAction<C>>,
    cancel: Option<String>,
    open: bool,
    close: Option<Press<C>>,
}

/// A question in the middle of the screen. Use it for a decision that can't
/// wait: two answers at most, plus Cancel.
///
/// ```rust,ignore
/// alert_dialog("Delete 3 notes?")
///     .key("delete-alert")
///     .message("They'll be gone from every device.")
///     .action(prompt_action("Delete").destructive().on_press(|a: &mut App| a.delete()))
///     .cancel("Cancel")
///     .open(app.asking)
///     .on_close(|a: &mut App| a.asking = false)
/// ```
pub fn alert_dialog<C>(title: impl Into<String>) -> Prompt<C> {
    prompt(false, title.into())
}
/// Choices that rise from the bottom, near the thumb, with Cancel apart at
/// the foot. The title may be empty.
pub fn action_sheet<C>(title: impl Into<String>) -> Prompt<C> {
    prompt(true, title.into())
}
fn prompt<C>(sheet: bool, title: String) -> Prompt<C> {
    Prompt {
        sheet,
        key: None,
        title,
        message: String::new(),
        actions: Vec::new(),
        cancel: None,
        open: false,
        close: None,
    }
}
impl<C> Prompt<C> {
    /// Required. Buttons are keyed `{key}-action-{n}` and `{key}-cancel`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// A sentence or two under the title.
    pub fn message(mut self, text: impl Into<String>) -> Self {
        self.message = text.into();
        self
    }
    pub fn action(mut self, action: PromptAction<C>) -> Self {
        if self.actions.len() < 8 {
            self.actions.push(action);
        }
        self
    }
    /// A button that closes without doing anything. Action sheets always
    /// have one; it says "Cancel" unless you give it a label.
    pub fn cancel(mut self, label: impl Into<String>) -> Self {
        self.cancel = Some(label.into());
        self
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    /// Required: close it (app state). Called after any answer, and when a
    /// sheet is dismissed by tapping outside it.
    pub fn on_close(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.close = Some(Rc::new(RefCell::new(f)));
        self
    }
}

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}

/// A button that closes the prompt, then runs its action.
fn answer<C: 'static>(
    key: String,
    label: &str,
    ink: ColorToken,
    close: Press<C>,
    press: Option<Press<C>>,
) -> Container<C> {
    let mut b = hstack![text(label.to_owned()).color(ink)]
        .key(key)
        .align(Align::Center)
        .accessible_label(label.to_owned())
        .pressed_style(|s| s.background(ColorToken::BaseContent.alpha(18)))
        .on_click_with(move |c: &mut C, _| {
            (close.borrow_mut())(c);
            if let Some(f) = &press {
                (f.borrow_mut())(c);
            }
        });
    b.0.node.access_role = Some(AccessRole::Button);
    b
}

/// Material 3's raised surface for dialogs and sheets.
const SURFACE: &str = "@base-200|@base-300";

impl<C: 'static> From<Prompt<C>> for Element<C> {
    fn from(p: Prompt<C>) -> Self {
        let Some(key) = p.key.filter(|k| !k.is_empty()) else {
            return invalid("alert_dialog and action_sheet require a key");
        };
        let Some(close) = p.close else {
            return invalid("alert_dialog and action_sheet require on_close");
        };
        if p.actions.is_empty() && p.cancel.is_none() {
            return invalid("a prompt needs an action or a cancel button");
        }
        // Not open: nothing in the tree, so nothing takes input.
        let mut root = Element::new(Kind::ZStack);
        root.node.id = key.clone();
        root.auto_id = false;
        if !p.open {
            return root;
        }
        let cancel_label = p
            .cancel
            .clone()
            .or_else(|| p.sheet.then(|| "Cancel".to_owned()));
        let cancel_key = format!("{key}-cancel");

        // What a host that shows its own prompt reads.
        let mut buttons: Vec<PromptButton> = p
            .actions
            .iter()
            .enumerate()
            .map(|(i, a)| PromptButton {
                id: format!("{key}-action-{i}"),
                label: a.label.clone(),
                destructive: a.destructive,
                cancel: false,
            })
            .collect();
        if let Some(label) = &cancel_label {
            buttons.push(PromptButton {
                id: cancel_key.clone(),
                label: label.clone(),
                destructive: false,
                cancel: true,
            });
        }
        let spec = PromptSpec {
            sheet: p.sheet,
            title: p.title.clone(),
            message: p.message.clone(),
            actions: buttons,
        };

        // Where the host doesn't show its own (Android, desktop), Volt
        // draws Material 3's: a dialog, or a modal bottom sheet of choices.
        let ink = |a: &PromptAction<C>| {
            if a.destructive {
                ColorToken::Error
            } else {
                ColorToken::Primary
            }
        };
        let content: Element<C> = if p.sheet {
            let mut parts: Vec<Element<C>> = vec![hstack![vstack(Vec::<Element<C>>::new())
                .width(32.)
                .height(4.)
                .rounded(Rounding::Full)
                .background(ColorToken::Muted)]
            .key(format!("{key}-handle"))
            .width_fraction(1.)
            .justify(Justify::Center)
            .padding(16., Edge::Vertical)
            .into()];
            if !p.title.is_empty() || !p.message.is_empty() {
                let mut head: Vec<Element<C>> = Vec::new();
                if !p.title.is_empty() {
                    head.push(text(p.title.clone()).font_size(16.).font_weight(600).into());
                }
                if !p.message.is_empty() {
                    head.push(
                        text(p.message.clone())
                            .font_size(14.)
                            .color(ColorToken::Muted)
                            .into(),
                    );
                }
                parts.push(
                    vstack(head)
                        .key(format!("{key}-head"))
                        .gap(4., Axis::Vertical)
                        .padding(24., Edge::Horizontal)
                        .padding(8., Edge::Bottom)
                        .into(),
                );
            }
            // Material sheets have no Cancel row: the scrim and Back close.
            for (i, a) in p.actions.into_iter().enumerate() {
                let tone = if a.destructive {
                    ColorToken::Error
                } else {
                    ColorToken::BaseContent
                };
                parts.push(
                    answer(
                        format!("{key}-action-{i}"),
                        &a.label,
                        tone,
                        close.clone(),
                        a.press,
                    )
                    .width_fraction(1.)
                    .height(56.)
                    .padding(24., Edge::Horizontal)
                    .font_size(16.)
                    .into(),
                );
            }
            parts.push(vstack(Vec::<Element<C>>::new()).height(16.).into());
            let mut card = vstack(parts)
                .key(format!("{key}-card"))
                .width_fraction(1.)
                .max_width(640.)
                .background(SURFACE);
            card.0.node.style.corners = Some([
                Rounding::Radius(28.),
                Rounding::Radius(28.),
                Rounding::Radius(0.),
                Rounding::Radius(0.),
            ]);
            card.into()
        } else {
            let mut body: Vec<Element<C>> = Vec::new();
            if !p.title.is_empty() {
                body.push(text(p.title.clone()).font_size(24.).into());
            }
            if !p.message.is_empty() {
                body.push(
                    text(p.message.clone())
                        .font_size(14.)
                        .color(ColorToken::Muted)
                        .into(),
                );
            }
            // Text buttons at the trailing edge: dismiss first, confirm last.
            let mut row: Vec<Element<C>> = vec![spacer().into()];
            if let Some(label) = &cancel_label {
                row.push(
                    answer(
                        cancel_key.clone(),
                        label,
                        ColorToken::Primary,
                        close.clone(),
                        None,
                    )
                    .height(40.)
                    .padding(12., Edge::Horizontal)
                    .rounded(Rounding::Full)
                    .font_size(14.)
                    .font_weight(600)
                    .into(),
                );
            }
            for (i, a) in p.actions.into_iter().enumerate() {
                let tone = ink(&a);
                row.push(
                    answer(
                        format!("{key}-action-{i}"),
                        &a.label,
                        tone,
                        close.clone(),
                        a.press,
                    )
                    .height(40.)
                    .padding(12., Edge::Horizontal)
                    .rounded(Rounding::Full)
                    .font_size(14.)
                    .font_weight(600)
                    .into(),
                );
            }
            body.push(
                hstack(row)
                    .key(format!("{key}-buttons"))
                    .gap(8., Axis::Horizontal)
                    .padding(8., Edge::Top)
                    .into(),
            );
            vstack(body)
                .key(format!("{key}-card"))
                .width(312.)
                .gap(16., Axis::Vertical)
                .padding(24., Edge::All)
                .rounded(28.)
                .background(SURFACE)
                .into()
        };

        // A sheet goes when you tap outside it; an alert waits for an answer.
        let dismiss = close.clone();
        let mut layer = super::dialogs::overlay_scope(
            content,
            format!("{key}-layer"),
            OverlayKind::Modal,
            Box::new(move |c: &mut C| (dismiss.borrow_mut())(c)),
        );
        let overlay = layer.node.overlay.as_mut().unwrap();
        overlay.backdrop_dismiss = p.sheet;
        overlay.escape_dismiss = true;
        // Material's scrim: 32% of black.
        layer.node.style.background = Some("#00000052".into());
        if p.sheet {
            layer.node.flow = Some(Flow::Bottom);
        }
        layer.node.effects.transition = Some(super::dialogs::popup_motion());
        layer.node.ext.prompt = Some(Box::new(spec));
        root.children.push(layer);
        root
    }
}
