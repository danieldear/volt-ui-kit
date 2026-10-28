//! Experimental, platform-owned touch view. No windowing or OS dependencies.
//!
//! Coordinates and sizes are logical points; the renderer uses physical pixels.
//! A platform host owns safe-area/keyboard insets and its GPU surface. This first
//! host supports single-finger tap/drag scrolling, not inertia or multi-touch.
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
            host_font,
        };
        // The first mobile prototype uses no perpetual animation/display link.
        value.runtime.set_reduced_motion(true);
        value.layout()?;
        Ok(value)
    }
    pub fn context(&self) -> &C {
        &self.context
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
    fn layout(&mut self) -> Result<(), String> {
        self.runtime
            .layout(self.viewport, &Bindings::default(), &mut self.painter)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    fn recompose(&mut self) -> Result<(), String> {
        let element = (self.view)(&self.context);
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
        let Some(t) = self.touch.as_mut() else {
            return Ok(());
        };
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
        if t.scrolling {
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
        if !t.scrolling && !self.suspended {
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
    pub fn cancel_touch(&mut self) {
        self.touch = None;
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
    pub fn focus_text(&mut self, id: &str) -> Result<bool, String> {
        if self.suspended
            || !self.runtime.accessibility_nodes().iter().any(|n| {
                n.id == id
                    && n.enabled
                    && !n.read_only
                    && matches!(n.role, crate::declarative::AccessRole::TextInput)
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
        self.layout()?;
        let batches = self.runtime.paint_ui(&mut self.painter, theme);
        renderer.present(&batches, &mut self.painter, theme.palette.base[0])
    }
}
