use super::document::{parse_color, FontSlant};
use super::{
    effects::{Effects, Shadow, VisualStyle},
    motion::{Motion, ResolvedShadow, Visual},
};
use super::{
    Align, Bindings, Document, DocumentError, Event, Handlers, Kind, Node, Rounding, Tone, Value,
};
use crate::graphics::{BackgroundImage, Canvas, Clip, ClipRegion, VectorPath};
use crate::{Button, ButtonColors, Painter, Panel, Rect, SurfaceStyle, Theme, UiTheme, Viewport};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

#[cfg(feature = "rust-ui")]
pub(super) mod accessibility;
#[cfg(feature = "rust-ui")]
pub(super) mod editing;
#[cfg(feature = "rust-ui")]
mod overlays;
#[cfg(feature = "rust-ui")]
pub(super) mod value_controls;
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
    /// Caret advances at valid UTF-8 byte offsets on one unwrapped line.
    /// Shaping adapters can override with cluster positions; legacy adapters use
    /// their own shared text measurement, never character-count pixel estimates.
    fn line_advances(&mut self, text: &str, style: &TextStyle, byte_offsets: &[usize]) -> Vec<f32> {
        byte_offsets
            .iter()
            .map(|at| self.measure_styled(text.get(..*at).unwrap_or(""), style, 1_000_000.)[0])
            .collect()
    }
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
    /// Optional subtree group-compositing hooks. Legacy adapters ignore opacity;
    /// adapters supporting effects override both hooks.
    fn ui_layer_begin(&mut self, _out: &mut Vec<Self::Shape>, _opacity: f32) {}
    fn ui_layer_end(&mut self, _out: &mut Vec<Self::Shape>) {}
    /// Optional surface capabilities; old adapters remain source compatible.
    fn ui_clip_begin(&mut self, _out: &mut Vec<Self::Shape>, _clip: ClipRegion) {}
    fn ui_clip_end(&mut self, _out: &mut Vec<Self::Shape>) {}
    fn ui_image(
        &mut self,
        _out: &mut Vec<Self::Shape>,
        _bounds: [f32; 4],
        _image: &BackgroundImage,
        _scale: f32,
    ) {
    }
    fn ui_path(
        &mut self,
        _out: &mut Vec<Self::Shape>,
        _path: &VectorPath,
        _color: [f32; 4],
        _stroke: Option<f32>,
    ) {
    }

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
/// Navigation within a Rust tab/segmented group containing the focused control.
/// Other controls ignore these keys. Disabled/unlaid-out options are skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKey {
    Previous,
    Next,
    First,
    Last,
}
fn selection_group<'a>(node: &'a Node, focused: &str) -> Option<&'a Node> {
    if node.selection_group && node.children.iter().any(|child| child.id == focused) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| selection_group(child, focused))
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
    pub(crate) editor: Option<super::document::EditorOptions>,
    #[cfg(feature = "rust-ui")]
    pub(crate) on_text_changed: Option<String>,
    #[cfg(feature = "rust-ui")]
    pub(crate) on_submit: Option<String>,
    pub(crate) slider: Option<super::document::SliderProps>,
    pub(crate) tone: Tone,
    pub(crate) primary: bool,
    /// Horizontal label placement for buttons (fractions 0..1); centred if None.
    pub(crate) label_flow: Option<super::Flow>,
    pub(crate) disclosure: Option<bool>,
    pub(crate) effects: Effects,
    pub(crate) depth: usize,
    ancestor_clip: Rect,
    effect_clip: Rect,
    clip_regions: Vec<ClipRegion>,
    content_clip: Option<ClipRegion>,
    image: Option<BackgroundImage>,
    canvas: Canvas,
    pub scroll_max: [f32; 2],
    pub scroll_offset: [f32; 2],
    pub scroll: Option<super::Scroll>,
    pub scrollbars: super::Scrollbars,
    layout_visible: bool,
    #[cfg_attr(not(feature = "rust-ui"), allow(dead_code))]
    pub(crate) split: Option<super::document::SplitProps>,
    pub(crate) split_handle: Option<String>,
    #[cfg_attr(not(feature = "rust-ui"), allow(dead_code))]
    pub(crate) virtual_list: Option<super::document::VirtualListProps>,
    #[cfg_attr(not(feature = "rust-ui"), allow(dead_code))]
    pub(crate) progress: Option<super::document::ProgressProps>,
    pub(crate) detached_visual: Option<Visual>,
    pub(crate) detached_mount: f32,
    pub(crate) departing: bool,
    pub(crate) detached_state: Option<ButtonState>,
}
#[derive(Clone)]
struct ScrollbarCapture {
    id: String,
    axis: usize,
    grab: f32,
}
struct ScrollbarGeometry {
    track: [f32; 4],
    thumb: [f32; 4],
    start: f32,
    length: f32,
    travel: f32,
}
fn scrollbar_geometry(b: &LayoutBox, s: f32, axis: usize) -> Option<ScrollbarGeometry> {
    if b.scrollbars == super::Scrollbars::Hidden || b.scroll_max[axis] <= 0. {
        return None;
    }
    let [x, y, w, h] = b.bounds.bounds();
    let viewport = if axis == 0 { w } else { h };
    let track = (viewport - 12. * s).max(0.);
    let length = (track * viewport / (viewport + b.scroll_max[axis] * s))
        .max(16. * s)
        .min(track);
    let travel = track - length;
    if travel <= 0. {
        return None;
    }
    let start = if axis == 0 { x + 6. * s } else { y + 6. * s };
    let offset = travel * b.scroll_offset[axis] / b.scroll_max[axis];
    let (lane, thumb) = if axis == 0 {
        (
            [x + 6. * s, y + h - 9. * s, track, 8. * s],
            [start + offset, y + h - 6. * s, length, 3. * s],
        )
    } else {
        (
            [x + w - 9. * s, y + 6. * s, 8. * s, track],
            [x + w - 6. * s, start + offset, 3. * s, length],
        )
    };
    Some(ScrollbarGeometry {
        track: lane,
        thumb,
        start,
        length,
        travel,
    })
}
/// Shared document layout and event state; owns no window or background tasks.
pub struct Runtime {
    document: Document,
    boxes: Vec<LayoutBox>,
    layout_boxes: Vec<LayoutBox>,
    scrollbar_capture: Option<ScrollbarCapture>,
    focused: Option<String>,
    hovered: Option<String>,
    visual_hover: BTreeSet<String>,
    capture: Option<String>,
    keyboard: Option<String>,
    scale: f32,
    cache: Option<([f32; 3], Bindings)>,
    last_viewport: Option<[f32; 3]>,
    motion: RefCell<Motion>,
    scroll_offsets: BTreeMap<String, [f32; 2]>,
    #[cfg(feature = "rust-ui")]
    editors: RefCell<BTreeMap<String, editing::EditorState>>,
    #[cfg(feature = "rust-ui")]
    ime_target: Option<String>,
    #[cfg(feature = "rust-ui")]
    ime_cancelled: bool,
    #[cfg(feature = "rust-ui")]
    slider_capture: Option<String>,
    #[cfg(feature = "rust-ui")]
    slider_values: BTreeMap<String, f32>,
    #[cfg(feature = "rust-ui")]
    split_capture: Option<value_controls::SplitCapture>,
    #[cfg(feature = "rust-ui")]
    split_values: BTreeMap<String, f32>,
    #[cfg(feature = "rust-ui")]
    progress_clock: value_controls::ProgressClock,
    #[cfg(feature = "rust-ui")]
    overlays: overlays::OverlayState,
}
impl Runtime {
    pub fn new<C>(
        document: Document,
        handlers: &Handlers<'_, C>,
        values: &Bindings,
    ) -> Result<Self, DocumentError> {
        document.validate_connections(handlers, values)?;
        let runtime = Self {
            document,
            boxes: vec![],
            layout_boxes: vec![],
            scrollbar_capture: None,
            focused: None,
            hovered: None,
            visual_hover: BTreeSet::new(),
            capture: None,
            keyboard: None,
            scale: 1.,
            cache: None,
            last_viewport: None,
            motion: RefCell::new(Motion::default()),
            scroll_offsets: BTreeMap::new(),
            #[cfg(feature = "rust-ui")]
            editors: RefCell::new(BTreeMap::new()),
            #[cfg(feature = "rust-ui")]
            ime_target: None,
            #[cfg(feature = "rust-ui")]
            ime_cancelled: false,
            #[cfg(feature = "rust-ui")]
            slider_capture: None,
            #[cfg(feature = "rust-ui")]
            slider_values: BTreeMap::new(),
            #[cfg(feature = "rust-ui")]
            split_capture: None,
            #[cfg(feature = "rust-ui")]
            split_values: BTreeMap::new(),
            #[cfg(feature = "rust-ui")]
            progress_clock: value_controls::ProgressClock::default(),
            #[cfg(feature = "rust-ui")]
            overlays: overlays::OverlayState::default(),
        };
        #[cfg(feature = "rust-ui")]
        let mut runtime = runtime;
        #[cfg(feature = "rust-ui")]
        {
            runtime.reconcile_editors();
            runtime.sync_overlay_state();
        }
        Ok(runtime)
    }
    /// Host-provided monotonic clock; never calls application handlers or layout.
    pub fn advance_animations(&mut self, now: Duration) -> bool {
        let changed = self.motion.get_mut().advance(now);
        #[cfg(feature = "rust-ui")]
        let changed = self.advance_progress(now) || changed;
        changed
    }
    pub fn is_animating(&self) -> bool {
        let active = self.motion.borrow().active();
        #[cfg(feature = "rust-ui")]
        let active = self.progress_active() || active;
        active
    }
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        #[cfg(feature = "rust-ui")]
        self.set_progress_reduced(reduced);
        let m = self.motion.get_mut();
        m.reduced = reduced;
        if reduced {
            m.settle();
        }
    }
    pub fn settle_animations(&mut self) {
        self.motion.get_mut().settle();
    }
    /// Draggable header regions never steal input from interactive controls,
    /// including disabled controls, or from a later painted non-header subtree.
    pub fn drag_region_hit(&self, x: f32, y: f32) -> bool {
        #[cfg(feature = "rust-ui")]
        if self.overlay_active() {
            return false;
        }
        if self.boxes.iter().any(|b| {
            interactive_box(b)
                && (!self.portal_passthrough(&b.id))
                && b.clip.contains(x, y)
                && b.clip_regions.iter().all(|r| r.contains(x, y))
                && b.content_clip.as_ref().is_none_or(|r| r.contains(x, y))
                && b.bounds.contains_corners(b.radii(self.scale), x, y)
        }) {
            return false;
        }
        let Some(mut index) = self.boxes.iter().rposition(|b| {
            (!self.portal_passthrough(&b.id))
                && b.clip.contains(x, y)
                && b.clip_regions.iter().all(|r| r.contains(x, y))
                && b.content_clip.as_ref().is_none_or(|r| r.contains(x, y))
                && b.bounds.contains_corners(b.radii(self.scale), x, y)
        }) else {
            return false;
        };
        if interactive(self.boxes[index].kind) {
            return false;
        }
        loop {
            let b = &self.boxes[index];
            if b.effects.drag_region {
                return true;
            }
            if b.depth == 0 {
                break;
            }
            let depth = b.depth - 1;
            let Some(parent) = self.boxes[..index].iter().rposition(|b| b.depth == depth) else {
                break;
            };
            index = parent;
        }
        false
    }
    fn portal_passthrough(&self, id: &str) -> bool {
        #[cfg(feature = "rust-ui")]
        {
            self.portal_hit_passthrough(id)
        }
        #[cfg(not(feature = "rust-ui"))]
        {
            let _ = id;
            false
        }
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
                    old.kind == new.kind && interactive_node(new) && enabled(new, values)
                }
                _ => false,
            }
        });
        if !keep_focus {
            self.focused = None;
        }
        fn keys(n: &Node, ids: &mut BTreeSet<String>) {
            ids.insert(n.id.clone());
            for c in &n.children {
                keys(c, ids);
            }
        }
        let mut ids = BTreeSet::new();
        keys(next.root(), &mut ids);
        self.scroll_offsets.retain(|id, _| ids.contains(id));
        self.motion.get_mut().remove(&self.boxes, &ids);
        #[cfg(feature = "rust-ui")]
        let slider_capture = self.slider_capture.clone();
        #[cfg(feature = "rust-ui")]
        let split_capture = self.split_capture.clone();
        #[cfg(feature = "rust-ui")]
        let composition = self.editor_composition_snapshot(&next);
        let scrollbar_capture = self.scrollbar_capture.clone();
        self.document = next;
        #[cfg(feature = "rust-ui")]
        {
            self.reconcile_editors();
            self.slider_values.clear();
            self.split_values.clear();
            self.sync_overlay_state();
        }
        self.cache = None;
        self.boxes.clear();
        self.layout_boxes.clear();
        self.cancel_input();
        if let Some(capture) = scrollbar_capture {
            if find(self.document.root(), &capture.id).is_some_and(|n| {
                n.surface.scroll.is_some()
                    && n.surface.scrollbars != super::Scrollbars::Hidden
                    && enabled(n, values)
            }) && self.scope_allows_id(&capture.id)
            {
                self.scrollbar_capture = Some(capture);
            }
        }
        #[cfg(feature = "rust-ui")]
        if let Some(composition) = composition {
            self.restore_editor_composition(composition);
        }
        #[cfg(feature = "rust-ui")]
        if let Some(id) = slider_capture {
            if find(self.document.root(), &id)
                .is_some_and(|n| n.slider.is_some() && enabled(n, values))
                && self.scope_allows_id(&id)
            {
                self.slider_capture = Some(id);
            }
        }
        #[cfg(feature = "rust-ui")]
        if let Some(capture) = split_capture {
            if find(self.document.root(), &capture.handle).is_some_and(|n| {
                n.split_handle.as_deref() == Some(&capture.owner) && enabled(n, values)
            }) && self.scope_allows_id(&capture.handle)
            {
                self.split_capture = Some(capture);
            }
        }
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
        if self.last_viewport.is_some_and(|old| old != key) {
            self.cancel_input();
        }
        self.last_viewport = Some(key);
        self.scale = viewport.scale;
        #[cfg(feature = "rust-ui")]
        self.prepare_overlay_layout(viewport);
        let mut boxes = vec![];
        let mut memo = BTreeMap::new();
        let mut styles = BTreeMap::new();
        resolve_styles(
            self.document.root(),
            &InheritedStyle::default(),
            self.scale,
            viewport.width / self.scale,
            &mut styles,
            0,
        );
        let area = Rect::new(0., 0., viewport.width, viewport.height).unwrap();
        place(
            self.document.root(),
            area,
            area,
            area,
            &[],
            &mut self.scroll_offsets,
            values,
            metrics,
            self.scale,
            &mut boxes,
            &mut memo,
            &styles,
            [false; 2],
            true,
        );
        #[cfg(feature = "rust-ui")]
        for node in self.visible_overlay_nodes() {
            if node.id == self.document.root().id {
                continue;
            }
            let first = boxes.len();
            place(
                &node,
                area,
                area,
                area,
                &[],
                &mut self.scroll_offsets,
                values,
                metrics,
                self.scale,
                &mut boxes,
                &mut memo,
                &styles,
                [false; 2],
                true,
            );
            for b in &mut boxes[first..] {
                b.depth = b.depth.saturating_sub(styles[&node.id].depth);
            }
        }
        self.layout_boxes = boxes;
        self.boxes = self
            .layout_boxes
            .iter()
            .filter(|b| b.layout_visible)
            .cloned()
            .collect();
        #[cfg(feature = "rust-ui")]
        self.layout_editors(metrics);
        self.motion.get_mut().reconcile(&self.boxes);
        let valid = |id: &Option<String>| {
            id.as_ref().is_none_or(|id| {
                self.layout_boxes
                    .iter()
                    .any(|b| b.id == *id && interactive_box(b) && b.enabled)
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
        self.visual_hover.clear();
        #[cfg(feature = "rust-ui")]
        self.finish_overlay_layout();
        #[cfg(feature = "rust-ui")]
        if self.take_tooltip_layout_change() {
            self.cache = None;
            self.layout(viewport, values, metrics)?;
            return Ok(true);
        }
        self.cache = Some((key, values.clone()));
        Ok(true)
    }
    /// Reveal the focused control from the innermost scroll viewport outward.
    /// All mounted controls participate, even when currently clipped from painting.
    fn reveal_focus(&mut self, id: &str) -> bool {
        let Some(mut index) = self.layout_boxes.iter().position(|b| b.id == id) else {
            return false;
        };
        let mut changed = false;
        let mut target = self.layout_boxes[index].bounds.bounds();
        while self.layout_boxes[index].depth > 0 {
            let depth = self.layout_boxes[index].depth - 1;
            let Some(parent) = self.layout_boxes[..index]
                .iter()
                .rposition(|b| b.depth == depth)
            else {
                break;
            };
            index = parent;
            let b = &self.layout_boxes[index];
            if !self.scope_allows_id(&b.id) {
                break;
            }
            let Some(mode) = b.scroll else { continue };
            let [x, y, w, h] = b.bounds.bounds();
            let [top, right, bottom, left] = b.style.layout.padding;
            let origin = [x + left, y + top];
            let extent = [(w - left - right).max(0.), (h - top - bottom).max(0.)];
            let offset = self
                .scroll_offsets
                .entry(b.id.clone())
                .or_insert(b.scroll_offset);
            for (axis, enabled) in mode.axes().into_iter().enumerate() {
                if !enabled {
                    continue;
                }
                let shift = if target[axis] < origin[axis] {
                    target[axis] - origin[axis]
                } else if target[axis] + target[axis + 2] > origin[axis] + extent[axis] {
                    // Oversized controls align their leading edge rather than oscillate.
                    target[axis] - origin[axis]
                        + (target[axis + 2].min(extent[axis]) - extent[axis])
                } else {
                    0.
                };
                let old = offset[axis];
                offset[axis] = (old + shift / self.scale).clamp(0., b.scroll_max[axis]);
                target[axis] -= (offset[axis] - old) * self.scale;
                if old != offset[axis] {
                    self.cache = None;
                    changed = true;
                }
            }
        }
        changed
    }
    fn scrollbar_pointer_down(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let hit = self.boxes.iter().enumerate().rev().find_map(|(index, b)| {
            if b.scrollbars == super::Scrollbars::Hidden
                || !self.scope_allows_id(&b.id)
                || !b.enabled
                || !b.clip.contains(x, y)
                || !b.clip_regions.iter().all(|r| r.contains(x, y))
            {
                return None;
            }
            (0..2).find_map(|axis| {
                let geometry = scrollbar_geometry(b, self.scale, axis)?;
                // The 8px interaction lane surrounds the visible 3px thumb.
                let [tx, ty, tw, th] = geometry.track;
                if x >= tx && y >= ty && x <= tx + tw && y <= ty + th {
                    let subtree_end = self.boxes[index + 1..]
                        .iter()
                        .position(|next| next.depth <= b.depth)
                        .map_or(self.boxes.len(), |i| index + 1 + i);
                    if self.boxes[subtree_end..].iter().any(|next| {
                        !self.portal_passthrough(&next.id)
                            && next.clip.contains(x, y)
                            && next.bounds.contains(x, y)
                            && next.clip_regions.iter().all(|r| r.contains(x, y))
                    }) {
                        return None;
                    }
                    let position = if axis == 0 { x } else { y };
                    let thumb_start = geometry.thumb[axis];
                    let thumb_end = thumb_start + geometry.thumb[axis + 2];
                    Some((
                        b.id.clone(),
                        axis,
                        if position >= thumb_start && position <= thumb_end {
                            position - thumb_start
                        } else {
                            geometry.length * 0.5
                        },
                    ))
                } else {
                    None
                }
            })
        });
        let Some((id, axis, grab)) = hit else {
            return false;
        };
        self.cancel_input();
        self.scrollbar_capture = Some(ScrollbarCapture { id, axis, grab });
        self.scrollbar_pointer_move(x, y);
        true
    }
    fn scrollbar_pointer_move(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(capture) = self.scrollbar_capture.as_ref() else {
            return false;
        };
        let Some(b) = self.boxes.iter().find(|b| b.id == capture.id && b.enabled) else {
            self.scrollbar_capture = None;
            return false;
        };
        let Some(g) = scrollbar_geometry(b, self.scale, capture.axis) else {
            self.scrollbar_capture = None;
            return false;
        };
        let axis = capture.axis;
        let position = if axis == 0 { x } else { y };
        let value =
            ((position - g.start - capture.grab) / g.travel).clamp(0., 1.) * b.scroll_max[axis];
        let offset = self
            .scroll_offsets
            .entry(capture.id.clone())
            .or_insert(b.scroll_offset);
        let changed = offset[axis] != value;
        offset[axis] = value;
        if changed {
            self.invalidate_layout();
        }
        changed
    }
    #[cfg(feature = "rust-ui")]
    pub fn scroll_outcome<C>(
        &mut self,
        x: f32,
        y: f32,
        delta: [f32; 2],
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let changed = self.scroll_at(x, y, delta);
        let activated = if changed {
            self.dispatch_virtual_windows(handlers, context)?
        } else {
            false
        };
        Ok(InputOutcome { changed, activated })
    }

    /// Logical scroll offsets survive keyed recomposition and clamp on layout.
    pub fn scroll_offset(&self, id: &str) -> [f32; 2] {
        self.scroll_offsets.get(id).copied().unwrap_or([0.; 2])
    }
    /// Pointer is physical pixels; delta is logical pixels, positive toward end.
    /// Unconsumed deltas bubble from the topmost scroll view to its ancestors.
    pub fn scroll_at(&mut self, x: f32, y: f32, mut delta: [f32; 2]) -> bool {
        if !delta.iter().all(|v| v.is_finite() && v.abs() <= 16_384.) {
            return false;
        }
        #[cfg(feature = "rust-ui")]
        let mut changed = self.editor_scroll_at(x, y, &mut delta);
        #[cfg(not(feature = "rust-ui"))]
        let mut changed = false;
        let Some(mut index) = self.boxes.iter().rposition(|b| {
            self.scope_allows_id(&b.id)
                && !b.effects.scroll_passthrough
                && (!self.portal_passthrough(&b.id))
                && b.clip.contains(x, y)
                && b.clip_regions.iter().all(|r| r.contains(x, y))
                && b.content_clip.as_ref().is_none_or(|r| r.contains(x, y))
                && b.bounds.contains_corners(b.radii(self.scale), x, y)
        }) else {
            return changed;
        };
        loop {
            let b = &self.boxes[index];
            if !self.scope_allows_id(&b.id) {
                break;
            }
            if let Some(mode) = b.scroll {
                let axes = mode.axes();
                let offset = self
                    .scroll_offsets
                    .entry(b.id.clone())
                    .or_insert(b.scroll_offset);
                for axis in 0..2 {
                    if axes[axis] {
                        let old = offset[axis];
                        offset[axis] = (old + delta[axis]).clamp(0., b.scroll_max[axis]);
                        delta[axis] -= offset[axis] - old;
                        changed |= old != offset[axis];
                    }
                }
            }
            if self.boxes[index].depth == 0 {
                break;
            }
            let depth = self.boxes[index].depth - 1;
            let Some(parent) = self.boxes[..index].iter().rposition(|b| b.depth == depth) else {
                break;
            };
            index = parent;
        }
        if changed {
            self.invalidate_layout();
            self.cancel_input();
        }
        changed
    }
    fn hit(&self, x: f32, y: f32) -> Option<String> {
        self.boxes
            .iter()
            .rev()
            .find(|b| {
                self.scope_allows_id(&b.id)
                    && interactive_box(b)
                    && b.enabled
                    && (!self.portal_passthrough(&b.id))
                    && b.clip.contains(x, y)
                    && b.clip_regions.iter().all(|r| r.contains(x, y))
                    && b.content_clip.as_ref().is_none_or(|r| r.contains(x, y))
                    && b.bounds.contains_corners(b.radii(self.scale), x, y)
            })
            .map(|b| b.id.clone())
    }
    fn visual_hover_at(&self, x: f32, y: f32, hit: Option<&str>) -> BTreeSet<String> {
        let mut result = BTreeSet::new();
        let index = hit
            .and_then(|id| self.boxes.iter().position(|b| b.id == id))
            .or_else(|| {
                self.boxes.iter().rposition(|b| {
                    (!self.portal_passthrough(&b.id))
                        && b.clip.contains(x, y)
                        && b.clip_regions.iter().all(|r| r.contains(x, y))
                        && b.content_clip.as_ref().is_none_or(|r| r.contains(x, y))
                        && b.bounds.contains_corners(b.radii(self.scale), x, y)
                })
            });
        let Some(mut index) = index else {
            return result;
        };
        loop {
            let b = &self.boxes[index];
            let s = &b.effects.states[0];
            if b.enabled
                && !interactive_box(b)
                && (s.color.is_some()
                    || s.background.is_some()
                    || s.border_color.is_some()
                    || s.border_width.is_some()
                    || s.opacity.is_some()
                    || s.shadow.is_some()
                    || s.glow.is_some())
            {
                result.insert(b.id.clone());
            }
            if b.depth == 0 {
                break;
            }
            let depth = b.depth - 1;
            let Some(parent) = self.boxes[..index].iter().rposition(|b| b.depth == depth) else {
                break;
            };
            index = parent;
        }
        result
    }
    /// Full input adapter for controls which dispatch continuously (sliders).
    #[cfg(feature = "rust-ui")]
    pub fn pointer_move_outcome<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if self.scrollbar_capture.is_some() {
            let changed = self.scrollbar_pointer_move(x, y);
            let activated = self.dispatch_virtual_windows(handlers, context)?;
            return Ok(InputOutcome { changed, activated });
        }
        if let Some(outcome) = self.split_pointer_move(x, y, handlers, context)? {
            return Ok(outcome);
        }
        if let Some(outcome) = self.slider_pointer_move(x, y, handlers, context)? {
            return Ok(outcome);
        }
        Ok(InputOutcome {
            changed: self.pointer_move(x, y),
            activated: false,
        })
    }
    #[cfg(feature = "rust-ui")]
    pub fn pointer_down_outcome<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        self.cancel_pointer_captures();
        if let Some(changed) = self.overlay_pointer_down(x, y) {
            return Ok(InputOutcome {
                changed,
                activated: false,
            });
        }
        if self.scrollbar_pointer_down(x, y) {
            let activated = self.dispatch_virtual_windows(handlers, context)?;
            return Ok(InputOutcome {
                changed: true,
                activated,
            });
        }
        if let Some(outcome) = self.split_pointer_down(x, y, handlers, context)? {
            self.sync_tooltip_visibility();
            return Ok(outcome);
        }
        if let Some(outcome) = self.slider_pointer_down(x, y, handlers, context)? {
            self.sync_tooltip_visibility();
            return Ok(outcome);
        }
        Ok(InputOutcome {
            changed: self.pointer_down(x, y),
            activated: false,
        })
    }
    pub fn pointer_move(&mut self, x: f32, y: f32) -> bool {
        if self.scrollbar_capture.is_some() {
            return self.scrollbar_pointer_move(x, y);
        }
        #[cfg(feature = "rust-ui")]
        let editing_changed = self.editor_pointer_drag(x, y);
        #[cfg(not(feature = "rust-ui"))]
        let editing_changed = false;
        let next = self.hit(x, y);
        let visual = self.visual_hover_at(x, y, next.as_deref());
        let changed = editing_changed || next != self.hovered || visual != self.visual_hover;
        self.hovered = next;
        self.visual_hover = visual;
        #[cfg(feature = "rust-ui")]
        let changed = self.tooltip_pointer(Some([x, y])) || changed;
        changed
    }
    pub fn pointer_leave(&mut self) -> bool {
        let changed = self.hovered.take().is_some() || !self.visual_hover.is_empty();
        self.visual_hover.clear();
        #[cfg(feature = "rust-ui")]
        let changed = self.tooltip_pointer(None) || changed;
        changed
    }
    pub fn pointer_down(&mut self, x: f32, y: f32) -> bool {
        if self.scrollbar_pointer_down(x, y) {
            return true;
        }
        #[cfg(feature = "rust-ui")]
        self.ime_cancel();
        #[cfg(feature = "rust-ui")]
        {
            if let Some(changed) = self.overlay_pointer_down(x, y) {
                return changed;
            }
            if self.editor_pointer_down(x, y, false) {
                return true;
            }
        }
        let hover_changed = self.pointer_move(x, y);
        let hit = self.hit(x, y);
        let changed = hover_changed
            || self.capture != hit
            || self.focused != hit
            || self.hovered != hit
            || self.keyboard.is_some();
        self.capture = hit.clone();
        self.hovered = hit.clone();
        self.focused = hit;
        self.keyboard = None;
        #[cfg(feature = "rust-ui")]
        self.sync_tooltip_visibility();
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
        #[cfg(feature = "rust-ui")]
        {
            if let Some(outcome) = self.overlay_pointer_up(x, y, handlers, context)? {
                return Ok(outcome);
            }
            if self.editor_pointer_up() {
                return Ok(InputOutcome {
                    changed: true,
                    activated: false,
                });
            }
            if self.split_pointer_up() {
                return Ok(InputOutcome {
                    changed: true,
                    activated: false,
                });
            }
            if self.slider_pointer_up() {
                return Ok(InputOutcome {
                    changed: true,
                    activated: false,
                });
            }
        }
        if self.scrollbar_capture.take().is_some() {
            return Ok(InputOutcome {
                changed: true,
                activated: false,
            });
        }
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
        #[cfg(feature = "rust-ui")]
        if self.overlay_key(if backwards {
            SelectionKey::Previous
        } else {
            SelectionKey::Next
        }) {
            return true;
        }
        #[cfg(feature = "rust-ui")]
        {
            self.ime_cancel();
            self.editor_pointer_up();
            self.slider_capture = None;
            self.split_capture = None;
            self.cancel_overlay_capture();
        }
        self.capture = None;
        self.keyboard = None;
        self.scrollbar_capture = None;
        let ids: Vec<_> = self
            .layout_boxes
            .iter()
            .filter(|b| self.scope_allows_id(&b.id) && interactive_box(b) && b.enabled)
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
        self.reveal_focus(&ids[index]);
        #[cfg(feature = "rust-ui")]
        self.sync_tooltip_visibility();
        true
    }
    fn cancel_pointer_captures(&mut self) -> bool {
        let changed = self.capture.take().is_some() | self.scrollbar_capture.take().is_some();
        #[cfg(feature = "rust-ui")]
        let changed = changed
            | self.slider_capture.take().is_some()
            | self.split_capture.take().is_some()
            | self.editor_pointer_up()
            | self.cancel_overlay_capture();
        changed
    }
    pub fn key_down(&mut self) -> bool {
        self.cancel_pointer_captures();
        #[cfg(feature = "rust-ui")]
        if self.editing_focus() {
            return false;
        }
        if self.keyboard.is_some() {
            return false;
        }
        self.capture = None;
        self.keyboard = self.focused.clone();
        self.keyboard.is_some()
    }
    /// Select on arrow/Home/End press; the host recomposes after `activated`.
    /// This does not change ordinary button or Tab/Shift-Tab traversal behavior.
    pub fn selection_key<C>(
        &mut self,
        key: SelectionKey,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let Some(focused) = self.focused.as_deref() else {
            return Ok(InputOutcome::default());
        };
        let Some(group) = selection_group(self.document.root(), focused) else {
            return Ok(InputOutcome::default());
        };
        let ids: Vec<_> = group
            .children
            .iter()
            .filter(|node| {
                self.layout_boxes.iter().any(|b| {
                    self.scope_allows_id(&b.id)
                        && b.id == node.id
                        && b.kind == Kind::Button
                        && b.enabled
                })
            })
            .map(|node| node.id.clone())
            .collect();
        let Some(index) = ids.iter().position(|id| id == focused) else {
            return Ok(InputOutcome::default());
        };
        let next = match key {
            SelectionKey::Previous => (index + ids.len() - 1) % ids.len(),
            SelectionKey::Next => (index + 1) % ids.len(),
            SelectionKey::First => 0,
            SelectionKey::Last => ids.len() - 1,
        };
        let id = &ids[next];
        let cancelled = self.cancel_pointer_captures();
        let changed = cancelled
            || self.focused.as_ref() != Some(id)
            || self.capture.is_some()
            || self.keyboard.is_some();
        self.capture = None;
        self.keyboard = None;
        self.focused = Some(id.clone());
        let changed = self.reveal_focus(id) || changed;
        #[cfg(feature = "rust-ui")]
        self.sync_tooltip_visibility();
        let activated = self.activate(id, handlers, context)?;
        Ok(InputOutcome { changed, activated })
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
        let changed = self.scrollbar_capture.take().is_some()
            || self.capture.is_some()
            || self.keyboard.is_some()
            || self.hovered.is_some()
            || !self.visual_hover.is_empty();
        self.capture = None;
        self.keyboard = None;
        self.hovered = None;
        self.visual_hover.clear();
        #[cfg(feature = "rust-ui")]
        {
            let slider_changed =
                self.slider_capture.take().is_some() | self.split_capture.take().is_some();
            let editing_changed =
                self.editor_pointer_up() | self.ime_cancel() | self.cancel_overlay_capture();
            changed || slider_changed || editing_changed
        }
        #[cfg(not(feature = "rust-ui"))]
        changed
    }
    fn activate<C>(
        &self,
        id: &str,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        if !self.scope_allows_id(id) {
            return Ok(false);
        }
        if let Some(b) = self.layout_boxes.iter().find(|b| b.id == id && b.enabled) {
            if b.checked
                && b.kind == Kind::Button
                && selection_group(self.document.root(), id).is_some()
            {
                return Ok(false);
            }
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
        self.paint_ui(p, &UiTheme::from_legacy(theme))
    }
    /// Semantic tokens resolve on paint: changing theme needs no recomposition.
    pub fn paint_ui<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
    ) -> Vec<PaintBatch<P::Shape, P::Glyph>> {
        let mut out = Vec::new();
        let mut motion = self.motion.borrow_mut();
        let boxes = motion.paint_boxes(&self.boxes);
        self.paint_nodes(
            p,
            ui,
            boxes.as_deref().unwrap_or(&self.boxes),
            &mut motion,
            &mut out,
        );
        out
    }
    #[allow(clippy::too_many_arguments)]
    fn paint_nodes<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
        boxes: &[LayoutBox],
        motion: &mut Motion,
        out: &mut Vec<PaintBatch<P::Shape, P::Glyph>>,
    ) {
        let mut groups: Vec<(usize, bool, bool, Option<LayoutBox>)> = Vec::new();
        for source in boxes {
            let mut themed = source.clone();
            themed.style.resolve_colors(ui);
            let original = &themed;
            while groups.last().is_some_and(|g| g.0 >= original.depth) {
                let (_, clip, layer, scroll) = groups.pop().unwrap();
                if let Some(b) = scroll {
                    let mut shapes = vec![];
                    paint_scrollbars(p, &mut shapes, &b, ui, self.scale);
                    out.push(PaintBatch {
                        clip: b.clip,
                        shapes,
                        glyphs: vec![],
                    });
                }
                let mut shapes = vec![];
                if clip {
                    p.ui_clip_end(&mut shapes);
                }
                if layer {
                    p.ui_layer_end(&mut shapes);
                }
                out.push(PaintBatch {
                    clip: original.effect_clip,
                    shapes,
                    glyphs: vec![],
                });
            }
            let state = self.state_for_box(original);
            let target = visual_target(original, state, ui, self.scale);
            let flags = [
                state.hovered && !state.disabled,
                state.pressed && !state.disabled,
                state.focused && !state.disabled,
            ];
            let visual = if original.departing {
                motion.exit_value(original, target)
            } else {
                motion.value(
                    &original.id,
                    original.kind,
                    target,
                    flags,
                    &original.effects,
                )
            };
            if visual.opacity < 1. {
                let mut shapes = vec![];
                p.ui_layer_begin(&mut shapes, visual.opacity);
                out.push(PaintBatch {
                    clip: original.effect_clip,
                    shapes,
                    glyphs: vec![],
                });
            }
            let mut shapes = vec![];
            for shadow in [visual.shadow, visual.glow].into_iter().flatten() {
                if shadow.color[3] > 0. {
                    p.card_shadow(
                        &mut shapes,
                        original.bounds.bounds(),
                        original.radii(self.scale),
                        shadow.blur,
                        shadow.spread,
                        shadow.offset,
                        shadow.color,
                    );
                }
            }
            if !shapes.is_empty() {
                out.push(PaintBatch {
                    clip: original.effect_clip,
                    shapes,
                    glyphs: vec![],
                });
            }
            if let Some(clip) = &original.content_clip {
                let mut shapes = vec![];
                p.ui_clip_begin(&mut shapes, clip.clone());
                out.push(PaintBatch {
                    clip: original.ancestor_clip,
                    shapes,
                    glyphs: vec![],
                });
            }
            if visual.opacity < 1. || original.content_clip.is_some() {
                groups.push((
                    original.depth,
                    original.content_clip.is_some(),
                    visual.opacity < 1.,
                    original.scroll.map(|_| original.clone()),
                ));
            }
            let treatment = has_visual_patch(&original.effects)
                || !ui.is_legacy()
                || original.style.background_opacity < 1.;
            let mut styled;
            let b = if treatment {
                styled = original.clone();
                styled.style.color = Some(visual.color);
                styled.style.background = Some(visual.background);
                styled.style.border_color = Some(visual.border);
                styled.style.border_width = Some(visual.width);
                &styled
            } else {
                original
            };
            let batch = self.paint_one(p, ui, b, treatment.then_some(visual));
            #[cfg(feature = "rust-ui")]
            let mut batch = batch;
            #[cfg(feature = "rust-ui")]
            if b.editor.is_some() && !batch.glyphs.is_empty() {
                let glyphs = std::mem::take(&mut batch.glyphs);
                out.push(batch);
                let [x, y, w, h] = self.editor_inner(b);
                if let Some(inner) = Rect::new(x, y, w, h).and_then(|r| intersection(r, b.clip)) {
                    out.push(PaintBatch {
                        clip: inner,
                        shapes: Vec::new(),
                        glyphs,
                    });
                }
                continue;
            }
            out.push(batch);
        }
        if let Some(b) = boxes.last() {
            for (_, clip, layer, scroll) in groups.into_iter().rev() {
                if let Some(b) = scroll {
                    let mut shapes = vec![];
                    paint_scrollbars(p, &mut shapes, &b, ui, self.scale);
                    out.push(PaintBatch {
                        clip: b.clip,
                        shapes,
                        glyphs: vec![],
                    });
                }
                let mut shapes = vec![];
                if clip {
                    p.ui_clip_end(&mut shapes);
                }
                if layer {
                    p.ui_layer_end(&mut shapes);
                }
                out.push(PaintBatch {
                    clip: b.ancestor_clip,
                    shapes,
                    glyphs: vec![],
                });
            }
        }
    }
    fn paint_one<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
        b: &LayoutBox,
        visual: Option<Visual>,
    ) -> PaintBatch<P::Shape, P::Glyph> {
        #[cfg(feature = "rust-ui")]
        if b.progress.is_some() {
            return self.paint_progress(p, ui, b);
        }
        #[cfg(feature = "rust-ui")]
        if b.split_handle.is_some() {
            return self.paint_split(p, ui, b);
        }
        #[cfg(feature = "rust-ui")]
        if b.editor.is_some() {
            return self.paint_editor(p, ui, b);
        }
        #[cfg(feature = "rust-ui")]
        if b.slider.is_some() {
            let mut shapes = Vec::new();
            value_controls::paint_slider(
                p,
                &mut shapes,
                b,
                ui,
                self.state_for_box(b),
                self.scale,
                self.slider_paint_value(b),
            );
            return PaintBatch {
                clip: b.clip,
                shapes,
                glyphs: Vec::new(),
            };
        }
        let theme = &ui.legacy_theme();
        let c = ui.colors();
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
                        b.radius_spec(self.scale),
                        b.style.background.unwrap_or(c.panel),
                    );
                    paint_border(
                        p,
                        &mut shapes,
                        r,
                        b.radius_spec(self.scale),
                        b.style.border_width.unwrap_or(self.scale),
                        b.style.border_color.unwrap_or(c.border),
                    );
                } else if let Some(panel) =
                    Panel::new(b.bounds, 12., self.scale, SurfaceStyle::from_theme(theme))
                {
                    panel.paint(p, &mut shapes);
                }
            }
            Kind::Row | Kind::Column | Kind::Stack | Kind::ZStack | Kind::Text | Kind::Canvas => {
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
                let surface =
                    visual.map_or_else(|| state_surface(base, state, theme), |v| v.background);
                let radius = b.radius_spec(self.scale);
                // Primary labels pick black or white for contrast with the
                // actual fill, as `Palette::with_accent` does for themes.
                let mut color = b.style.color.unwrap_or(if b.primary {
                    crate::theme::contrast(base)
                } else {
                    c.text
                });
                if state.disabled && visual.is_none() {
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
                // A disclosure chevron takes the trailing edge; its label
                // starts at the leading edge like a native pop-up button.
                let chevron = if b.disclosure.is_some() {
                    18. * self.scale
                } else {
                    0.
                };
                if let Some(open) = b.disclosure {
                    let s = self.scale;
                    let (cx, cy) = (r[0] + r[2] - inset - 5. * s, r[1] + r[3] * 0.5);
                    let dy = if open { -2. * s } else { 2. * s };
                    let path = VectorPath::new()
                        .move_to(cx - 4. * s, cy - dy)
                        .line_to(cx, cy + dy)
                        .line_to(cx + 4. * s, cy - dy);
                    let mut ink = color;
                    ink[3] *= 0.75;
                    p.ui_path(&mut shapes, &path, ink, Some(1.5 * s));
                }
                let max_width = (r[2] - 2. * inset - chevron).max(0.1);
                let [w, h] = p.measure_styled(&b.text, &b.text_style, max_width);
                let default_along = if b.disclosure.is_some() { 0. } else { 0.5 };
                let along = b
                    .label_flow
                    .map_or(default_along, |flow| flow.fractions()[0]);
                p.ui_text_styled(
                    &mut glyphs,
                    &b.text,
                    [
                        r[0] + inset + ((r[2] - 2. * inset - w) * along).max(0.),
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
                let radius = b.radius_spec(self.scale);
                if let Some(background) = b.style.background {
                    paint_surface(
                        p,
                        &mut shapes,
                        r,
                        radius,
                        visual.map_or_else(
                            || state_surface(background, state, theme),
                            |v| v.background,
                        ),
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
                    paint_border(p, &mut shapes, track, control_h * 0.5, self.scale, border);
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
                let radius = b.radius_spec(self.scale);
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
            Kind::Spacer => {
                paint_overrides(p, &mut shapes, r, &b.style, self.scale);
            }
            Kind::Separator => paint_surface(
                p,
                &mut shapes,
                r,
                b.radius_spec(self.scale),
                b.style
                    .background
                    .or(b.style.color)
                    .or(b.style.border_color)
                    .unwrap_or(c.line),
            ),
        }
        if let Some(image) = &b.image {
            let border = b.style.border_width.unwrap_or(
                if b.kind == Kind::Panel || b.style.border_color.is_some() {
                    self.scale
                } else {
                    0.
                },
            );
            let inner = [
                r[0] + border,
                r[1] + border,
                (r[2] - 2. * border).max(0.),
                (r[3] - 2. * border).max(0.),
            ];
            if inner[2] > 0. && inner[3] > 0. {
                p.ui_clip_begin(
                    &mut shapes,
                    ClipRegion::Rounded {
                        bounds: inner,
                        radii: b.radii(self.scale).map(|v| (v - border).max(0.)),
                    },
                );
                let mut image = image.clone();
                image.opacity *= b.style.background_opacity;
                p.ui_image(&mut shapes, r, &image, self.scale);
                p.ui_clip_end(&mut shapes);
            }
        }
        for path in &b.canvas.paths {
            let physical = path
                .path
                .clone()
                .transformed([self.scale, 0., 0., self.scale, r[0], r[1]]);
            if let Some(color) = ui.resolve(&path.color) {
                p.ui_path(
                    &mut shapes,
                    &physical,
                    color,
                    path.stroke.map(|w| w * self.scale),
                );
            }
        }
        PaintBatch {
            clip: b.clip,
            shapes,
            glyphs,
        }
    }
    fn state_for_box(&self, b: &LayoutBox) -> ButtonState {
        if let Some(state) = b.detached_state {
            return state;
        }
        ButtonState {
            hovered: self.hovered.as_deref() == Some(&b.id) || self.visual_hover.contains(&b.id),
            pressed: (self.capture.as_deref() == Some(&b.id)
                && self.hovered.as_deref() == Some(&b.id))
                || self.keyboard.as_deref() == Some(&b.id),
            focused: self.focused.as_deref() == Some(&b.id),
            disabled: !b.enabled,
        }
    }
}
#[derive(Clone, Copy)]
enum RadiusSpec {
    Uniform(f32),
    Corners([f32; 4]),
}
impl From<f32> for RadiusSpec {
    fn from(v: f32) -> Self {
        Self::Uniform(v)
    }
}
impl LayoutBox {
    fn radius_spec(&self, scale: f32) -> RadiusSpec {
        self.style.corners.map_or(
            RadiusSpec::Uniform(self.style.radius.unwrap_or(
                match self.kind {
                    Kind::Panel => 12.,
                    Kind::Button => 7.,
                    Kind::Checkbox | Kind::Toggle => 6.,
                    Kind::Badge => 8.,
                    _ => 0.,
                } * scale,
            )),
            RadiusSpec::Corners,
        )
    }
    fn radii(&self, scale: f32) -> [f32; 4] {
        let radii = match self.radius_spec(scale) {
            RadiusSpec::Uniform(v) => [v; 4],
            RadiusSpec::Corners(v) => v,
        };
        let [_, _, w, h] = self.bounds.bounds();
        radii.map(|r| r.min(w * 0.5).min(h * 0.5))
    }
}
fn has_visual_patch(e: &Effects) -> bool {
    e.animation.is_some()
        || e.animations.iter().any(Option::is_some)
        || std::iter::once(&e.base).chain(&e.states).any(|s| {
            s.color.is_some()
                || s.background.is_some()
                || s.border_color.is_some()
                || s.border_width.is_some()
        })
}
fn resolved_shadow(shadow: &Shadow, scale: f32, ui: &UiTheme) -> ResolvedShadow {
    ResolvedShadow {
        color: ui.resolve(&shadow.color).unwrap(),
        blur: shadow.blur * scale,
        spread: shadow.spread * scale,
        offset: shadow.offset.map(|v| v * scale),
    }
}
fn apply_patch(v: &mut Visual, patch: &VisualStyle, scale: f32, ui: &UiTheme) {
    if let Some(c) = patch.color.as_deref().and_then(|v| ui.resolve(v)) {
        v.color = c;
    }
    if let Some(c) = patch.background.as_deref().and_then(|v| ui.resolve(v)) {
        v.background = c;
    }
    if let Some(c) = patch.border_color.as_deref().and_then(|v| ui.resolve(v)) {
        v.border = c;
        if patch.border_width.is_none() && v.width == 0. {
            v.width = scale;
        }
    }
    if let Some(w) = patch.border_width {
        v.width = w * scale;
    }
    if let Some(o) = patch.opacity {
        v.opacity = o;
    }
    if let Some(s) = &patch.shadow {
        v.shadow = Some(resolved_shadow(s, scale, ui));
    }
    if let Some(g) = &patch.glow {
        v.glow = Some(resolved_shadow(&g.0, scale, ui));
    }
}
fn visual_target(b: &LayoutBox, state: ButtonState, ui: &UiTheme, scale: f32) -> Visual {
    let theme = &ui.legacy_theme();
    let c = ui.colors();
    let base = b.style.background.unwrap_or(match b.kind {
        Kind::Button if b.primary => b.style.accent_color.unwrap_or(c.accent),
        Kind::Button | Kind::Badge => c.selected,
        Kind::Panel => c.panel,
        Kind::Text if b.editor.is_some() => c.panel,
        _ => [0.; 4],
    });
    let color = b
        .style
        .color
        .unwrap_or(if b.kind == Kind::Button && b.primary {
            ui.palette.on_primary
        } else {
            match b.tone {
                Tone::Muted => c.muted,
                Tone::Accent => c.accent,
                _ => c.text,
            }
        });
    let background = if matches!(b.kind, Kind::Checkbox | Kind::Toggle)
        && b.style.background.is_none()
        && (state.hovered || state.pressed)
    {
        let mut bg = c.selected;
        bg[3] = if state.pressed { 0.7 } else { 0.35 };
        bg
    } else if interactive_box(b) {
        state_surface(base, state, theme)
    } else {
        base
    };
    let mut v = Visual {
        color,
        background,
        border: b.style.border_color.unwrap_or(c.border),
        width: b.style.border_width.unwrap_or(
            if b.kind == Kind::Panel || b.style.border_color.is_some() {
                scale
            } else {
                0.
            },
        ),
        opacity: 1.,
        shadow: None,
        glow: None,
    };
    apply_patch(&mut v, &b.effects.base, scale, ui);
    if !state.disabled {
        for (i, active) in [(2, state.focused), (0, state.hovered), (1, state.pressed)] {
            if active {
                apply_patch(&mut v, &b.effects.states[i], scale, ui);
            }
        }
    }
    if b.kind == Kind::Button && state.disabled {
        v.color[3] *= 0.45;
    }
    v.background[3] *= b.style.background_opacity;
    v
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
    radius: impl Into<RadiusSpec>,
    color: [f32; 4],
) {
    if bounds[2] > 0. && bounds[3] > 0. && color[3] > 0. {
        match radius.into() {
            RadiusSpec::Uniform(r) => p.card_round(out, bounds, r, color),
            RadiusSpec::Corners(r) => p.card_corners(out, bounds, r, color),
        }
    }
}
fn paint_overrides<P: Painter>(
    p: &P,
    out: &mut Vec<P::Shape>,
    r: [f32; 4],
    style: &ResolvedStyle,
    scale: f32,
) {
    let radius = style.corners.map_or(
        RadiusSpec::Uniform(style.radius.unwrap_or(0.)),
        RadiusSpec::Corners,
    );
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
    radius: impl Into<RadiusSpec>,
    width: f32,
    color: [f32; 4],
) {
    let radius = match radius.into() {
        RadiusSpec::Uniform(r) => r,
        RadiusSpec::Corners(r) => {
            p.card_outline(out, [x, y, w, h], r, width, color);
            return;
        }
    };
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
fn paint_scrollbars<P: Painter>(
    p: &P,
    out: &mut Vec<P::Shape>,
    b: &LayoutBox,
    ui: &UiTheme,
    s: f32,
) {
    if b.scrollbars == super::Scrollbars::Hidden {
        return;
    }
    let mut color = ui.palette.muted;
    color[3] *= 0.65;
    for axis in 0..2 {
        if let Some(geometry) = scrollbar_geometry(b, s, axis) {
            p.card_round(out, geometry.thumb, 1.5 * s, color);
        }
    }
}
fn interactive_node(n: &Node) -> bool {
    interactive(n.kind) || n.editor.is_some() || n.slider.is_some() || n.split_handle.is_some()
}
fn interactive_box(b: &LayoutBox) -> bool {
    interactive(b.kind) || b.editor.is_some() || b.slider.is_some() || b.split_handle.is_some()
}
#[cfg(not(feature = "rust-ui"))]
impl Runtime {
    fn scope_allows_id(&self, _: &str) -> bool {
        true
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
    color_specs: [Option<String>; 4],
    background_opacity: f32,
    layout: ResolvedLayout,
    text: TextStyle,
    color: Option<[f32; 4]>,
    background: Option<[f32; 4]>,
    border_color: Option<[f32; 4]>,
    accent_color: Option<[f32; 4]>,
    border_width: Option<f32>,
    radius: Option<f32>,
    round_full: bool,
    corner_rules: Option<[Rounding; 4]>,
    corners: Option<[f32; 4]>,
    depth: usize,
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
    fn resolve_colors(&mut self, ui: &UiTheme) {
        let resolved = self
            .color_specs
            .each_ref()
            .map(|v| v.as_deref().and_then(|v| ui.resolve(v)));
        [
            self.color,
            self.background,
            self.border_color,
            self.accent_color,
        ] = resolved;
    }
    fn surface_override(&self) -> bool {
        self.background.is_some()
            || self.border_color.is_some()
            || self.border_width.is_some()
            || self.radius.is_some()
            || self.corners.is_some()
    }
}
#[derive(Clone, Default)]
struct InheritedStyle {
    family: Option<String>,
    size: Option<f32>,
    weight: Option<u16>,
    italic: Option<bool>,
    color: Option<String>,
}
fn resolve_styles(
    n: &Node,
    inherited: &InheritedStyle,
    scale: f32,
    logical_width: f32,
    out: &mut BTreeMap<String, ResolvedStyle>,
    depth: usize,
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
    text.color = n.style.color.clone().or(text.color);
    let default_size = if matches!(n.kind, Kind::Button | Kind::Badge) {
        12.
    } else {
        14.
    };
    out.insert(
        n.id.clone(),
        ResolvedStyle {
            color_specs: [
                text.color.clone(),
                n.style.background.clone(),
                n.style.border_color.clone(),
                n.style.accent_color.clone(),
            ],
            background_opacity: n.style.background_opacity.unwrap_or(1.),
            layout: ResolvedLayout::new(n, scale, logical_width),
            text: TextStyle {
                family: text.family.clone().unwrap_or_else(|| "system".into()),
                size: text.size.unwrap_or(default_size) * scale,
                weight: text.weight.unwrap_or(400),
                italic: text.italic.unwrap_or(false),
            },
            color: text.color.as_deref().and_then(parse_color),
            background: n.style.background.as_deref().and_then(parse_color),
            border_color: n.style.border_color.as_deref().and_then(parse_color),
            accent_color: n.style.accent_color.as_deref().and_then(parse_color),
            border_width: n.style.border_width.map(|w| w * scale),
            radius: n.style.radius.and_then(|r| match r {
                Rounding::Radius(v) => Some(v * scale),
                Rounding::Full => None,
            }),
            round_full: n.style.radius == Some(Rounding::Full),
            corner_rules: n.style.corners,
            corners: None,
            depth,
        },
    );
    for child in &n.children {
        resolve_styles(child, &text, scale, logical_width, out, depth + 1);
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
type MeasureMemo = BTreeMap<(String, u32, u32, [bool; 2]), [f32; 2]>;
/// Natural scroll sizing propagates through automatic-size descendants. Explicit
/// dimensions establish a bounded inner layout again (so spacers still fill a
/// fixed-height panel). Auto keeps the cross axis bounded for responsive flex.
fn natural_children(n: &Node, inherited: [bool; 2], main: usize) -> [bool; 2] {
    let mut axes = [
        inherited[0] && n.width.is_none(),
        inherited[1] && n.height.is_none(),
    ];
    if let Some(mode) = n.surface.scroll {
        let own = if mode == super::Scroll::Auto {
            let mut own = [false; 2];
            own[main] = true;
            own
        } else {
            mode.axes()
        };
        for a in 0..2 {
            axes[a] |= own[a];
        }
    }
    axes
}
#[allow(clippy::too_many_arguments)]
fn intrinsic(
    n: &Node,
    avail: [f32; 2],
    v: &Bindings,
    m: &mut impl TextMetrics,
    s: f32,
    memo: &mut MeasureMemo,
    styles: &BTreeMap<String, ResolvedStyle>,
    natural_axes: [bool; 2],
) -> [f32; 2] {
    let key = (
        n.id.clone(),
        avail[0].to_bits(),
        avail[1].to_bits(),
        natural_axes,
    );
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
    let child_natural = natural_children(n, natural_axes, styles[&n.id].layout.axis);
    let mut size = match n.kind {
        Kind::Text if n.editor.is_some() => [
            content[0],
            if n.editor.as_ref().is_some_and(|e| e.multiline) {
                128. * s
            } else {
                36. * s
            },
        ],
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
            // Outward physical-pixel rounding prevents padding add/subtract
            // roundoff from rewrapping the last glyph at paint time.
            [w.ceil() + 20. * s, h + 10. * s]
        }
        Kind::Separator => [content[0], s],
        Kind::Spacer | Kind::Canvas => [0., 0.],
        Kind::ZStack => {
            let mut result = [0_f32; 2];
            for child in n.children.iter().filter(|c| c.overlay.is_none()) {
                let size = intrinsic(child, content, v, m, s, memo, styles, child_natural);
                for axis in 0..2 {
                    result[axis] = result[axis].max(size[axis]);
                }
            }
            [result[0] + inset_size[0], result[1] + inset_size[1]]
        }
        _ => {
            let (sizes, gap) = child_sizes(n, content, v, m, s, memo, styles, child_natural);
            let axis = styles[&n.id].layout.axis;
            let mut result = [0_f32; 2];
            for size in sizes {
                result[axis] += size[axis];
                result[1 - axis] = result[1 - axis].max(size[1 - axis]);
            }
            result[axis] += gap
                * n.children
                    .iter()
                    .filter(|c| c.overlay.is_none())
                    .count()
                    .saturating_sub(1) as f32;
            [result[0] + inset_size[0], result[1] + inset_size[1]]
        }
    };
    if n.width.is_some() {
        size[0] = limit[0];
    }
    if n.height.is_some() {
        size[1] = limit[1];
    }
    if n.flex > 0. {
        if let Some(mode) = n.surface.scroll {
            let axes = mode.axes();
            if axes[0] && n.width.is_none() {
                size[0] = 0.;
            }
            if axes[1] && n.height.is_none() {
                size[1] = 0.;
            }
        }
    }
    let result = [size[0].max(0.).min(avail[0]), size[1].max(0.).min(avail[1])];
    memo.insert(key, result);
    result
}
#[allow(clippy::too_many_arguments)]
fn child_sizes(
    n: &Node,
    available: [f32; 2],
    v: &Bindings,
    m: &mut impl TextMetrics,
    s: f32,
    memo: &mut MeasureMemo,
    styles: &BTreeMap<String, ResolvedStyle>,
    natural_axes: [bool; 2],
) -> (Vec<[f32; 2]>, f32) {
    let axis = styles[&n.id].layout.axis;
    let gap = styles[&n.id].layout.gap.min(
        available[axis]
            / n.children
                .iter()
                .filter(|c| c.overlay.is_none())
                .count()
                .max(1) as f32,
    );
    let room = (available[axis]
        - gap
            * n.children
                .iter()
                .filter(|c| c.overlay.is_none())
                .count()
                .saturating_sub(1) as f32)
        .max(0.);
    let mut sizes: Vec<_> = n
        .children
        .iter()
        .map(|child| {
            if child.overlay.is_some() {
                [0.; 2]
            } else {
                intrinsic(child, available, v, m, s, memo, styles, natural_axes)
            }
        })
        .collect();
    let natural: f32 = sizes.iter().map(|size| size[axis]).sum();
    let total_flex: f32 = n
        .children
        .iter()
        .filter(|c| c.overlay.is_none())
        .map(|child| child.flex)
        .sum();
    for (size, child) in sizes.iter_mut().zip(&n.children) {
        if child.overlay.is_some() {
            continue;
        }
        if natural > room
            && natural > 0.
            && !natural_axes[axis]
            && !n.surface.scroll.is_some_and(|mode| mode.axes()[axis])
        {
            size[axis] *= room / natural;
        } else if !natural_axes[axis] && total_flex > 0. && room > natural {
            size[axis] += (room - natural) * child.flex / total_flex;
        }
        if axis == 0 {
            let measured = intrinsic(
                child,
                [size[0], available[1]],
                v,
                m,
                s,
                memo,
                styles,
                natural_axes,
            );
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
    effect_ancestor: Rect,
    regions: &[ClipRegion],
    offsets: &mut BTreeMap<String, [f32; 2]>,
    v: &Bindings,
    m: &mut impl TextMetrics,
    s: f32,
    out: &mut Vec<LayoutBox>,
    memo: &mut MeasureMemo,
    styles: &BTreeMap<String, ResolvedStyle>,
    natural_axes: [bool; 2],
    visible: bool,
) {
    let [x, y, aw, ah] = area.bounds();
    let w = n.width.map_or(aw, |w| (w * s).min(aw));
    let h = n.height.map_or(ah, |h| (h * s).min(ah));
    let Some(bounds) = Rect::new(x, y, w, h) else {
        return;
    };
    let intersection_clip = intersection(bounds, ancestor);
    let visible = visible && intersection_clip.is_some();
    let clip = intersection_clip.unwrap_or(ancestor);
    let mut resolved = styles[&n.id].clone();
    if resolved.round_full {
        resolved.radius = Some(w.min(h) * 0.5);
    }
    if let Some(rules) = resolved.corner_rules {
        resolved.corners = Some(rules.map(|r| match r {
            Rounding::Radius(v) => v * s,
            Rounding::Full => w.min(h) * 0.5,
        }));
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
        text: if n.editor.as_ref().is_some_and(|o| o.password) {
            "<redacted>".into()
        } else {
            text(n, v)
        },
        editor: n.editor.clone(),
        #[cfg(feature = "rust-ui")]
        on_text_changed: n.on_text_changed.clone(),
        #[cfg(feature = "rust-ui")]
        on_submit: n.on_submit.clone(),
        slider: n.slider.clone(),
        action: n
            .on_activate
            .clone()
            .or_else(|| n.on_toggle_changed.clone()),
        tone: n.tone,
        primary: n.primary,
        label_flow: (n.kind == Kind::Button).then_some(n.flow).flatten(),
        disclosure: n.disclosure,
        effects: n.effects.clone(),
        depth: styles[&n.id].depth,
        ancestor_clip: ancestor,
        effect_clip: effect_ancestor,
        clip_regions: regions.to_vec(),
        content_clip: None,
        image: n.surface.image.clone(),
        canvas: n.surface.canvas.clone(),
        scroll_max: [0.; 2],
        scroll_offset: [0.; 2],
        scroll: n.surface.scroll,
        scrollbars: n.surface.scrollbars,
        layout_visible: visible,
        split: n.split.clone(),
        split_handle: n.split_handle.clone(),
        virtual_list: n.virtual_list.clone(),
        progress: n.progress.clone(),
        detached_visual: None,
        detached_mount: 1.,
        departing: false,
        detached_state: None,
    });
    let index = out.len() - 1;
    let content_clip = match n.surface.clip.as_ref() {
        Some(Clip::Bounds) => Some(ClipRegion::Rounded {
            bounds: bounds.bounds(),
            radii: [0.; 4],
        }),
        Some(Clip::Rounded) => Some(ClipRegion::Rounded {
            bounds: bounds.bounds(),
            radii: out[index].radii(s),
        }),
        Some(Clip::Circle) => Some(ClipRegion::Circle {
            bounds: bounds.bounds(),
        }),
        Some(Clip::Path(p)) => Some(ClipRegion::Path(
            p.clone().transformed([s, 0., 0., s, x, y]),
        )),
        None if n.surface.scroll.is_some() => Some(ClipRegion::Rounded {
            bounds: bounds.bounds(),
            radii: out[index].radii(s),
        }),
        None => None,
    };
    out[index].content_clip = content_clip.clone();
    let mut child_regions = regions.to_vec();
    if let Some(r) = content_clip {
        child_regions.push(r);
    }
    let child_effect = if out[index].content_clip.is_some() {
        clip
    } else {
        effect_ancestor
    };
    let [top, right, bottom, left] = styles[&n.id].layout.padding;
    let mut start = [x + left, y + top];
    let mut available = [(w - (left + right)).max(0.), (h - (top + bottom)).max(0.)];
    let child_natural = natural_children(n, natural_axes, styles[&n.id].layout.axis);
    if available.contains(&0.) {
        return;
    }
    let child_clip = Rect::new(start[0], start[1], available[0], available[1])
        .and_then(|r| intersection(r, clip));
    let child_visible = visible && child_clip.is_some();
    let child_clip = child_clip.unwrap_or(clip);
    if let Some(mode) = n.surface.scroll {
        let axes = mode.axes();
        let mut measure = available;
        for axis in 0..2 {
            if axes[axis] {
                measure[axis] = 16_384. * s;
            }
        }
        if mode == super::Scroll::Auto {
            let cross = 1 - styles[&n.id].layout.axis;
            measure[cross] = n
                .children
                .iter()
                .filter_map(|child| {
                    if cross == 0 {
                        child.width
                    } else {
                        child.height
                    }
                })
                .fold(available[cross], |a, b| a.max(b * s));
        }
        let (sizes, gap) = child_sizes(n, measure, v, m, s, memo, styles, child_natural);
        let main = styles[&n.id].layout.axis;
        let mut content = [0_f32; 2];
        for size in &sizes {
            if n.kind == Kind::ZStack {
                for a in 0..2 {
                    content[a] = content[a].max(size[a]);
                }
            } else {
                content[main] += size[main];
                content[1 - main] = content[1 - main].max(size[1 - main]);
            }
        }
        if n.kind != Kind::ZStack {
            content[main] += gap
                * n.children
                    .iter()
                    .filter(|c| c.overlay.is_none())
                    .count()
                    .saturating_sub(1) as f32;
        }
        if let Some(list) = &n.virtual_list {
            content[1] = list.count as f32 * list.row_height * s;
        }
        let offset = offsets.entry(n.id.clone()).or_insert_with(|| {
            n.virtual_list
                .as_ref()
                .map_or([0.; 2], |list| [0., list.start as f32 * list.row_height])
        });
        for a in 0..2 {
            let max = if axes[a] {
                (content[a] - available[a]).max(0.) / s
            } else {
                0.
            };
            offset[a] = offset[a].clamp(0., max);
            out[index].scroll_max[a] = max;
            start[a] -= offset[a] * s;
            if axes[a] {
                available[a] = available[a].max(content[a]);
            }
        }
        out[index].scroll_offset = *offset;
    }
    if let Some(list) = &n.virtual_list {
        for (row, child) in n.children.iter().enumerate() {
            let height = list.row_height * s;
            if let Some(r) = Rect::new(
                start[0],
                start[1] + (list.start + row) as f32 * height,
                available[0],
                height,
            ) {
                place(
                    child,
                    r,
                    child_clip,
                    child_effect,
                    &child_regions,
                    offsets,
                    v,
                    m,
                    s,
                    out,
                    memo,
                    styles,
                    child_natural,
                    child_visible,
                );
            }
        }
        return;
    }
    #[cfg(feature = "rust-ui")]
    if let Some(split) = &n.split {
        let axis = if split.axis == super::Direction::Horizontal {
            0
        } else {
            1
        };
        let sizes = value_controls::split_sizes(available[axis], split, s);
        let mut cursor = start[axis];
        for (child, length) in n.children.iter().zip(sizes) {
            let mut pos = start;
            pos[axis] = cursor;
            let mut size = available;
            size[axis] = length;
            if let Some(r) = Rect::new(pos[0], pos[1], size[0], size[1]) {
                place(
                    child,
                    r,
                    child_clip,
                    child_effect,
                    &child_regions,
                    offsets,
                    v,
                    m,
                    s,
                    out,
                    memo,
                    styles,
                    child_natural,
                    child_visible,
                );
            }
            cursor += length;
        }
        return;
    }
    if n.kind == Kind::ZStack {
        let fractions = n.flow.map_or([0.5, 0.5], |flow| flow.fractions());
        for child in n.children.iter().filter(|c| c.overlay.is_none()) {
            let mut size = intrinsic(child, available, v, m, s, memo, styles, child_natural);
            if n.align == Align::Stretch {
                if child.width.is_none() {
                    size[0] = available[0];
                }
                if child.height.is_none() {
                    size[1] = available[1];
                }
            }
            let mut pos = [
                start[0] + (available[0] - size[0]).max(0.) * fractions[0],
                start[1] + (available[1] - size[1]).max(0.) * fractions[1],
            ];
            if let Some(options) = &n.overlay {
                if matches!(
                    options.kind,
                    super::document::OverlayKind::Popup | super::document::OverlayKind::Tooltip
                ) {
                    if let Some(point) = options.position {
                        pos[0] = (point[0] * s).clamp(x, (x + w - size[0]).max(x));
                        let below = point[1] * s;
                        pos[1] = if below + size[1] <= y + h {
                            below
                        } else {
                            below - size[1]
                        };
                        pos[1] = pos[1].clamp(y, (y + h - size[1]).max(y));
                    } else if let Some(anchor) = options
                        .anchor
                        .as_deref()
                        .and_then(|id| out.iter().find(|b| b.id == id))
                    {
                        let [ax, ay, aw, ah] = anchor.bounds.bounds();
                        let _ = aw;
                        pos[0] = ax.clamp(x, (x + w - size[0]).max(x));
                        let below = ay + ah + 4. * s;
                        pos[1] = if below + size[1] <= y + h {
                            below
                        } else {
                            ay - size[1] - 4. * s
                        };
                        pos[1] = pos[1].clamp(y, (y + h - size[1]).max(y));
                    }
                }
            }
            if let Some(r) = Rect::new(pos[0], pos[1], size[0], size[1]) {
                place(
                    child,
                    r,
                    child_clip,
                    child_effect,
                    &child_regions,
                    offsets,
                    v,
                    m,
                    s,
                    out,
                    memo,
                    styles,
                    child_natural,
                    child_visible,
                );
            }
        }
        return;
    }
    let axis = styles[&n.id].layout.axis;
    let cross = 1 - axis;
    let (sizes, gap) = child_sizes(n, available, v, m, s, memo, styles, child_natural);
    let occupied = sizes.iter().map(|size| size[axis]).sum::<f32>()
        + gap
            * n.children
                .iter()
                .filter(|c| c.overlay.is_none())
                .count()
                .saturating_sub(1) as f32;
    let placement = n.flow.map(|flow| flow.fractions());
    let mut cursor = start[axis]
        + placement.map_or(0., |fraction| {
            (available[axis] - occupied).max(0.) * fraction[axis]
        });
    for (child, mut size) in n.children.iter().zip(sizes) {
        if child.overlay.is_some() {
            continue;
        }
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
            place(
                child,
                r,
                child_clip,
                child_effect,
                &child_regions,
                offsets,
                v,
                m,
                s,
                out,
                memo,
                styles,
                child_natural,
                child_visible,
            );
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
    #[cfg(feature = "rust-ui")]
    #[test]
    fn nested_scroll_natural_axes_survive_automatic_containers_but_fixed_panels_keep_spacers() {
        use crate::ui::prelude::*;
        for scale in [1., 2.] {
            for width in [600., 1120.] {
                let ui: Element<()> = vstack![vstack![
                    text("Introduction").key("intro"),
                    stack![
                        panel![text("First"), spacer(), text("Bottom").key("bottom")]
                            .key("first")
                            .height(100.)
                            .flex(1.),
                        panel![text("Second")].key("second").height(100.).flex(1.),
                    ]
                    .direction(
                        responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal)
                    )
                    .gap(16., Axis::Both),
                ]
                .gap(12., Axis::Vertical)]
                .scroll(Scroll::Auto)
                .into();
                let (doc, callbacks) = ui.build().unwrap().into_parts();
                let values = Bindings::default();
                let mut runtime = Runtime::new(doc, &callbacks, &values).unwrap();
                let mut metrics = Metrics(0);
                runtime
                    .layout(
                        Viewport::new(width * scale, 760. * scale, scale).unwrap(),
                        &values,
                        &mut metrics,
                    )
                    .unwrap();
                let bounds = |id: &str| {
                    runtime
                        .boxes()
                        .iter()
                        .find(|b| b.id == id)
                        .unwrap()
                        .bounds
                        .bounds()
                        .map(|v| v / scale)
                };
                let first = bounds("first");
                let second = bounds("second");
                let axis = usize::from(width < 768.);
                assert!((second[axis] - first[axis] - first[axis + 2] - 16.).abs() < 0.01);
                assert_eq!(first[3], 100.);
                assert!(bounds("intro")[3] >= 14. * 1.2);
                // The fixed-height panel is bounded again: its spacer must fill
                // down to the panel's 12px bottom padding.
                let bottom = bounds("bottom");
                assert!((bottom[1] + bottom[3] + 12. - first[1] - first[3]).abs() < 0.01);
            }
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
