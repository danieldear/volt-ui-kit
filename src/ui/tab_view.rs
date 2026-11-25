//! A phone app's top-level sections, switched from a bar at the bottom.
//!
//! iOS's host draws the system tab bar (Liquid Glass, SF Symbols, badges) and
//! keeps one navigation stack per tab; elsewhere Volt draws Material 3's
//! flexible navigation bar: 64 points, a 24-point icon over its label, and a
//! pill behind the selected icon that slides to the tab you choose.
use super::*;
use crate::declarative::{AccessRole, TabBar, TabBarItem};
use std::{cell::RefCell, rc::Rc};

/// One section of a [`tab_view`]: a value, a short label and an icon.
pub struct TabDestination<T> {
    value: T,
    label: String,
    icon: Icon,
    symbol: Option<String>,
    badge: Option<String>,
}
/// A section of a tab view. Labels are required (icons alone are ambiguous);
/// keep them to one word where possible.
pub fn tab_destination<T>(value: T, label: impl Into<String>, icon: Icon) -> TabDestination<T> {
    TabDestination {
        value,
        label: label.into(),
        icon,
        symbol: None,
        badge: None,
    }
}
impl<T> TabDestination<T> {
    /// The platform's own symbol for this tab: an SF Symbol name on iOS,
    /// such as `"tray.full"`. Prefer the filled variant; iOS does.
    pub fn symbol(mut self, name: impl Into<String>) -> Self {
        self.symbol = Some(name.into());
        self
    }
    /// A count or short text, for something in this section that needs
    /// attention. Reserve badges for what matters.
    pub fn badge(mut self, text: impl Into<String>) -> Self {
        self.badge = Some(text.into());
        self
    }
    /// A small dot instead of a count.
    pub fn dot(mut self) -> Self {
        self.badge = Some(String::new());
        self
    }
}

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}

