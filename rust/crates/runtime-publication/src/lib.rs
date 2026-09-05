//! Typed, host-neutral publications emitted by one product runtime.
//!
//! This crate is the logical boundary between a concrete runtime owner and a
//! serving host. It carries Engine-owned render and UI facts, binding and
//! baseline markers, and no browser wire representation. HTTP/SSE envelopes,
//! output retention, byte limits, and delivery cursors stay with the serving
//! host that adapts these values.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use render_host_contracts::RendererViewComposition;
use render_model::{RenderFrameDiff, JSON_SAFE_U64_MAX};
use render_presentation::PresentationFrameDiff;
use runtime_input::RuntimeInputBinding;
use runtime_session::RuntimeReceipt as SessionReceipt;
use runtime_ui::RuntimeUiProjectionEnvelope;

/// The neutral receipt carried by a runtime owner before a host adds delivery
/// metadata such as an SSE/output cursor.
pub type RuntimeReceipt<T> = SessionReceipt<T, RuntimePublication>;

/// A typed renderer stream frontier captured at a complete baseline boundary.
///
/// The renderer stream identity and JSON-safe revision are logical facts used
/// by downstream retained projection state. The serving host may apply any
/// additional envelope or aggregate limits when converting this value to its
/// wire DTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePublicationFrontier {
    stream: String,
    revision: u64,
}

impl RuntimePublicationFrontier {
    /// The renderer contract currently uses a bounded stream label and a
    /// JavaScript-safe revision. These checks happen before any host encoding
    /// so a typed baseline cannot contain an invalid retained frontier.
    pub fn new(stream: impl Into<String>, revision: u64) -> Result<Self, RuntimePublicationError> {
        let stream = stream.into();
        if stream.trim().is_empty() || stream.len() > 256 {
            return Err(RuntimePublicationError::InvalidFrontierStream);
        }
        if revision > JSON_SAFE_U64_MAX {
            return Err(RuntimePublicationError::InvalidFrontierRevision);
        }
        Ok(Self { stream, revision })
    }

    pub fn stream(&self) -> &str {
        &self.stream
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

/// Closed realization family for one typed animation cue definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAnimationCueSignalDomain {
    Audio,
    Particle,
}

/// Copied Engine animation facts. The marker remains in admitted milliseconds
/// until a host-specific renderer wire adapter derives its presentation unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAnimationCueDefinition {
    cue_id: String,
    asset: String,
    clip: String,
    marker_millis: u64,
    signal_domain: RuntimeAnimationCueSignalDomain,
    signal_id: String,
}

impl RuntimeAnimationCueDefinition {
    pub const MAX_DEFINITIONS: usize = 128;
    pub const MAX_TEXT_BYTES: usize = 96;

    pub fn new(
        cue_id: impl Into<String>,
        asset: impl Into<String>,
        clip: impl Into<String>,
        marker_millis: u64,
        signal_domain: RuntimeAnimationCueSignalDomain,
        signal_id: impl Into<String>,
    ) -> Result<Self, RuntimePublicationError> {
        let cue_id = cue_id.into();
        let asset = asset.into();
        let clip = clip.into();
        let signal_id = signal_id.into();
        for (field, value) in [
            ("cue_id", cue_id.as_str()),
            ("asset", asset.as_str()),
            ("clip", clip.as_str()),
            ("signal_id", signal_id.as_str()),
        ] {
            if value.is_empty() || value.len() > Self::MAX_TEXT_BYTES {
                return Err(RuntimePublicationError::InvalidAnimationCueField { field });
            }
        }
        Ok(Self {
            cue_id,
            asset,
            clip,
            marker_millis,
            signal_domain,
            signal_id,
        })
    }

    pub fn cue_id(&self) -> &str {
        &self.cue_id
    }

    pub fn asset(&self) -> &str {
        &self.asset
    }

    pub fn clip(&self) -> &str {
        &self.clip
    }

    pub const fn marker_millis(&self) -> u64 {
        self.marker_millis
    }

