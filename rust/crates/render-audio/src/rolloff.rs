//! Distance rolloff of a spatial voice, applied on the audio thread from the
//! listener distance kira reports to its spatial track.

use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;
use render_presentation::AudioRolloff;

/// The amplitude a spatial voice plays at by distance from the listener
/// ([`AudioRolloff::gain`]).
#[derive(Debug, Clone, Copy)]
pub(crate) struct DistanceRolloff {
    max: f32,
    model: AudioRolloff,
}

impl DistanceRolloff {
    pub(crate) fn new(max_distance: f32, model: AudioRolloff) -> Self {
        Self {
            max: max_distance,
            model,
        }
    }

    pub(crate) fn gain(self, distance: f32) -> f32 {
        self.model.gain(self.max, distance)
    }
}

impl EffectBuilder for DistanceRolloff {
    type Handle = ();

    fn build(self) -> (Box<dyn Effect>, Self::Handle) {
        (
            Box::new(RolloffEffect {
                rolloff: self,
                gain: None,
            }),
            (),
        )
    }
}

struct RolloffEffect {
    rolloff: DistanceRolloff,
    /// The gain the previous block ended at; each block ramps from it so a
    /// moving source or listener does not step.
    gain: Option<f32>,
}

impl Effect for RolloffEffect {
    fn process(&mut self, input: &mut [Frame], _dt: f64, info: &Info) {
        let target = info
            .listener_distance()
            .map_or(0.0, |distance| self.rolloff.gain(distance));
        let start = self.gain.unwrap_or(target);
        let frames = input.len() as f32;
        for (index, frame) in input.iter_mut().enumerate() {
            *frame *= start + (target - start) * (index + 1) as f32 / frames;
        }
        self.gain = Some(target);
    }
}
