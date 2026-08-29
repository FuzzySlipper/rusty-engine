use std::cmp::Ordering;

use runtime_lifecycle::{
    RuntimeLifecycle, RuntimePhase, RuntimePhaseToken, RuntimeState, SimulationStep,
};

use crate::{
    CompiledTimelineCatalog, CompiledTimelineStep, RuntimeProvenance, RuntimeSourceKind,
    RuntimeTimelineError, RuntimeTimelineInspection, TimelineCompletionEnvelope,
    TimelineCompletionOutcome, TimelineCompletionTicketId, TimelineInsertionSequence,
    TimelineOperationIdentity, TimelineOperationReplacement, TimelineOperationRevision,
    TimelineOperationSpec, TimelineRecurrence, MAX_TIMELINE_COMPLETION_TICKETS,
    MAX_TIMELINE_OPERATIONS, MAX_TIMELINE_RELEASE_PREFIX, MAX_TIMELINE_SNAPSHOT_ITEMS,
};

/// A receipt that identifies one exact live operation revision. Cancel and
/// replace require the receipt returned by the latest successful mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineOperationReceipt {
    binding: crate::RuntimeTimelineBinding,
    operation_id: TimelineOperationIdentity,
    insertion_sequence: TimelineInsertionSequence,
    revision: TimelineOperationRevision,
}

impl TimelineOperationReceipt {
    pub const fn binding(self) -> crate::RuntimeTimelineBinding {
        self.binding
    }

    pub const fn operation_id(self) -> TimelineOperationIdentity {
        self.operation_id
    }

    pub const fn insertion_sequence(self) -> TimelineInsertionSequence {
        self.insertion_sequence
    }

    pub const fn revision(self) -> TimelineOperationRevision {
        self.revision
    }
}

/// One lane-issued completion ticket bound to a compiled timeline step.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineCompletionTicket {
    id: TimelineCompletionTicketId,
    issue_sequence: TimelineInsertionSequence,
    binding: crate::RuntimeTimelineBinding,
    operation_id: TimelineOperationIdentity,
    operation_revision: TimelineOperationRevision,
    operation_bound: bool,
    timeline_id: String,
    step_id: String,
    capability_target: String,
    capability_kind: String,
    source: RuntimeSourceKind,
    correlation: String,
    result_contract: String,
    provenance: RuntimeProvenance,
}

impl TimelineCompletionTicket {
    /// Constructs a typed snapshot candidate. The lane validates the exact
    /// ticket-to-operation/template relationship during snapshot restore.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: TimelineCompletionTicketId,
        issue_sequence: TimelineInsertionSequence,
        binding: crate::RuntimeTimelineBinding,
        operation_id: TimelineOperationIdentity,
        operation_revision: TimelineOperationRevision,
        operation_bound: bool,
        timeline_id: impl Into<String>,
        step_id: impl Into<String>,
        capability_target: impl Into<String>,
        capability_kind: impl Into<String>,
        source: RuntimeSourceKind,
        correlation: impl Into<String>,
        result_contract: impl Into<String>,
        provenance: RuntimeProvenance,
    ) -> Self {
        Self {
            id,
            issue_sequence,
            binding,
            operation_id,
            operation_revision,
            operation_bound,
            timeline_id: timeline_id.into(),
            step_id: step_id.into(),
            capability_target: capability_target.into(),
            capability_kind: capability_kind.into(),
            source,
            correlation: correlation.into(),
            result_contract: result_contract.into(),
            provenance,
        }
    }

    pub const fn id(&self) -> TimelineCompletionTicketId {
        self.id
    }

    pub const fn issue_sequence(&self) -> TimelineInsertionSequence {
        self.issue_sequence
    }

    pub const fn binding(&self) -> crate::RuntimeTimelineBinding {
        self.binding
    }

    pub const fn operation_id(&self) -> TimelineOperationIdentity {
        self.operation_id
    }

    pub const fn operation_revision(&self) -> TimelineOperationRevision {
        self.operation_revision
    }

    pub const fn operation_bound(&self) -> bool {
        self.operation_bound
    }

    pub fn timeline_id(&self) -> &str {
        &self.timeline_id
    }

    pub fn step_id(&self) -> &str {
        &self.step_id
    }

    pub fn capability_target(&self) -> &str {
        &self.capability_target
    }

    pub fn capability_kind(&self) -> &str {
        &self.capability_kind
    }

    pub const fn source(&self) -> RuntimeSourceKind {
        self.source
    }

    pub fn correlation(&self) -> &str {
        &self.correlation
    }

    pub fn result_contract(&self) -> &str {
        &self.result_contract
    }

    pub fn provenance(&self) -> &RuntimeProvenance {
        &self.provenance
    }
}

/// Data-only result of admitting an exact completion envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineCompletionAdmission {
    ticket: TimelineCompletionTicketId,
    issue_sequence: TimelineInsertionSequence,
}

impl TimelineCompletionAdmission {
    pub const fn ticket(self) -> TimelineCompletionTicketId {
        self.ticket
    }

    pub const fn issue_sequence(self) -> TimelineInsertionSequence {
        self.issue_sequence
    }
}

/// Data-only receipt for a lifecycle rebind. Same-generation control changes
/// retain operations but invalidate old-revision completion tickets; a fresh
/// generation clears all live state and resets lane counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineRebindReceipt {
    binding: crate::RuntimeTimelineBinding,
    generation_reset: bool,
    retained_operations: usize,
    invalidated_tickets: usize,
    invalidated_admissions: u64,
    cleared_operations: usize,
    cleared_tickets: usize,
}

impl TimelineRebindReceipt {
    pub const fn binding(self) -> crate::RuntimeTimelineBinding {
        self.binding
    }

    pub const fn generation_reset(self) -> bool {
        self.generation_reset
    }

    pub const fn retained_operations(self) -> usize {
        self.retained_operations
    }

    pub const fn invalidated_tickets(self) -> usize {
        self.invalidated_tickets
    }

    pub const fn invalidated_admissions(self) -> u64 {
        self.invalidated_admissions
    }

    pub const fn cleared_operations(self) -> usize {
        self.cleared_operations
    }

    pub const fn cleared_tickets(self) -> usize {
        self.cleared_tickets
    }
}

/// Immutable operation release record. A release never invokes the compiled
/// capability; a later mutation owner consumes this data.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleasedTimelineOperation {
    operation_id: TimelineOperationIdentity,
    insertion_sequence: TimelineInsertionSequence,
    revision: TimelineOperationRevision,
    due_step: SimulationStep,
    step: CompiledTimelineStep,
    provenance: RuntimeProvenance,
}

impl ReleasedTimelineOperation {
    pub const fn operation_id(&self) -> TimelineOperationIdentity {
        self.operation_id
    }

    pub const fn insertion_sequence(&self) -> TimelineInsertionSequence {
        self.insertion_sequence
    }

    pub const fn revision(&self) -> TimelineOperationRevision {
        self.revision
    }

    pub const fn due_step(&self) -> SimulationStep {
        self.due_step
    }

    pub fn step(&self) -> &CompiledTimelineStep {
        &self.step
    }

    pub fn provenance(&self) -> &RuntimeProvenance {
        &self.provenance
    }
}

/// Completion release status. A cancelled ticket is data-only and closes an
/// issue-order gap without pretending that external work succeeded.
#[derive(Debug, Clone, PartialEq)]
pub enum ReleasedCompletionStatus {
    Completed(TimelineCompletionOutcome),
    Cancelled,
}

/// Immutable completion release record bound to its original compiled step.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleasedTimelineCompletion {
    ticket: TimelineCompletionTicket,
    step: CompiledTimelineStep,
    status: ReleasedCompletionStatus,
}

