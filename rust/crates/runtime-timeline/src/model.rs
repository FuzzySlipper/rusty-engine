use std::fmt;

use runtime_lifecycle::{
    RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId, SimulationStep,
};
use serde_json::Value;

/// Maximum number of scheduled operations retained by one timeline lane.
pub const MAX_TIMELINE_OPERATIONS: usize = 1_024;
/// Maximum number of completion tickets retained by one timeline lane.
pub const MAX_TIMELINE_COMPLETION_TICKETS: usize = 1_024;
/// Maximum number of released events returned from one call.
pub const MAX_TIMELINE_RELEASE_PREFIX: usize = 256;
/// Maximum compact JSON bytes in one opaque runtime data value.
pub const MAX_RUNTIME_OPAQUE_DATA_BYTES: usize = 4_096;
/// Maximum UTF-8 bytes in a caller correlation or provenance label.
pub const MAX_RUNTIME_CORRELATION_BYTES: usize = product_model::MAX_IDENTITY_BYTES;
/// Maximum finite recurrence occurrences accepted for one operation.
pub const MAX_RECURRENCE_OCCURRENCES: u32 = 1_024;
/// Maximum number of operations or tickets represented by one typed snapshot.
pub const MAX_TIMELINE_SNAPSHOT_ITEMS: usize = MAX_TIMELINE_OPERATIONS;

/// Identity and lifecycle epoch of one explicit timeline owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeTimelineBinding {
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
}

impl RuntimeTimelineBinding {
    pub const fn new(
        instance_id: RuntimeInstanceId,
        generation: RuntimeGeneration,
        control_revision: RuntimeControlRevision,
    ) -> Self {
        Self {
            instance_id,
            generation,
            control_revision,
        }
    }

    pub const fn instance_id(self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn generation(self) -> RuntimeGeneration {
        self.generation
    }

    pub const fn control_revision(self) -> RuntimeControlRevision {
        self.control_revision
    }
}

/// Caller-chosen operation identity. The lane never invents game meaning for
/// this value; it uses it only for uniqueness, exact mutation guards, and
/// deterministic release tie-breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineOperationIdentity(u64);

impl TimelineOperationIdentity {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Lane-issued insertion ordering. It is distinct from due step and caller
/// operation identity so equal-deadline releases remain deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineInsertionSequence(u64);

impl TimelineInsertionSequence {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Exact mutation revision for one live operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineOperationRevision(u64);

impl TimelineOperationRevision {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Lane-issued completion ticket identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineCompletionTicketId(u64);

impl TimelineCompletionTicketId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// The only external completion source categories admitted by the timeline
/// lane. They are source facts, not host services or dispatch handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeSourceKind {
    Filesystem,
    Network,
    Inference,
    External,
}

impl RuntimeSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Filesystem => "filesystem",
            Self::Network => "network",
            Self::Inference => "inference",
            Self::External => "external",
        }
    }
}

/// Bounded opaque JSON carried as data across the runtime boundary.
///
/// This value is intentionally semantic-neutral. The runtime only bounds its
/// JSON shape and bytes; product-owned meanings such as paths, URLs, tokens,
/// or credentials are not interpreted here. Hosts must still avoid placing
/// secrets in a product result contract.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOpaqueData(Value);

impl RuntimeOpaqueData {
    pub fn new(value: Value) -> Result<Self, RuntimeTimelineDataError> {
        let bytes =
            serde_json::to_vec(&value).map_err(|_| RuntimeTimelineDataError::OpaqueDataNotJson)?;
        if bytes.len() > MAX_RUNTIME_OPAQUE_DATA_BYTES {
            return Err(RuntimeTimelineDataError::OpaqueDataTooLarge {
                actual: bytes.len(),
                maximum: MAX_RUNTIME_OPAQUE_DATA_BYTES,
            });
        }
        let mut nodes = 0usize;
        validate_opaque_value(&value, &mut nodes, 0)?;
        Ok(Self(value))
    }

    pub fn value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }

    pub(crate) fn validate(&self) -> Result<(), RuntimeTimelineDataError> {
        // Snapshot candidates can be assembled from typed records rather than
        // this constructor. Re-run the same structural bound without assigning
        // any meaning to the product-owned JSON.
        Self::new(self.0.clone()).map(|_| ())
    }
}

/// Bounded runtime provenance and correlation data. This is descriptive data
/// only; it cannot resolve a capability or invoke a host owner.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeProvenance {
    correlation: String,
    detail: Option<RuntimeOpaqueData>,
}

