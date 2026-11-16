//! Controlled non-modal quick actions; keyboard focus stays in ordinary buttons.
use super::*;
use std::{cell::RefCell, rc::Rc};
fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}
type Change<C> = Rc<RefCell<dyn FnMut(&mut C, bool)>>;
type Action<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
pub struct SpeedDial<C, T> {
    key: Option<String>,
    label: String,
    items: Vec<(T, String)>,
    open: bool,
    change: Option<Change<C>>,
    action: Option<Action<C, T>>,
    gap: f32,
}
/// Upward expanding floating action menu. Items are `(value, readable label)`.
/// Non-modal: no focus trap or app-level input blocking; Escape closes it.
pub fn speed_dial<C, T, S: Into<String>>(
    label: impl Into<String>,
    items: impl IntoIterator<Item = (T, S)>,
) -> SpeedDial<C, T> {
    SpeedDial {
        key: None,
        label: label.into(),
        items: items.into_iter().map(|(v, s)| (v, s.into())).collect(),
        open: false,
        change: None,
        action: None,
        gap: 8.,
    }
}
impl<C, T> SpeedDial<C, T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    pub fn gap(mut self, pixels: f32) -> Self {
        self.gap = pixels;
        self
    }
    pub fn on_open_change(mut self, f: impl FnMut(&mut C, bool) + 'static) -> Self {
        self.change = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn on_action(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.action = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static, T: Clone + 'static> From<SpeedDial<C, T>> for Element<C> {
    fn from(d: SpeedDial<C, T>) -> Self {
        let Some(key) = d.key.filter(|k| !k.is_empty()) else {
            return invalid("speed dial requires a key");
        };
        let (Some(change), Some(action)) = (d.change, d.action) else {
            return invalid("speed dial requires callbacks");
        };
        if d.label.trim().is_empty()
            || d.items.is_empty()
            || d.items.len() > 12
            || d.items.iter().any(|(_, label)| label.trim().is_empty())
            || !d.gap.is_finite()
            || !(0. ..=64.).contains(&d.gap)
        {
            return invalid("invalid speed dial label, items or spacing");
        }
        let mut rows = Vec::<Element<C>>::new();
        if d.open {
            for (i, (value, label)) in d.items.into_iter().enumerate() {
                let change = change.clone();
                let action = action.clone();
                rows.push(
                    button(label)
                        .key(format!("{key}-action-{i}"))
                        .height(44.)
                        .variant(ButtonVariant::Soft)
                        .transition(Transition::slide(0., 12.).duration_ms(180).stagger(i, 25))
                        .on_click(move |app: &mut C| {
                            (change.borrow_mut())(app, false);
                            (action.borrow_mut())(app, value.clone());
                        })
                        .into(),
                );
            }
        }
        let toggle = change.clone();
        let close = change.clone();
        rows.push(
            fab(if d.open { "×" } else { "+" })
                .key(format!("{key}-toggle"))
                .accessible_label(format!(
                    "{} {}",
                    if d.open { "Close" } else { "Open" },
                    d.label
                ))
                .on_click(move |app: &mut C| (toggle.borrow_mut())(app, !d.open))
                .into(),
        );
        let root = vstack(rows)
            .key(key)
            .align(Align::End)
            .gap(d.gap, Axis::Vertical);
        if d.open {
            root.on_escape(move |app: &mut C| (close.borrow_mut())(app, false))
                .into()
        } else {
            root.into()
        }
    }
}
