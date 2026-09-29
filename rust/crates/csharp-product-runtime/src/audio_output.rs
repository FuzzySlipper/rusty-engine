//! Audio realized on this process's output device.
//!
//! `RUSTY_AUDIO_OUTPUT=device` opens the default output device when the
//! runtime loads and closes it when the runtime drops. Committed audio ops
//! then play here instead of in the browser: they are taken out of the
//! published presentation so no second realization plays them or reports
//! feedback for them. Natural completions and device diagnostics reach the
//! Engine through the same realization facts the browser reports.

use csharp_engine_services::{AudioRealizationFact, EngineServiceSet};
use render_audio::{AudioRealizer, NoEntityPositions, RealizedAudioFact};
use render_presentation::{PresentationFrameDiff, PresentationOp};
use runtime_publication::RuntimePublication;

use crate::{native_audio_diagnostic_code, CsharpProductRuntimeError};

pub(crate) const AUDIO_OUTPUT_ENV: &str = "RUSTY_AUDIO_OUTPUT";
const AUDIO_OUTPUT_DEVICE: &str = "device";
/// The Engine's realization feedback admits this many facts per report.
const MAX_FACTS_PER_REPORT: usize = 128;

pub(crate) struct AudioOutput {
    realizer: AudioRealizer,
    next_fact_id: u64,
}

impl AudioOutput {
    /// The device path is opt-in until the desktop shell owns it. An unset
    /// variable keeps browser realization; an unknown value or a device
    /// that will not open is an error rather than silent fallback.
    pub(crate) fn from_environment() -> Result<Option<Self>, CsharpProductRuntimeError> {
        let Some(value) = std::env::var_os(AUDIO_OUTPUT_ENV) else {
            return Ok(None);
        };
        if value != AUDIO_OUTPUT_DEVICE {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_AUDIO_OUTPUT",
                format!("{AUDIO_OUTPUT_ENV} must be `{AUDIO_OUTPUT_DEVICE}` when set"),
            ));
        }
        let realizer = AudioRealizer::open_default_device()
            .map_err(|message| CsharpProductRuntimeError::new("CSHARP_AUDIO_OUTPUT", message))?;
        Ok(Some(Self {
            realizer,
            next_fact_id: 1,
        }))
    }

    /// Plays a committed call's audio ops and removes them from its
    /// publications.
    pub(crate) fn realize(
        &mut self,
        services: &EngineServiceSet,
        outputs: &mut [RuntimePublication],
    ) {
        let ops = take_audio_ops(outputs);
        if !ops.is_empty() {
            self.realizer.apply(
                &ops,
                &|hash: &str| services.audio_clip_bytes(hash),
                &NoEntityPositions,
            );
        }
    }

    /// Replaces the realization with the committed baseline after the
    /// realization owner was reset. Voices resume at their Engine cursors;
    /// earlier one-shots are not replayed.
    pub(crate) fn rebaseline(
        &mut self,
        services: &EngineServiceSet,
    ) -> Result<(), CsharpProductRuntimeError> {
        let baseline = services.audio_snapshot_frame()?;
        self.realizer.reset();
        self.realizer.apply(
            &baseline.ops,
            &|hash: &str| services.audio_clip_bytes(hash),
            &NoEntityPositions,
        );
        Ok(())
    }

    /// Reports what the device finished or failed since the last call, and
    /// releases decoded clips the Engine no longer owns. Call between
    /// product calls.
    pub(crate) fn report(
        &mut self,
        services: &mut EngineServiceSet,
    ) -> Result<(), CsharpProductRuntimeError> {
        self.realizer.refresh(&NoEntityPositions);
        self.realizer
            .retain_clips(|hash| services.audio_clip_bytes(hash).is_some());
        let facts = self.realizer.take_facts();
        for chunk in facts.chunks(MAX_FACTS_PER_REPORT) {
            let facts: Vec<_> = chunk
                .iter()
                .map(|fact| self.engine_fact(fact.clone()))
                .collect();
            services.ingest_audio_realization_feedback(false, 0, facts)?;
        }
        Ok(())
    }

    pub(crate) fn set_suspended(&mut self, suspended: bool) {
        self.realizer.set_suspended(suspended);
    }

    pub(crate) fn silence(&mut self) {
        self.realizer.reset();
    }

    fn engine_fact(&mut self, fact: RealizedAudioFact) -> AudioRealizationFact {
        let fact_id = self.next_fact_id;
        self.next_fact_id += 1;
        match fact {
            RealizedAudioFact::OneShotCompleted {
                sequence,
                signal_handle,
            } => AudioRealizationFact::NaturalCompletionOneShot {
                fact_id,
                sequence,
                signal_handle: signal_handle.raw(),
            },
            RealizedAudioFact::VoiceCompleted { sequence, handle } => {
                AudioRealizationFact::NaturalCompletionRetainedVoice {
                    fact_id,
                    sequence,
                    voice_handle: handle.raw(),
                }
            }
            RealizedAudioFact::Diagnostic {
                diagnostic,
                signal_handle,
            } => AudioRealizationFact::Diagnostic {
                fact_id,
                code: native_audio_diagnostic_code(diagnostic.code),
                sequence: diagnostic.sequence,
                signal_handle: signal_handle.map(|handle| handle.raw()),
                voice_handle: diagnostic.handle.map(|handle| handle.raw()),
            },
        }
    }
}

