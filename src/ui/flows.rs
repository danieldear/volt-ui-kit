//! Small phone patterns: a task in steps inside one sheet, quick picks in
//! front of a full picker, and a line that says how fresh the page is.
use super::*;
use crate::declarative::AccessRole;
use std::{cell::RefCell, rc::Rc, time::Duration};

type Press<C> = Rc<RefCell<dyn FnMut(&mut C)>>;
type Pick<C> = Rc<RefCell<dyn FnMut(&mut C, usize)>>;

// ── Sheet flow ──────────────────────────────────────────────────────────

/// One step of a [`sheet_flow`].
pub struct FlowStep<C> {
    title: String,
    content: Element<C>,
    height: f32,
    ready: bool,
}
/// A step: its title, what it asks, and how tall the sheet is for it.
pub fn flow_step<C>(title: impl Into<String>, content: impl Into<Element<C>>) -> FlowStep<C> {
    FlowStep {
        title: title.into(),
        content: content.into(),
        height: 360.,
        ready: true,
    }
}
impl<C> FlowStep<C> {
    /// The sheet's height for this step, in points.
    pub fn height(mut self, points: f32) -> Self {
        self.height = points;
        self
    }
    /// Whether Next (or Done) can be pressed yet.
    pub fn ready(mut self, value: bool) -> Self {
        self.ready = value;
        self
    }
}

/// See [`sheet_flow`].
pub struct SheetFlow<C> {
    key: Option<String>,
    at: usize,
    steps: Vec<FlowStep<C>>,
    open: bool,
    on_step: Option<Pick<C>>,
    on_finish: Option<Press<C>>,
    on_cancel: Option<Press<C>>,
    finish: String,
}

/// A task in a few steps inside one sheet: Cancel or Back at the leading
/// edge, Next or the finishing word at the trailing edge, the steps' dots
/// at the top, and the sheet's height following each step.
///
/// ```rust,ignore
/// sheet_flow(app.step)
///     .key("new-project")
///     .step(flow_step("Name", name_field).height(260.).ready(!app.name.is_empty()))
///     .step(flow_step("Agent", agent_picker).height(420.))
///     .finish("Create")
///     .open(app.creating)
///     .on_step(|a: &mut App, i| a.step = i)
///     .on_finish(|a: &mut App| a.create())
///     .on_cancel(|a: &mut App| a.creating = false)
/// ```
pub fn sheet_flow<C>(at: usize) -> SheetFlow<C> {
    SheetFlow {
        key: None,
        at,
        steps: Vec::new(),
        open: false,
        on_step: None,
        on_finish: None,
        on_cancel: None,
        finish: "Done".into(),
    }
}
impl<C> SheetFlow<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn step(mut self, step: FlowStep<C>) -> Self {
        self.steps.push(step);
        self
    }
    /// The last step's button: say what it does ("Create", "Send").
    pub fn finish(mut self, label: impl Into<String>) -> Self {
        self.finish = label.into();
        self
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    /// Required: go to step `i` (Next and Back).
    pub fn on_step(mut self, f: impl FnMut(&mut C, usize) + 'static) -> Self {
        self.on_step = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Required: the last step's button.
    pub fn on_finish(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.on_finish = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Required: Cancel, or the sheet swiped away.
    pub fn on_cancel(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(RefCell::new(f)));
        self
    }
}

impl<C: 'static> From<SheetFlow<C>> for Element<C> {
    fn from(f: SheetFlow<C>) -> Self {
        let invalid = |m: &str| {
            let mut e = Element::new(Kind::Column);
            e.validation_error = Some(m.into());
            e
        };
        let Some(key) = f.key.filter(|k| !k.is_empty()) else {
            return invalid("sheet_flow requires a key");
        };
        let (Some(step_to), Some(finish), Some(cancel)) = (f.on_step, f.on_finish, f.on_cancel)
        else {
            return invalid("sheet_flow requires on_step, on_finish and on_cancel");
        };
        if f.steps.is_empty() {
            return invalid("sheet_flow needs a step");
        }
        let count = f.steps.len();
        let at = f.at.min(count - 1);
        let last = at + 1 == count;
        let mut steps = f.steps;
        let step = steps.swap_remove(at);
        let (ready, height, title) = (step.ready, step.height, step.title);

        // Where you are: a dot per step, the current one long.
        let dots: Vec<Element<C>> = (0..count)
            .map(|i| {
                let current = i == at;
                vstack(Vec::<Element<C>>::new())
                    .width(if current { 20. } else { 6. })
                    .height(6.)
                    .rounded(Rounding::Full)
                    .background(if i <= at {
                        ColorToken::Primary
                    } else {
                        ColorToken::Border
                    })
                    .animation(Animation::spring())
                    .into()
            })
            .collect();
        let mut place = hstack(dots)
            .key(format!("{key}-steps"))
            .gap(6., Axis::Horizontal)
            .justify(Justify::Center)
            .accessible_label(format!("Step {} of {count}", at + 1));
        place.0.node.access_role = Some(AccessRole::Group);
        let body = vstack![place, step.content]
            .key(format!("{key}-step-{at}"))
            .gap(16., Axis::Vertical);

        let back = step_to.clone();
        let next = step_to;
        let mut sheet = bottom_sheet(body)
            .key(key)
            .title(title)
            .detents([SheetDetent::Points(height)])
            .open(f.open);
        let shut = cancel.clone();
        sheet = sheet.on_dismiss(move |c: &mut C| (shut.borrow_mut())(c));
        sheet = if at == 0 {
            sheet.cancel("Cancel", move |c: &mut C| (cancel.borrow_mut())(c))
        } else {
            sheet.cancel("Back", move |c: &mut C| (back.borrow_mut())(c, at - 1))
        };
        sheet = if last {
            sheet.done(f.finish, move |c: &mut C| (finish.borrow_mut())(c))
        } else {
            sheet.done("Next", move |c: &mut C| (next.borrow_mut())(c, at + 1))
        };
        sheet.done_enabled(ready).into()
    }
}

