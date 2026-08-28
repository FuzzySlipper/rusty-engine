use std::fmt;

use runtime_input::RuntimeInputEvent;
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use runtime_timeline::{
    RuntimeOpaqueData, RuntimeProvenance, RuntimeTimelineBinding, TimelineCompletionEnvelope,
    TimelineCompletionOutcome, TimelineCompletionTicketId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ProductDevHostError, ProductDevRuntimeError, MAX_OUTPUT_EVENT_BYTES, MAX_OUTPUT_QUEUE_ITEMS,
};

/// Fixed Engine-owned local-runtime route prefix consumed by product-browser-host.
pub const PRODUCT_DEV_RUNTIME_BASE_PATH: &str = "/__rusty/product/runtime/";
/// Identity for this local development host, not a product release/schema number.
pub const PRODUCT_DEV_HOST_ARTIFACT: &str = "rusty.product.dev-host";

/// A JSON u64 always represented by its canonical decimal string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalU64(u64);

impl CanonicalU64 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    /// Strictly decodes one canonical decimal JSON string from a host-owned
    /// payload. Keeping this admission on the typed value lets an in-process
    /// WebView adapter use the same u64 rule as the loopback host without
    /// exposing an unchecked string-to-u64 conversion.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductDevHostError> {
        decode_strict_json(
            bytes,
            "DEV_HOST_CANONICAL_U64",
            "canonical u64 JSON is invalid",
        )
    }
}

impl fmt::Display for CanonicalU64 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for CanonicalU64 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for CanonicalU64 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.is_empty()
            || (raw.len() > 1 && raw.starts_with('0'))
            || !raw.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(serde::de::Error::custom(
                "u64 must be canonical decimal text",
            ));
        }
        raw.parse().map(Self).map_err(serde::de::Error::custom)
    }
}

/// Exact runtime generation binding used by browser input, operations, and outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductDevRuntimeBinding {
    pub instance_id: CanonicalU64,
    pub generation: CanonicalU64,
    pub control_revision: CanonicalU64,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevOperationKind {
    Start,
    Pause,
    Resume,
    Restart,
    Shutdown,
    ReportFault,
    ReplaceControl,
    ReleaseControl,
    AdvanceRealtime,
    AdmitDemandStep,
    AdmitExternalStep,
    ReportAudioFeedback,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevAudioFeedbackResult {
    pub accepted: bool,
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
            runtime,
            accepted_through_fact_id: None,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }
}

/// Minimal local readout passed through from the generated runtime owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevRuntimeReadout {
    artifact: &'static str,
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
            artifact: "rusty.product.runtime-readout",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevRuntimeMode {
    Realtime,
    Demand,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevRuntimeState {
    Created,
    Running,
    Paused,
    Faulted,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevRuntimeFault {
    OwnerReported,
    CounterExhausted,
}

/// Direct operation result supplied by the generated runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevOperationResult {
    accepted: bool,
    operation: ProductDevOperationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    binding: Option<ProductDevRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readout: Option<ProductDevRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<String>,
}

impl ProductDevOperationResult {
    pub fn accepted(
        operation: ProductDevOperationKind,
        binding: ProductDevRuntimeBinding,
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
            operation,
            binding: Some(binding),
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
            operation,
            binding: None,
            readout: None,
            diagnostic: Some(diagnostic),
        })
    }
}

/// Typed input batch passed from the dev host to the runtime input owner.
#[derive(Debug, Clone)]
pub struct ProductDevInputBatch {
    events: Vec<RuntimeInputEvent>,
}

impl ProductDevInputBatch {
    pub fn new(events: Vec<RuntimeInputEvent>) -> Self {
        Self { events }
    }

    pub fn events(&self) -> &[RuntimeInputEvent] {
        &self.events
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
        Ok(Self::new(events))
    }
}

/// Typed input result supplied by the generated runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevInputResult {
    accepted: bool,
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    binding: Option<ProductDevRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readout: Option<ProductDevRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<String>,
}

