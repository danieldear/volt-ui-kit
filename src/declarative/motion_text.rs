use super::{motion::Motion, LayoutBox};
use crate::motion::{TextEffect, TextEffectKind};
use std::{collections::BTreeMap, time::Duration};
use unicode_segmentation::UnicodeSegmentation;
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TextPaint {
    CrossFade { from: String, progress: f32 },
    Rolling { from: String, progress: f32 },
    Shimmer(f32),
}
#[derive(Default)]
pub(super) struct TextMotion {
    tracks: BTreeMap<String, Track>,
}
struct Track {
    target: String,
    from: String,
    effect: TextEffect,
    start: Duration,
    finished: bool,
    visible: bool,
}
impl Motion {
    pub fn reconcile_text(&mut self, boxes: &[LayoutBox]) {
        self.texts.tracks.retain(|id, _| {
            boxes
                .iter()
                .any(|b| b.id == *id && b.effects.text_effect.is_some())
        });
        for b in boxes {
            if let Some(effect) = b.effects.text_effect {
                let track = self
                    .texts
                    .tracks
                    .entry(b.id.clone())
                    .or_insert_with(|| Track {
                        target: b.text.clone(),
                        from: if effect.kind == TextEffectKind::Typing {
                            String::new()
                        } else {
                            b.text.clone()
                        },
                        effect,
                        start: self.now,
                        finished: self.reduced
                            || matches!(
                                effect.kind,
                                TextEffectKind::Morph | TextEffectKind::Rolling
                            ),
                        visible: b.layout_visible,
                    });
                if track.target != b.text || track.effect != effect {
                    track.from = if effect.kind == TextEffectKind::Typing {
                        String::new()
                    } else {
                        track.target.clone()
                    };
                    track.target = b.text.clone();
                    track.effect = effect;
                    track.start = self.now;
                    track.finished = self.reduced;
                }
                track.visible = b.layout_visible;
                if self.reduced {
                    track.finished = true;
                }
            }
        }
    }
    pub fn text_visibility(&mut self, visible: &std::collections::BTreeSet<String>) {
        for (id, track) in &mut self.texts.tracks {
            track.visible = visible.contains(id);
        }
    }
    pub fn text_active(&self) -> bool {
        !self.reduced
            && self.texts.tracks.values().any(|t| {
                t.visible
                    && !t.finished
                    && (t.effect.kind == TextEffectKind::Shimmer
                        || t.effect
                            .animation
                            .wait(self.now.saturating_sub(t.start))
                            .is_some())
            })
    }
    pub fn text_wait(&self) -> Option<Duration> {
        if self.reduced {
            return None;
        }
        self.texts
            .tracks
            .values()
            .filter(|t| t.visible && !t.finished)
            .filter_map(|t| {
                if t.effect.kind == TextEffectKind::Shimmer {
                    Some(Duration::ZERO)
                } else {
                    t.effect.animation.wait(self.now.saturating_sub(t.start))
                }
            })
            .min()
    }
    pub fn resume_text(&mut self) {
        for t in self.texts.tracks.values_mut() {
            if t.effect.kind == TextEffectKind::Shimmer {
                t.finished = false;
            }
        }
    }
    pub fn settle_text(&mut self) {
        for t in self.texts.tracks.values_mut() {
            t.finished = true;
        }
    }
    pub fn apply_text(&self, boxes: &mut [LayoutBox]) {
        for b in boxes {
            let Some(t) = self.texts.tracks.get(&b.id) else {
                continue;
            };
            if self.reduced || t.finished {
                continue;
            }
            let elapsed = self.now.saturating_sub(t.start);
            let progress = t
                .effect
                .animation
                .progress(elapsed.as_secs_f32() * 1000.)
                .clamp(0., 1.);
            match t.effect.kind {
                TextEffectKind::Typing => {
                    let graphemes = b.text.grapheme_indices(true).collect::<Vec<_>>();
                    let n = (graphemes.len() as f32 * progress).floor() as usize;
                    let end = graphemes.get(n).map_or(b.text.len(), |g| g.0);
                    b.text.truncate(end);
                }
                TextEffectKind::Morph if progress < 1. => {
                    b.text_paint = Some(TextPaint::CrossFade {
                        from: t.from.clone(),
                        progress,
                    })
                }
                TextEffectKind::Rolling if progress < 1. => {
                    b.text_paint = Some(TextPaint::Rolling {
                        from: t.from.clone(),
                        progress,
                    })
                }
                TextEffectKind::Shimmer => {
                    b.text_paint = Some(TextPaint::Shimmer(
                        (elapsed.as_secs_f64() * 1000. / t.effect.animation.duration as f64).fract()
                            as f32,
                    ))
                }
                _ => {}
            }
        }
    }
}
