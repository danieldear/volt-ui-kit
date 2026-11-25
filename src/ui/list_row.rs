//! Rows of a phone list: a title with optional subtitle, leading icon,
//! trailing value and chevron; swipe actions on either side; and a Select
//! mode with checkmarks.
//!
//! Swipe actions are invisible on both platforms until you know to look,
//! easy to trigger by accident, and the same actions are often nowhere
//! else. Here a row's swipe actions are only shortcuts: put the same actions
//! in Select mode and the page's dock, and follow a destructive one with an
//! undo rather than a confirmation.
use super::*;
use crate::declarative::AccessRole;
use crate::motion::{DragMotion, Transform2D};
use std::{cell::RefCell, rc::Rc};

type Press<C> = Rc<RefCell<dyn FnMut(&mut C)>>;
type Open<C> = Rc<RefCell<dyn FnMut(&mut C, Option<SwipeSide>)>>;

/// Which side of a row is swiped open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeSide {
    /// Swiped right: the actions at the row's leading edge show.
    Leading,
    /// Swiped left: the actions at the trailing edge show.
    Trailing,
}

/// What a swipe action does, by meaning: its colour follows the theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwipeTone {
    #[default]
    Neutral,
    /// The accent: pin, mark read.
    Accent,
    /// Needs attention: flag, snooze.
    Warning,
    /// Removes something: delete, leave.
    Danger,
}

/// One action behind a row: see [`swipe_action`].
pub struct SwipeAction<C> {
    label: String,
    icon: Icon,
    tone: SwipeTone,
    press: Option<Press<C>>,
    full_swipe: bool,
}
/// An action revealed by swiping a row. Keep labels to one short word.
pub fn swipe_action<C>(label: impl Into<String>, icon: Icon) -> SwipeAction<C> {
    SwipeAction {
        label: label.into(),
        icon,
        tone: SwipeTone::Neutral,
        press: None,
        full_swipe: false,
    }
}
impl<C> SwipeAction<C> {
    pub fn tone(mut self, tone: SwipeTone) -> Self {
        self.tone = tone;
        self
    }
    pub fn on_press(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.press = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// A long swipe runs this action without stopping at the buttons. Only
    /// the outermost action on a side can have it. Follow a destructive one
    /// with an undo.
    pub fn full_swipe(mut self) -> Self {
        self.full_swipe = true;
        self
    }
}

/// See [`list_row`].
pub struct ListRow<C> {
    key: Option<String>,
    title: String,
    subtitle: Option<String>,
    leading: Option<Element<C>>,
    icon: Option<(Icon, ColorToken)>,
    value: Option<String>,
    chevron: bool,
    press: Option<Press<C>>,
    leading_actions: Vec<SwipeAction<C>>,
    trailing_actions: Vec<SwipeAction<C>>,
    open: Option<SwipeSide>,
    on_open: Option<Open<C>>,
    selecting: bool,
    selected: bool,
    on_toggle: Option<Press<C>>,
}
/// A list row. Put rows in a [`list_section`].
///
/// ```rust,ignore
/// list_row(&note.title)
///     .key(format!("note-{}", note.id))
///     .subtitle(&note.preview)
///     .value(&note.age)
///     .chevron(true)
///     .on_press(move |a: &mut App| a.open(id))
///     .trailing_actions([
///         swipe_action("Delete", Icon::Trash).tone(SwipeTone::Danger).full_swipe()
///             .on_press(move |a: &mut App| a.delete_with_undo(id)),
///     ])
///     .swipe_open(app.open_row(id))
///     .on_swipe_open(move |a: &mut App, side| a.set_open_row(id, side))
///     .selecting(app.selecting)
///     .selected(app.selection.contains(&id))
///     .on_toggle(move |a: &mut App| a.toggle(id))
/// ```
pub fn list_row<C>(title: impl Into<String>) -> ListRow<C> {
    ListRow {
        key: None,
        title: title.into(),
        subtitle: None,
        leading: None,
        icon: None,
        value: None,
        chevron: false,
        press: None,
        leading_actions: Vec::new(),
        trailing_actions: Vec::new(),
        open: None,
        on_open: None,
        selecting: false,
        selected: false,
        on_toggle: None,
    }
}
impl<C> ListRow<C> {
    /// Required. Parts are keyed `{key}-content`, `{key}-action-leading-{n}`
    /// and `{key}-action-trailing-{n}`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// A second line: a preview, a detail. One line; longer text is cut.
    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = Some(text.into());
        self
    }
    /// An icon in a small rounded tile of `tint`, as in Settings.
    pub fn icon(mut self, icon: Icon, tint: ColorToken) -> Self {
        self.icon = Some((icon, tint));
        self
    }
    /// Anything at the leading edge instead of an icon: an avatar.
    pub fn leading(mut self, element: impl Into<Element<C>>) -> Self {
        self.leading = Some(element.into());
        self
    }
    /// A short value at the trailing edge: a count, a time, a setting.
    pub fn value(mut self, text: impl Into<String>) -> Self {
        self.value = Some(text.into());
        self
    }
    /// A chevron: the row opens a page.
    pub fn chevron(mut self, value: bool) -> Self {
        self.chevron = value;
        self
    }
    pub fn on_press(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.press = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Actions revealed by swiping right. Up to three.
    pub fn leading_actions(mut self, actions: impl IntoIterator<Item = SwipeAction<C>>) -> Self {
        self.leading_actions = actions.into_iter().take(3).collect();
        self
    }
    /// Actions revealed by swiping left. Up to three; the first is outermost.
    pub fn trailing_actions(mut self, actions: impl IntoIterator<Item = SwipeAction<C>>) -> Self {
        self.trailing_actions = actions.into_iter().take(3).collect();
        self
    }
    /// Which side is swiped open (app state). Keep at most one row open in a
    /// list: opening another, scrolling, or tapping closes it.
    pub fn swipe_open(mut self, side: Option<SwipeSide>) -> Self {
        self.open = side;
        self
    }
    /// The row was swiped open or closed. Required with swipe actions.
    pub fn on_swipe_open(mut self, f: impl FnMut(&mut C, Option<SwipeSide>) + 'static) -> Self {
        self.on_open = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Select mode: the row shows a checkmark circle, a tap toggles it, and
    /// swipe actions are off.
    pub fn selecting(mut self, value: bool) -> Self {
        self.selecting = value;
        self
    }
    pub fn selected(mut self, value: bool) -> Self {
        self.selected = value;
        self
    }
    /// A tap in Select mode. Required with `selecting`.
    pub fn on_toggle(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(RefCell::new(f)));
        self
    }
}

/// Row and action measurements, in points.
const ACTION_WIDTH: f32 = 76.;
/// How far past its buttons a row must be swiped to run its full-swipe action.
const FULL_SWIPE: f32 = 120.;
/// How far a row must move to open, or how fast.
const OPEN_DISTANCE: f32 = 40.;
const OPEN_SPEED: f32 = 500.;

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}

