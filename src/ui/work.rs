//! Things that run: a work card for each job, and an activity pill above the
//! bottom bars while background work goes on.
//!
//! State is colour, shape and words together, never colour alone: working
//! has a spinner, waiting a clock, failed a warning sign, done a check.
use super::*;
use crate::declarative::AccessRole;
use std::{cell::RefCell, rc::Rc};

type Press<C> = Rc<RefCell<dyn FnMut(&mut C)>>;

/// Where a job is.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkState {
    /// Running. `Some` progress is 0..=1; `None` doesn't know how far.
    Working(Option<f32>),
    /// Stopped until someone answers: say what it needs.
    Waiting,
    /// Stopped by a problem: say why and what to do.
    Failed,
    Done,
}
impl WorkState {
    fn tint(&self) -> ColorToken {
        match self {
            WorkState::Working(_) => ColorToken::Info,
            WorkState::Waiting => ColorToken::Warning,
            WorkState::Failed => ColorToken::Error,
            WorkState::Done => ColorToken::Success,
        }
    }
    fn word(&self) -> &'static str {
        match self {
            WorkState::Working(_) => "Working",
            WorkState::Waiting => "Needs you",
            WorkState::Failed => "Failed",
            WorkState::Done => "Done",
        }
    }
}

/// The state's mark: a spinner while working, an icon otherwise.
fn mark<C>(state: &WorkState, size: f32) -> Element<C> {
    let tint = state.tint();
    let inner: Element<C> = match state {
        WorkState::Working(_) => spinner()
            .width(size * 0.55)
            .height(size * 0.55)
            .accent_color(tint)
            .into(),
        WorkState::Waiting => icon(Icon::Clock)
            .size(size * 0.5)
            .stroke(2.)
            .color(tint)
            .into(),
        WorkState::Failed => icon(Icon::Warning)
            .size(size * 0.5)
            .stroke(2.)
            .color(tint)
            .into(),
        WorkState::Done => icon(Icon::Check)
            .size(size * 0.5)
            .stroke(2.5)
            .color(tint)
            .into(),
    };
    zstack![inner]
        .width(size)
        .height(size)
        .flow(Flow::Center)
        .rounded(Rounding::Full)
        .background(tint.alpha(36))
        .into()
}

/// See [`work_card`].
pub struct WorkCard<C> {
    key: Option<String>,
    title: String,
    state: WorkState,
    detail: String,
    action: Option<(String, Press<C>)>,
    press: Option<Press<C>>,
}

/// A job: what it is, where it is, and the one thing to do about it.
///
/// ```rust,ignore
/// work_card("Refactor the parser")
///     .key("job-7")
///     .state(WorkState::Waiting)
///     .detail("Wants to run the test suite")
///     .action("Allow", |a: &mut App| a.allow(7))
///     .on_press(|a: &mut App| a.open_job(7))
/// ```
pub fn work_card<C>(title: impl Into<String>) -> WorkCard<C> {
    WorkCard {
        key: None,
        title: title.into(),
        state: WorkState::Working(None),
        detail: String::new(),
        action: None,
        press: None,
    }
}
impl<C> WorkCard<C> {
    /// Required. The action is keyed `{key}-action`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn state(mut self, state: WorkState) -> Self {
        self.state = state;
        self
    }
    /// One line: what it's doing, what it needs, why it failed, what it made.
    pub fn detail(mut self, text: impl Into<String>) -> Self {
        self.detail = text.into();
        self
    }
    /// The one thing to do now: "Allow", "Try again", "Open".
    pub fn action(mut self, label: impl Into<String>, f: impl FnMut(&mut C) + 'static) -> Self {
        self.action = Some((label.into(), Rc::new(RefCell::new(f))));
        self
    }
    /// Open the job.
    pub fn on_press(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.press = Some(Rc::new(RefCell::new(f)));
        self
    }
}