impl ReleasedTimelineCompletion {
    pub fn ticket(&self) -> &TimelineCompletionTicket {
        &self.ticket
    }

    pub fn step(&self) -> &CompiledTimelineStep {
        &self.step
    }

    pub fn status(&self) -> &ReleasedCompletionStatus {
        &self.status
    }
}

/// One bounded deterministic prefix emitted at a timeline phase boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineRelease {
    step: SimulationStep,
    events: Vec<ReleasedTimelineEvent>,
}

impl TimelineRelease {
    pub const fn step(&self) -> SimulationStep {
        self.step
    }

    pub fn events(&self) -> &[ReleasedTimelineEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<ReleasedTimelineEvent> {
        self.events
    }
}

/// The only values a timeline lane releases. Both variants are immutable data.
#[derive(Debug, Clone, PartialEq)]
pub enum ReleasedTimelineEvent {
    Operation(Box<ReleasedTimelineOperation>),
    Completion(Box<ReleasedTimelineCompletion>),
}

/// Bounded readout of live timeline mechanism state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTimelineReadout {
    binding: crate::RuntimeTimelineBinding,
    operation_count: usize,
    ticket_count: usize,
    next_insertion_sequence: u64,
    next_ticket_id: u64,
    last_release_step: Option<SimulationStep>,
    next_expected_step: u64,
    invalidated_ticket_count: u64,
    invalidated_admission_count: u64,
    disposed: bool,
}

impl RuntimeTimelineReadout {
    pub const fn binding(self) -> crate::RuntimeTimelineBinding {
        self.binding
    }

    pub const fn operation_count(self) -> usize {
        self.operation_count
    }

    pub const fn ticket_count(self) -> usize {
        self.ticket_count
    }

    pub const fn next_insertion_sequence(self) -> u64 {
        self.next_insertion_sequence
    }

    pub const fn next_ticket_id(self) -> u64 {
        self.next_ticket_id
    }

    pub const fn last_release_step(self) -> Option<SimulationStep> {
        self.last_release_step
    }

    pub const fn next_expected_step(self) -> u64 {
        self.next_expected_step
    }

    pub const fn invalidated_ticket_count(self) -> u64 {
        self.invalidated_ticket_count
    }

    pub const fn invalidated_admission_count(self) -> u64 {
        self.invalidated_admission_count
    }