    pub const fn signal_domain(&self) -> RuntimeAnimationCueSignalDomain {
        self.signal_domain
    }

    pub fn signal_id(&self) -> &str {
        &self.signal_id
    }
}

/// One logical output from a runtime operation.
///
/// Progress pulses, readouts, and input-result receipts remain operation/host
/// observations. They are intentionally not part of this Engine publication
/// model; the serving adapter may add those wire facts around a receipt.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimePublication {
    Binding {
        runtime: RuntimeInputBinding,
        next_input_sequence: u64,
        publication_frontiers: Option<Vec<RuntimePublicationFrontier>>,
    },
    CompleteBaseline {
        runtime: RuntimeInputBinding,
        publication_frontiers: Vec<RuntimePublicationFrontier>,
    },
    Frame(RenderFrameDiff),
    ViewComposition(RendererViewComposition),
    Presentation(PresentationFrameDiff),
    AnimationCueDefinitions(Vec<RuntimeAnimationCueDefinition>),
    UiProjection(RuntimeUiProjectionEnvelope),
}

impl RuntimePublication {
    pub fn binding(runtime: RuntimeInputBinding, next_input_sequence: u64) -> Self {
        Self::Binding {
            runtime,
            next_input_sequence,
            publication_frontiers: None,
        }
    }

    pub fn frame(frame: &RenderFrameDiff) -> Result<Self, RuntimePublicationError> {
        frame
            .validate()
            .map_err(|_| RuntimePublicationError::InvalidFrame)?;
        Ok(Self::Frame(frame.clone()))
    }

    pub fn view_composition(
        composition: &RendererViewComposition,
    ) -> Result<Self, RuntimePublicationError> {
        composition
            .validate()
            .map_err(|_| RuntimePublicationError::InvalidViewComposition)?;
        Ok(Self::ViewComposition(composition.clone()))
    }

    pub fn presentation(frame: &PresentationFrameDiff) -> Result<Self, RuntimePublicationError> {
        frame
            .validate()
            .map_err(|_| RuntimePublicationError::InvalidPresentation)?;
        Ok(Self::Presentation(frame.clone()))
    }

    pub fn animation_cue_definitions(
        definitions: Vec<RuntimeAnimationCueDefinition>,
    ) -> Result<Self, RuntimePublicationError> {
        if definitions.len() > RuntimeAnimationCueDefinition::MAX_DEFINITIONS {
            return Err(RuntimePublicationError::TooManyAnimationCueDefinitions);
        }
        let mut keys = BTreeSet::new();
        for definition in &definitions {
            definition.validate()?;
            if !keys.insert((definition.asset(), definition.clip(), definition.cue_id())) {
                return Err(RuntimePublicationError::DuplicateAnimationCueDefinition);
            }
        }
        Ok(Self::AnimationCueDefinitions(definitions))
    }

    pub fn ui_projection(
        envelope: &RuntimeUiProjectionEnvelope,
    ) -> Result<Self, RuntimePublicationError> {
        envelope
            .encode_json()
            .map_err(|_| RuntimePublicationError::InvalidUiProjection)?;
        Ok(Self::UiProjection(envelope.clone()))
    }

    pub fn complete_baseline_with_frontiers(
        runtime: RuntimeInputBinding,
        publication_frontiers: Vec<RuntimePublicationFrontier>,
    ) -> Self {
        Self::CompleteBaseline {
            runtime,
            publication_frontiers,
        }
    }

    pub fn complete_baseline(runtime: RuntimeInputBinding) -> Self {
        Self::complete_baseline_with_frontiers(runtime, Vec::new())
    }