fn tone_colours(tone: SwipeTone) -> (ColorToken, ColorToken) {
    match tone {
        SwipeTone::Neutral => (ColorToken::Neutral, ColorToken::NeutralContent),
        SwipeTone::Accent => (ColorToken::Primary, ColorToken::PrimaryContent),
        SwipeTone::Warning => (ColorToken::Warning, ColorToken::WarningContent),
        SwipeTone::Danger => (ColorToken::Error, ColorToken::ErrorContent),
    }
}

fn action_button<C: 'static>(
    key: String,
    action: SwipeAction<C>,
    close: Option<Open<C>>,
) -> Element<C> {
    let (fill, ink) = tone_colours(action.tone);
    let press = action.press;
    let mut b = vstack![
        icon(action.icon).size(20.).stroke(2.).color(ink),
        text(action.label.clone())
            .font_size(12.)
            .font_weight(600)
            .color(ink),
    ]
    .key(key)
    .width(ACTION_WIDTH)
    .height_fraction(1.)
    .align(Align::Center)
    .justify(Justify::Center)
    .gap(4., Axis::Vertical)
    .background(fill)
    .accessible_label(action.label)
    .pressed_style(|s| s.opacity(0.7))
    // Running an action closes the row first, as on iOS.
    .on_click_with(move |c: &mut C, _| {
        if let Some(close) = &close {
            (close.borrow_mut())(c, None);
        }
        if let Some(f) = &press {
            (f.borrow_mut())(c);
        }
    });
    b.0.node.access_role = Some(AccessRole::Button);
    b.into()
}

fn check_circle<C>(selected: bool) -> Element<C> {
    let mut parts: Vec<Element<C>> = Vec::new();
    if selected {
        parts.push(
            icon(Icon::Check)
                .size(14.)
                .stroke(3.)
                .color(ColorToken::PrimaryContent)
                .into(),
        );
    }
    let circle = zstack(parts)
        .width(24.)
        .height(24.)
        .flow(Flow::Center)
        .rounded(Rounding::Full);
    if selected {
        circle.background(ColorToken::Primary).into()
    } else {
        circle
            .border_color(ColorToken::Muted)
            .border_width(1.5)
            .into()
    }
}

