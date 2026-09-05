use render_model::JSON_SAFE_U64_MAX;
pub use runtime_diagnostics::CanonicalU64;
use runtime_diagnostics::{RuntimeDiagnosticRuntimeBinding, RuntimeUpdateAttribution};
use runtime_input::RuntimeInputEvent;
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use runtime_timeline::{
    RuntimeOpaqueData, RuntimeProvenance, RuntimeTimelineBinding, TimelineCompletionEnvelope,
    TimelineCompletionOutcome, TimelineCompletionTicketId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ProductDevHostError, ProductDevInvalidatedScope, ProductDevMutationCertainty,
    ProductDevNextAction, ProductDevRuntimeError, ProductDevRuntimeRecovery,
    MAX_BASELINE_AGGREGATE_BYTES, MAX_OUTPUT_AGGREGATE_BYTES, MAX_OUTPUT_QUEUE_ITEMS,
};

/// Fixed Engine-owned local-runtime route prefix consumed by product-browser-host.
pub const PRODUCT_DEV_RUNTIME_BASE_PATH: &str = "/__rusty/product/runtime/";
/// Identity for this local development host, not a product release/schema number.
pub const PRODUCT_DEV_HOST_ARTIFACT: &str = "rusty.product.dev-host";

/// Exact runtime generation binding used by browser input, operations, and outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevRuntimeBinding {
    pub instance_id: CanonicalU64,
    pub generation: CanonicalU64,
    pub control_revision: CanonicalU64,
}

impl From<ProductDevRuntimeBinding> for RuntimeDiagnosticRuntimeBinding {
    fn from(value: ProductDevRuntimeBinding) -> Self {
        Self {
            instance_id: value.instance_id,
            generation: value.generation,
            control_revision: value.control_revision,
        }
    }
}

impl From<RuntimeDiagnosticRuntimeBinding> for ProductDevRuntimeBinding {
    fn from(value: RuntimeDiagnosticRuntimeBinding) -> Self {
        Self {
            instance_id: value.instance_id,
            generation: value.generation,
            control_revision: value.control_revision,
        }
    }
}

/// Closed lifecycle vocabulary with dedicated HTTP routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductDevLifecycleOperation {
    Start,
    Pause,
    Resume,
    Restart,
    Shutdown,
    ReportFault,
}

/// Closed host control vocabulary. These operations change only which current
/// controller binding may submit later input; product simulation meaning stays
/// with the downstream product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductDevControlOperation {
    Replace,
    Release,
}

impl ProductDevControlOperation {
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Release => "release",
        }
    }

    pub const fn operation_kind(self) -> ProductDevOperationKind {
        match self {
            Self::Replace => ProductDevOperationKind::ReplaceControl,
            Self::Release => ProductDevOperationKind::ReleaseControl,
        }
    }
}

impl ProductDevLifecycleOperation {
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Restart => "restart",
            Self::Shutdown => "shutdown",
            Self::ReportFault => "report-fault",
        }
    }

    pub const fn operation_kind(self) -> ProductDevOperationKind {
        match self {
            Self::Start => ProductDevOperationKind::Start,
            Self::Pause => ProductDevOperationKind::Pause,
            Self::Resume => ProductDevOperationKind::Resume,
            Self::Restart => ProductDevOperationKind::Restart,
            Self::Shutdown => ProductDevOperationKind::Shutdown,
            Self::ReportFault => ProductDevOperationKind::ReportFault,
        }
    }
}

/// Closed operation identities returned by direct runtime calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevOperationKind {
    Connect,
    Start,
    Pause,
    Resume,
    Restart,
    Shutdown,
    ReportFault,
    ReplaceControl,
    ReleaseControl,
    Input,
    AdvanceRealtime,
    AdmitDemandStep,
    AdmitExternalStep,
    CompleteTimeline,
    ReportAudioFeedback,
    ReportAnimationFeedback,
    ReportGhostPlateFeedback,
    ReportRendererDiagnostics,
    ExecuteDebug,
}

/// Closed recovery posture for one host operation result. The code identifies
/// the precise failure while this value tells a host what it may safely do
/// next without interpreting the diagnostic text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevFaultDisposition {
    Accepted,
    RejectedRecoverable,
    Degraded,
    ResyncRequired,
    Terminal,
}

impl ProductDevFaultDisposition {
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

const ACCEPTED_FAULT_CODE: &str = "DEV_HOST_ACCEPTED";

pub fn runtime_fault_disposition(error: &ProductDevRuntimeError) -> ProductDevFaultDisposition {
    match error.recovery() {
        ProductDevRuntimeRecovery {
            mutation: ProductDevMutationCertainty::NotApplied,
            invalidated_scope: ProductDevInvalidatedScope::None,
            next_action: ProductDevNextAction::Continue,
        } => ProductDevFaultDisposition::RejectedRecoverable,
        ProductDevRuntimeRecovery {
            invalidated_scope: ProductDevInvalidatedScope::Outputs,
            next_action: ProductDevNextAction::Rebaseline,
            ..
        } => ProductDevFaultDisposition::ResyncRequired,
        // New failures do not need a central code registration. Unless their
        // source marks a pre-admission rejection, treat the current runtime
        // incarnation as tainted and preserve the existing terminal wire
        // disposition until the supervisor consumes replacement semantics.
        _ => ProductDevFaultDisposition::Terminal,
    }
}

fn runtime_fault_fields(
    error: ProductDevRuntimeError,
) -> (
    String,
    ProductDevFaultDisposition,
    String,
    ProductDevRuntimeRecovery,
) {
    let disposition = runtime_fault_disposition(&error);
    (
        error.code().to_owned(),
        disposition,
        bounded_diagnostic(error.diagnostic().to_owned())
            .expect("runtime error diagnostics are bounded at construction"),
        error.recovery(),
    )
}

/// Bounded, host-realized audio facts. These are observations from the
/// Engine browser host rather than audio projector/admission state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProductDevAudioFeedbackFact {
    NaturalCompletion {
        fact_id: CanonicalU64,
        sequence: u32,
        #[serde(flatten)]
        source: ProductDevAudioCompletionSource,
    },
    Diagnostic {
        fact_id: CanonicalU64,
        code: render_presentation::AudioProjectionDiagnosticCode,
        sequence: u32,
        voice_handle: Option<CanonicalU64>,
    },
}

/// The two Engine-owned realization identities. This matches the closed
/// browser-host transport shape without exposing browser objects downstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "source",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProductDevAudioCompletionSource {
    OneShot { signal_handle: CanonicalU64 },
    RetainedVoice { voice_handle: CanonicalU64 },
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum ProductDevAudioFeedbackFactWire {
    NaturalCompletion {
        fact_id: CanonicalU64,
        sequence: u32,
        source: String,
        #[serde(default)]
        signal_handle: Option<CanonicalU64>,
        #[serde(default)]
        voice_handle: Option<CanonicalU64>,
    },
    Diagnostic {
        fact_id: CanonicalU64,
        code: render_presentation::AudioProjectionDiagnosticCode,
        sequence: u32,
        voice_handle: Option<CanonicalU64>,
    },
}

fn decode_audio_feedback_facts<'de, D>(
    deserializer: D,
) -> Result<Vec<ProductDevAudioFeedbackFact>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<ProductDevAudioFeedbackFactWire>::deserialize(deserializer)?
        .into_iter()
        .map(|fact| match fact {
            ProductDevAudioFeedbackFactWire::NaturalCompletion {
                fact_id,
                sequence,
                source,
                signal_handle,
                voice_handle,
            } => match source.as_str() {
                "oneShot" if voice_handle.is_none() => signal_handle
                    .map(
                        |signal_handle| ProductDevAudioFeedbackFact::NaturalCompletion {
                            fact_id,
                            sequence,
                            source: ProductDevAudioCompletionSource::OneShot { signal_handle },
                        },
                    )
                    .ok_or_else(|| {
                        serde::de::Error::custom("oneShot completion requires signalHandle")
                    }),
                "retainedVoice" if signal_handle.is_none() => voice_handle
                    .map(
                        |voice_handle| ProductDevAudioFeedbackFact::NaturalCompletion {
                            fact_id,
                            sequence,
                            source: ProductDevAudioCompletionSource::RetainedVoice { voice_handle },
                        },
                    )
                    .ok_or_else(|| {
                        serde::de::Error::custom("retainedVoice completion requires voiceHandle")
                    }),
                _ => Err(serde::de::Error::custom(
                    "audio completion source and handle are incoherent",
                )),
            },
            ProductDevAudioFeedbackFactWire::Diagnostic {
                fact_id,
                code,
                sequence,
                voice_handle,
            } => Ok(ProductDevAudioFeedbackFact::Diagnostic {
                fact_id,
                code,
                sequence,
                voice_handle,
            }),
        })
        .collect()
}

impl ProductDevAudioFeedbackFact {
    pub const fn fact_id(&self) -> CanonicalU64 {
        match self {
            Self::NaturalCompletion { fact_id, .. } | Self::Diagnostic { fact_id, .. } => *fact_id,
        }
    }
}

/// One fixed host-to-runtime audio realization snapshot. `facts` is bounded
/// to the same 128-item FIFO retained by the browser Engine host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevAudioFeedback {
    pub runtime: ProductDevRuntimeBinding,
    pub replace_owner: bool,
    pub evicted_fact_count: CanonicalU64,
    #[serde(deserialize_with = "decode_audio_feedback_facts")]
    pub facts: Vec<ProductDevAudioFeedbackFact>,
}

impl ProductDevAudioFeedback {
    pub const MAX_FACTS: usize = 128;

    pub fn validate(&self) -> Result<(), ProductDevHostError> {
        if self.facts.len() > Self::MAX_FACTS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_AUDIO_FEEDBACK_BOUNDS",
                "audio feedback exceeds the 128 fact host bound",
            ));
        }
        if self
            .facts
            .windows(2)
            .any(|facts| facts[0].fact_id() >= facts[1].fact_id())
        {
            return Err(ProductDevHostError::new(
                "DEV_HOST_AUDIO_FEEDBACK_ORDER",
                "audio feedback fact ids must be strictly increasing",
            ));
        }
        Ok(())
    }
}

/// Fixed response for the browser host's audio feedback route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevAudioFeedbackResult {
    pub accepted: bool,
    pub code: String,
    pub disposition: ProductDevFaultDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<ProductDevRuntimeRecovery>,
    pub runtime: ProductDevRuntimeBinding,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_through_fact_id: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl ProductDevAudioFeedbackResult {
    pub fn accepted(
        runtime: ProductDevRuntimeBinding,
        accepted_through_fact_id: Option<CanonicalU64>,
    ) -> Self {
        Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductDevFaultDisposition::Accepted,
            recovery: None,
            runtime,
            accepted_through_fact_id,
            diagnostic: None,
        }
    }

    pub fn rejected(
        runtime: ProductDevRuntimeBinding,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_AUDIO_FEEDBACK_REJECTED".to_owned(),
            disposition: ProductDevFaultDisposition::RejectedRecoverable,
            recovery: None,
            runtime,
            accepted_through_fact_id: None,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }

    pub fn rejected_runtime(
        runtime: ProductDevRuntimeBinding,
        error: ProductDevRuntimeError,
    ) -> Result<Self, ProductDevHostError> {
        let (code, disposition, diagnostic, recovery) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
            recovery: Some(recovery),
            runtime,
            accepted_through_fact_id: None,
            diagnostic: Some(diagnostic),
        })
    }
}

