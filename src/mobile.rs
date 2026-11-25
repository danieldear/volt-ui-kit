//! Experimental, platform-owned touch view. No windowing or OS dependencies.
//!
//! Coordinates and sizes are logical points; the renderer uses physical pixels.
//! A platform host owns safe-area/keyboard insets and its GPU surface. This first
//! host supports single-finger tap/drag scrolling, not inertia or multi-touch.
//!
//! Motion starts reduced. A host with a frame loop calls `set_reduced_motion`
//! with the system setting, then draws while `is_animating` is true.
use crate::{
    declarative::{AccessNode, AccessValue, Bindings, Handlers, Runtime},
    rendering::{PresentStatus, RasterPainter, SurfaceRenderer},
    ui::Element,
    UiTheme, Viewport,
};

struct Touch {
    start: [f32; 2],
    last: [f32; 2],
    scrolling: bool,
}

/// One Rust UI mounted inside a platform-owned view. Call only on its UI thread.
/// The view function and callbacks own no borrowed application state.
pub struct MobileView<C: 'static> {
    context: C,
    view: fn(&C) -> Element<C>,
    runtime: Runtime,
    handlers: Handlers<'static, C>,
    painter: RasterPainter,
    viewport: Viewport,
    touch: Option<Touch>,
    suspended: bool,
    host_font: Option<String>,
    /// Monotonic animation clock, advanced on input and before each draw.
    clock: std::time::Instant,
    /// The host draws navigation bars; Volt's copies are hidden.
    native_navigation: bool,
    /// Safe area insets the host reported, for overlays.
    safe_area: [f32; 4],
    /// Points the host's bottom bars cover: tab bar, home indicator, keyboard.
    bars_bottom: f32,
    /// Points the keyboard covers, on its own.
    keyboard: f32,
    /// Reporting scroll ends: the recompose it causes doesn't report again.
    reporting_ends: bool,
    /// How long each self-closing message has been on screen, unpaused.
    /// With the stamp of the message being timed.
    shown: std::collections::BTreeMap<String, (u64, std::time::Duration)>,
    /// The clock when message timers last advanced.
    timers_at: std::time::Duration,
    /// A screen reader is on: timed messages stay until dismissed.
    screen_reader: bool,
}
impl<C: 'static> MobileView<C> {
    pub fn new(context: C, view: fn(&C) -> Element<C>) -> Result<Self, String> {
        Self::create(context, view, None)
    }
    /// Supply a platform font fallback without overriding an app's explicit root font.
    pub fn new_with_font(
        context: C,
        view: fn(&C) -> Element<C>,
        family: impl Into<String>,
    ) -> Result<Self, String> {
        Self::create(context, view, Some(family.into()))
    }
    fn create(
        context: C,
        view: fn(&C) -> Element<C>,
        host_font: Option<String>,
    ) -> Result<Self, String> {
        let element = view(&context);
        let element = if let Some(family) = &host_font {
            element.host_font(family)
        } else {
            element
        };
        let (doc, handlers) = element.build().map_err(|e| e.to_string())?.into_parts();
        let runtime =
            Runtime::new(doc, &handlers, &Bindings::default()).map_err(|e| e.to_string())?;
        let viewport = Viewport::new(390., 760., 1.).unwrap();
        let mut value = Self {
            context,
            view,
            runtime,
            handlers,
            painter: RasterPainter::new(viewport),
            viewport,
            touch: None,
            suspended: false,
            reporting_ends: false,
            shown: Default::default(),
            timers_at: std::time::Duration::ZERO,
            screen_reader: false,
            host_font,
            clock: std::time::Instant::now(),
            native_navigation: false,
            safe_area: [0.; 4],
            bars_bottom: 0.,
            keyboard: 0.,
        };
        // Hosts without a frame loop keep motion reduced so nothing stops halfway.
        value.runtime.set_reduced_motion(true);
        value.layout()?;
        Ok(value)
    }
    pub fn context(&self) -> &C {
        &self.context
    }
    /// Follow the system's Reduce Motion setting. Only a host that keeps
    /// drawing while `is_animating` returns true should allow motion.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.runtime.set_reduced_motion(reduced);
    }
    /// Whether the host should draw another frame (press feedback, transitions).
    pub fn is_animating(&self) -> bool {
        !self.suspended && self.runtime.is_animating()
    }
    /// Let the platform's scroll view own scrolling: its momentum, bounce and
    /// indicators. Drags then only cancel presses; report positions with
    /// `scroll_to`.
    pub fn set_native_scrolling(&mut self, native: bool) -> Result<(), String> {
        self.runtime.set_native_scrolling(native);
        self.layout()
    }
    /// Let the platform draw navigation bars (title, back button, bar
    /// buttons). Volt's bars are hidden; run their controls with
    /// `activate_navigation`, and read them from `Runtime::navigation_bars`.
    pub fn set_native_navigation(&mut self, native: bool) -> Result<(), String> {
        if self.native_navigation != native {
            self.native_navigation = native;
            self.recompose()?;
        }
        Ok(())
    }
    /// The screen's safe area, [top, right, bottom, left] in logical points,
    /// when the view runs under the status bar and home indicator. Sheets,
    /// dialogs and menus stay inside it.
    /// How far the host's own bars (tab bar, home indicator, keyboard) reach
    /// up from the bottom, in points. An action dock sits just above them.
    /// `keyboard` is the keyboard's part alone: sheets rise above it.
    pub fn set_bars(&mut self, bottom: f32, keyboard: f32) -> Result<(), String> {
        if ![bottom, keyboard]
            .iter()
            .all(|v| v.is_finite() && (0. ..=2048.).contains(v))
        {
            return Err("bottom bars must be 0 to 2048 points".into());
        }
        if self.bars_bottom != bottom || self.keyboard != keyboard {
            self.bars_bottom = bottom;
            self.keyboard = keyboard;
            self.recompose()?;
        }
        Ok(())
    }
    pub fn set_safe_area(&mut self, insets: [f32; 4]) -> Result<(), String> {
        if !insets
            .iter()
            .all(|v| v.is_finite() && (0. ..=512.).contains(v))
        {
            return Err("safe area insets must be 0 to 512 points".into());
        }
        if self.safe_area != insets {
            self.safe_area = insets;
            self.recompose()?;
        }
        Ok(())
    }
    /// Let the platform draw switches and sliders over their laid-out boxes.
    pub fn set_native_controls(&mut self, native: bool) {
        self.runtime.set_native_controls(native);
    }
    /// A platform slider's new value.
    pub fn set_value(&mut self, id: &str, value: f32) -> Result<bool, String> {
        if self.suspended || !value.is_finite() {
            return Ok(false);
        }
        self.tick();
        let o = self
            .runtime
            .accessibility_set_value(
                id,
                AccessValue::Number(value),
                &mut self.handlers,
                &mut self.context,
            )
            .map_err(|e| e.to_string())?;
        if o.activated {
            self.recompose()?;
        } else if o.changed {
            self.layout()?;
        }
        Ok(o.activated)
    }
    /// Run a navigation bar control (Back or a bar button) the platform drew.
    /// Pull to refresh: the host saw scroll view `id` pulled past its top and
    /// let go. Runs its `on_refresh`.
    pub fn refresh(&mut self, id: &str) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        self.tick();
        let refreshed = self
            .runtime
            .refresh(id, &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        if refreshed {
            self.recompose()?;
        }
        Ok(refreshed)
    }
    pub fn activate_navigation(&mut self, id: &str) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        self.tick();
        let activated = self
            .runtime
            .activate_navigation(id, &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        if activated {
            self.recompose()?;
        }
        Ok(activated)
    }
    /// Space a platform scroll view keeps around its content, as [top, right,
    /// bottom, left] logical points: the bars it scrolls under.
    pub fn set_scroll_insets(&mut self, id: &str, insets: [f32; 4]) -> Result<bool, String> {
        let changed = self.runtime.set_scroll_insets(id, insets);
        if changed {
            self.layout()?;
        }
        Ok(changed)
    }
    /// A platform scroll view's position for scroll view `id`, in logical
    /// points; past either end while it bounces.
    pub fn scroll_to(&mut self, id: &str, position: [f32; 2]) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        self.tick();
        let changed = self.runtime.set_scroll_position(id, position);
        if changed {
            self.layout()?;
        }
        Ok(changed)
    }
    /// Advance motion to now. Called on input and before drawing, never on a timer.
    fn tick(&mut self) -> bool {
        self.runtime.advance_animations(self.clock.elapsed())
    }
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
    /// Apply an external platform change (for example system appearance).
    pub fn update(&mut self, change: impl FnOnce(&mut C)) -> Result<(), String> {
        change(&mut self.context);
        self.recompose()
    }
    pub fn resize(&mut self, width: f32, height: f32, scale: f32) -> Result<(), String> {
        let next =
            Viewport::new(width * scale, height * scale, scale).ok_or("invalid mobile viewport")?;
        if next.width < 1.
            || next.height < 1.
            || next.width > 16384.
            || next.height > 16384.
            || next.width * next.height > 16_777_216.
            || !(0.25..=4.).contains(&next.scale)
        {
            return Err("mobile viewport exceeds bounds".into());
        }
        if self.viewport != next {
            self.cancel_touch();
            self.viewport = next;
            self.painter.set_viewport(next);
        }
        self.layout()?;
        // UIKit shrinks the viewport when its keyboard appears. Keep the
        // active native text proxy's Rust field reachable in that viewport.
        if let Some(id) = self
            .runtime
            .focused()
            .filter(|_| self.runtime.editing_focus())
            .map(str::to_owned)
        {
            if self.runtime.accessibility_reveal(&id) {
                self.layout()?;
            }
        }
        Ok(())
    }
    /// VoiceOver or TalkBack is on: self-closing messages wait to be
    /// dismissed, since nobody can say when the reader has finished them.
    pub fn set_screen_reader(&mut self, on: bool) {
        self.screen_reader = on;
    }
    /// Self-closing messages don't count time while a finger is on the
    /// screen, the app is in the background or a screen reader is on.
    fn timers_paused(&self) -> bool {
        self.touch.is_some() || self.suspended || self.screen_reader
    }
    /// Advance self-closing messages by the time since the last call (paused
    /// time doesn't count; call this before an event changes whether it's
    /// paused) and close the ones that are due. Returns whether any closed.
    pub fn run_timers(&mut self) -> Result<bool, String> {
        let now = self.clock.elapsed();
        let step = now.saturating_sub(self.timers_at);
        self.timers_at = now;
        let paused = self.timers_paused();
        let live = self.runtime.close_timers();
        self.shown
            .retain(|id, _| live.iter().any(|(l, _, _)| l == id));
        let mut due = Vec::new();
        for (id, ms, stamp) in live {
            // New ones, and new messages under the same key, start now.
            let seen = self.shown.get(&id).is_some_and(|(s, _)| *s == stamp);
            let (s, t) = self
                .shown
                .entry(id.clone())
                .or_insert((stamp, std::time::Duration::ZERO));
            if !seen {
                *s = stamp;
                *t = std::time::Duration::ZERO;
            } else if !paused {
                *t += step;
            }
            if *t >= std::time::Duration::from_millis(ms.into()) {
                due.push(id);
            }
        }
        let mut closed = false;
        for id in due {
            self.shown.remove(&id);
            closed |= self.activate(&format!("{id}-close"))?;
        }
        Ok(closed)
    }
    /// How long until the next self-closing message is due, if one is
    /// counting down: the host wakes then and calls `run_timers`.
    pub fn next_timer(&self) -> Option<std::time::Duration> {
        if self.timers_paused() {
            return None;
        }
        self.runtime
            .close_timers()
            .into_iter()
            .map(|(id, ms, stamp)| {
                let shown = self
                    .shown
                    .get(&id)
                    .filter(|(s, _)| *s == stamp)
                    .map_or(std::time::Duration::ZERO, |(_, t)| *t);
                std::time::Duration::from_millis(ms.into()).saturating_sub(shown)
            })
            .min()
    }
    fn layout(&mut self) -> Result<(), String> {
        self.runtime
            .layout(self.viewport, &Bindings::default(), &mut self.painter)
            .map_err(|e| e.to_string())?;
        // A transcript the reader scrolled away from (or back to) tells the
        // app, which shows or hides its "Jump to latest".
        if !self.reporting_ends {
            let ran = self
                .runtime
                .dispatch_scroll_ends(&mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            if ran {
                self.reporting_ends = true;
                let done = self.recompose();
                self.reporting_ends = false;
                done?;
            }
        }
        Ok(())
    }
    fn recompose(&mut self) -> Result<(), String> {
        let mut element = (self.view)(&self.context);
        if self.native_navigation {
            element.hide_navigation_bars();
        }
        if self.runtime.native_controls() {
            element.hide_native_parts();
        }
        if self.safe_area != [0.; 4] {
            element.inset_overlays(self.safe_area);
        }
        // Room under a dock or message box for what covers the canvas: the
        // host's bars and keyboard. Where Volt draws a tab bar (Android) it
        // already clears the system bar, and hides while the keyboard is up.
        let drawn_tabs = !self.native_navigation && element.has_tab_bar();
        if !self.native_navigation {
            element.inset_drawn_bars(self.safe_area, self.keyboard > 0.);
        }
        let bottom = if drawn_tabs {
            self.keyboard
        } else {
            self.bars_bottom
        };
        if bottom > 0. {
            element.inset_above_bars(bottom);
        }
        if self.keyboard > 0. {
            element.lift_sheets(self.keyboard);
        }
        let element = if let Some(family) = &self.host_font {
            element.host_font(family)
        } else {
            element
        };
        let (doc, handlers) = element.build().map_err(|e| e.to_string())?.into_parts();
        self.runtime
            .replace_document(doc, &handlers, &Bindings::default())
            .map_err(|e| e.to_string())?;
        self.handlers = handlers;
        self.layout()
    }
    fn point(&self, p: [f32; 2]) -> Option<[f32; 2]> {
        let p = p.map(|v| v * self.viewport.scale);
        p.iter().all(|v| v.is_finite()).then_some(p)
    }
    pub fn touch_begin(&mut self, point: [f32; 2]) -> Result<(), String> {
        self.cancel_touch();
        self.tick();
        if self.suspended {
            return Ok(());
        }
        let p = self.point(point).ok_or("invalid touch")?;
        self.runtime.pointer_move(p[0], p[1]);
        let outcome = self
            .runtime
            .pointer_down_outcome(p[0], p[1], &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        self.touch = Some(Touch {
            start: point,
            last: point,
            scrolling: false,
        });
        if outcome.activated {
            self.recompose()?;
        }
        Ok(())
    }
    pub fn touch_move(&mut self, point: [f32; 2]) -> Result<(), String> {
        if self.point(point).is_none() {
            self.cancel_touch();
            return Err("invalid touch".into());
        }
        self.tick();
        let Some(t) = self.touch.as_mut() else {
            return Ok(());
        };
        // A press on something draggable (a sheet, a row's swipe, a drawn
        // slider) drags it; anywhere else, moving past the slop scrolls.
        if self.runtime.pointer_dragging() {
            t.last = point;
            let p = point.map(|v| v * self.viewport.scale);
            let outcome = self
                .runtime
                .pointer_move_outcome(p[0], p[1], &mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            if outcome.activated {
                self.recompose()?;
            }
            return Ok(());
        }
        if !t.scrolling && (point[0] - t.start[0]).hypot(point[1] - t.start[1]) >= 8. {
            t.scrolling = true;
            self.runtime.cancel_input();
        }
        let delta = if t.scrolling {
            [t.last[0] - point[0], t.last[1] - point[1]]
        } else {
            [0.; 2]
        };
        t.last = point;
        let p = t.start.map(|v| v * self.viewport.scale);
        // A platform scroll view moves the content; a drag only ends the press.
        if t.scrolling && !self.runtime.native_scrolling() {
            let outcome = self
                .runtime
                .scroll_outcome(p[0], p[1], delta, &mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            if outcome.activated {
                self.recompose()?;
            } else if outcome.changed {
                self.layout()?;
            }
        }
        Ok(())
    }
    pub fn touch_end(&mut self, point: [f32; 2]) -> Result<(), String> {
        // Include the final movement; crossing the drag threshold at release
        // must never activate the row under the finger.
        self.touch_move(point)?;
        let Some(t) = self.touch.take() else {
            return Ok(());
        };
        if (!t.scrolling || self.runtime.pointer_dragging()) && !self.suspended {
            let p = self.point(point).ok_or("invalid touch")?;
            self.runtime.pointer_move(p[0], p[1]);
            let outcome = self
                .runtime
                .pointer_up_outcome(p[0], p[1], &mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            if outcome.activated {
                self.recompose()?;
            }
        }
        self.runtime.pointer_leave();
        Ok(())
    }
    /// The finger rested without moving: open the long-press menu under it,
    /// if there is one (a [`context_menu`](crate::ui::context_menu) or a
    /// row's hold actions). The touch ends here; true when a menu opened, so
    /// the host can play its haptic.
    pub fn hold(&mut self, point: [f32; 2]) -> Result<bool, String> {
        self.tick();
        if self.suspended || self.touch.as_ref().is_none_or(|t| t.scrolling) {
            return Ok(false);
        }
        let p = self.point(point).ok_or("invalid touch")?;
        let outcome = self
            .runtime
            .context_menu_at(p[0], p[1], &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        if !outcome.activated {
            return Ok(false);
        }
        self.touch = None;
        self.runtime.cancel_drag();
        self.runtime.pointer_leave();
        self.recompose()?;
        Ok(true)
    }
    /// The host's search key (or return) in field `id`: run its submit.
    pub fn submit(&mut self, id: &str) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        self.runtime.accessibility_focus(id);
        let outcome = self
            .runtime
            .editor_key(
                crate::declarative::EditKey::Submit,
                false,
                &mut self.handlers,
                &mut self.context,
            )
            .map_err(|e| e.to_string())?;
        if outcome.activated {
            self.recompose()?;
        }
        Ok(outcome.activated)
    }
    /// Open the long-press menu for `id` without a touch: a screen reader's
    /// "Show menu". True when one opened.
    pub fn show_menu(&mut self, id: &str) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        self.cancel_touch();
        self.runtime.accessibility_focus(id);
        let outcome = self
            .runtime
            .context_menu_focused(&mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        if outcome.activated {
            self.recompose()?;
        }
        Ok(outcome.activated)
    }
    pub fn cancel_touch(&mut self) {
        self.touch = None;
        self.runtime.cancel_drag();
        self.runtime.cancel_input();
        self.runtime.pointer_leave();
    }
    pub fn set_suspended(&mut self, suspended: bool) {
        self.cancel_touch();
        self.suspended = suspended;
        self.runtime.settle_animations();
    }
    pub fn activate(&mut self, id: &str) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        let o = self
            .runtime
            .accessibility_activate(id, &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        // A tap shows no keyboard focus ring on a phone.
        self.runtime.hide_focus_ring();
        if o.activated {
            self.recompose()?;
        }
        Ok(o.activated)
    }
    /// Controlled text replacement from a native text-input proxy. Composition
    /// stays owned by that proxy; send only committed text, not marked text.
    pub fn set_text(&mut self, id: &str, text: &str) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        let o = self
            .runtime
            .accessibility_set_value(
                id,
                AccessValue::Text(text.into()),
                &mut self.handlers,
                &mut self.context,
            )
            .map_err(|e| e.to_string())?;
        if o.activated {
            self.recompose()?;
        }
        Ok(o.activated)
    }
    pub fn accessibility_nodes(&self) -> Vec<AccessNode> {
        self.runtime.accessibility_nodes()
    }
    /// Focus a mounted editable field before the platform shows its keyboard.
    /// The host's keyboard went away: the field that had it loses focus, so
    /// nothing asks for the keyboard again until someone taps a field.
    pub fn blur(&mut self) -> Result<(), String> {
        if self.runtime.blur() {
            self.layout()?;
        }
        Ok(())
    }
    pub fn focus_text(&mut self, id: &str) -> Result<bool, String> {
        if self.suspended
            || !self.runtime.accessibility_nodes().iter().any(|n| {
                n.id == id
                    && n.enabled
                    && !n.read_only
                    && matches!(
                        n.role,
                        crate::declarative::AccessRole::TextInput
                            | crate::declarative::AccessRole::MultilineTextInput
                    )
            })
        {
            return Ok(false);
        }
        let changed = self.runtime.accessibility_focus(id);
        if changed {
            self.layout()?;
        }
        Ok(changed)
    }
    /// Assistive-technology scroll request, in logical points.
    pub fn scroll(&mut self, id: &str, delta: [f32; 2]) -> Result<bool, String> {
        if self.suspended {
            return Ok(false);
        }
        let Some(node) = self
            .runtime
            .accessibility_nodes()
            .into_iter()
            .find(|n| n.id == id && n.enabled && n.scroll.is_some())
        else {
            return Ok(false);
        };
        let [x, y, w, h] = node.bounds;
        let o = self
            .runtime
            .scroll_outcome(
                x + w / 2.,
                y + h / 2.,
                delta,
                &mut self.handlers,
                &mut self.context,
            )
            .map_err(|e| e.to_string())?;
        if o.activated {
            self.recompose()?;
        } else if o.changed {
            self.layout()?;
        }
        Ok(o.changed)
    }
    /// Present the shared drawing batches to the host-owned surface. The
    /// renderer reports GPU drawing versus its explicit software fallback.
    pub fn present(
        &mut self,
        renderer: &mut SurfaceRenderer,
        theme: &UiTheme,
    ) -> Result<PresentStatus, String> {
        if self.suspended {
            return Ok(PresentStatus::Retry);
        }
        self.tick();
        self.layout()?;
        let batches = self.runtime.paint_ui(&mut self.painter, theme);
        renderer.present(&batches, &mut self.painter, theme.palette.base[0])
    }
}
