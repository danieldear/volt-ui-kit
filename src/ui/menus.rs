//! Typed, application-owned in-window menus; not operating-system menus.
use super::*;
use crate::declarative::document::OverlayKind;
use std::{cell::RefCell, rc::Rc};

/// One action row or separator in a declarative menu.
pub struct MenuItem<T> {
    kind: ItemKind<T>,
    key: Option<String>,
}
enum ItemKind<T> {
    Action {
        value: T,
        label: String,
        enabled: bool,
        danger: bool,
    },
    Separator,
}
pub fn menu_item<T>(value: T, label: impl Into<String>) -> MenuItem<T> {
    MenuItem {
        kind: ItemKind::Action {
            value,
            label: label.into(),
            enabled: true,
            danger: false,
        },
        key: None,
    }
}
pub fn menu_separator<T>() -> MenuItem<T> {
    MenuItem {
        kind: ItemKind::Separator,
        key: None,
    }
}
impl<T> MenuItem<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        if let ItemKind::Action { enabled: value, .. } = &mut self.kind {
            *value = enabled;
        }
        self
    }
    pub fn danger(mut self, danger: bool) -> Self {
        if let ItemKind::Action { danger: value, .. } = &mut self.kind {
            *value = danger;
        }
        self
    }
}
type Action<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
type Open<C> = Rc<RefCell<dyn FnMut(&mut C, bool)>>;
struct MenuOptions<C, T> {
    items: Vec<MenuItem<T>>,
    open: bool,
    enabled: bool,
    action: Option<Action<C, T>>,
    change: Option<Open<C>>,
}
impl<C, T> MenuOptions<C, T> {
    fn new(items: impl IntoIterator<Item = MenuItem<T>>) -> Self {
        Self {
            items: items.into_iter().take(129).collect(),
            open: false,
            enabled: true,
            action: None,
            change: None,
        }
    }
}
/// Controlled dropdown action menu. A stable key and both callbacks are required.
pub struct Menu<C, T>(pub(super) Element<C>, String, MenuOptions<C, T>);
pub fn menu<C, T>(
    label: impl Into<String>,
    items: impl IntoIterator<Item = MenuItem<T>>,
) -> Menu<C, T> {
    let mut e = Element::new(Kind::Column);
    e.node.width = Some(208.);
    Menu(e, label.into(), MenuOptions::new(items))
}
type ContextOpen<C> = Box<dyn FnMut(&mut C, [f32; 2])>;
/// A right-click menu attached to a target subtree. Positions are logical pixels.
pub struct ContextMenu<C, T>(
    pub(super) Element<C>,
    Element<C>,
    MenuOptions<C, T>,
    [f32; 2],
    Option<ContextOpen<C>>,
);
pub fn context_menu<C, T>(
    target: impl Into<Element<C>>,
    items: impl IntoIterator<Item = MenuItem<T>>,
) -> ContextMenu<C, T> {
    ContextMenu(
        Element::new(Kind::ZStack),
        target.into(),
        MenuOptions::new(items),
        [0.; 2],
        None,
    )
}
macro_rules! menu_options {
    ($kind:ident,$field:tt) => {
        impl<C, T> $kind<C, T> {
            pub fn open(mut self, open: bool) -> Self {
                self.$field.open = open;
                self
            }
            pub fn enabled(mut self, enabled: bool) -> Self {
                self.$field.enabled = enabled;
                self
            }
            pub fn on_action(mut self, action: impl FnMut(&mut C, T) + 'static) -> Self {
                self.$field.action = Some(Rc::new(RefCell::new(action)));
                self
            }
            pub fn on_open_change(mut self, change: impl FnMut(&mut C, bool) + 'static) -> Self {
                self.$field.change = Some(Rc::new(RefCell::new(change)));
                self
            }
        }
    };
}
menu_options!(Menu, 2);
menu_options!(ContextMenu, 2);
impl<C, T> ContextMenu<C, T> {
    pub fn position(mut self, point: [f32; 2]) -> Self {
        self.3 = point;
        self
    }
    /// Native hosts pass a right-press position; update position/open app state.
    pub fn on_context_open(mut self, callback: impl FnMut(&mut C, [f32; 2]) + 'static) -> Self {
        self.4 = Some(Box::new(callback));
        self
    }
}
fn error<C, T>(root: &Element<C>, options: &MenuOptions<C, T>) -> Option<String> {
    if root.auto_id {
        return Some("menu requires a stable .key(...)".into());
    }
    if options.items.is_empty()
        || options.items.len() > 128
        || !options
            .items
            .iter()
            .any(|i| matches!(i.kind, ItemKind::Action { .. }))
    {
        return Some("menu requires 1..=128 rows including an action".into());
    }
    if options.action.is_none() || options.change.is_none() {
        return Some("menu requires on_action and on_open_change".into());
    }
    None
}
fn popup<C: 'static, T: Clone + 'static>(
    items: Vec<MenuItem<T>>,
    width: f32,
    key: String,
    anchor: String,
    action: Action<C, T>,
    change: Open<C>,
) -> Element<C> {
    let mut card = Element::new(Kind::Panel);
    card.node.id = format!("{key}-content");
    card.auto_id = false;
    card.node.width = Some(width);
    card.node.padding = 6.;
    card.node.gap = 2.;
    card.node.surface.scroll = Some(Scroll::Vertical);
    card.node.style.radius = Some(Rounding::Radius(10.));
    card.node.style.background = Some(ColorToken::Base200.into());
    card.node.style.border_color = Some(ColorToken::Border.into());
    card.node.style.border_width = Some(1.);
    let mut height = 12.;
    let rows = items.len();
    for item in items {
        let mut child: Element<C> = match item.kind {
            ItemKind::Separator => {
                height += 9.;
                separator().height(9.).into()
            }
            ItemKind::Action {
                value,
                label,
                enabled,
                danger,
            } => {
                height += 34.;
                let action = action.clone();
                let change = change.clone();
                let mut b = button(label)
                    .height(34.)
                    .width((width - 12.).max(0.))
                    .rounded(6)
                    .background("#00000000")
                    .border_width(0.)
                    .enabled(enabled)
                    .hover_style(|s| s.background(ColorToken::Base300))
                    .on_click(move |app: &mut C| {
                        (action.borrow_mut())(app, value.clone());
                        (change.borrow_mut())(app, false);
                    });
                if danger {
                    b = b.color(ColorToken::Error);
                }
                b.into()
            }
        };
        if let Some(key) = item.key {
            child.node.id = key;
            child.auto_id = false;
        }
        card.children.push(child);
    }
    card.node.height = Some((height + 2. * rows.saturating_sub(1) as f32).min(320.));
    let close = change.clone();
    let mut root = super::dialogs::overlay_scope(
        card,
        key,
        OverlayKind::Popup,
        Box::new(move |app: &mut C| (close.borrow_mut())(app, false)),
    );
    root.node.overlay.as_mut().unwrap().anchor = Some(anchor);
    root
}
impl<C: 'static, T: Clone + 'static> From<Menu<C, T>> for Element<C> {
    fn from(mut menu: Menu<C, T>) -> Self {
        if let Some(error) = error(&menu.0, &menu.2) {
            menu.0.validation_error = Some(error);
            return menu.0;
        }
        let width = menu.0.node.width.unwrap_or(208.);
        let trigger_key = format!("{}-trigger", menu.0.node.id);
        let change = menu.2.change.unwrap();
        let open = menu.2.open;
        let callback = change.clone();
        let mut trigger = button(format!("{}  {}", menu.1, if open { "▴" } else { "▾" }))
            .key(trigger_key.clone())
            .height(menu.0.node.height.unwrap_or(36.))
            .enabled(menu.2.enabled)
            .on_click(move |app: &mut C| (callback.borrow_mut())(app, !open));
        trigger.0.node.width = menu.0.node.width;
        trigger.0.node.style = menu.0.node.style.clone();
        menu.0.children.push(trigger.into());
        if open && menu.2.enabled {
            let key = format!("{}-popup", menu.0.node.id);
            menu.0.children.push(popup(
                menu.2.items,
                width,
                key,
                trigger_key,
                menu.2.action.unwrap(),
                change,
            ));
        }
        menu.0
    }
}
impl<C: 'static, T: Clone + 'static> From<ContextMenu<C, T>> for Element<C> {
    fn from(mut menu: ContextMenu<C, T>) -> Self {
        if let Some(error) = error(&menu.0, &menu.2) {
            menu.0.validation_error = Some(error);
            return menu.0;
        }
        let Some(callback) = menu.4 else {
            menu.0.validation_error = Some("context_menu requires on_context_open".into());
            return menu.0;
        };
        if !menu
            .3
            .iter()
            .all(|v| v.is_finite() && (0. ..=65_536.).contains(v))
        {
            menu.0.validation_error =
                Some("context_menu position must be finite nonnegative logical pixels".into());
            return menu.0;
        }
        menu.0.node.enabled = menu.2.enabled;
        menu.0.callback = Some(Callback::Context(callback));
        if menu.1.auto_id {
            menu.1.node.id = format!("{}-target", menu.0.node.id);
            menu.1.auto_id = false;
        }
        let anchor = menu.1.node.id.clone();
        menu.0.children.push(menu.1);
        if menu.2.open && menu.2.enabled {
            let key = format!("{}-popup", menu.0.node.id);
            let mut popup = popup(
                menu.2.items,
                menu.0.node.width.unwrap_or(208.),
                key,
                anchor,
                menu.2.action.unwrap(),
                menu.2.change.unwrap(),
            );
            popup.node.overlay.as_mut().unwrap().position = Some(menu.3);
            menu.0.children.push(popup);
        }
        menu.0
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;
    #[test]
    fn menu_visual_modifiers_style_the_actual_trigger() {
        let ui: Element<()> = menu("Actions", [menu_item(1usize, "Save")])
            .key("actions")
            .fill_width()
            .height(40.)
            .rounded(10.)
            .font_size(13.)
            .background(ColorToken::Base300)
            .on_action(|_, _| {})
            .on_open_change(|_, _| {})
            .into();
        assert!(ui.node.width.is_none());
        let trigger = &ui.children[0].node;
        assert!(trigger.width.is_none());
        assert_eq!(trigger.height, Some(40.));
        assert_eq!(trigger.style.radius, Some(Rounding::Radius(10.)));
        assert_eq!(trigger.style.font_size, Some(13.));
        assert_eq!(trigger.style.background.as_deref(), Some("@base-300"));
        ui.build().unwrap();
    }
}