// ── Quick picks ─────────────────────────────────────────────────────────

/// See [`quick_picks`].
pub struct QuickPicks<C> {
    key: Option<String>,
    items: Vec<String>,
    selected: Option<usize>,
    on_pick: Option<Pick<C>>,
    more: Option<(String, Press<C>)>,
}
/// The few answers people choose most, as chips in front of the full
/// picker: "Today", "Tomorrow", "Next week" before a calendar.
pub fn quick_picks<C>(items: impl IntoIterator<Item = impl Into<String>>) -> QuickPicks<C> {
    QuickPicks {
        key: None,
        items: items.into_iter().take(6).map(Into::into).collect(),
        selected: None,
        on_pick: None,
        more: None,
    }
}
impl<C> QuickPicks<C> {
    /// Required. Chips are keyed `{key}-{n}`, More `{key}-more`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }
    /// Required: chip `i` was chosen.
    pub fn on_pick(mut self, f: impl FnMut(&mut C, usize) + 'static) -> Self {
        self.on_pick = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// The full picker, behind a last chip: "More dates", "All models".
    pub fn more(mut self, label: impl Into<String>, f: impl FnMut(&mut C) + 'static) -> Self {
        self.more = Some((label.into(), Rc::new(RefCell::new(f))));
        self
    }
}
/// A chip: Material 3's filter chip on Android, a capsule on iOS.
fn chip<C: 'static>(key: String, label: String, on: bool, press: Press<C>) -> Element<C> {
    let android = cfg!(target_os = "android");
    let mut c = hstack![text(label.clone())
        .font_size(if android { 14. } else { 15. })
        .font_weight(if on { 600 } else { 400 })
        .color(if on {
            ColorToken::PrimaryContent
        } else {
            ColorToken::BaseContent
        })]
    .key(key)
    .height(if android { 32. } else { 34. })
    .padding(if android { 16. } else { 14. }, Edge::Horizontal)
    .align(Align::Center)
    .rounded(if android {
        Rounding::Radius(8.)
    } else {
        Rounding::Full
    })
    // iOS fills an unchosen chip; Material outlines it.
    .background(if on {
        ColorToken::Primary.into()
    } else if android {
        "#00000000".to_owned()
    } else {
        // A tint of the text colour shows on any surface, sheets included.
        ColorToken::BaseContent.alpha(12)
    })
    .accessible_label(label)
    .pressed_style(|s| s.opacity(0.75))
    .on_click_with(move |ctx: &mut C, _| (press.borrow_mut())(ctx));
    if !on && android {
        c = c.border_color(ColorToken::Border).border_width(1.);
    }
    c.0.node.access_role = Some(AccessRole::Button);
    c.0.node.access_selected = Some(on);
    c.into()
}
impl<C: 'static> From<QuickPicks<C>> for Element<C> {
    fn from(q: QuickPicks<C>) -> Self {
        let (Some(key), Some(pick)) = (q.key.filter(|k| !k.is_empty()), q.on_pick) else {
            let mut e = Element::new(Kind::Column);
            e.validation_error = Some("quick_picks requires a key and on_pick".into());
            return e;
        };
        let mut chips: Vec<Element<C>> = q
            .items
            .into_iter()
            .enumerate()
            .map(|(i, label)| {
                let pick = pick.clone();
                let press: Press<C> =
                    Rc::new(RefCell::new(move |c: &mut C| (pick.borrow_mut())(c, i)));
                chip(format!("{key}-{i}"), label, q.selected == Some(i), press)
            })
            .collect();
        if let Some((label, press)) = q.more {
            chips.push(chip(format!("{key}-more"), label, false, press));
        }
        hstack(chips)
            .key(key)
            .gap(8., Axis::Horizontal)
            .scroll(Scroll::Horizontal)
            .into()
    }
}

