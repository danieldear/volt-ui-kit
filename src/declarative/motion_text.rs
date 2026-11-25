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
    /// Streaming: graphemes shown when the latest piece arrived, and how
    /// long the rest takes to appear.
    from_count: usize,
    reveal: Duration,
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
                        // Text already there when a stream appears shows at
                        // once: only what arrives later eases in.
                        from_count: b.text.graphemes(true).count(),
                        reveal: Duration::ZERO,
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
                if effect.kind == TextEffectKind::Stream && track.effect == effect {
                    if track.target != b.text {
                        let now = self.now;
                        let shown = stream_shown(track, now);
                        let total = b.text.graphemes(true).count();
                        // A continuation picks up where the reveal was; any
                        // other change shows the new text at once.
                        let kept = track
                            .target
                            .grapheme_indices(true)
                            .nth(shown)
                            .map_or(track.target.as_str(), |(i, _)| &track.target[..i]);
                        track.from_count = if b.text.starts_with(kept) {
                            shown.min(total)
                        } else {
                            total
                        };
                        track.reveal = stream_time(total - track.from_count);
                        track.target = b.text.clone();
                        track.start = now;
                        track.finished = self.reduced || track.from_count == total;
                    }
                } else if track.target != b.text || track.effect != effect {
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
                        || (t.effect.kind == TextEffectKind::Stream
                            && self.now.saturating_sub(t.start) < t.reveal)
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
                } else if t.effect.kind == TextEffectKind::Stream {
                    (self.now.saturating_sub(t.start) < t.reveal).then_some(Duration::ZERO)
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
                TextEffectKind::Stream => {
                    let n = stream_shown(t, self.now);
                    if let Some((end, _)) = b.text.grapheme_indices(true).nth(n) {
                        b.text.truncate(end);
                    }
                }
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

/// Graphemes of a stream showing at `now`.
fn stream_shown(t: &Track, now: Duration) -> usize {
    let total = t.target.graphemes(true).count();
    if t.finished || t.reveal.is_zero() {
        return total;
    }
    let p = (now.saturating_sub(t.start).as_secs_f32() / t.reveal.as_secs_f32()).clamp(0., 1.);
    t.from_count + ((total - t.from_count) as f32 * p).round() as usize
}
/// How long `n` new graphemes take to appear: a steady pace for a trickle,
/// faster for a burst so the reveal never falls far behind.
fn stream_time(n: usize) -> Duration {
    Duration::from_millis((n as u64 * 16).clamp(90, 650))
}
