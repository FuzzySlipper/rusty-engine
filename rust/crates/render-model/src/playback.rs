//! What an animated instance plays at a time through direct playback: the
//! current playback and a prior one fading out under it, as weighted clip
//! times. The Engine's pose evaluation (`animation-pose`) poses from this,
//! the retained presentation world keeps it, and the Engine services sample
//! an instance's current playback from it.
//!
//! Clips are named by descriptor id; [`DirectPlayback::resolve`] binds them
//! to a clip table's indices, which [`PlaybackState::actions`] reads.

use serde::{Deserialize, Serialize};

use crate::{
    AnimatedMeshClipPose, AnimatedMeshPlaybackCommand, AnimatedMeshPlaybackTimeline,
    AnimationLoopMode,
};

/// One clip at a time with a weight: `(clip index, seconds, weight)`.
pub type ClipAction = (usize, f32, f32);

/// One direct-playback state, kept as it fades out.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DirectPlayback {
    pub timeline: AnimatedMeshPlaybackTimeline,
    /// A frozen `SamplePose`: clips with their times and weights.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose: Option<Vec<AnimatedMeshClipPose>>,
    /// The timeline's clip and the pose's clips as clip indices, resolved
    /// when the playback or the clip table changes ([`Self::resolve`]).
    #[serde(skip)]
    timeline_clip: Option<usize>,
    #[serde(skip)]
    pose_clips: Vec<Option<usize>>,
}

/// Resolved clip indices are a reader's cache, not part of the playback.
impl PartialEq for DirectPlayback {
    fn eq(&self, other: &Self) -> bool {
        self.timeline == other.timeline && self.pose == other.pose
    }
}

impl DirectPlayback {
    pub fn unset() -> Self {
        Self {
            timeline: AnimatedMeshPlaybackTimeline::Unset,
            pose: None,
            timeline_clip: None,
            pose_clips: Vec::new(),
        }
    }

    /// Binds the playback's clips to `index`'s clip table.
    pub fn resolve(&mut self, index: &dyn Fn(&str) -> Option<usize>) {
        self.timeline_clip = self.current_clip().and_then(|clip| index(&clip));
        self.pose_clips = self
            .pose
            .iter()
            .flatten()
            .map(|clip| index(&clip.clip))
            .collect();
    }

    pub fn current_clip(&self) -> Option<String> {
        match &self.timeline {
            AnimatedMeshPlaybackTimeline::Playing { clip, .. }
            | AnimatedMeshPlaybackTimeline::Paused { clip, .. }
            | AnimatedMeshPlaybackTimeline::Sampled { clip, .. } => Some(clip.clone()),
            _ => None,
        }
    }

    /// Clip times at `now`, each wrapped by its loop mode, and whether a
    /// `once` clip has reached its end. An unresolved clip draws nothing.
    pub fn actions(&self, now: f64, duration: &dyn Fn(usize) -> f32) -> (Vec<ClipAction>, bool) {
        let length = |clip: Option<usize>| clip.map_or(0.0, duration);
        if let Some(pose) = &self.pose {
            let actions = pose
                .iter()
                .zip(&self.pose_clips)
                .filter_map(|(clip, index)| {
                    let time = wrap(clip.time_seconds, length(*index), AnimationLoopMode::Repeat);
                    index.map(|index| (index, time, clip.weight))
                })
                .collect();
            return (actions, false);
        }
        let clip = self.timeline_clip;
        let action = |time: f32, weight: f32| clip.map(|index| (index, time, weight));
        match &self.timeline {
            AnimatedMeshPlaybackTimeline::Playing {
                r#loop,
                speed,
                weight,
                base_offset_seconds,
                anchor_seconds,
                ..
            } => {
                let raw = base_offset_seconds + (now - anchor_seconds) * f64::from(*speed);
                let length = length(clip);
                let finished = *r#loop == AnimationLoopMode::Once && raw >= f64::from(length);
                (
                    action(wrap(raw, length, *r#loop), *weight)
                        .into_iter()
                        .collect(),
                    finished,
                )
            }
            AnimatedMeshPlaybackTimeline::Paused {
                r#loop,
                weight,
                offset_seconds,
                ..
            } => (
                action(wrap(*offset_seconds, length(clip), *r#loop), *weight)
                    .into_iter()
                    .collect(),
                false,
            ),
            AnimatedMeshPlaybackTimeline::Sampled {
                normalized_time, ..
            } => (
                action(length(clip) * normalized_time, 1.0)
                    .into_iter()
                    .collect(),
                false,
            ),
            AnimatedMeshPlaybackTimeline::Unset | AnimatedMeshPlaybackTimeline::Stopped => {
                (Vec::new(), false)
            }
        }
    }
}

/// A prior playback fading out from `start` over `duration`, Engine
/// presentation seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaybackFade {
    pub prior: DirectPlayback,
    pub start: f64,
    pub duration: f64,
    /// A cross-fade also ramps the new clip in; a stop only fades out.
    pub cross: bool,
}

/// An instance's direct playback and the prior one fading out under it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackState {
    pub playback: DirectPlayback,
    pub fade: Option<PlaybackFade>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            playback: DirectPlayback::unset(),
            fade: None,
        }
    }
}