// ── Freshness line ──────────────────────────────────────────────────────

/// See [`freshness_line`].
pub struct FreshnessLine<C> {
    key: Option<String>,
    age: Option<Duration>,
    stale_after: Duration,
    offline: bool,
    refreshing: bool,
    refresh: Option<Press<C>>,
}
/// How fresh the page is, under its title: "Updated 2 min ago". It turns
/// amber when stale, says "Offline" plainly, and a tap refreshes. `age` is
/// how long since the last update; `None` means never.
pub fn freshness_line<C>(age: Option<Duration>) -> FreshnessLine<C> {
    FreshnessLine {
        key: None,
        age,
        stale_after: Duration::from_secs(15 * 60),
        offline: false,
        refreshing: false,
        refresh: None,
    }
}
impl<C> FreshnessLine<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// When it starts to look stale. Default 15 minutes.
    pub fn stale_after(mut self, age: Duration) -> Self {
        self.stale_after = age;
        self
    }
    /// No connection: say so rather than how old it is.
    pub fn offline(mut self, value: bool) -> Self {
        self.offline = value;
        self
    }
    /// A refresh is under way.
    pub fn refreshing(mut self, value: bool) -> Self {
        self.refreshing = value;
        self
    }
    /// A tap refreshes.
    pub fn on_refresh(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.refresh = Some(Rc::new(RefCell::new(f)));
        self
    }
}
/// "just now", "2 min ago", "3 h ago", "2 days ago".
pub fn ago(age: Duration) -> String {
    let s = age.as_secs();
    match s {
        0..=44 => "just now".into(),
        45..=3599 => format!("{} min ago", s.div_ceil(60).max(1)),
        3600..=86_399 => format!("{} h ago", s / 3600),
        _ => match s / 86_400 {
            1 => "yesterday".into(),
            d => format!("{d} days ago"),
        },
    }
}
impl<C: 'static> From<FreshnessLine<C>> for Element<C> {
    fn from(f: FreshnessLine<C>) -> Self {
        let key = f.key.unwrap_or_else(|| "freshness".into());
        let stale = f.age.is_none_or(|a| a >= f.stale_after);
        let (words, tint, glyph): (String, ColorToken, Option<Icon>) = if f.offline {
            (
                "Offline. Showing what was saved.".into(),
                ColorToken::Muted,
                Some(Icon::Warning),
            )
        } else if f.refreshing {
            ("Updating".into(), ColorToken::Muted, None)
        } else {
            match f.age {
                None => (
                    "Not updated yet".into(),
                    ColorToken::Warning,
                    Some(Icon::Refresh),
                ),
                Some(a) => (
                    format!("Updated {}", ago(a)),
                    if stale {
                        ColorToken::Warning
                    } else {
                        ColorToken::Muted
                    },
                    stale.then_some(Icon::Refresh),
                ),
            }
        };
        let mut parts: Vec<Element<C>> = Vec::new();
        if f.refreshing && !f.offline {
            parts.push(
                spinner()
                    .width(14.)
                    .height(14.)
                    .accent_color(ColorToken::Muted)
                    .into(),
            );
        } else if let Some(g) = glyph {
            parts.push(icon(g).size(14.).stroke(2.).color(tint).into());
        }
        parts.push(text(words.clone()).font_size(13.).color(tint).into());
        let mut line = hstack(parts)
            .key(key)
            .gap(6., Axis::Horizontal)
            .align(Align::Center)
            .height(28.)
            .accessible_label(words);
        if let (Some(refresh), false) = (f.refresh, f.refreshing) {
            line = line
                .pressed_style(|s| s.opacity(0.6))
                .on_click_with(move |c: &mut C, _| (refresh.borrow_mut())(c));
            line.0.node.access_role = Some(AccessRole::Button);
        }
        line.into()
    }
}

#[cfg(test)]
mod tests {
    use super::ago;
    use std::time::Duration;
    #[test]
    fn ages_read_like_people_say_them() {
        let s = Duration::from_secs;
        assert_eq!(ago(s(10)), "just now");
        assert_eq!(ago(s(90)), "2 min ago");
        assert_eq!(ago(s(3 * 3600)), "3 h ago");
        assert_eq!(ago(s(86_400 + 5)), "yesterday");
        assert_eq!(ago(s(3 * 86_400)), "3 days ago");
    }
}
