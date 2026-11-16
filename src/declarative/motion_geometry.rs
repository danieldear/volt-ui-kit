//! Keyed geometry tracks use one clock and preserve the sampled rectangle on retarget.
use super::{effects::Animation, Kind, LayoutBox};
use crate::motion::Affine2D;
use std::{collections::BTreeMap, time::Duration};
#[derive(Clone)]
struct GeometryTrack {
    from: [f32; 4],
    to: [f32; 4],
    start: Duration,
    animation: Animation,
}
impl GeometryTrack {
    fn sample(&self, now: Duration) -> [f32; 4] {
        let t = self
            .animation
            .progress(now.saturating_sub(self.start).as_secs_f32() * 1000.);
        std::array::from_fn(|i| {
            self.from[i] + (self.to[i] - self.from[i]) * if i < 2 { t } else { t.clamp(0., 1.) }
        })
    }
    fn wait(&self, now: Duration) -> Option<Duration> {
        self.animation.wait(now.saturating_sub(self.start))
    }
}
#[derive(Default)]
pub(super) struct GeometryMotion {
    previous: BTreeMap<String, (Kind, [f32; 4])>,
    tracks: BTreeMap<String, GeometryTrack>,
    scale: f32,
}
impl GeometryMotion {
    fn id(b: &LayoutBox) -> String {
        b.effects
            .shared
            .as_ref()
            .map_or_else(|| format!("id:{}", b.id), |key| format!("shared:{key}"))
    }
    pub fn reconcile(&mut self, boxes: &[LayoutBox], now: Duration, scale: f32, reduced: bool) {
        if self.scale != scale {
            self.previous.clear();
            self.tracks.clear();
            self.scale = scale;
        }
        let mut next = BTreeMap::new();
        for b in boxes
            .iter()
            .filter(|b| b.effects.layout_animation.is_some())
        {
            let id = Self::id(b);
            let to = b.bounds.bounds();
            next.insert(id.clone(), (b.kind, to));
            if !reduced {
                if let Some((kind, old)) = self
                    .previous
                    .get(&id)
                    .filter(|(kind, old)| *kind == b.kind && *old != to)
                {
                    let _ = kind;
                    let from = self.tracks.get(&id).map_or(*old, |t| t.sample(now));
                    if from[2] > 0. && from[3] > 0. && to[2] > 0. && to[3] > 0. {
                        self.tracks.insert(
                            id.clone(),
                            GeometryTrack {
                                from,
                                to,
                                start: now,
                                animation: b.effects.layout_animation.unwrap(),
                            },
                        );
                    }
                }
            }
        }
        self.tracks
            .retain(|id, t| next.contains_key(id) && t.wait(now).is_some() && !reduced);
        self.previous = next;
    }
    /// Scroll is direct manipulation, not a new layout transition. Move both
    /// endpoints of existing tracks with the content so they do not chase the wheel.
    pub fn translate(&mut self, boxes: &[LayoutBox], delta: [f32; 2]) {
        for b in boxes
            .iter()
            .filter(|b| b.effects.layout_animation.is_some())
        {
            let id = Self::id(b);
            if let Some((_, rect)) = self.previous.get_mut(&id) {
                for (i, d) in delta.iter().enumerate() {
                    rect[i] += d;
                }
            }
            if let Some(track) = self.tracks.get_mut(&id) {
                for (i, d) in delta.iter().enumerate() {
                    track.from[i] += d;
                    track.to[i] += d;
                }
            }
        }
    }
    pub fn matrix(&self, b: &LayoutBox, now: Duration) -> Affine2D {
        let Some(t) = self.tracks.get(&Self::id(b)) else {
            return Affine2D::IDENTITY;
        };
        let [x, y, w, h] = t.sample(now);
        let [tx, ty, tw, th] = b.bounds.bounds();
        if tw <= 0. || th <= 0. {
            return Affine2D::IDENTITY;
        }
        let sx = f64::from(w / tw);
        let sy = f64::from(h / th);
        Affine2D([
            sx,
            0.,
            0.,
            sy,
            f64::from(x) - sx * f64::from(tx),
            f64::from(y) - sy * f64::from(ty),
        ])
    }
    pub fn relative(&self, b: &LayoutBox, now: Duration, parent: Affine2D) -> (Affine2D, Affine2D) {
        if self.tracks.contains_key(&Self::id(b)) {
            let absolute = self.matrix(b, now);
            (
                absolute.then(parent.inverse().unwrap_or_default()),
                absolute,
            )
        } else {
            (Affine2D::IDENTITY, parent)
        }
    }
    pub fn wait(&self, now: Duration) -> Option<Duration> {
        self.tracks.values().filter_map(|t| t.wait(now)).min()
    }
    pub fn advance(&mut self, now: Duration) {
        self.tracks.retain(|_, t| t.wait(now).is_some());
    }
    pub fn settle(&mut self) {
        self.tracks.clear();
    }
}
