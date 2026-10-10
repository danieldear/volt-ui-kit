//! Typed scalar controls and a controlled, portal-mounted dropdown.
use super::*;
use crate::declarative::document::{OverlayKind, SliderProps};
use std::{cell::RefCell, ops::RangeInclusive, rc::Rc};

/// Controlled horizontal slider. Application state remains authoritative.
pub struct Slider<C>(pub(super) Element<C>);
pub fn slider<C>(value: f32) -> Slider<C> {
    let mut element = Element::new(Kind::Panel);
    element.node.width = Some(180.);
    element.node.height = Some(32.);
    element.node.slider = Some(SliderProps {
        value,
        min: 0.,
        max: 1.,
        step: 0.01,
        on_change: None,
    });
    Slider(element)
}
impl<C> Slider<C> {
    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        let props = self.0.node.slider.as_mut().expect("slider metadata");
        props.min = *range.start();
        props.max = *range.end();
        self
    }
    pub fn step(mut self, value: f32) -> Self {
        self.0.node.slider.as_mut().expect("slider metadata").step = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.0.node.enabled = value;
        self
    }
    pub fn on_change(mut self, callback: impl FnMut(&mut C, f32) + 'static) -> Self {
        self.0.callback = Some(Callback::Number(Box::new(callback)));
        self
    }
}
impl<C> From<Slider<C>> for Element<C> {
    fn from(value: Slider<C>) -> Self {
        value.0
    }
}

