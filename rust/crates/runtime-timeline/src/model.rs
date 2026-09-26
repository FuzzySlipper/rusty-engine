use std::fmt;

use runtime_lifecycle::{
    validate_runtime_identity as validate_neutral_identity, RuntimeControlRevision,
    RuntimeGeneration, RuntimeInstanceId,
};
use serde_json::Value;

/// Maximum UTF-8 bytes in a completion correlation label.
pub const MAX_RUNTIME_CORRELATION_BYTES: usize = runtime_lifecycle::MAX_RUNTIME_IDENTITY_BYTES;

/// Identity and lifecycle epoch carried by a completion.
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

/// Product-issued completion ticket identity.
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

/// Opaque JSON carried as data across the runtime boundary.
///
/// This value is intentionally semantic-neutral. Product-owned meanings such
/// as paths, URLs, tokens, or credentials are not interpreted here. Hosts must
/// still avoid placing secrets in a product result contract.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOpaqueData(Value);

impl RuntimeOpaqueData {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub fn value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }
}

/// Bounded runtime provenance and correlation data. This is descriptive data
/// only; it cannot resolve or invoke an operation or host owner.
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
}

/// Completion outcome data whose meaning belongs to the product.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineCompletionOutcome {
    Success(Option<RuntimeOpaqueData>),
    Failure(Option<RuntimeOpaqueData>),
}

impl TimelineCompletionOutcome {
    pub const fn is_failure(&self) -> bool {
        matches!(self, Self::Failure(_))
    }
}

/// A typed completion envelope returned by external work. It carries only the
/// ticket/correlation binding and data outcome; no operation can be supplied.
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
    InvalidIdentity,
    TextTooLarge { maximum: usize },
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
    if validate_neutral_identity(value).is_err() {
        return Err(RuntimeTimelineDataError::InvalidIdentity);
    }
    Ok(())
}
