use super::{motion::Motion, LayoutBox};
use crate::motion::{MotionPose, Timeline, TimelinePlayback, TimelinePlayer};
use std::collections::BTreeMap;
#[derive(Default)]
pub(super) struct Timelines {
    tracks: BTreeMap<String, Track>,
}
struct Track {
    spec: Timeline,
    player: TimelinePlayer,
    pose: MotionPose,
}
impl Motion {
    pub fn reconcile_timelines(&mut self, boxes: &[LayoutBox]) {
        self.timelines.tracks.retain(|id, _| {
            boxes
                .iter()
                .any(|b| b.id == *id && b.effects.timeline.is_some())
        });
        for b in boxes {
            if let Some(spec) = &b.effects.timeline {
                let reset = self
                    .timelines
                    .tracks
                    .get(&b.id)
                    .is_none_or(|t| !t.spec.same_sequence(spec));
                if reset {
                    let mut player = TimelinePlayer::new(spec.clone()).expect("validated timeline");
                    if spec.playback == TimelinePlayback::Reverse {
                        player.seek(1.);
                    }
                    match spec.playback {
                        TimelinePlayback::Play => player.play(self.now),
                        TimelinePlayback::Reverse => player.reverse(self.now),
                        TimelinePlayback::Paused => {}
                        TimelinePlayback::Seek(p) => player.seek(p),
                    }
                    if self.reduced {
                        player.finish();
                    }
                    let pose = player.sample(self.now);
                    self.timelines.tracks.insert(
                        b.id.clone(),
                        Track {
                            spec: spec.clone(),
                            player,
                            pose,
                        },
                    );
                } else {
                    let track = self.timelines.tracks.get_mut(&b.id).unwrap();
                    if track.spec.playback != spec.playback {
                        match spec.playback {
                            TimelinePlayback::Play => track.player.play(self.now),
                            TimelinePlayback::Reverse => track.player.reverse(self.now),
                            TimelinePlayback::Paused => track.player.pause(self.now),
                            TimelinePlayback::Seek(p) => track.player.seek(p),
                        }
                        track.spec = spec.clone();
                    }
                    if self.reduced {
                        track.player.finish();
                    }
                    track.pose = track.player.sample(self.now);
                }
            }
        }
    }
    pub fn timeline_pose(&self, id: &str) -> MotionPose {
        self.timelines
            .tracks
            .get(id)
            .map_or_else(MotionPose::new, |t| t.pose)
    }
    pub fn advance_timelines(&mut self) {
        for t in self.timelines.tracks.values_mut() {
            t.pose = t.player.sample(self.now);
        }
    }
    pub fn timelines_active(&self) -> bool {
        self.timelines
            .tracks
            .values()
            .any(|t| t.player.is_playing())
    }
    pub fn settle_timelines(&mut self) {
        for t in self.timelines.tracks.values_mut() {
            t.player.finish();
            t.pose = t.player.sample(self.now);
        }
    }
}
