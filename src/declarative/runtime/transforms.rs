//! Input uses the same sampled pose as paint, not an axis-aligned approximation.
use super::*;
use crate::motion::Affine2D;
impl Runtime {
    pub(super) fn world_transform(&self, b: &LayoutBox) -> Affine2D {
        if !self.has_transforms {
            return Affine2D::IDENTITY;
        }
        let Some(index) = self.boxes.iter().position(|v| v.id == b.id) else {
            return Affine2D::IDENTITY;
        };
        let mut ancestry = vec![b];
        let mut depth = b.depth;
        for parent in self.boxes[..index].iter().rev() {
            if parent.depth < depth {
                ancestry.push(parent);
                depth = parent.depth;
            }
        }
        let theme = self.input_theme.borrow();
        let motion = self.motion.try_borrow().ok();
        let mut result = Affine2D::IDENTITY;
        let mut geometry = Affine2D::IDENTITY;
        for node in ancestry.into_iter().rev() {
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

        result
    }
    /// Conservative world-space visibility, including ancestor clipping. This
    /// controls frame scheduling, not glyph/shape antialiasing.
    pub(super) fn motion_visible(&self, b: &LayoutBox) -> bool {
        if !self.has_transforms {
            return b.layout_visible;
        }
        let Some([w, h, _]) = self.last_viewport else {
            return false;
        };
        let bounds = self.world_transform(b).bounds(b.bounds.bounds());
        let Some(mut visible) = Rect::new(bounds[0], bounds[1], bounds[2], bounds[3])
            .and_then(|r| intersection(r, Rect::new(0., 0., w, h).unwrap()))
        else {
            return false;
        };
        let Some(index) = self.boxes.iter().position(|p| p.id == b.id) else {
            return false;
        };
        let mut depth = b.depth + 1;
        for (at, node) in self.boxes[..=index].iter().enumerate().rev() {
            if node.depth >= depth {
                continue;
            }
            depth = node.depth;
            let parent = self.boxes[..at].iter().rev().find(|p| p.depth < node.depth);
            let matrix = parent.map_or(Affine2D::IDENTITY, |p| self.world_transform(p));
            let r = matrix.bounds(node.ancestor_clip.bounds());
            let Some(next) =
                Rect::new(r[0], r[1], r[2], r[3]).and_then(|r| intersection(visible, r))
            else {
                return false;
            };
            visible = next;
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
        if !self.has_transforms {
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
        let Some(index) = self.boxes.iter().position(|v| v.id == b.id) else {
            return false;
        };
        let mut depth = b.depth + 1;
        for node in self.boxes[..=index].iter().rev() {
            if node.depth < depth {
                depth = node.depth;
                let at = self.boxes[..=index]
                    .iter()
                    .position(|p| p.id == node.id)
                    .unwrap();
                let parent = self.boxes[..at].iter().rev().find(|p| p.depth < node.depth);
                let matrix = parent.map_or(Affine2D::IDENTITY, |p| self.world_transform(p));
                if !matrix
                    .inverse()
                    .map(|m| m.point([x, y]))
                    .is_some_and(|p| node.ancestor_clip.contains(p[0], p[1]))
                {
                    return false;
                }

                if let Some(clip) = &node.content_clip {
                    if !self
                        .input_point(node, x, y)
                        .is_some_and(|p| clip.contains(p[0], p[1]))
                    {
                        return false;
                    }
                }
            }
        }
        self.input_point(b, x, y).is_some()
    }
}