/// Removes audio ops from presentation publications, in order. A frame's
/// publication stamp counts its own ops, so the count follows the removal;
/// its revisions are unchanged.
pub(crate) fn take_audio_ops(outputs: &mut [RuntimePublication]) -> Vec<PresentationOp> {
    let mut taken = Vec::new();
    for output in outputs {
        let RuntimePublication::Presentation(frame) = output else {
            continue;
        };
        if !frame
            .ops
            .iter()
            .any(|op| matches!(op, PresentationOp::Audio { .. }))
        {
            continue;
        }
        let (audio, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut frame.ops)
            .into_iter()
            .partition(|op| matches!(op, PresentationOp::Audio { .. }));
        taken.extend(audio);
        frame.ops = rest;
        recount(frame);
    }
    taken
}

fn recount(frame: &mut PresentationFrameDiff) {
    if let Some(publication) = frame.publication.as_mut() {
        publication.operation_count = u32::try_from(frame.ops.len())
            .expect("a frame that fit its count still fits after removal");
    }
}

#[cfg(test)]
mod tests {
    use render_presentation::{
        AudioBus, AudioBusControl, AudioProjectionOp, PresentationOpMeta, TelemetryOverlayHandle,
        TelemetryOverlayProjectionOp,
    };

    use super::*;

    #[test]
    fn audio_ops_leave_the_publication_and_its_count_follows() {
        let audio = PresentationOp::Audio {
            meta: PresentationOpMeta::new(1),
            op: AudioProjectionOp::BusControl {
                bus: AudioBus::Sfx,
                control: AudioBusControl::SetMuted { muted: true },
            },
        };
        let other = PresentationOp::TelemetryOverlay {
            meta: PresentationOpMeta::new(2),
            op: TelemetryOverlayProjectionOp::Destroy {
                handle: TelemetryOverlayHandle::new(1),
            },
        };
        let mut frame = PresentationFrameDiff::new();
        frame.ops = vec![audio.clone(), other.clone()];
        frame.publication = Some(render_model::RenderFramePublication {
            stream: "presentation-world".to_owned(),
            base_revision: 4,
            revision: 5,
            operation_count: 2,
        });
        let mut outputs = vec![RuntimePublication::Presentation(frame)];

        assert_eq!(take_audio_ops(&mut outputs), [audio]);
        let RuntimePublication::Presentation(frame) = &outputs[0] else {
            unreachable!();
        };
        assert_eq!(frame.ops, [other]);
        let publication = frame.publication.as_ref().expect("stamp kept");
        assert_eq!((publication.revision, publication.operation_count), (5, 1));
    }
}
