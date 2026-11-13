//! Input uses the same sampled pose as paint, not an axis-aligned approximation.
use super::*;
use crate::motion::Affine2D;
use std::collections::HashMap;

/// Layout topology only. Poses are sampled at the current clock/input state,
/// never cached across hover, retargeting or animation frames.
#[derive(Default)]
pub(super) struct TransformIndex {
    ids: HashMap<String, usize>,
    parents: Vec<Option<usize>>,
    affected: Vec<bool>,
    paint: Vec<bool>,
}
impl TransformIndex {
    pub fn build(boxes: &[LayoutBox]) -> Self {
        let mut result = Self::default();
        let mut stack: Vec<usize> = vec![];
        for (i, b) in boxes.iter().enumerate() {
            while stack.last().is_some_and(|p| boxes[*p].depth >= b.depth) {
                stack.pop();
            }
            let parent = stack.last().copied();
            let affected = parent.is_some_and(|p| result.affected[p]) || b.effects.has_geometry();
            result.ids.insert(b.id.clone(), i);
            result.parents.push(parent);
            result.affected.push(affected);
            result.paint.push(affected);
            stack.push(i);
        }
        // A moving leaf can enter the viewport through a non-clipping,
        // offscreen parent. Keep its ancestor groups for clips/opacity/masks.
        for i in (0..boxes.len()).rev() {
            if result.paint[i] {
                if let Some(parent) = result.parents[i] {
                    result.paint[parent] = true;
                }
            }
        }
        result
    }
    pub fn affected(&self, id: &str) -> bool {
        self.ids.get(id).is_some_and(|i| self.affected[*i])
    }
    pub fn paint(&self, b: &LayoutBox) -> bool {
        b.layout_visible || self.ids.get(&b.id).is_some_and(|i| self.paint[*i])
    }
}
impl Runtime {
    pub(super) fn rebuild_transform_index(&mut self) {
        self.transform_index = if self.has_transforms {
            TransformIndex::build(&self.boxes)
        } else {
            TransformIndex::default()
        };
    }
    /// One ancestry walk, root to leaf, shared by input clipping/visibility.
    /// Unaffected siblings stay on the ordinary axis-aligned fast path.
    fn transform_path(&self, b: &LayoutBox) -> Vec<(usize, Affine2D)> {
        let Some(&index) = self.transform_index.ids.get(&b.id) else {
            return vec![];
        };
        let mut ancestry = vec![index];
        let mut parent = self.transform_index.parents[index];
        while let Some(i) = parent {
            ancestry.push(i);
            parent = self.transform_index.parents[i];
        }
        let theme = self.input_theme.borrow();
        let motion = self.motion.try_borrow().ok();
        let mut result = Affine2D::IDENTITY;
        let mut geometry = Affine2D::IDENTITY;
        ancestry
            .into_iter()
            .rev()
            .map(|i| {
                if self.transform_index.affected[i] {
                    let node = &self.boxes[i];
                    let target =
                        visual_target(node, self.state_for_box(node), &theme, self.scale).transform;
                    let pose = motion
                        .as_ref()
                        .map_or(target, |m| m.input_transform(&node.id, target));
                    if let Some(m) = motion.as_ref() {
                        let (local, next) = m.node_matrix(node, pose, self.scale, geometry);
                        result = local.then(result);
                        geometry = next;
                    } else {
                        result = pose
                            .matrix(node.bounds.bounds().map(|v| v / self.scale), self.scale)
                            .then(result);
                    }
                }
                (i, result)
            })
            .collect()
    }
    pub(super) fn world_transform(&self, b: &LayoutBox) -> Affine2D {
        if !self.has_transforms || !self.transform_index.affected(&b.id) {
            return Affine2D::IDENTITY;
        }
        self.transform_path(b)
            .last()
            .map_or(Affine2D::IDENTITY, |(_, m)| *m)
    }
    /// Conservative world-space visibility, including ancestor clipping. This
    /// controls frame scheduling, not glyph/shape antialiasing.
    pub(super) fn motion_visible(&self, b: &LayoutBox) -> bool {
        if !self.has_transforms || !self.transform_index.affected(&b.id) {
            return b.layout_visible;
        }
        let Some([w, h, _]) = self.last_viewport else {
            return false;
        };
        let path = self.transform_path(b);
        let matrix = path.last().map_or(Affine2D::IDENTITY, |(_, m)| *m);
        let bounds = matrix.bounds(b.bounds.bounds());
        let Some(mut visible) = Rect::new(bounds[0], bounds[1], bounds[2], bounds[3])
            .and_then(|r| intersection(r, Rect::new(0., 0., w, h).unwrap()))
        else {
            return false;
        };
        let mut parent = Affine2D::IDENTITY;
        for (i, matrix) in path {
            let r = parent.bounds(self.boxes[i].ancestor_clip.bounds());
            let Some(next) =
                Rect::new(r[0], r[1], r[2], r[3]).and_then(|r| intersection(visible, r))
            else {
                return false;
            };
            visible = next;
            parent = matrix;
        }
        true
    }
    #[cfg(feature = "rust-ui")]
    pub(super) fn refresh_motion_visibility(&mut self) {
        let visible = self
            .boxes
            .iter()
            .filter(|b| b.effects.text_effect.is_some() && self.motion_visible(b))
            .map(|b| b.id.clone())
            .collect();
        self.motion.get_mut().text_visibility(&visible);
    }
    #[cfg(feature = "rust-ui")]
    pub(super) fn surface_local(&self, id: &str, x: f32, y: f32) -> [f32; 2] {
        let Some(b) = self.boxes.iter().find(|b| b.id == id) else {
            return [0.; 2];
        };
        let [px, py] = self.input_point(b, x, y).unwrap_or([x, y]);
        let [bx, by, _, _] = b.bounds.bounds();
        [(px - bx) / self.scale, (py - by) / self.scale]
    }
    pub(super) fn input_point(&self, b: &LayoutBox, x: f32, y: f32) -> Option<[f32; 2]> {
        self.world_transform(b).inverse().map(|m| m.point([x, y]))
    }
    pub(super) fn hit_bounds(&self, b: &LayoutBox, x: f32, y: f32) -> bool {
        self.input_point(b, x, y)
            .is_some_and(|p| b.bounds.contains(p[0], p[1]))
    }
    pub(super) fn hit_rounded(&self, b: &LayoutBox, x: f32, y: f32) -> bool {
        self.input_point(b, x, y)
            .is_some_and(|p| b.bounds.contains_corners(b.radii(self.scale), p[0], p[1]))
    }
    pub(super) fn hit_clip(&self, b: &LayoutBox, x: f32, y: f32) -> bool {
        if !self.has_transforms || !self.transform_index.affected(&b.id) {
            return b.clip.contains(x, y)
                && b.clip_regions.iter().all(|r| r.contains(x, y))
                && b.content_clip.as_ref().is_none_or(|r| r.contains(x, y));
        }
        let Some([w, h, _]) = self.last_viewport else {
            return false;
        };
        if !(x >= 0. && y >= 0. && x < w && y < h) {
            return false;
        }
        let mut parent = Affine2D::IDENTITY;
        for (i, matrix) in self.transform_path(b) {
            let node = &self.boxes[i];
            if !parent
                .inverse()
                .map(|m| m.point([x, y]))
                .is_some_and(|p| node.ancestor_clip.contains(p[0], p[1]))
            {
                return false;
            }
            let Some(point) = matrix.inverse().map(|m| m.point([x, y])) else {
                return false;
            };
            if node
                .content_clip
                .as_ref()
                .is_some_and(|clip| !clip.contains(point[0], point[1]))
            {
                return false;
            }
            parent = matrix;
        }
        true
    }
}