    pub const fn is_disposed(self) -> bool {
        self.disposed
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LiveOperation {
    operation_id: TimelineOperationIdentity,
    insertion_sequence: TimelineInsertionSequence,
    revision: TimelineOperationRevision,
    timeline_id: String,
    step_id: String,
    due_step: SimulationStep,
    recurrence: TimelineRecurrence,
    provenance: RuntimeProvenance,
}

#[derive(Debug, Clone, PartialEq)]
enum LiveTicketStatus {
    Pending,
    Completed(TimelineCompletionOutcome),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
struct LiveTicket {
    ticket: TimelineCompletionTicket,
    status: LiveTicketStatus,
}

#[derive(Debug, Clone, PartialEq)]
struct TimelineState {
    next_insertion_sequence: u64,
    next_ticket_id: u64,
    operations: Vec<LiveOperation>,
    tickets: Vec<LiveTicket>,
    last_release_step: Option<SimulationStep>,
    next_expected_step: u64,
    invalidated_ticket_count: u64,
    invalidated_admission_count: u64,
}

impl TimelineState {
    const fn new() -> Self {
        Self {
            next_insertion_sequence: 0,
            next_ticket_id: 0,
            operations: Vec::new(),
            tickets: Vec::new(),
            last_release_step: None,
            next_expected_step: 0,
            invalidated_ticket_count: 0,
            invalidated_admission_count: 0,
        }
    }
}

/// One instance-owned timeline runtime lane. It resolves only compiled static
/// templates and emits immutable records; it stores no callback, executor,
/// host state, clock, live game state, or capability registry.
#[derive(Debug)]
pub struct RuntimeTimeline {
    catalog: CompiledTimelineCatalog,
    binding: crate::RuntimeTimelineBinding,
    state: TimelineState,
    disposed: bool,
    release_in_progress: bool,
}

impl RuntimeTimeline {
    pub(crate) fn bind(
        catalog: CompiledTimelineCatalog,
        lifecycle: &RuntimeLifecycle,
    ) -> Result<Self, RuntimeTimelineError> {
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeTimelineError::LifecycleNotRunning);
        }
        let admitted_steps = lifecycle.readout().admitted_simulation_steps();
        if admitted_steps != 0 {
            return Err(RuntimeTimelineError::AlreadyAdvanced { admitted_steps });
        }
        Ok(Self {
            catalog,
            binding: crate::RuntimeTimelineBinding::new(
                lifecycle.instance_id(),
                lifecycle.generation(),
                lifecycle.control_revision(),
            ),
            state: TimelineState::new(),
            disposed: false,
            release_in_progress: false,
        })
    }

    pub fn catalog(&self) -> &CompiledTimelineCatalog {
        &self.catalog
    }

    pub fn inspection(&self) -> &RuntimeTimelineInspection {
        self.catalog.inspection()
    }

    pub const fn binding(&self) -> crate::RuntimeTimelineBinding {
        self.binding
    }

    pub const fn is_disposed(&self) -> bool {
        self.disposed
    }

    pub fn readout(&self) -> RuntimeTimelineReadout {
        RuntimeTimelineReadout {
            binding: self.binding,
            operation_count: self.state.operations.len(),
            ticket_count: self.state.tickets.len(),
            next_insertion_sequence: self.state.next_insertion_sequence,
            next_ticket_id: self.state.next_ticket_id,
            last_release_step: self.state.last_release_step,
            next_expected_step: self.state.next_expected_step,
            invalidated_ticket_count: self.state.invalidated_ticket_count,
            invalidated_admission_count: self.state.invalidated_admission_count,
            disposed: self.disposed,
        }
    }

    /// Returns a fresh exact receipt for a retained live operation after a
    /// same-generation lifecycle rebind. The old receipt remains stale by
    /// design; callers explicitly reacquire this readout before mutation.
    pub fn operation_receipt(
        &self,
        operation_id: TimelineOperationIdentity,
    ) -> Result<TimelineOperationReceipt, RuntimeTimelineError> {
        let index = self.operation_index(operation_id)?;
        Ok(self.receipt(&self.state.operations[index]))
    }

    /// Clears the lane without mutating the compiled static catalog.
    pub fn dispose(&mut self) {
        self.state = TimelineState::new();
        self.disposed = true;
    }

    /// Enqueues an operation at a caller-supplied due step. The lane issues
    /// insertion ordering and validates the selected static template before
    /// publishing any queue change.
    pub fn schedule(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        spec: TimelineOperationSpec,
    ) -> Result<TimelineOperationReceipt, RuntimeTimelineError> {
        let current = self.validate_token(lifecycle, token)?;
        self.require_not_disposed()?;
        if spec.due_step() < current {
            return Err(RuntimeTimelineError::DueStepBeforeCurrent {
                current,
                due: spec.due_step(),
            });
        }
        self.schedule_inert(spec)
    }

    /// Atomically enqueues one bounded product-owned request batch at the
    /// current Timeline boundary. Existing state and every readout counter
    /// remain unchanged if a request duplicates a live/candidate identity or
    /// fails any static-template, due-step, or capacity validation.
    pub fn schedule_batch(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        specs: Vec<TimelineOperationSpec>,
    ) -> Result<Vec<TimelineOperationReceipt>, RuntimeTimelineError> {
        let current = self.validate_token(lifecycle, token)?;
        self.require_not_disposed()?;
        if specs.len() > MAX_TIMELINE_RELEASE_PREFIX {
            return Err(RuntimeTimelineError::BoundsExceeded("schedule batch"));
        }
        let original = self.state.clone();
        let result = specs
            .into_iter()
            .map(|spec| {
                if spec.due_step() < current {
                    return Err(RuntimeTimelineError::DueStepBeforeCurrent {
                        current,
                        due: spec.due_step(),
                    });
                }
                self.schedule_inert(spec)
            })
            .collect::<Result<Vec<_>, _>>();
        if result.is_err() {
            self.state = original;
        }
        result
    }

    /// Admits queue data without re-entering a lifecycle phase. The operation
    /// remains inert until [`Self::release_due`] validates an exact current
    /// Timeline token.
    fn schedule_inert(
        &mut self,
        spec: TimelineOperationSpec,
    ) -> Result<TimelineOperationReceipt, RuntimeTimelineError> {
        self.require_not_disposed()?;
        self.require_step(spec.timeline_id(), spec.step_id())?;
        if self
            .state
            .operations
            .iter()
            .any(|operation| operation.operation_id == spec.operation_id())
        {
            return Err(RuntimeTimelineError::OperationIdentityInUse(
                spec.operation_id(),
            ));
        }
        if self.state.operations.len() >= MAX_TIMELINE_OPERATIONS {
            return Err(RuntimeTimelineError::BoundsExceeded("live operations"));
        }
        let insertion_sequence = TimelineInsertionSequence::new(self.state.next_insertion_sequence);
        let next_insertion = self
            .state
            .next_insertion_sequence
            .checked_add(1)
            .ok_or(RuntimeTimelineError::CounterExhausted("insertion sequence"))?;
        self.state.next_insertion_sequence = next_insertion;
        self.state.operations.push(LiveOperation {
            operation_id: spec.operation_id(),
            insertion_sequence,
            revision: TimelineOperationRevision::ZERO,
            timeline_id: spec.timeline_id().to_owned(),
            step_id: spec.step_id().to_owned(),
            due_step: spec.due_step(),
            recurrence: spec.recurrence(),
            provenance: spec.provenance().clone(),
        });
        Ok(self.receipt(&self.state.operations[self.state.operations.len() - 1]))
    }

    /// Cancels one exact operation revision. All validation occurs before the
    /// operation is removed.
    pub fn cancel(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        receipt: TimelineOperationReceipt,
    ) -> Result<TimelineOperationReceipt, RuntimeTimelineError> {
        self.validate_token(lifecycle, token)?;
        self.cancel_inert(receipt)
    }

    /// Cancels one exact operation revision while the current Timeline phase
    /// is active. The exact receipt still guards the operation revision and
    /// current binding.
    fn cancel_inert(
        &mut self,
        receipt: TimelineOperationReceipt,
    ) -> Result<TimelineOperationReceipt, RuntimeTimelineError> {
        self.require_not_disposed()?;
        self.require_receipt(receipt)?;
        let index = self.operation_index(receipt.operation_id())?;
        let revision = self.state.operations[index].revision;
        self.cancel_bound_tickets(receipt.operation_id(), revision);
        self.state.operations.remove(index);
        Ok(receipt)
    }

    /// Replaces an operation's complete static step/cadence/provenance while
    /// preserving the original insertion sequence.
    pub fn replace(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        receipt: TimelineOperationReceipt,
        replacement: TimelineOperationReplacement,
    ) -> Result<TimelineOperationReceipt, RuntimeTimelineError> {
        let current = self.validate_token(lifecycle, token)?;
        self.require_not_disposed()?;
        if replacement.due_step() < current {
            return Err(RuntimeTimelineError::DueStepBeforeCurrent {
                current,
                due: replacement.due_step(),
            });
        }
        self.replace_inert(receipt, replacement)
    }

    /// Replaces one exact operation revision while the current Timeline phase
    /// is active. Validation is complete before the old operation is changed.
    fn replace_inert(
        &mut self,
        receipt: TimelineOperationReceipt,
        replacement: TimelineOperationReplacement,
    ) -> Result<TimelineOperationReceipt, RuntimeTimelineError> {
        self.require_not_disposed()?;
        self.require_step(replacement.timeline_id(), replacement.step_id())?;
        self.require_receipt(receipt)?;
        let index = self.operation_index(receipt.operation_id())?;
        let operation = self.state.operations[index].clone();
        let revision = operation
            .revision
            .value()
            .checked_add(1)
            .map(TimelineOperationRevision::new)
            .ok_or(RuntimeTimelineError::CounterExhausted("operation revision"))?;
        self.cancel_bound_tickets(receipt.operation_id(), operation.revision);
        self.state.operations[index] = LiveOperation {
            operation_id: operation.operation_id,
            insertion_sequence: operation.insertion_sequence,
            revision,
            timeline_id: replacement.timeline_id().to_owned(),
            step_id: replacement.step_id().to_owned(),
            due_step: replacement.due_step(),
            recurrence: replacement.recurrence(),
            provenance: replacement.provenance().clone(),
        };
        Ok(self.receipt(&self.state.operations[index]))
    }

    /// Registers a completion ticket before external work starts. The ticket
    /// is bound to the selected compiled step and current lifecycle revision.
    pub fn register_completion(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        spec: TimelineCompletionSpec,
    ) -> Result<TimelineCompletionTicket, RuntimeTimelineError> {
        self.validate_token(lifecycle, token)?;
        self.register_completion_inert(spec)
    }

    /// Registers a ticket during the current Timeline phase. External work may
    /// begin only after the returned ticket is retained by the caller; release
    /// still requires an exact current Timeline token.
    fn register_completion_inert(
        &mut self,
        spec: TimelineCompletionSpec,
    ) -> Result<TimelineCompletionTicket, RuntimeTimelineError> {
        self.require_not_disposed()?;
        self.require_step(&spec.timeline_id, &spec.step_id)?;
        let selected_step = self
            .catalog
            .step(&spec.timeline_id, &spec.step_id)
            .ok_or_else(|| RuntimeTimelineError::UnknownStep {
                timeline: spec.timeline_id.clone(),
                step: spec.step_id.clone(),
            })?;
        if self.state.tickets.len() >= MAX_TIMELINE_COMPLETION_TICKETS {
            return Err(RuntimeTimelineError::BoundsExceeded("completion tickets"));
        }
        let operation_bound = if let Some(operation) = self
            .state
            .operations
            .iter()
            .find(|operation| operation.operation_id == spec.operation_id)
        {
            if operation.revision != spec.operation_revision {
                return Err(RuntimeTimelineError::OperationReceiptMismatch {
                    operation: spec.operation_id,
                    expected: operation.revision,
                    received: spec.operation_revision,
                });
            }
            true
        } else {
            if spec.operation_revision != TimelineOperationRevision::ZERO {
                return Err(RuntimeTimelineError::OperationNotFound(spec.operation_id));
            }
            false
        };
        let id = TimelineCompletionTicketId::new(self.state.next_ticket_id);
        let next = self
            .state
            .next_ticket_id
            .checked_add(1)
            .ok_or(RuntimeTimelineError::CounterExhausted("completion ticket"))?;
        self.state.next_ticket_id = next;
        let ticket = TimelineCompletionTicket {
            id,
            issue_sequence: TimelineInsertionSequence::new(id.value()),
            binding: self.binding,
            operation_id: spec.operation_id,
            operation_revision: spec.operation_revision,
            operation_bound,
            timeline_id: spec.timeline_id,
            step_id: spec.step_id,
            capability_target: selected_step.capability().target().to_owned(),
            capability_kind: selected_step.capability().kind().to_owned(),
            source: spec.source,
            correlation: spec.correlation,
            result_contract: spec.result_contract,
            provenance: spec.provenance,
        };
        self.state.tickets.push(LiveTicket {
            ticket: ticket.clone(),
            status: LiveTicketStatus::Pending,
        });
        Ok(ticket)
    }

    /// Queues an exact ticket completion as inert data while the lifecycle is
    /// Running and bound to this lane. It never resolves a new capability and
    /// never releases an event by itself; release still requires a Timeline
    /// token. This queue-only path intentionally does not require that the
    /// caller retain a phase token while external work finishes.
    pub fn admit_completion(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        envelope: TimelineCompletionEnvelope,
    ) -> Result<TimelineCompletionAdmission, RuntimeTimelineError> {
        self.validate_queue_lifecycle(lifecycle)?;
        self.admit_completion_inert(envelope)
    }

    /// Admits completion data without re-entering a lifecycle phase. The
    /// completed ticket remains held until a later exact Timeline release.
    fn admit_completion_inert(
        &mut self,
        envelope: TimelineCompletionEnvelope,
    ) -> Result<TimelineCompletionAdmission, RuntimeTimelineError> {
        self.require_not_disposed()?;
        let index = self.ticket_index(envelope.ticket())?;
        let live = &self.state.tickets[index];
        if live.ticket.binding() != envelope.binding() {
            return Err(RuntimeTimelineError::TicketBindingMismatch(
                envelope.ticket(),
            ));
        }
        if live.ticket.binding() != self.binding {
            return Err(RuntimeTimelineError::TicketStaleRevision(envelope.ticket()));
        }
        if matches!(live.status, LiveTicketStatus::Completed(_)) {
            return Err(RuntimeTimelineError::TicketAlreadyCompleted(
                envelope.ticket(),
            ));
        }
        if matches!(live.status, LiveTicketStatus::Cancelled) {
            return Err(RuntimeTimelineError::TicketCancelled(envelope.ticket()));
        }
        if let Some(operation) = self
            .state
            .operations
            .iter()
            .find(|operation| operation.operation_id == live.ticket.operation_id())
        {
            if operation.revision != live.ticket.operation_revision() {
                return Err(RuntimeTimelineError::TicketStaleRevision(envelope.ticket()));
            }
        } else if live.ticket.operation_bound {
            return Err(RuntimeTimelineError::TicketStaleRevision(envelope.ticket()));
        }
        if live.ticket.correlation() != envelope.correlation() {
            return Err(RuntimeTimelineError::TicketCorrelationMismatch(
                envelope.ticket(),
            ));
        }
        if live.ticket.provenance() != envelope.provenance() {
            return Err(RuntimeTimelineError::TicketProvenanceMismatch(
                envelope.ticket(),
            ));
        }
        self.state.tickets[index].status = LiveTicketStatus::Completed(envelope.outcome().clone());
        Ok(TimelineCompletionAdmission {
            ticket: envelope.ticket(),
            issue_sequence: self.state.tickets[index].ticket.issue_sequence(),
        })
    }

    /// Cancels a pending completion ticket, closing its issue-order gap.
    pub fn cancel_completion(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        ticket: TimelineCompletionTicketId,
    ) -> Result<(), RuntimeTimelineError> {
        self.validate_token(lifecycle, token)?;
        self.cancel_completion_inert(ticket)
    }

    /// Cancels a ticket as inert data, closing its completion-order gap for a
    /// future exact Timeline release.
    fn cancel_completion_inert(
        &mut self,
        ticket: TimelineCompletionTicketId,
    ) -> Result<(), RuntimeTimelineError> {
        self.require_not_disposed()?;
        let index = self.ticket_index(ticket)?;
        if matches!(
            self.state.tickets[index].status,
            LiveTicketStatus::Completed(_)
        ) {
            return Err(RuntimeTimelineError::TicketAlreadyCompleted(ticket));
        }
        if matches!(
            self.state.tickets[index].status,
            LiveTicketStatus::Cancelled
        ) {
            return Err(RuntimeTimelineError::TicketCancelled(ticket));
        }
        self.state.tickets[index].status = LiveTicketStatus::Cancelled;
        Ok(())
    }

    /// Releases a bounded deterministic prefix for exactly the next
    /// simulation step. Operations sort by `(due, insertion, operation id)`;
    /// completions sort by ticket issue order and stop at the first pending
    /// ticket gap.
    pub fn release_due(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        maximum_events: usize,
    ) -> Result<TimelineRelease, RuntimeTimelineError> {
        let current = self.validate_token(lifecycle, token)?;
        self.require_not_disposed()?;
        if maximum_events == 0 || maximum_events > MAX_TIMELINE_RELEASE_PREFIX {
            return Err(RuntimeTimelineError::ReleaseLimitInvalid);
        }
        if self.release_in_progress {
            return Err(RuntimeTimelineError::ActiveRelease);
        }
        let expected = self.state.next_expected_step;
        if current.value() != expected {
            return Err(RuntimeTimelineError::StepRegression {
                expected_next: expected,
                received: current,
            });
        }
        current
            .value()
            .checked_add(1)
            .ok_or(RuntimeTimelineError::CounterExhausted("release step"))?;
        self.release_in_progress = true;
        let original_state = self.state.clone();
        let result = self.release_due_inner(current, maximum_events);
        if result.is_err() {
            self.state = original_state;
        }
        self.release_in_progress = false;
        result
    }

    /// Rebinds after a lifecycle control revision or new generation. The
    /// operation queue remains data-only across pause/resume; old completion
    /// tickets cannot cross the revision boundary.
    pub fn rebind(
        &mut self,
        lifecycle: &RuntimeLifecycle,
    ) -> Result<TimelineRebindReceipt, RuntimeTimelineError> {
        self.require_not_disposed()?;
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeTimelineError::RebindNotRunning);
        }
        if lifecycle.instance_id() != self.binding.instance_id() {
            return Err(RuntimeTimelineError::RebindForeignInstance);
        }
        if self.release_in_progress {
            return Err(RuntimeTimelineError::RebindActiveRelease);
        }
        let generation_changed = lifecycle.generation() != self.binding.generation();
        let revision_changed = lifecycle.control_revision() != self.binding.control_revision();
        if !generation_changed && !revision_changed {
            return Err(RuntimeTimelineError::RebindRegression);
        }
        if lifecycle.generation().value() < self.binding.generation().value()
            || (lifecycle.generation() == self.binding.generation()
                && lifecycle.control_revision().value() < self.binding.control_revision().value())
        {
            return Err(RuntimeTimelineError::BindingRegression);
        }
        let next_binding = crate::RuntimeTimelineBinding::new(
            lifecycle.instance_id(),
            lifecycle.generation(),
            lifecycle.control_revision(),
        );
        if generation_changed {
            let cleared_operations = self.state.operations.len();
            let cleared_tickets = self.state.tickets.len();
            self.state = TimelineState::new();
            self.binding = next_binding;
            return Ok(TimelineRebindReceipt {
                binding: self.binding,
                generation_reset: true,
                retained_operations: 0,
                invalidated_tickets: 0,
                invalidated_admissions: 0,
                cleared_operations,
                cleared_tickets,
            });
        }
        let admitted_steps = lifecycle.readout().admitted_simulation_steps();
        if admitted_steps < self.state.next_expected_step {
            return Err(RuntimeTimelineError::BindingRegression);
        }
        let invalidated_admissions = admitted_steps - self.state.next_expected_step;
        let next_invalidated_admission_count = self
            .state
            .invalidated_admission_count
            .checked_add(invalidated_admissions)
            .ok_or(RuntimeTimelineError::CounterExhausted(
                "invalidated admissions",
            ))?;
        if next_invalidated_admission_count > admitted_steps {
            return Err(RuntimeTimelineError::SnapshotCursorInvalid(
                "invalidated admissions",
            ));
        }
        let invalidated_tickets = self.state.tickets.len();
        let invalidated_tickets_u64 = u64::try_from(invalidated_tickets)
            .map_err(|_| RuntimeTimelineError::CounterExhausted("invalidated tickets"))?;
        let next_invalidated_ticket_count = self
            .state
            .invalidated_ticket_count
            .checked_add(invalidated_tickets_u64)
            .ok_or(RuntimeTimelineError::CounterExhausted(
                "invalidated tickets",
            ))?;
        self.state.tickets.clear();
        self.state.next_expected_step = admitted_steps;
        self.state.invalidated_ticket_count = next_invalidated_ticket_count;
        self.state.invalidated_admission_count = next_invalidated_admission_count;
        self.binding = next_binding;
        Ok(TimelineRebindReceipt {
            binding: self.binding,
            generation_reset: false,
            retained_operations: self.state.operations.len(),
            invalidated_tickets,
            invalidated_admissions,
            cleared_operations: 0,
            cleared_tickets: 0,
        })
    }

    /// Takes an instance-mechanism snapshot suitable for product-owned
    /// persistence. It does not encode product save meaning.
    pub fn snapshot(&self) -> TimelineSnapshot {
        let mut operations = self
            .state
            .operations
            .iter()
            .map(TimelineOperationSnapshot::from_live)
            .collect::<Vec<_>>();
        operations.sort_by(operation_snapshot_order);
        let mut tickets = self
            .state
            .tickets
            .iter()
            .map(TimelineTicketSnapshot::from_live)
            .collect::<Vec<_>>();
        tickets.sort_by_key(|ticket| ticket.ticket.issue_sequence.value());
        TimelineSnapshot {
            binding: self.binding,
            next_insertion_sequence: self.state.next_insertion_sequence,
            next_ticket_id: self.state.next_ticket_id,
            last_release_step: self.state.last_release_step,
            next_expected_step: self.state.next_expected_step,
            invalidated_ticket_count: self.state.invalidated_ticket_count,
            invalidated_admission_count: self.state.invalidated_admission_count,
            operations,
            tickets,
        }
    }

    /// Validates a complete snapshot into a temporary state before replacing
    /// this lane's live mechanism state.
    pub fn restore_snapshot(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        snapshot: TimelineSnapshot,
    ) -> Result<(), RuntimeTimelineError> {
        self.validate_queue_lifecycle(lifecycle)?;
        if snapshot.binding != self.binding {
            return Err(RuntimeTimelineError::SnapshotBindingMismatch);
        }
        let candidate = self.validate_snapshot(&snapshot, lifecycle)?;
        self.state = candidate;
        Ok(())
    }

    fn validate_snapshot(
        &self,
        snapshot: &TimelineSnapshot,
        lifecycle: &RuntimeLifecycle,
    ) -> Result<TimelineState, RuntimeTimelineError> {
        if snapshot.operations.len() > MAX_TIMELINE_SNAPSHOT_ITEMS
            || snapshot.operations.len() > MAX_TIMELINE_OPERATIONS
            || snapshot.tickets.len() > MAX_TIMELINE_SNAPSHOT_ITEMS
            || snapshot.tickets.len() > MAX_TIMELINE_COMPLETION_TICKETS
        {
            return Err(RuntimeTimelineError::SnapshotTooLarge);
        }
        if snapshot
            .operations
            .windows(2)
            .any(|window| operation_snapshot_order(&window[0], &window[1]) == Ordering::Greater)
        {
            return Err(RuntimeTimelineError::SnapshotUnsortedOperations);
        }
        if snapshot.tickets.windows(2).any(|window| {
            window[0].ticket.issue_sequence.value() >= window[1].ticket.issue_sequence.value()
        }) {
            return Err(RuntimeTimelineError::SnapshotUnsortedTickets);
        }
        let mut operation_ids = std::collections::BTreeSet::new();
        let mut insertion_ids = std::collections::BTreeSet::new();
        let mut operations = Vec::with_capacity(snapshot.operations.len());
        for operation in &snapshot.operations {
            if !operation_ids.insert(operation.operation_id) {
                return Err(RuntimeTimelineError::SnapshotDuplicateOperation(
                    operation.operation_id,
                ));
            }
            if !insertion_ids.insert(operation.insertion_sequence.value()) {
                return Err(RuntimeTimelineError::SnapshotInvariant(
                    "duplicate insertion sequence",
                ));
            }
            if operation.insertion_sequence.value() >= snapshot.next_insertion_sequence {
                return Err(RuntimeTimelineError::SnapshotCursorInvalid(
                    "insertion cursor",
                ));
            }
            operation.recurrence.validate().map_err(|_| {
                RuntimeTimelineError::SnapshotInvariant("invalid operation recurrence")
            })?;
            self.require_step(&operation.timeline_id, &operation.step_id)?;
            operations.push(operation.to_live());
        }
        let mut ticket_ids = std::collections::BTreeSet::new();
        let mut tickets = Vec::with_capacity(snapshot.tickets.len());
        for ticket in &snapshot.tickets {
            if !ticket_ids.insert(ticket.ticket.id()) {
                return Err(RuntimeTimelineError::SnapshotDuplicateTicket(
                    ticket.ticket.id(),
                ));
            }
            if ticket.ticket.id().value() >= snapshot.next_ticket_id {
                return Err(RuntimeTimelineError::SnapshotCursorInvalid("ticket cursor"));
            }
            if ticket.ticket.issue_sequence().value() != ticket.ticket.id().value() {
                return Err(RuntimeTimelineError::SnapshotCursorInvalid(
                    "ticket issue sequence",
                ));
            }
            if ticket.ticket.binding() != self.binding {
                return Err(RuntimeTimelineError::SnapshotInvariant("ticket binding"));
            }
            crate::model::validate_runtime_identity(ticket.ticket.correlation())
                .map_err(|_| RuntimeTimelineError::SnapshotInvariant("ticket correlation"))?;
            crate::model::validate_runtime_identity(ticket.ticket.result_contract())
                .map_err(|_| RuntimeTimelineError::SnapshotInvariant("result contract"))?;
            ticket
                .ticket
                .provenance()
                .validate()
                .map_err(|_| RuntimeTimelineError::SnapshotInvariant("ticket provenance"))?;
            let compiled = self
                .catalog
                .step(ticket.ticket.timeline_id(), ticket.ticket.step_id())
                .ok_or_else(|| RuntimeTimelineError::UnknownStep {
                    timeline: ticket.ticket.timeline_id().to_owned(),
                    step: ticket.ticket.step_id().to_owned(),
                })?;
            if compiled.capability().target() != ticket.ticket.capability_target()
                || compiled.capability().kind() != ticket.ticket.capability_kind()
            {
                return Err(
                    RuntimeTimelineError::SnapshotBoundOperationTemplateMismatch(
                        ticket.ticket.id(),
                    ),
                );
            }
            match ticket.status() {
                TimelineTicketSnapshotStatus::Completed(outcome) => outcome
                    .validate()
                    .map_err(|_| RuntimeTimelineError::SnapshotInvariant("ticket outcome"))?,
                TimelineTicketSnapshotStatus::Pending | TimelineTicketSnapshotStatus::Cancelled => {
                }
            }
            if ticket.ticket.operation_bound() {
                let Some(operation) = snapshot
                    .operations
                    .iter()
                    .find(|operation| operation.operation_id == ticket.ticket.operation_id())
                else {
                    return Err(RuntimeTimelineError::SnapshotBoundOperationMissing(
                        ticket.ticket.operation_id(),
                    ));
                };
                if operation.revision != ticket.ticket.operation_revision() {
                    return Err(
                        RuntimeTimelineError::SnapshotBoundOperationRevisionMismatch {
                            operation: ticket.ticket.operation_id(),
                            ticket: ticket.ticket.operation_revision(),
                            operation_snapshot: operation.revision,
                        },
                    );
                }
                if operation.timeline_id != ticket.ticket.timeline_id()
                    || operation.step_id != ticket.ticket.step_id()
                {
                    return Err(
                        RuntimeTimelineError::SnapshotBoundOperationTemplateMismatch(
                            ticket.ticket.id(),
                        ),
                    );
                }
            } else if ticket.ticket.operation_revision() != TimelineOperationRevision::ZERO {
                return Err(RuntimeTimelineError::SnapshotInvariant(
                    "unbound ticket revision",
                ));
            }
            tickets.push(ticket.to_live());
        }
        let admitted_steps = lifecycle.readout().admitted_simulation_steps();
        if snapshot.next_expected_step > admitted_steps
            || snapshot.invalidated_admission_count > admitted_steps
        {
            return Err(RuntimeTimelineError::SnapshotCursorInvalid(
                "admission cursor",
            ));
        }
        if let Some(last) = snapshot.last_release_step {
            if last.value() >= admitted_steps {
                return Err(RuntimeTimelineError::SnapshotCursorInvalid(
                    "release cursor",
                ));
            }
            let expected =
                last.value()
                    .checked_add(1)
                    .ok_or(RuntimeTimelineError::SnapshotCursorInvalid(
                        "release cursor",
                    ))?;
            if expected > snapshot.next_expected_step {
                return Err(RuntimeTimelineError::SnapshotCursorInvalid(
                    "release cursor",
                ));
            }
        }
        let released_frontier = snapshot
            .last_release_step
            .and_then(|step| step.value().checked_add(1))
            .unwrap_or(0);
        let invalidated_gap = snapshot
            .next_expected_step
            .checked_sub(released_frontier)
            .ok_or(RuntimeTimelineError::SnapshotCursorInvalid(
                "admission cursor",
            ))?;
        if snapshot.invalidated_admission_count < invalidated_gap {
            return Err(RuntimeTimelineError::SnapshotInvariant(
                "admission invalidation count",
            ));
        }
        Ok(TimelineState {
            next_insertion_sequence: snapshot.next_insertion_sequence,
            next_ticket_id: snapshot.next_ticket_id,
            operations,
            tickets,
            last_release_step: snapshot.last_release_step,
            next_expected_step: snapshot.next_expected_step,
            invalidated_ticket_count: snapshot.invalidated_ticket_count,
            invalidated_admission_count: snapshot.invalidated_admission_count,
        })
    }

    fn release_due_inner(
        &mut self,
        current: SimulationStep,
        maximum_events: usize,
    ) -> Result<TimelineRelease, RuntimeTimelineError> {
        let mut events = Vec::new();
        let mut operation_indices = self
            .state
            .operations
            .iter()
            .enumerate()
            .filter(|(_, operation)| operation.due_step <= current)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        operation_indices.sort_by(|left, right| {
            operation_order(
                &self.state.operations[*left],
                &self.state.operations[*right],
            )
        });
        let selected = operation_indices
            .into_iter()
            .take(maximum_events)
            .collect::<Vec<_>>();
        // Preflight every recurrence transition before removing or updating
        // any operation. The outer method also leaves the original lane
        // untouched if this method returns an error.
        for index in &selected {
            if let TimelineRecurrence::Every {
                interval_steps,
                remaining,
            } = self.state.operations[*index].recurrence
            {
                if remaining > 1 {
                    self.state.operations[*index]
                        .due_step
                        .value()
                        .checked_add(interval_steps)
                        .ok_or(RuntimeTimelineError::CounterExhausted(
                            "recurrence due step",
                        ))?;
                }
            }
        }
        let mut remove = Vec::new();
        for index in selected {
            let operation = self.state.operations[index].clone();
            let step = self
                .catalog
                .step(&operation.timeline_id, &operation.step_id)
                .ok_or_else(|| RuntimeTimelineError::UnknownStep {
                    timeline: operation.timeline_id.clone(),
                    step: operation.step_id.clone(),
                })?
                .clone();
            self.cancel_bound_tickets(operation.operation_id, operation.revision);
            events.push(ReleasedTimelineEvent::Operation(Box::new(
                ReleasedTimelineOperation {
                    operation_id: operation.operation_id,
                    insertion_sequence: operation.insertion_sequence,
                    revision: operation.revision,
                    due_step: operation.due_step,
                    step,
                    provenance: operation.provenance,
                },
            )));
            match operation.recurrence {
                TimelineRecurrence::Once => remove.push(index),
                TimelineRecurrence::Every { remaining, .. } if remaining <= 1 => remove.push(index),
                TimelineRecurrence::Every {
                    interval_steps,
                    remaining,
                } => {
                    let next_due = operation
                        .due_step
                        .value()
                        .checked_add(interval_steps)
                        .ok_or(RuntimeTimelineError::CounterExhausted(
                            "recurrence due step",
                        ))?;
                    let next_revision = operation
                        .revision
                        .value()
                        .checked_add(1)
                        .map(TimelineOperationRevision::new)
                        .ok_or(RuntimeTimelineError::CounterExhausted("operation revision"))?;
                    self.state.operations[index].due_step = SimulationStep::new(next_due);
                    self.state.operations[index].recurrence = TimelineRecurrence::Every {
                        interval_steps,
                        remaining: remaining - 1,
                    };
                    self.state.operations[index].revision = next_revision;
                }
            }
        }
        remove.sort_unstable();
        for index in remove.into_iter().rev() {
            self.state.operations.remove(index);
        }
        if events.len() < maximum_events {
            let available = self
                .state
                .tickets
                .iter()
                .take_while(|ticket| {
                    matches!(
                        ticket.status,
                        LiveTicketStatus::Completed(_) | LiveTicketStatus::Cancelled
                    )
                })
                .map(|ticket| ticket.ticket.id())
                .collect::<Vec<_>>();
            for ticket_id in available.into_iter().take(maximum_events - events.len()) {
                let index = self.ticket_index(ticket_id)?;
                let live = self.state.tickets.remove(index);
                let step = self
                    .catalog
                    .step(live.ticket.timeline_id(), live.ticket.step_id())
                    .ok_or_else(|| RuntimeTimelineError::UnknownStep {
                        timeline: live.ticket.timeline_id().to_owned(),
                        step: live.ticket.step_id().to_owned(),
                    })?
                    .clone();
                let status = match live.status {
                    LiveTicketStatus::Completed(outcome) => {
                        ReleasedCompletionStatus::Completed(outcome)
                    }
                    LiveTicketStatus::Cancelled => ReleasedCompletionStatus::Cancelled,
                    LiveTicketStatus::Pending => unreachable!("pending ticket selected by prefix"),
                };
                events.push(ReleasedTimelineEvent::Completion(Box::new(
                    ReleasedTimelineCompletion {
                        ticket: live.ticket,
                        step,
                        status,
                    },
                )));
            }
        }
        self.state.last_release_step = Some(current);
        self.state.next_expected_step = current
            .value()
            .checked_add(1)
            .ok_or(RuntimeTimelineError::CounterExhausted("release step"))?;
        Ok(TimelineRelease {
            step: current,
            events,
        })
    }

    fn validate_token(
        &self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
    ) -> Result<SimulationStep, RuntimeTimelineError> {
        if token.phase() != RuntimePhase::Timeline {
            return Err(RuntimeTimelineError::WrongPhase {
                expected: RuntimePhase::Timeline,
                received: token.phase(),
            });
        }
        match lifecycle.state() {
            RuntimeState::Running => {}
            RuntimeState::Faulted => return Err(RuntimeTimelineError::LifecycleFaulted),
            RuntimeState::Shutdown => return Err(RuntimeTimelineError::LifecycleShutdown),
            state => return Err(RuntimeTimelineError::WrongLifecycleState { state }),
        }
        lifecycle
            .validate_phase_token(token, RuntimePhase::Timeline)
            .map_err(|_| RuntimeTimelineError::LifecycleValidation)?;
        let simulation = token.simulation();
        if simulation.instance_id() != self.binding.instance_id() {
            return Err(RuntimeTimelineError::ForeignInstance {
                expected: self.binding.instance_id(),
                received: simulation.instance_id(),
            });
        }
        if simulation.generation() != self.binding.generation()
            || simulation.control_revision() != self.binding.control_revision()
        {
            return Err(RuntimeTimelineError::StaleBinding {
                expected_generation: self.binding.generation(),
                expected_control_revision: self.binding.control_revision(),
                received_generation: simulation.generation(),
                received_control_revision: simulation.control_revision(),
            });
        }
        Ok(simulation.step())
    }

    fn validate_queue_lifecycle(
        &self,
        lifecycle: &RuntimeLifecycle,
    ) -> Result<(), RuntimeTimelineError> {
        self.require_not_disposed()?;
        match lifecycle.state() {
            RuntimeState::Running => {}
            RuntimeState::Faulted => return Err(RuntimeTimelineError::LifecycleFaulted),
            RuntimeState::Shutdown => return Err(RuntimeTimelineError::LifecycleShutdown),
            _ => return Err(RuntimeTimelineError::LifecycleNotRunning),
        }
        if lifecycle.instance_id() != self.binding.instance_id() {
            return Err(RuntimeTimelineError::ForeignInstance {
                expected: self.binding.instance_id(),
                received: lifecycle.instance_id(),
            });
        }
        if lifecycle.generation() != self.binding.generation()
            || lifecycle.control_revision() != self.binding.control_revision()
        {
            return Err(RuntimeTimelineError::StaleBinding {
                expected_generation: self.binding.generation(),
                expected_control_revision: self.binding.control_revision(),
                received_generation: lifecycle.generation(),
                received_control_revision: lifecycle.control_revision(),
            });
        }
        Ok(())
    }

    fn require_not_disposed(&self) -> Result<(), RuntimeTimelineError> {
        if self.disposed {
            Err(RuntimeTimelineError::Disposed)
        } else {
            Ok(())
        }
    }

    fn require_step(&self, timeline_id: &str, step_id: &str) -> Result<(), RuntimeTimelineError> {
        if self.catalog.step(timeline_id, step_id).is_some() {
            return Ok(());
        }
        if self.catalog.timeline(timeline_id).is_none() {
            return Err(RuntimeTimelineError::UnknownTimeline(
                timeline_id.to_owned(),
            ));
        }
        Err(RuntimeTimelineError::UnknownStep {
            timeline: timeline_id.to_owned(),
            step: step_id.to_owned(),
        })
    }

    fn operation_index(
        &self,
        operation_id: TimelineOperationIdentity,
    ) -> Result<usize, RuntimeTimelineError> {
        self.state
            .operations
            .iter()
            .position(|operation| operation.operation_id == operation_id)
            .ok_or(RuntimeTimelineError::OperationNotFound(operation_id))
    }

    fn ticket_index(
        &self,
        ticket: TimelineCompletionTicketId,
    ) -> Result<usize, RuntimeTimelineError> {
        self.state
            .tickets
            .iter()
            .position(|value| value.ticket.id() == ticket)
            .ok_or(RuntimeTimelineError::TicketNotFound(ticket))
    }

    fn cancel_bound_tickets(
        &mut self,
        operation_id: TimelineOperationIdentity,
        operation_revision: TimelineOperationRevision,
    ) {
        for live in &mut self.state.tickets {
            if live.ticket.operation_bound()
                && live.ticket.operation_id() == operation_id
                && live.ticket.operation_revision() == operation_revision
                && matches!(live.status, LiveTicketStatus::Pending)
            {
                live.status = LiveTicketStatus::Cancelled;
            }
        }
    }

    fn require_receipt(
        &self,
        receipt: TimelineOperationReceipt,
    ) -> Result<(), RuntimeTimelineError> {
        if receipt.binding != self.binding {
            return Err(RuntimeTimelineError::StaleBinding {
                expected_generation: self.binding.generation(),
                expected_control_revision: self.binding.control_revision(),
                received_generation: receipt.binding.generation(),
                received_control_revision: receipt.binding.control_revision(),
            });
        }
        let index = self.operation_index(receipt.operation_id)?;
        let actual = self.state.operations[index].revision;
        if actual != receipt.revision {
            return Err(RuntimeTimelineError::OperationReceiptMismatch {
                operation: receipt.operation_id,
                expected: actual,
                received: receipt.revision,
            });
        }
        Ok(())
    }

    fn receipt(&self, operation: &LiveOperation) -> TimelineOperationReceipt {
        TimelineOperationReceipt {
            binding: self.binding,
            operation_id: operation.operation_id,
            insertion_sequence: operation.insertion_sequence,
            revision: operation.revision,
        }
    }
}

