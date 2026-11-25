//! A page's actions at the bottom of the screen, where thumbs reach.
//!
//! Both platforms put a page's most-used buttons (Add, Edit, Share, Done) in
//! the top bar, the hardest place to reach on a modern phone, and Android's
//! floating action button covers the last row of every list. The action dock
//! floats just above the tab bar instead: one main action, up to two more,
//! and "More" for the rest in a sheet. It moves away while you scroll down a
//! list and comes back as soon as you scroll up, and it rides on top of the
//! keyboard.
use super::*;
use crate::declarative::AccessRole;
use std::{cell::RefCell, rc::Rc};

type Press<C> = Rc<RefCell<dyn FnMut(&mut C)>>;

/// One action in an [`action_dock`].
pub struct DockAction<C> {
    label: String,
    icon: Icon,
    press: Option<Press<C>>,
    enabled: bool,
    danger: bool,
}
/// An action: a label that says what happens, and an icon for it.
pub fn dock_action<C>(label: impl Into<String>, icon: Icon) -> DockAction<C> {
    DockAction {
        label: label.into(),
        icon,
        press: None,
        enabled: true,
        danger: false,
    }
}
impl<C> DockAction<C> {
    pub fn on_press(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.press = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Show it but don't run it. Say on the page why it can't run yet.
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
    /// It destroys something: shown in the failed colour. Follow it with an
    /// undo bar rather than asking first, unless it truly can't come back.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

type Toggle<C> = Rc<RefCell<dyn FnMut(&mut C, bool)>>;
/// See [`action_dock`].
pub struct ActionDock<C> {
    key: Option<String>,
    content: Element<C>,
    primary: Option<DockAction<C>>,
    secondary: Vec<DockAction<C>>,
    more: Vec<DockAction<C>>,
    more_open: bool,
    on_more: Option<Toggle<C>>,
    follows: Option<String>,
    back: Option<(String, Press<C>)>,
}
/// A page with its actions in a dock at the bottom:
///
/// ```rust,ignore
/// action_dock(scroll_view_of_ideas.key("ideas"))
///     .key("ideas-dock")
///     .follows("ideas")
///     .primary(dock_action("New idea", Icon::Plus).on_press(|a: &mut App| a.compose()))
///     .secondary(dock_action("Search", Icon::Search).on_press(|a: &mut App| a.search()))
///     .more([dock_action("Select", Icon::Check).on_press(|a: &mut App| a.select_mode())])
///     .more_open(app.dock_more)
///     .on_more(|a: &mut App, open| a.dock_more = open)
/// ```
///
/// Put the page's top-bar actions here instead; the bar keeps the title and
/// Back.
pub fn action_dock<C>(content: impl Into<Element<C>>) -> ActionDock<C> {
    ActionDock {
        key: None,
        content: content.into(),
        primary: None,
        secondary: Vec::new(),
        more: Vec::new(),
        more_open: false,
        on_more: None,
        follows: None,
        back: None,
    }
}
impl<C> ActionDock<C> {
    /// Required. Parts are keyed `{key}-primary`, `{key}-action-{n}`,
    /// `{key}-more` and `{key}-back`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// The one thing people come to this page to do. Filled with the accent.
    pub fn primary(mut self, action: DockAction<C>) -> Self {
        self.primary = Some(action);
        self
    }
    /// A frequent action, shown as an icon button. Two at most; later ones
    /// move to More.
    pub fn secondary(mut self, action: DockAction<C>) -> Self {
        if self.secondary.len() < 2 {
            self.secondary.push(action);
        } else {
            self.more.insert(0, action);
        }
        self
    }
    /// Everything else, listed in a sheet from the dock's More button.
    /// Needs [`more_open`](Self::more_open) and [`on_more`](Self::on_more).
    pub fn more(mut self, actions: impl IntoIterator<Item = DockAction<C>>) -> Self {
        self.more.extend(actions);
        self
    }
    pub fn more_open(mut self, open: bool) -> Self {
        self.more_open = open;
        self
    }
    pub fn on_more(mut self, f: impl FnMut(&mut C, bool) + 'static) -> Self {
        self.on_more = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// The scroll view (by key) that the dock moves away for while it
    /// scrolls toward its end. That scroll view also gets room at its end
    /// so its last row clears the dock. Without it, the dock stays put and
    /// the first vertical scroll view in `content` gets the room.
    pub fn follows(mut self, scroll_key: impl Into<String>) -> Self {
        self.follows = Some(scroll_key.into());
        self
    }
    /// A Back button at the dock's leading end, in reach, for deep pages.
    /// The top bar's Back and the edge swipe still work.
    pub fn back(mut self, label: impl Into<String>, f: impl FnMut(&mut C) + 'static) -> Self {
        self.back = Some((label.into(), Rc::new(RefCell::new(f))));
        self
    }
}

/// The dock's measurements, in points.
const HEIGHT: f32 = 60.;
const INSET: f32 = 6.;
const BUTTON: f32 = HEIGHT - 2. * INSET;
const MARGIN: f32 = 12.;
const MAX_WIDTH: f32 = 560.;
/// Room a scroll view keeps at its end for the dock.
pub(crate) const CLEARANCE: f32 = HEIGHT + 2. * MARGIN;

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}

/// A pressable part of the dock: a container that dims and shrinks a little
/// under the finger, read as a button.
fn pressable<C: 'static>(
    key: String,
    label: &str,
    children: Vec<Element<C>>,
    press: Option<Press<C>>,
    enabled: bool,
) -> Container<C> {
    let mut part = hstack(children)
        .key(key)
        .height(BUTTON)
        .align(Align::Center)
        .justify(Justify::Center)
        .gap(8., Axis::Horizontal)
        .rounded(Rounding::Full)
        .accessible_label(label)
        .pressed_style(|s| s.opacity(0.72).scale(0.96, 0.96))
        .animate_on(Interaction::Pressed, Animation::spring());
    if enabled {
        if let Some(f) = press {
            part = part.on_click_with(move |c: &mut C, _| (f.borrow_mut())(c));
        }
    } else {
        part = part.opacity(0.4);
        part.0.node.enabled = false;
    }
    part.0.node.access_role = Some(AccessRole::Button);
    part
}

fn round<C: 'static>(
    key: String,
    label: &str,
    glyph: Icon,
    colour: ColorToken,
    press: Option<Press<C>>,
    enabled: bool,
) -> Element<C> {
    pressable(
        key,
        label,
        vec![icon(glyph).size(22.).stroke(2.).color(colour).into()],
        press,
        enabled,
    )
    .width(BUTTON)
    .background(ColorToken::Base300)
    .background_opacity(0.6)
    .into()
}