/// Copied browser-renderer animation observations. Playback is deliberately an
/// observation, never a claim that a one-shot completed naturally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProductDevAnimationFeedbackFact {
    PlaybackObservation {
        fact_id: CanonicalU64,
        object_id: CanonicalU64,
        generation: CanonicalU64,
        sequence: u32,
        status: String,
        selected_clip: Option<String>,
        sampled_at_seconds: Option<f64>,
    },
    NaturalCompletion {
        fact_id: CanonicalU64,
        object_id: CanonicalU64,
        generation: CanonicalU64,
        clip: String,
    },
    Diagnostic {
        fact_id: CanonicalU64,
        object_id: Option<CanonicalU64>,
        generation: Option<CanonicalU64>,
        code: String,
        sequence: u32,
    },
    Cue {
        fact_id: CanonicalU64,
        object_id: CanonicalU64,
        generation: CanonicalU64,
        cue_id: String,
        clip: String,
        marker_seconds: f64,
        sampled_at_seconds: f64,
        signal_domain: String,
        signal_id: String,
    },
    Stopped {
        fact_id: CanonicalU64,
        object_id: CanonicalU64,
        generation: CanonicalU64,
        sequence: u32,
        reason: String,
    },
}

impl ProductDevAnimationFeedbackFact {
    pub const fn fact_id(&self) -> CanonicalU64 {
        match self {
            Self::PlaybackObservation { fact_id, .. }
            | Self::NaturalCompletion { fact_id, .. }
            | Self::Diagnostic { fact_id, .. }
            | Self::Cue { fact_id, .. }
            | Self::Stopped { fact_id, .. } => *fact_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevAnimationFeedback {
    pub runtime: ProductDevRuntimeBinding,
    pub replace_owner: bool,
    pub evicted_fact_count: CanonicalU64,
    pub facts: Vec<ProductDevAnimationFeedbackFact>,
}

impl ProductDevAnimationFeedback {
    pub const MAX_FACTS: usize = 128;
    /// Matches `NativeAnimationFeedbackText`; values cross the ABI inline.
    pub const MAX_INLINE_TEXT_BYTES: usize = 96;
    pub fn validate(&self) -> Result<(), ProductDevHostError> {
        if self.facts.len() > Self::MAX_FACTS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_ANIMATION_FEEDBACK_BOUNDS",
                "animation feedback exceeds the 128 fact host bound",
            ));
        }
        if self
            .facts
            .windows(2)
            .any(|facts| facts[0].fact_id() >= facts[1].fact_id())
        {
            return Err(ProductDevHostError::new(
                "DEV_HOST_ANIMATION_FEEDBACK_ORDER",
                "animation feedback fact ids must be strictly increasing",
            ));
        }
        for fact in &self.facts {
            match fact {
                ProductDevAnimationFeedbackFact::PlaybackObservation {
                    status,
                    selected_clip,
                    sampled_at_seconds,
                    ..
                } => {
                    if !matches!(
                        status.as_str(),
                        "unavailable"
                            | "not_started"
                            | "playing"
                            | "paused"
                            | "sampled"
                            | "stopped"
                    ) || !animation_feedback_text_fits(status)
                        || selected_clip
                            .as_ref()
                            .is_some_and(|clip| !animation_feedback_text_fits(clip))
                        || sampled_at_seconds.is_some_and(|time| !time.is_finite() || time < 0.0)
                    {
                        return Err(ProductDevHostError::new(
                            "DEV_HOST_ANIMATION_FEEDBACK_FACT",
                            "animation playback observation is invalid",
                        ));
                    }
                }
                ProductDevAnimationFeedbackFact::NaturalCompletion { clip, .. }
                    if !animation_feedback_text_fits(clip) =>
                {
                    return Err(ProductDevHostError::new(
                        "DEV_HOST_ANIMATION_FEEDBACK_FACT",
                        "animation natural completion is invalid",
                    ))
                }
                ProductDevAnimationFeedbackFact::Diagnostic { code, .. }
                    if !animation_feedback_text_fits(code) =>
                {
                    return Err(ProductDevHostError::new(
                        "DEV_HOST_ANIMATION_FEEDBACK_FACT",
                        "animation diagnostic code is empty",
                    ))
                }
                ProductDevAnimationFeedbackFact::Cue {
                    cue_id,
                    clip,
                    marker_seconds,
                    sampled_at_seconds,
                    signal_domain,
                    signal_id,
                    ..
                } if !animation_feedback_text_fits(cue_id)
                    || !animation_feedback_text_fits(clip)
                    || !animation_feedback_text_fits(signal_id)
                    || !animation_feedback_text_fits(signal_domain)
                    || !matches!(signal_domain.as_str(), "audio" | "particle")
                    || !marker_seconds.is_finite()
                    || !sampled_at_seconds.is_finite()
                    || *marker_seconds < 0.0
                    || *sampled_at_seconds < 0.0 =>
                {
                    return Err(ProductDevHostError::new(
                        "DEV_HOST_ANIMATION_FEEDBACK_FACT",
                        "animation cue observation is invalid",
                    ))
                }
                ProductDevAnimationFeedbackFact::Stopped { reason, .. }
                    if !animation_feedback_text_fits(reason)
                        || !matches!(reason.as_str(), "destroyed" | "teardown") =>
                {
                    return Err(ProductDevHostError::new(
                        "DEV_HOST_ANIMATION_FEEDBACK_FACT",
                        "animation stop observation is invalid",
                    ))
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn animation_feedback_text_fits(value: &str) -> bool {
    !value.is_empty() && value.len() <= ProductDevAnimationFeedback::MAX_INLINE_TEXT_BYTES
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevAnimationFeedbackResult {
    pub accepted: bool,
    pub code: String,
    pub disposition: ProductDevFaultDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<ProductDevRuntimeRecovery>,
    pub runtime: ProductDevRuntimeBinding,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_through_fact_id: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl ProductDevAnimationFeedbackResult {
    pub fn accepted(
        runtime: ProductDevRuntimeBinding,
        accepted_through_fact_id: Option<CanonicalU64>,
    ) -> Self {
        Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductDevFaultDisposition::Accepted,
            recovery: None,
            runtime,
            accepted_through_fact_id,
            diagnostic: None,
        }
    }
    pub fn rejected(
        runtime: ProductDevRuntimeBinding,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_ANIMATION_FEEDBACK_REJECTED".to_owned(),
            disposition: ProductDevFaultDisposition::RejectedRecoverable,
            recovery: None,
            runtime,
            accepted_through_fact_id: None,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }

    pub fn rejected_runtime(
        runtime: ProductDevRuntimeBinding,
        error: ProductDevRuntimeError,
    ) -> Result<Self, ProductDevHostError> {
        let (code, disposition, diagnostic, recovery) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
            recovery: Some(recovery),
            runtime,
            accepted_through_fact_id: None,
            diagnostic: Some(diagnostic),
        })
    }
}

/// Closed fallback classifications reported by the retained ghost-plate host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProductDevGhostPlateFallbackReason {
    None,
    PreparedSourceUnsupported,
    RealizationFailed,
}

/// One latest-state observation keyed by the opaque Engine ghost owner. This
/// is deliberately a bounded snapshot, not a renderer event stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevGhostPlateFeedbackFact {
    pub presentation: CanonicalU64,
    pub source_matches: bool,
    pub current_sector: u32,
    pub local_angular_offset_degrees: Option<f64>,
    pub fallback_active: bool,
    pub fallback_reason: ProductDevGhostPlateFallbackReason,
    /// Closed GhostPlateLimitationMask bits copied from the renderer host.
    pub limitation_mask: u32,
    pub preparation_cpu_milliseconds: Option<f64>,
    pub capture_cpu_submission_milliseconds: Option<f64>,
    pub retained_sector_count: u32,
    pub retained_mesh_count: u32,
    pub retained_material_count: u32,
    pub retained_borrowed_texture_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevGhostPlateFeedback {
    pub runtime: ProductDevRuntimeBinding,
    pub replace_owner: bool,
    pub facts: Vec<ProductDevGhostPlateFeedbackFact>,
}

impl ProductDevGhostPlateFeedback {
    pub const MAX_FACTS: usize = 128;

    pub fn validate(&self) -> Result<(), ProductDevHostError> {
        if self.facts.len() > Self::MAX_FACTS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_GHOST_PLATE_FEEDBACK_BOUNDS",
                "ghost plate feedback exceeds the 128 presentation bound",
            ));
        }
        let mut owners = std::collections::BTreeSet::new();
        for fact in &self.facts {
            let fallback_active = !matches!(
                fact.fallback_reason,
                ProductDevGhostPlateFallbackReason::None
            );
            let invalid_angle = fact
                .local_angular_offset_degrees
                .is_some_and(|value| !value.is_finite() || !(-360.0..=360.0).contains(&value));
            let invalid_timing = [
                fact.preparation_cpu_milliseconds,
                fact.capture_cpu_submission_milliseconds,
            ]
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite() || value < 0.0);
            let unsupported_limitation_mask = !matches!(fact.limitation_mask, 125 | 127);
            if fact.presentation.get() == 0
                || !owners.insert(fact.presentation.get())
                || fact.fallback_active != fallback_active
                || invalid_angle
                || invalid_timing
                || unsupported_limitation_mask
            {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_GHOST_PLATE_FEEDBACK_FACT",
                    "ghost plate feedback must contain unique owners, coherent fallback state, and finite observations",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevGhostPlateFeedbackResult {
    pub accepted: bool,
    pub code: String,
    pub disposition: ProductDevFaultDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<ProductDevRuntimeRecovery>,
    pub runtime: ProductDevRuntimeBinding,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl ProductDevGhostPlateFeedbackResult {
    pub fn accepted(runtime: ProductDevRuntimeBinding) -> Self {
        Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductDevFaultDisposition::Accepted,
            recovery: None,
            runtime,
            diagnostic: None,
        }
    }

    pub fn rejected(
        runtime: ProductDevRuntimeBinding,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_GHOST_PLATE_FEEDBACK_REJECTED".to_owned(),
            disposition: ProductDevFaultDisposition::RejectedRecoverable,
            recovery: None,
            runtime,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }

    pub fn rejected_runtime(
        runtime: ProductDevRuntimeBinding,
        error: ProductDevRuntimeError,
    ) -> Result<Self, ProductDevHostError> {
        let (code, disposition, diagnostic, recovery) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
            recovery: Some(recovery),
            runtime,
            diagnostic: Some(diagnostic),
        })
    }
}

/// Latest bounded browser-owned renderer observation. Its payload is a closed
/// versioned snapshot produced by renderer-host, not a command or telemetry bus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevRendererDiagnosticsFeedback {
    pub runtime: ProductDevRuntimeBinding,
    pub snapshot: Value,
}

impl ProductDevRendererDiagnosticsFeedback {
    pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;

    pub fn validate(&self) -> Result<(), ProductDevHostError> {
        let valid_version = self
            .snapshot
            .as_object()
            .and_then(|object| object.get("schemaVersion"))
            .and_then(Value::as_u64)
            == Some(1);
        let encoded = serde_json::to_vec(&self.snapshot).map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_RENDERER_DIAGNOSTICS_ENCODE",
                "renderer diagnostics snapshot could not be encoded",
            )
        })?;
        if !valid_version || encoded.len() > Self::MAX_SNAPSHOT_BYTES {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RENDERER_DIAGNOSTICS_BOUNDS",
                "renderer diagnostics must be a version 1 object within 256 KiB",
            ));
        }
        Ok(())
    }
}