/// Description for registering one async completion ticket.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineCompletionSpec {
    timeline_id: String,
    step_id: String,
    operation_id: TimelineOperationIdentity,
    operation_revision: TimelineOperationRevision,
    source: RuntimeSourceKind,
    correlation: String,
    result_contract: String,
    provenance: RuntimeProvenance,
}

impl TimelineCompletionSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        timeline_id: impl Into<String>,
        step_id: impl Into<String>,
        operation_id: TimelineOperationIdentity,
        operation_revision: TimelineOperationRevision,
        source: RuntimeSourceKind,
        correlation: impl Into<String>,
        result_contract: impl Into<String>,
        provenance: RuntimeProvenance,
    ) -> Result<Self, crate::RuntimeTimelineDataError> {
        let timeline_id = timeline_id.into();
        let step_id = step_id.into();
        let correlation = correlation.into();
        let result_contract = result_contract.into();
        if timeline_id.is_empty() || step_id.is_empty() || result_contract.is_empty() {
            return Err(crate::RuntimeTimelineDataError::EmptyIdentity);
        }
        if correlation.len() > crate::MAX_RUNTIME_CORRELATION_BYTES
            || runtime_lifecycle::validate_runtime_identity(&correlation).is_err()
        {
            if correlation.is_empty() {
                return Err(crate::RuntimeTimelineDataError::EmptyIdentity);
            }
            return Err(crate::RuntimeTimelineDataError::InvalidIdentity);
        }
        if result_contract.len() > crate::MAX_RUNTIME_CORRELATION_BYTES
            || runtime_lifecycle::validate_runtime_identity(&result_contract).is_err()
        {
            if result_contract.is_empty() {
                return Err(crate::RuntimeTimelineDataError::EmptyIdentity);
            }
            return Err(crate::RuntimeTimelineDataError::InvalidIdentity);
        }
        Ok(Self {
            timeline_id,
            step_id,
            operation_id,
            operation_revision,
            source,
            correlation,
            result_contract,
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

    pub const fn operation_revision(&self) -> TimelineOperationRevision {
        self.operation_revision
    }

    pub const fn source(&self) -> RuntimeSourceKind {
        self.source
    }

    pub fn correlation(&self) -> &str {
        &self.correlation
    }

    pub fn result_contract(&self) -> &str {
        &self.result_contract
    }

    pub fn provenance(&self) -> &RuntimeProvenance {
        &self.provenance
    }
}

/// Typed snapshot of queue/ticket mechanism state. Product persistence owns
/// when and where this value is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSnapshot {
    binding: crate::RuntimeTimelineBinding,
    next_insertion_sequence: u64,
    next_ticket_id: u64,
    last_release_step: Option<SimulationStep>,
    next_expected_step: u64,
    invalidated_ticket_count: u64,
    invalidated_admission_count: u64,
    operations: Vec<TimelineOperationSnapshot>,
    tickets: Vec<TimelineTicketSnapshot>,
}

impl TimelineSnapshot {
    /// Constructs a typed candidate for restore. The lane performs all
    /// uniqueness, canonical-order, quota, cursor, and compiled-step checks;
    /// this constructor deliberately performs none of those checks.
    pub fn from_parts(
        binding: crate::RuntimeTimelineBinding,
        next_insertion_sequence: u64,
        next_ticket_id: u64,
        last_release_step: Option<SimulationStep>,
        invalidated_ticket_count: u64,
        operations: Vec<TimelineOperationSnapshot>,
        tickets: Vec<TimelineTicketSnapshot>,
    ) -> Self {
        let next_expected_step = last_release_step
            .and_then(|step| step.value().checked_add(1))
            .unwrap_or(0);
        Self::from_parts_with_cursors(
            binding,
            next_insertion_sequence,
            next_ticket_id,
            last_release_step,
            next_expected_step,
            invalidated_ticket_count,
            0,
            operations,
            tickets,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_parts_with_cursors(
        binding: crate::RuntimeTimelineBinding,
        next_insertion_sequence: u64,
        next_ticket_id: u64,
        last_release_step: Option<SimulationStep>,
        next_expected_step: u64,
        invalidated_ticket_count: u64,
        invalidated_admission_count: u64,
        operations: Vec<TimelineOperationSnapshot>,
        tickets: Vec<TimelineTicketSnapshot>,
    ) -> Self {
        Self {
            binding,
            next_insertion_sequence,
            next_ticket_id,
            last_release_step,
            next_expected_step,
            invalidated_ticket_count,
            invalidated_admission_count,
            operations,
            tickets,
        }
    }

    pub const fn binding(&self) -> crate::RuntimeTimelineBinding {
        self.binding
    }

    pub const fn next_insertion_sequence(&self) -> u64 {
        self.next_insertion_sequence
    }

    pub const fn next_ticket_id(&self) -> u64 {
        self.next_ticket_id
    }

    pub const fn last_release_step(&self) -> Option<SimulationStep> {
        self.last_release_step
    }

    pub const fn next_expected_step(&self) -> u64 {
        self.next_expected_step
    }

    pub const fn invalidated_ticket_count(&self) -> u64 {
        self.invalidated_ticket_count
    }

    pub const fn invalidated_admission_count(&self) -> u64 {
        self.invalidated_admission_count
    }

    pub fn operations(&self) -> &[TimelineOperationSnapshot] {
        &self.operations
    }

    pub fn tickets(&self) -> &[TimelineTicketSnapshot] {
        &self.tickets
    }
}

/// Typed operation snapshot record.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineOperationSnapshot {
    operation_id: TimelineOperationIdentity,
    insertion_sequence: TimelineInsertionSequence,
    revision: TimelineOperationRevision,
    timeline_id: String,
    step_id: String,
    due_step: SimulationStep,
    recurrence: TimelineRecurrence,
    provenance: RuntimeProvenance,
}

impl TimelineOperationSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        operation_id: TimelineOperationIdentity,
        insertion_sequence: TimelineInsertionSequence,
        revision: TimelineOperationRevision,
        timeline_id: impl Into<String>,
        step_id: impl Into<String>,
        due_step: SimulationStep,
        recurrence: TimelineRecurrence,
        provenance: RuntimeProvenance,
    ) -> Self {
        Self {
            operation_id,
            insertion_sequence,
            revision,
            timeline_id: timeline_id.into(),
            step_id: step_id.into(),
            due_step,
            recurrence,
            provenance,
        }
    }

    pub const fn operation_id(&self) -> TimelineOperationIdentity {
        self.operation_id
    }

    pub const fn insertion_sequence(&self) -> TimelineInsertionSequence {
        self.insertion_sequence
    }

    pub const fn revision(&self) -> TimelineOperationRevision {
        self.revision
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

    fn from_live(operation: &LiveOperation) -> Self {
        Self {
            operation_id: operation.operation_id,
            insertion_sequence: operation.insertion_sequence,
            revision: operation.revision,
            timeline_id: operation.timeline_id.clone(),
            step_id: operation.step_id.clone(),
            due_step: operation.due_step,
            recurrence: operation.recurrence,
            provenance: operation.provenance.clone(),
        }
    }

    fn to_live(&self) -> LiveOperation {
        LiveOperation {
            operation_id: self.operation_id,
            insertion_sequence: self.insertion_sequence,
            revision: self.revision,
            timeline_id: self.timeline_id.clone(),
            step_id: self.step_id.clone(),
            due_step: self.due_step,
            recurrence: self.recurrence,
            provenance: self.provenance.clone(),
        }
    }
}

/// Typed ticket state retained in a snapshot.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineTicketSnapshotStatus {
    Pending,
    Completed(TimelineCompletionOutcome),
    Cancelled,
}