impl<C: 'static> From<WorkCard<C>> for Element<C> {
    fn from(w: WorkCard<C>) -> Self {
        let Some(key) = w.key.filter(|k| !k.is_empty()) else {
            let mut e = Element::new(Kind::Column);
            e.validation_error = Some("work_card requires a key".into());
            return e;
        };
        let tint = w.state.tint();
        let status = match &w.state {
            WorkState::Working(Some(p)) => {
                format!("{} {}%", w.state.word(), (p.clamp(0., 1.) * 100.).round())
            }
            other => other.word().to_owned(),
        };
        let mut lines: Vec<Element<C>> = vec![
            text(w.title.clone()).font_size(17.).font_weight(600).into(),
            text(status.clone())
                .font_size(15.)
                .font_weight(600)
                .color(tint)
                .into(),
        ];
        if !w.detail.is_empty() {
            lines.push(
                text(w.detail.clone())
                    .font_size(15.)
                    .color(ColorToken::Muted)
                    .into(),
            );
        }
        let mut body: Vec<Element<C>> = vec![hstack![
            mark(&w.state, 36.),
            vstack(lines).gap(2., Axis::Vertical).flex(1.),
        ]
        .gap(12., Axis::Horizontal)
        .align(Align::Start)
        .into()];
        if let WorkState::Working(Some(p)) = w.state {
            let mut bar = progress(p.clamp(0., 1.))
                .width_fraction(1.)
                .height(4.)
                .accent_color(tint);
            // Across the card, not the default bar width.
            bar.0.node.width = None;
            body.push(bar.into());
        }
        if let Some((label, press)) = w.action {
            // Filled in the state's colour: the thing to do now.
            let mut b = hstack![text(label.clone())
                .font_size(15.)
                .font_weight(600)
                .color(ColorToken::PrimaryContent)]
            .key(format!("{key}-action"))
            .height(36.)
            .padding(16., Edge::Horizontal)
            .align(Align::Center)
            .rounded(Rounding::Full)
            .background(if matches!(w.state, WorkState::Done) {
                ColorToken::Primary
            } else {
                tint
            })
            .accessible_label(label)
            .pressed_style(|s| s.opacity(0.8))
            .on_click_with(move |c: &mut C, _| (press.borrow_mut())(c));
            b.0.node.access_role = Some(AccessRole::Button);
            body.push(hstack![b].padding(48., Edge::Left).into());
        }
        let mut card = vstack(body)
            .key(key)
            .gap(12., Axis::Vertical)
            .padding(16., Edge::All)
            .rounded(22.)
            .background(ColorToken::Base200)
            .accessible_label(format!("{}, {status}", w.title));
        if let Some(press) = w.press {
            card = card
                .pressed_style(|s| s.opacity(0.85))
                .on_click_with(move |c: &mut C, _| (press.borrow_mut())(c));
            card.0.node.access_role = Some(AccessRole::Button);
        }
        card.into()
    }
}

/// See [`activity_pill`].
pub struct ActivityPill<C> {
    key: Option<String>,
    label: String,
    state: WorkState,
    press: Option<Press<C>>,
}

/// A small capsule above the bottom bars while work runs in the
/// background: "Syncing 3 notes", "2 agents working". It never covers
/// content; show it only while there is something running.
pub fn activity_pill<C>(label: impl Into<String>) -> ActivityPill<C> {
    ActivityPill {
        key: None,
        label: label.into(),
        state: WorkState::Working(None),
        press: None,
    }
}
impl<C> ActivityPill<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Working by default; Waiting when something needs an answer.
    pub fn state(mut self, state: WorkState) -> Self {
        self.state = state;
        self
    }
    /// Open what's running.
    pub fn on_press(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.press = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> ActivityPill<C> {
    pub(super) fn pill(self) -> Option<Element<C>> {
        let key = self.key.filter(|k| !k.is_empty())?;
        let mut p = hstack![
            mark(&self.state, 24.),
            text(self.label.clone()).font_size(15.).font_weight(600),
        ]
        .key(key)
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .height(40.)
        .padding(8., Edge::Left)
        .padding(16., Edge::Right)
        .rounded(Rounding::Full)
        .background("@base-200|@base-300")
        .shadow(
            crate::declarative::effects::Shadow::new()
                .color("#0000001F|#00000066")
                .blur(16.)
                .offset(0., 4.),
        )
        .accessible_label(self.label);
        if let Some(press) = self.press {
            p = p
                .pressed_style(|s| s.opacity(0.8))
                .on_click_with(move |c: &mut C, _| (press.borrow_mut())(c));
            p.0.node.access_role = Some(AccessRole::Button);
        }
        let mut e: Element<C> = p.into();
        e.node.effects.transition =
            Some(crate::declarative::Transition::slide(0., 16.).duration_ms(240));
        Some(e)
    }
}
impl<C: 'static> From<ActivityPill<C>> for Element<C> {
    fn from(a: ActivityPill<C>) -> Self {
        let key = a.key.clone().unwrap_or_default();
        let mut parts: Vec<Element<C>> = vec![spacer().into()];
        if let Some(pill) = a.pill() {
            parts.push(hstack![pill].justify(Justify::Center).into());
        }
        let mut foot = vstack(Vec::<Element<C>>::new())
            .key(format!("{key}-foot"))
            .height(12.);
        foot.0.node.ext.above_bars = true;
        parts.push(foot.into());
        vstack(parts)
            .key(format!("{key}-layer"))
            .width_fraction(1.)
            .height_fraction(1.)
            .scroll_passthrough(true)
            .into()
    }
}
