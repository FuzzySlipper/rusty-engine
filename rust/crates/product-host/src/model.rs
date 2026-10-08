use crate::publication::{RuntimePublication, RuntimePublicationError};
use crate::timeline::{
    RuntimeOpaqueData, RuntimeProvenance, RuntimeTimelineBinding, TimelineCompletionEnvelope,
    TimelineCompletionOutcome, TimelineCompletionTicketId,
};
pub use runtime_diagnostics::CanonicalU64;
use runtime_diagnostics::{RuntimeDiagnosticRuntimeBinding, RuntimeUpdateAttribution};
use runtime_input::{RuntimeInputBinding, RuntimeInputEvent};
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::{ProductHostError, ProductHostRuntimeError};

/// Fixed Engine-owned local-runtime route prefix consumed by product-browser-host.
pub const PRODUCT_HOST_RUNTIME_BASE_PATH: &str = "/__rusty/product/runtime/";
/// Identity for this local product host, not a product release/schema number.
pub const PRODUCT_HOST_ARTIFACT: &str = "rusty.product.host";

/// Exact runtime generation binding used by browser input, operations, and outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostRuntimeBinding {
    pub instance_id: CanonicalU64,
    pub generation: CanonicalU64,
    pub control_revision: CanonicalU64,
}

impl From<ProductHostRuntimeBinding> for RuntimeDiagnosticRuntimeBinding {
    fn from(value: ProductHostRuntimeBinding) -> Self {
        Self {
            instance_id: value.instance_id,
            generation: value.generation,
            control_revision: value.control_revision,
        }
    }
}

impl From<RuntimeDiagnosticRuntimeBinding> for ProductHostRuntimeBinding {
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
pub enum ProductHostLifecycleOperation {
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
pub enum ProductHostControlOperation {
    Replace,
    Release,
}

impl ProductHostControlOperation {
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Release => "release",
        }
    }

    pub const fn operation_kind(self) -> ProductHostOperationKind {
        match self {
            Self::Replace => ProductHostOperationKind::ReplaceControl,
            Self::Release => ProductHostOperationKind::ReleaseControl,
        }
    }
}

impl ProductHostLifecycleOperation {
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

    pub const fn operation_kind(self) -> ProductHostOperationKind {
        match self {
            Self::Start => ProductHostOperationKind::Start,
            Self::Pause => ProductHostOperationKind::Pause,
            Self::Resume => ProductHostOperationKind::Resume,
            Self::Restart => ProductHostOperationKind::Restart,
            Self::Shutdown => ProductHostOperationKind::Shutdown,
            Self::ReportFault => ProductHostOperationKind::ReportFault,
        }
    }
}

/// Closed operation identities returned by direct runtime calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostOperationKind {
    Connect,
    Start,
    Pause,
    Resume,
    Restart,
    Shutdown,
    ReportFault,
    ReplaceControl,
    ReleaseControl,
    ClaimControl,
    Input,
    AdvanceRealtime,
    CompleteTimeline,
    ExecuteDebug,
}

/// Closed recovery posture for one host operation result. The code identifies
/// the precise failure while this value tells a host what it may safely do
/// next without interpreting the diagnostic text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostFaultDisposition {
    Accepted,
    RejectedRecoverable,
    Degraded,
    ResyncRequired,
    Terminal,
}

impl ProductHostFaultDisposition {
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

const ACCEPTED_FAULT_CODE: &str = "PRODUCT_HOST_ACCEPTED";

pub fn runtime_fault_disposition(error: &ProductHostRuntimeError) -> ProductHostFaultDisposition {
    error.disposition()
}

fn runtime_fault_fields(
    error: ProductHostRuntimeError,
) -> (String, ProductHostFaultDisposition, String) {
    (
        error.code().to_owned(),
        error.disposition(),
        error.diagnostic().to_owned(),
    )
}

/// Fixed browser-host observation batch. This is deliberately a small health
/// report, not a browser console or generic diagnostic transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostBrowserDiagnosticsReport {
    pub host_state: ProductHostBrowserHostState,
    pub runtime_progress: CanonicalU64,
    pub transport_state: ProductHostBrowserConnectionState,
    pub output_state: ProductHostBrowserConnectionState,
    #[ts(optional)]
    pub first_terminal: Option<ProductHostBrowserTerminalDiagnostic>,
    #[ts(optional)]
    pub recoverable_event: Option<ProductHostBrowserTerminalDiagnostic>,
    pub page_events: Vec<ProductHostBrowserPageDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub attachment: Option<ProductHostBrowserAttachment>,
}

/// A browser connection identity and, after a fresh successful attachment, its
/// retained runtime boundary. This is deliberately limited to the one browser
/// attachment that may correlate a previously lost HTTP response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostBrowserAttachment {
    pub id: String,
    #[ts(optional)]
    pub replaces: Option<String>,
    #[ts(optional)]
    pub baseline: Option<ProductHostBrowserAttachmentBaseline>,
}