/// Fixed browser-host observation batch. This is deliberately a small health
/// report, not a browser console or generic diagnostic transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevBrowserDiagnosticsReport {
    pub host_state: ProductDevBrowserHostState,
    pub runtime_progress: CanonicalU64,
    pub transport_state: ProductDevBrowserConnectionState,
    pub output_state: ProductDevBrowserConnectionState,
    pub last_renderer_sequence: Option<CanonicalU64>,
    pub renderer_observation_age_ms: Option<CanonicalU64>,
    pub first_terminal: Option<ProductDevBrowserTerminalDiagnostic>,
    pub recoverable_event: Option<ProductDevBrowserTerminalDiagnostic>,
    pub page_events: Vec<ProductDevBrowserPageDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevBrowserHostState {
    Loading,
    Ready,
    Degraded,
    Failed,
    Disposed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevBrowserConnectionState {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevBrowserTerminalDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevBrowserPageDiagnostic {
    pub kind: ProductDevBrowserPageDiagnosticKind,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevBrowserPageDiagnosticKind {
    Error,
    UnhandledRejection,
}

impl ProductDevBrowserDiagnosticsReport {
    pub const MAX_PAGE_EVENTS: usize = 8;

    pub fn validate(&self) -> Result<(), ProductDevHostError> {
        if self.page_events.len() > Self::MAX_PAGE_EVENTS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_BROWSER_DIAGNOSTICS_BOUNDS",
                "browser diagnostics page event count exceeds its fixed bound",
            ));
        }
        if let Some(diagnostic) = &self.first_terminal {
            validate_browser_diagnostic(&diagnostic.code, &diagnostic.message)?;
        }
        if let Some(diagnostic) = &self.recoverable_event {
            validate_browser_diagnostic(&diagnostic.code, &diagnostic.message)?;
            if !matches!(
                diagnostic.code.as_str(),
                "CSHARP_LIFECYCLE_CLOCK_REGRESSION"
                    | "BROWSER_RENDERER_DIAGNOSTICS_UNAVAILABLE"
                    | "BROWSER_LOCAL_REQUEST_UNAVAILABLE"
            ) {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_BROWSER_DIAGNOSTICS_BOUNDS",
                    "browser recoverable diagnostic code is not admitted",
                ));
            }
        }
        for diagnostic in &self.page_events {
            validate_browser_diagnostic(&diagnostic.code, &diagnostic.message)?;
        }
        Ok(())
    }
}

fn validate_browser_diagnostic(code: &str, message: &str) -> Result<(), ProductDevHostError> {
    if code.is_empty()
        || code.len() > 128
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
        || message.is_empty()
        || message.len() > 1_024
        || message
            .chars()
            .any(|character| character.is_control() && character != '\t')
    {
        return Err(ProductDevHostError::new(
            "DEV_HOST_BROWSER_DIAGNOSTICS_BOUNDS",
            "browser diagnostic code or message is outside fixed bounds",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevBrowserDiagnosticsResult {
    pub accepted: bool,
    pub reported: u8,
}

/// Bounded host-owned product-lane telemetry returned alongside the existing
/// diagnostics batch. These are observations only; renderer cadence remains
/// in the browser renderer diagnostics report and is intentionally not folded
/// into this product-lane snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevTelemetrySnapshot {
    pub in_flight_operation: Option<ProductDevOperationKind>,
    pub in_flight_age_ms: Option<CanonicalU64>,
    pub last_product_admission_latency_ms: Option<CanonicalU64>,
    pub last_input_admission_latency_ms: Option<CanonicalU64>,
    pub queued_input_batches: usize,
    pub queued_input_events: usize,
    pub input_batch_capacity: usize,
    pub oldest_input_age_ms: Option<CanonicalU64>,
    pub input_overflow_pending: bool,
    /// Progress rate in millihertz, retaining useful values below one update
    /// per second without introducing floating point into the wire snapshot.
    pub runtime_progress_rate_millihertz: Option<CanonicalU64>,
    pub runtime_progress_age_ms: Option<CanonicalU64>,
    pub connections: usize,
    pub subscribers: usize,
    pub output_queue_items: usize,
    pub output_queue_capacity: usize,
    pub output_queue_floor: CanonicalU64,
    pub output_binding_active: bool,
    /// Bounded attribution for completed C# update callbacks. Service totals
    /// are nested within the callback duration, not additional frame time.
    pub update_attribution: Option<ProductDevUpdateAttributionSnapshot>,
}

/// One complete C# update callback observation. Durations are integer
/// microseconds so the diagnostics wire remains canonical and float-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevUpdateAttribution {
    pub callback_duration_us: CanonicalU64,
    pub character_step_calls: CanonicalU64,
    pub character_step_duration_us: CanonicalU64,
    /// Controller casts reported by character-step receipts; this is not a
    /// low-level narrow-phase counter.
    pub character_step_cast_count: CanonicalU64,
    /// Projection entries admitted by conservative character-query bounds,
    /// plus active obstacles that have no cached projection bound.
    pub character_step_candidate_count: CanonicalU64,
    /// Actual Parry character cast/contact calls, nested within the logical
    /// controller cast count and candidate count.
    pub character_step_narrow_phase_count: CanonicalU64,
    pub voxel_residency_calls: CanonicalU64,
    pub voxel_residency_duration_us: CanonicalU64,
    pub voxel_scene_presentation_calls: CanonicalU64,
    pub voxel_scene_presentation_duration_us: CanonicalU64,
}

impl Default for ProductDevUpdateAttribution {
    fn default() -> Self {
        Self {
            callback_duration_us: CanonicalU64::new(0),
            character_step_calls: CanonicalU64::new(0),
            character_step_duration_us: CanonicalU64::new(0),
            character_step_cast_count: CanonicalU64::new(0),
            character_step_candidate_count: CanonicalU64::new(0),
            character_step_narrow_phase_count: CanonicalU64::new(0),
            voxel_residency_calls: CanonicalU64::new(0),
            voxel_residency_duration_us: CanonicalU64::new(0),
            voxel_scene_presentation_calls: CanonicalU64::new(0),
            voxel_scene_presentation_duration_us: CanonicalU64::new(0),
        }
    }
}

impl From<RuntimeUpdateAttribution> for ProductDevUpdateAttribution {
    fn from(value: RuntimeUpdateAttribution) -> Self {
        Self {
            callback_duration_us: CanonicalU64::new(value.callback_duration_us),
            character_step_calls: CanonicalU64::new(value.character_step_calls),
            character_step_duration_us: CanonicalU64::new(value.character_step_duration_us),
            character_step_cast_count: CanonicalU64::new(value.character_step_cast_count),
            character_step_candidate_count: CanonicalU64::new(value.character_step_candidate_count),
            character_step_narrow_phase_count: CanonicalU64::new(
                value.character_step_narrow_phase_count,
            ),
            voxel_residency_calls: CanonicalU64::new(value.voxel_residency_calls),
            voxel_residency_duration_us: CanonicalU64::new(value.voxel_residency_duration_us),
            voxel_scene_presentation_calls: CanonicalU64::new(value.voxel_scene_presentation_calls),
            voxel_scene_presentation_duration_us: CanonicalU64::new(
                value.voxel_scene_presentation_duration_us,
            ),
        }
    }
}

/// Host-owned rolling distribution and long-lived slowest complete update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevUpdateAttributionSnapshot {
    pub sample_count: CanonicalU64,
    pub callback_duration_us_p50: CanonicalU64,
    pub callback_duration_us_p95: CanonicalU64,
    pub callback_duration_us_max: CanonicalU64,
    pub latest: ProductDevUpdateAttribution,
    /// Slowest complete callback retained in the current rolling window.
    pub rolling_slowest: ProductDevUpdateAttribution,
    pub rolling_slowest_age_ms: CanonicalU64,
    /// Slowest complete callback observed for this host lifetime.
    pub slowest: ProductDevUpdateAttribution,
    pub slowest_age_ms: CanonicalU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevRendererDiagnosticsFeedbackResult {
    pub accepted: bool,
    pub code: String,
    pub disposition: ProductDevFaultDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<ProductDevRuntimeRecovery>,
    pub runtime: ProductDevRuntimeBinding,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl ProductDevRendererDiagnosticsFeedbackResult {
    pub fn accepted(runtime: ProductDevRuntimeBinding) -> Self {
        Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductDevFaultDisposition::Accepted,
            recovery: None,
            runtime,
            diagnostic: None,
        }
    }

    pub fn rejected(
        runtime: ProductDevRuntimeBinding,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_RENDERER_DIAGNOSTICS_REJECTED".to_owned(),
            disposition: ProductDevFaultDisposition::RejectedRecoverable,
            recovery: None,
            runtime,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }

    pub fn rejected_runtime(
        runtime: ProductDevRuntimeBinding,
        error: ProductDevRuntimeError,
    ) -> Result<Self, ProductDevHostError> {
        let (code, disposition, diagnostic, recovery) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
            recovery: Some(recovery),
            runtime,
            diagnostic: Some(diagnostic),
        })
    }
}

/// Minimal local readout passed through from the generated runtime owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevRuntimeReadout {
    artifact: String,
    runtime: ProductDevRuntimeBinding,
    mode: ProductDevRuntimeMode,
    state: ProductDevRuntimeState,
    admitted_simulation_steps: CanonicalU64,
    admitted_presentations: CanonicalU64,
    dropped_realtime_steps: CanonicalU64,
    clock_regressions: CanonicalU64,
    scaled_remainder: Option<u32>,
    last_observed_time_ns: Option<CanonicalU64>,
    fault: Option<ProductDevRuntimeFault>,
}

impl ProductDevRuntimeReadout {
    pub fn new(
        runtime: ProductDevRuntimeBinding,
        mode: ProductDevRuntimeMode,
        state: ProductDevRuntimeState,
    ) -> Self {
        Self {
            artifact: "rusty.product.runtime-readout".to_owned(),
            runtime,
            mode,
            state,
            admitted_simulation_steps: CanonicalU64::new(0),
            admitted_presentations: CanonicalU64::new(0),
            dropped_realtime_steps: CanonicalU64::new(0),
            clock_regressions: CanonicalU64::new(0),
            scaled_remainder: None,
            last_observed_time_ns: None,
            fault: None,
        }
    }

    pub fn with_counters(
        mut self,
        admitted_simulation_steps: u64,
        admitted_presentations: u64,
        dropped_realtime_steps: u64,
        clock_regressions: u64,
    ) -> Self {
        self.admitted_simulation_steps = CanonicalU64::new(admitted_simulation_steps);
        self.admitted_presentations = CanonicalU64::new(admitted_presentations);
        self.dropped_realtime_steps = CanonicalU64::new(dropped_realtime_steps);
        self.clock_regressions = CanonicalU64::new(clock_regressions);
        self
    }

    pub fn with_clock(
        mut self,
        scaled_remainder: Option<u32>,
        last_observed_time_ns: Option<u64>,
    ) -> Self {
        self.scaled_remainder = scaled_remainder;
        self.last_observed_time_ns = last_observed_time_ns.map(CanonicalU64::new);
        self
    }

    pub fn with_fault(mut self, fault: ProductDevRuntimeFault) -> Self {
        self.fault = Some(fault);
        self
    }

    pub const fn runtime(&self) -> ProductDevRuntimeBinding {
        self.runtime
    }

