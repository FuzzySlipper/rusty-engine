//! Distance rolloff of a spatial voice, applied on the audio thread from the
//! listener distance kira reports to its spatial track.

use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;
use render_presentation::AudioRolloff;

/// Emitters within this distance play at full volume, as the browser
/// panner's reference distance of 1.
const REFERENCE_DISTANCE: f32 = 1.0;
/// The level `LinearDecibels` falls to at the maximum distance.
const SILENCE_DECIBELS: f32 = -60.0;

/// The amplitude a spatial voice plays at by distance from the listener: one
/// within `min(1, max / 2)`, falling by its model to zero at `max`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DistanceRolloff {
    reference: f32,
    max: f32,
    model: AudioRolloff,
}

impl DistanceRolloff {
    pub(crate) fn new(max_distance: f32, model: AudioRolloff) -> Self {
        Self {
            reference: REFERENCE_DISTANCE.min(max_distance * 0.5),
            max: max_distance,
            model,
        }
    }

    pub(crate) fn gain(self, distance: f32) -> f32 {
        if distance >= self.max {
            return 0.0;
        }
        let fraction = ((distance - self.reference) / (self.max - self.reference)).clamp(0.0, 1.0);
        match self.model {
            AudioRolloff::Linear => 1.0 - fraction,
            AudioRolloff::LinearDecibels => 10.0_f32.powf(SILENCE_DECIBELS * fraction / 20.0),
        }
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
