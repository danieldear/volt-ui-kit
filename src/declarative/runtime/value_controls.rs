//! Scalar input state. All arithmetic is constant-time and bounded by validated props.
use super::super::document::SliderProps;
use super::*;

/// Horizontal slider navigation; page keys advance ten steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKey {
    Decrease,
    Increase,
    First,
    Last,
    PageDecrease,
    PageIncrease,
}

fn snap(props: &SliderProps, value: f32) -> f32 {
    let value = f64::from(value.clamp(props.min, props.max));
    let min = f64::from(props.min);
    let max = f64::from(props.max);
    let step = f64::from(props.step);
    let grid = (min + ((value - min) / step).round() * step).clamp(min, max);
    if (max - value).abs() <= (grid - value).abs() {
        props.max
    } else {
        grid as f32
    }
}
fn advance(props: &SliderProps, value: f32, key: ValueKey) -> f32 {
    if key == ValueKey::First {
        return props.min;
    }
    if key == ValueKey::Last {
        return props.max;
    }
    let min = f64::from(props.min);
    let max = f64::from(props.max);
    let step = f64::from(props.step);
    let ratio = (f64::from(value) - min) / step;
    let nearest = ratio.round();
    let on_grid = (min + nearest * step) as f32 == value || (ratio - nearest).abs() <= 1e-5;
    let count = if matches!(key, ValueKey::PageDecrease | ValueKey::PageIncrease) {
        10.
    } else {
        1.
    };
    let index = if matches!(key, ValueKey::Increase | ValueKey::PageIncrease) {
        if on_grid {
            nearest + count
        } else {
            ratio.floor() + count
        }
    } else if on_grid {
        nearest - count
    } else {
        ratio.ceil() - count
    };
    (min + index * step).clamp(min, max) as f32
}
fn slider_geometry(b: &LayoutBox, scale: f32) -> (f32, f32, f32) {
    let [x, _, w, h] = b.bounds.bounds();
    let radius = (8. * scale).min(w * 0.5).min(h * 0.5);
    (x + radius, (w - 2. * radius).max(0.), radius)
}
impl Runtime {
    fn slider_at(&self, x: f32, y: f32) -> Option<String> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let id = self.hit(x, y)?;
        self.boxes
            .iter()
            .find(|b| b.id == id && b.enabled && b.slider.is_some())
            .map(|b| b.id.clone())
    }
    fn slider_pointer_value(&self, id: &str, x: f32) -> Option<f32> {
        if !x.is_finite() {
            return None;
        }
        let b = self.boxes.iter().find(|b| b.id == id && b.enabled)?;
        let props = b.slider.as_ref()?;
        let (left, width, _) = slider_geometry(b, self.scale);
        let ratio = if width > 0. {
            (f64::from(x) - f64::from(left)) / f64::from(width)
        } else {
            0.
        };
        Some(snap(
            props,
            (f64::from(props.min)
                + (f64::from(props.max) - f64::from(props.min)) * ratio.clamp(0., 1.))
                as f32,
        ))
    }
    fn emit_slider<C>(
        &mut self,
        id: &str,
        value: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        let Some(b) = self.boxes.iter().find(|b| b.id == id && b.enabled) else {
            return Ok(false);
        };
        let Some(props) = &b.slider else {
            return Ok(false);
        };
        if !value.is_finite() || value < props.min || value > props.max {
            return Ok(false);
        }
        let previous = self.slider_values.get(id).copied().unwrap_or(props.value);
        if value == previous {
            return Ok(false);
        }
        let Some(action) = props.on_change.clone() else {
            return Ok(false);
        };
        handlers
            .dispatch(
                &action,
                Event::ValueChanged {
                    bits: value.to_bits(),
                },
                context,
            )
            .map_err(|e| DocumentError(e.to_string()))?;
        self.slider_values.insert(id.into(), value);
        Ok(true)
    }
    pub(super) fn slider_pointer_down<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<Option<InputOutcome>, DocumentError> {
        let Some(id) = self.slider_at(x, y) else {
            return Ok(None);
        };
        self.slider_capture = Some(id.clone());
        self.capture = None;
        self.keyboard = None;
        self.focused = Some(id.clone());
        self.hovered = Some(id.clone());
        let activated = match self.slider_pointer_value(&id, x) {
            Some(value) => self.emit_slider(&id, value, handlers, context)?,
            None => false,
        };
        Ok(Some(InputOutcome {
            changed: true,
            activated,
        }))
    }
    pub(super) fn slider_pointer_move<C>(
        &mut self,
        x: f32,
        _y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<Option<InputOutcome>, DocumentError> {
        let Some(id) = self.slider_capture.clone() else {
            return Ok(None);
        };
        if !self
            .boxes
            .iter()
            .any(|b| b.id == id && b.enabled && b.slider.is_some())
        {
            self.slider_capture = None;
            return Ok(Some(InputOutcome {
                changed: true,
                activated: false,
            }));
        }
        let activated = match self.slider_pointer_value(&id, x) {
            Some(value) => self.emit_slider(&id, value, handlers, context)?,
            None => false,
        };
        Ok(Some(InputOutcome {
            changed: activated,
            activated,
        }))
    }
    pub(super) fn slider_pointer_up(&mut self) -> bool {
        self.slider_capture.take().is_some()
    }
    /// Change only the focused enabled slider. Same-value input does not dispatch.
    pub fn slider_key<C>(
        &mut self,
        key: ValueKey,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let Some(id) = self.focused.clone() else {
            return Ok(InputOutcome::default());
        };
        let Some(props) = self
            .boxes
            .iter()
            .find(|b| b.id == id && b.enabled)
            .and_then(|b| b.slider.clone())
        else {
            return Ok(InputOutcome::default());
        };
        let capture_changed = self.cancel_pointer_captures();
        let value = self.slider_values.get(&id).copied().unwrap_or(props.value);
        let activated = self.emit_slider(&id, advance(&props, value, key), handlers, context)?;
        Ok(InputOutcome {
            changed: activated || capture_changed,
            activated,
        })
    }
    pub(super) fn slider_paint_value(&self, b: &LayoutBox) -> f32 {
        self.slider_values
            .get(&b.id)
            .copied()
            .unwrap_or_else(|| b.slider.as_ref().map_or(0., |p| p.value))
    }
}
/// Track, progress and circular thumb use the existing retained-shape painter.
pub(super) fn paint_slider<P: DocumentPainter>(
    p: &mut P,
    shapes: &mut Vec<P::Shape>,
    b: &LayoutBox,
    ui: &UiTheme,
    state: ButtonState,
    scale: f32,
    value: f32,
) {
    let Some(props) = &b.slider else {
        return;
    };
    let c = ui.colors();
    let [x, y, w, h] = b.bounds.bounds();
    let (left, length, radius) = slider_geometry(b, scale);
    let center = y + h * 0.5;
    let fraction = ((f64::from(value) - f64::from(props.min))
        / (f64::from(props.max) - f64::from(props.min)))
    .clamp(0., 1.) as f32;
    let thumb = left + length * fraction;
    let thickness = (4. * scale).min(h);
    let mut track = b.style.background.unwrap_or(c.selected);
    let mut accent = b.style.accent_color.unwrap_or(c.accent);
    let mut thumb_color = b.style.color.unwrap_or(c.text);
    if state.disabled {
        track[3] *= 0.45;
        accent[3] *= 0.45;
        thumb_color[3] *= 0.45;
    }
    if length > 0. && thickness > 0. {
        paint_surface(
            p,
            shapes,
            [left, center - thickness * 0.5, length, thickness],
            thickness * 0.5,
            track,
        );
        if fraction > 0. {
            paint_surface(
                p,
                shapes,
                [left, center - thickness * 0.5, length * fraction, thickness],
                thickness * 0.5,
                accent,
            );
        }
    }
    if radius > 0. {
        paint_surface(
            p,
            shapes,
            [thumb - radius, center - radius, radius * 2., radius * 2.],
            radius,
            thumb_color,
        );
        if state.focused {
            paint_border(
                p,
                shapes,
                [thumb - radius, center - radius, radius * 2., radius * 2.],
                radius,
                2. * scale,
                accent,
            );
        }
    }
    if state.focused {
        paint_border(p, shapes, [x, y, w, h], b.radius_spec(scale), scale, accent);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn props() -> SliderProps {
        SliderProps {
            value: 0.,
            min: 0.,
            max: 1.,
            step: 0.3,
            on_change: None,
        }
    }
    #[test]
    fn uneven_endpoint_is_reachable_and_reversible() {
        let p = props();
        let mut value = 0.;
        for _ in 0..4 {
            value = advance(&p, value, ValueKey::Increase);
        }
        assert_eq!(value, 1.);
        assert_eq!(advance(&p, value, ValueKey::Increase), 1.);
        let previous = advance(&p, value, ValueKey::Decrease);
        assert!((previous - 0.9).abs() < 1e-6);
        assert_eq!(snap(&p, 0.99), 1.);
        assert_eq!(advance(&p, 0.9, ValueKey::PageDecrease), 0.);
    }
}

#[cfg(test)]
mod numeric_properties {
    use super::*;
    #[test]
    fn steps_and_snaps_stay_finite_bounded_and_monotonic_for_offset_ranges() {
        for (min, max, step) in [
            (-10., 10., 0.3),
            (10., 20., 7.),
            (-1., 1., 0.6),
            (100_000., 100_100., 3.),
        ] {
            let props = SliderProps {
                value: min,
                min,
                max,
                step,
                on_change: None,
            };
            let mut value = min;
            for _ in 0..256 {
                let next = advance(&props, value, ValueKey::Increase);
                assert!(next.is_finite() && next >= value && next <= max);
                value = next;
            }
            assert_eq!(value, max);
            for _ in 0..256 {
                let next = advance(&props, value, ValueKey::Decrease);
                assert!(next.is_finite() && next <= value && next >= min);
                value = next;
            }
            assert_eq!(value, min);
            for index in 0..=100 {
                let input = min + (max - min) * index as f32 / 100.;
                let rounded = snap(&props, input);
                assert!(rounded.is_finite() && rounded >= min && rounded <= max);
            }
        }
    }
}

use super::super::document::{ProgressKind, SplitProps};
#[derive(Clone)]
pub(super) struct SplitCapture {
    pub handle: String,
    pub owner: String,
    pub grab_offset: f32,
}
#[derive(Default)]
pub(super) struct ProgressClock {
    now: Duration,
    reduced: bool,
}
/// Physical pane lengths. Unsatisfiable minima are scaled proportionally rather
/// than overlapping panes or producing negative geometry.
pub(super) fn split_sizes(total: f32, props: &SplitProps, scale: f32) -> [f32; 3] {
    let handle = (props.handle_size * scale).min(total.max(0.));
    let available = (total - handle).max(0.);
    let first_min = props.min_first * scale;
    let second_min = props.min_second * scale;
    let sum = first_min + second_min;
    let first = if sum > available && sum > 0. {
        available * first_min / sum
    } else {
        (available * props.ratio).clamp(
            first_min.min(available),
            (available - second_min).max(first_min.min(available)),
        )
    };
    [first, handle, (available - first).max(0.)]
}
impl Runtime {
    fn split_geometry(&self, owner: &str) -> Option<(LayoutBox, SplitProps, usize, f32, f32, f32)> {
        let parent = self.layout_boxes.iter().find(|b| b.id == owner)?.clone();
        let props = parent.split.clone()?;
        let axis = usize::from(props.axis == super::super::Direction::Vertical);
        let r = parent.bounds.bounds();
        let pad = parent.style.layout.padding;
        let origin = r[axis] + pad[if axis == 0 { 3 } else { 0 }];
        let total = (r[axis + 2]
            - if axis == 0 {
                pad[1] + pad[3]
            } else {
                pad[0] + pad[2]
            })
        .max(0.);
        let sizes = split_sizes(total, &props, self.scale);
        Some((
            parent,
            props,
            axis,
            origin,
            total,
            (total - sizes[1]).max(0.),
        ))
    }
    fn emit_split<C>(
        &mut self,
        owner: &str,
        value: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        let Some((_, mut props, _, _, total, available)) = self.split_geometry(owner) else {
            return Ok(false);
        };
        if available <= 0. || !value.is_finite() {
            return Ok(false);
        }
        props.ratio = self.split_values.get(owner).copied().unwrap_or(props.ratio);
        let previous = split_sizes(total, &props, self.scale)[0] / available;
        props.ratio = value.clamp(0., 1.);
        let value = split_sizes(total, &props, self.scale)[0] / available;
        if (previous - value).abs() <= f32::EPSILON {
            return Ok(false);
        }
        let Some(action) = props.on_change else {
            return Ok(false);
        };
        handlers
            .dispatch(
                &action,
                Event::ValueChanged {
                    bits: value.to_bits(),
                },
                context,
            )
            .map_err(|e| DocumentError(e.to_string()))?;
        self.split_values.insert(owner.into(), value);
        Ok(true)
    }
    pub(super) fn split_pointer_down<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<Option<InputOutcome>, DocumentError> {
        let _ = (handlers, context);
        if !x.is_finite() || !y.is_finite() {
            return Ok(None);
        }
        let Some(id) = self.hit(x, y) else {
            return Ok(None);
        };
        let Some(handle) = self
            .boxes
            .iter()
            .find(|b| b.id == id && b.enabled && b.split_handle.is_some())
        else {
            return Ok(None);
        };
        let owner = handle.split_handle.clone().unwrap();
        let Some((_, _, axis, _, _, _)) = self.split_geometry(&owner) else {
            return Ok(None);
        };
        let r = handle.bounds.bounds();
        let coordinate = [x, y][axis];
        let grab_offset = coordinate - (r[axis] + r[axis + 2] * 0.5);
        self.split_capture = Some(SplitCapture {
            handle: id.clone(),
            owner,
            grab_offset,
        });
        self.capture = None;
        self.keyboard = None;
        self.focused = Some(id.clone());
        self.hovered = Some(id);
        Ok(Some(InputOutcome {
            changed: true,
            activated: false,
        }))
    }
    pub(super) fn split_pointer_move<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<Option<InputOutcome>, DocumentError> {
        let Some(capture) = self.split_capture.clone() else {
            return Ok(None);
        };
        if !self.scope_allows_id(&capture.handle)
            || !self.boxes.iter().any(|b| {
                b.id == capture.handle
                    && b.enabled
                    && b.split_handle.as_deref() == Some(&capture.owner)
            })
        {
            self.split_capture = None;
            return Ok(Some(InputOutcome {
                changed: true,
                activated: false,
            }));
        }
        if !x.is_finite() || !y.is_finite() {
            return Ok(Some(InputOutcome::default()));
        }
        let Some((_, props, axis, origin, total, available)) = self.split_geometry(&capture.owner)
        else {
            return Ok(Some(InputOutcome::default()));
        };
        if available <= 0. {
            return Ok(Some(InputOutcome::default()));
        }
        let handle = split_sizes(total, &props, self.scale)[1];
        let ratio = ([x, y][axis] - capture.grab_offset - origin - handle * 0.5) / available;
        let activated = self.emit_split(&capture.owner, ratio, handlers, context)?;
        Ok(Some(InputOutcome {
            changed: activated,
            activated,
        }))
    }
    pub(super) fn split_pointer_up(&mut self) -> bool {
        self.split_capture.take().is_some()
    }
    pub fn split_key<C>(
        &mut self,
        key: ValueKey,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let Some(id) = self.focused.clone() else {
            return Ok(InputOutcome::default());
        };
        let Some(owner) = self
            .boxes
            .iter()
            .find(|b| b.id == id && b.enabled)
            .and_then(|b| b.split_handle.clone())
        else {
            return Ok(InputOutcome::default());
        };
        if !self.scope_allows_id(&id) {
            return Ok(InputOutcome::default());
        }
        let Some((_, mut props, _, _, total, available)) = self.split_geometry(&owner) else {
            return Ok(InputOutcome::default());
        };
        if available <= 0. {
            return Ok(InputOutcome::default());
        }
        let capture_changed = self.cancel_pointer_captures();
        props.ratio = self
            .split_values
            .get(&owner)
            .copied()
            .unwrap_or(props.ratio);
        let current = split_sizes(total, &props, self.scale)[0] / available;
        let value = match key {
            ValueKey::First => 0.,
            ValueKey::Last => 1.,
            ValueKey::Decrease => current - 0.02,
            ValueKey::Increase => current + 0.02,
            ValueKey::PageDecrease => current - 0.1,
            ValueKey::PageIncrease => current + 0.1,
        };
        let activated = self.emit_split(&owner, value, handlers, context)?;
        Ok(InputOutcome {
            changed: activated || capture_changed,
            activated,
        })
    }
    /// Called after layout/scrolling by hosts; proposed windows are always bounded.
    pub fn dispatch_virtual_windows<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        let candidates: Vec<_> = self
            .boxes
            .iter()
            .filter_map(|b| {
                b.virtual_list
                    .as_ref()
                    .filter(|_| self.scope_allows_id(&b.id))
                    .map(|props| (b.clone(), props.clone()))
            })
            .collect();
        let mut activated = false;
        for (b, props) in candidates {
            let r = b.bounds.bounds();
            let pad = b.style.layout.padding;
            let height = ((r[3] - pad[0] - pad[2]) / self.scale).max(0.);
            let offset = self.scroll_offsets.get(&b.id).map_or(0., |v| v[1]);
            let first = (f64::from(offset) / f64::from(props.row_height))
                .floor()
                .max(0.) as usize;
            let visible = (height / props.row_height).ceil() as usize;
            let start = first.saturating_sub(props.overscan).min(props.count);
            let end = first
                .saturating_add(visible)
                .saturating_add(props.overscan)
                .saturating_add(1)
                .min(props.count)
                .min(start.saturating_add(96));
            if start != props.start || end != props.end {
                if let Some(action) = props.on_window_change {
                    handlers
                        .dispatch(&action, Event::WindowChanged { start, end }, context)
                        .map_err(|e| DocumentError(e.to_string()))?;
                    activated = true;
                }
            }
        }
        Ok(activated)
    }
    pub(super) fn advance_progress(&mut self, now: Duration) -> bool {
        let changed = now > self.progress_clock.now;
        self.progress_clock.now = self.progress_clock.now.max(now);
        changed && self.progress_active()
    }
    pub(super) fn progress_active(&self) -> bool {
        !self.progress_clock.reduced
            && self.boxes.iter().any(|b| {
                b.enabled && !b.departing && b.progress.as_ref().is_some_and(|p| p.value.is_none())
            })
    }
    pub(super) fn set_progress_reduced(&mut self, reduced: bool) {
        self.progress_clock.reduced = reduced;
    }
    pub(super) fn paint_progress<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
        b: &LayoutBox,
    ) -> PaintBatch<P::Shape, P::Glyph> {
        let mut shapes = vec![];
        let Some(props) = &b.progress else {
            return PaintBatch {
                clip: b.clip,
                shapes,
                glyphs: vec![],
            };
        };
        let c = ui.colors();
        let mut accent = b.style.accent_color.unwrap_or(c.accent);
        let mut background = b.style.background.unwrap_or(c.selected);
        if !b.enabled {
            accent[3] *= 0.45;
            background[3] *= 0.45;
        }
        let r = b.bounds.bounds();
        let still = self.progress_clock.reduced || !b.enabled;
        let phase = if still {
            0.
        } else {
            ((self.progress_clock.now.as_secs_f64() * 1000. / f64::from(props.period_ms)).fract())
                as f32
        };
        match props.kind {
            ProgressKind::Bar => {
                paint_surface(p, &mut shapes, r, b.radius_spec(self.scale), background);
                let (start, width) = if let Some(value) = props.value {
                    (0., value * r[2])
                } else if still {
                    (r[2] * 0.35, r[2] * 0.3)
                } else {
                    (phase * r[2] * 0.7, r[2] * 0.3)
                };
                if width > 0. {
                    paint_surface(
                        p,
                        &mut shapes,
                        [r[0] + start, r[1], width, r[3]],
                        b.radius_spec(self.scale),
                        accent,
                    );
                }
            }
            ProgressKind::Spinner => {
                let dot = (r[2].min(r[3]) * 0.07).max(0.1);
                let radius = (r[2].min(r[3]) * 0.5 - dot).max(0.);
                let tick = (phase * 12.) as usize;
                for index in 0..12 {
                    let angle = index as f32 * std::f32::consts::TAU / 12.;
                    let mut color = accent;
                    color[3] *= 0.18 + 0.82 * ((index + 12 - tick) % 12) as f32 / 11.;
                    let x = r[0] + r[2] * 0.5 + angle.cos() * radius - dot;
                    let y = r[1] + r[3] * 0.5 + angle.sin() * radius - dot;
                    paint_surface(p, &mut shapes, [x, y, dot * 2., dot * 2.], dot, color);
                }
            }
        }
        PaintBatch {
            clip: b.clip,
            shapes,
            glyphs: vec![],
        }
    }
    pub(super) fn paint_split<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
        b: &LayoutBox,
    ) -> PaintBatch<P::Shape, P::Glyph> {
        let mut shapes = vec![];
        let c = ui.colors();
        let r = b.bounds.bounds();
        let state = self.state_for_box(b);
        let mut surface = b.style.background.unwrap_or(c.selected);
        let mut ink = b.style.color.unwrap_or(c.muted);
        if state.disabled {
            surface[3] *= 0.45;
            ink[3] *= 0.45;
        }
        paint_surface(p, &mut shapes, r, b.radius_spec(self.scale), surface);
        let vertical = r[3] >= r[2];
        let dot = (1.2 * self.scale).min(r[2] * 0.25).min(r[3] * 0.25);
        for offset in [-4., 0., 4.] {
            let x = r[0] + r[2] * 0.5 + if vertical { 0. } else { offset * self.scale };
            let y = r[1] + r[3] * 0.5 + if vertical { offset * self.scale } else { 0. };
            paint_surface(
                p,
                &mut shapes,
                [x - dot, y - dot, 2. * dot, 2. * dot],
                dot,
                ink,
            );
        }
        if state.focused {
            paint_border(
                p,
                &mut shapes,
                r,
                b.radius_spec(self.scale),
                self.scale,
                c.accent,
            );
        }
        PaintBatch {
            clip: b.clip,
            shapes,
            glyphs: vec![],
        }
    }
}