#[cfg(all(test, feature = "rust-ui", feature = "rendering"))]
mod tests {
    use super::*;
    use crate::{
        rendering::RasterPainter,
        ui::{prelude::*, text},
        Painter,
    };

    fn fixture(scale: f32) -> (Runtime, RasterPainter) {
        let mut children: Vec<Element<()>> =
            vec![text("Tilted").key("tilted").rotate(12.).height(40.).into()];
        for i in 0..30 {
            children.push(
                vstack![text(format!("Row {i}"))]
                    .key(format!("row-{i}"))
                    .height(45.)
                    .padding(5., Edge::All)
                    .background(ColorToken::Base200)
                    .rounded(6.)
                    .opacity(0.8)
                    .clip_contents(true)
                    .into(),
            );
        }
        let tree: Element<()> = vstack(children)
            .key("scroll")
            .height(180.)
            .scroll(Scroll::Vertical)
            .clip_contents(true)
            .into();
        let (d, h) = tree.build().unwrap().into_parts();
        let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
        let vp = Viewport::new(400. * scale, 180. * scale, scale).unwrap();
        let mut p = RasterPainter::new(vp);
        r.layout(vp, &Bindings::default(), &mut p).unwrap();
        (r, p)
    }
    #[test]
    fn culled_motion_paint_matches_full_scene_pixels_during_scroll() {
        for scale in [1., 1.25, 2.] {
            let (mut r, mut p) = fixture(scale);
            for delta in [0., 25., 85., 170., -140.] {
                r.scroll_at(120. * scale, 100. * scale, [0., delta]);
                r.layout(p.viewport(), &Bindings::default(), &mut p)
                    .unwrap();
                let culled = r.paint_ui(&mut p, &UiTheme::dark());
                let actual = p.rasterize(&culled, [0.; 4]).unwrap();
                let flags = r.transform_index.paint.clone();
                r.transform_index.paint.fill(true);
                let full = r.paint_ui(&mut p, &UiTheme::dark());
                let expected = p.rasterize(&full, [0.; 4]).unwrap();
                r.transform_index.paint = flags;
                assert_eq!(
                    actual.pixels(),
                    expected.pixels(),
                    "DPI {scale}, delta {delta}"
                );
                assert!(!r.transform_index.affected("row-20"));
                assert!(r.transform_index.affected("tilted"));
            }
        }
    }
}
