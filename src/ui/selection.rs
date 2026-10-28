//! Application-owned, stable-identity multi-selection; no rendering or OS I/O.
//!
//! This model does not install input listeners. Existing button callbacks do not
//! expose pointer modifiers, and text-editing key hooks are not list key hooks.
//! A host must explicitly translate its input into these operations. In
//! particular, a model alone does not make a composed list keyboard accessible.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

/// Semantic modifiers, translated by the host rather than tied to one OS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectionModifiers {
    /// Platform selection modifier: usually Command on macOS, Control elsewhere.
    /// Pointer selection toggles; keyboard movement with this modifier alone
    /// changes focus without changing the selected set.
    pub toggle: bool,
    /// Shift: extend from the stable anchor. Together with `toggle`, add the
    /// range without discarding existing selections.
    pub extend: bool,
}

/// Logical movement in the caller's current selectable-item order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMove {
    Previous,
    Next,
    First,
    Last,
}

/// Invalid replacement order. Failed reconciliation leaves all state intact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionError {
    TooManyItems {
        count: usize,
        maximum: usize,
    },
    /// Index of the second occurrence in the proposed order.
    DuplicateIdentity {
        index: usize,
    },
}

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyItems { count, maximum } => {
                write!(f, "selection order has {count} items; maximum is {maximum}")
            }
            Self::DuplicateIdentity { index } => {
                write!(f, "selection order repeats an identity at index {index}")
            }
        }
    }
}
impl std::error::Error for SelectionError {}

/// Multi-selection identified by stable values, not snapshot indices.
///
/// Reconcile the complete current selectable scope after a sort or data refresh;
/// do not pass only a virtual viewport or filtered-away items will be removed.
/// Disabled items must be omitted. Selection, focus, and anchor are distinct;
/// toggling the final selected item off preserves its focus and anchor.
///
/// Newly discovered items are never implicitly selected, including after
/// [`Self::select_all`]. Clear explicitly when changing locations/scopes.
///
/// ```
/// use volt_ui_kit::ui::prelude::*;
/// let mut selection = SelectionModel::new();
/// selection.reconcile(&["invoice-a", "invoice-b", "invoice-c"])?;
/// selection.select(&"invoice-a", SelectionModifiers::default());
/// selection.select(&"invoice-c", SelectionModifiers { extend: true, ..Default::default() });
/// assert_eq!(selection.len(), 3);
/// selection.reconcile(&["invoice-b", "new-invoice"])?;
/// assert_eq!(selection.selected_in_order().copied().collect::<Vec<_>>(), ["invoice-b"]);
/// # Ok::<(), SelectionError>(())
/// ```
#[derive(Clone, Debug)]
pub struct SelectionModel<T> {
    order: Vec<T>,
    indices: HashMap<T, usize>,
    selected: HashSet<T>,
    focused: Option<T>,
    anchor: Option<T>,
}

impl<T> Default for SelectionModel<T> {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            indices: HashMap::new(),
            selected: HashSet::new(),
            focused: None,
            anchor: None,
        }
    }
}

impl<T: Clone + Eq + Hash> SelectionModel<T> {
    /// Upper bound on identities retained by one model. Payload size per ID is
    /// application-owned; prefer small keys or shared immutable path values.
    pub const MAX_ITEMS: usize = 100_000;

    pub fn new() -> Self {
        Self::default()
    }

    /// Current ordered selectable identities, not only selected items.
    pub fn items(&self) -> &[T] {
        &self.order
    }

    /// Number of selected identities.
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn contains(&self, item: &T) -> bool {
        self.selected.contains(item)
    }

    pub fn focused(&self) -> Option<&T> {
        self.focused.as_ref()
    }

    pub fn anchor(&self) -> Option<&T> {
        self.anchor.as_ref()
    }

    /// Selected IDs in the current presentation order. Iteration is O(n) in
    /// the complete scope; there is no HashSet iteration-order dependency.
    pub fn selected_in_order(&self) -> impl Iterator<Item = &T> {
        self.order.iter().filter(|id| self.selected.contains(*id))
    }

    /// Replace order atomically, retaining only existing selected identities.
    /// A removed focus falls back to the first surviving selected ID; a removed
    /// anchor falls back to the surviving focus. No new item is selected.
    pub fn reconcile(&mut self, items: &[T]) -> Result<(), SelectionError> {
        if items.len() > Self::MAX_ITEMS {
            return Err(SelectionError::TooManyItems {
                count: items.len(),
                maximum: Self::MAX_ITEMS,
            });
        }
        let mut indices = HashMap::with_capacity(items.len());
        for (index, id) in items.iter().enumerate() {
            if indices.insert(id.clone(), index).is_some() {
                return Err(SelectionError::DuplicateIdentity { index });
            }
        }
        self.selected.retain(|id| indices.contains_key(id));
        self.focused = self
            .focused
            .take()
            .filter(|id| indices.contains_key(id))
            .or_else(|| items.iter().find(|id| self.selected.contains(*id)).cloned());
        self.anchor = self
            .anchor
            .take()
            .filter(|id| indices.contains_key(id))
            .or_else(|| self.focused.clone());
        self.order = items.to_vec();
        self.indices = indices;
        Ok(())
    }

