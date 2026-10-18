//! The animation controller's share of what an animated instance plays:
//! its projected state advanced to a time, as weighted clip times. Direct
//! playback ([`PlaybackState`]) lives with the retained model.

use render_model::{
    wrap, AnimatedMeshClipPose, AnimatedMeshPlaybackCommand, AnimationLoopMode, ClipAction,
};
use render_presentation::AnimationControllerProjectionState;

/// An animation controller's projected state, received at `received_at`.
#[derive(Clone, Debug)]
pub struct ControllerPlayback {
    pub state: AnimationControllerProjectionState,
    /// The Engine time this state was received at.
    pub received_at: f64,
    /// The clips this state names, resolved to a clip table's indices when
    /// the state or the table changes. A frame matches the few clips the
    /// state samples against this table.
    clips: Vec<(String, usize)>,
}

impl ControllerPlayback {
    pub fn new(state: AnimationControllerProjectionState, received_at: f64) -> Self {
        Self {
            state,
            received_at,
            clips: Vec::new(),
        }
    }

    /// Binds the clips the state names to `index`'s clip table.
    pub fn resolve(&mut self, index: &dyn Fn(&str) -> Option<usize>) {
        self.clips.clear();
        let motions = std::iter::once(&self.state.motion).chain(
            self.state
                .transition
                .iter()
                .map(|transition| &transition.target_motion),
        );
        let names = motions
            .flat_map(|motion| std::iter::once(&motion.clip_a).chain(motion.clip_b.as_ref()))
            .chain(self.state.clip_phases.iter().map(|phase| &phase.clip));
        for name in names {
            if self.clips.iter().any(|(known, _)| known == name) {
                continue;
            }
            if let Some(clip) = index(name) {
                self.clips.push((name.clone(), clip));
            }
        }
    }

    pub fn clip(&self, name: &str) -> Option<usize> {
        self.clips
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, index)| *index)
    }

    /// The weighted clip times at `now`: the published phases advanced by
    /// each clip's speed since the update.
    pub fn pose(&self, now: f64) -> Vec<AnimatedMeshClipPose> {
        let elapsed = (now - self.received_at).max(0.0);
        let mut state = self.state.clone();
        let speed = |clip: &str| {
            std::iter::once(&self.state.motion)
                .chain(
                    self.state
                        .transition
                        .iter()
                        .map(|transition| &transition.target_motion),
                )
                .find(|motion| motion.clip_a == clip || motion.clip_b.as_deref() == Some(clip))
                .map_or(1.0, |motion| f64::from(motion.speed_milli) / 1000.0)
        };
        for phase in &mut state.clip_phases {
            phase.time_seconds += elapsed * speed(&phase.clip);
        }
        match state.frozen_pose(self.state.phase_seconds + elapsed) {
            AnimatedMeshPlaybackCommand::SamplePose { clips } => clips,
            _ => Vec::new(),
        }
    }

    /// [`Self::pose`] as resolved clip actions, each clip's time repeating.
    pub fn actions(&self, now: f64, duration: &dyn Fn(usize) -> f32) -> Vec<ClipAction> {
        self.pose(now)
            .into_iter()
            .filter_map(|clip| {
                let index = self.clip(&clip.clip)?;
                let time = wrap(
                    clip.time_seconds,
                    duration(index),
                    AnimationLoopMode::Repeat,
                );
                Some((index, time, clip.weight))
            })
            .collect()
    }
}