impl RuntimeProvenance {
    pub fn new(
        correlation: impl Into<String>,
        detail: Option<RuntimeOpaqueData>,
    ) -> Result<Self, RuntimeTimelineDataError> {
        let correlation = correlation.into();
        validate_runtime_identity(&correlation)?;
        Ok(Self {
            correlation,
            detail,
        })
    }

    pub fn correlation(&self) -> &str {
        &self.correlation
    }

    pub fn detail(&self) -> Option<&RuntimeOpaqueData> {
        self.detail.as_ref()
    }

    pub(crate) fn validate(&self) -> Result<(), RuntimeTimelineDataError> {
        validate_runtime_identity(&self.correlation)?;
        if let Some(detail) = &self.detail {
            detail.validate()?;
        }
        Ok(())
    }
}

/// A finite recurrence declaration. `remaining` includes the first release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineRecurrence {
    Once,
    Every { interval_steps: u64, remaining: u32 },
}

impl TimelineRecurrence {
    pub fn validate(self) -> Result<(), RuntimeTimelineDataError> {
        match self {
            Self::Once => Ok(()),
            Self::Every {
                interval_steps,
                remaining,
            } => {
                if interval_steps == 0 {
                    return Err(RuntimeTimelineDataError::ZeroRecurrenceInterval);
                }
                if remaining == 0 || remaining > MAX_RECURRENCE_OCCURRENCES {
                    return Err(RuntimeTimelineDataError::InvalidRecurrenceCount {
                        received: remaining,
                        maximum: MAX_RECURRENCE_OCCURRENCES,
                    });
                }
                Ok(())
            }
        }
    }

    pub const fn remaining(self) -> Option<u32> {
        match self {
            Self::Once => None,
            Self::Every { remaining, .. } => Some(remaining),
        }
    }
}

/// Caller description used to enqueue a timeline operation.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineOperationSpec {
    timeline_id: String,
    step_id: String,
    operation_id: TimelineOperationIdentity,
    due_step: SimulationStep,
    recurrence: TimelineRecurrence,
    provenance: RuntimeProvenance,
}

impl TimelineOperationSpec {
    pub fn new(
        timeline_id: impl Into<String>,
        step_id: impl Into<String>,
        operation_id: TimelineOperationIdentity,
        due_step: SimulationStep,
        recurrence: TimelineRecurrence,
        provenance: RuntimeProvenance,
    ) -> Result<Self, RuntimeTimelineDataError> {
        let timeline_id = timeline_id.into();
        let step_id = step_id.into();
        if timeline_id.is_empty() || step_id.is_empty() {
            return Err(RuntimeTimelineDataError::EmptyIdentity);
        }
        recurrence.validate()?;
        Ok(Self {
            timeline_id,
            step_id,
            operation_id,
            due_step,
            recurrence,
            provenance,
        })
    }

    pub fn timeline_id(&self) -> &str {
        &self.timeline_id
    }

    pub fn step_id(&self) -> &str {
        &self.step_id
    }

    pub const fn operation_id(&self) -> TimelineOperationIdentity {
        self.operation_id
    }

    pub const fn due_step(&self) -> SimulationStep {
        self.due_step
    }

    pub const fn recurrence(&self) -> TimelineRecurrence {
        self.recurrence
    }

    pub fn provenance(&self) -> &RuntimeProvenance {
        &self.provenance
    }
}

/// Full replacement candidate for a live operation. The caller cannot alter
/// its lane-issued insertion sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineOperationReplacement {
    timeline_id: String,
    step_id: String,
    due_step: SimulationStep,
    recurrence: TimelineRecurrence,
    provenance: RuntimeProvenance,
}

impl TimelineOperationReplacement {
    pub fn new(
        timeline_id: impl Into<String>,
        step_id: impl Into<String>,
        due_step: SimulationStep,
        recurrence: TimelineRecurrence,
        provenance: RuntimeProvenance,
    ) -> Result<Self, RuntimeTimelineDataError> {
        let timeline_id = timeline_id.into();
        let step_id = step_id.into();
        if timeline_id.is_empty() || step_id.is_empty() {
            return Err(RuntimeTimelineDataError::EmptyIdentity);
        }
        recurrence.validate()?;
        Ok(Self {
            timeline_id,
            step_id,
            due_step,
            recurrence,
            provenance,
        })
    }

    pub fn timeline_id(&self) -> &str {
        &self.timeline_id
    }

    pub fn step_id(&self) -> &str {
        &self.step_id
    }

    pub const fn due_step(&self) -> SimulationStep {
        self.due_step
    }

    pub const fn recurrence(&self) -> TimelineRecurrence {
        self.recurrence
    }

    pub fn provenance(&self) -> &RuntimeProvenance {
        &self.provenance
    }
}