    /// Lifecycle mode selected by the standard runtime configuration.
    pub const fn mode(&self) -> ProductDevRuntimeMode {
        self.mode
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevRuntimeMode {
    Realtime,
    Demand,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevRuntimeState {
    Created,
    Running,
    Paused,
    Faulted,
    Shutdown,
}

/// Host scheduling posture reported by one runtime owner. The host uses this
/// small state seam to decide when its monotonic realtime loop may run; it
/// does not infer product lifecycle from browser cadence or duplicate the
/// lifecycle state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductDevRuntimeScheduleState {
    /// This runtime is demand/external driven or otherwise does not opt into
    /// the standard Rust-host realtime scheduler.
    Unsupported,
    Created,
    Running,
    Paused,
    Faulted,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevRuntimeFault {
    OwnerReported,
    CounterExhausted,
}

/// Direct operation result supplied by the generated runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevOperationResult {
    accepted: bool,
    code: String,
    disposition: ProductDevFaultDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery: Option<ProductDevRuntimeRecovery>,
    operation: ProductDevOperationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    binding: Option<ProductDevRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_input_sequence: Option<CanonicalU64>,
    /// Last lifecycle simulation step admitted before this operation result.
    /// It is present on a resync receipt when admission has already advanced
    /// but the downstream callback/update could not be completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    admitted_through: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readout: Option<ProductDevRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<String>,
}

/// One bounded result returned by a product-owned generated debug catalog.
/// A failed command is a completed product operation, not a host/ABI failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevDebugResult {
    succeeded: bool,
    message: String,
}

/// Read-only product-generated descriptor data for live-debug completion and
/// help. It is never a dispatch schema: command invocation remains the single
/// explicit `execute_debug` operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevDebugCatalog {
    available: bool,
    commands: Vec<ProductDevDebugCommandDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevDebugCommandDescriptor {
    name: String,
    description: String,
    parameters: Vec<ProductDevDebugCommandParameterDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevDebugCommandParameterDescriptor {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
}

impl ProductDevDebugCatalog {
    pub const MAX_COMMANDS: usize = 256;
    pub const MAX_PARAMETERS_PER_COMMAND: usize = 16;
    const MAX_TEXT_BYTES: usize = 1_024;

    pub const fn unavailable() -> Self {
        Self {
            available: false,
            commands: Vec::new(),
        }
    }

    pub fn with_renderer_diagnostics(mut self) -> Self {
        self.available = true;
        for (name, description) in [
            (
                "engine.renderer",
                "Show the latest compact browser renderer timing, pacing, canvas, and resource summary",
            ),
            (
                "engine.renderer.detail",
                "Show bounded renderer admission, callback phase, cadence, pacing, and texture details",
            ),
            (
                "engine.renderer.show",
                "Show every mounted Engine renderer metrics widget",
            ),
            (
                "engine.renderer.hide",
                "Hide every mounted Engine renderer metrics widget",
            ),
            (
                "engine.renderer.toggle",
                "Toggle every mounted Engine renderer metrics widget",
            ),
            (
                "engine.renderer.status",
                "Show renderer metrics widget visibility and the latest compact summary",
            ),
        ] {
            if self.commands.iter().any(|command| command.name == name) {
                continue;
            }
            self.commands.push(ProductDevDebugCommandDescriptor {
                name: name.to_owned(),
                description: description.to_owned(),
                parameters: Vec::new(),
            });
        }
        self
    }

    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductDevHostError> {
        let catalog: Self = serde_json::from_slice(bytes).map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_DEBUG_CATALOG_DECODE",
                "generated debug catalog descriptor payload is invalid",
            )
        })?;
        catalog.validate()?;
        Ok(catalog)
    }

    fn validate(&self) -> Result<(), ProductDevHostError> {
        if !self.available || self.commands.len() > Self::MAX_COMMANDS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_DEBUG_CATALOG_BOUNDS",
                "generated debug catalog availability or command count is invalid",
            ));
        }
        for command in &self.commands {
            validate_debug_descriptor_text(&command.name)?;
            validate_debug_descriptor_text(&command.description)?;
            if command.parameters.len() > Self::MAX_PARAMETERS_PER_COMMAND {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_DEBUG_CATALOG_BOUNDS",
                    "generated debug catalog command has too many parameters",
                ));
            }
            for parameter in &command.parameters {
                validate_debug_descriptor_text(&parameter.name)?;
                validate_debug_descriptor_text(&parameter.type_name)?;
            }
        }
        Ok(())
    }
}

fn validate_debug_descriptor_text(value: &str) -> Result<(), ProductDevHostError> {
    if value.len() > ProductDevDebugCatalog::MAX_TEXT_BYTES || value.contains('\0') {
        return Err(ProductDevHostError::new(
            "DEV_HOST_DEBUG_CATALOG_BOUNDS",
            "generated debug catalog descriptor text is invalid",
        ));
    }
    Ok(())
}

impl ProductDevDebugResult {
    pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;

    pub fn new(succeeded: bool, message: String) -> Result<Self, ProductDevHostError> {
        if message.len() > Self::MAX_MESSAGE_BYTES {
            return Err(ProductDevHostError::new(
                "DEV_HOST_DEBUG_RESULT_BOUNDS",
                "debug result exceeds the host result bound",
            ));
        }
        Ok(Self { succeeded, message })
    }

    pub const fn succeeded(&self) -> bool {
        self.succeeded
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl ProductDevOperationResult {
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }

    pub const fn disposition(&self) -> ProductDevFaultDisposition {
        self.disposition
    }

    pub const fn admitted_through(&self) -> Option<CanonicalU64> {
        self.admitted_through
    }

    pub const fn binding(&self) -> Option<ProductDevRuntimeBinding> {
        self.binding
    }

    pub const fn next_input_sequence(&self) -> Option<CanonicalU64> {
        self.next_input_sequence
    }

    pub fn readout(&self) -> Option<&ProductDevRuntimeReadout> {
        self.readout.as_ref()
    }

    pub fn accepted(
        operation: ProductDevOperationKind,
        binding: ProductDevRuntimeBinding,
        next_input_sequence: CanonicalU64,
        readout: ProductDevRuntimeReadout,
    ) -> Result<Self, ProductDevHostError> {
        if readout.runtime() != binding {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RESULT_BINDING",
                "runtime operation receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductDevFaultDisposition::Accepted,
            recovery: None,
            operation,
            binding: Some(binding),
            next_input_sequence: Some(next_input_sequence),
            admitted_through: None,
            readout: Some(readout),
            diagnostic: None,
        })
    }

    pub fn rejected(
        operation: ProductDevOperationKind,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        let diagnostic = bounded_diagnostic(diagnostic.into())?;
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_OPERATION_REJECTED".to_owned(),
            disposition: ProductDevFaultDisposition::RejectedRecoverable,
            recovery: None,
            operation,
            binding: None,
            next_input_sequence: None,
            admitted_through: None,
            readout: None,
            diagnostic: Some(diagnostic),
        })
    }

    pub fn rejected_runtime(
        operation: ProductDevOperationKind,
        error: ProductDevRuntimeError,
    ) -> Result<Self, ProductDevHostError> {
        let (code, disposition, diagnostic, recovery) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
            recovery: Some(recovery),
            operation,
            binding: None,
            next_input_sequence: None,
            admitted_through: None,
            readout: None,
            diagnostic: Some(diagnostic),
        })
    }

    /// Reports a lifecycle admission which already advanced Rust-owned
    /// counters but could not safely claim completion of the downstream
    /// callback. The current binding/readout and admitted frontier let the
    /// host resynchronize without replaying the operation.
    #[allow(
        clippy::too_many_arguments,
        reason = "the direct runtime receipt preserves each typed recovery fact at the host boundary"
    )]
    pub fn resync_required(
        operation: ProductDevOperationKind,
        binding: ProductDevRuntimeBinding,
        next_input_sequence: CanonicalU64,
        readout: ProductDevRuntimeReadout,
        admitted_through: Option<CanonicalU64>,
        code: impl Into<String>,
        diagnostic: impl Into<String>,
        recovery: ProductDevRuntimeRecovery,
    ) -> Result<Self, ProductDevHostError> {
        if readout.runtime() != binding {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RESULT_BINDING",
                "runtime resync receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: false,
            code: code.into(),
            disposition: ProductDevFaultDisposition::ResyncRequired,
            recovery: Some(recovery),
            operation,
            binding: Some(binding),
            next_input_sequence: Some(next_input_sequence),
            admitted_through,
            readout: Some(readout),
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }
}

/// Typed input batch passed from the dev host to the runtime input owner.
#[derive(Debug, Clone)]
pub struct ProductDevInputBatch {
    events: Vec<RuntimeInputEvent>,
    wire_json: Option<Vec<u8>>,
}

impl ProductDevInputBatch {
    pub fn new(events: Vec<RuntimeInputEvent>) -> Self {
        Self {
            events,
            wire_json: None,
        }
    }

    pub fn events(&self) -> &[RuntimeInputEvent] {
        &self.events
    }

    /// Preserves the one already-admitted host wire encoding when a
    /// disposable worker must receive the same input batch.  Programmatic
    /// callers retain the typed in-process route and need not manufacture a
    /// second wire form.
    pub fn encoded_json(&self) -> Option<&[u8]> {
        self.wire_json.as_deref()
    }

    /// Strictly decodes the ordered runtime-input wire array used by host
    /// adapters. The runtime-input crate owns event semantics and its exact
    /// event/count limits; this method only maps its bounded decoder error to
    /// the product development host error surface.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductDevHostError> {
        if bytes.len() > crate::MAX_REQUEST_BODY_BYTES {
            return Err(ProductDevHostError::new(
                "DEV_HOST_BODY_BOUNDS",
                "input batch exceeds the host JSON body bound",
            ));
        }
        let events = runtime_input::decode_runtime_input_wire_events_json(bytes).map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_INPUT_DECODE",
                "input batch is not a strict runtime-input wire batch",
            )
        })?;
        Ok(Self {
            events,
            wire_json: Some(bytes.to_vec()),
        })
    }
}

/// Typed input result supplied by the generated runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevInputResult {
    accepted: bool,
    code: String,
    disposition: ProductDevFaultDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery: Option<ProductDevRuntimeRecovery>,
    /// Number of submitted events in this batch. Kept as `count` for
    /// compatibility with existing host adapters. A strict-decode rejection
    /// preserves the host-bounded submitted count even when it is above the
    /// smaller admitted-wire-event limit.
    count: usize,
    /// Number of events admitted into the input lane. A safe stale/duplicate
    /// drop makes this less than `count` while retaining the current cursor.
    accepted_count: usize,
    dropped_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    accepted_through: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    consumed_through: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_input_sequence: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    binding: Option<ProductDevRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readout: Option<ProductDevRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<String>,
}