/// Give the scroll view keyed `key`, or the first vertical one, room at its
/// end: a spacer as its last child. (Padding would clip its content short of
/// the bars, which content should scroll under.)
fn make_room<C>(element: &mut Element<C>, key: Option<&str>, spacer_key: &str) -> bool {
    let vertical = element.node.surface.scroll.is_some_and(|s| s.axes()[1]);
    let matches = match key {
        Some(k) => element.node.id == k,
        None => vertical,
    };
    if matches {
        let room: Element<C> = vstack(Vec::<Element<C>>::new())
            .key(spacer_key)
            .height(CLEARANCE)
            .into();
        // A scroll view holding one stack (the usual page) takes the room
        // inside that stack, so its gaps and padding stay as they were.
        match element.children.as_mut_slice() {
            [only] if only.node.surface.scroll.is_none() && !only.children.is_empty() => {
                only.children.push(room);
            }
            _ => {
                element.children.push(room);
            }
        }
        return true;
    }
    element
        .children
        .iter_mut()
        .any(|c| make_room(c, key, spacer_key))
}

impl<C: 'static> From<ActionDock<C>> for Element<C> {
    fn from(dock: ActionDock<C>) -> Self {
        let Some(key) = dock.key.filter(|k| !k.is_empty()) else {
            return invalid("action_dock requires a key");
        };
        if dock.primary.is_none() && dock.secondary.is_empty() && dock.more.is_empty() {
            return invalid("action_dock requires at least one action");
        }
        if !dock.more.is_empty() && dock.on_more.is_none() {
            return invalid("action_dock with More actions requires on_more");
        }
        let mut content = dock.content;
        make_room(
            &mut content,
            dock.follows.as_deref(),
            &format!("{key}-room"),
        );

        let mut parts: Vec<Element<C>> = Vec::new();
        if let Some((label, f)) = dock.back {
            parts.push(round(
                format!("{key}-back"),
                &label,
                Icon::ChevronLeft,
                ColorToken::BaseContent,
                Some(f),
                true,
            ));
        }
        // With no main action the others share the dock with their labels.
        let labelled = dock.primary.is_none();
        for (i, a) in dock.secondary.into_iter().enumerate() {
            let colour = if a.danger {
                ColorToken::Error
            } else {
                ColorToken::BaseContent
            };
            if labelled {
                parts.push(
                    pressable(
                        format!("{key}-action-{i}"),
                        &a.label,
                        vec![
                            icon(a.icon).size(20.).stroke(2.).color(colour).into(),
                            text(a.label.clone())
                                .font_size(15.)
                                .font_weight(600)
                                .color(colour)
                                .into(),
                        ],
                        a.press,
                        a.enabled,
                    )
                    .flex(1.)
                    .background(ColorToken::Base300)
                    .background_opacity(0.6)
                    .into(),
                );
            } else {
                parts.push(round(
                    format!("{key}-action-{i}"),
                    &a.label,
                    a.icon,
                    colour,
                    a.press,
                    a.enabled,
                ));
            }
        }
        if let Some(a) = dock.primary {
            let (fill, ink) = if a.danger {
                (ColorToken::Error, ColorToken::ErrorContent)
            } else {
                (ColorToken::Primary, ColorToken::PrimaryContent)
            };
            parts.push(
                pressable(
                    format!("{key}-primary"),
                    &a.label,
                    vec![
                        icon(a.icon).size(20.).stroke(2.25).color(ink).into(),
                        text(a.label.clone())
                            .font_size(16.)
                            .font_weight(600)
                            .color(ink)
                            .into(),
                    ],
                    a.press,
                    a.enabled,
                )
                .flex(1.)
                .padding(18., Edge::Horizontal)
                .background(fill)
                .into(),
            );
        }
        let more = !dock.more.is_empty();
        let toggle = dock.on_more.clone();
        if more {
            let open = toggle.clone();
            let press: Press<C> = Rc::new(RefCell::new(move |c: &mut C| {
                if let Some(f) = &open {
                    (f.borrow_mut())(c, true);
                }
            }));
            parts.push(round(
                format!("{key}-more"),
                "More actions",
                Icon::MoreHorizontal,
                ColorToken::BaseContent,
                Some(press),
                true,
            ));
        }

        let mut bar = hstack(parts)
            .key(format!("{key}-bar"))
            .height(HEIGHT)
            .width_fraction(1.)
            .max_width(MAX_WIDTH)
            .padding(INSET, Edge::All)
            .gap(INSET, Axis::Horizontal)
            .align(Align::Center)
            .rounded(HEIGHT / 2.)
            .background(ColorToken::Base200)
            .border_color(ColorToken::Border)
            .border_width(0.5)
            // Lifted, but within its own margin: a deeper shadow would be
            // cut by the bars below.
            .shadow(Shadow::new().color("#0000001F").blur(10.).offset(0., 2.))
            .layout_animation(Animation::spring());
        bar.0.node.access_role = Some(AccessRole::Group);
        if let Some(scroll) = &dock.follows {
            bar.0.node.ext.hides_with = Some(scroll.clone());
        }

        // The layer spans the page but takes no input of its own: touches
        // and scrolling outside the bar reach the page under it. Margins are
        // spacers, not padding, so nothing clips the bar's shadow.
        let gap = |w: f32| -> Element<C> { vstack(Vec::<Element<C>>::new()).width(w).into() };
        let row = hstack![gap(16.), bar.flex(1.), gap(16.)]
            .key(format!("{key}-row"))
            .justify(Justify::Center);
        let mut foot = vstack(Vec::<Element<C>>::new())
            .key(format!("{key}-foot"))
            .height(MARGIN);
        foot.0.node.ext.above_bars = true;
        let layer = vstack![spacer(), row, foot]
            .key(format!("{key}-layer"))
            .width_fraction(1.)
            .height_fraction(1.)
            .scroll_passthrough(true);

        let mut page: Vec<Element<C>> = vec![
            vstack![content]
                .key(format!("{key}-page"))
                .width_fraction(1.)
                .height_fraction(1.)
                .into(),
            layer.into(),
        ];
        if more {
            let close = toggle.clone();
            let mut rows: Vec<Element<C>> = Vec::new();
            for (i, a) in dock.more.into_iter().enumerate() {
                let colour = if a.danger {
                    ColorToken::Error
                } else {
                    ColorToken::BaseContent
                };
                // Choosing an action closes the sheet first, then runs it.
                let (f, shut) = (a.press, toggle.clone());
                let press: Press<C> = Rc::new(RefCell::new(move |c: &mut C| {
                    if let Some(shut) = &shut {
                        (shut.borrow_mut())(c, false);
                    }
                    if let Some(f) = &f {
                        (f.borrow_mut())(c);
                    }
                }));
                let row = pressable(
                    format!("{key}-more-{i}"),
                    &a.label,
                    vec![
                        icon(a.icon).size(20.).stroke(2.).color(colour).into(),
                        text(a.label.clone())
                            .font_size(17.)
                            .color(colour)
                            .flex(1.)
                            .into(),
                    ],
                    Some(press),
                    a.enabled,
                )
                .height(50.)
                .justify(Justify::Start)
                .gap(14., Axis::Horizontal)
                .padding(16., Edge::Horizontal)
                .rounded(10.)
                .width_fraction(1.)
                .pressed_style(|s| s.opacity(0.6));
                rows.push(row.into());
            }
            page.push(
                bottom_sheet(vstack(rows).gap(2., Axis::Vertical))
                    .key(format!("{key}-more-sheet"))
                    .title("More actions")
                    .open(dock.more_open)
                    .detents([SheetDetent::Medium])
                    .on_dismiss(move |c: &mut C| {
                        if let Some(f) = &close {
                            (f.borrow_mut())(c, false);
                        }
                    })
                    .into(),
            );
        }
        zstack(page).key(key).flex(1.).into()
    }
}
