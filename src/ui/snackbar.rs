//! A short message at the foot of the screen, with one action: most often
//! Undo after something was deleted. It doesn't block the page.
//!
//! Android draws Material 3's snackbar; iOS has no system one, so it is a
//! floating capsule above the bottom bars, as iOS apps show it.
use super::*;
use crate::declarative::AccessRole;
use crate::declarative::Transition;
use std::{cell::RefCell, rc::Rc};

type Press<C> = Rc<RefCell<dyn FnMut(&mut C)>>;

/// See [`snackbar`].
pub struct Snackbar<C> {
    key: Option<String>,
    message: String,
    action: Option<(String, Press<C>)>,
    open: bool,
    close: Option<Press<C>>,
    close_after: Option<f32>,
}

/// A message with an optional action, such as "Deleted 3 notes" and Undo.
/// Put it last in a `zstack!` over the page, or give it to an action dock
/// with `notice`. Closing it after a few seconds is the app's: use an
/// `Updater` from a timer thread.
///
/// ```rust,ignore
/// snackbar(format!("Deleted {}.", note.title))
///     .key("undo")
///     .action("Undo", |a: &mut App| a.undo())
///     .open(app.undo.is_some())
///     .on_close(|a: &mut App| a.undo = None)
/// ```
pub fn snackbar<C>(message: impl Into<String>) -> Snackbar<C> {
    Snackbar {
        key: None,
        message: message.into(),
        action: None,
        open: false,
        close: None,
        close_after: None,
    }
}
impl<C> Snackbar<C> {
    /// Required. The action is keyed `{key}-action`, the close `{key}-close`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// One action, a word or two: "Undo", "View".
    pub fn action(mut self, label: impl Into<String>, f: impl FnMut(&mut C) + 'static) -> Self {
        self.action = Some((label.into(), Rc::new(RefCell::new(f))));
        self
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    /// Close it after `seconds` on screen (4 to 10 is usual; longer with an
    /// action). On phones the time stops while a finger is on the screen
    /// and while the app is in the background, and with a screen reader on
    /// it stays until dismissed. Without this, closing is the app's.
    pub fn close_after(mut self, seconds: f32) -> Self {
        self.close_after = Some(seconds);
        self
    }
    /// Required: close it (app state). Its action closes it too.
    pub fn on_close(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.close = Some(Rc::new(RefCell::new(f)));
        self
    }
}

impl<C: 'static> Snackbar<C> {
    /// The bar alone, without the layer that places it.
    pub(super) fn bar(self) -> Option<Element<C>> {
        let key = self.key.filter(|k| !k.is_empty())?;
        let close = self.close?;
        if !self.open {
            return None;
        }
        // Material 3 puts it on the inverse surface; iOS on a raised one.
        let android = cfg!(target_os = "android");
        let (surface, ink, accent): (String, ColorToken, ColorToken) = if android {
            (
                ColorToken::BaseContent.into(),
                ColorToken::Base100,
                ColorToken::Primary,
            )
        } else {
            (
                "@base-200|@base-300".into(),
                ColorToken::BaseContent,
                ColorToken::Primary,
            )
        };
        let mut row: Vec<Element<C>> = vec![text(self.message.clone())
            .font_size(if android { 14. } else { 15. })
            .color(ink)
            .flex(1.)
            .into()];
        if let Some((label, press)) = self.action {
            let shut = close.clone();
            let mut b = hstack![text(label.clone())
                .font_size(if android { 14. } else { 15. })
                .font_weight(600)
                .color(accent)]
            .key(format!("{key}-action"))
            .height(36.)
            .padding(12., Edge::Horizontal)
            .align(Align::Center)
            .rounded(Rounding::Full)
            .accessible_label(label)
            .pressed_style(|s| s.background(ColorToken::Primary.alpha(24)))
            // The action first: closing may let go of what it needs (Undo
            // needs what was deleted).
            .on_click_with(move |c: &mut C, _| {
                (press.borrow_mut())(c);
                (shut.borrow_mut())(c);
            });
            b.0.node.access_role = Some(AccessRole::Button);
            row.push(b.into());
        }
        let shut = close;
        let mut x = zstack![icon(Icon::Close).size(16.).stroke(2.).color(if android {
            ColorToken::Base100
        } else {
            ColorToken::Muted
        })]
        .key(format!("{key}-close"))
        .width(36.)
        .height(36.)
        .flow(Flow::Center)
        .rounded(Rounding::Full)
        .accessible_label("Dismiss")
        .on_click_with(move |c: &mut C, _| (shut.borrow_mut())(c));
        x.0.node.access_role = Some(AccessRole::Button);
        row.push(x.into());
        let mut card = hstack(row)
            .key(key)
            .gap(4., Axis::Horizontal)
            .align(Align::Center)
            .padding(16., Edge::Left)
            .padding(6., Edge::Right)
            .height(if android { 48. } else { 52. })
            .rounded(if android { 4. } else { 26. })
            .background(surface)
            .shadow(
                crate::declarative::effects::Shadow::new()
                    .color("#00000026|#00000066")
                    .blur(20.)
                    .offset(0., 6.),
            );
        if let Some(s) = self.close_after.filter(|s| s.is_finite() && *s > 0.) {
            use std::hash::{Hash, Hasher};
            let mut stamp = std::collections::hash_map::DefaultHasher::new();
            self.message.hash(&mut stamp);
            card.0.node.ext.close_after_ms =
                Some(((s * 1000.).min(u32::MAX as f32) as u32, stamp.finish()));
        }
        let mut card: Element<C> = card.into();
        // It rises in from below and fades as it goes.
        card.node.effects.transition = Some(Transition::slide(0., 24.).duration_ms(260));
        Some(card)
    }
}

impl<C: 'static> From<Snackbar<C>> for Element<C> {
    fn from(s: Snackbar<C>) -> Self {
        let key = s.key.clone().unwrap_or_default();
        if key.is_empty() || s.close.is_none() {
            let mut e = Element::new(Kind::Column);
            e.validation_error = Some("snackbar requires a key and on_close".into());
            return e;
        }
        let mut parts: Vec<Element<C>> = vec![spacer().into()];
        if let Some(bar) = s.bar() {
            parts.push(
                hstack![bar.flex_into()]
                    .padding(12., Edge::Horizontal)
                    .into(),
            );
        }
        // Just above the tab bar, home indicator or keyboard.
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

trait FlexInto<C> {
    fn flex_into(self) -> Element<C>;
}
impl<C> FlexInto<C> for Element<C> {
    /// Fill the row's width.
    fn flex_into(mut self) -> Element<C> {
        self.node.flex = 1.;
        self
    }
}