/// Completion outcome data. Failure closes a completion-order gap just as a
/// successful completion does; the later mutation owner decides its meaning.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineCompletionOutcome {
    Success(Option<RuntimeOpaqueData>),
    Failure(Option<RuntimeOpaqueData>),
}

impl TimelineCompletionOutcome {
    pub const fn is_failure(&self) -> bool {
        matches!(self, Self::Failure(_))
    }

    pub(crate) fn validate(&self) -> Result<(), RuntimeTimelineDataError> {
        let data = match self {
            Self::Success(data) | Self::Failure(data) => data,
        };
        if let Some(data) = data {
            data.validate()?;
        }
        Ok(())
    }
}

/// A typed completion envelope returned by external work. It carries only the
/// ticket/correlation binding and data outcome; no capability can be supplied.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineCompletionEnvelope {
    ticket: TimelineCompletionTicketId,
    binding: RuntimeTimelineBinding,
    correlation: String,
    outcome: TimelineCompletionOutcome,
    provenance: RuntimeProvenance,
}

impl TimelineCompletionEnvelope {
    pub fn new(
        ticket: TimelineCompletionTicketId,
        binding: RuntimeTimelineBinding,
        correlation: impl Into<String>,
        outcome: TimelineCompletionOutcome,
        provenance: RuntimeProvenance,
    ) -> Result<Self, RuntimeTimelineDataError> {
        let correlation = correlation.into();
        validate_runtime_identity(&correlation)?;
        Ok(Self {
            ticket,
            binding,
            correlation,
            outcome,
            provenance,
        })
    }

    pub const fn ticket(&self) -> TimelineCompletionTicketId {
        self.ticket
    }

    pub const fn binding(&self) -> RuntimeTimelineBinding {
        self.binding
    }

    pub fn correlation(&self) -> &str {
        &self.correlation
    }

    pub fn outcome(&self) -> &TimelineCompletionOutcome {
        &self.outcome
    }

    pub fn provenance(&self) -> &RuntimeProvenance {
        &self.provenance
    }
}

/// Errors from constructing bounded caller-supplied timeline data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTimelineDataError {
    EmptyIdentity,
    InvalidIdentity,
    TextTooLarge { maximum: usize },
    OpaqueDataNotJson,
    OpaqueDataTooLarge { actual: usize, maximum: usize },
    OpaqueDataTooDeep,
    OpaqueDataTooManyNodes,
    ZeroRecurrenceInterval,
    InvalidRecurrenceCount { received: u32, maximum: u32 },
}

impl fmt::Display for RuntimeTimelineDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid runtime timeline data: {self:?}")
    }
}

impl std::error::Error for RuntimeTimelineDataError {}

pub(crate) fn validate_runtime_identity(value: &str) -> Result<(), RuntimeTimelineDataError> {
    if value.len() > MAX_RUNTIME_CORRELATION_BYTES {
        return Err(RuntimeTimelineDataError::TextTooLarge {
            maximum: MAX_RUNTIME_CORRELATION_BYTES,
        });
    }
    if product_model::validate_product_identity(value).is_err() {
        return Err(RuntimeTimelineDataError::InvalidIdentity);
    }
    Ok(())
}

fn validate_opaque_value(
    value: &Value,
    nodes: &mut usize,
    depth: usize,
) -> Result<(), RuntimeTimelineDataError> {
    if depth > product_model::MAX_OPAQUE_JSON_DEPTH {
        return Err(RuntimeTimelineDataError::OpaqueDataTooDeep);
    }
    *nodes = (*nodes)
        .checked_add(1)
        .ok_or(RuntimeTimelineDataError::OpaqueDataTooManyNodes)?;
    if *nodes > 256 {
        return Err(RuntimeTimelineDataError::OpaqueDataTooManyNodes);
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        Value::String(_) => Ok(()),
        Value::Array(values) => {
            if values.len() > 128 {
                return Err(RuntimeTimelineDataError::OpaqueDataTooManyNodes);
            }
            let child_depth = depth
                .checked_add(1)
                .ok_or(RuntimeTimelineDataError::OpaqueDataTooDeep)?;
            for value in values {
                validate_opaque_value(value, nodes, child_depth)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            if values.len() > 128 {
                return Err(RuntimeTimelineDataError::OpaqueDataTooManyNodes);
            }
            let child_depth = depth
                .checked_add(1)
                .ok_or(RuntimeTimelineDataError::OpaqueDataTooDeep)?;
            for (_key, value) in values {
                validate_opaque_value(value, nodes, child_depth)?;
            }
            Ok(())
        }
    }
}
