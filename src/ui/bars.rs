//! Rust-only composites lowered to existing layout/button primitives.
use super::*;
use std::{cell::RefCell, rc::Rc};

/// A horizontal action group. Joined groups suppress gaps and share outer corners.
pub struct ButtonBar<C>(pub(super) Element<C>, bool);
/// An application-owned toolbar, not an OS-native menu or titlebar.
pub struct Toolbar<C>(pub(super) Element<C>);
pub fn button_bar<C>(children: impl IntoIterator<Item = Element<C>>) -> ButtonBar<C> {
    let mut e = container(Kind::Row, children).0;
    e.node.gap = 8.;
    e.node.style.radius = Some(Rounding::Radius(6.));
    ButtonBar(e, false)
}
pub fn toolbar<C>(children: impl IntoIterator<Item = Element<C>>) -> Toolbar<C> {
    let mut e = container(Kind::Row, children).0;
    e.node.gap = 8.;
    e.node.align = Align::Center;
    Toolbar(e)
}
/// Square the inner corners of a group's children and separate them with a
/// 1px seam through which the surface behind shows, so the segments read as
/// one divided control. (A group background would fill the full row width
/// when a column stretches the group.)
fn join<C>(e: &mut Element<C>) {
    e.node.gap = 1.;
    e.node.gap_horizontal = None;
    e.node.responsive.gap = [None, None];
    let radius = e.node.style.radius.unwrap_or(Rounding::Radius(6.));
    let corners = e.node.style.corners.unwrap_or([radius; 4]);
    let last = e.children.len().saturating_sub(1);
    for (i, child) in e.children.iter_mut().enumerate() {
        child.node.style.radius = None;
        child.node.style.corners = Some([
            if i == 0 {
                corners[0]
            } else {
                Rounding::Radius(0.)
            },
            if i == last {
                corners[1]
            } else {
                Rounding::Radius(0.)
            },
            if i == last {
                corners[2]
            } else {
                Rounding::Radius(0.)
            },
            if i == 0 {
                corners[3]
            } else {
                Rounding::Radius(0.)
            },
        ]);
    }
}
impl<C> ButtonBar<C> {
    pub fn joined(mut self, joined: bool) -> Self {
        self.1 = joined;
        self
    }
}
impl<C> From<ButtonBar<C>> for Element<C> {
    fn from(mut bar: ButtonBar<C>) -> Self {
        if bar.1 {
            join(&mut bar.0);
        }
        bar.0
    }
}
impl<C> From<Toolbar<C>> for Element<C> {
    fn from(bar: Toolbar<C>) -> Self {
        bar.0
    }
}