    /// Validates the typed facts before a serving host retains or converts
    /// them. This intentionally does not inspect wire aggregate sizes.
    pub fn validate(&self) -> Result<(), RuntimePublicationError> {
        match self {
            Self::Binding {
                publication_frontiers,
                ..
            } => validate_frontiers(publication_frontiers.as_deref().unwrap_or_default()),
            Self::CompleteBaseline {
                publication_frontiers,
                ..
            } => validate_frontiers(publication_frontiers),
            Self::Frame(frame) => frame
                .validate()
                .map_err(|_| RuntimePublicationError::InvalidFrame),
            Self::ViewComposition(composition) => composition
                .validate()
                .map_err(|_| RuntimePublicationError::InvalidViewComposition),
            Self::Presentation(frame) => frame
                .validate()
                .map_err(|_| RuntimePublicationError::InvalidPresentation),
            Self::AnimationCueDefinitions(definitions) => {
                if definitions.len() > RuntimeAnimationCueDefinition::MAX_DEFINITIONS {
                    return Err(RuntimePublicationError::TooManyAnimationCueDefinitions);
                }
                let mut keys = BTreeSet::new();
                for definition in definitions {
                    definition.validate()?;
                    if !keys.insert((definition.asset(), definition.clip(), definition.cue_id())) {
                        return Err(RuntimePublicationError::DuplicateAnimationCueDefinition);
                    }
                }
                Ok(())
            }
            Self::UiProjection(envelope) => envelope
                .encode_json()
                .map(|_| ())
                .map_err(|_| RuntimePublicationError::InvalidUiProjection),
        }
    }

    /// Returns only newly emitted presentation events for baseline assembly.
    /// Retained presentation state is intentionally omitted from the returned
    /// publication so rebuilding a baseline cannot replay old effects.
    pub fn transient_presentation(&self) -> Result<Option<Self>, RuntimePublicationError> {
        let Self::Presentation(frame) = self else {
            return Ok(None);
        };
        let transient = frame.transient_events();
        if transient.is_empty() {
            Ok(None)
        } else {
            Self::presentation(&transient).map(Some)
        }
    }

    pub const fn binding_marker(&self) -> Option<RuntimeInputBinding> {
        match self {
            Self::Binding { runtime, .. } => Some(*runtime),
            _ => None,
        }
    }

    pub const fn complete_baseline_marker(&self) -> Option<RuntimeInputBinding> {
        match self {
            Self::CompleteBaseline { runtime, .. } => Some(*runtime),
            _ => None,
        }
    }
}

impl RuntimeAnimationCueDefinition {
    pub fn validate(&self) -> Result<(), RuntimePublicationError> {
        for (field, value) in [
            ("cue_id", self.cue_id()),
            ("asset", self.asset()),
            ("clip", self.clip()),
            ("signal_id", self.signal_id()),
        ] {
            if value.is_empty() || value.len() > Self::MAX_TEXT_BYTES {
                return Err(RuntimePublicationError::InvalidAnimationCueField { field });
            }
        }
        Ok(())
    }
}

fn validate_frontiers(
    frontiers: &[RuntimePublicationFrontier],
) -> Result<(), RuntimePublicationError> {
    let mut streams = BTreeSet::new();
    for frontier in frontiers {
        if frontier.stream.trim().is_empty() || frontier.stream.len() > 256 {
            return Err(RuntimePublicationError::InvalidFrontierStream);
        }
        if frontier.revision > JSON_SAFE_U64_MAX {
            return Err(RuntimePublicationError::InvalidFrontierRevision);
        }
        if !streams.insert(frontier.stream.as_str()) {
            return Err(RuntimePublicationError::DuplicateFrontierStream);
        }
    }
    Ok(())
}

/// Typed publication validation failures. Host adapters map these to their
/// own bounded diagnostics without serializing this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePublicationError {
    InvalidFrontierStream,
    InvalidFrontierRevision,
    DuplicateFrontierStream,
    InvalidAnimationCueField { field: &'static str },
    TooManyAnimationCueDefinitions,
    DuplicateAnimationCueDefinition,
    InvalidFrame,
    InvalidViewComposition,
    InvalidPresentation,
    InvalidUiProjection,
}