impl PlaybackState {
    /// Applies a playback command at `now`: a `Play` with a fade to another
    /// clip cross-fades from the playback before it, a `Stop` with a fade
    /// fades it out. The clips are unresolved until [`Self::resolve`].
    pub fn apply(&mut self, command: &AnimatedMeshPlaybackCommand, now: f64) -> Result<(), String> {
        let prior = self.playback.clone();
        let prior_clip = prior.current_clip();
        self.playback
            .timeline
            .apply_command(command, now)
            .map_err(|error| format!("{error:?}"))?;
        self.playback.pose = match command {
            AnimatedMeshPlaybackCommand::SamplePose { clips } => Some(clips.clone()),
            _ => None,
        };
        self.fade = match command {
            AnimatedMeshPlaybackCommand::Play {
                clip, fade_seconds, ..
            } => {
                let fade = fade_seconds.unwrap_or(0.0);
                (fade > 0.0 && prior_clip.as_deref().is_some_and(|prior| prior != clip)).then(
                    || PlaybackFade {
                        prior,
                        start: now,
                        duration: f64::from(fade),
                        cross: true,
                    },
                )
            }
            AnimatedMeshPlaybackCommand::Stop { fade_seconds } => {
                let fade = fade_seconds.unwrap_or(0.0);
                (fade > 0.0).then(|| PlaybackFade {
                    prior,
                    start: now,
                    duration: f64::from(fade),
                    cross: false,
                })
            }
            _ => None,
        };
        Ok(())
    }

    /// Binds the playback's clips, and its fading prior's, to `index`'s clip
    /// table.
    pub fn resolve(&mut self, index: &dyn Fn(&str) -> Option<usize>) {
        self.playback.resolve(index);
        if let Some(fade) = &mut self.fade {
            fade.prior.resolve(index);
        }
    }

    /// The weighted clip times at `now`: the fading prior playback weighted
    /// down, then `controller`'s clips (a controller replaces direct
    /// playback) or the direct playback's, ramped in by a cross-fade; and
    /// whether a `once` clip of the direct playback has reached its end. A
    /// fade that has ended is dropped first.
    pub fn actions(
        &mut self,
        now: f64,
        duration: &dyn Fn(usize) -> f32,
        controller: Option<Vec<ClipAction>>,
    ) -> (Vec<ClipAction>, bool) {
        if self
            .fade
            .as_ref()
            .is_some_and(|fade| now >= fade.start + fade.duration)
        {
            self.fade = None;
        }
        let mut actions = Vec::new();
        let progress = self
            .fade
            .as_ref()
            .map(|fade| ((now - fade.start) / fade.duration).clamp(0.0, 1.0) as f32);
        if let (Some(fade), Some(progress)) = (&self.fade, progress) {
            for (clip, time, weight) in fade.prior.actions(now, duration).0 {
                actions.push((clip, time, weight * (1.0 - progress)));
            }
        }
        let (current, finished) = match controller {
            Some(clips) => (clips, false),
            None => self.playback.actions(now, duration),
        };
        let fade_in = match (&self.fade, progress) {
            (Some(fade), Some(progress)) if fade.cross => progress,
            _ => 1.0,
        };
        actions.extend(
            current
                .into_iter()
                .map(|(clip, time, weight)| (clip, time, weight * fade_in)),
        );
        (actions, finished)
    }
}

/// A clip time wrapped by loop mode: `once` clamps, `repeat` wraps,
/// `pingPong` reflects on odd loops.
pub fn wrap(time: f64, duration: f32, mode: AnimationLoopMode) -> f32 {
    let duration = f64::from(duration);
    if duration <= 0.0 {
        return 0.0;
    }
    let wrapped = match mode {
        AnimationLoopMode::Once => time.clamp(0.0, duration),
        AnimationLoopMode::Repeat => time.rem_euclid(duration),
        AnimationLoopMode::PingPong => {
            let cycle = (time / duration).floor();
            let within = time - cycle * duration;
            if (cycle as i64).rem_euclid(2) == 1 {
                duration - within
            } else {
                within
            }
        }
    };
    wrapped as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_modes_wrap_clip_time() {
        assert_eq!(wrap(1.5, 1.0, AnimationLoopMode::Once), 1.0);
        assert_eq!(wrap(1.25, 1.0, AnimationLoopMode::Repeat), 0.25);
        assert_eq!(wrap(1.25, 1.0, AnimationLoopMode::PingPong), 0.75);
        assert_eq!(wrap(2.25, 1.0, AnimationLoopMode::PingPong), 0.25);
    }
}
