//! A bottom sheet, designed from Apple's and Material's guidance:
//!
//! - It rests at heights you choose (detents): about half the screen, nearly
//!   all of it, or an exact height. A grabber shows it can be resized; drag
//!   it, or tap it to step through the heights.
//! - Swipe it down to close, as people expect. If closing would lose work,
//!   `on_dismiss_attempt` runs instead, so the app can ask first.
//! - Only the top corners are rounded and there's no border: the sheet sits
//!   on the screen's bottom edge. It is never wider than 640 points, and the
//!   page above it stays visible under a scrim.
//! - Cancel sits at the leading edge and Done at the trailing edge, as iOS
//!   puts them. Without either, a Close button keeps a way out that isn't a
//!   gesture.
use super::dialogs::{overlay_scope, overlay_slot, with_motion};
use super::*;
use crate::declarative::document::OverlayKind;
use std::{cell::RefCell, rc::Rc};

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}

/// A height a sheet rests at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SheetDetent {
    /// About half the screen, for a first look; drag up for more.
    Medium,
    /// The full height, below a margin that keeps the page above in view.
    Large,
    /// An exact height in points.
    Points(f32),
}
/// Space kept above a sheet at its largest, so the page behind shows.
const TOP_MARGIN: f32 = 56.;
const MAX_WIDTH: f32 = 640.;
const RADIUS: f32 = 28.;
/// How far, or how fast, a drag must go to change the sheet's height.
const STEP_DISTANCE: f32 = 96.;
const STEP_SPEED: f32 = 700.;

