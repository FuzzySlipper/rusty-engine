//! Host-neutral completion data transported to the C# product callback.
//!
//! Scheduling and ticket ownership belong to the product. This module does not
//! retain a queue, invoke work, or own a clock or product state.

use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use serde_json::Value;

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
/// The product chose the correlation when it issued the ticket and routes the
/// result by it; the optional provenance detail is descriptive data only.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineCompletionEnvelope {
    ticket: TimelineCompletionTicketId,
    binding: RuntimeTimelineBinding,
    correlation: String,
    outcome: TimelineCompletionOutcome,
    provenance: Option<RuntimeOpaqueData>,
}

impl TimelineCompletionEnvelope {
    pub fn new(
        ticket: TimelineCompletionTicketId,
        binding: RuntimeTimelineBinding,
        correlation: impl Into<String>,
        outcome: TimelineCompletionOutcome,
        provenance: Option<RuntimeOpaqueData>,
    ) -> Self {
        Self {
            ticket,
            binding,
            correlation: correlation.into(),
            outcome,
            provenance,
        }
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

    pub fn provenance(&self) -> Option<&RuntimeOpaqueData> {
        self.provenance.as_ref()
    }
}