/// The fixed runtime facts that prove a browser attachment has a fresh output
/// baseline. It is not a general browser state snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostBrowserAttachmentBaseline {
    pub runtime: ProductHostRuntimeBinding,
    pub next_input_sequence: CanonicalU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostBrowserHostState {
    Loading,
    Ready,
    Degraded,
    Failed,
    Disposed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostBrowserConnectionState {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostBrowserTerminalDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostBrowserPageDiagnostic {
    pub kind: ProductHostBrowserPageDiagnosticKind,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostBrowserPageDiagnosticKind {
    Error,
    UnhandledRejection,
}

impl ProductHostBrowserDiagnosticsReport {
    pub const MAX_PAGE_EVENTS: usize = 8;

    pub fn validate(&self) -> Result<(), ProductHostError> {
        if self.page_events.len() > Self::MAX_PAGE_EVENTS {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_BROWSER_DIAGNOSTICS_BOUNDS",
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
                "CSHARP_LIFECYCLE_CLOCK_REGRESSION" | "BROWSER_LOCAL_REQUEST_UNAVAILABLE"
            ) {
                return Err(ProductHostError::new(
                    "PRODUCT_HOST_BROWSER_DIAGNOSTICS_BOUNDS",
                    "browser recoverable diagnostic code is not admitted",
                ));
            }
        }
        for diagnostic in &self.page_events {
            validate_browser_diagnostic(&diagnostic.code, &diagnostic.message)?;
        }
        if let Some(attachment) = &self.attachment {
            validate_browser_attachment_id(&attachment.id)?;
            if let Some(replaces) = &attachment.replaces {
                validate_browser_attachment_id(replaces)?;
            }
        }
        Ok(())
    }
}

pub(crate) fn browser_attachment_id_is_admitted(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn validate_browser_attachment_id(value: &str) -> Result<(), ProductHostError> {
    if browser_attachment_id_is_admitted(value) {
        Ok(())
    } else {
        Err(ProductHostError::new(
            "PRODUCT_HOST_BROWSER_DIAGNOSTICS_BOUNDS",
            "browser attachment identity is outside fixed bounds",
        ))
    }
}

fn validate_browser_diagnostic(code: &str, message: &str) -> Result<(), ProductHostError> {
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
        return Err(ProductHostError::new(
            "PRODUCT_HOST_BROWSER_DIAGNOSTICS_BOUNDS",
            "browser diagnostic code or message is outside fixed bounds",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostBrowserDiagnosticsResult {
    pub accepted: bool,
    pub reported: u8,
}

/// Bounded host-owned product-lane telemetry returned alongside the existing
/// diagnostics batch. These are observations only; renderer statistics are
/// answered by `engine.renderer` and are not folded into this product-lane
/// snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostTelemetrySnapshot {
    pub in_flight_operation: Option<ProductHostOperationKind>,
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
    pub runtime_progress_unavailable_reason: Option<String>,
    pub connections: usize,
    pub subscribers: usize,
    pub output_queue_items: usize,
    pub output_queue_capacity: usize,
    pub output_binding_active: bool,
    /// Bounded attribution for completed C# update callbacks. Service totals
    /// are nested within the callback duration, not additional frame time.
    pub update_attribution: Option<ProductHostUpdateAttributionSnapshot>,
}

/// One complete C# update callback observation. Durations are integer
/// microseconds so the diagnostics wire remains canonical and float-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostUpdateAttribution {
    pub runtime: Option<ProductHostRuntimeBinding>,
    pub simulation_step: CanonicalU64,
    pub admitted_step_count: CanonicalU64,
    pub post_callback_duration_us: CanonicalU64,
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

impl Default for ProductHostUpdateAttribution {
    fn default() -> Self {
        Self {
            runtime: None,
            simulation_step: CanonicalU64::new(0),
            admitted_step_count: CanonicalU64::new(0),
            post_callback_duration_us: CanonicalU64::new(0),
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

impl From<RuntimeUpdateAttribution> for ProductHostUpdateAttribution {
    fn from(value: RuntimeUpdateAttribution) -> Self {
        Self {
            runtime: None,
            simulation_step: CanonicalU64::new(0),
            admitted_step_count: CanonicalU64::new(0),
            post_callback_duration_us: CanonicalU64::new(value.post_callback_duration_us),
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostUpdateAttributionSnapshot {
    pub sample_count: CanonicalU64,
    pub callback_duration_us_p50: CanonicalU64,
    pub callback_duration_us_p95: CanonicalU64,
    pub callback_duration_us_max: CanonicalU64,
    pub latest: ProductHostUpdateAttribution,
    /// Slowest complete callback retained in the current rolling window.
    pub rolling_slowest: ProductHostUpdateAttribution,
    pub rolling_slowest_age_ms: CanonicalU64,
    /// Slowest complete callback observed for this host lifetime.
    pub slowest: ProductHostUpdateAttribution,
    pub slowest_age_ms: CanonicalU64,
}

/// Minimal local readout passed through from the generated runtime owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostRuntimeReadout {
    #[ts(type = "\"rusty.product.runtime-readout\"")]
    artifact: String,
    runtime: ProductHostRuntimeBinding,
    state: ProductHostRuntimeState,
    admitted_simulation_steps: CanonicalU64,
    admitted_presentations: CanonicalU64,
    dropped_realtime_steps: CanonicalU64,
    clock_regressions: CanonicalU64,
    scaled_remainder: u32,
    last_observed_time_ns: Option<CanonicalU64>,
    fault: Option<ProductHostRuntimeFault>,
}

impl ProductHostRuntimeReadout {
    pub fn new(runtime: ProductHostRuntimeBinding, state: ProductHostRuntimeState) -> Self {
        Self {
            artifact: "rusty.product.runtime-readout".to_owned(),
            runtime,
            state,
            admitted_simulation_steps: CanonicalU64::new(0),
            admitted_presentations: CanonicalU64::new(0),
            dropped_realtime_steps: CanonicalU64::new(0),
            clock_regressions: CanonicalU64::new(0),
            scaled_remainder: 0,
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

    pub fn with_clock(mut self, scaled_remainder: u32, last_observed_time_ns: Option<u64>) -> Self {
        self.scaled_remainder = scaled_remainder;
        self.last_observed_time_ns = last_observed_time_ns.map(CanonicalU64::new);
        self
    }

    pub fn with_fault(mut self, fault: ProductHostRuntimeFault) -> Self {
        self.fault = Some(fault);
        self
    }

    pub const fn runtime(&self) -> ProductHostRuntimeBinding {
        self.runtime
    }

    /// Whether a browser holding `previous` needs this readout: identity,
    /// state and fault. Per-tick counters and clock samples stay with
    /// live debug (`engine.time`).
    pub fn changes_browser_view(&self, previous: &Self) -> bool {
        self.runtime != previous.runtime
            || self.state != previous.state
            || self.fault != previous.fault
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostRuntimeState {
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
pub enum ProductHostRuntimeScheduleState {
    /// This runtime is caller driven and does not opt into the standard
    /// Rust-host realtime scheduler.
    Unsupported,
    Created,
    Running,
    /// Running, with playtest time held: only debug commands advance it, and
    /// queued input waits for the command that steps it.
    Held,
    Paused,
    Faulted,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostRuntimeFault {
    OwnerReported,
    CounterExhausted,
}

/// Direct operation result supplied by the generated runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostOperationResult {
    accepted: bool,
    code: String,
    disposition: ProductHostFaultDisposition,
    operation: ProductHostOperationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    binding: Option<ProductHostRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    next_input_sequence: Option<CanonicalU64>,
    /// Last lifecycle simulation step admitted before this operation result.
    /// It is present on a resync receipt when admission has already advanced
    /// but the downstream callback/update could not be completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    admitted_through: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    readout: Option<ProductHostRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    diagnostic: Option<String>,
}

/// One bounded result returned by a product-owned generated debug catalog.
/// A failed command is a completed product operation, not a host/ABI failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostDebugResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    readout: Option<ProductHostRuntimeReadout>,
    succeeded: bool,
    message: String,
}

/// Read-only product-generated descriptor data for live-debug completion and
/// help. It is never a dispatch schema: command invocation remains the single
/// explicit `execute_debug` operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostDebugCatalog {
    available: bool,
    commands: Vec<ProductHostDebugCommandDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostDebugCommandDescriptor {
    name: String,
    description: String,
    parameters: Vec<ProductHostDebugCommandParameterDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductHostDebugCommandParameterDescriptor {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
}

impl ProductHostDebugCatalog {
    pub const MAX_COMMANDS: usize = 256;
    pub const MAX_PARAMETERS_PER_COMMAND: usize = 16;
    const MAX_TEXT_BYTES: usize = 1_024;

    pub const fn unavailable() -> Self {
        Self {
            available: false,
            commands: Vec::new(),
        }
    }

    /// Adds the inspection commands of a runtime that renders the world in
    /// its own process (the streaming browser mode).
    pub fn with_runtime_renderer_inspection(mut self) -> Self {
        for (name, description) in [
            (
                "engine.renderer.camera",
                "Read the observer camera, set it with `x y z yawDegrees pitchDegrees`, or clear it with `none`; a change draws a frame and names it",
            ),
            (
                "engine.renderer.drawing",
                "Read the drawing mode or select `continuous` or `on-demand`",
            ),
            (
                "engine.renderer.frame",
                "Draw one frame now, even on demand, and name its frame sequence and step",
            ),
        ] {
            if self.commands.iter().any(|command| command.name == name) {
                continue;
            }
            self.commands.push(ProductHostDebugCommandDescriptor {
                name: name.to_owned(),
                description: description.to_owned(),
                parameters: Vec::new(),
            });
        }
        self
    }

    pub fn with_renderer_diagnostics(mut self) -> Self {
        self.available = true;
        for (name, description) in [
            ("engine.time", "Read simulation time mode and current forward step"),
            ("engine.time.mode", "Select realtime, manual or action-driven time; never rewind"),
            ("engine.time.advance", "Advance held simulation by a bounded duration in milliseconds"),
            (
                "engine.renderer",
                "Show the renderer's adapter, output, recent stream frame costs, and skipped operations",
            ),
            (
                "engine.renderer.presentation",
                "Read the last drawn frame's sequence, step, cameras and viewport; GET frames/capture draws a tool capture that carries its own step",
            ),
            (
                "engine.renderer.snapshot",
                "Write the committed scene and its renderer resources to a file: engine.renderer.snapshot <path> (relative to the host's working directory); rusty-scene-render draws it offline",
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
            self.commands.push(ProductHostDebugCommandDescriptor {
                name: name.to_owned(),
                description: description.to_owned(),
                parameters: Vec::new(),
            });
        }
        self
    }

    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductHostError> {
        let catalog: Self = serde_json::from_slice(bytes).map_err(|_| {
            ProductHostError::new(
                "PRODUCT_HOST_DEBUG_CATALOG_DECODE",
                "generated debug catalog descriptor payload is invalid",
            )
        })?;
        catalog.validate()?;
        Ok(catalog)
    }

    fn validate(&self) -> Result<(), ProductHostError> {
        if !self.available || self.commands.len() > Self::MAX_COMMANDS {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_DEBUG_CATALOG_BOUNDS",
                "generated debug catalog availability or command count is invalid",
            ));
        }
        for command in &self.commands {
            validate_debug_descriptor_text(&command.name)?;
            validate_debug_descriptor_text(&command.description)?;
            if command.parameters.len() > Self::MAX_PARAMETERS_PER_COMMAND {
                return Err(ProductHostError::new(
                    "PRODUCT_HOST_DEBUG_CATALOG_BOUNDS",
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

fn validate_debug_descriptor_text(value: &str) -> Result<(), ProductHostError> {
    if value.len() > ProductHostDebugCatalog::MAX_TEXT_BYTES || value.contains('\0') {
        return Err(ProductHostError::new(
            "PRODUCT_HOST_DEBUG_CATALOG_BOUNDS",
            "generated debug catalog descriptor text is invalid",
        ));
    }
    Ok(())
}

impl ProductHostDebugResult {
    pub fn with_readout(mut self, readout: ProductHostRuntimeReadout) -> Self {
        self.readout = Some(readout);
        self
    }
    pub fn readout(&self) -> Option<&ProductHostRuntimeReadout> {
        self.readout.as_ref()
    }
    pub fn new(succeeded: bool, message: String) -> Self {
        Self {
            succeeded,
            message,
            readout: None,
        }
    }

    pub const fn succeeded(&self) -> bool {
        self.succeeded
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl ProductHostOperationResult {
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }

    pub const fn disposition(&self) -> ProductHostFaultDisposition {
        self.disposition
    }

    pub const fn admitted_through(&self) -> Option<CanonicalU64> {
        self.admitted_through
    }

    pub const fn binding(&self) -> Option<ProductHostRuntimeBinding> {
        self.binding
    }

    pub const fn next_input_sequence(&self) -> Option<CanonicalU64> {
        self.next_input_sequence
    }

    pub fn readout(&self) -> Option<&ProductHostRuntimeReadout> {
        self.readout.as_ref()
    }

    pub fn accepted(
        operation: ProductHostOperationKind,
        binding: ProductHostRuntimeBinding,
        next_input_sequence: CanonicalU64,
        readout: ProductHostRuntimeReadout,
    ) -> Result<Self, ProductHostError> {
        if readout.runtime() != binding {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_RESULT_BINDING",
                "runtime operation receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductHostFaultDisposition::Accepted,
            operation,
            binding: Some(binding),
            next_input_sequence: Some(next_input_sequence),
            admitted_through: None,
            readout: Some(readout),
            diagnostic: None,
        })
    }

    pub fn rejected(
        operation: ProductHostOperationKind,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductHostError> {
        let diagnostic = diagnostic.into();
        Ok(Self {
            accepted: false,
            code: "PRODUCT_HOST_OPERATION_REJECTED".to_owned(),
            disposition: ProductHostFaultDisposition::RejectedRecoverable,
            operation,
            binding: None,
            next_input_sequence: None,
            admitted_through: None,
            readout: None,
            diagnostic: Some(diagnostic),
        })
    }

    pub fn rejected_runtime(
        operation: ProductHostOperationKind,
        error: ProductHostRuntimeError,
    ) -> Result<Self, ProductHostError> {
        let (code, disposition, diagnostic) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
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
    pub fn resync_required(
        operation: ProductHostOperationKind,
        binding: ProductHostRuntimeBinding,
        next_input_sequence: CanonicalU64,
        readout: ProductHostRuntimeReadout,
        admitted_through: Option<CanonicalU64>,
        code: impl Into<String>,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductHostError> {
        if readout.runtime() != binding {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_RESULT_BINDING",
                "runtime resync receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: false,
            code: code.into(),
            disposition: ProductHostFaultDisposition::ResyncRequired,
            operation,
            binding: Some(binding),
            next_input_sequence: Some(next_input_sequence),
            admitted_through,
            readout: Some(readout),
            diagnostic: Some(diagnostic.into()),
        })
    }
}

/// Typed input batch passed from the product host to the runtime input owner.
#[derive(Debug, Clone)]
pub struct ProductHostInputBatch {
    events: Vec<RuntimeInputEvent>,
    wire_json: Option<Vec<u8>>,
}

impl ProductHostInputBatch {
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
    /// adapters. This host owns HTTP byte framing; runtime-input owns event
    /// semantics and the event-page count. Decoder errors are mapped to the
    /// product host error surface.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductHostError> {
        if bytes.len() > crate::MAX_REQUEST_BODY_BYTES {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_BODY_BOUNDS",
                "input batch exceeds the host JSON body bound",
            ));
        }
        let events = runtime_input::decode_runtime_input_wire_events_json(bytes).map_err(|_| {
            ProductHostError::new(
                "PRODUCT_HOST_INPUT_DECODE",
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostInputResult {
    accepted: bool,
    code: String,
    disposition: ProductHostFaultDisposition,
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
    #[ts(optional)]
    accepted_through: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    consumed_through: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    next_input_sequence: Option<CanonicalU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    binding: Option<ProductHostRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    readout: Option<ProductHostRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    diagnostic: Option<String>,
}

impl ProductHostInputResult {
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }

    pub fn accepted(
        count: usize,
        binding: ProductHostRuntimeBinding,
        readout: ProductHostRuntimeReadout,
    ) -> Result<Self, ProductHostError> {
        if count > runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_INPUT_RESULT_BOUNDS",
                "runtime input receipt count exceeds admitted batch bound",
            ));
        }
        if readout.runtime() != binding {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_RESULT_BINDING",
                "runtime input receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductHostFaultDisposition::Accepted,
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
    pub fn queued(count: usize) -> Result<Self, ProductHostError> {
        if count > runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_INPUT_RESULT_BOUNDS",
                "queued runtime input count exceeds admitted batch bound",
            ));
        }
        Ok(Self {
            accepted: true,
            code: "PRODUCT_HOST_INPUT_QUEUED".to_owned(),
            disposition: ProductHostFaultDisposition::Accepted,
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
    pub fn mailbox_full(count: usize) -> Result<Self, ProductHostError> {
        if count > runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_INPUT_RESULT_BOUNDS",
                "mailbox refusal count exceeds admitted batch bound",
            ));
        }
        Ok(Self {
            accepted: false,
            code: "PRODUCT_HOST_INPUT_MAILBOX_FULL".to_owned(),
            disposition: ProductHostFaultDisposition::ResyncRequired,
            count,
            accepted_count: 0,
            dropped_count: count,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: None,
            readout: None,
            diagnostic: Some("Rust-host input mailbox overflow cleared queued input; obtain a fresh runtime baseline before retrying".to_owned()),
        })
    }

    /// Reports that the browser submitted a structurally invalid wire batch
    /// and the runtime replaced its input control fence before continuing.
    /// The rejected batch is deliberately not replayable: the replacement
    /// binding and its sequence-zero clear arrive through the accompanying
    /// runtime output receipt. `count` is diagnostic-only here: the host
    /// already bounded the JSON request, while the admitted-wire-event bound
    /// does not apply to a batch rejected before admission.
    pub fn wire_decode_resynchronized(count: usize) -> Result<Self, ProductHostError> {
        Ok(Self {
            accepted: false,
            code: "PRODUCT_HOST_INPUT_DECODE".to_owned(),
            disposition: ProductHostFaultDisposition::ResyncRequired,
            count,
            accepted_count: 0,
            dropped_count: count,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: None,
            readout: None,
            diagnostic: Some("input batch was not a strict runtime-input wire batch; the runtime input binding was resynchronized".to_owned()),
        })
    }

    /// The callback-facing queue was fenced before product callback entry.
    /// The accompanying complete baseline names the committed replacement fence.
    pub fn pending_resynchronized(
        count: usize,
        next_input_sequence: CanonicalU64,
        binding: ProductHostRuntimeBinding,
        readout: ProductHostRuntimeReadout,
    ) -> Result<Self, ProductHostError> {
        let mut result = Self::with_progress(
            count,
            0,
            count,
            None,
            None,
            next_input_sequence,
            binding,
            readout,
        )?;
        result.code = "CSHARP_INPUT_PENDING_BOUNDS".to_owned();
        result.disposition = ProductHostFaultDisposition::ResyncRequired;
        result.diagnostic = Some("callback input pressure was resynchronized before callback entry; continue from the accompanying baseline".to_owned());
        Ok(result)
    }

    pub fn rejected(diagnostic: impl Into<String>) -> Result<Self, ProductHostError> {
        Ok(Self {
            accepted: false,
            code: "PRODUCT_HOST_INPUT_REJECTED".to_owned(),
            disposition: ProductHostFaultDisposition::RejectedRecoverable,
            count: 0,
            accepted_count: 0,
            dropped_count: 0,
            accepted_through: None,
            consumed_through: None,
            next_input_sequence: None,
            binding: None,
            readout: None,
            diagnostic: Some(diagnostic.into()),
        })
    }

    pub fn rejected_runtime(error: ProductHostRuntimeError) -> Result<Self, ProductHostError> {
        let (code, disposition, diagnostic) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
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
        binding: ProductHostRuntimeBinding,
        readout: ProductHostRuntimeReadout,
    ) -> Result<Self, ProductHostError> {
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
            return Err(ProductHostError::new(
                "PRODUCT_HOST_INPUT_RESULT_BOUNDS",
                "runtime input progress receipt has incoherent batch boundaries",
            ));
        }
        if readout.runtime() != binding {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_RESULT_BINDING",
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
                ProductHostFaultDisposition::Accepted
            } else {
                ProductHostFaultDisposition::RejectedRecoverable
            },
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
                Some(format!(
                    "dropped {dropped_count} stale or duplicate input event(s); synchronize the input cursor and do not replay them"
                ))
            },
        })
    }
}

/// One exact completion forwarded to the runtime timeline owner.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductHostTimelineCompletion {
    envelope: TimelineCompletionEnvelope,
    wire_json: Option<Vec<u8>>,
}

impl ProductHostTimelineCompletion {
    /// Strictly decodes the timeline completion wire object accepted by the
    /// host. The private wire DTO remains private; a packaged adapter receives
    /// only this validated, transport-neutral completion value.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductHostError> {
        let value: ProductHostTimelineCompletionWire = decode_strict_json(
            bytes,
            "PRODUCT_HOST_TIMELINE_DECODE",
            "timeline completion JSON is malformed or has unknown fields",
        )?;
        let mut completion = Self::from_wire(value)?;
        completion.wire_json = Some(bytes.to_vec());
        Ok(completion)
    }

    pub(crate) fn from_wire(
        value: ProductHostTimelineCompletionWire,
    ) -> Result<Self, ProductHostError> {
        if value.provenance.correlation != value.correlation {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_TIMELINE_CORRELATION",
                "timeline provenance correlation must match completion correlation",
            ));
        }
        let outcome_data = match value.outcome {
            ProductHostTimelineOutcomeWire::Success { data } => {
                TimelineCompletionOutcome::Success(data.map(RuntimeOpaqueData::new))
            }
            ProductHostTimelineOutcomeWire::Failure { data } => {
                TimelineCompletionOutcome::Failure(data.map(RuntimeOpaqueData::new))
            }
        };
        let provenance = RuntimeProvenance::new(
            value.provenance.correlation,
            value.provenance.detail.map(RuntimeOpaqueData::new),
        )
        .map_err(|_| {
            ProductHostError::new(
                "PRODUCT_HOST_TIMELINE_PROVENANCE",
                "timeline provenance violates timeline bounds",
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
            ProductHostError::new(
                "PRODUCT_HOST_TIMELINE_COMPLETION",
                "timeline completion violates timeline bounds",
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

#[derive(Debug, Clone, PartialEq, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename = "ProductHostTimelineCompletion")]
pub(crate) struct ProductHostTimelineCompletionWire {
    pub ticket: CanonicalU64,
    pub runtime: ProductHostRuntimeBinding,
    pub correlation: String,
    pub outcome: ProductHostTimelineOutcomeWire,
    pub provenance: ProductHostTimelineProvenanceWire,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
#[ts(rename = "ProductHostTimelineOutcome")]
pub(crate) enum ProductHostTimelineOutcomeWire {
    Success {
        #[serde(default)]
        #[ts(optional)]
        data: Option<Value>,
    },
    Failure {
        #[serde(default)]
        #[ts(optional)]
        data: Option<Value>,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename = "ProductHostTimelineProvenance")]
pub(crate) struct ProductHostTimelineProvenanceWire {
    pub correlation: String,
    #[serde(default)]
    #[ts(optional)]
    pub detail: Option<Value>,
}

/// Completion result supplied by the generated runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostTimelineCompletionResult {
    accepted: bool,
    code: String,
    disposition: ProductHostFaultDisposition,
    ticket: CanonicalU64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    binding: Option<ProductHostRuntimeBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    readout: Option<ProductHostRuntimeReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    diagnostic: Option<String>,
}

impl ProductHostTimelineCompletionResult {
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }

    pub const fn ticket(&self) -> CanonicalU64 {
        self.ticket
    }

    pub fn accepted(
        ticket: CanonicalU64,
        binding: ProductHostRuntimeBinding,
        readout: ProductHostRuntimeReadout,
    ) -> Result<Self, ProductHostError> {
        if readout.runtime() != binding {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_RESULT_BINDING",
                "timeline completion receipt binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: true,
            code: ACCEPTED_FAULT_CODE.to_owned(),
            disposition: ProductHostFaultDisposition::Accepted,
            ticket,
            binding: Some(binding),
            readout: Some(readout),
            diagnostic: None,
        })
    }
    pub fn rejected(
        ticket: CanonicalU64,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductHostError> {
        Ok(Self {
            accepted: false,
            code: "PRODUCT_HOST_TIMELINE_REJECTED".to_owned(),
            disposition: ProductHostFaultDisposition::RejectedRecoverable,
            ticket,
            binding: None,
            readout: None,
            diagnostic: Some(diagnostic.into()),
        })
    }

    /// A completion rejected before product callback entry still names the
    /// current runtime so a browser can retain its exact binding rather than
    /// treating the result as a stale-output failure.
    pub fn rejected_with_current(
        ticket: CanonicalU64,
        binding: ProductHostRuntimeBinding,
        readout: ProductHostRuntimeReadout,
        code: impl Into<String>,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductHostError> {
        if readout.runtime() != binding {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_RESULT_BINDING",
                "timeline rejection binding does not match its readout",
            ));
        }
        Ok(Self {
            accepted: false,
            code: code.into(),
            disposition: ProductHostFaultDisposition::RejectedRecoverable,
            ticket,
            binding: Some(binding),
            readout: Some(readout),
            diagnostic: Some(diagnostic.into()),
        })
    }

    pub fn rejected_runtime(
        ticket: CanonicalU64,
        error: ProductHostRuntimeError,
    ) -> Result<Self, ProductHostError> {
        let (code, disposition, diagnostic) = runtime_fault_fields(error);
        Ok(Self {
            accepted: false,
            code,
            disposition,
            ticket,
            binding: None,
            readout: None,
            diagnostic: Some(diagnostic),
        })
    }
}

/// One Rust-authoritative output pushed to the browser shell. The world is
/// rendered in the runtime, so the shell receives only its binding, the
/// product UI projection, readouts and input results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProductHostRuntimeOutput {
    wire: ProductHostRuntimeOutputWire,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[ts(rename = "ProductHostRuntimeOutput")]
pub(crate) enum ProductHostRuntimeOutputWire {
    Binding {
        runtime: ProductHostRuntimeBinding,
        #[serde(rename = "nextInputSequence")]
        next_input_sequence: CanonicalU64,
        /// The harness holding input, when one has claimed it: the page shows
        /// it and sends no input until a binding without a claim arrives.
        #[serde(
            rename = "inputClaim",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        #[ts(optional)]
        input_claim: Option<String>,
    },
    /// Ends the outputs that together make one binding's complete baseline.
    /// The host consumes it; the browser sees the `rusty-output-baseline`
    /// event instead.
    #[ts(skip)]
    CompleteBaseline {
        runtime: ProductHostRuntimeBinding,
    },
    UiProjection {
        #[ts(as = "runtime_ui::RuntimeUiProjectionWire")]
        envelope: runtime_ui::RuntimeUiProjectionEnvelope,
    },
    RuntimeReadout {
        readout: ProductHostRuntimeReadout,
    },
    /// One input batch admitted by the runtime at a scheduled update
    /// boundary. The result carries the authoritative input cursor and
    /// recovery disposition that cannot be returned by the earlier queued
    /// HTTP acknowledgement.
    RuntimeInputResult {
        result: ProductHostInputResult,
    },
}

impl ProductHostRuntimeOutput {
    /// Decodes one output through the same JSON representation used by the
    /// browser shell.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, ProductHostError> {
        serde_json::from_slice(bytes).map_err(|_| {
            ProductHostError::new(
                "PRODUCT_HOST_OUTPUT_DECODE",
                "output is not a valid runtime output",
            )
        })
    }

    /// The browser shell's share of one Engine publication. Graphics,
    /// presentation and animation cue publications stay in the runtime,
    /// whose renderer applies them.
    pub fn from_publication(publication: RuntimePublication) -> Option<Self> {
        let wire = match publication {
            RuntimePublication::Binding {
                runtime,
                next_input_sequence,
                input_claim,
                ..
            } => ProductHostRuntimeOutputWire::Binding {
                runtime: host_runtime_binding(runtime),
                next_input_sequence: CanonicalU64::new(next_input_sequence),
                input_claim,
            },
            RuntimePublication::CompleteBaseline { runtime, .. } => {
                ProductHostRuntimeOutputWire::CompleteBaseline {
                    runtime: host_runtime_binding(runtime),
                }
            }
            RuntimePublication::UiProjection(envelope) => {
                ProductHostRuntimeOutputWire::UiProjection { envelope }
            }
            RuntimePublication::Frame(_)
            | RuntimePublication::ViewComposition(_)
            | RuntimePublication::Presentation(_) => return None,
        };
        Some(Self { wire })
    }

    /// A UI projection carrying `value`, for transport tests that vary byte
    /// volume and ordering.
    #[cfg(test)]
    pub(crate) fn test_value(value: Value) -> Self {
        use runtime_ui::{RuntimeUiProjectionEnvelope, RuntimeUiRuntimeBinding};
        let binding = RuntimeUiRuntimeBinding::new(
            RuntimeInstanceId::new(1),
            RuntimeGeneration::new(1),
            RuntimeControlRevision::new(1),
        );
        let envelope = RuntimeUiProjectionEnvelope::new(binding, 1, "test", "test", value)
            .expect("fixture projection");
        Self::ui_projection(&envelope)
    }

    pub fn binding(runtime: ProductHostRuntimeBinding, next_input_sequence: CanonicalU64) -> Self {
        Self {
            wire: ProductHostRuntimeOutputWire::Binding {
                runtime,
                next_input_sequence,
                input_claim: None,
            },
        }
    }

    pub fn ui_projection(envelope: &runtime_ui::RuntimeUiProjectionEnvelope) -> Self {
        Self {
            wire: ProductHostRuntimeOutputWire::UiProjection {
                envelope: envelope.clone(),
            },
        }
    }

    pub fn runtime_readout(readout: ProductHostRuntimeReadout) -> Self {
        Self {
            wire: ProductHostRuntimeOutputWire::RuntimeReadout { readout },
        }
    }

    /// Publishes one scheduled input admission result through the ordered
    /// runtime output stream. The HTTP input route only acknowledges mailbox
    /// admission; this is the later runtime-owned receipt.
    pub fn runtime_input_result(result: ProductHostInputResult) -> Self {
        Self {
            wire: ProductHostRuntimeOutputWire::RuntimeInputResult { result },
        }
    }

    /// Marks the end of one complete current-binding projection. The host
    /// buffers its preceding binding-tagged facts and exposes them together;
    /// later facts for that binding are incremental.
    pub fn complete_baseline(runtime: ProductHostRuntimeBinding) -> Self {
        Self {
            wire: ProductHostRuntimeOutputWire::CompleteBaseline { runtime },
        }
    }

    pub(crate) const fn binding_marker(&self) -> Option<ProductHostRuntimeBinding> {
        match &self.wire {
            ProductHostRuntimeOutputWire::Binding { runtime, .. } => Some(*runtime),
            _ => None,
        }
    }

    pub(crate) const fn complete_baseline_marker(&self) -> Option<ProductHostRuntimeBinding> {
        match &self.wire {
            ProductHostRuntimeOutputWire::CompleteBaseline { runtime } => Some(*runtime),
            _ => None,
        }
    }

    /// Checks binding/completion coherence without encoding or imposing delivery budgets.
    /// Returns a binding only when the whole group is one complete baseline.
    pub fn validate_output_group(
        outputs: &[Self],
    ) -> Result<Option<ProductHostRuntimeBinding>, ProductHostError> {
        let mut whole_baseline = None;
        let mut index = 0;
        while index < outputs.len() {
            if let Some(binding) = outputs[index].binding_marker() {
                if let Some(end) = complete_baseline_end(outputs, index, binding)? {
                    if index == 0 && end + 1 == outputs.len() {
                        whole_baseline = Some(binding);
                    }
                    index = end;
                }
            }
            index += 1;
        }
        Ok(whole_baseline)
    }
}

fn publication_error(error: RuntimePublicationError) -> ProductHostError {
    ProductHostError::new("PRODUCT_HOST_RUNTIME_PUBLICATION", error.to_string())
}

fn host_runtime_binding(binding: RuntimeInputBinding) -> ProductHostRuntimeBinding {
    ProductHostRuntimeBinding {
        instance_id: CanonicalU64::new(binding.instance_id().value()),
        generation: CanonicalU64::new(binding.generation().value()),
        control_revision: CanonicalU64::new(binding.control_revision().value()),
    }
}

/// One direct runtime receipt. The explicit owned output batch avoids a
/// separate server-side output mutation/callback path.
#[derive(Debug, Clone)]
pub struct ProductHostRuntimeReceipt<T> {
    receipt: crate::publication::RuntimeReceipt<T>,
}

fn decode_strict_json<T>(
    bytes: &[u8],
    code: &'static str,
    detail: &'static str,
) -> Result<T, ProductHostError>
where
    T: for<'de> Deserialize<'de>,
{
    if bytes.len() > crate::MAX_REQUEST_BODY_BYTES {
        return Err(ProductHostError::new(
            "PRODUCT_HOST_BODY_BOUNDS",
            "JSON payload exceeds the host body bound",
        ));
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = T::deserialize(&mut decoder).map_err(|_| ProductHostError::new(code, detail))?;
    decoder
        .end()
        .map_err(|_| ProductHostError::new(code, detail))?;
    Ok(value)
}

fn complete_baseline_end(
    outputs: &[ProductHostRuntimeOutput],
    start: usize,
    binding: ProductHostRuntimeBinding,
) -> Result<Option<usize>, ProductHostError> {
    for (index, output) in outputs.iter().enumerate().skip(start + 1) {
        if output.binding_marker().is_some() {
            return Ok(None);
        }
        if let Some(completion) = output.complete_baseline_marker() {
            if completion != binding {
                return Err(ProductHostError::new(
                    "PRODUCT_HOST_OUTPUT_BASELINE",
                    "a baseline completion does not match its binding",
                ));
            }
            return Ok(Some(index));
        }
    }
    Ok(None)
}

impl<T> ProductHostRuntimeReceipt<T> {
    pub fn new(result: T, outputs: Vec<RuntimePublication>) -> Result<Self, ProductHostError> {
        for output in &outputs {
            output.validate().map_err(publication_error)?;
        }
        Ok(Self {
            receipt: crate::publication::RuntimeReceipt::new(result, outputs),
        })
    }

    pub fn result(&self) -> &T {
        self.receipt.result()
    }

    pub fn into_parts(self) -> (T, Vec<RuntimePublication>) {
        self.receipt.into_parts()
    }

    /// Encode only at the serving edge. Runtime receipts retain typed
    /// Engine facts; the browser shell's share is converted here.
    pub fn into_wire_parts(self) -> Result<(T, Vec<ProductHostRuntimeOutput>), ProductHostError> {
        let (result, publications) = self.receipt.into_parts();
        let outputs = publications
            .into_iter()
            .filter_map(ProductHostRuntimeOutput::from_publication)
            .collect::<Vec<_>>();
        ProductHostRuntimeOutput::validate_output_group(&outputs)?;
        Ok((result, outputs))
    }
}

/// Concrete runtime owner implemented by the native product.
///
/// The server serializes calls with one mutex. Implementors own lifecycle,
/// input, schedule, timeline, mutation, and projection authority. They return
/// exact output receipts, so this trait has no subscription/callback method.
pub trait ProductHostRuntime: Send + 'static {
    /// Re-reads the runtime's staged content after a development content edit
    /// without restarting the product. A runtime without reloadable content
    /// has nothing to do.
    fn reload_content(&mut self) -> Result<(), ProductHostRuntimeError> {
        Ok(())
    }

    /// Takes the one completed update-callback attribution sample, if this
    /// runtime exposes it. The host copies it after the callback, outside its
    /// diagnostics read path; older runtimes remain source-compatible.
    fn take_update_attribution(&mut self) -> Option<ProductHostUpdateAttribution> {
        None
    }
    /// Reports whether the runtime participates in the standard Rust-host
    /// realtime scheduler. Caller-driven runtimes return `Unsupported` by
    /// default.
    fn realtime_schedule_state(&self) -> ProductHostRuntimeScheduleState {
        ProductHostRuntimeScheduleState::Unsupported
    }

    /// Returns the admitted realtime observation interval. A standard host
    /// must derive cadence from the runtime's own fixed-step configuration;
    /// it must not duplicate a product-specific hertz constant.
    fn realtime_schedule_interval(&self) -> Option<std::time::Duration> {
        None
    }

    /// How often the host should present between realtime observations, when
    /// the runtime has Engine-owned motion to show faster than its fixed
    /// steps (tweens on a faster display); `None` when there is none.
    fn presentation_interval(&self) -> Option<std::time::Duration> {
        None
    }

    /// Shows Engine-owned motion at `observed_time_ns` between realtime
    /// observations. It admits no step, delivers no input and never calls
    /// the product; the next observation accounts the time it showed.
    fn present_realtime(
        &mut self,
        _observed_time_ns: CanonicalU64,
    ) -> Result<Option<ProductHostRuntimeReceipt<ProductHostOperationResult>>, ProductHostRuntimeError>
    {
        Ok(None)
    }

    /// Establishes one browser connection to the current runtime generation.
    /// A concrete runtime may start from `Created` or publish a fresh baseline
    /// for an already-active generation, but must not reset active product
    /// state merely because another browser attached.
    fn connect(
        &mut self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.lifecycle(ProductHostLifecycleOperation::Start)
    }

    fn lifecycle(
        &mut self,
        operation: ProductHostLifecycleOperation,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>;

    /// The standard host supplies the caller's observed control binding when
    /// it has one. Legacy runtime owners retain their existing lifecycle
    /// implementation; binding-aware owners can reject stale control actions
    /// without teaching the host any product/session policy.
    fn lifecycle_with_binding(
        &mut self,
        operation: ProductHostLifecycleOperation,
        _binding: Option<ProductHostRuntimeBinding>,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.lifecycle(operation)
    }

    /// Binding-aware runtimes use this narrow path for controller replacement
    /// and release. The default keeps older runtime owners source-compatible.
    fn control(
        &mut self,
        operation: ProductHostControlOperation,
        _binding: ProductHostRuntimeBinding,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        Err(ProductHostRuntimeError::new_not_applied(
            "PRODUCT_HOST_CONTROL_UNSUPPORTED",
            format!(
                "{} control is not supported by this runtime",
                operation.as_wire()
            ),
        ))
    }

    /// A harness claims input under a fresh binding, labelled `label`, until
    /// `control/release` or until `lease` passes without its input. The
    /// published binding carries the claim, so an attached page sends no
    /// input and shows who holds it.
    fn claim_control(
        &mut self,
        _binding: ProductHostRuntimeBinding,
        _label: String,
        _lease: std::time::Duration,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        Err(ProductHostRuntimeError::new_not_applied(
            "PRODUCT_HOST_CONTROL_UNSUPPORTED",
            "claim control is not supported by this runtime",
        ))
    }

    fn input(
        &mut self,
        batch: ProductHostInputBatch,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostInputResult>, ProductHostRuntimeError>;

    /// Clears a host-mailbox overflow at the runtime binding fence. A
    /// binding-aware runtime can advance its control revision and publish a
    /// fresh baseline; older runtime implementations remain source-compatible
    /// and simply report that this recovery lane is unavailable.
    fn recover_input_overflow(
        &mut self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        Err(ProductHostRuntimeError::new_not_applied(
            "PRODUCT_HOST_INPUT_RESYNC_UNSUPPORTED",
            "runtime does not expose host input-overflow recovery",
        ))
    }

    /// Executes one bounded product-owned generated debug command between
    /// normal runtime operations. A semantic command failure remains a typed
    /// result; ABI/callback failures are runtime errors.
    fn execute_debug(
        &mut self,
        _command: &str,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostDebugResult>, ProductHostRuntimeError> {
        Err(ProductHostRuntimeError::new_not_applied(
            "PRODUCT_HOST_DEBUG_UNSUPPORTED",
            "live debug commands are not supported by this runtime",
        ))
    }

    /// Returns generated product catalog descriptor data when this product
    /// exports it. Older products remain loadable and report unavailable.
    fn describe_debug(
        &mut self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostDebugCatalog>, ProductHostRuntimeError> {
        ProductHostRuntimeReceipt::new(ProductHostDebugCatalog::unavailable(), Vec::new())
            .map_err(|error| ProductHostRuntimeError::new(error.code(), error.detail()))
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>;

    fn complete_timeline(
        &mut self,
        completion: ProductHostTimelineCompletion,
    ) -> Result<
        ProductHostRuntimeReceipt<ProductHostTimelineCompletionResult>,
        ProductHostRuntimeError,
    >;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_attachment_report_uses_bounded_typed_baseline_facts() {
        let report: ProductHostBrowserDiagnosticsReport = serde_json::from_str(
            r#"{
                "hostState":"ready",
                "runtimeProgress":"9",
                "transportState":"open",
                "outputState":"open",
                "pageEvents":[],
                "attachment":{
                    "id":"attachment-2",
                    "replaces":"attachment-1",
                    "baseline":{
                        "runtime":{"instanceId":"7","generation":"3","controlRevision":"12"},
                        "nextInputSequence":"14"
                    }
                }
            }"#,
        )
        .unwrap();

        report.validate().unwrap();
        let encoded = serde_json::to_value(&report).unwrap();
        assert_eq!(encoded["attachment"]["id"], "attachment-2");
        assert_eq!(encoded["attachment"]["baseline"]["nextInputSequence"], "14");

        let invalid_id: ProductHostBrowserDiagnosticsReport = serde_json::from_str(
            r#"{"hostState":"ready","runtimeProgress":"9","transportState":"open","outputState":"open","pageEvents":[],"attachment":{"id":"invalid id","replaces":null,"baseline":null}}"#,
        )
        .unwrap();
        assert_eq!(
            invalid_id.validate().unwrap_err().code(),
            "PRODUCT_HOST_BROWSER_DIAGNOSTICS_BOUNDS"
        );
    }

    #[test]
    fn runtime_faults_carry_their_source_disposition() {
        let result = |error| {
            serde_json::to_value(
                ProductHostOperationResult::rejected_runtime(
                    ProductHostOperationKind::Start,
                    error,
                )
                .unwrap(),
            )
            .unwrap()
        };
        let ordinary = result(ProductHostRuntimeError::new_not_applied(
            "CSHARP_NEW_SOURCE_REJECTION",
            "source rejected this operation before admission",
        ));
        assert_eq!(ordinary["code"], "CSHARP_NEW_SOURCE_REJECTION");
        assert_eq!(ordinary["disposition"], "rejected-recoverable");
        assert!(ordinary.get("recovery").is_none());

        let unknown = result(ProductHostRuntimeError::new(
            "CSHARP_NEW_FAILURE",
            "unmapped runtime failure",
        ));
        assert_eq!(unknown["code"], "CSHARP_NEW_FAILURE");
        assert_eq!(unknown["disposition"], "terminal");
    }

    #[test]
    fn wire_decode_resync_receipt_preserves_host_bounded_rejected_counts() {
        for count in [
            0,
            1,
            runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS,
            runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS + 1,
        ] {
            let result = ProductHostInputResult::wire_decode_resynchronized(count).unwrap();
            assert!(!result.accepted);
            assert_eq!(result.code, "PRODUCT_HOST_INPUT_DECODE");
            assert_eq!(
                result.disposition,
                ProductHostFaultDisposition::ResyncRequired
            );
            assert_eq!(result.count, count);
            assert_eq!(result.accepted_count, 0);
            assert_eq!(result.dropped_count, count);
        }

        assert!(
            ProductHostInputResult::queued(runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS + 1)
                .is_err()
        );
    }

    #[test]
    fn runtime_output_wire_round_trips_without_a_wrapper() {
        let output = ProductHostRuntimeOutput::test_value(serde_json::json!({
            "sequence": "7",
            "changed": true,
        }));
        let encoded = serde_json::to_vec(&output).unwrap();

        assert_eq!(
            serde_json::from_slice::<Value>(&encoded).unwrap()["kind"],
            "ui-projection"
        );
        assert_eq!(
            ProductHostRuntimeOutput::decode_json(&encoded).unwrap(),
            output
        );
    }

    #[test]
    fn only_the_shells_publications_reach_the_wire() {
        use crate::publication::{RuntimePublication, RuntimePublicationFrontier};
        use render_model::RenderFrameDiff;
        use runtime_input::RuntimeInputBinding;
        use runtime_ui::{RuntimeUiProjectionEnvelope, RuntimeUiRuntimeBinding};

        let binding = RuntimeInputBinding::new(
            RuntimeInstanceId::new(7),
            RuntimeGeneration::new(3),
            RuntimeControlRevision::new(5),
        );
        let frontier = RuntimePublicationFrontier::new("voxel", 4).unwrap();
        let baseline = ProductHostRuntimeOutput::from_publication(
            RuntimePublication::complete_baseline_with_frontiers(binding, vec![frontier]),
        )
        .expect("a baseline end reaches the shell");
        assert_eq!(
            serde_json::to_value(baseline).unwrap(),
            serde_json::json!({
                "kind": "complete-baseline",
                "runtime": { "instanceId": "7", "generation": "3", "controlRevision": "5" },
            }),
        );

        let frame = RenderFrameDiff::try_from_ops(vec![render_model::RenderDiff::Destroy {
            handle: render_model::RenderHandle::new(17),
        }])
        .unwrap();
        assert!(
            ProductHostRuntimeOutput::from_publication(RuntimePublication::Frame(frame)).is_none()
        );
        let envelope = RuntimeUiProjectionEnvelope::new(
            RuntimeUiRuntimeBinding::new(
                RuntimeInstanceId::new(7),
                RuntimeGeneration::new(3),
                RuntimeControlRevision::new(5),
            ),
            1,
            "hud",
            "fixture",
            serde_json::json!({ "visible": true }),
        )
        .unwrap();
        let canonical: Value = serde_json::from_slice(&envelope.encode_json().unwrap()).unwrap();
        let ui_wire =
            ProductHostRuntimeOutput::from_publication(RuntimePublication::UiProjection(envelope))
                .unwrap();
        let encoded = serde_json::to_vec(&ui_wire).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&encoded).unwrap()["envelope"],
            canonical
        );
        assert_eq!(
            ProductHostRuntimeOutput::decode_json(&encoded).unwrap(),
            ui_wire
        );
    }

    #[test]
    fn readouts_change_the_browser_view_only_on_lifecycle_facts() {
        let binding = ProductHostRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(1),
            control_revision: CanonicalU64::new(1),
        };
        let running = |steps: u64| {
            ProductHostRuntimeReadout::new(binding, ProductHostRuntimeState::Running)
                .with_counters(steps, steps, 0, 0)
                .with_clock(3, Some(steps * 16_666_667))
        };
        assert!(!running(2).changes_browser_view(&running(1)));
        let paused = ProductHostRuntimeReadout {
            state: ProductHostRuntimeState::Paused,
            ..running(2)
        };
        assert!(paused.changes_browser_view(&running(1)));
        let rebound = ProductHostRuntimeReadout {
            runtime: ProductHostRuntimeBinding {
                control_revision: CanonicalU64::new(2),
                ..binding
            },
            ..running(2)
        };
        assert!(rebound.changes_browser_view(&running(1)));
    }

    #[test]
    fn output_group_recognizes_baselines_without_a_size_preflight() {
        let binding = ProductHostRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(3),
            control_revision: CanonicalU64::new(5),
        };
        let large = ProductHostRuntimeOutput::test_value(serde_json::json!({
            "payload": "x".repeat(16 * 1024 * 1024 + 1),
        }));
        assert_eq!(
            ProductHostRuntimeOutput::validate_output_group(std::slice::from_ref(&large)).unwrap(),
            None
        );

        assert_eq!(
            ProductHostRuntimeOutput::validate_output_group(&[
                ProductHostRuntimeOutput::binding(binding, CanonicalU64::new(0)),
                large.clone(),
                ProductHostRuntimeOutput::complete_baseline(binding),
            ])
            .expect("complete retained baseline gets its bounded recovery budget"),
            Some(binding),
        );

        assert_eq!(
            ProductHostRuntimeOutput::validate_output_group(&[
                ProductHostRuntimeOutput::binding(binding, CanonicalU64::new(0)),
                large,
                ProductHostRuntimeOutput::complete_baseline(binding),
                ProductHostRuntimeOutput::test_value(serde_json::json!({})),
            ])
            .expect("a bounded recovery baseline may share publication with following ticks"),
            None,
        );
    }
}
