//! Typed, host-neutral publications emitted by one product runtime.
//!
//! This module is the logical boundary between a concrete runtime owner and a
//! serving host. It carries Engine-owned render and UI facts, binding and
//! baseline markers, and no browser wire representation. HTTP/SSE envelopes,
//! output retention, byte limits, and delivery cursors stay with the serving
//! host that adapts these values.

use render_host_contracts::RendererViewComposition;
use render_model::RenderFrameDiff;
use render_presentation::PresentationFrameDiff;
use runtime_input::RuntimeInputBinding;
use runtime_ui::RuntimeUiProjectionEnvelope;

/// The neutral receipt carried by a runtime owner before a host adds delivery
/// metadata such as an SSE/output cursor: an operation's owned result and
/// logical outputs, independent of transport encoding, delivery cursors,
/// queue bounds, and host DTOs.
#[derive(Debug, Clone)]
pub struct RuntimeReceipt<T> {
    result: T,
    outputs: Vec<RuntimePublication>,
}

impl<T> RuntimeReceipt<T> {
    pub fn new(result: T, outputs: Vec<RuntimePublication>) -> Self {
        Self { result, outputs }
    }
    pub fn result(&self) -> &T {
        &self.result
    }
    pub fn into_parts(self) -> (T, Vec<RuntimePublication>) {
        (self.result, self.outputs)
    }
}

/// One logical output from a runtime operation.
///
/// Progress pulses, readouts, and input-result receipts remain operation/host
/// observations. They are intentionally not part of this Engine publication
/// model; the serving adapter may add those wire facts around a receipt.
/// Frames are owned Engine facts, moved here without validation or copying.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimePublication {
    Binding {
        runtime: RuntimeInputBinding,
        next_input_sequence: u64,
        /// The label of a harness that holds input under this binding. While
        /// one does, an attached page shows the claim and sends no input.
        input_claim: Option<String>,
        /// The desktop window delivers gameplay input under this binding
        /// (its pointer lock is held), so an attached page sends none.
        window_input: bool,
    },
    CompleteBaseline {
        runtime: RuntimeInputBinding,
    },
    Frame(RenderFrameDiff),
    ViewComposition(RendererViewComposition),
    Presentation(PresentationFrameDiff),
    UiProjection(RuntimeUiProjectionEnvelope),
}

impl RuntimePublication {
    pub fn binding(runtime: RuntimeInputBinding, next_input_sequence: u64) -> Self {
        Self::held_binding(runtime, next_input_sequence, None, false)
    }

    /// A binding whose input a harness's claim or the desktop window may
    /// hold instead of the page.
    pub fn held_binding(
        runtime: RuntimeInputBinding,
        next_input_sequence: u64,
        input_claim: Option<String>,
        window_input: bool,
    ) -> Self {
        Self::Binding {
            runtime,
            next_input_sequence,
            input_claim,
            window_input,
        }
    }

    pub fn complete_baseline(runtime: RuntimeInputBinding) -> Self {
        Self::CompleteBaseline { runtime }
    }

    /// Returns only newly emitted presentation events for baseline assembly.
    /// Retained presentation state is intentionally omitted from the returned
    /// publication so rebuilding a baseline cannot replay old effects.
    pub fn transient_presentation(&self) -> Option<Self> {
        let Self::Presentation(frame) = self else {
            return None;
        };
        let transient = frame.transient_events();
        if transient.is_empty() {
            None
        } else {
            Some(Self::Presentation(transient))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_presentation::{
        AudioBus, AudioClipRef, AudioEmitter, AudioProjectionOp, AudioSignalHandle,
        AudioSourceDescriptor, PresentationOp, PresentationOpMeta,
    };

    #[test]
    fn transient_presentation_drops_retained_operations() {
        let frame = PresentationFrameDiff {
            publication: None,
            schema_version: render_presentation::PRESENTATION_FRAME_SCHEMA_VERSION,
            ops: vec![PresentationOp::Audio {
                meta: PresentationOpMeta::new(0),
                op: AudioProjectionOp::Emit {
                    signal_handle: AudioSignalHandle::new(1),
                    signal_id: "footstep".to_owned(),
                    descriptor: AudioSourceDescriptor {
                        clip: AudioClipRef {
                            asset: "audio/footstep".to_owned(),
                            content_hash: "aa".to_owned(),
                            duration_seconds: None,
                        },
                        bus: AudioBus::Sfx,
                        volume: 1.0,
                        pitch: 1.0,
                        looping: false,
                        spatial_blend: 0.0,
                        max_distance: 1.0,
                        rolloff: render_presentation::AudioRolloff::Linear,
                        pan: 0.0,
                        emitter: AudioEmitter::Global2d,
                    },
                },
            }],
        };
        let publication = RuntimePublication::Presentation(frame);
        let transient = publication
            .transient_presentation()
            .expect("one emitted event");
        assert!(matches!(transient, RuntimePublication::Presentation(_)));
    }
}