type Choose<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
/// See [`tab_view`].
pub struct TabView<C, T> {
    key: Option<String>,
    selected: T,
    items: Vec<TabDestination<T>>,
    content: Element<C>,
    on_select: Option<Choose<C, T>>,
    on_reselect: Option<Choose<C, T>>,
}
/// Top-level sections with a bar at the bottom. `content` is the selected
/// section's page, usually its own [`navigation_stack`], so every tab keeps
/// its place when you leave it:
///
/// ```rust,ignore
/// tab_view(app.tab, [
///     tab_destination(Tab::Ideas, "Ideas", Icon::Inbox).symbol("lightbulb.fill"),
///     tab_destination(Tab::Mac, "Mac", Icon::Command).symbol("desktopcomputer").dot(),
///     tab_destination(Tab::Settings, "Settings", Icon::Settings).symbol("gearshape.fill"),
/// ], navigation_stack(app.history(app.tab), |page| pages(app, page)))
/// .key("tabs")
/// .on_select(|a: &mut App, tab| a.tab = tab)
/// .on_reselect(|a: &mut App, tab| a.history_mut(tab).pop_to_root())
/// ```
///
/// Three to five tabs. Don't hide or disable one when its section is empty;
/// say why it's empty on its page instead.
pub fn tab_view<C, T>(
    selected: T,
    items: impl IntoIterator<Item = TabDestination<T>>,
    content: impl Into<Element<C>>,
) -> TabView<C, T> {
    TabView {
        key: None,
        selected,
        items: items.into_iter().collect(),
        content: content.into(),
        on_select: None,
        on_reselect: None,
    }
}
impl<C, T> TabView<C, T> {
    /// Required. Tabs are keyed `{key}-tab-{n}`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// A different tab was chosen.
    pub fn on_select(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.on_select = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// The selected tab was chosen again. Both platforms go back to the
    /// section's first page, or scroll it to the top if already there.
    pub fn on_reselect(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.on_reselect = Some(Rc::new(RefCell::new(f)));
        self
    }
}

/// Material 3 flexible navigation bar measurements, in points.
const BAR_HEIGHT: f32 = 64.;
const INDICATOR: [f32; 2] = [56., 32.];
const ICON: f32 = 24.;

fn badge_mark<C>(text: &str) -> Element<C> {
    if text.is_empty() {
        return vstack(Vec::<Element<C>>::new())
            .width(6.)
            .height(6.)
            .rounded(Rounding::Full)
            .background(ColorToken::Error)
            .offset(10., -8.)
            .into();
    }
    let text: String = text.chars().take(4).collect();
    hstack![text_el(text)]
        .height(16.)
        .padding(4., Edge::Horizontal)
        .rounded(Rounding::Full)
        .background(ColorToken::Error)
        .align(Align::Center)
        .offset(14., -10.)
        .into()
}
fn text_el<C>(value: String) -> Element<C> {
    text(value)
        .font_size(11.)
        .font_weight(600)
        .color(ColorToken::ErrorContent)
        .into()
}

impl<C: 'static, T: Clone + PartialEq + 'static> From<TabView<C, T>> for Element<C> {
    fn from(view: TabView<C, T>) -> Self {
        let Some(key) = view.key.filter(|k| !k.is_empty()) else {
            return invalid("tab_view requires a key");
        };
        if view.items.is_empty() {
            return invalid("tab_view requires at least one tab");
        }
        if view.items.len() > 6 {
            return invalid("tab_view takes at most six tabs; put the rest on a page");
        }
        let Some(selected) = view.items.iter().position(|i| i.value == view.selected) else {
            return invalid("tab_view's selected value is not one of its tabs");
        };
        let mut description = TabBar {
            items: Vec::new(),
            selected,
        };
        let mut tabs: Vec<Element<C>> = Vec::new();
        for (i, item) in view.items.into_iter().enumerate() {
            let id = format!("{key}-tab-{i}");
            let on = i == selected;
            description.items.push(TabBarItem {
                id: id.clone(),
                label: item.label.clone(),
                symbol: item.symbol.clone(),
                icon: format!("{:?}", item.icon),
                badge: item.badge.clone(),
            });
            let colour = if on {
                ColorToken::Primary
            } else {
                ColorToken::Muted
            };
            // The pill lives only in the selected tab; matching it across
            // tabs makes it slide from the old tab to the new one.
            let mut mark: Vec<Element<C>> = Vec::new();
            if on {
                mark.push(
                    vstack(Vec::<Element<C>>::new())
                        .key(format!("{key}-indicator"))
                        .shared_element(format!("{key}-indicator"))
                        .layout_animation(Animation::spring())
                        .width(INDICATOR[0])
                        .height(INDICATOR[1])
                        .rounded(Rounding::Full)
                        .background(ColorToken::Primary)
                        .background_opacity(0.16)
                        .into(),
                );
            }
            mark.push(
                icon(item.icon)
                    .size(ICON)
                    .stroke(if on { 2.25 } else { 1.75 })
                    .color(colour)
                    .into(),
            );
            if let Some(badge) = &item.badge {
                mark.push(badge_mark(badge));
            }
            let (choose, again, value) = (
                view.on_select.clone(),
                view.on_reselect.clone(),
                item.value.clone(),
            );
            let label = match &item.badge {
                Some(b) if !b.is_empty() => format!("{}, {b}", item.label),
                Some(_) => format!("{}, new", item.label),
                None => item.label.clone(),
            };
            let mut tab = vstack![
                zstack(mark)
                    .width(INDICATOR[0])
                    .height(INDICATOR[1])
                    .flow(Flow::Center),
                text(item.label)
                    .font_size(12.)
                    .font_weight(if on { 700 } else { 500 })
                    .color(if on {
                        ColorToken::BaseContent
                    } else {
                        ColorToken::Muted
                    }),
            ]
            .key(id)
            .flex(1.)
            .height(BAR_HEIGHT)
            .gap(4., Axis::Vertical)
            .align(Align::Center)
            .justify(Justify::Center)
            .accessible_label(label)
            .on_click_with(move |c: &mut C, _| {
                let callback = if on { &again } else { &choose };
                if let Some(f) = callback {
                    (f.borrow_mut())(c, value.clone());
                }
            });
            tab.0.node.access_role = Some(AccessRole::Tab);
            tab.0.node.access_selected = Some(on);
            tabs.push(tab.into());
        }
        let mut bar = hstack(tabs)
            .key(format!("{key}-bar"))
            .height(BAR_HEIGHT)
            .background(ColorToken::Base200)
            .border_sides(Sides::TOP)
            .border_color(ColorToken::Border)
            .border_width(0.5);
        bar.0.node.access_role = Some(AccessRole::TabList);
        let mut bar: Element<C> = bar.into();
        bar.node.ext.tab_bar = Some(Box::new(description));
        vstack![
            vstack![view.content].key(format!("{key}-page")).flex(1.),
            bar
        ]
        .key(key)
        .flex(1.)
        .into()
    }
}