impl ProductDevInputResult {
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }

    pub fn accepted(
        count: usize,
        binding: ProductDevRuntimeBinding,
        readout: ProductDevRuntimeReadout,
    ) -> Result<Self, ProductDevHostError> {
        if count > runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_INPUT_RESULT_BOUNDS",
                "runtime input receipt count exceeds admitted batch bound",
            ));
        }
        if readout.runtime() != binding {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RESULT_BINDING",
                "runtime input receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductDevFaultDisposition::Accepted,
            recovery: None,
            count,
            accepted_count: count,
            dropped_count: 0,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: Some(binding),
            readout: Some(readout),
            diagnostic: None,
        })
    }

    /// Acknowledges that the host mailbox accepted the submitted batch. This
    /// deliberately does not claim that RuntimeInputLane or C# has consumed
    /// it; the later scheduled update remains the semantic admission point.
    pub fn queued(count: usize) -> Result<Self, ProductDevHostError> {
        if count > runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_INPUT_RESULT_BOUNDS",
                "queued runtime input count exceeds admitted batch bound",
            ));
        }
        Ok(Self {
            accepted: true,
            code: "DEV_HOST_INPUT_QUEUED".to_owned(),
            disposition: ProductDevFaultDisposition::Accepted,
            recovery: None,
            count,
            accepted_count: count,
            dropped_count: 0,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: None,
            readout: None,
            diagnostic: None,
        })
    }

    /// Reports a bounded mailbox refusal without pretending that the runtime
    /// consumed the batch. Overflow clears the queued prefix and fences the
    /// runtime on the next host observation, so retrying against the old
    /// binding would be incoherent; the transport must request a fresh
    /// baseline instead.
    pub fn mailbox_full(count: usize) -> Result<Self, ProductDevHostError> {
        if count > runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_INPUT_RESULT_BOUNDS",
                "mailbox refusal count exceeds admitted batch bound",
            ));
        }
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_INPUT_MAILBOX_FULL".to_owned(),
            disposition: ProductDevFaultDisposition::ResyncRequired,
            recovery: None,
            count,
            accepted_count: 0,
            dropped_count: count,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: None,
            readout: None,
            diagnostic: Some(bounded_diagnostic(
                "Rust-host input mailbox overflow cleared queued input; obtain a fresh runtime baseline before retrying".to_owned(),
            )?),
        })
    }

    /// Reports that the browser submitted a structurally invalid wire batch
    /// and the runtime replaced its input control fence before continuing.
    /// The rejected batch is deliberately not replayable: the replacement
    /// binding and its sequence-zero clear arrive through the accompanying
    /// runtime output receipt. `count` is diagnostic-only here: the host
    /// already bounded the JSON request, while the admitted-wire-event bound
    /// does not apply to a batch rejected before admission.
    pub fn wire_decode_resynchronized(count: usize) -> Result<Self, ProductDevHostError> {
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_INPUT_DECODE".to_owned(),
            disposition: ProductDevFaultDisposition::ResyncRequired,
            recovery: None,
            count,
            accepted_count: 0,
            dropped_count: count,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: None,
            readout: None,
            diagnostic: Some(bounded_diagnostic(
                "input batch was not a strict runtime-input wire batch; the runtime input binding was resynchronized".to_owned(),
            )?),
        })
    }

    pub fn rejected(diagnostic: impl Into<String>) -> Result<Self, ProductDevHostError> {
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_INPUT_REJECTED".to_owned(),
            disposition: ProductDevFaultDisposition::RejectedRecoverable,
            recovery: None,
            count: 0,
            accepted_count: 0,
            dropped_count: 0,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: None,
            readout: None,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }

    pub fn rejected_runtime(error: ProductDevRuntimeError) -> Result<Self, ProductDevHostError> {
        let (code, disposition, diagnostic, recovery) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
            recovery: Some(recovery),
            count: 0,
            accepted_count: 0,
            dropped_count: 0,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: None,
            readout: None,
            diagnostic: Some(diagnostic),
        })
    }

    /// Constructs the input receipt for a completed or safely degraded batch.
    /// `accepted` is true only when every submitted event was admitted. A
    /// stale/duplicate drop is recoverable, names the current cursor, and is
    /// never an invitation to replay the original batch.
    #[allow(
        clippy::too_many_arguments,
        reason = "the direct runtime receipt preserves the input admission and cursor facts separately"
    )]
    pub fn with_progress(
        count: usize,
        accepted_count: usize,
        dropped_count: usize,
        accepted_through: Option<CanonicalU64>,
        consumed_through: Option<CanonicalU64>,
        next_input_sequence: CanonicalU64,
        binding: ProductDevRuntimeBinding,
        readout: ProductDevRuntimeReadout,
    ) -> Result<Self, ProductDevHostError> {
        if count > runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS
            || accepted_count > count
            || dropped_count > count
            || accepted_count
                .checked_add(dropped_count)
                .is_none_or(|total| total != count)
            || (accepted_count == 0 && accepted_through.is_some())
            || (accepted_count > 0 && accepted_through.is_none())
            || (count == 0 && consumed_through.is_some())
        {
            return Err(ProductDevHostError::new(
                "DEV_HOST_INPUT_RESULT_BOUNDS",
                "runtime input progress receipt has incoherent batch boundaries",
            ));
        }
        if readout.runtime() != binding {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RESULT_BINDING",
                "runtime input receipt binding does not match its readout",
            ));
        }
        let complete = dropped_count == 0;
        Ok(Self {
            accepted: complete,
            code: if complete {
                ACCEPTED_FAULT_CODE.to_owned()
            } else {
                "CSHARP_INPUT_STALE_DROPPED".to_owned()
            },
            disposition: if complete {
                ProductDevFaultDisposition::Accepted
            } else {
                ProductDevFaultDisposition::RejectedRecoverable
            },
            recovery: None,
            count,
            accepted_count,
            dropped_count,
            accepted_through,
            consumed_through,
            next_input_sequence: Some(next_input_sequence),
            binding: Some(binding),
            readout: Some(readout),
            diagnostic: if complete {
                None
            } else {
                Some(bounded_diagnostic(format!(
                    "dropped {dropped_count} stale or duplicate input event(s); synchronize the input cursor and do not replay them"
                ))?)
            },
        })
    }
}

/// One exact completion forwarded to the runtime timeline owner.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductDevTimelineCompletion {
    envelope: TimelineCompletionEnvelope,
    wire_json: Option<Vec<u8>>,
}

impl ProductDevTimelineCompletion {
    /// Strictly decodes the timeline completion wire object accepted by the
    /// host. The private wire DTO remains private; a packaged adapter receives
    /// only this validated, transport-neutral completion value.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductDevHostError> {
        let value: ProductDevTimelineCompletionWire = decode_strict_json(
            bytes,
            "DEV_HOST_TIMELINE_DECODE",
            "timeline completion JSON is malformed or has unknown fields",
        )?;
        let mut completion = Self::from_wire(value)?;
        completion.wire_json = Some(bytes.to_vec());
        Ok(completion)
    }

    pub(crate) fn from_wire(
        value: ProductDevTimelineCompletionWire,
    ) -> Result<Self, ProductDevHostError> {
        if value.provenance.correlation != value.correlation {
            return Err(ProductDevHostError::new(
                "DEV_HOST_TIMELINE_CORRELATION",
                "timeline provenance correlation must match completion correlation",
            ));
        }
        let outcome_data = match value.outcome {
            ProductDevTimelineOutcomeWire::Success { data } => TimelineCompletionOutcome::Success(
                data.map(RuntimeOpaqueData::new).transpose().map_err(|_| {
                    ProductDevHostError::new(
                        "DEV_HOST_TIMELINE_DATA",
                        "timeline outcome data violates runtime-timeline bounds",
                    )
                })?,
            ),
            ProductDevTimelineOutcomeWire::Failure { data } => TimelineCompletionOutcome::Failure(
                data.map(RuntimeOpaqueData::new).transpose().map_err(|_| {
                    ProductDevHostError::new(
                        "DEV_HOST_TIMELINE_DATA",
                        "timeline outcome data violates runtime-timeline bounds",
                    )
                })?,
            ),
        };
        let provenance = RuntimeProvenance::new(
            value.provenance.correlation,
            value
                .provenance
                .detail
                .map(RuntimeOpaqueData::new)
                .transpose()
                .map_err(|_| {
                    ProductDevHostError::new(
                        "DEV_HOST_TIMELINE_DATA",
                        "timeline provenance data violates runtime-timeline bounds",
                    )
                })?,
        )
        .map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_TIMELINE_PROVENANCE",
                "timeline provenance violates runtime-timeline bounds",
            )
        })?;
        let binding = RuntimeTimelineBinding::new(
            RuntimeInstanceId::new(value.runtime.instance_id.get()),
            RuntimeGeneration::new(value.runtime.generation.get()),
            RuntimeControlRevision::new(value.runtime.control_revision.get()),
        );
        let envelope = TimelineCompletionEnvelope::new(
            TimelineCompletionTicketId::new(value.ticket.get()),
            binding,
            value.correlation,
            outcome_data,
            provenance,
        )
        .map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_TIMELINE_COMPLETION",
                "timeline completion violates runtime-timeline bounds",
            )
        })?;
        Ok(Self {
            envelope,
            wire_json: None,
        })
    }

    pub fn envelope(&self) -> &TimelineCompletionEnvelope {
        &self.envelope
    }

    pub fn into_envelope(self) -> TimelineCompletionEnvelope {
        self.envelope
    }

    pub fn encoded_json(&self) -> Option<&[u8]> {
        self.wire_json.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductDevTimelineCompletionWire {
    pub ticket: CanonicalU64,
    pub runtime: ProductDevRuntimeBinding,
    pub correlation: String,
    pub outcome: ProductDevTimelineOutcomeWire,
    pub provenance: ProductDevTimelineProvenanceWire,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum ProductDevTimelineOutcomeWire {
    Success {
        #[serde(default)]
        data: Option<Value>,
    },
    Failure {
        #[serde(default)]
        data: Option<Value>,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductDevTimelineProvenanceWire {
    pub correlation: String,
    #[serde(default)]
    pub detail: Option<Value>,
}

/// Completion result supplied by the generated runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevTimelineCompletionResult {
    accepted: bool,
    code: String,
    disposition: ProductDevFaultDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery: Option<ProductDevRuntimeRecovery>,
    ticket: CanonicalU64,
    #[serde(skip_serializing_if = "Option::is_none")]
    binding: Option<ProductDevRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readout: Option<ProductDevRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<String>,
}

impl ProductDevTimelineCompletionResult {
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }

    pub const fn ticket(&self) -> CanonicalU64 {
        self.ticket
    }

    pub fn accepted(
        ticket: CanonicalU64,
        binding: ProductDevRuntimeBinding,
        readout: ProductDevRuntimeReadout,
    ) -> Result<Self, ProductDevHostError> {
        if readout.runtime() != binding {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RESULT_BINDING",
                "timeline completion receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductDevFaultDisposition::Accepted,
            recovery: None,
            ticket,
            binding: Some(binding),
            readout: Some(readout),
            diagnostic: None,
        })
    }
    pub fn rejected(
        ticket: CanonicalU64,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        Ok(Self {
            accepted: false,
            code: "DEV_HOST_TIMELINE_REJECTED".to_owned(),
            disposition: ProductDevFaultDisposition::RejectedRecoverable,
            recovery: None,
            ticket,
            binding: None,
            readout: None,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }

    /// A completion rejected before product callback entry still names the
    /// current runtime so a browser can retain its exact binding rather than
    /// treating the result as a stale-output failure.
    pub fn rejected_with_current(
        ticket: CanonicalU64,
        binding: ProductDevRuntimeBinding,
        readout: ProductDevRuntimeReadout,
        code: impl Into<String>,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        if readout.runtime() != binding {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RESULT_BINDING",
                "timeline rejection binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: false,
            code: code.into(),
            disposition: ProductDevFaultDisposition::RejectedRecoverable,
            recovery: None,
            ticket,
            binding: Some(binding),
            readout: Some(readout),
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }

    /// Reports a timeline callback/receipt failure after the callback was
    /// entered. The current binding/readout are evidence for resync only; no
    /// rollback or retry of the product-owned callback is implied.
    pub fn resync_required_with_current(
        ticket: CanonicalU64,
        binding: ProductDevRuntimeBinding,
        readout: ProductDevRuntimeReadout,
        code: impl Into<String>,
        diagnostic: impl Into<String>,
        recovery: ProductDevRuntimeRecovery,
    ) -> Result<Self, ProductDevHostError> {
        if readout.runtime() != binding {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RESULT_BINDING",
                "timeline resync receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: false,
            code: code.into(),
            disposition: ProductDevFaultDisposition::ResyncRequired,
            recovery: Some(recovery),
            ticket,
            binding: Some(binding),
            readout: Some(readout),
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }

    pub fn rejected_runtime(
        ticket: CanonicalU64,
        error: ProductDevRuntimeError,
    ) -> Result<Self, ProductDevHostError> {
        let (code, disposition, diagnostic, recovery) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
            recovery: Some(recovery),
            ticket,
            binding: None,
            readout: None,
            diagnostic: Some(diagnostic),
        })
    }
}

/// One Rust-authoritative output pushed to the local browser projection.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductDevRuntimeOutput {
    wire: ProductDevRuntimeOutputWire,
}

/// One retained renderer stream frontier captured at a complete baseline
/// boundary. Revisions intentionally remain JSON numbers: renderer frame
/// publication uses JavaScript-safe numeric revisions rather than the host's
/// canonical-string control counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevRendererPublicationFrontier {
    pub stream: String,
    pub revision: u64,
}

impl ProductDevRendererPublicationFrontier {
    pub fn new(stream: String, revision: u64) -> Result<Self, ProductDevHostError> {
        if stream.trim().is_empty() || stream.len() > 256 {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RENDERER_FRONTIER",
                "renderer publication frontier stream must contain 1..=256 characters",
            ));
        }
        if revision > JSON_SAFE_U64_MAX {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RENDERER_FRONTIER",
                "renderer publication frontier revision is outside the JSON-safe range",
            ));
        }
        Ok(Self { stream, revision })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum ProductDevRuntimeOutputWire {
    Binding {
        runtime: ProductDevRuntimeBinding,
        #[serde(rename = "nextInputSequence")]
        next_input_sequence: CanonicalU64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "publicationFrontiers"
        )]
        publication_frontiers: Option<Vec<ProductDevRendererPublicationFrontier>>,
    },
    CompleteBaseline {
        runtime: ProductDevRuntimeBinding,
        #[serde(rename = "publicationFrontiers")]
        publication_frontiers: Vec<ProductDevRendererPublicationFrontier>,
    },
    Frame {
        frame: Value,
    },
    ViewComposition {
        composition: Value,
    },
    Presentation {
        frame: Value,
    },
    AnimationCueDefinitions {
        definitions: Vec<ProductDevAnimationCueDefinition>,
    },
    UiProjection {
        envelope: Value,
    },
    RuntimeReadout {
        readout: ProductDevRuntimeReadout,
    },
    /// One input batch admitted by the runtime at a scheduled update
    /// boundary. The result carries the authoritative input cursor and
    /// recovery disposition that cannot be returned by the earlier queued
    /// HTTP acknowledgement.
    RuntimeInputResult {
        result: ProductDevInputResult,
    },
    /// One host-owned realtime observation was admitted. This is a progress
    /// pulse, not a simulation step count; the accompanying readout carries
    /// authoritative counters when the runtime supplies one.
    RuntimeProgress {
        owner: String,
    },
}

