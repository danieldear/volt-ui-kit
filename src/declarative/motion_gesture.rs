use super::{motion::Motion, LayoutBox};
use crate::motion::{DragGesture, DragMotion, InertiaPlayer};
use std::collections::BTreeMap;
#[derive(Default)]
pub(super) struct GestureMotion {
    states: BTreeMap<String, State>,
}
struct State {
    policy: DragMotion,
    position: [f32; 2],
    anchor: [f32; 2],
    gesture: DragGesture,
    inertia: Option<InertiaPlayer>,
    /// Springing home: where it was let go, and when.
    home: Option<([f32; 2], std::time::Duration)>,
}
/// A critically damped spring's remaining share of the distance after
/// `t` seconds: quick, with no overshoot. About 0.35 s to settle.
fn spring_share(t: f32) -> f32 {
    const OMEGA: f32 = 18.;
    (1. + OMEGA * t) * (-OMEGA * t).exp()
}
impl Motion {
    pub fn reconcile_gestures(&mut self, boxes: &[LayoutBox]) {
        self.gestures.states.retain(|id, _| {
            boxes
                .iter()
                .any(|b| b.id == *id && b.effects.drag_motion.is_some())
        });
        for b in boxes {
            if let Some(policy) = b.effects.drag_motion {
                let s = self.gestures.states.entry(b.id.clone()).or_insert(State {
                    policy,
                    position: [0.; 2],
                    anchor: [0.; 2],
                    gesture: DragGesture::default(),
                    inertia: None,
                    home: None,
                });
                s.policy = policy;
                s.position = policy.clamp(s.position);
            }
        }
    }
    pub fn gesture_offset(&self, id: &str) -> [f32; 2] {
        self.gestures.states.get(id).map_or([0.; 2], |s| s.position)
    }
    pub fn gesture_begin(&mut self, id: &str) {
        if let Some(s) = self.gestures.states.get_mut(id) {
            s.inertia = None;
            s.home = None;
            s.anchor = s.position;
            s.gesture.begin(self.now, s.position);
        }
    }
    pub fn gesture_move(&mut self, id: &str, delta: [f32; 2]) -> bool {
        if let Some(s) = self.gestures.states.get_mut(id) {
            s.position = s
                .policy
                .clamp(std::array::from_fn(|i| s.anchor[i] + delta[i]));
            s.gesture.update(self.now, s.position);
            return true;
        }
        false
    }
    /// End a drag; returns the release velocity in logical points per second.
    pub fn gesture_end(&mut self, id: &str) -> [f32; 2] {
        if let Some(s) = self.gestures.states.get_mut(id) {
            let velocity = s.gesture.release(self.now);
            if s.policy.spring_back {
                s.inertia = None;
                if self.reduced {
                    s.position = [0.; 2];
                } else {
                    s.home = Some((s.position, self.now));
                }
                return velocity;
            }
            s.inertia = if s.policy.momentum && !self.reduced {
                s.policy
                    .inertia
                    .release(s.position, velocity)
                    .ok()
                    .map(|p| p.at(self.now))
            } else {
                None
            };
            return velocity;
        }
        [0.; 2]
    }
    pub fn advance_gestures(&mut self) {
        for s in self.gestures.states.values_mut() {
            if let Some((from, start)) = s.home {
                let share = spring_share(self.now.saturating_sub(start).as_secs_f32());
                if share < 0.002 {
                    s.position = [0.; 2];
                    s.home = None;
                } else {
                    s.position = from.map(|v| v * share);
                }
            }
            if let Some(inertia) = &mut s.inertia {
                s.position = inertia.sample(self.now);
                if !inertia.is_active() {
                    s.inertia = None;
                }
            }
        }
    }
    pub fn gestures_active(&self) -> bool {
        self.gestures
            .states
            .values()
            .any(|s| s.home.is_some() || s.inertia.as_ref().is_some_and(|i| i.is_active()))
    }
    pub fn settle_gestures(&mut self) {
        for s in self.gestures.states.values_mut() {
            s.inertia = None;
            if s.home.take().is_some() {
                s.position = [0.; 2];
            }
            s.gesture = DragGesture::default();
        }
    }
}