type Act<C> = Rc<RefCell<dyn FnMut(&mut C)>>;
type Rest<C> = Rc<RefCell<dyn FnMut(&mut C, usize)>>;
/// See [`bottom_sheet`].
pub struct BottomSheet<C> {
    key: Option<String>,
    content: Element<C>,
    title: Option<String>,
    open: bool,
    motion: bool,
    detents: Vec<SheetDetent>,
    detent: usize,
    on_detent: Option<Rest<C>>,
    grabber: Option<bool>,
    on_dismiss: Option<Act<C>>,
    on_attempt: Option<Act<C>>,
    cancel: Option<(String, Act<C>)>,
    done: Option<(String, Act<C>, bool)>,
}
/// A sheet that rises from the bottom of the screen for a task related to
/// the page behind it. Requires a key, `open` and `on_dismiss`.
///
/// ```rust,ignore
/// bottom_sheet(new_idea_form(app))
///     .key("new-idea")
///     .title("New idea")
///     .detents([SheetDetent::Medium, SheetDetent::Large])
///     .detent(app.sheet_height)
///     .on_detent(|a: &mut App, i| a.sheet_height = i)
///     .cancel("Cancel", |a: &mut App| a.composing = false)
///     .done("Start", |a: &mut App| a.start_idea())
///     .done_enabled(!app.draft.is_empty())
///     .open(app.composing)
///     .on_dismiss(|a: &mut App| a.composing = false)
/// ```
pub fn bottom_sheet<C>(content: impl Into<Element<C>>) -> BottomSheet<C> {
    BottomSheet {
        key: None,
        content: content.into(),
        title: None,
        open: false,
        motion: true,
        detents: vec![SheetDetent::Medium],
        detent: 0,
        on_detent: None,
        grabber: None,
        on_dismiss: None,
        on_attempt: None,
        cancel: None,
        done: None,
    }
}
impl<C> BottomSheet<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = Some(value.into());
        self
    }
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }
    /// Built-in motion (default on). Reduce Motion is followed either way.
    pub fn motion(mut self, value: bool) -> Self {
        self.motion = value;
        self
    }
    /// The heights it rests at, smallest first. Default: medium only.
    pub fn detents(mut self, detents: impl IntoIterator<Item = SheetDetent>) -> Self {
        self.detents = detents.into_iter().collect();
        self
    }
    /// The height it rests at now, as an index into `detents`.
    pub fn detent(mut self, index: usize) -> Self {
        self.detent = index;
        self
    }
    /// A drag or a tap on the grabber chose another height.
    pub fn on_detent(mut self, f: impl FnMut(&mut C, usize) + 'static) -> Self {
        self.on_detent = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Show the grabber. Default: when there's more than one height.
    pub fn grabber(mut self, value: bool) -> Self {
        self.grabber = Some(value);
        self
    }
    /// A fixed height in points: the same as `detents([Points(h)])`.
    pub fn size(self, height: f32) -> Self {
        self.detents([SheetDetent::Points(height)])
    }
    /// Kept for older code; sheets use the platform's corner radius.
    pub fn corner_radius(self, _pixels: f32) -> Self {
        self
    }
    /// Swipe down, a tap on the scrim, Escape, Close or Cancel. Set
    /// `open = false`.
    pub fn on_dismiss(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Closing would lose something: swipes, the scrim and Escape call this
    /// instead of `on_dismiss`, so the app can confirm (with an action sheet)
    /// first. Cancel still calls its own action.
    pub fn on_dismiss_attempt(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.on_attempt = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// A Cancel button at the leading edge, which leaves without saving.
    pub fn cancel(mut self, label: impl Into<String>, f: impl FnMut(&mut C) + 'static) -> Self {
        self.cancel = Some((label.into(), Rc::new(RefCell::new(f))));
        self
    }
    /// A Done button at the trailing edge, which finishes the task. Pair it
    /// with Cancel so finishing isn't the only way out.
    pub fn done(mut self, label: impl Into<String>, f: impl FnMut(&mut C) + 'static) -> Self {
        self.done = Some((label.into(), Rc::new(RefCell::new(f)), true));
        self
    }
    /// Whether Done can be pressed yet (the task is complete).
    pub fn done_enabled(mut self, value: bool) -> Self {
        if let Some(done) = &mut self.done {
            done.2 = value;
        }
        self
    }
}

fn sized<C>(card: Container<C>, detent: SheetDetent) -> Container<C> {
    match detent {
        SheetDetent::Medium => card.height_fraction(0.5),
        SheetDetent::Large => card.height_fraction(1.),
        SheetDetent::Points(h) => card.height(h),
    }
}

impl<C: 'static> From<BottomSheet<C>> for Element<C> {
    fn from(s: BottomSheet<C>) -> Self {
        let Some(key) = s.key.filter(|k| !k.is_empty()) else {
            return invalid("bottom_sheet requires a key");
        };
        let slot = Some(format!("{key}-slot"));
        if !s.open {
            return overlay_slot(slot, None);
        }
        let Some(dismiss) = s.on_dismiss else {
            return invalid("open bottom_sheet requires on_dismiss");
        };
        if s.detents.is_empty() {
            return invalid("bottom_sheet requires at least one detent");
        }
        let count = s.detents.len();
        let current = s.detent.min(count - 1);
        // Swipes, the scrim and Escape ask first when the app wants them to.
        let leave = s.on_attempt.clone().unwrap_or_else(|| dismiss.clone());

        let mut head: Vec<Element<C>> = Vec::new();
        let title = s.title.clone().unwrap_or_default();
        let has_actions = s.cancel.is_some() || s.done.is_some();
        if let Some((label, f)) = s.cancel {
            head.push(
                button(label)
                    .key(format!("{key}-cancel"))
                    .font_size(17.)
                    .variant(ButtonVariant::Ghost)
                    .color(ColorToken::Primary)
                    .on_click(move |c: &mut C| (f.borrow_mut())(c))
                    .into(),
            );
        }
        if has_actions {
            head.push(spacer().into());
            head.push(
                text(title)
                    .key(format!("{key}-title"))
                    .font_size(17.)
                    .font_weight(600)
                    .into(),
            );
            head.push(spacer().into());
        } else {
            head.push(
                text(title)
                    .key(format!("{key}-title"))
                    .font_size(20.)
                    .font_weight(700)
                    .flex(1.)
                    .into(),
            );
        }
        if let Some((label, f, enabled)) = s.done {
            head.push(
                button(label)
                    .key(format!("{key}-done"))
                    .font_size(17.)
                    .variant(ButtonVariant::Ghost)
                    .color(ColorToken::Primary)
                    .font_weight(600)
                    .enabled(enabled)
                    .on_click(move |c: &mut C| (f.borrow_mut())(c))
                    .into(),
            );
        } else if !has_actions {
            let close = dismiss.clone();
            head.push(
                zstack![
                    button("")
                        .key(format!("{key}-close"))
                        .accessible_label("Close")
                        .variant(ButtonVariant::Ghost)
                        .rounded(Rounding::Full)
                        .background(ColorToken::Base300)
                        .width(30.)
                        .height(30.)
                        .on_click(move |c: &mut C| (close.borrow_mut())(c)),
                    icon(Icon::Close)
                        .size(14.)
                        .stroke(2.4)
                        .color(ColorToken::Muted),
                ]
                .key(format!("{key}-close-layer"))
                .width(30.)
                .height(30.)
                .flow(Flow::Center)
                .into(),
            );
        }

        let mut parts: Vec<Element<C>> = Vec::new();
        let grabber = s.grabber.unwrap_or(count > 1);
        if grabber {
            // Part of the sheet's drag surface: drag it to resize, or tap it
            // to step to the next height, as on iOS.
            parts.push(
                zstack![vstack(Vec::<Element<C>>::new())
                    .width(36.)
                    .height(5.)
                    .rounded(Rounding::Full)
                    .background(ColorToken::Muted)
                    .background_opacity(0.45)]
                .key(format!("{key}-grabber"))
                .accessible_label("Resize")
                .height(20.)
                .flow(Flow::Center)
                .into(),
            );
        }
        parts.push(
            hstack(head)
                .key(format!("{key}-head"))
                .height(44.)
                .padding(if has_actions { 4. } else { 20. }, Edge::Horizontal)
                .padding(if has_actions { 4. } else { 14. }, Edge::Right)
                .align(Align::Center)
                .gap(8., Axis::Horizontal)
                .into(),
        );
        parts.push(
            vstack![s.content]
                .key(format!("{key}-body"))
                .padding(20., Edge::Horizontal)
                .padding(8., Edge::Top)
                .padding(24., Edge::Bottom)
                .flex(1.)
                .scroll(Scroll::Auto)
                .into(),
        );

        // The sheet follows the finger and springs back; where it settles
        // is decided from how far and how fast it moved.
        let (step, close) = (s.on_detent.clone(), leave.clone());
        let card = vstack(parts)
            .key(format!("{key}-content"))
            .width_fraction(1.)
            .max_width(MAX_WIDTH)
            .padding(4., Edge::Top)
            // Light: the base surface, which fields and rows stand out on.
            // Dark: a raised surface, so the sheet stands out from the page
            // behind it (pure black on black reads as a hole), with fields
            // still a step darker inside it.
            .background("@base-100|@base-300")
            .rounded_on(RADIUS, Corners::Top)
            .shadow(tokens::shadow::xl())
            .layout_animation(Animation::spring())
            .drag_motion(
                crate::motion::DragMotion::new()
                    .bounds([0., -48.], [0., 4096.])
                    .momentum(false)
                    .spring_back(true),
            )
            .on_pointer(move |c: &mut C, p: Pointer| {
                if p.phase != PointerPhase::Release {
                    return;
                }
                let [_, dy] = p.offset;
                // A tap on the grabber steps to the next height.
                if grabber && dy.abs() < 6. && p.at[1] < 28. {
                    if let Some(f) = &step {
                        (f.borrow_mut())(c, (current + 1) % count);
                    }
                    return;
                }
                let [_, vy] = p.velocity;
                let down = dy > STEP_DISTANCE || (vy > STEP_SPEED && dy > 8.);
                let up = dy < -STEP_DISTANCE / 2. || (vy < -STEP_SPEED && dy < -8.);
                if down {
                    if current == 0 {
                        (close.borrow_mut())(c);
                    } else if let Some(f) = &step {
                        (f.borrow_mut())(c, current - 1);
                    }
                } else if up && current + 1 < count {
                    if let Some(f) = &step {
                        (f.borrow_mut())(c, current + 1);
                    }
                }
            });
        let mut card = sized(card, s.detents[current]);
        card.0.node.ext.keyboard_room = true;
        let card = with_motion(
            card,
            s.motion,
            Transition::slide(0., 320.).animation(Animation::spring()),
        );
        let mut root = overlay_scope(
            card,
            key,
            OverlayKind::Modal,
            Box::new(move |c: &mut C| (leave.borrow_mut())(c)),
        );
        root.node.flow = Some(Flow::Bottom);
        root.node.align = Align::Center;
        root.node.padding_top = Some(TOP_MARGIN);
        overlay_slot(slot, Some(root))
    }
}
