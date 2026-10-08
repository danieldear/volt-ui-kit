use super::document::{parse_color, FontSlant};
use super::{
    Align, Bindings, Document, DocumentError, Event, Handlers, Kind, Node, Rounding, Tone, Value,
};
use crate::{Button, ButtonColors, Colors, Painter, Panel, Rect, SurfaceStyle, Theme, Viewport};
use std::collections::BTreeMap;

/// Physical-pixel typography resolved from inherited document styles.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: String,
    pub size: f32,
    pub weight: u16,
    pub italic: bool,
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: "system".into(),
            size: 14.,
            weight: 400,
            italic: false,
        }
    }
}
/// New runtime text service. Legacy Painter estimates remain unchanged.
pub trait TextMetrics {
    /// Physical-pixel size, including wrapping at max_width and 1.2-em line height.
    fn measure(&mut self, text: &str, size: f32, max_width: f32) -> [f32; 2];
    /// Existing adapters remain valid; styled adapters override this for typography.
    fn measure_styled(&mut self, text: &str, style: &TextStyle, max_width: f32) -> [f32; 2] {
        self.measure(text, style.size, max_width)
    }
}
/// Paint adapters for the new runtime provide the same measurement and draw rules.
pub trait DocumentPainter: Painter + TextMetrics {
    fn ui_text(
        &mut self,
        out: &mut Vec<Self::Glyph>,
        text: &str,
        bounds: [f32; 4],
        size: f32,
        color: [f32; 4],
    );
    /// Must use the same style/font fallback rules as measure_styled.
    fn ui_text_styled(
        &mut self,
        out: &mut Vec<Self::Glyph>,
        text: &str,
        bounds: [f32; 4],
        style: &TextStyle,
        color: [f32; 4],
    ) {
        self.ui_text(out, text, bounds, style.size, color);
    }
}
/// A batch keeps shapes/text together in tree paint order and shares the hit clip.
#[derive(Clone, PartialEq)]
pub struct PaintBatch<S, G> {
    pub clip: Rect,
    pub shapes: Vec<S>,
    pub glyphs: Vec<G>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub disabled: bool,
}
/// Separate visual press-state changes from actual callback dispatch.
/// Native view bodies only recompose when `activated` is true.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputOutcome {
    pub changed: bool,
    pub activated: bool,
}
#[derive(Clone, Debug)]
pub struct LayoutBox {
    pub id: String,
    pub kind: Kind,
    pub bounds: Rect,
    pub clip: Rect,
    pub enabled: bool,
    /// Host-owned checked value; activation emits the proposed next value.
    pub checked: bool,
    pub text_style: TextStyle,
    style: ResolvedStyle,
    pub(crate) text: String,
    pub(crate) action: Option<String>,
    pub(crate) tone: Tone,
    pub(crate) primary: bool,
}
/// Shared document layout and event state; owns no window or background tasks.
pub struct Runtime {
    document: Document,
    boxes: Vec<LayoutBox>,
    focused: Option<String>,
    hovered: Option<String>,
    capture: Option<String>,
    keyboard: Option<String>,
    scale: f32,
    cache: Option<([f32; 3], Bindings)>,
}
impl Runtime {
    pub fn new<C>(
        document: Document,
        handlers: &Handlers<'_, C>,
        values: &Bindings,
    ) -> Result<Self, DocumentError> {
        document.validate_connections(handlers, values)?;
        Ok(Self {
            document,
            boxes: vec![],
            focused: None,
            hovered: None,
            capture: None,
            keyboard: None,
            scale: 1.,
            cache: None,
        })
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn boxes(&self) -> &[LayoutBox] {
        &self.boxes
    }
    pub fn focused(&self) -> Option<&str> {
        self.focused.as_deref()
    }
    /// Parse and atomically install an optional JSON document.
    pub fn reload<C>(
        &mut self,
        source: &str,
        handlers: &Handlers<'_, C>,
        values: &Bindings,
    ) -> Result<(), DocumentError> {
        self.replace_document(Document::parse(source)?, handlers, values)
    }
    /// Validate before replacing the tree. Rejected candidates leave all runtime
    /// state untouched. Successful installs cancel transient input; focus survives
    /// only if its key still names the same enabled interactive component kind.
    /// The host must install the matching handlers immediately on success.
    pub fn replace_document<C>(
        &mut self,
        next: Document,
        handlers: &Handlers<'_, C>,
        values: &Bindings,
    ) -> Result<(), DocumentError> {
        next.validate_connections(handlers, values)?;
        fn find<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
            if node.id() == id {
                return Some(node);
            }
            node.children().iter().find_map(|child| find(child, id))
        }
        let keep_focus = self.focused.as_deref().is_some_and(|id| {
            match (find(self.document.root(), id), find(next.root(), id)) {
                (Some(old), Some(new)) => {
                    old.kind == new.kind && interactive(new.kind) && enabled(new, values)
                }
                _ => false,
            }
        });
        if !keep_focus {
            self.focused = None;
        }
        self.document = next;
        self.cache = None;
        self.boxes.clear();
        self.cancel_input();
        Ok(())
    }
    pub fn invalidate_layout(&mut self) {
        self.cache = None;
    }
    pub fn layout(
        &mut self,
        viewport: Viewport,
        values: &Bindings,
        metrics: &mut impl TextMetrics,
    ) -> Result<bool, DocumentError> {
        if Viewport::new(viewport.width, viewport.height, viewport.scale).is_none()
            || viewport.width > 16384.
            || viewport.height > 16384.
            || !(0.25..=4.).contains(&viewport.scale)
        {
            return Err(DocumentError(
                "viewport: invalid or exceeds runtime limits".into(),
            ));
        }
        let key = [viewport.width, viewport.height, viewport.scale];
        if self
            .cache
            .as_ref()
            .is_some_and(|(k, v)| *k == key && v == values)
        {
            return Ok(false);
        }
        self.document.validate_bindings(values)?;
        self.scale = viewport.scale;
        let mut boxes = vec![];
        let mut memo = BTreeMap::new();
        let mut styles = BTreeMap::new();
        resolve_styles(
            self.document.root(),
            &InheritedStyle::default(),
            self.scale,
            viewport.width / self.scale,
            &mut styles,
        );
        let area = Rect::new(0., 0., viewport.width, viewport.height).unwrap();
        place(
            self.document.root(),
            area,
            area,
            values,
            metrics,
            self.scale,
            &mut boxes,
            &mut memo,
            &styles,
        );
        self.boxes = boxes;
        let valid = |id: &Option<String>| {
            id.as_ref().is_none_or(|id| {
                self.boxes
                    .iter()
                    .any(|b| b.id == *id && interactive(b.kind) && b.enabled)
            })
        };
        if !valid(&self.focused) {
            self.focused = None;
        }
        if !valid(&self.capture) {
            self.capture = None;
        }
        if !valid(&self.keyboard) {
            self.keyboard = None;
        }
        self.hovered = None;
        self.cache = Some((key, values.clone()));
        Ok(true)
    }
    fn hit(&self, x: f32, y: f32) -> Option<String> {
        self.boxes
            .iter()
            .rev()
            .find(|b| {
                interactive(b.kind)
                    && b.enabled
                    && b.clip.contains(x, y)
                    && b.bounds
                        .contains_rounded(b.style.radius.unwrap_or(7. * self.scale), x, y)
            })
            .map(|b| b.id.clone())
    }
    pub fn pointer_move(&mut self, x: f32, y: f32) -> bool {
        let next = self.hit(x, y);
        let changed = next != self.hovered;
        self.hovered = next;
        changed
    }
    pub fn pointer_leave(&mut self) -> bool {
        self.hovered.take().is_some()
    }
    pub fn pointer_down(&mut self, x: f32, y: f32) -> bool {
        let hit = self.hit(x, y);
        let changed = self.capture != hit
            || self.focused != hit
            || self.hovered != hit
            || self.keyboard.is_some();
        self.capture = hit.clone();
        self.hovered = hit.clone();
        self.focused = hit;
        self.keyboard = None;
        changed
    }
    /// Compatibility wrapper: true means a pressed capture was released, even
    /// if release was outside. Use pointer_up_outcome to distinguish activation.
    pub fn pointer_up<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        Ok(self.pointer_up_outcome(x, y, handlers, context)?.changed)
    }
    pub fn pointer_up_outcome<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let pressed = self.capture.take();
        let activated = if let Some(id) = pressed
            .as_ref()
            .filter(|id| Some((*id).clone()) == self.hit(x, y))
        {
            self.activate(id, handlers, context)?
        } else {
            false
        };
        Ok(InputOutcome {
            changed: pressed.is_some(),
            activated,
        })
    }
    pub fn focus_next(&mut self, backwards: bool) -> bool {
        self.capture = None;
        self.keyboard = None;
        let ids: Vec<_> = self
            .boxes
            .iter()
            .filter(|b| interactive(b.kind) && b.enabled)
            .map(|b| b.id.clone())
            .collect();
        if ids.is_empty() {
            return self.focused.take().is_some();
        }
        let current = self
            .focused
            .as_ref()
            .and_then(|id| ids.iter().position(|v| v == id));
        let index = match current {
            Some(i) if backwards => (i + ids.len() - 1) % ids.len(),
            Some(i) => (i + 1) % ids.len(),
            None if backwards => ids.len() - 1,
            None => 0,
        };
        self.focused = Some(ids[index].clone());
        true
    }
    pub fn key_down(&mut self) -> bool {
        if self.keyboard.is_some() {
            return false;
        }
        self.capture = None;
        self.keyboard = self.focused.clone();
        self.keyboard.is_some()
    }
    /// Compatibility wrapper; true means the keyboard press was released.
    pub fn key_up<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        Ok(self.key_up_outcome(handlers, context)?.changed)
    }
    pub fn key_up_outcome<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let pressed = self.keyboard.take();
        let activated = if let Some(id) = pressed
            .as_ref()
            .filter(|id| self.focused.as_ref() == Some(id))
        {
            self.activate(id, handlers, context)?
        } else {
            false
        };
        Ok(InputOutcome {
            changed: pressed.is_some(),
            activated,
        })
    }
    pub fn cancel_input(&mut self) -> bool {
        let changed = self.capture.is_some() || self.keyboard.is_some() || self.hovered.is_some();
        self.capture = None;
        self.keyboard = None;
        self.hovered = None;
        changed
    }
    fn activate<C>(
        &self,
        id: &str,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        if let Some(b) = self.boxes.iter().find(|b| b.id == id && b.enabled) {
            if let Some(action) = &b.action {
                handlers
                    .dispatch(
                        action,
                        if matches!(b.kind, Kind::Checkbox | Kind::Toggle) {
                            Event::ToggleChanged(!b.checked)
                        } else {
                            Event::Activate
                        },
                        context,
                    )
                    .map_err(|e| DocumentError(e.to_string()))?;
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn button_state(&self, id: &str) -> ButtonState {
        ButtonState {
            hovered: self.hovered.as_deref() == Some(id),
            pressed: (self.capture.as_deref() == Some(id) && self.hovered.as_deref() == Some(id))
                || self.keyboard.as_deref() == Some(id),
            focused: self.focused.as_deref() == Some(id),
            disabled: self
                .boxes
                .iter()
                .find(|b| b.id == id)
                .is_none_or(|b| !b.enabled),
        }
    }
    pub fn paint<P: DocumentPainter>(
        &self,
        p: &mut P,
        theme: &Theme,
    ) -> Vec<PaintBatch<P::Shape, P::Glyph>> {
        let c = Colors::from_theme(theme);
        self.boxes
            .iter()
            .map(|b| {
                let mut shapes = vec![];
                let mut glyphs = vec![];
                let r = b.bounds.bounds();
                let tone_color = match b.tone {
                    Tone::Muted => c.muted,
                    Tone::Accent => c.accent,
                    _ => c.text,
                };
                let text_color = b.style.color.unwrap_or(tone_color);
                match b.kind {
                    Kind::Panel => {
                        if b.style.surface_override() {
                            paint_surface(
                                p,
                                &mut shapes,
                                r,
                                b.style.radius.unwrap_or(12. * self.scale),
                                b.style.background.unwrap_or(c.panel),
                            );
                            paint_border(
                                p,
                                &mut shapes,
                                r,
                                b.style.radius.unwrap_or(12. * self.scale),
                                b.style.border_width.unwrap_or(self.scale),
                                b.style.border_color.unwrap_or(c.border),
                            );
                        } else if let Some(panel) =
                            Panel::new(b.bounds, 12., self.scale, SurfaceStyle::from_theme(theme))
                        {
                            panel.paint(p, &mut shapes);
                        }
                    }
                    Kind::Row | Kind::Column | Kind::Stack | Kind::ZStack | Kind::Text => {
                        paint_overrides(p, &mut shapes, r, &b.style, self.scale);
                        if b.kind == Kind::Text {
                            p.ui_text_styled(&mut glyphs, &b.text, r, &b.text_style, text_color);
                        }
                    }
                    Kind::Button => {
                        let state = self.state_for_box(b);
                        let base = b.style.background.unwrap_or(if b.primary {
                            b.style.accent_color.unwrap_or(c.accent)
                        } else {
                            c.selected
                        });
                        let surface = state_surface(base, state, theme);
                        let radius = b.style.radius.unwrap_or(7. * self.scale);
                        let mut color = b.style.color.unwrap_or(if b.primary {
                            theme.background
                        } else {
                            c.text
                        });
                        if state.disabled {
                            color[3] *= 0.45;
                        }
                        if b.style.surface_override() {
                            paint_surface(p, &mut shapes, r, radius, surface);
                            paint_border(
                                p,
                                &mut shapes,
                                r,
                                radius,
                                b.style
                                    .border_width
                                    .unwrap_or(if b.style.border_color.is_some() {
                                        self.scale
                                    } else {
                                        0.
                                    }),
                                b.style.border_color.unwrap_or(c.border),
                            );
                        } else {
                            // Keep the legacy surface recipe on the unstyled default path.
                            Button {
                                label: "",
                                icon: None,
                                enabled: b.enabled,
                            }
                            .paint(
                                p,
                                &mut shapes,
                                &mut glyphs,
                                r,
                                self.scale,
                                ButtonColors {
                                    surface,
                                    text: color,
                                },
                            );
                        }
                        if state.focused {
                            paint_border(
                                p,
                                &mut shapes,
                                r,
                                radius,
                                2. * self.scale,
                                b.style.accent_color.unwrap_or(c.accent_bright),
                            );
                        }
                        let inset = 10. * self.scale;
                        let max_width = (r[2] - 2. * inset).max(0.1);
                        let [w, h] = p.measure_styled(&b.text, &b.text_style, max_width);
                        p.ui_text_styled(
                            &mut glyphs,
                            &b.text,
                            [
                                r[0] + ((r[2] - w) * 0.5).max(inset),
                                r[1] + (r[3] - h) * 0.5,
                                max_width,
                                h,
                            ],
                            &b.text_style,
                            color,
                        );
                    }
                    Kind::Checkbox | Kind::Toggle => {
                        let state = self.state_for_box(b);
                        let radius = b.style.radius.unwrap_or(6. * self.scale);
                        if let Some(background) = b.style.background {
                            paint_surface(
                                p,
                                &mut shapes,
                                r,
                                radius,
                                state_surface(background, state, theme),
                            );
                        } else if state.hovered || state.pressed {
                            let mut background = c.selected;
                            background[3] = if state.pressed { 0.7 } else { 0.35 };
                            paint_surface(p, &mut shapes, r, radius, background);
                        }
                        let border = b.style.border_color.unwrap_or(c.border);
                        paint_border(
                            p,
                            &mut shapes,
                            r,
                            radius,
                            b.style.border_width.unwrap_or(0.),
                            border,
                        );
                        if state.focused {
                            paint_border(
                                p,
                                &mut shapes,
                                r,
                                radius,
                                2. * self.scale,
                                b.style.accent_color.unwrap_or(c.accent_bright),
                            );
                        }
                        let mut accent = b.style.accent_color.unwrap_or(c.accent);
                        let mut color = text_color;
                        let mut quiet = c.selected;
                        if state.disabled {
                            accent[3] *= 0.45;
                            color[3] *= 0.45;
                            quiet[3] *= 0.45;
                        }
                        let control_h = (22. * self.scale).min(r[3]);
                        let left = r[0] + 2. * self.scale;
                        let top = r[1] + (r[3] - control_h) * 0.5;
                        let leading = if b.kind == Kind::Toggle {
                            56. * self.scale
                        } else {
                            34. * self.scale
                        };
                        if b.kind == Kind::Checkbox {
                            let box_bounds = [left, top, control_h, control_h];
                            paint_surface(
                                p,
                                &mut shapes,
                                box_bounds,
                                4. * self.scale,
                                if b.checked { accent } else { quiet },
                            );
                            paint_border(
                                p,
                                &mut shapes,
                                box_bounds,
                                4. * self.scale,
                                self.scale,
                                if state.hovered { accent } else { border },
                            );
                            if b.checked {
                                let check = if state.disabled {
                                    [
                                        theme.background[0],
                                        theme.background[1],
                                        theme.background[2],
                                        0.45,
                                    ]
                                } else {
                                    theme.background
                                };
                                p.card_line(
                                    &mut shapes,
                                    [left + 5. * self.scale, top + 11. * self.scale],
                                    [left + 9. * self.scale, top + 15. * self.scale],
                                    2. * self.scale,
                                    check,
                                );
                                p.card_line(
                                    &mut shapes,
                                    [left + 9. * self.scale, top + 15. * self.scale],
                                    [left + 17. * self.scale, top + 7. * self.scale],
                                    2. * self.scale,
                                    check,
                                );
                            }
                        } else {
                            let track_w = (42. * self.scale).min((r[2] - 2. * self.scale).max(0.));
                            let track = [left, top, track_w, control_h];
                            paint_surface(
                                p,
                                &mut shapes,
                                track,
                                control_h * 0.5,
                                if b.checked { accent } else { quiet },
                            );
                            paint_border(
                                p,
                                &mut shapes,
                                track,
                                control_h * 0.5,
                                self.scale,
                                border,
                            );
                            let diameter = (control_h - 6. * self.scale).max(0.);
                            let knob_x = left
                                + if b.checked {
                                    (track_w - diameter - 3. * self.scale).max(0.)
                                } else {
                                    3. * self.scale
                                };
                            let mut knob = text_color;
                            if state.disabled {
                                knob[3] *= 0.45;
                            }
                            paint_surface(
                                p,
                                &mut shapes,
                                [knob_x, top + 3. * self.scale, diameter, diameter],
                                diameter * 0.5,
                                knob,
                            );
                        }
                        let width = (r[2] - leading).max(0.1);
                        let [_, height] = p.measure_styled(&b.text, &b.text_style, width);
                        p.ui_text_styled(
                            &mut glyphs,
                            &b.text,
                            [r[0] + leading, r[1] + (r[3] - height) * 0.5, width, height],
                            &b.text_style,
                            color,
                        );
                    }
                    Kind::Badge => {
                        let radius = b.style.radius.unwrap_or(8. * self.scale);
                        paint_surface(
                            p,
                            &mut shapes,
                            r,
                            radius,
                            b.style.background.unwrap_or(c.selected),
                        );
                        paint_border(
                            p,
                            &mut shapes,
                            r,
                            radius,
                            b.style
                                .border_width
                                .unwrap_or(if b.style.border_color.is_some() {
                                    self.scale
                                } else {
                                    0.
                                }),
                            b.style.border_color.unwrap_or(c.border),
                        );
                        p.ui_text_styled(
                            &mut glyphs,
                            &b.text,
                            [
                                r[0] + 10. * self.scale,
                                r[1] + 5. * self.scale,
                                (r[2] - 20. * self.scale).max(0.),
                                (r[3] - 10. * self.scale).max(0.),
                            ],
                            &b.text_style,
                            text_color,
                        );
                    }
                    Kind::Spacer => {}
                    Kind::Separator => paint_surface(
                        p,
                        &mut shapes,
                        r,
                        b.style.radius.unwrap_or(0.),
                        b.style
                            .background
                            .or(b.style.color)
                            .or(b.style.border_color)
                            .unwrap_or(c.line),
                    ),
                }
                PaintBatch {
                    clip: b.clip,
                    shapes,
                    glyphs,
                }
            })
            .collect()
    }
    fn state_for_box(&self, b: &LayoutBox) -> ButtonState {
        ButtonState {
            hovered: self.hovered.as_deref() == Some(&b.id),
            pressed: (self.capture.as_deref() == Some(&b.id)
                && self.hovered.as_deref() == Some(&b.id))
                || self.keyboard.as_deref() == Some(&b.id),
            focused: self.focused.as_deref() == Some(&b.id),
            disabled: !b.enabled,
        }
    }
}
fn state_surface(mut surface: [f32; 4], state: ButtonState, theme: &Theme) -> [f32; 4] {
    if state.hovered && !state.disabled {
        for v in &mut surface[..3] {
            *v = (*v + 0.06).min(1.);
        }
    }
    if state.pressed {
        for v in &mut surface[..3] {
            *v *= 0.8;
        }
    }
    if state.disabled {
        for (v, bg) in surface[..3].iter_mut().zip(theme.background) {
            *v = (*v + bg) * 0.5;
        }
    }
    surface
}
fn paint_surface<P: Painter>(
    p: &P,
    out: &mut Vec<P::Shape>,
    bounds: [f32; 4],
    radius: f32,
    color: [f32; 4],
) {
    if bounds[2] > 0. && bounds[3] > 0. && color[3] > 0. {
        p.card_round(out, bounds, radius, color);
    }
}
fn paint_overrides<P: Painter>(
    p: &P,
    out: &mut Vec<P::Shape>,
    r: [f32; 4],
    style: &ResolvedStyle,
    scale: f32,
) {
    let radius = style.radius.unwrap_or(0.);
    if let Some(background) = style.background {
        paint_surface(p, out, r, radius, background);
    }
    if let Some(color) = style.border_color {
        paint_border(
            p,
            out,
            r,
            radius,
            style.border_width.unwrap_or(scale),
            color,
        );
    }
}
/// Draw a true outline rather than overpainting a filled border, so RGBA fills stay correct.
fn paint_border<P: Painter>(
    p: &P,
    out: &mut Vec<P::Shape>,
    [x, y, w, h]: [f32; 4],
    radius: f32,
    width: f32,
    color: [f32; 4],
) {
    let width = width.min(w * 0.5).min(h * 0.5);
    if width <= 0. || color[3] <= 0. {
        return;
    }
    let inset = width * 0.5;
    let x = x + inset;
    let y = y + inset;
    let w = w - width;
    let h = h - width;
    let r = (radius - inset).max(0.).min(w * 0.5).min(h * 0.5);
    let mut line = |a, b| p.card_line(out, a, b, width, color);
    line([x + r, y], [x + w - r, y]);
    line([x + w, y + r], [x + w, y + h - r]);
    line([x + w - r, y + h], [x + r, y + h]);
    line([x, y + h - r], [x, y + r]);
    if r > 0. {
        for (cx, cy, start) in [
            (x + w - r, y + r, -90.),
            (x + w - r, y + h - r, 0.),
            (x + r, y + h - r, 90.),
            (x + r, y + r, 180.),
        ] {
            for i in 0..8 {
                let a = (start + i as f32 * 11.25).to_radians();
                let b = (start + (i + 1) as f32 * 11.25).to_radians();
                line(
                    [cx + r * a.cos(), cy + r * a.sin()],
                    [cx + r * b.cos(), cy + r * b.sin()],
                );
            }
        }
    }
}
fn interactive(kind: Kind) -> bool {
    matches!(kind, Kind::Button | Kind::Checkbox | Kind::Toggle)
}
fn checked(n: &Node, values: &Bindings) -> bool {
    n.bind_checked.as_ref().map_or(n.checked, |name| {
        matches!(values.get(name), Some(Value::Bool(true)))
    })
}
#[derive(Clone, Debug)]
struct ResolvedStyle {
    layout: ResolvedLayout,
    text: TextStyle,
    color: Option<[f32; 4]>,
    background: Option<[f32; 4]>,
    border_color: Option<[f32; 4]>,
    accent_color: Option<[f32; 4]>,
    border_width: Option<f32>,
    radius: Option<f32>,
    round_full: bool,
}
/// Resolve once per layout, so measurement, allocation, placement, and hits agree.
#[derive(Clone, Debug)]
struct ResolvedLayout {
    axis: usize,
    padding: [f32; 4],
    gap: f32,
}
impl ResolvedLayout {
    fn new(n: &Node, scale: f32, logical_width: f32) -> Self {
        let axis = if n.kind == Kind::Stack {
            n.responsive
                .direction
                .as_ref()
                .map_or(n.direction, |v| *v.resolve(logical_width))
                .axis()
        } else {
            usize::from(n.kind != Kind::Row)
        };
        let mut padding = n.padding_sides();
        for (value, rule) in padding.iter_mut().zip(&n.responsive.padding) {
            if let Some(rule) = rule {
                *value = *rule.resolve(logical_width);
            }
        }
        Self {
            axis,
            padding: padding.map(|v| v * scale),
            gap: n.responsive.gap[axis]
                .as_ref()
                .map_or(n.main_gap(axis), |v| *v.resolve(logical_width))
                * scale,
        }
    }
}
impl ResolvedStyle {
    fn surface_override(&self) -> bool {
        self.background.is_some()
            || self.border_color.is_some()
            || self.border_width.is_some()
            || self.radius.is_some()
    }
}
#[derive(Clone, Default)]
struct InheritedStyle {
    family: Option<String>,
    size: Option<f32>,
    weight: Option<u16>,
    italic: Option<bool>,
    color: Option<[f32; 4]>,
}
fn resolve_styles(
    n: &Node,
    inherited: &InheritedStyle,
    scale: f32,
    logical_width: f32,
    out: &mut BTreeMap<String, ResolvedStyle>,
) {
    let mut text = inherited.clone();
    if let Some(family) = &n.style.font_family {
        text.family = Some(family.clone());
    }
    text.size = n.style.font_size.or(n.font_size).or(text.size);
    text.weight = n.style.font_weight.or(text.weight);
    text.italic = n
        .style
        .font_style
        .map(|v| matches!(v, FontSlant::Italic))
        .or(text.italic);
    text.color = n
        .style
        .color
        .as_deref()
        .and_then(parse_color)
        .or(text.color);
    let default_size = if matches!(n.kind, Kind::Button | Kind::Badge) {
        12.
    } else {
        14.
    };
    out.insert(
        n.id.clone(),
        ResolvedStyle {
            layout: ResolvedLayout::new(n, scale, logical_width),
            text: TextStyle {
                family: text.family.clone().unwrap_or_else(|| "system".into()),
                size: text.size.unwrap_or(default_size) * scale,
                weight: text.weight.unwrap_or(400),
                italic: text.italic.unwrap_or(false),
            },
            color: text.color,
            background: n.style.background.as_deref().and_then(parse_color),
            border_color: n.style.border_color.as_deref().and_then(parse_color),
            accent_color: n.style.accent_color.as_deref().and_then(parse_color),
            border_width: n.style.border_width.map(|w| w * scale),
            radius: n.style.radius.and_then(|r| match r {
                Rounding::Radius(v) => Some(v * scale),
                Rounding::Full => None,
            }),
            round_full: n.style.radius == Some(Rounding::Full),
        },
    );
    for child in &n.children {
        resolve_styles(child, &text, scale, logical_width, out);
    }
}
fn text(n: &Node, values: &Bindings) -> String {
    n.bind_text
        .as_ref()
        .and_then(|name| match values.get(name) {
            Some(Value::Text(v)) => Some(v.clone()),
            _ => None,
        })
        .or_else(|| n.text.clone())
        .or_else(|| n.label.clone())
        .unwrap_or_default()
}
fn enabled(n: &Node, values: &Bindings) -> bool {
    n.enabled
        && n.bind_enabled
            .as_ref()
            .is_none_or(|name| matches!(values.get(name), Some(Value::Bool(true))))
}
fn intersection(a: Rect, b: Rect) -> Option<Rect> {
    let [x, y, w, h] = a.bounds();
    let [bx, by, bw, bh] = b.bounds();
    let l = x.max(bx);
    let t = y.max(by);
    Rect::new(l, t, (x + w).min(bx + bw) - l, (y + h).min(by + bh) - t)
}
type MeasureMemo = BTreeMap<(String, u32, u32), [f32; 2]>;
fn intrinsic(
    n: &Node,
    avail: [f32; 2],
    v: &Bindings,
    m: &mut impl TextMetrics,
    s: f32,
    memo: &mut MeasureMemo,
    styles: &BTreeMap<String, ResolvedStyle>,
) -> [f32; 2] {
    let key = (n.id.clone(), avail[0].to_bits(), avail[1].to_bits());
    if let Some(size) = memo.get(&key) {
        return *size;
    }
    let limit = [
        n.width.map_or(avail[0], |w| (w * s).min(avail[0])),
        n.height.map_or(avail[1], |h| (h * s).min(avail[1])),
    ];
    let [top, right, bottom, left] = styles[&n.id].layout.padding;
    let inset_size = [left + right, top + bottom];
    let content = [
        (limit[0] - inset_size[0]).max(0.1),
        (limit[1] - inset_size[1]).max(0.1),
    ];
    let style = &styles[&n.id].text;
    let mut size = match n.kind {
        Kind::Text => m.measure_styled(&text(n, v), style, content[0]),
        Kind::Button | Kind::Checkbox | Kind::Toggle => {
            let [w, h] = m.measure_styled(&text(n, v), style, 1_000_000.);
            let leading = match n.kind {
                Kind::Checkbox => 36.,
                Kind::Toggle => 60.,
                _ => 28.,
            };
            [w + leading * s, (38. * s).max(h + 16. * s)]
        }
        Kind::Badge => {
            let [w, h] = m.measure_styled(&text(n, v), style, (content[0] - 20. * s).max(0.1));
            [w + 20. * s, h + 10. * s]
        }
        Kind::Separator => [content[0], s],
        Kind::Spacer => [0., 0.],
        Kind::ZStack => {
            let mut result = [0_f32; 2];
            for child in &n.children {
                let size = intrinsic(child, content, v, m, s, memo, styles);
                for axis in 0..2 {
                    result[axis] = result[axis].max(size[axis]);
                }
            }
            [result[0] + inset_size[0], result[1] + inset_size[1]]
        }
        _ => {
            let (sizes, gap) = child_sizes(n, content, v, m, s, memo, styles);
            let axis = styles[&n.id].layout.axis;
            let mut result = [0_f32; 2];
            for size in sizes {
                result[axis] += size[axis];
                result[1 - axis] = result[1 - axis].max(size[1 - axis]);
            }
            result[axis] += gap * n.children.len().saturating_sub(1) as f32;
            [result[0] + inset_size[0], result[1] + inset_size[1]]
        }
    };
    if n.width.is_some() {
        size[0] = limit[0];
    }
    if n.height.is_some() {
        size[1] = limit[1];
    }
    let result = [size[0].max(0.).min(avail[0]), size[1].max(0.).min(avail[1])];
    memo.insert(key, result);
    result
}
fn child_sizes(
    n: &Node,
    available: [f32; 2],
    v: &Bindings,
    m: &mut impl TextMetrics,
    s: f32,
    memo: &mut MeasureMemo,
    styles: &BTreeMap<String, ResolvedStyle>,
) -> (Vec<[f32; 2]>, f32) {
    let axis = styles[&n.id].layout.axis;
    let gap = styles[&n.id]
        .layout
        .gap
        .min(available[axis] / n.children.len().max(1) as f32);
    let room = (available[axis] - gap * n.children.len().saturating_sub(1) as f32).max(0.);
    let mut sizes: Vec<_> = n
        .children
        .iter()
        .map(|child| intrinsic(child, available, v, m, s, memo, styles))
        .collect();
    let natural: f32 = sizes.iter().map(|size| size[axis]).sum();
    let total_flex: f32 = n.children.iter().map(|child| child.flex).sum();
    for (size, child) in sizes.iter_mut().zip(&n.children) {
        if natural > room && natural > 0. {
            size[axis] *= room / natural;
        } else if total_flex > 0. {
            size[axis] += (room - natural) * child.flex / total_flex;
        }
        if axis == 0 {
            let measured = intrinsic(child, [size[0], available[1]], v, m, s, memo, styles);
            size[1] = measured[1];
        }
    }
    (sizes, gap)
}
#[allow(clippy::too_many_arguments)]
fn place(
    n: &Node,
    area: Rect,
    ancestor: Rect,
    v: &Bindings,
    m: &mut impl TextMetrics,
    s: f32,
    out: &mut Vec<LayoutBox>,
    memo: &mut MeasureMemo,
    styles: &BTreeMap<String, ResolvedStyle>,
) {
    let [x, y, aw, ah] = area.bounds();
    let w = n.width.map_or(aw, |w| (w * s).min(aw));
    let h = n.height.map_or(ah, |h| (h * s).min(ah));
    let Some(bounds) = Rect::new(x, y, w, h) else {
        return;
    };
    let Some(clip) = intersection(bounds, ancestor) else {
        return;
    };
    let mut resolved = styles[&n.id].clone();
    if resolved.round_full {
        resolved.radius = Some(w.min(h) * 0.5);
    }
    out.push(LayoutBox {
        id: n.id.clone(),
        kind: n.kind,
        bounds,
        clip,
        enabled: enabled(n, v),
        checked: checked(n, v),
        text_style: styles[&n.id].text.clone(),
        style: resolved,
        text: text(n, v),
        action: n
            .on_activate
            .clone()
            .or_else(|| n.on_toggle_changed.clone()),
        tone: n.tone,
        primary: n.primary,
    });
    let [top, right, bottom, left] = styles[&n.id].layout.padding;
    let start = [x + left, y + top];
    let available = [(w - (left + right)).max(0.), (h - (top + bottom)).max(0.)];
    if available.contains(&0.) {
        return;
    }
    let child_clip = Rect::new(start[0], start[1], available[0], available[1])
        .and_then(|r| intersection(r, clip));
    let Some(child_clip) = child_clip else { return };
    if n.kind == Kind::ZStack {
        let fractions = n.flow.map_or([0.5, 0.5], |flow| flow.fractions());
        for child in &n.children {
            let mut size = intrinsic(child, available, v, m, s, memo, styles);
            if n.align == Align::Stretch {
                if child.width.is_none() {
                    size[0] = available[0];
                }
                if child.height.is_none() {
                    size[1] = available[1];
                }
            }
            let pos = [
                start[0] + (available[0] - size[0]).max(0.) * fractions[0],
                start[1] + (available[1] - size[1]).max(0.) * fractions[1],
            ];
            if let Some(r) = Rect::new(pos[0], pos[1], size[0], size[1]) {
                place(child, r, child_clip, v, m, s, out, memo, styles);
            }
        }
        return;
    }
    let axis = styles[&n.id].layout.axis;
    let cross = 1 - axis;
    let (sizes, gap) = child_sizes(n, available, v, m, s, memo, styles);
    let occupied = sizes.iter().map(|size| size[axis]).sum::<f32>()
        + gap * n.children.len().saturating_sub(1) as f32;
    let placement = n.flow.map(|flow| flow.fractions());
    let mut cursor = start[axis]
        + placement.map_or(0., |fraction| {
            (available[axis] - occupied).max(0.) * fraction[axis]
        });
    for (child, mut size) in n.children.iter().zip(sizes) {
        if n.align == Align::Stretch
            || (child.kind == Kind::Spacer
                && if cross == 0 {
                    child.width.is_none()
                } else {
                    child.height.is_none()
                })
        {
            size[cross] = available[cross];
        }
        // Columns stretch container/text by default to provide useful responsive layouts.
        if axis == 1
            && n.align == Align::Start
            && n.flow.is_none()
            && child.width.is_none()
            && child.kind != Kind::Badge
        {
            size[cross] = available[cross];
        }
        let offset = if let Some(fraction) = placement {
            (available[cross] - size[cross]).max(0.) * fraction[cross]
        } else {
            match n.align {
                Align::Center => (available[cross] - size[cross]) * 0.5,
                Align::End => available[cross] - size[cross],
                _ => 0.,
            }
        };
        let mut pos = start;
        pos[axis] = cursor;
        pos[cross] += offset;
        if let Some(r) = Rect::new(pos[0], pos[1], size[0], size[1]) {
            place(child, r, child_clip, v, m, s, out, memo, styles);
        }
        cursor += size[axis] + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Metrics(usize);
    impl TextMetrics for Metrics {
        fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
            self.0 += 1;
            let w = text.chars().count() as f32 * size * 0.5;
            [
                w.min(width),
                (w / width.max(0.1)).ceil().max(1.) * size * 1.2,
            ]
        }
    }
    const SOURCE: &str = r#"{"schema_version":1,"root":{"type":"column","id":"root","padding":10,"gap":10,"children":[{"type":"text","id":"status","bind_text":"status"},{"type":"button","id":"save","label":"Save","on_activate":"save"},{"type":"button","id":"disabled","label":"Disabled","enabled":false,"on_activate":"save"},{"type":"button","id":"reset","label":"Reset","on_activate":"reset","bind_enabled":"ready"}]}}"#;
    fn setup() -> (Runtime, Handlers<'static, usize>, Bindings) {
        let mut h = Handlers::default();
        h.on_activate("save", |c: &mut usize| *c += 1).unwrap();
        h.on_activate("reset", |c: &mut usize| *c = 0).unwrap();
        let mut v = Bindings::default();
        v.set_text("status", "Ready");
        v.set_bool("ready", true);
        (
            Runtime::new(Document::parse(SOURCE).unwrap(), &h, &v).unwrap(),
            h,
            v,
        )
    }
    fn center(r: &Runtime, id: &str) -> [f32; 2] {
        let [x, y, w, h] = r.boxes.iter().find(|b| b.id == id).unwrap().bounds.bounds();
        [x + w * 0.5, y + h * 0.5]
    }
    #[test]
    fn dpi_layout_hits_and_capture_cancellation() {
        for scale in [1., 1.5, 2., 3.] {
            let (mut r, mut h, v) = setup();
            r.layout(
                Viewport::new(400. * scale, 400. * scale, scale).unwrap(),
                &v,
                &mut Metrics(0),
            )
            .unwrap();
            let [x, y] = center(&r, "save");
            let mut count = 0;
            r.pointer_move(x, y);
            assert!(r.pointer_down(x, y));
            assert!(r.button_state("save").pressed);
            r.pointer_up(-1., -1., &mut h, &mut count).unwrap();
            assert_eq!(count, 0);
            r.pointer_down(x, y);
            r.cancel_input();
            r.pointer_up(x, y, &mut h, &mut count).unwrap();
            assert_eq!(count, 0);
            r.pointer_down(x, y);
            r.pointer_up(x, y, &mut h, &mut count).unwrap();
            assert_eq!(count, 1);
            let [x, y] = center(&r, "disabled");
            r.pointer_down(x, y);
            r.pointer_up(x, y, &mut h, &mut count).unwrap();
            assert_eq!(count, 1);
            for b in r.boxes() {
                let [x, y, w, h] = b.bounds.bounds();
                let [cx, cy, cw, ch] = b.clip.bounds();
                assert!(cx >= x && cy >= y && cx + cw <= x + w + 0.01 && cy + ch <= y + h + 0.01);
            }
        }
    }
    #[test]
    fn focus_skips_disabled_and_keyboard_repeats_do_not_activate_twice() {
        let (mut r, mut h, v) = setup();
        r.layout(Viewport::new(400., 400., 1.).unwrap(), &v, &mut Metrics(0))
            .unwrap();
        let mut count = 0;
        r.focus_next(false);
        assert_eq!(r.focused(), Some("save"));
        assert!(r.key_down());
        assert!(!r.key_down());
        r.key_up(&mut h, &mut count).unwrap();
        r.key_up(&mut h, &mut count).unwrap();
        assert_eq!(count, 1);
        r.focus_next(false);
        assert_eq!(r.focused(), Some("reset"));
        r.focus_next(false);
        assert_eq!(r.focused(), Some("save"));
        r.focus_next(true);
        assert_eq!(r.focused(), Some("reset"));
        r.key_down();
        r.focus_next(false);
        r.key_up(&mut h, &mut count).unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn binding_updates_disable_focus_and_layout_is_cached() {
        let (mut r, _, mut v) = setup();
        let mut m = Metrics(0);
        let view = Viewport::new(400., 400., 1.).unwrap();
        assert!(r.layout(view, &v, &mut m).unwrap());
        let calls = m.0;
        assert!(!r.layout(view, &v, &mut m).unwrap());
        assert_eq!(calls, m.0);
        r.focus_next(true);
        assert_eq!(r.focused(), Some("reset"));
        v.set_bool("ready", false);
        v.set_text("status", "Changed");
        r.layout(view, &v, &mut m).unwrap();
        assert_eq!(r.focused(), None);
        assert!(r.button_state("reset").disabled);
        assert_eq!(
            r.boxes.iter().find(|b| b.id == "status").unwrap().text,
            "Changed"
        );
        v.set_text("ready", "wrong");
        assert!(r.layout(view, &v, &mut m).is_err());
    }
    #[test]
    fn reload_is_atomic_and_preserves_compatible_focus() {
        let (mut r, h, v) = setup();
        let view = Viewport::new(400., 400., 1.).unwrap();
        r.layout(view, &v, &mut Metrics(0)).unwrap();
        r.focus_next(false);
        let len = r.boxes.len();
        assert!(r.reload("{", &h, &v).is_err());
        assert_eq!(r.boxes.len(), len);
        assert_eq!(r.focused(), Some("save"));
        assert!(r
            .reload(&SOURCE.replace("\"save\"", "\"missing\""), &h, &v)
            .is_err());
        assert_eq!(r.boxes.len(), len);
        r.reload(SOURCE, &h, &v).unwrap();
        r.layout(view, &v, &mut Metrics(0)).unwrap();
        assert_eq!(r.focused(), Some("save"));
        assert!(!r.button_state("save").pressed);
    }
    #[test]
    fn narrowed_row_remeasures_wrapped_content() {
        let source=serde_json::json!({"schema_version":1,"root":{"type":"column","id":"root","children":[{"type":"row","id":"row","children":[{"type":"panel","id":"left","padding":10,"flex":1,"children":[{"type":"text","id":"left_text","text":"a".repeat(40),"font_size":10}]},{"type":"panel","id":"right","padding":10,"flex":1,"children":[{"type":"text","id":"right_text","text":"a".repeat(40),"font_size":10}]}]}]}}).to_string();
        let h = Handlers::<()>::default();
        let v = Bindings::default();
        let mut r = Runtime::new(Document::parse(&source).unwrap(), &h, &v).unwrap();
        r.layout(Viewport::new(300., 200., 1.).unwrap(), &v, &mut Metrics(0))
            .unwrap();
        let b = r.boxes.iter().find(|b| b.id == "left_text").unwrap();
        let [_, _, w, height] = b.bounds.bounds();
        assert_eq!(w, 130.);
        assert!(height >= 24., "{height}");
        assert!(
            r.boxes
                .iter()
                .find(|b| b.id == "row")
                .unwrap()
                .bounds
                .bounds()[3]
                >= 44.
        );
    }
    #[test]
    fn fixed_width_container_measures_before_height() {
        let source = r#"{"schema_version":1,"root":{"type":"column","id":"root","children":[{"type":"panel","id":"fixed","width":100,"children":[{"type":"text","id":"text","text":"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz","font_size":10}]}]}}"#;
        let h = Handlers::<()>::default();
        let v = Bindings::default();
        let mut r = Runtime::new(Document::parse(source).unwrap(), &h, &v).unwrap();
        r.layout(Viewport::new(800., 600., 1.).unwrap(), &v, &mut Metrics(0))
            .unwrap();
        assert_eq!(
            r.boxes
                .iter()
                .find(|b| b.id == "text")
                .unwrap()
                .bounds
                .bounds()[3],
            36.
        );
    }
    #[test]
    fn invalid_scaled_viewport_does_not_panic() {
        let (mut r, _, v) = setup();
        for scale in [f32::MIN_POSITIVE, 0., 10.] {
            assert!(r
                .layout(
                    Viewport {
                        width: 100.,
                        height: 100.,
                        scale,
                        top: 0.
                    },
                    &v,
                    &mut Metrics(0)
                )
                .is_err());
        }
    }
}

#[cfg(test)]
mod style_component_tests {
    use super::*;
    #[derive(Clone, Debug, PartialEq)]
    enum Shape {
        Rectangle([f32; 4], [f32; 4]),
        Rounded([f32; 4], f32, [f32; 4]),
        Triangle([[f32; 2]; 3], [f32; 4]),
    }
    #[derive(Clone, Debug, PartialEq)]
    struct Glyph(String, TextStyle, [f32; 4]);
    struct Recorder;
    impl TextMetrics for Recorder {
        fn measure(&mut self, text: &str, size: f32, max_width: f32) -> [f32; 2] {
            [
                ((text.chars().count() as f32) * size * 0.5).min(max_width),
                size * 1.2,
            ]
        }
    }
    impl Painter for Recorder {
        type Shape = Shape;
        type Glyph = Glyph;
        fn viewport(&self) -> Viewport {
            Viewport::new(500., 500., 1.).unwrap()
        }
        fn draw_rect(&self, out: &mut Vec<Shape>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
            out.push(Shape::Rectangle([x, y, w, h], color));
        }
        fn card_round(&self, out: &mut Vec<Shape>, r: [f32; 4], radius: f32, color: [f32; 4]) {
            out.push(Shape::Rounded(r, radius, color));
        }
        fn card_triangle(&self, out: &mut Vec<Shape>, points: [[f32; 2]; 3], color: [f32; 4]) {
            out.push(Shape::Triangle(points, color));
        }
        fn card_text(&mut self, _: &mut Vec<Glyph>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
        fn draw_text_colored(
            &mut self,
            _: &mut Vec<Glyph>,
            _: &str,
            _: f32,
            _: f32,
            _: f32,
            _: f32,
            _: &dyn Fn(usize) -> [f32; 4],
        ) {
        }
    }
    impl DocumentPainter for Recorder {
        fn ui_text(
            &mut self,
            out: &mut Vec<Glyph>,
            text: &str,
            _: [f32; 4],
            size: f32,
            color: [f32; 4],
        ) {
            out.push(Glyph(
                text.into(),
                TextStyle {
                    size,
                    ..TextStyle::default()
                },
                color,
            ));
        }
        fn ui_text_styled(
            &mut self,
            out: &mut Vec<Glyph>,
            text: &str,
            _: [f32; 4],
            style: &TextStyle,
            color: [f32; 4],
        ) {
            out.push(Glyph(text.into(), style.clone(), color));
        }
    }
    fn center(r: &Runtime, id: &str) -> [f32; 2] {
        let [x, y, w, h] = r
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .bounds
            .bounds();
        [x + w * 0.5, y + h * 0.5]
    }
    #[test]
    fn typography_inherits_and_local_overrides_take_precedence() {
        let source = r##"{"schema_version":1,"root":{"type":"column","id":"root","style":{"font_family":"serif","font_size":18,"font_weight":600,"font_style":"italic","color":"#aabbcc80","background":"#101820"},"children":[{"type":"text","id":"body","text":"Inherited"},{"type":"text","id":"override","text":"Override","font_size":24,"style":{"font_size":20,"font_style":"normal"}},{"type":"button","id":"button","label":"Apply","on_activate":"apply","style":{"font_weight":700,"color":"#ffffff","radius":12,"background":"#003366","border_color":"#abcdef","border_width":2}},{"type":"panel","id":"panel","height":40}]}}"##;
        let mut h = Handlers::<()>::default();
        h.on_activate("apply", |_| {}).unwrap();
        let v = Bindings::default();
        let mut r = Runtime::new(Document::parse(source).unwrap(), &h, &v).unwrap();
        r.layout(Viewport::new(1000., 1000., 2.).unwrap(), &v, &mut Recorder)
            .unwrap();
        let body = r.boxes().iter().find(|b| b.id == "body").unwrap();
        assert_eq!(
            body.text_style,
            TextStyle {
                family: "serif".into(),
                size: 36.,
                weight: 600,
                italic: true
            }
        );
        let local = r.boxes().iter().find(|b| b.id == "override").unwrap();
        assert_eq!(local.text_style.size, 40.);
        assert!(!local.text_style.italic);
        let button = r.boxes().iter().find(|b| b.id == "button").unwrap();
        assert_eq!(button.text_style.weight, 700);
        assert_eq!(button.style.radius, Some(24.));
        assert_eq!(
            r.boxes()
                .iter()
                .find(|b| b.id == "panel")
                .unwrap()
                .style
                .background,
            None
        );
        let paint = r.paint(&mut Recorder, &Theme::dark());
        assert!(
            matches!(paint[0].shapes[0], Shape::Rounded(_, _, [a, b, c, d]) if a == 16./255. && b == 24./255. && c == 32./255. && d == 1.)
        );
        assert_eq!(paint[1].glyphs[0].1, body.text_style);
        assert_eq!(
            paint[1].glyphs[0].2,
            [170. / 255., 187. / 255., 204. / 255., 128. / 255.]
        );
        let button_batch = &paint[r.boxes().iter().position(|b| b.id == "button").unwrap()];
        assert!(button_batch
            .shapes
            .iter()
            .any(|s| matches!(s, Shape::Rounded(_, radius, _) if *radius == 24.)));
        assert_eq!(button_batch.glyphs[0].2, [1.; 4]);
    }
    #[test]
    fn old_documents_keep_default_typography_and_styled_trait_defaults() {
        let source = r#"{"schema_version":1,"root":{"type":"column","id":"root","children":[{"type":"text","id":"text","text":"Body"},{"type":"button","id":"button","label":"Apply","on_activate":"apply"}]}}"#;
        let mut h = Handlers::<()>::default();
        h.on_activate("apply", |_| {}).unwrap();
        let v = Bindings::default();
        let mut r = Runtime::new(Document::parse(source).unwrap(), &h, &v).unwrap();
        r.layout(Viewport::new(500., 500., 1.).unwrap(), &v, &mut Recorder)
            .unwrap();
        assert_eq!(
            r.boxes()
                .iter()
                .find(|b| b.id == "text")
                .unwrap()
                .text_style,
            TextStyle::default()
        );
        assert_eq!(
            r.boxes()
                .iter()
                .find(|b| b.id == "button")
                .unwrap()
                .text_style
                .size,
            12.
        );
        let custom = TextStyle {
            family: "mono".into(),
            size: 30.,
            weight: 900,
            italic: true,
        };
        assert_eq!(
            Recorder.measure_styled("test", &custom, 500.),
            Recorder.measure("test", 30., 500.)
        );
    }
    #[test]
    fn checkbox_and_toggle_emit_bool_without_mutating_host_bindings() {
        let source = r#"{"schema_version":1,"root":{"type":"column","id":"root","gap":10,"children":[{"type":"checkbox","id":"checkbox","label":"Check","bind_checked":"checked","on_toggle_changed":"change"},{"type":"toggle","id":"toggle","label":"Toggle","checked":true,"on_toggle_changed":"change"},{"type":"checkbox","id":"disabled","label":"Disabled","checked":false,"enabled":false,"on_toggle_changed":"change"},{"type":"badge","id":"badge","text":"Beta"},{"type":"separator","id":"separator"}]}}"#;
        let mut h = Handlers::default();
        h.on_toggle_changed("change", |events: &mut Vec<bool>, value| events.push(value))
            .unwrap();
        let mut values = Bindings::default();
        values.set_bool("checked", false);
        let mut r = Runtime::new(Document::parse(source).unwrap(), &h, &values).unwrap();
        let view = Viewport::new(500., 500., 1.).unwrap();
        r.layout(view, &values, &mut Recorder).unwrap();
        let mut events = vec![];
        let [x, y] = center(&r, "checkbox");
        r.pointer_down(x, y);
        assert!(r.button_state("checkbox").pressed);
        r.pointer_up(x, y, &mut h, &mut events).unwrap();
        assert_eq!(events, [true]);
        assert_eq!(values.get("checked"), Some(&Value::Bool(false)));
        assert!(
            !r.boxes()
                .iter()
                .find(|b| b.id == "checkbox")
                .unwrap()
                .checked
        );
        values.set_bool("checked", true);
        r.layout(view, &values, &mut Recorder).unwrap();
        assert!(
            r.boxes()
                .iter()
                .find(|b| b.id == "checkbox")
                .unwrap()
                .checked
        );
        r.focus_next(false);
        assert_eq!(r.focused(), Some("toggle"));
        r.key_down();
        assert!(!r.key_down());
        r.key_up(&mut h, &mut events).unwrap();
        assert_eq!(events, [true, false]);
        r.focus_next(false);
        assert_eq!(r.focused(), Some("checkbox"));
        let [x, y] = center(&r, "disabled");
        r.pointer_down(x, y);
        r.pointer_up(x, y, &mut h, &mut events).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            r.boxes()
                .iter()
                .find(|b| b.id == "separator")
                .unwrap()
                .bounds
                .bounds()[3],
            1.
        );
        assert!(
            r.boxes()
                .iter()
                .find(|b| b.id == "badge")
                .unwrap()
                .bounds
                .bounds()[2]
                < 500.
        );
    }
    #[test]
    fn style_radius_matches_hit_routing_and_cancellation_applies_to_checkables() {
        let source = r##"{"schema_version":1,"root":{"type":"toggle","id":"toggle","label":"Rounded","on_toggle_changed":"change","width":100,"height":40,"style":{"radius":20,"background":"#223344"}}}"##;
        let mut h = Handlers::default();
        h.on_toggle_changed("change", |events: &mut Vec<bool>, value| events.push(value))
            .unwrap();
        let values = Bindings::default();
        let mut events = vec![];
        let mut r = Runtime::new(Document::parse(source).unwrap(), &h, &values).unwrap();
        r.layout(
            Viewport::new(500., 500., 1.).unwrap(),
            &values,
            &mut Recorder,
        )
        .unwrap();
        assert!(!r.pointer_down(0., 0.));
        r.pointer_up(0., 0., &mut h, &mut events).unwrap();
        assert!(events.is_empty());
        let [x, y] = center(&r, "toggle");
        r.pointer_down(x, y);
        r.cancel_input();
        r.pointer_up(x, y, &mut h, &mut events).unwrap();
        assert!(events.is_empty());
        r.focus_next(false);
        r.key_down();
        r.cancel_input();
        r.key_up(&mut h, &mut events).unwrap();
        assert!(events.is_empty());
    }
    #[test]
    fn local_accent_colors_style_checked_controls_and_focus_without_inheriting() {
        let source = r##"{"schema_version":1,"root":{"type":"column","id":"root","style":{"accent_color":"#ff0000"},"children":[{"type":"checkbox","id":"check","label":"Checked","checked":true,"on_toggle_changed":"change","style":{"accent_color":"#a7b8fa"}},{"type":"toggle","id":"toggle","label":"Toggle","checked":true,"on_toggle_changed":"change","style":{"accent_color":"#85c8b3"}},{"type":"button","id":"button","label":"Focus","on_activate":"activate"}]}}"##;
        let mut h = Handlers::<()>::default();
        h.on_toggle_changed("change", |_, _| {}).unwrap();
        h.on_activate("activate", |_| {}).unwrap();
        let v = Bindings::default();
        let mut r = Runtime::new(Document::parse(source).unwrap(), &h, &v).unwrap();
        r.layout(Viewport::new(500., 500., 1.).unwrap(), &v, &mut Recorder)
            .unwrap();
        r.focus_next(false);
        let paint = r.paint(&mut Recorder, &Theme::dark());
        let blue = [167. / 255., 184. / 255., 250. / 255., 1.];
        let green = [133. / 255., 200. / 255., 179. / 255., 1.];
        assert!(paint[1]
            .shapes
            .iter()
            .any(|s| matches!(s, Shape::Rounded(_, _, c) if *c == blue)));
        assert!(paint[1]
            .shapes
            .iter()
            .any(|s| matches!(s, Shape::Triangle(_, c) if *c == blue)));
        assert!(paint[2]
            .shapes
            .iter()
            .any(|s| matches!(s, Shape::Rounded(_, _, c) if *c == green)));
        assert_eq!(
            r.boxes()
                .iter()
                .find(|b| b.id == "button")
                .unwrap()
                .style
                .accent_color,
            None
        );
    }
}