    /// Clear selection, focus and anchor, retaining the selectable order.
    pub fn clear(&mut self) {
        self.selected.clear();
        self.focused = None;
        self.anchor = None;
    }

    /// Select only the identities currently known. Future reconcile operations
    /// never grow this set. Keeps valid focus, otherwise focuses the first item.
    pub fn select_all(&mut self) {
        self.selected = self.order.iter().cloned().collect();
        if self.focused.is_none() {
            self.focused = self.order.first().cloned();
        }
        self.anchor = self.focused.clone();
    }

    /// Apply pointer-like selection. Returns false only for an unknown ID
    /// (state untouched); true does not imply the resulting state changed.
    pub fn select(&mut self, item: &T, modifiers: SelectionModifiers) -> bool {
        let Some(&index) = self.indices.get(item) else {
            return false;
        };
        if modifiers.extend {
            self.extend_to(index, modifiers.toggle);
        } else {
            if modifiers.toggle {
                if !self.selected.remove(item) {
                    self.selected.insert(item.clone());
                }
            } else {
                self.selected.clear();
                self.selected.insert(item.clone());
            }
            self.anchor = Some(item.clone());
        }
        self.focused = Some(item.clone());
        true
    }

    /// Move focus and optionally selection in the current order. With no focus,
    /// Next/First starts at the first item, Previous/Last at the last. Movement
    /// clamps at either end. Returns false only for an empty order.
    pub fn navigate(&mut self, movement: SelectionMove, modifiers: SelectionModifiers) -> bool {
        if self.order.is_empty() {
            return false;
        }
        let last = self.order.len() - 1;
        let focused = self
            .focused
            .as_ref()
            .and_then(|id| self.indices.get(id))
            .copied();
        let target = match movement {
            SelectionMove::First => 0,
            SelectionMove::Last => last,
            SelectionMove::Previous => focused.map_or(last, |i| i.saturating_sub(1)),
            SelectionMove::Next => focused.map_or(0, |i| i.saturating_add(1).min(last)),
        };
        if modifiers.extend {
            self.extend_to(target, modifiers.toggle);
        } else if !modifiers.toggle {
            self.selected.clear();
            self.selected.insert(self.order[target].clone());
            self.anchor = Some(self.order[target].clone());
        } else if self.anchor.is_none() {
            self.anchor = Some(self.order[target].clone());
        }
        self.focused = Some(self.order[target].clone());
        true
    }

    fn extend_to(&mut self, target: usize, additive: bool) {
        let anchor = self.anchor.as_ref().or(self.focused.as_ref());
        let start = anchor
            .and_then(|id| self.indices.get(id))
            .copied()
            .unwrap_or(target);
        if self.anchor.is_none() {
            self.anchor = Some(self.order[start].clone());
        }
        if !additive {
            self.selected.clear();
        }
        self.selected.extend(
            self.order[start.min(target)..=start.max(target)]
                .iter()
                .cloned(),
        );
    }
}

impl From<super::InputModifiers> for SelectionModifiers {
    fn from(value: super::InputModifiers) -> Self {
        Self {
            toggle: value.primary(),
            extend: value.shift,
        }
    }
}

/// A proposal from a selectable list, applied to application-owned state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionAction<T> {
    Select {
        id: T,
        modifiers: SelectionModifiers,
    },
    /// `from` is the actual focused row, which can differ from model focus after Tab.
    Navigate {
        from: T,
        movement: SelectionMove,
        modifiers: SelectionModifiers,
    },
    SelectAll,
    Clear,
}
impl<T: Clone + Eq + Hash> SelectionModel<T> {
    /// Apply a list's proposal. Navigation starts at its actual focused row;
    /// unknown/stale identities are ignored without clearing existing selection.
    pub fn apply(&mut self, action: SelectionAction<T>) -> bool {
        match action {
            SelectionAction::Select { id, modifiers } => self.select(&id, modifiers),
            SelectionAction::Navigate {
                from,
                movement,
                modifiers,
            } => {
                if !self.indices.contains_key(&from) {
                    return false;
                }
                self.focused = Some(from);
                self.navigate(movement, modifiers)
            }
            SelectionAction::SelectAll => {
                self.select_all();
                true
            }
            SelectionAction::Clear => {
                self.clear();
                true
            }
        }
    }
}

