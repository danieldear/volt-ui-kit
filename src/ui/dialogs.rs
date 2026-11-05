//! Controlled in-window overlays. These are not operating-system dialogs.
use super::*;
use crate::declarative::document::{Overlay, OverlayKind};

/// A modal card rendered above the window's ordinary content.
/// Put dialogs in the tree after the content they cover; their children are only
/// mounted while `open` is true. The application owns open/closed state.
pub struct Dialog<C>(pub(super) Element<C>, DialogOptions<C>);
struct DialogOptions<C> {
    open: bool,
    title: Option<String>,
    dismiss: Option<Click<C>>,
    backdrop_dismiss: bool,
    escape_dismiss: bool,
    motion: bool,
}
/// Compose a controlled modal. Its width/styles apply to the card, not backdrop.
pub fn dialog<C>(content: impl Into<Element<C>>) -> Dialog<C> {
    let card = panel([content.into()])
        .width(420.)
        .padding(24., Edge::All)
        .gap(12., Axis::Vertical)
        .scroll(Scroll::Auto)
        .rounded(16)
        .background(ColorToken::Base200)
        .border_color(ColorToken::Border)
        .border_width(1.);
    Dialog(
        card.into(),
        DialogOptions {
            open: false,
            title: None,
            dismiss: None,
            backdrop_dismiss: true,
            escape_dismiss: true,
            motion: true,
        },
    )
}
impl<C> Dialog<C> {
    pub fn open(mut self, open: bool) -> Self {
        self.1.open = open;
        self
    }
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.1.title = Some(title.into());
        self
    }
    /// Called for an allowed Escape or press/release on the backdrop. It must
    /// update application state (usually set `open = false`) for recomposition.
    pub fn on_dismiss(mut self, dismiss: impl FnMut(&mut C) + 'static) -> Self {
        self.1.dismiss = Some(Box::new(dismiss));
        self
    }
    pub fn backdrop_dismiss(mut self, dismiss: bool) -> Self {
        self.1.backdrop_dismiss = dismiss;
        self
    }
    pub fn escape_dismiss(mut self, dismiss: bool) -> Self {
        self.1.escape_dismiss = dismiss;
        self
    }
    /// Built-in motion (default on): the backdrop and card fade in while the
    /// card rises into place, and both fade out on close. `false` opens and
    /// closes instantly. `.transition(..)` replaces the card's rise.
    /// `Motion::None` on the application turns it off everywhere.
    pub fn motion(mut self, value: bool) -> Self {
        self.1.motion = value;
        self
    }
}
impl<C> From<Dialog<C>> for Element<C> {
    fn from(mut dialog: Dialog<C>) -> Self {
        let key = (!dialog.0.auto_id).then(|| dialog.0.node.id.clone());
        let slot = key.as_ref().map(|key| format!("{key}-slot"));
        if !dialog.1.open {
            return overlay_slot(slot, None);
        }
        if let Some(title) = dialog.1.title {
            dialog
                .0
                .children
                .insert(0, text(title).font_size(20.).font_weight(600).into());
        }
        if let Some(key) = &key {
            dialog.0.node.id = format!("{key}-content");
        }
        if dialog.1.motion && dialog.0.node.effects.transition.is_none() {
            dialog.0.node.effects.transition = Some(card_rise());
        }
        let mut root = scope(dialog.0, OverlayKind::Modal, dialog.1.dismiss);
        root.node.overlay.as_mut().unwrap().backdrop_dismiss = dialog.1.backdrop_dismiss;
        root.node.overlay.as_mut().unwrap().escape_dismiss = dialog.1.escape_dismiss;
        if dialog.1.motion {
            root.node.effects.transition = Some(overlay_fade());
        }
        if let Some(key) = key {
            root.node.id = key;
            root.auto_id = false;
        }
        overlay_slot(slot, Some(root))
    }
}
/// Built-in overlay motion: the whole portal fades, backdrop included. Its
/// departing copy then paints last and unclipped, like the live portal.
pub(super) fn overlay_fade() -> Transition {
    Transition::fade().animation(Animation::ease_out().duration_ms(tokens::motion::BASE))
}
/// A modal card rising 8 logical pixels into place as it fades in.
pub(super) fn card_rise() -> Transition {
    Transition::slide(0., 8.).animation(
        Animation::ease_out()
            .easing(tokens::motion::EMPHASIZED)
            .duration_ms(tokens::motion::MEDIUM),
    )
}
/// Anchored popups fade while settling 4 logical pixels down into place.
pub(super) fn popup_motion() -> Transition {
    Transition::slide(0., -4.).animation(Animation::ease_out().duration_ms(tokens::motion::BASE))
}
/// Attach a component's built-in transition unless `.motion(false)` was set.
pub(super) fn with_motion<C>(
    element: impl Into<Element<C>>,
    motion: bool,
    transition: Transition,
) -> Element<C> {
    let mut element = element.into();
    if motion {
        element.node.effects.transition = Some(transition);
    }
    element
}
/// A zero-size slot that is always in the tree and holds the overlay root
/// only while open. The root is then new on open and removed on close, so
/// its transition plays both ways whether or not the component has a key.
pub(super) fn overlay_slot<C>(key: Option<String>, root: Option<Element<C>>) -> Element<C> {
    let mut slot = Element::new(Kind::ZStack);
    slot.node.width = Some(0.);
    slot.node.height = Some(0.);
    slot.node.effects.scroll_passthrough = true;
    if let Some(key) = key {
        slot.node.id = key;
        slot.auto_id = false;
    }
    slot.children = root.into_iter().collect();
    slot
}
fn scope<C>(content: Element<C>, kind: OverlayKind, dismiss: Option<Click<C>>) -> Element<C> {
    let mut root = Element::new(Kind::ZStack);
    root.node.overlay = Some(Overlay {
        kind,
        dismiss_action: None,
        backdrop_dismiss: true,
        escape_dismiss: true,
        anchor: None,
        position: None,
        keep_anchor: false,
    });
    root.node.style.background = Some(
        match kind {
            OverlayKind::Modal => "#00000080",
            OverlayKind::Popup | OverlayKind::Tooltip => "#00000000",
        }
        .into(),
    );
    root.node.flow = Some(Flow::Center);
    root.children = vec![content];
    root.callback = dismiss.map(Callback::Dismiss);
    if root.callback.is_none() {
        root.validation_error = Some("open overlay requires on_dismiss".into());
    }
    root
}
/// Shared portal composition for controlled selection popups.
pub(super) fn overlay_scope<C>(
    content: Element<C>,
    key: String,
    kind: OverlayKind,
    dismiss: Click<C>,
) -> Element<C> {
    let mut root = scope(content, kind, Some(dismiss));
    root.node.id = key;
    root.auto_id = false;
    root
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    /// Whether each overlay root under `element` carries a transition.
    fn overlay_motion<C>(element: &Element<C>) -> Vec<bool> {
        let mut found = Vec::new();
        if element.node.overlay.is_some() {
            found.push(element.node.effects.transition.is_some());
        }
        for child in &element.children {
            found.extend(overlay_motion(child));
        }
        found
    }
    fn dialog_with(motion: bool, key: bool) -> Element<()> {
        let d = dialog(text("Body")).open(true).motion(motion);
        let d = if key { d.key("d") } else { d };
        d.on_dismiss(|_| {}).into()
    }
    #[test]
    fn overlays_ship_built_in_motion_that_each_can_switch_off() {
        let items = || [crate::ui::menu_item(1usize, "Save")];
        let menu = |motion| -> Element<()> {
            crate::ui::menu("Actions", items())
                .key("m")
                .open(true)
                .motion(motion)
                .on_action(|_, _| {})
                .on_open_change(|_, _| {})
                .into()
        };
        let context = |motion| -> Element<()> {
            crate::ui::context_menu(text("Target"), items())
                .key("c")
                .open(true)
                .motion(motion)
                .on_action(|_, _| {})
                .on_open_change(|_, _| {})
                .on_context_open(|_, _| {})
                .into()
        };
        let select = |motion| -> Element<()> {
            crate::ui::select([crate::ui::select_item(0, "Zero")])
                .key("s")
                .open(true)
                .motion(motion)
                .on_select(|_, _| {})
                .on_open_change(|_, _| {})
                .into()
        };
        let popover = |motion| -> Element<()> {
            crate::ui::popover(text("Pop"))
                .key("p")
                .anchor("opener")
                .open(true)
                .motion(motion)
                .on_dismiss(|_| {})
                .into()
        };
        let tooltip = |motion| -> Element<()> {
            crate::ui::tooltip("Hint", text("Target"))
                .key("t")
                .motion(motion)
                .into()
        };
        for build in [
            &menu as &dyn Fn(bool) -> Element<()>,
            &context,
            &select,
            &popover,
            &tooltip,
        ] {
            assert_eq!(overlay_motion(&build(true)), [true]);
            assert_eq!(overlay_motion(&build(false)), [false]);
        }
        for key in [true, false] {
            assert_eq!(overlay_motion(&dialog_with(true, key)), [true]);
            assert_eq!(overlay_motion(&dialog_with(false, key)), [false]);
        }
    }
    #[test]
    fn dialog_card_rises_unless_the_app_sets_its_own_transition() {
        let card = |e: &Element<()>| e.children[0].children[0].node.effects.transition;
        assert_eq!(card(&dialog_with(true, true)), Some(card_rise()));
        assert_eq!(card(&dialog_with(false, true)), None);
        let own = Transition::fade().duration_ms(90);
        let custom: Element<()> = dialog(text("Body"))
            .key("d")
            .open(true)
            .transition(own)
            .on_dismiss(|_| {})
            .into();
        assert_eq!(card(&custom), Some(own));
    }
    #[test]
    fn overlays_live_in_a_stable_slot_so_the_root_is_new_on_open() {
        let closed: Element<()> = dialog(text("Body")).key("d").into();
        assert_eq!(closed.node.id, "d-slot");
        assert!(closed.children.is_empty());
        let open = dialog_with(true, true);
        assert_eq!(open.node.id, "d-slot");
        assert_eq!(open.children[0].node.id, "d");
        assert_eq!(open.children[0].children[0].node.id, "d-content");
        let closed: Element<()> = crate::ui::popover(text("Pop")).key("p").into();
        assert_eq!(closed.node.id, "p-slot");
        assert!(closed.children.is_empty());
        let closed: Element<()> = crate::ui::drawer(text("Body")).key("dr").into();
        assert_eq!(closed.node.id, "dr-slot");
        assert!(closed.children.is_empty());
        let open: Element<()> = crate::ui::drawer(text("Body"))
            .key("dr")
            .open(true)
            .on_dismiss(|_| {})
            .into();
        assert_eq!(open.node.id, "dr-slot");
        assert_eq!(open.children[0].node.id, "dr");
    }
}