impl<C: 'static> From<ListRow<C>> for Element<C> {
    fn from(row: ListRow<C>) -> Self {
        let Some(key) = row.key.filter(|k| !k.is_empty()) else {
            return invalid("list_row requires a key");
        };
        let swipes = !row.leading_actions.is_empty() || !row.trailing_actions.is_empty();
        if swipes && row.on_open.is_none() {
            return invalid("list_row with swipe actions requires on_swipe_open");
        }
        if row.selecting && row.on_toggle.is_none() {
            return invalid("list_row in Select mode requires on_toggle");
        }
        let has_subtitle = row.subtitle.is_some();
        let height = if has_subtitle { 64. } else { 52. };

        // The visible row.
        let mut parts: Vec<Element<C>> = Vec::new();
        if row.selecting {
            parts.push(check_circle(row.selected));
        }
        if let Some(leading) = row.leading {
            parts.push(leading);
        } else if let Some((glyph, tint)) = row.icon {
            parts.push(
                zstack![icon(glyph)
                    .size(17.)
                    .stroke(2.)
                    .color(ColorToken::PrimaryContent)]
                .width(30.)
                .height(30.)
                .flow(Flow::Center)
                .rounded(8.)
                .background(tint)
                .into(),
            );
        }
        let mut words: Vec<Element<C>> = vec![text(row.title.clone()).font_size(17.).into()];
        if let Some(sub) = &row.subtitle {
            words.push(
                text(sub.clone())
                    .font_size(15.)
                    .color(ColorToken::Muted)
                    .into(),
            );
        }
        parts.push(
            vstack(words)
                .flex(1.)
                .gap(2., Axis::Vertical)
                .justify(Justify::Center)
                .into(),
        );
        if let Some(value) = &row.value {
            parts.push(
                text(value.clone())
                    .font_size(17.)
                    .color(ColorToken::Muted)
                    .into(),
            );
        }
        if row.chevron && !row.selecting {
            parts.push(
                icon(Icon::ChevronRight)
                    .size(16.)
                    .stroke(2.25)
                    .color(ColorToken::Muted)
                    .into(),
            );
        }

        // Where the row rests: shifted aside by its open actions.
        let open = if row.selecting { None } else { row.open };
        let leading_w = row.leading_actions.len() as f32 * ACTION_WIDTH;
        let trailing_w = row.trailing_actions.len() as f32 * ACTION_WIDTH;
        let rest = match open {
            Some(SwipeSide::Leading) => leading_w,
            Some(SwipeSide::Trailing) => -trailing_w,
            None => 0.,
        };
        let full_leading = row.leading_actions.first().is_some_and(|a| a.full_swipe);
        let full_trailing = row.trailing_actions.first().is_some_and(|a| a.full_swipe);

        let label = match (&row.subtitle, &row.value) {
            (Some(s), Some(v)) => format!("{}, {s}, {v}", row.title),
            (Some(s), None) => format!("{}, {s}", row.title),
            (None, Some(v)) => format!("{}, {v}", row.title),
            (None, None) => row.title.clone(),
        };
        let mut content = hstack(parts)
            .key(format!("{key}-content"))
            .width_fraction(1.)
            .height(height)
            .padding(16., Edge::Horizontal)
            .gap(12., Axis::Horizontal)
            .align(Align::Center)
            .background(ColorToken::Base200)
            .accessible_label(label)
            .transform(Transform2D::new().translate(rest, 0.))
            .animation(Animation::spring());
        if row.selecting {
            content.0.node.access_role = Some(AccessRole::CheckBox);
            content.0.node.checked = row.selected;
        } else if row.press.is_some() {
            content.0.node.access_role = Some(AccessRole::Button);
        }
        let (press, toggle, close) = (row.press, row.on_toggle, row.on_open.clone());
        let selecting = row.selecting;
        content = content
            .pressed_style(|s| s.opacity(0.85))
            .on_click_with(move |c: &mut C, _| {
                if selecting {
                    if let Some(f) = &toggle {
                        (f.borrow_mut())(c);
                    }
                } else if open.is_some() {
                    // A tap on an open row closes it.
                    if let Some(f) = &close {
                        (f.borrow_mut())(c, None);
                    }
                } else if let Some(f) = &press {
                    (f.borrow_mut())(c);
                }
            });

        let mut leading_full: Option<Press<C>> = None;
        let mut trailing_full: Option<Press<C>> = None;
        if swipes && !row.selecting {
            // The finger moves the row; where it settles is decided on
            // release from how far and how fast it went.
            let reach_right = if row.leading_actions.is_empty() {
                0.
            } else if full_leading {
                leading_w + FULL_SWIPE + 80.
            } else {
                leading_w + 24.
            };
            let reach_left = if row.trailing_actions.is_empty() {
                0.
            } else if full_trailing {
                trailing_w + FULL_SWIPE + 80.
            } else {
                trailing_w + 24.
            };
            content = content.drag_motion(
                DragMotion::new()
                    .bounds([-reach_left - rest, 0.], [reach_right - rest, 0.])
                    .momentum(false)
                    .spring_back(true),
            );
            if full_leading {
                leading_full = row.leading_actions[0].press.clone();
            }
            if full_trailing {
                trailing_full = row.trailing_actions[0].press.clone();
            }
            let on_open = row.on_open.clone();
            let (has_leading, has_trailing) = (
                !row.leading_actions.is_empty(),
                !row.trailing_actions.is_empty(),
            );
            content = content.on_pointer(move |c: &mut C, p: Pointer| {
                if p.phase != PointerPhase::Release {
                    return;
                }
                let [dx, dy] = p.offset;
                if dx.abs() < 6. || dy.abs() > dx.abs() {
                    return;
                }
                let at = rest + dx;
                let vx = p.velocity[0];
                let set = |c: &mut C, side: Option<SwipeSide>| {
                    if let Some(f) = &on_open {
                        (f.borrow_mut())(c, side);
                    }
                };
                // A long swipe runs the outermost action.
                if at <= -(trailing_w + FULL_SWIPE) {
                    if let Some(f) = &trailing_full {
                        set(c, None);
                        (f.borrow_mut())(c);
                        return;
                    }
                }
                if at >= leading_w + FULL_SWIPE {
                    if let Some(f) = &leading_full {
                        set(c, None);
                        (f.borrow_mut())(c);
                        return;
                    }
                }
                let next = if has_trailing && (at < -OPEN_DISTANCE || (vx < -OPEN_SPEED && at < 0.))
                {
                    Some(SwipeSide::Trailing)
                } else if has_leading && (at > OPEN_DISTANCE || (vx > OPEN_SPEED && at > 0.)) {
                    Some(SwipeSide::Leading)
                } else {
                    None
                };
                // A flick back toward the middle closes, wherever it ends.
                let next = match next {
                    Some(SwipeSide::Trailing) if vx > OPEN_SPEED => None,
                    Some(SwipeSide::Leading) if vx < -OPEN_SPEED => None,
                    other => other,
                };
                set(c, next);
            });
        }

        let mut layers: Vec<Element<C>> = Vec::new();
        if swipes && !row.selecting {
            let mut lead: Vec<Element<C>> = Vec::new();
            for (i, a) in row.leading_actions.into_iter().enumerate() {
                lead.push(action_button(
                    format!("{key}-action-leading-{i}"),
                    a,
                    row.on_open.clone(),
                ));
            }
            let mut trail: Vec<Element<C>> = Vec::new();
            // The first trailing action is outermost, at the trailing edge.
            for (i, a) in row.trailing_actions.into_iter().enumerate().rev() {
                trail.push(action_button(
                    format!("{key}-action-trailing-{i}"),
                    a,
                    row.on_open.clone(),
                ));
            }
            // Only the side that is open (or being swiped) shows; the other
            // side's buttons stay behind the row.
            let mut under: Vec<Element<C>> = lead;
            under.push(spacer().into());
            under.extend(trail);
            layers.push(
                hstack(under)
                    .key(format!("{key}-actions"))
                    .width_fraction(1.)
                    .height(height)
                    .into(),
            );
        }
        layers.push(content.into());
        zstack(layers)
            .key(key)
            .width_fraction(1.)
            .height(height)
            .clip_contents(true)
            .into()
    }
}