/// Closed renderer realization families for a sampled animation marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProductDevAnimationCueSignalDomain {
    Audio,
    Particle,
}

/// A copied animation cue declaration sent through the fixed renderer output
/// stream. `at_seconds` is derived from an Engine-admitted millisecond marker,
/// so it cannot borrow product memory or become non-finite in transit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevAnimationCueDefinition {
    pub cue_id: String,
    pub asset: String,
    pub clip: String,
    pub at_seconds: f64,
    pub signal_domain: ProductDevAnimationCueSignalDomain,
    pub signal_id: String,
}

impl ProductDevAnimationCueDefinition {
    pub const MAX_DEFINITIONS: usize = 128;
    pub const MAX_TEXT_BYTES: usize = 96;

    pub fn new(
        cue_id: String,
        asset: String,
        clip: String,
        marker_millis: u64,
        signal_domain: ProductDevAnimationCueSignalDomain,
        signal_id: String,
    ) -> Result<Self, ProductDevHostError> {
        for (field, value) in [
            ("animation cue id", &cue_id),
            ("animation cue asset", &asset),
            ("animation cue clip", &clip),
            ("animation cue signal id", &signal_id),
        ] {
            if value.is_empty() || value.len() > Self::MAX_TEXT_BYTES {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_ANIMATION_CUE",
                    format!("{field} must be non-empty and no more than 96 UTF-8 bytes"),
                ));
            }
        }
        let at_seconds = marker_millis as f64 / 1_000.0;
        debug_assert!(at_seconds.is_finite() && at_seconds >= 0.0);
        Ok(Self {
            cue_id,
            asset,
            clip,
            at_seconds,
            signal_domain,
            signal_id,
        })
    }
}

impl ProductDevRuntimeOutput {
    /// Decodes one worker-retained output through the same bounded JSON
    /// representation used by the browser projection.  The local worker is
    /// trusted, but a shell still refuses malformed or oversized output
    /// rather than letting it poison its retained SSE history.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductDevHostError> {
        if bytes.len() > MAX_BASELINE_AGGREGATE_BYTES {
            return Err(ProductDevHostError::new(
                "DEV_HOST_WORKER_OUTPUT_DECODE",
                "worker output exceeds the maximum baseline output bound",
            ));
        }
        serde_json::from_slice(bytes).map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_WORKER_OUTPUT_DECODE",
                "worker output is not a valid runtime output",
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn test_frame_value(frame: Value) -> Self {
        Self {
            wire: ProductDevRuntimeOutputWire::Frame { frame },
        }
    }

    pub fn binding(runtime: ProductDevRuntimeBinding, next_input_sequence: CanonicalU64) -> Self {
        Self {
            wire: ProductDevRuntimeOutputWire::Binding {
                runtime,
                next_input_sequence,
                publication_frontiers: None,
            },
        }
    }
    pub fn frame(frame: &render_model::RenderFrameDiff) -> Result<Self, ProductDevHostError> {
        let frame = encode_validated_wire(frame.encode_json().map_err(|_| {
            ProductDevHostError::new("DEV_HOST_RENDER_FRAME", "render frame is invalid")
        })?)?;
        Ok(Self {
            wire: ProductDevRuntimeOutputWire::Frame { frame },
        })
    }
    /// Carries one Engine-owned camera/view composition to the existing
    /// renderer host. Products publish typed facts; browser realization and
    /// resize observation remain behind this fixed Engine output lane.
    pub fn view_composition(
        composition: &render_host_contracts::RendererViewComposition,
    ) -> Result<Self, ProductDevHostError> {
        composition.validate().map_err(|_| {
            ProductDevHostError::new("DEV_HOST_VIEW_COMPOSITION", "view composition is invalid")
        })?;
        let composition = serde_json::to_value(composition).map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_VIEW_COMPOSITION",
                "view composition could not be encoded",
            )
        })?;
        Ok(Self {
            wire: ProductDevRuntimeOutputWire::ViewComposition { composition },
        })
    }
    /// Preserve only newly emitted, non-retained presentation events when a
    /// lifecycle operation publishes its complete committed snapshot.
    pub fn transient_presentation(&self) -> Result<Option<Self>, ProductDevHostError> {
        let ProductDevRuntimeOutputWire::Presentation { frame } = &self.wire else {
            return Ok(None);
        };
        let frame: render_presentation::PresentationFrameDiff =
            serde_json::from_value(frame.clone()).map_err(|_| {
                ProductDevHostError::new(
                    "DEV_HOST_PRESENTATION_FRAME",
                    "presentation frame is invalid",
                )
            })?;
        let events = frame.transient_events();
        if events.is_empty() {
            Ok(None)
        } else {
            Self::presentation(&events).map(Some)
        }
    }

    pub fn presentation(
        frame: &render_presentation::PresentationFrameDiff,
    ) -> Result<Self, ProductDevHostError> {
        let frame = encode_validated_wire(frame.encode_json().map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_PRESENTATION_FRAME",
                "presentation frame is invalid",
            )
        })?)?;
        Ok(Self {
            wire: ProductDevRuntimeOutputWire::Presentation { frame },
        })
    }
    /// Replaces all active animation cue definitions in the generic renderer
    /// host. This is a fixed typed output, not a product event stream.
    pub fn animation_cue_definitions(
        definitions: Vec<ProductDevAnimationCueDefinition>,
    ) -> Result<Self, ProductDevHostError> {
        if definitions.len() > ProductDevAnimationCueDefinition::MAX_DEFINITIONS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_ANIMATION_CUE",
                "animation cue definition replacement exceeds the 128 definition bound",
            ));
        }
        if definitions.iter().any(|definition| {
            definition.cue_id.is_empty()
                || definition.asset.is_empty()
                || definition.clip.is_empty()
                || definition.signal_id.is_empty()
                || definition.cue_id.len() > ProductDevAnimationCueDefinition::MAX_TEXT_BYTES
                || definition.asset.len() > ProductDevAnimationCueDefinition::MAX_TEXT_BYTES
                || definition.clip.len() > ProductDevAnimationCueDefinition::MAX_TEXT_BYTES
                || definition.signal_id.len() > ProductDevAnimationCueDefinition::MAX_TEXT_BYTES
                || !definition.at_seconds.is_finite()
                || definition.at_seconds < 0.0
        }) {
            return Err(ProductDevHostError::new(
                "DEV_HOST_ANIMATION_CUE",
                "animation cue definitions must use bounded non-empty text and finite non-negative markers",
            ));
        }
        let mut keys = std::collections::BTreeSet::new();
        if definitions.iter().any(|definition| {
            !keys.insert((
                definition.asset.as_str(),
                definition.clip.as_str(),
                definition.cue_id.as_str(),
            ))
        }) {
            return Err(ProductDevHostError::new(
                "DEV_HOST_ANIMATION_CUE",
                "animation cue definitions must not duplicate an asset, clip, and cue id",
            ));
        }
        Ok(Self {
            wire: ProductDevRuntimeOutputWire::AnimationCueDefinitions { definitions },
        })
    }
    pub fn ui_projection(
        envelope: &runtime_ui::RuntimeUiProjectionEnvelope,
    ) -> Result<Self, ProductDevHostError> {
        let bytes = envelope.encode_json().map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_UI_PROJECTION",
                "UI projection envelope is invalid",
            )
        })?;
        let envelope = serde_json::from_slice(&bytes).map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_UI_PROJECTION",
                "UI projection envelope could not be decoded",
            )
        })?;
        Ok(Self {
            wire: ProductDevRuntimeOutputWire::UiProjection { envelope },
        })
    }
    pub fn runtime_readout(readout: ProductDevRuntimeReadout) -> Self {
        Self {
            wire: ProductDevRuntimeOutputWire::RuntimeReadout { readout },
        }
    }

    /// Publishes one scheduled input admission result through the ordered
    /// runtime output stream. The HTTP input route only acknowledges mailbox
    /// admission; this is the later runtime-owned receipt.
    pub fn runtime_input_result(result: ProductDevInputResult) -> Self {
        Self {
            wire: ProductDevRuntimeOutputWire::RuntimeInputResult { result },
        }
    }

    /// Marks one realtime observation admitted by the Rust host scheduler.
    /// Browser hosts use this to update progress without becoming the clock.
    pub fn runtime_progress() -> Self {
        Self {
            wire: ProductDevRuntimeOutputWire::RuntimeProgress {
                owner: "rust-host".to_owned(),
            },
        }
    }
    /// Marks the end of one complete current-binding projection. The host
    /// buffers its preceding binding-tagged facts and exposes them together;
    /// later facts for that binding are incremental.
    pub fn complete_baseline(runtime: ProductDevRuntimeBinding) -> Self {
        Self::complete_baseline_with_frontiers(runtime, Vec::new())
    }

    pub fn complete_baseline_with_frontiers(
        runtime: ProductDevRuntimeBinding,
        publication_frontiers: Vec<ProductDevRendererPublicationFrontier>,
    ) -> Self {
        Self {
            wire: ProductDevRuntimeOutputWire::CompleteBaseline {
                runtime,
                publication_frontiers,
            },
        }
    }

    pub(crate) const fn binding_marker(&self) -> Option<ProductDevRuntimeBinding> {
        match &self.wire {
            ProductDevRuntimeOutputWire::Binding { runtime, .. } => Some(*runtime),
            _ => None,
        }
    }

    pub(crate) const fn complete_baseline_marker(&self) -> Option<ProductDevRuntimeBinding> {
        match &self.wire {
            ProductDevRuntimeOutputWire::CompleteBaseline { runtime, .. } => Some(*runtime),
            _ => None,
        }
    }

    /// Validates one published output group. Each complete
    /// binding-to-completion subsequence receives the larger retained
    /// baseline budget, while every ordinary contiguous subsequence keeps the
    /// normal output bound. This permits a recovery baseline and following
    /// ordinary tick facts to share one worker publication without widening
    /// the incremental lane.
    ///
    /// Returns the binding only when the entire group is one baseline, which
    /// remains useful to callers that need to recognize that simpler shape.
    pub fn validate_output_group(
        outputs: &[Self],
    ) -> Result<Option<ProductDevRuntimeBinding>, ProductDevHostError> {
        if outputs.len() > MAX_OUTPUT_QUEUE_ITEMS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_OUTPUT_BATCH_BOUNDS",
                "runtime receipt contains too many output events",
            ));
        }
        let mut index = 0;
        let mut entire_group_baseline = None;
        while index < outputs.len() {
            let complete = outputs[index]
                .binding_marker()
                .map(|binding| complete_baseline_end(outputs, index, binding))
                .transpose()?;
            let Some(complete) = complete.flatten() else {
                let mut aggregate = 0_usize;
                while index < outputs.len() {
                    if let Some(binding) = outputs[index].binding_marker() {
                        if complete_baseline_end(outputs, index, binding)?.is_some() {
                            break;
                        }
                    }
                    aggregate = aggregate.saturating_add(encoded_output_bytes(&outputs[index])?);
                    if aggregate > MAX_OUTPUT_AGGREGATE_BYTES {
                        return Err(ProductDevHostError::new(
                            "DEV_HOST_OUTPUT_BOUNDS",
                            "runtime receipt outputs exceed the maximum aggregate byte length",
                        ));
                    }
                    index += 1;
                }
                continue;
            };

            let mut aggregate = 0_usize;
            for output in &outputs[index..=complete] {
                aggregate = aggregate.saturating_add(encoded_output_bytes(output)?);
                if aggregate > MAX_BASELINE_AGGREGATE_BYTES {
                    return Err(ProductDevHostError::new(
                        "DEV_HOST_OUTPUT_BOUNDS",
                        "complete retained baseline exceeds the maximum aggregate byte length",
                    ));
                }
            }
            if index == 0 && complete + 1 == outputs.len() {
                entire_group_baseline = outputs[index].binding_marker();
            }
            index = complete + 1;
        }
        Ok(entire_group_baseline)
    }

    pub(crate) fn attach_complete_baseline_frontiers_to_binding(
        &self,
        binding: &mut Self,
    ) -> Result<(), ProductDevHostError> {
        let ProductDevRuntimeOutputWire::CompleteBaseline {
            publication_frontiers,
            ..
        } = &self.wire
        else {
            return Ok(());
        };
        if publication_frontiers.is_empty() {
            return Ok(());
        }
        let ProductDevRuntimeOutputWire::Binding {
            publication_frontiers: destination,
            ..
        } = &mut binding.wire
        else {
            return Err(ProductDevHostError::new(
                "DEV_HOST_OUTPUT_BASELINE",
                "a complete renderer frontier must attach to its baseline binding",
            ));
        };
        *destination = Some(publication_frontiers.clone());
        Ok(())
    }
}