/// A typed option. Keys are stable across reorderings; disabled options cannot activate.
pub struct SelectItem<T> {
    value: T,
    label: String,
    key: Option<String>,
    enabled: bool,
}
pub fn select_item<T>(value: T, label: impl Into<String>) -> SelectItem<T> {
    SelectItem {
        value,
        label: label.into(),
        key: None,
        enabled: true,
    }
}
impl<T> SelectItem<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}
type Selected<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
type Opened<C> = Rc<RefCell<dyn FnMut(&mut C, bool)>>;
/// Controlled dropdown. A stable `.key(...)` is required for popup/focus identity.
/// Recompose after callbacks: `.open(app.open)` mirrors the application-owned open flag.
pub struct Select<C, T>(pub(super) Element<C>, SelectOptions<C, T>);
struct SelectOptions<C, T> {
    items: Vec<SelectItem<T>>,
    selected: Option<T>,
    open: bool,
    enabled: bool,
    on_select: Option<Selected<C, T>>,
    on_open_change: Option<Opened<C>>,
}
pub fn select<C, T>(items: impl IntoIterator<Item = SelectItem<T>>) -> Select<C, T> {
    let mut element = Element::new(Kind::Column);
    element.node.width = Some(220.);
    Select(
        element,
        SelectOptions {
            items: items.into_iter().collect(),
            selected: None,
            open: false,
            enabled: true,
            on_select: None,
            on_open_change: None,
        },
    )
}
impl<C, T> Select<C, T> {
    pub fn selected(mut self, value: T) -> Self {
        self.1.selected = Some(value);
        self
    }
    pub fn open(mut self, value: bool) -> Self {
        self.1.open = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.1.enabled = value;
        self
    }
    pub fn on_select(mut self, callback: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.on_select = Some(Rc::new(RefCell::new(callback)));
        self
    }
    pub fn on_open_change(mut self, callback: impl FnMut(&mut C, bool) + 'static) -> Self {
        self.1.on_open_change = Some(Rc::new(RefCell::new(callback)));
        self
    }
}
const MAX_OPTIONS: usize = 128;
impl<C: 'static, T: Clone + PartialEq + 'static> From<Select<C, T>> for Element<C> {
    fn from(mut select: Select<C, T>) -> Self {
        let error = if select.0.auto_id {
            Some("select requires a stable .key(...)".into())
        } else if select.1.items.is_empty() || select.1.items.len() > MAX_OPTIONS {
            Some("select requires 1..=128 options".into())
        } else if select.1.items.iter().enumerate().any(|(i, item)| {
            select.1.items[..i]
                .iter()
                .any(|other| other.value == item.value)
        }) {
            Some("duplicate select value".into())
        } else {
            None
        };
        if let Some(error) = error {
            select.0.validation_error = Some(error);
            return select.0;
        }
        let selected = select
            .1
            .selected
            .or_else(|| select.1.items.first().map(|item| item.value.clone()))
            .expect("nonempty options");
        let Some(index) = select
            .1
            .items
            .iter()
            .position(|item| item.value == selected)
        else {
            select.0.validation_error = Some("selected value is not a select option".into());
            return select.0;
        };
        let Some(on_select) = select.1.on_select else {
            select.0.validation_error = Some("select requires on_select".into());
            return select.0;
        };
        let Some(on_open) = select.1.on_open_change else {
            select.0.validation_error = Some("select requires on_open_change".into());
            return select.0;
        };
        let trigger_key = format!("{}-trigger", select.0.node.id);
        let popup_key = format!("{}-popup", select.0.node.id);
        let open = select.1.open;
        let callback = on_open.clone();
        let mut trigger: Element<C> = button(select.1.items[index].label.clone())
            .disclosure(open)
            .key(trigger_key.clone())
            .enabled(select.1.enabled)
            .height(select.0.node.height.unwrap_or(36.))
            .width(select.0.node.width.unwrap_or(220.))
            .on_click(move |app: &mut C| (callback.borrow_mut())(app, !open))
            .into();
        trigger.node.accessible_label = select.0.node.accessible_label.take();
        // Public surface modifiers style the actual interactive trigger. The
        // outer layout node keeps typography and clipping, but must not paint
        // a second translucent background or border underneath that trigger.
        let mut style = select.0.node.style.clone();
        style.radius = style.radius.or(trigger.node.style.radius);
        trigger.node.style = style;
        trigger.node.effects.states = std::mem::take(&mut select.0.node.effects.states);
        trigger.node.effects.animations = std::mem::take(&mut select.0.node.effects.animations);
        select.0.node.style.background = None;
        select.0.node.style.background_opacity = None;
        select.0.node.style.border_color = None;
        select.0.node.style.border_width = None;
        select.0.children.push(trigger);
        // Closed/disabled popups do not mount options or callbacks at all.
        if !open || !select.1.enabled {
            return select.0;
        }
        let mut items = Element::new(Kind::Panel);
        items.node.access_role = Some(AccessRole::ListBox);
        items.node.access_set_size = Some(select.1.items.len());
        items.node.padding = 8.;
        items.node.style.radius = Some(Rounding::Radius(8.));
        items.node.style.background = Some(ColorToken::Base200.into());
        items.node.style.border_color = Some(ColorToken::Border.into());
        items.node.style.border_width = Some(1.);
        items.node.width = select.0.node.width;
        items.node.gap = 2.;
        items.node.surface.scroll = Some(Scroll::Vertical);
        items.node.height = Some((select.1.items.len() as f32 * 36. + 16.).min(320.));
        for (position, item) in select.1.items.into_iter().enumerate() {
            let active = item.value == selected;
            let on_select = on_select.clone();
            let on_open = on_open.clone();
            let value = item.value;
            let mut option = button(item.label)
                .height(34.)
                .width(select.0.node.width.unwrap_or(220.))
                .primary(active)
                .enabled(item.enabled)
                .on_click(move |app: &mut C| {
                    if !active {
                        (on_select.borrow_mut())(app, value.clone());
                    }
                    (on_open.borrow_mut())(app, false);
                });
            if active {
                option = option.color(ColorToken::PrimaryContent);
            }
            option.0.node.checked = active;
            option.0.node.access_role = Some(AccessRole::ListBoxOption);
            option.0.node.access_selected = Some(active);
            option.0.node.access_position = Some(position);
            if let Some(key) = item.key {
                option = option.key(key);
            }
            items.children.push(option.into());
        }
        let close = on_open.clone();
        let mut popup = super::dialogs::overlay_scope(
            items,
            popup_key,
            OverlayKind::Popup,
            Box::new(move |app: &mut C| (close.borrow_mut())(app, false)),
        );
        popup.node.overlay.as_mut().expect("popup metadata").anchor = Some(trigger_key);
        select.0.children.push(popup);
        select.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn select_surface_and_interaction_modifiers_style_its_trigger_once() {
        let element: Element<()> = select([select_item(0, "Zero")])
            .key("styled")
            .height(44.)
            .rounded(12.)
            .background("#10203080")
            .background_opacity(0.5)
            .border_color("#ffffff")
            .border_width(1.)
            .hover_style(|style| style.background("#405060"))
            .on_select(|_, _| {})
            .on_open_change(|_, _| {})
            .into();
        let trigger = &element.children[0];
        assert_eq!(trigger.node.height, Some(44.));
        assert_eq!(trigger.node.style.radius, Some(Rounding::Radius(12.)));
        assert_eq!(trigger.node.style.background.as_deref(), Some("#10203080"));
        assert_eq!(trigger.node.style.background_opacity, Some(0.5));
        assert_eq!(
            trigger.node.effects.states[0].background.as_deref(),
            Some("#405060")
        );
        assert_eq!(element.node.style.background, None);
        assert_eq!(element.node.style.border_color, None);
        assert_eq!(element.node.style.border_width, None);
        assert_eq!(element.node.effects.states[0].background, None);
        element.build().unwrap();
    }
}