impl ProductDevInputResult {
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
            count,
            binding: Some(binding),
            readout: Some(readout),
            diagnostic: None,
        })
    }

    pub fn rejected(diagnostic: impl Into<String>) -> Result<Self, ProductDevHostError> {
        Ok(Self {
            accepted: false,
            count: 0,
            binding: None,
            readout: None,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }
}

/// One exact completion forwarded to the runtime timeline owner.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductDevTimelineCompletion {
    envelope: TimelineCompletionEnvelope,
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
        Self::from_wire(value)
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
        Ok(Self { envelope })
    }

    pub fn envelope(&self) -> &TimelineCompletionEnvelope {
        &self.envelope
    }

    pub fn into_envelope(self) -> TimelineCompletionEnvelope {
        self.envelope
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevTimelineCompletionResult {
    accepted: bool,
    ticket: CanonicalU64,
    #[serde(skip_serializing_if = "Option::is_none")]
    binding: Option<ProductDevRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readout: Option<ProductDevRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<String>,
}

impl ProductDevTimelineCompletionResult {
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
            ticket,
            binding: None,
            readout: None,
            diagnostic: Some(bounded_diagnostic(diagnostic.into())?),
        })
    }
}

/// One Rust-authoritative output pushed to the local browser projection.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductDevRuntimeOutput {
    wire: ProductDevRuntimeOutputWire,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum ProductDevRuntimeOutputWire {
    Binding { runtime: ProductDevRuntimeBinding },
    CompleteBaseline { runtime: ProductDevRuntimeBinding },
    Frame { frame: Value },
    ViewComposition { composition: Value },
    Presentation { frame: Value },
    UiProjection { envelope: Value },
    RuntimeReadout { readout: ProductDevRuntimeReadout },
}

impl ProductDevRuntimeOutput {
    pub fn binding(runtime: ProductDevRuntimeBinding) -> Self {
        Self {
            wire: ProductDevRuntimeOutputWire::Binding { runtime },
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
    /// Marks the end of one complete current-binding projection. The host
    /// buffers its preceding binding-tagged facts and exposes them together;
    /// later facts for that binding are incremental.
    pub fn complete_baseline(runtime: ProductDevRuntimeBinding) -> Self {
        Self {
            wire: ProductDevRuntimeOutputWire::CompleteBaseline { runtime },
        }
    }

    pub(crate) const fn binding_marker(&self) -> Option<ProductDevRuntimeBinding> {
        match &self.wire {
            ProductDevRuntimeOutputWire::Binding { runtime } => Some(*runtime),
            _ => None,
        }
    }

    pub(crate) const fn complete_baseline_marker(&self) -> Option<ProductDevRuntimeBinding> {
        match &self.wire {
            ProductDevRuntimeOutputWire::CompleteBaseline { runtime } => Some(*runtime),
            _ => None,
        }
    }
}

impl Serialize for ProductDevRuntimeOutput {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.wire.serialize(serializer)
    }
}

/// One direct runtime receipt. The explicit owned output batch avoids a
/// separate server-side output mutation/callback path.
#[derive(Debug, Clone)]
pub struct ProductDevRuntimeReceipt<T> {
    result: T,
    outputs: Vec<ProductDevRuntimeOutput>,
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
        if outputs.len() > MAX_OUTPUT_QUEUE_ITEMS {
            return Err(ProductDevHostError::new(
                "DEV_HOST_OUTPUT_BATCH_BOUNDS",
                "runtime receipt contains too many output events",
            ));
        }
        for output in &outputs {
            let encoded = serde_json::to_vec(output).map_err(|error| {
                ProductDevHostError::new("DEV_HOST_OUTPUT_ENCODE", error.to_string())
            })?;
            if encoded.len() > MAX_OUTPUT_EVENT_BYTES {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_OUTPUT_BOUNDS",
                    "runtime receipt output exceeds the maximum event byte length",
                ));
            }
        }
        Ok(Self { result, outputs })
    }

    pub fn result(&self) -> &T {
        &self.result
    }

    pub fn into_parts(self) -> (T, Vec<ProductDevRuntimeOutput>) {
        (self.result, self.outputs)
    }
}

/// Source-linked concrete runtime owner implemented by Product Assembly.
///
/// The server serializes calls with one mutex. Implementors own lifecycle,
/// input, schedule, timeline, mutation, and projection authority. They return
/// exact output receipts, so this trait has no subscription/callback method.
pub trait ProductDevRuntime: Send + 'static {
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
        Err(ProductDevRuntimeError::new(
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
        Err(ProductDevRuntimeError::new(
            "DEV_HOST_AUDIO_FEEDBACK_UNSUPPORTED",
            "audio feedback is not supported by this runtime",
        )
        .expect("fixed audio-feedback diagnostic"))
    }
}