/// A typed selection item. Explicit keys are recommended for dynamic/reordered items.
pub struct TabItem<T> {
    value: T,
    label: String,
    key: Option<String>,
    enabled: bool,
}
pub fn tab_item<T>(value: T, label: impl Into<String>) -> TabItem<T> {
    TabItem {
        value,
        label: label.into(),
        key: None,
        enabled: true,
    }
}
impl<T> TabItem<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
type Select<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
/// How a tab strip draws its tabs. Segmented controls keep their joined look
/// whatever the variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabVariant {
    /// Text tabs over a hairline; the selected tab is underlined in the
    /// primary colour.
    #[default]
    Line,
    /// A quiet track; the selected tab is a raised chip.
    Contained,
}
/// Discrete typed selection; Left/Right and Home/End navigate the group in native hosts.
pub struct SelectionControl<C, T>(pub(super) Element<C>, SelectionOptions<C, T>);
struct SelectionOptions<C, T> {
    items: Vec<TabItem<T>>,
    selected: Option<T>,
    callback: Option<Select<C, T>>,
    enabled: bool,
    joined: bool,
    variant: TabVariant,
}
pub fn tab_bar<C, T>(items: impl IntoIterator<Item = TabItem<T>>) -> SelectionControl<C, T> {
    let mut e = Element::new(Kind::Row);
    e.node.gap = 4.;
    e.node.selection_group = true;
    e.node.access_role = Some(AccessRole::TabList);
    SelectionControl(
        e,
        SelectionOptions {
            items: items.into_iter().collect(),
            selected: None,
            callback: None,
            enabled: true,
            joined: false,
            variant: TabVariant::Line,
        },
    )
}
pub fn segmented_control<C, T>(
    items: impl IntoIterator<Item = (T, impl Into<String>)>,
) -> SelectionControl<C, T> {
    let mut bar = tab_bar(
        items
            .into_iter()
            .map(|(value, label)| tab_item(value, label)),
    );
    bar.1.joined = true;
    bar.0.node.access_role = Some(AccessRole::RadioGroup);
    bar
}
impl<C, T> SelectionControl<C, T> {
    pub fn selected(mut self, selected: T) -> Self {
        self.1.selected = Some(selected);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.1.enabled = enabled;
        self
    }
    /// Join the options into one divided control, as `segmented_control` does.
    pub fn joined(mut self, joined: bool) -> Self {
        self.1.joined = joined;
        self
    }
    /// Line (default) or contained tabs. Ignored by joined controls.
    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.1.variant = variant;
        self
    }
    pub fn on_select(mut self, callback: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.callback = Some(Rc::new(RefCell::new(callback)));
        self
    }
}
fn selection_error<T: PartialEq>(items: &[TabItem<T>], selected: &T) -> Option<String> {
    // Reject oversized candidates before the quadratic PartialEq duplicate scan.
    if items.len() >= MAX_NODES {
        return Some("selection item count exceeds the document node limit".into());
    }
    if items.is_empty() {
        return Some("selection requires at least one item".into());
    }
    if !items.iter().any(|item| item.value == *selected) {
        return Some("selected value is not an item".into());
    }
    if items
        .iter()
        .enumerate()
        .any(|(i, item)| items[..i].iter().any(|other| other.value == item.value))
    {
        return Some("duplicate selection value".into());
    }
    None
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<SelectionControl<C, T>> for Element<C> {
    fn from(mut bar: SelectionControl<C, T>) -> Self {
        let selected = bar
            .1
            .selected
            .or_else(|| bar.1.items.first().map(|item| item.value.clone()));
        let Some(selected) = selected else {
            bar.0.validation_error = Some("selection requires at least one item".into());
            return bar.0;
        };
        if let Some(error) = selection_error(&bar.1.items, &selected) {
            bar.0.validation_error = Some(error);
            return bar.0;
        }
        let Some(callback) = bar.1.callback else {
            bar.0.validation_error = Some("selection requires on_select".into());
            return bar.0;
        };
        let segmented = bar.1.joined;
        if !segmented {
            style_strip(&mut bar.0, bar.1.variant);
        }
        for item in bar.1.items {
            let active = item.value == selected;
            let value = item.value;
            let handler = callback.clone();
            let mut child = button(item.label)
                .enabled(bar.1.enabled && item.enabled)
                .on_click(move |context: &mut C| {
                    if !active {
                        (handler.borrow_mut())(context, value.clone());
                    }
                });
            child = if !segmented {
                style_tab(child, bar.1.variant, active)
            } else if active {
                // The selected surface uses Primary; a parent's general text
                // color must not replace its paired foreground (e.g. white on
                // pale Iris).
                child.primary(true).color(ColorToken::PrimaryContent)
            } else {
                child
            };
            child.0.node.checked = active;
            child.0.node.access_selected = Some(active);
            child.0.node.access_role =
                Some(if bar.0.node.access_role == Some(AccessRole::RadioGroup) {
                    AccessRole::RadioButton
                } else {
                    AccessRole::Tab
                });
            if let Some(key) = item.key {
                child = child.key(key);
            }
            bar.0.children.push(child.into());
        }
        if segmented {
            join(&mut bar.0);
        }
        bar.0
    }
}
/// The strip behind the tabs, where the app has not styled it: a hairline
/// under line tabs, a rounded track around contained ones.
fn style_strip<C>(strip: &mut Element<C>, variant: TabVariant) {
    let node = &mut strip.node;
    match variant {
        TabVariant::Line => {
            if node.style.border_color.is_none() && node.style.border_width.is_none() {
                node.ext.border_sides = Some(Sides::BOTTOM);
                node.style.border_width = Some(1.);
                node.style.border_color = Some(ColorToken::Border.into());
            }
        }
        TabVariant::Contained => {
            node.gap = 2.;
            if node.style.background.is_none() {
                node.style.background = Some(ColorToken::BaseContent.alpha(6));
            }
            if node.style.radius.is_none() && node.style.corners.is_none() {
                node.style.radius = Some(Rounding::Radius(9.));
            }
            if node.padding == 0. && node.responsive.padding.iter().all(Option::is_none) {
                node.padding = 3.;
            }
            if node.height.is_none() {
                node.height = Some(36.);
            }
            // Chips fill the track's height.
            if node.align == Align::Start {
                node.align = Align::Stretch;
            }
        }
    }
}
/// A tab drawn as text: muted until selected, then underlined (line) or
/// raised on a chip (contained). The rounding only shapes the focus ring;
/// the underline stays straight.
fn style_tab<C>(tab: Button<C>, variant: TabVariant, active: bool) -> Button<C> {
    let tab = tab
        .background(ColorToken::BaseContent.alpha(0))
        .rounded(6.)
        .font_weight(if active { 600 } else { 500 })
        .color(if active {
            ColorToken::BaseContent
        } else {
            ColorToken::Muted
        })
        .hover_style(|s| s.color(ColorToken::BaseContent));
    match (variant, active) {
        (TabVariant::Line, true) => tab
            .border_sides(Sides::BOTTOM)
            .border_width(2.)
            .border_color(ColorToken::Primary),
        (TabVariant::Line, false) => tab,
        // Chips share a stretched track equally; a natural-width track
        // (e.g. in a row) has no spare width, so they keep their size.
        (TabVariant::Contained, true) => tab
            .flex(1.)
            .background(ColorToken::Base100)
            .shadow(Shadow::new().color("#0000001A").blur(4.).offset(0., 1.)),
        (TabVariant::Contained, false) => tab.flex(1.),
    }
}

/// Placement of the tab strip relative to its active content, not mobile navigation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabPlacement {
    #[default]
    Top,
    Bottom,
}
pub struct Tab<C, T> {
    item: TabItem<T>,
    content: Element<C>,
}
pub fn tab<C, T>(value: T, label: impl Into<String>, content: impl Into<Element<C>>) -> Tab<C, T> {
    Tab {
        item: tab_item(value, label),
        content: content.into(),
    }
}
impl<C, T> Tab<C, T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.item = self.item.key(key);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.item = self.item.enabled(enabled);
        self
    }
}
pub struct Tabs<C, T>(pub(super) Element<C>, TabsOptions<C, T>);
struct TabsOptions<C, T> {
    items: Vec<Tab<C, T>>,
    selected: Option<T>,
    callback: Option<Select<C, T>>,
    placement: TabPlacement,
    variant: TabVariant,
}
pub fn tabs<C, T>(items: impl IntoIterator<Item = Tab<C, T>>) -> Tabs<C, T> {
    let mut e = Element::new(Kind::Column);
    e.node.gap = 12.;
    Tabs(
        e,
        TabsOptions {
            items: items.into_iter().collect(),
            selected: None,
            callback: None,
            placement: TabPlacement::Top,
            variant: TabVariant::Line,
        },
    )
}
impl<C, T> Tabs<C, T> {
    pub fn selected(mut self, value: T) -> Self {
        self.1.selected = Some(value);
        self
    }
    pub fn placement(mut self, value: TabPlacement) -> Self {
        self.1.placement = value;
        self
    }
    /// Line (default) or contained tabs.
    pub fn variant(mut self, value: TabVariant) -> Self {
        self.1.variant = value;
        self
    }
    pub fn on_select(mut self, callback: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.callback = Some(Rc::new(RefCell::new(callback)));
        self
    }
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<Tabs<C, T>> for Element<C> {
    fn from(mut tabs: Tabs<C, T>) -> Self {
        if tabs.1.items.len() >= MAX_NODES {
            tabs.0.validation_error = Some("tab count exceeds the document node limit".into());
            return tabs.0;
        }
        let selected = tabs
            .1
            .selected
            .or_else(|| tabs.1.items.first().map(|tab| tab.item.value.clone()));
        let Some(selected) = selected else {
            tabs.0.validation_error = Some("tabs require at least one tab".into());
            return tabs.0;
        };
        let mut items = Vec::new();
        let mut content = None;
        for tab in tabs.1.items {
            if tab.item.value == selected && content.is_none() {
                content = Some(tab.content);
            }
            items.push(tab.item);
        }
        let variant = tabs.1.variant;
        let mut bar = tab_bar(items).selected(selected).variant(variant);
        bar.1.callback = tabs.1.callback;
        let mut strip: Element<C> = bar.into();
        if variant == TabVariant::Line && strip.node.ext.fraction[0].is_none() {
            // The hairline runs under the whole panel, not just the tabs.
            strip.node.ext.fraction[0] = Some(1.);
        }
        if !tabs.0.auto_id {
            strip.node.id = format!("{}-strip", tabs.0.node.id);
            strip.auto_id = false;
        }
        if let Some(content) = content {
            tabs.0.children = match tabs.1.placement {
                TabPlacement::Top => vec![strip, content],
                TabPlacement::Bottom => vec![content, strip],
            };
        } else {
            tabs.0.children = vec![strip];
        }
        tabs.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_segment_uses_the_paired_primary_foreground() {
        let e: Element<()> = segmented_control([(0, "One"), (1, "Two")])
            .selected(0)
            .on_select(|_: &mut (), _| {})
            .color(ColorToken::BaseContent)
            .into();
        assert!(e.children[0].node.primary);
        assert_eq!(
            e.children[0].node.style.color.as_deref(),
            Some("@primary-content")
        );
        assert_eq!(e.children[1].node.style.color, None);
    }
    #[test]
    fn segmented_controls_ignore_the_tab_variant() {
        let e: Element<()> = segmented_control([(0, "One"), (1, "Two")])
            .variant(TabVariant::Contained)
            .on_select(|_: &mut (), _| {})
            .into();
        assert_eq!(e.node.style.background, None);
        assert_eq!(e.node.gap, 1., "joined, not a contained track");
    }
    fn two_tabs(variant: TabVariant) -> Element<()> {
        tab_bar([tab_item(0, "One"), tab_item(1, "Two")])
            .selected(0)
            .variant(variant)
            .on_select(|_: &mut (), _| {})
            .into()
    }
    #[test]
    fn line_tabs_underline_the_selection_over_a_hairline() {
        let e = two_tabs(TabVariant::Line);
        assert_eq!(e.node.ext.border_sides, Some(Sides::BOTTOM));
        assert_eq!(e.node.style.border_color.as_deref(), Some("@border"));
        let [on, off] = [&e.children[0].node, &e.children[1].node];
        assert!(!on.primary, "no filled selection");
        assert_eq!(on.ext.border_sides, Some(Sides::BOTTOM));
        assert_eq!(on.style.border_width, Some(2.));
        assert_eq!(on.style.border_color.as_deref(), Some("@primary"));
        assert_eq!(on.style.color.as_deref(), Some("@base-content"));
        assert_eq!(off.ext.border_sides, None);
        assert_eq!(off.style.color.as_deref(), Some("@muted"));
        assert!(e.build().is_ok());
    }
    #[test]
    fn contained_tabs_raise_the_selection_in_a_track() {
        let e = two_tabs(TabVariant::Contained);
        assert_eq!(e.node.style.background.as_deref(), Some("@base-content/6"));
        assert_eq!(e.node.padding, 3.);
        assert_eq!(e.node.align, Align::Stretch);
        let [on, off] = [&e.children[0].node, &e.children[1].node];
        assert_eq!(on.style.background.as_deref(), Some("@base-100"));
        assert_eq!(on.ext.border_sides, None);
        assert_eq!(off.style.background.as_deref(), Some("@base-content/0"));
        assert!(
            on.flex > 0. && on.flex == off.flex,
            "chips share a wide track"
        );
        assert!(e.build().is_ok());
    }
    #[test]
    fn an_app_styled_strip_keeps_its_own_surface() {
        let e: Element<()> = tab_bar([tab_item(0, "One")])
            .variant(TabVariant::Contained)
            .background(ColorToken::Base200)
            .padding(8., Edge::All)
            .on_select(|_: &mut (), _| {})
            .into();
        assert_eq!(e.node.style.background.as_deref(), Some("@base-200"));
        assert_eq!(e.node.padding, 0., "responsive padding still applies");
    }
    #[test]
    fn tabs_stretch_the_line_strip_under_the_panel() {
        let e: Element<()> = tabs([tab(0, "One", text("A")), tab(1, "Two", text("B"))])
            .key("tabs")
            .on_select(|_: &mut (), _| {})
            .into();
        assert_eq!(e.children[0].node.ext.fraction[0], Some(1.));
        assert_eq!(e.children[0].node.id, "tabs-strip");
    }
    #[test]
    fn oversized_selection_is_rejected_before_lowering() {
        let items = (0..MAX_NODES)
            .map(|i| tab_item(i, "Item"))
            .collect::<Vec<_>>();
        assert!(selection_error(&items, &0).unwrap().contains("node limit"));
        let e: Element<()> = tab_bar(items).on_select(|_: &mut (), _| {}).into();
        assert!(e.children.is_empty());
        assert!(e.build().is_err());
    }
    fn buttons() -> ButtonBar<()> {
        button_bar([
            button("One").on_click(|_: &mut ()| {}).into(),
            button("Two").on_click(|_: &mut ()| {}).into(),
            button("Three").on_click(|_: &mut ()| {}).into(),
        ])
    }
    #[test]
    fn joined_corners_keep_symbolic_full_and_square_inner_edges() {
        let e: Element<()> = buttons()
            .joined(true)
            .rounded(Rounding::Full)
            .gap(20., Axis::Both)
            .into();
        let zero = Rounding::Radius(0.);
        assert_eq!(e.node.gap, 1., "a 1px seam between joined segments");
        assert_eq!(
            e.children[0].node.style.corners,
            Some([Rounding::Full, zero, zero, Rounding::Full])
        );
        assert_eq!(e.children[1].node.style.corners, Some([zero; 4]));
        assert_eq!(
            e.children[2].node.style.corners,
            Some([zero, Rounding::Full, Rounding::Full, zero])
        );
        assert!(e.build().is_ok());
    }
    #[test]
    fn joined_corners_respect_selected_outer_corners() {
        let e: Element<()> = buttons()
            .joined(true)
            .rounded_on(10., [Corner::TopLeft, Corner::BottomRight])
            .into();
        let zero = Rounding::Radius(0.);
        assert_eq!(
            e.children[0].node.style.corners,
            Some([Rounding::Radius(10.), zero, zero, zero])
        );
        assert_eq!(
            e.children[2].node.style.corners,
            Some([zero, zero, Rounding::Radius(10.), zero])
        );
    }
    #[test]
    fn single_joined_button_keeps_all_outer_corners() {
        let e: Element<()> = button_bar![button("Only").on_click(|_: &mut ()| {})]
            .joined(true)
            .rounded(8.)
            .into();
        assert_eq!(
            e.children[0].node.style.corners,
            Some([Rounding::Radius(8.); 4])
        );
    }
}
