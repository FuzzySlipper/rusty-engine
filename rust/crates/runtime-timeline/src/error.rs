use std::fmt;

use runtime_lifecycle::{
    RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId, RuntimePhase, RuntimeState,
    SimulationStep,
};

use crate::{TimelineCompletionTicketId, TimelineOperationIdentity, TimelineOperationRevision};

/// Rejections from the static timeline descriptor catalog and instance-owned
/// lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTimelineError {
    BoundsExceeded(&'static str),
    UnknownTimeline(String),
    UnknownStep {
        timeline: String,
        step: String,
    },
    DuplicateTimeline(String),
    InspectionEncode(String),
    LifecycleNotRunning,
    LifecycleFaulted,
    LifecycleShutdown,
    LifecycleValidation,
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
    Disposed,
    StepRegression {
        expected_next: u64,
        received: SimulationStep,
    },
    DueStepBeforeCurrent {
        current: SimulationStep,
        due: SimulationStep,
    },
    ReleaseLimitInvalid,
    ReleaseBacklogExceeded,
    OperationIdentityInUse(TimelineOperationIdentity),
    OperationNotFound(TimelineOperationIdentity),
    OperationReceiptMismatch {
        operation: TimelineOperationIdentity,
        expected: TimelineOperationRevision,
        received: TimelineOperationRevision,
    },
    CounterExhausted(&'static str),
    TicketIdentityInUse(TimelineCompletionTicketId),
    TicketNotFound(TimelineCompletionTicketId),
    TicketAlreadyCompleted(TimelineCompletionTicketId),
    TicketBindingMismatch(TimelineCompletionTicketId),
    TicketCorrelationMismatch(TimelineCompletionTicketId),
    TicketProvenanceMismatch(TimelineCompletionTicketId),
    TicketCancelled(TimelineCompletionTicketId),
    TicketStaleRevision(TimelineCompletionTicketId),
    SnapshotTooLarge,
    SnapshotBindingMismatch,
    SnapshotDuplicateOperation(TimelineOperationIdentity),
    SnapshotDuplicateTicket(TimelineCompletionTicketId),
    SnapshotBoundOperationMissing(TimelineOperationIdentity),
    SnapshotBoundOperationRevisionMismatch {
        operation: TimelineOperationIdentity,
        ticket: TimelineOperationRevision,
        operation_snapshot: TimelineOperationRevision,
    },
    SnapshotBoundOperationDescriptorMismatch(TimelineCompletionTicketId),
    SnapshotUnsortedOperations,
    SnapshotUnsortedTickets,
    SnapshotCursorInvalid(&'static str),
    SnapshotInvariant(&'static str),
    ActiveRelease,
    RebindActiveRelease,
    RebindForeignInstance,
    RebindRegression,
    RebindNotRunning,
    RebindClearedGeneration {
        generation: RuntimeGeneration,
        operations: usize,
        tickets: usize,
    },
    RebindInvalidatedTickets(usize),
    WrongLifecycleState {
        state: RuntimeState,
    },
}

impl fmt::Display for RuntimeTimelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime timeline rejected operation: {self:?}")
    }
}

impl std::error::Error for RuntimeTimelineError {}