/// Rows on one rounded surface with hairlines between them, an optional
/// header above and footer below: iOS's inset grouped list, Material's list
/// in a card.
pub fn list_section<C>(
    header: impl Into<String>,
    rows: impl IntoIterator<Item = Element<C>>,
    footer: impl Into<String>,
) -> Container<C> {
    let (header, footer) = (header.into(), footer.into());
    let mut items: Vec<Element<C>> = Vec::new();
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            items.push(
                vstack![divider().height(1. / 3.).color(ColorToken::Border)]
                    .padding(16., Edge::Left)
                    .background(ColorToken::Base200)
                    .align(Align::Stretch)
                    .into(),
            );
        }
        items.push(r);
    }
    let mut parts: Vec<Element<C>> = Vec::new();
    if !header.is_empty() {
        parts.push(
            hstack![text(header)
                .font_size(15.)
                .font_weight(600)
                .color(ColorToken::Muted)]
            .padding(16., Edge::Horizontal)
            .into(),
        );
    }
    parts.push(
        vstack(items)
            .background(ColorToken::Base200)
            .rounded(22.)
            .clip_contents(true)
            .into(),
    );
    if !footer.is_empty() {
        parts.push(
            hstack![text(footer).font_size(13.).color(ColorToken::Muted)]
                .padding(16., Edge::Horizontal)
                .into(),
        );
    }
    vstack(parts).gap(8., Axis::Vertical)
}