impl Serialize for ProductDevRuntimeOutput {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProductDevRuntimeOutput {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ProductDevRuntimeOutputWire::deserialize(deserializer).map(|wire| Self { wire })
    }
}

/// One direct runtime receipt. The explicit owned output batch avoids a
/// separate server-side output mutation/callback path.
#[derive(Debug, Clone)]
pub struct ProductDevRuntimeReceipt<T> {
    receipt: runtime_session::RuntimeReceipt<T, ProductDevRuntimeOutput>,
    connection_output_cursor: Option<u64>,
}

fn encode_validated_wire(value: String) -> Result<Value, ProductDevHostError> {
    serde_json::from_str(&value).map_err(|_| {
        ProductDevHostError::new(
            "DEV_HOST_WIRE_ENCODE",
            "typed render frame could not be converted to wire JSON",
        )
    })
}

fn decode_strict_json<T>(
    bytes: &[u8],
    code: &'static str,
    detail: &'static str,
) -> Result<T, ProductDevHostError>
where
    T: for<'de> Deserialize<'de>,
{
    if bytes.len() > crate::MAX_REQUEST_BODY_BYTES {
        return Err(ProductDevHostError::new(
            "DEV_HOST_BODY_BOUNDS",
            "JSON payload exceeds the host body bound",
        ));
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = T::deserialize(&mut decoder).map_err(|_| ProductDevHostError::new(code, detail))?;
    decoder
        .end()
        .map_err(|_| ProductDevHostError::new(code, detail))?;
    Ok(value)
}

fn encoded_output_bytes(output: &ProductDevRuntimeOutput) -> Result<usize, ProductDevHostError> {
    serde_json::to_vec(output)
        .map(|encoded| encoded.len())
        .map_err(|error| ProductDevHostError::new("DEV_HOST_OUTPUT_ENCODE", error.to_string()))
}

fn complete_baseline_end(
    outputs: &[ProductDevRuntimeOutput],
    start: usize,
    binding: ProductDevRuntimeBinding,
) -> Result<Option<usize>, ProductDevHostError> {
    for (index, output) in outputs.iter().enumerate().skip(start + 1) {
        if output.binding_marker().is_some() {
            return Ok(None);
        }
        if let Some(completion) = output.complete_baseline_marker() {
            if completion != binding {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_OUTPUT_BASELINE",
                    "a baseline completion does not match its binding",
                ));
            }
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn bounded_diagnostic(value: String) -> Result<String, ProductDevHostError> {
    if value.is_empty() || value.len() > 1_024 {
        return Err(ProductDevHostError::new(
            "DEV_HOST_RESULT_DIAGNOSTIC",
            "runtime result diagnostic exceeds host bounds",
        ));
    }
    Ok(value)
}

impl<T> ProductDevRuntimeReceipt<T> {
    pub fn new(
        result: T,
        outputs: Vec<ProductDevRuntimeOutput>,
    ) -> Result<Self, ProductDevHostError> {
        ProductDevRuntimeOutput::validate_output_group(&outputs)?;
        Ok(Self {
            receipt: runtime_session::RuntimeReceipt::new(result, outputs),
            connection_output_cursor: None,
        })
    }

    pub fn result(&self) -> &T {
        self.receipt.result()
    }

    /// Attaches the shell-retained output cursor captured at a fresh worker
    /// connection boundary. This is local delivery metadata, not a product
    /// output or a worker-wire field.
    pub fn with_connection_output_cursor(mut self, cursor: u64) -> Self {
        self.connection_output_cursor = Some(cursor);
        self
    }

    /// Returns the shell-retained output cursor captured with a fresh worker
    /// connection response, when this receipt originated from that path.
    pub const fn connection_output_cursor(&self) -> Option<u64> {
        self.connection_output_cursor
    }

    pub fn into_parts(self) -> (T, Vec<ProductDevRuntimeOutput>) {
        self.receipt.into_parts()
    }
}

/// Concrete runtime owner implemented by the native product.
///
/// The server serializes calls with one mutex. Implementors own lifecycle,
/// input, schedule, timeline, mutation, and projection authority. They return
/// exact output receipts, so this trait has no subscription/callback method.
pub trait ProductDevRuntime: Send + 'static {
    /// Takes the one completed update-callback attribution sample, if this
    /// runtime exposes it. The host copies it after the callback, outside its
    /// diagnostics read path; older runtimes remain source-compatible.
    fn take_update_attribution(&mut self) -> Option<ProductDevUpdateAttribution> {
        None
    }
    /// Reports whether the runtime participates in the standard Rust-host
    /// realtime scheduler. Older/demand/external runtimes remain caller
    /// driven and return `Unsupported` by default.
    fn realtime_schedule_state(&self) -> ProductDevRuntimeScheduleState {
        ProductDevRuntimeScheduleState::Unsupported
    }

    /// Returns the admitted realtime observation interval. A standard host
    /// must derive cadence from the runtime's own fixed-step configuration;
    /// it must not duplicate a product-specific hertz constant.
    fn realtime_schedule_interval(&self) -> Option<std::time::Duration> {
        None
    }

    /// Establishes one browser connection to the current runtime generation.
    /// A concrete runtime may start from `Created` or publish a fresh baseline
    /// for an already-active generation, but must not reset active product
    /// state merely because another browser attached.
    fn connect(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.lifecycle(ProductDevLifecycleOperation::Start)
    }

    fn lifecycle(
        &mut self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError>;

    /// The standard host supplies the caller's observed control binding when
    /// it has one. Legacy runtime owners retain their existing lifecycle
    /// implementation; binding-aware owners can reject stale control actions
    /// without teaching the host any product/session policy.
    fn lifecycle_with_binding(
        &mut self,
        operation: ProductDevLifecycleOperation,
        _binding: Option<ProductDevRuntimeBinding>,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.lifecycle(operation)
    }

    /// Binding-aware runtimes use this narrow path for controller replacement
    /// and release. The default keeps older runtime owners source-compatible.
    fn control(
        &mut self,
        operation: ProductDevControlOperation,
        _binding: ProductDevRuntimeBinding,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        Err(ProductDevRuntimeError::new_not_applied(
            "DEV_HOST_CONTROL_UNSUPPORTED",
            format!(
                "{} control is not supported by this runtime",
                operation.as_wire()
            ),
        )
        .expect("fixed control diagnostic"))
    }

    fn input(
        &mut self,
        batch: ProductDevInputBatch,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevInputResult>, ProductDevRuntimeError>;

    /// Clears a host-mailbox overflow at the runtime binding fence. A
    /// binding-aware runtime can advance its control revision and publish a
    /// fresh baseline; older runtime implementations remain source-compatible
    /// and simply report that this recovery lane is unavailable.
    fn recover_input_overflow(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        Err(ProductDevRuntimeError::new_not_applied(
            "DEV_HOST_INPUT_RESYNC_UNSUPPORTED",
            "runtime does not expose host input-overflow recovery",
        )
        .expect("fixed input-resync unsupported diagnostic"))
    }

    /// Executes one bounded product-owned generated debug command between
    /// normal runtime operations. A semantic command failure remains a typed
    /// result; ABI/callback failures are runtime errors.
    fn execute_debug(
        &mut self,
        _command: &str,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevDebugResult>, ProductDevRuntimeError> {
        Err(ProductDevRuntimeError::new_not_applied(
            "DEV_HOST_DEBUG_UNSUPPORTED",
            "live debug commands are not supported by this runtime",
        )
        .expect("fixed debug unsupported diagnostic"))
    }

    /// Returns generated product catalog descriptor data when this product
    /// exports it. Older products remain loadable and report unavailable.
    fn describe_debug(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevDebugCatalog>, ProductDevRuntimeError> {
        ProductDevRuntimeReceipt::new(ProductDevDebugCatalog::unavailable(), Vec::new()).map_err(
            |error| {
                ProductDevRuntimeError::new(error.code(), error.detail())
                    .expect("fixed catalog unavailable diagnostic")
            },
        )
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError>;

    fn admit_demand_step(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError>;

    fn admit_external_step(
        &mut self,
        step: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError>;

    fn complete_timeline(
        &mut self,
        completion: ProductDevTimelineCompletion,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>, ProductDevRuntimeError>;

    /// Ingests the fixed Engine browser-host audio realization snapshot. This
    /// deliberately has no callback: a later ordinary product call exposes
    /// the copied facts through the generated audio service readout.
    fn report_audio_feedback(
        &mut self,
        _feedback: ProductDevAudioFeedback,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevAudioFeedbackResult>, ProductDevRuntimeError>
    {
        Err(ProductDevRuntimeError::new_not_applied(
            "DEV_HOST_AUDIO_FEEDBACK_UNSUPPORTED",
            "audio feedback is not supported by this runtime",
        )
        .expect("fixed audio-feedback diagnostic"))
    }

    fn report_animation_feedback(
        &mut self,
        _feedback: ProductDevAnimationFeedback,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevAnimationFeedbackResult>, ProductDevRuntimeError>
    {
        Err(ProductDevRuntimeError::new_not_applied(
            "DEV_HOST_ANIMATION_FEEDBACK_UNSUPPORTED",
            "animation feedback is not supported by this runtime",
        )
        .expect("fixed animation-feedback diagnostic"))
    }

    /// Ingests the latest retained ghost-plate renderer snapshot. It is
    /// exposed only by a later normal generated C# service read.
    fn report_ghost_plate_feedback(
        &mut self,
        _feedback: ProductDevGhostPlateFeedback,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevGhostPlateFeedbackResult>, ProductDevRuntimeError>
    {
        Err(ProductDevRuntimeError::new_not_applied(
            "DEV_HOST_GHOST_PLATE_FEEDBACK_UNSUPPORTED",
            "ghost plate feedback is not supported by this runtime",
        )
        .expect("fixed ghost-plate feedback diagnostic"))
    }

    fn report_renderer_diagnostics(
        &mut self,
        _feedback: ProductDevRendererDiagnosticsFeedback,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevRendererDiagnosticsFeedbackResult>,
        ProductDevRuntimeError,
    > {
        Err(ProductDevRuntimeError::new_not_applied(
            "DEV_HOST_RENDERER_DIAGNOSTICS_UNSUPPORTED",
            "renderer diagnostics are not supported by this runtime",
        )
        .expect("fixed renderer-diagnostics diagnostic"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feedback(fact: ProductDevGhostPlateFeedbackFact) -> ProductDevGhostPlateFeedback {
        ProductDevGhostPlateFeedback {
            runtime: ProductDevRuntimeBinding {
                instance_id: CanonicalU64::new(1),
                generation: CanonicalU64::new(1),
                control_revision: CanonicalU64::new(1),
            },
            replace_owner: true,
            facts: vec![fact],
        }
    }

    fn fact() -> ProductDevGhostPlateFeedbackFact {
        ProductDevGhostPlateFeedbackFact {
            presentation: CanonicalU64::new(9),
            source_matches: true,
            current_sector: 2,
            local_angular_offset_degrees: None,
            fallback_active: false,
            fallback_reason: ProductDevGhostPlateFallbackReason::None,
            limitation_mask: 127,
            preparation_cpu_milliseconds: Some(1.0),
            capture_cpu_submission_milliseconds: Some(2.0),
            retained_sector_count: 4,
            retained_mesh_count: 1,
            retained_material_count: 1,
            retained_borrowed_texture_count: 0,
        }
    }

    #[test]
    fn ghost_plate_feedback_rejects_invalid_timings_without_an_angle() {
        let mut invalid_preparation = fact();
        invalid_preparation.preparation_cpu_milliseconds = Some(-0.1);
        assert!(feedback(invalid_preparation).validate().is_err());

        let mut invalid_capture = fact();
        invalid_capture.capture_cpu_submission_milliseconds = Some(f64::NAN);
        assert!(feedback(invalid_capture).validate().is_err());
    }

    #[test]
    fn ghost_plate_feedback_requires_fallback_flag_and_reason_to_agree() {
        let mut inactive_reason = fact();
        inactive_reason.fallback_reason = ProductDevGhostPlateFallbackReason::RealizationFailed;
        assert!(feedback(inactive_reason).validate().is_err());

        let mut active_none = fact();
        active_none.fallback_active = true;
        assert!(feedback(active_none).validate().is_err());
    }

    #[test]
    fn runtime_faults_use_source_owned_recovery_and_taint_unknown_failures() {
        let ordinary = ProductDevOperationResult::rejected_runtime(
            ProductDevOperationKind::Start,
            ProductDevRuntimeError::new_not_applied(
                "CSHARP_NEW_SOURCE_REJECTION",
                "source rejected this operation before admission",
            )
            .unwrap(),
        )
        .unwrap();
        let ordinary = serde_json::to_value(ordinary).unwrap();
        assert_eq!(ordinary["code"], "CSHARP_NEW_SOURCE_REJECTION");
        assert_eq!(ordinary["disposition"], "rejected-recoverable");
        assert_eq!(ordinary["recovery"]["mutation"], "not-applied");
        assert_eq!(ordinary["recovery"]["invalidatedScope"], "none");
        assert_eq!(ordinary["recovery"]["nextAction"], "continue");

        let exhausted = ProductDevOperationResult::rejected_runtime(
            ProductDevOperationKind::AdvanceRealtime,
            ProductDevRuntimeError::new(
                "CSHARP_LIFECYCLE_COUNTER_EXHAUSTED",
                "runtime counter exhausted",
            )
            .unwrap(),
        )
        .unwrap();
        let exhausted = serde_json::to_value(exhausted).unwrap();
        assert_eq!(exhausted["disposition"], "terminal");

        let unknown_error =
            ProductDevRuntimeError::new("CSHARP_NEW_FAILURE", "unmapped runtime failure").unwrap();
        assert_eq!(
            unknown_error.recovery().mutation(),
            ProductDevMutationCertainty::Unknown
        );
        assert_eq!(
            unknown_error.recovery().invalidated_scope(),
            ProductDevInvalidatedScope::Incarnation
        );
        assert_eq!(
            unknown_error.recovery().next_action(),
            ProductDevNextAction::ReplaceIncarnation
        );
        let unknown = ProductDevOperationResult::rejected_runtime(
            ProductDevOperationKind::Start,
            unknown_error,
        )
        .unwrap();
        let unknown = serde_json::to_value(unknown).unwrap();
        assert_eq!(unknown["code"], "CSHARP_NEW_FAILURE");
        assert_eq!(unknown["disposition"], "terminal");
        assert_eq!(unknown["recovery"]["mutation"], "unknown");
        assert_eq!(unknown["recovery"]["invalidatedScope"], "incarnation");
        assert_eq!(unknown["recovery"]["nextAction"], "replace-incarnation");

        let projection_only = ProductDevOperationResult::rejected_runtime(
            ProductDevOperationKind::Start,
            ProductDevRuntimeError::new_output_rebaseline(
                "DEV_HOST_PROJECTION_ONLY",
                "retained output needs a fresh baseline",
            )
            .unwrap(),
        )
        .unwrap();
        let projection_only = serde_json::to_value(projection_only).unwrap();
        assert_eq!(projection_only["disposition"], "resync-required");
        assert_eq!(projection_only["recovery"]["invalidatedScope"], "outputs");
        assert_eq!(projection_only["recovery"]["nextAction"], "rebaseline");
    }

    #[test]
    fn wire_decode_resync_receipt_preserves_host_bounded_rejected_counts() {
        for count in [
            0,
            1,
            runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS,
            runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS + 1,
        ] {
            let result = ProductDevInputResult::wire_decode_resynchronized(count).unwrap();
            assert!(!result.accepted);
            assert_eq!(result.code, "DEV_HOST_INPUT_DECODE");
            assert_eq!(
                result.disposition,
                ProductDevFaultDisposition::ResyncRequired
            );
            assert_eq!(result.count, count);
            assert_eq!(result.accepted_count, 0);
            assert_eq!(result.dropped_count, count);
        }

        assert!(
            ProductDevInputResult::queued(runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS + 1)
                .is_err()
        );
    }

    #[test]
    fn runtime_output_wire_round_trips_without_a_wrapper() {
        let output = ProductDevRuntimeOutput::test_frame_value(serde_json::json!({
            "sequence": "7",
            "changed": true,
        }));
        let encoded = serde_json::to_vec(&output).unwrap();

        assert_eq!(
            serde_json::from_slice::<Value>(&encoded).unwrap()["kind"],
            "frame"
        );
        assert_eq!(
            ProductDevRuntimeOutput::decode_json(&encoded).unwrap(),
            output
        );
    }

    #[test]
    fn complete_baseline_frontiers_are_carried_by_its_binding() {
        let binding = ProductDevRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(3),
            control_revision: CanonicalU64::new(5),
        };
        let completion = ProductDevRuntimeOutput::complete_baseline_with_frontiers(
            binding,
            vec![ProductDevRendererPublicationFrontier::new("voxel:active".to_owned(), 4).unwrap()],
        );
        let mut baseline = ProductDevRuntimeOutput::binding(binding, CanonicalU64::new(0));

        completion
            .attach_complete_baseline_frontiers_to_binding(&mut baseline)
            .unwrap();

        assert_eq!(
            serde_json::to_value(baseline).unwrap(),
            serde_json::json!({
                "kind": "binding",
                "runtime": {
                    "instanceId": "7",
                    "generation": "3",
                    "controlRevision": "5",
                },
                "nextInputSequence": "0",
                "publicationFrontiers": [{ "stream": "voxel:active", "revision": 4 }],
            }),
        );
    }

    #[test]
    fn output_group_only_grants_the_larger_budget_to_a_complete_baseline() {
        let binding = ProductDevRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(3),
            control_revision: CanonicalU64::new(5),
        };
        let large = ProductDevRuntimeOutput::test_frame_value(serde_json::json!({
            "payload": "x".repeat(MAX_OUTPUT_AGGREGATE_BYTES + 1),
        }));
        let ordinary = ProductDevRuntimeOutput::validate_output_group(std::slice::from_ref(&large))
            .expect_err("ordinary output keeps the 16 MiB limit");
        assert_eq!(ordinary.code(), "DEV_HOST_OUTPUT_BOUNDS");

        assert_eq!(
            ProductDevRuntimeOutput::validate_output_group(&[
                ProductDevRuntimeOutput::binding(binding, CanonicalU64::new(0)),
                large.clone(),
                ProductDevRuntimeOutput::complete_baseline(binding),
            ])
            .expect("complete retained baseline gets its bounded recovery budget"),
            Some(binding),
        );

        assert_eq!(
            ProductDevRuntimeOutput::validate_output_group(&[
                ProductDevRuntimeOutput::binding(binding, CanonicalU64::new(0)),
                large,
                ProductDevRuntimeOutput::complete_baseline(binding),
                ProductDevRuntimeOutput::runtime_progress(),
            ])
            .expect("a bounded recovery baseline may share publication with following ticks"),
            None,
        );
    }
}