/// One bounded eager list row with arbitrary Rust UI content.
/// `.key(...)` is required and independent of the application's identity value.
pub struct SelectableRow<C, T> {
    value: T,
    label: String,
    content: super::Element<C>,
    key: Option<String>,
    enabled: bool,
}
pub fn selectable_row<C, T>(
    value: T,
    label: impl Into<String>,
    content: impl Into<super::Element<C>>,
) -> SelectableRow<C, T> {
    SelectableRow {
        value,
        label: label.into(),
        content: content.into(),
        key: None,
        enabled: true,
    }
}
impl<C, T> SelectableRow<C, T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

type SelectionChange<C, T> = std::rc::Rc<std::cell::RefCell<dyn FnMut(&mut C, SelectionAction<T>)>>;
type RowActivate<C, T> = std::rc::Rc<std::cell::RefCell<dyn FnMut(&mut C, T)>>;
pub struct SelectableList<C, T>(pub(super) super::Element<C>, ListOptions<C, T>);
struct ListOptions<C, T> {
    rows: Vec<SelectableRow<C, T>>,
    selected: HashSet<T>,
    focused: Option<T>,
    change: Option<SelectionChange<C, T>>,
    activate: Option<RowActivate<C, T>>,
    plain: bool,
}

/// Rich, accessible, eager multi-selection list, limited to 96 rows (not virtual).
///
/// Reconcile the model to exactly the enabled row IDs in presentation order
/// before composing. This rejects accidentally passing a virtual window as a
/// full scope. The list copies at most 96 IDs, not the model's complete storage.
/// Add `.key(...)`, `.on_selection_change(...)`, and a meaningful accessible label.
/// Apply proposals to your model, then recompose; do not perform I/O in a view.
///
/// ArrowUp/Down and Home/End navigate, Shift extends, primary+arrow only moves
/// focus, primary+A selects all current rows, Space toggles the focused row,
/// Escape clears, and Enter calls the optional separate activation callback.
/// Tab and unrelated application shortcuts are never intercepted.
///
/// ```
/// use volt_ui_kit::ui::prelude::*;
/// struct App { selection: SelectionModel<u8> }
/// let mut app = App { selection: SelectionModel::new() };
/// app.selection.reconcile(&[1, 2])?;
/// let screen: Element<App> = selectable_list([
///     selectable_row(1, "Sources", hstack![text("Sources"), badge("12 files")]).key("sources"),
///     selectable_row(2, "Archives", text("Archives")).key("archives"),
/// ], &app.selection)
/// .key("folders").accessible_label("Folders")
/// .on_selection_change(|app: &mut App, action| { app.selection.apply(action); })
/// .into();
/// screen.build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn selectable_list<C, T: Clone + Eq + Hash>(
    rows: impl IntoIterator<Item = SelectableRow<C, T>>,
    model: &SelectionModel<T>,
) -> SelectableList<C, T> {
    use super::*;
    let rows: Vec<_> = rows.into_iter().take(97).collect();
    let mut element = Element::new(crate::declarative::Kind::Column);
    element.node.width = Some(360.);
    element.node.height = Some(320.);
    element.node.align = Align::Stretch;
    element.node.surface.scroll = Some(Scroll::Vertical);
    element.node.access_role = Some(crate::declarative::AccessRole::ListBox);
    element.node.access_multiselectable = true;
    let enabled_ids: Vec<_> = rows
        .iter()
        .filter(|r| r.enabled)
        .map(|r| &r.value)
        .collect();
    if rows.len() > 96 {
        element.validation_error =
            Some("selectable_list is eager and supports at most 96 rows".into());
    } else if !model.items().iter().eq(enabled_ids.iter().copied()) {
        element.validation_error =
            Some("reconcile selection to exactly the enabled list row IDs in order".into());
    }
    let selected = rows
        .iter()
        .filter(|r| model.contains(&r.value))
        .map(|r| r.value.clone())
        .collect();
    SelectableList(
        element,
        ListOptions {
            rows,
            selected,
            focused: model.focused().cloned(),
            change: None,
            activate: None,
            plain: false,
        },
    )
}
impl<C, T> SelectableList<C, T> {
    pub fn on_selection_change(
        mut self,
        f: impl FnMut(&mut C, SelectionAction<T>) + 'static,
    ) -> Self {
        self.1.change = Some(std::rc::Rc::new(std::cell::RefCell::new(f)));
        self
    }
    /// Separate from selection. Row click never opens a file/folder implicitly.
    pub fn on_activate(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.activate = Some(std::rc::Rc::new(std::cell::RefCell::new(f)));
        self
    }
    /// Flat, desktop-style rows: no card surface or border, only a rounded
    /// highlight on selected rows. Behaviour and accessibility are unchanged.
    /// Off by default, so existing lists keep their card rows.
    pub fn plain(mut self, value: bool) -> Self {
        self.1.plain = value;
        self
    }
    /// Take the remaining height of a column and scroll inside it, instead of
    /// the fixed default height. Use inside a parent with a bounded height.
    pub fn flexible_height(mut self) -> Self {
        self.0.node.height = None;
        self.0.node.flex = 1.;
        self
    }
}
impl<C: 'static, T: Clone + Eq + Hash + 'static> From<SelectableList<C, T>> for super::Element<C> {
    fn from(mut list: SelectableList<C, T>) -> Self {
        use super::*;
        use crate::declarative::{AccessRole, Kind};
        if list.0.auto_id {
            list.0.validation_error = Some("selectable_list requires a stable .key(...)".into());
        }
        let Some(change) = list.1.change else {
            list.0.validation_error = Some("selectable_list requires on_selection_change".into());
            return list.0;
        };
        let mut ids = HashSet::new();
        let total = list.1.rows.iter().filter(|r| r.enabled).count();
        let mut position = 0;
        for row in list.1.rows {
            if !ids.insert(row.value.clone()) {
                list.0.validation_error =
                    Some("selectable_list requires unique identity values".into());
            }
            let Some(key) = row.key else {
                list.0.validation_error = Some("selectable row requires a stable .key(...)".into());
                continue;
            };
            let selected = list.1.selected.contains(&row.value);
            let focused = list.1.focused.as_ref() == Some(&row.value);
            let surface = if list.1.plain {
                Kind::Column
            } else {
                Kind::Panel
            };
            let mut e = container(surface, [row.content])
                .key(key)
                .accessible_label(row.label)
                .focus_when(focused);
            e.0.node.padding = if list.1.plain { 4. } else { 6. };
            if list.1.plain {
                e.0.node.style.radius = Some(crate::declarative::Rounding::Radius(6.));
            }
            e.0.node.enabled = row.enabled;
            e.0.node.access_role = Some(AccessRole::ListBoxOption);
            e.0.node.access_selected = Some(selected);
            e.0.node.access_set_size = Some(total);
            if row.enabled {
                e.0.node.access_position = Some(position);
                position += 1;
            }
            e.0.node.style.background = selected.then(|| "@base-300".into());
            e.0.node.ext.min[1] = Some(28.);
            let value = row.value.clone();
            let clicked = change.clone();
            e = e.on_click_with(move |app, event| {
                (clicked.borrow_mut())(
                    app,
                    SelectionAction::Select {
                        id: value.clone(),
                        modifiers: event.modifiers.into(),
                    },
                );
            });
            let value = row.value;
            let changed = change.clone();
            let activate = list.1.activate.clone();
            e = e.on_input_key(move |app, event| {
                let modifiers = SelectionModifiers::from(event.modifiers);
                let movement = match event.key {
                    InputKey::ArrowUp => Some(SelectionMove::Previous),
                    InputKey::ArrowDown => Some(SelectionMove::Next),
                    InputKey::Home => Some(SelectionMove::First),
                    InputKey::End => Some(SelectionMove::Last),
                    _ => None,
                };
                if let Some(movement) = movement {
                    (changed.borrow_mut())(
                        app,
                        SelectionAction::Navigate {
                            from: value.clone(),
                            movement,
                            modifiers,
                        },
                    );
                    return true;
                }
                let action = match event.key {
                    InputKey::Space if !event.repeat => Some(SelectionAction::Select {
                        id: value.clone(),
                        modifiers: SelectionModifiers {
                            toggle: true,
                            extend: event.modifiers.shift,
                        },
                    }),
                    InputKey::SelectAll => Some(SelectionAction::SelectAll),
                    InputKey::Escape if !event.repeat => Some(SelectionAction::Clear),
                    InputKey::Enter if !event.repeat => {
                        if let Some(activate) = &activate {
                            (activate.borrow_mut())(app, value.clone());
                        }
                        // Consume even without an Open handler: Enter must not
                        // fall through to pointer-like selection on key-up.
                        return true;
                    }
                    // Repeated action keys must not fall through to legacy activation.
                    InputKey::Enter | InputKey::Space | InputKey::Escape => return true,
                    _ => None,
                };
                if let Some(action) = action {
                    (changed.borrow_mut())(app, action);
                    true
                } else {
                    false
                }
            });
            list.0.children.push(e.into());
        }
        list.0
    }
}
