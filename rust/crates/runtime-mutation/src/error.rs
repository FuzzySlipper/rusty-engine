use std::fmt;

use runtime_lifecycle::{
    RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId, RuntimePhase, SimulationStep,
};

use crate::{MutationCatalogIdentity, MutationDataError, MutationFingerprint, MutationOperationId};

/// Rejections from static mutation linkage and one instance-owned mutation
/// lane. The generic planner error preserves actionable named-owner detail
/// without turning the Engine into a universal error/event bus.
#[derive(Debug)]
pub enum RuntimeMutationError<E> {
    BoundsExceeded(&'static str),
    EmptyCatalog,
    InvalidDescriptor {
        index: usize,
        field: &'static str,
    },
    DuplicateDescriptorBinding(String),
    DuplicateDescriptorTarget(String),
    UnknownBinding(String),
    BindingTargetMismatch {
        binding: String,
        expected: String,
        received: String,
    },
    CapabilityUnavailable {
        target: String,
    },
    CapabilityKindMismatch {
        target: String,
        expected: &'static str,
        received: String,
    },
    MultiplePublicationDomains {
        expected: String,
        received: String,
    },
    DomainMismatch {
        expected: String,
        received: String,
    },
    InvalidBatch(MutationDataError),
    EmptyBatch,
    DuplicateOperationId(MutationOperationId),
    UnknownOperationBinding {
        binding: String,
        target: String,
    },
    OperationTargetMismatch {
        binding: String,
        expected: String,
        received: String,
    },
    OperationPayloadTooLarge {
        operation: MutationOperationId,
        actual: usize,
        maximum: usize,
    },
    InvalidOperationPayload {
        operation: MutationOperationId,
        error: MutationDataError,
    },
    OwnerEvidenceCount {
        expected: usize,
        received: usize,
    },
    OwnerEvidenceMismatch {
        index: usize,
        reason: &'static str,
    },
    Lifecycle(runtime_lifecycle::RuntimeLifecycleError),
    LifecycleNotRunning,
    WrongPhase {
        expected: RuntimePhase,
        received: RuntimePhase,
    },
    ForeignInstance {
        expected: RuntimeInstanceId,
        received: RuntimeInstanceId,
    },
    StaleBinding {
        expected_generation: RuntimeGeneration,
        expected_control_revision: RuntimeControlRevision,
        received_generation: RuntimeGeneration,
        received_control_revision: RuntimeControlRevision,
    },
    BindingRegression,
    AlreadyAdvanced {
        admitted_steps: u64,
    },
    StepOutOfOrder {
        expected: Option<u64>,
        received: SimulationStep,
    },
    BatchIdentityConflict {
        step: SimulationStep,
        batch_id: String,
        fingerprint: MutationFingerprint,
        catalog_identity: MutationCatalogIdentity,
    },
    BatchAlreadyApplied {
        batch_id: String,
        applied_step: SimulationStep,
        received_step: SimulationStep,
    },
    AuthorityGuardChanged,
    Planner(E),
    Disposed,
    RebindForeignInstance,
    RebindRegression,
    RebindNotRunning,
    RebindAdmissionRegression {
        expected_next_step: Option<u64>,
        admitted_steps: u64,
    },
    ReceiptEvictionOverflow,
    InvalidatedAdmissionOverflow,
}

impl<E: fmt::Debug> fmt::Display for RuntimeMutationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime mutation rejected operation: {self:?}")
    }
}

impl<E: fmt::Debug + 'static> std::error::Error for RuntimeMutationError<E> {}