/// Typed completion ticket snapshot record.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineTicketSnapshot {
    ticket: TimelineCompletionTicket,
    status: TimelineTicketSnapshotStatus,
}

impl TimelineTicketSnapshot {
    pub fn from_parts(
        ticket: TimelineCompletionTicket,
        status: TimelineTicketSnapshotStatus,
    ) -> Self {
        Self { ticket, status }
    }

    pub fn ticket(&self) -> &TimelineCompletionTicket {
        &self.ticket
    }

    pub fn status(&self) -> &TimelineTicketSnapshotStatus {
        &self.status
    }

    fn from_live(ticket: &LiveTicket) -> Self {
        let status = match &ticket.status {
            LiveTicketStatus::Pending => TimelineTicketSnapshotStatus::Pending,
            LiveTicketStatus::Completed(outcome) => {
                TimelineTicketSnapshotStatus::Completed(outcome.clone())
            }
            LiveTicketStatus::Cancelled => TimelineTicketSnapshotStatus::Cancelled,
        };
        Self {
            ticket: ticket.ticket.clone(),
            status,
        }
    }

    fn to_live(&self) -> LiveTicket {
        let status = match &self.status {
            TimelineTicketSnapshotStatus::Pending => LiveTicketStatus::Pending,
            TimelineTicketSnapshotStatus::Completed(outcome) => {
                LiveTicketStatus::Completed(outcome.clone())
            }
            TimelineTicketSnapshotStatus::Cancelled => LiveTicketStatus::Cancelled,
        };
        LiveTicket {
            ticket: self.ticket.clone(),
            status,
        }
    }
}

fn operation_order(left: &LiveOperation, right: &LiveOperation) -> Ordering {
    left.due_step
        .cmp(&right.due_step)
        .then(left.insertion_sequence.cmp(&right.insertion_sequence))
        .then(left.operation_id.cmp(&right.operation_id))
}

fn operation_snapshot_order(
    left: &TimelineOperationSnapshot,
    right: &TimelineOperationSnapshot,
) -> Ordering {
    left.due_step
        .cmp(&right.due_step)
        .then(left.insertion_sequence.cmp(&right.insertion_sequence))
        .then(left.operation_id.cmp(&right.operation_id))
}
