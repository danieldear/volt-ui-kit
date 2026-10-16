//! Independent destination navigation. Application routing remains app-owned.
use super::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NavigationPlacement {
    #[default]
    Sidebar,
    Rail,
    Bottom,
}
pub struct NavigationItem<T> {
    value: T,
    label: String,
    key: Option<String>,
    enabled: bool,
    section: Option<String>,
    badge: Option<String>,
}
pub fn navigation_item<T>(value: T, label: impl Into<String>) -> NavigationItem<T> {
    NavigationItem {
        value,
        label: label.into(),
        key: None,
        enabled: true,
        section: None,
        badge: None,
    }
}
impl<T> NavigationItem<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    /// Group the item under a heading. Consecutive items in the same section
    /// share one heading; headings show in the sidebar placement.
    pub fn section(mut self, title: impl Into<String>) -> Self {
        self.section = Some(title.into());
        self
    }
    /// A small trailing label, such as "New".
    pub fn badge(mut self, label: impl Into<String>) -> Self {
        self.badge = Some(label.into());
        self
    }
}
type Selection<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
pub struct Navigation<C, T>(pub(super) Element<C>, NavigationOptions<C, T>);
struct NavigationOptions<C, T> {
    items: Vec<NavigationItem<T>>,
    /// `None` until set; `Some(None)` highlights nothing.
    selected: Option<Option<T>>,
    callback: Option<Selection<C, T>>,
    placement: NavigationPlacement,
}
pub fn navigation<C, T>(items: impl IntoIterator<Item = NavigationItem<T>>) -> Navigation<C, T> {
    let mut element = Element::new(Kind::Column);
    element.node.gap = 2.;
    element.node.access_role = Some(AccessRole::Group);
    Navigation(
        element,
        NavigationOptions {
            items: items.into_iter().collect(),
            selected: None,
            callback: None,
            placement: NavigationPlacement::Sidebar,
        },
    )
}
impl<C, T> Navigation<C, T> {
    /// The current destination. Without this call the first item is
    /// selected; `None` highlights nothing (e.g. while a filter hides it).
    pub fn selected(mut self, value: impl Into<Option<T>>) -> Self {
        self.1.selected = Some(value.into());
        self
    }
    pub fn placement(mut self, value: NavigationPlacement) -> Self {
        self.1.placement = value;
        self
    }
    pub fn on_select(mut self, callback: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.callback = Some(Rc::new(RefCell::new(callback)));
        self
    }
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<Navigation<C, T>> for Element<C> {
    fn from(mut nav: Navigation<C, T>) -> Self {
        let invalid = nav.1.items.is_empty()
            || nav.1.items.len() >= MAX_NODES
            || nav
                .1
                .items
                .iter()
                .any(|i| i.key.as_deref().is_none_or(str::is_empty))
            || nav.1.items.iter().enumerate().any(|(n, i)| {
                nav.1.items[..n]
                    .iter()
                    .any(|p| p.value == i.value || p.key == i.key)
            })
            || nav
                .1
                .selected
                .as_ref()
                .and_then(Option::as_ref)
                .is_some_and(|v| !nav.1.items.iter().any(|i| &i.value == v));
        if invalid || nav.1.callback.is_none() || nav.0.auto_id {
            nav.0.validation_error = Some("navigation needs a group key, nonempty unique keyed items, valid selection and on_select".into());
            return nav.0;
        }
        let selected = nav
            .1
            .selected
            .unwrap_or_else(|| Some(nav.1.items[0].value.clone()));
        let callback = nav.1.callback.unwrap();
        let placement = nav.1.placement;
        let id = nav.0.node.id.clone();
        nav.0.node.kind = if placement == NavigationPlacement::Bottom {
            Kind::Row
        } else {
            Kind::Column
        };
        // Vertical placements give every row the full width.
        if placement != NavigationPlacement::Bottom && nav.0.node.align == Align::Start {
            nav.0.node.align = Align::Stretch;
        }
        let mut section: Option<String> = None;
        for item in nav.1.items {
            let key = item.key.unwrap();
            if placement == NavigationPlacement::Sidebar
                && item.section.is_some()
                && item.section != section
            {
                let first = nav.0.children.is_empty();
                nav.0.children.push(section_heading(
                    format!("{id}-{key}-section"),
                    item.section.clone().unwrap(),
                    first,
                ));
            }
            section = item.section;
            let active = selected.as_ref() == Some(&item.value);
            let value = item.value;
            let handler = callback.clone();
            let mut row = button(item.label)
                .key(format!("{id}-{key}"))
                .background(ColorToken::BaseContent.alpha(0))
                .label_align(if placement == NavigationPlacement::Sidebar {
                    Align::Start
                } else {
                    Align::Center
                })
                .height(30.)
                .font_size(13.)
                .rounded(6.)
                .enabled(item.enabled)
                .on_click(move |app: &mut C| {
                    if !active {
                        (handler.borrow_mut())(app, value.clone());
                    }
                });
            // The Catalog's sidebar rows: muted until hovered, the current
            // destination semibold on a faint tint.
            row = if active {
                row.background(ColorToken::BaseContent.alpha(7))
                    .font_weight(600)
                    .color(ColorToken::BaseContent)
            } else {
                row.color(ColorToken::Muted).hover_style(|s| {
                    s.background(ColorToken::BaseContent.alpha(5))
                        .color(ColorToken::BaseContent)
                })
            };
            row.0.node.access_selected = Some(active);
            let Some(label) = item.badge else {
                nav.0.children.push(row.into());
                continue;
            };
            let tag: Element<C> = badge(label)
                .key(format!("{id}-{key}-badge"))
                .font_size(10.)
                .rounded(Rounding::Full)
                .background(ColorToken::Primary.alpha(14))
                .color(ColorToken::Primary)
                .into();
            let entry: Element<C> = match placement {
                // Over the row's trailing edge, so labels stay centred; the
                // badge is not interactive, so clicks reach the row.
                NavigationPlacement::Rail => zstack([
                    row.into(),
                    hstack([spacer().into(), tag])
                        .key(format!("{id}-{key}-badge-lane"))
                        .align(Align::Center)
                        .padding(6., Edge::Right)
                        .into(),
                ])
                .key(format!("{id}-{key}-row"))
                .align(Align::Stretch)
                .into(),
                // Beside the row: the sidebar row takes the remaining width.
                NavigationPlacement::Sidebar | NavigationPlacement::Bottom => hstack([
                    if placement == NavigationPlacement::Sidebar {
                        row.flex(1.).into()
                    } else {
                        row.into()
                    },
                    tag,
                ])
                .key(format!("{id}-{key}-row"))
                .gap(6., Axis::Horizontal)
                .align(Align::Center)
                .into(),
            };
            nav.0.children.push(entry);
        }
        nav.0
    }
}
/// A section title above its items, aligned with their labels.
fn section_heading<C>(key: String, title: String, first: bool) -> Element<C> {
    vstack([text(title)
        .font_size(12.)
        .font_weight(600)
        .color(ColorToken::BaseContent)
        .into()])
    .key(key)
    .padding(if first { 4. } else { 16. }, Edge::Top)
    .padding(4., Edge::Bottom)
    .padding(10., Edge::Left)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn nav(placement: NavigationPlacement, selected: Option<Option<u8>>) -> Element<()> {
        let mut n = navigation([
            navigation_item(0u8, "Introduction")
                .key("intro")
                .section("Getting started"),
            navigation_item(1, "Theming")
                .key("theming")
                .section("Getting started")
                .badge("New"),
            navigation_item(2, "Button")
                .key("button")
                .section("Components"),
        ])
        .key("side")
        .placement(placement)
        .on_select(|_: &mut (), _| {});
        if let Some(selected) = selected {
            n = n.selected(selected);
        }
        n.into()
    }
    fn ids(e: &Element<()>) -> Vec<&str> {
        e.children.iter().map(|c| c.node.id.as_str()).collect()
    }
    #[test]
    fn sidebar_sections_get_one_heading_each() {
        let e = nav(NavigationPlacement::Sidebar, None);
        assert_eq!(
            ids(&e),
            [
                "side-intro-section",
                "side-intro",
                "side-theming-row",
                "side-button-section",
                "side-button"
            ]
        );
        assert_eq!(e.node.align, Align::Stretch);
        assert!(e.build().is_ok());
    }
    #[test]
    fn headings_are_sidebar_only() {
        for placement in [NavigationPlacement::Rail, NavigationPlacement::Bottom] {
            let e = nav(placement, None);
            assert!(!ids(&e).iter().any(|id| id.ends_with("-section")));
        }
    }
    #[test]
    fn badges_sit_beside_their_row_with_stable_keys() {
        let e = nav(NavigationPlacement::Sidebar, None);
        let row = &e.children[2];
        assert_eq!(row.children[0].node.id, "side-theming");
        assert!(row.children[0].node.flex > 0., "the sidebar row fills");
        assert_eq!(row.children[1].node.id, "side-theming-badge");
        // Horizontal rows keep their natural width.
        let bottom = nav(NavigationPlacement::Bottom, None);
        assert_eq!(bottom.children[1].children[0].node.flex, 0.);
    }
    #[test]
    fn rail_badges_overlay_the_row_so_labels_stay_centred() {
        let e = nav(NavigationPlacement::Rail, None);
        let row = &e.children[1];
        assert_eq!(row.node.kind, Kind::ZStack);
        assert_eq!(row.children[0].node.id, "side-theming");
        assert_eq!(row.children[1].children[1].node.id, "side-theming-badge");
        assert!(e.build().is_ok());
    }
    #[test]
    fn rows_follow_the_catalog_sidebar() {
        let e = nav(NavigationPlacement::Sidebar, Some(Some(2)));
        let [on, off] = [&e.children[4].node, &e.children[1].node];
        assert_eq!(on.height, Some(30.));
        assert_eq!(on.style.font_weight, Some(600));
        assert_eq!(on.style.background.as_deref(), Some("@base-content/7"));
        assert_eq!(on.access_selected, Some(true));
        assert_eq!(off.style.color.as_deref(), Some("@muted"));
        assert_eq!(off.access_selected, Some(false));
    }
    #[test]
    fn selection_defaults_to_the_first_item_and_none_highlights_nothing() {
        let first = nav(NavigationPlacement::Bottom, None);
        assert_eq!(first.children[0].node.access_selected, Some(true));
        let none = nav(NavigationPlacement::Bottom, Some(None));
        assert!(none
            .children
            .iter()
            .all(|c| c.node.access_selected != Some(true)
                && c.children
                    .iter()
                    .all(|g| g.node.access_selected != Some(true))));
        assert!(none.build().is_ok());
        // A value that is not an item is still rejected.
        assert!(nav(NavigationPlacement::Sidebar, Some(Some(9)))
            .build()
            .is_err());
    }
}