impl std::fmt::Display for RuntimePublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFrontierStream => "runtime publication frontier stream is invalid",
            Self::InvalidFrontierRevision => {
                "runtime publication frontier revision is outside the renderer range"
            }
            Self::DuplicateFrontierStream => {
                "runtime publication frontiers contain a duplicate stream"
            }
            Self::InvalidAnimationCueField { field } => {
                return write!(formatter, "runtime animation cue field {field} is invalid")
            }
            Self::TooManyAnimationCueDefinitions => {
                "runtime animation cue definitions exceed the bounded replacement"
            }
            Self::DuplicateAnimationCueDefinition => {
                "runtime animation cue definitions contain a duplicate identity"
            }
            Self::InvalidFrame => "runtime publication frame is invalid",
            Self::InvalidViewComposition => "runtime publication view composition is invalid",
            Self::InvalidPresentation => "runtime publication presentation is invalid",
            Self::InvalidUiProjection => "runtime publication UI projection is invalid",
        })
    }
}

impl std::error::Error for RuntimePublicationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use render_model::{Geometry, RenderDiff, RenderHandle};
    use render_presentation::{
        AudioBus, AudioClipRef, AudioEmitter, AudioProjectionOp, AudioSignalHandle,
        AudioSourceDescriptor, PresentationOp, PresentationOpMeta,
    };
    use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
    use runtime_ui::RuntimeUiRuntimeBinding;

    fn binding() -> RuntimeInputBinding {
        RuntimeInputBinding::new(
            RuntimeInstanceId::new(7),
            RuntimeGeneration::new(2),
            RuntimeControlRevision::new(3),
        )
    }

    #[test]
    fn typed_frame_and_binding_validate_without_wire_encoding() {
        let frame = RenderFrameDiff::try_from_ops(vec![RenderDiff::Create {
            handle: RenderHandle::new(1),
            parent: None,
            node: render_model::RenderNode::new(Geometry::Cube),
        }])
        .expect("fixture frame");
        let publication = RuntimePublication::frame(&frame).expect("typed frame");
        assert!(matches!(publication, RuntimePublication::Frame(_)));
        assert_eq!(
            RuntimePublication::binding(binding(), 11).binding_marker(),
            Some(binding())
        );
    }

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
                        attenuation: 1.0,
                        pan: 0.0,
                        emitter: AudioEmitter::Global2d,
                    },
                },
            }],
        };
        let publication = RuntimePublication::presentation(&frame).expect("typed presentation");
        let transient = publication
            .transient_presentation()
            .expect("transient extraction")
            .expect("one emitted event");
        assert!(matches!(transient, RuntimePublication::Presentation(_)));
        assert!(transient.validate().is_ok());
    }

    #[test]
    fn cue_validation_rejects_duplicate_identity() {
        let cue = |id| {
            RuntimeAnimationCueDefinition::new(
                id,
                "hero",
                "walk",
                125,
                RuntimeAnimationCueSignalDomain::Audio,
                "footstep",
            )
            .expect("cue")
        };
        assert_eq!(
            RuntimePublication::animation_cue_definitions(vec![cue("left"), cue("left")]),
            Err(RuntimePublicationError::DuplicateAnimationCueDefinition)
        );
    }

    #[test]
    fn frontier_validation_rejects_invalid_or_duplicate_streams() {
        assert_eq!(
            RuntimePublicationFrontier::new("", 0),
            Err(RuntimePublicationError::InvalidFrontierStream)
        );
        let first = RuntimePublicationFrontier::new("world", 4).unwrap();
        let second = RuntimePublicationFrontier::new("world", 5).unwrap();
        assert_eq!(
            RuntimePublication::complete_baseline_with_frontiers(binding(), vec![first, second])
                .validate(),
            Err(RuntimePublicationError::DuplicateFrontierStream)
        );
    }

    #[test]
    fn ui_projection_stays_typed() {
        let envelope = RuntimeUiProjectionEnvelope::new(
            RuntimeUiRuntimeBinding::new(
                RuntimeInstanceId::new(7),
                RuntimeGeneration::new(2),
                RuntimeControlRevision::new(3),
            ),
            1,
            "hud",
            "fixture",
            serde_json::json!({"visible": true}),
        )
        .expect("fixture envelope");
        let publication = RuntimePublication::ui_projection(&envelope).expect("typed UI");
        assert!(publication.validate().is_ok());
    }
}
