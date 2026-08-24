use std::{
    collections::{BTreeSet, VecDeque},
    marker::PhantomData,
};

use runtime_lifecycle::{
    RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId, RuntimeLifecycle, RuntimePhase,
    RuntimePhaseToken, RuntimeState, SimulationStep,
};

use crate::{
    compile::CompiledMutationCatalog,
    error::RuntimeMutationError,
    model::{
        validate_payload, MutationBatch, MutationBatchId, MutationCatalogIdentity,
        MutationCausation, MutationFingerprint, MutationOwnerEvidence, MutationProvenance,
        MutationResolvedBatch, MutationResolvedMetadata, MutationResolvedOperation, MutationStage,
        MAX_MUTATION_BATCH_OPERATIONS, MAX_MUTATION_RECEIPTS,
    },
};

/// Correlation identity of one instance-owned mutation lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeMutationBinding {
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
}

impl RuntimeMutationBinding {
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

/// A live product authority that can expose an exact, infallible guard.
///
/// The trait deliberately has no generic mutation method. The runtime lane
/// receives a shared view during planning and later assigns the complete owned
/// candidate through its caller-held `&mut A`.
///
/// This lane's Result-failure atomicity is conditional on a trusted Product
/// Assembly: `guard` must be an exact, side-effect-free, bounded readout;
/// `A::Guard`, owner evidence `E`, and their owned contents must have bounded,
/// non-panicking drop behavior; and the planner must not mutate `A` through
/// interior state while staging. Rust cannot prove or catch those conditions,
/// and this API makes no panic/crash/destructor atomicity claim. In
/// particular, assignment drops the replaced `A` value as part of the sole
/// publication assignment.
pub trait MutationAuthority {
    type Guard: Clone + Eq;

    fn guard(&self) -> Self::Guard;

    /// Stable Product Assembly publication domain for this authority bundle.
    fn publication_domain(&self) -> &str;
}

/// A Product Assembly planner supplied for one invocation only.
///
/// The planner must stage without interior-mutation side effects and return a
/// wholly owned candidate. It may call closed named Engine/Product Kernel
/// services internally, but the mutation lane stores no planner, callback,
/// service locator, registry, UI/TS entrypoint, or raw component reference.
pub trait MutationPlanner<A, E>
where
    A: MutationAuthority,
{
    type Error;

    fn stage(
        &mut self,
        authority: &A,
        batch: &MutationResolvedBatch,
    ) -> Result<MutationStage<A, E>, Self::Error>;
}

/// Immutable receipt for one successful batch publication.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationReceipt<G, E> {
    binding: RuntimeMutationBinding,
    step: SimulationStep,
    batch_id: MutationBatchId,
    batch_fingerprint: MutationFingerprint,
    catalog_identity: MutationCatalogIdentity,
    causation: MutationCausation,
    provenance: MutationProvenance,
    operations: Vec<MutationResolvedOperation>,
    observed_guard: G,
    committed_guard: G,
    owner_evidence: Vec<MutationOwnerEvidence<E>>,
}

impl<G, E> MutationReceipt<G, E> {
    fn new(
        binding: RuntimeMutationBinding,
        step: SimulationStep,
        batch: &MutationResolvedBatch,
        catalog_identity: MutationCatalogIdentity,
        observed_guard: G,
        committed_guard: G,
        owner_evidence: Vec<MutationOwnerEvidence<E>>,
    ) -> Self {
        Self {
            binding,
            step,
            batch_id: batch.id().clone(),
            batch_fingerprint: batch.fingerprint(),
            catalog_identity,
            causation: batch.causation().clone(),
            provenance: batch.provenance().clone(),
            operations: batch.operations().to_vec(),
            observed_guard,
            committed_guard,
            owner_evidence,
        }
    }

    pub const fn binding(&self) -> RuntimeMutationBinding {
        self.binding
    }

    pub const fn step(&self) -> SimulationStep {
        self.step
    }

    pub fn batch_id(&self) -> &MutationBatchId {
        &self.batch_id
    }

    pub const fn batch_fingerprint(&self) -> MutationFingerprint {
        self.batch_fingerprint
    }

    pub const fn catalog_identity(&self) -> MutationCatalogIdentity {
        self.catalog_identity
    }

    pub fn causation(&self) -> &MutationCausation {
        &self.causation
    }

    pub fn provenance(&self) -> &MutationProvenance {
        &self.provenance
    }

    pub fn operations(&self) -> &[MutationResolvedOperation] {
        &self.operations
    }

    pub fn observed_guard(&self) -> &G {
        &self.observed_guard
    }

    pub fn committed_guard(&self) -> &G {
        &self.committed_guard
    }

    pub fn owner_evidence(&self) -> &[MutationOwnerEvidence<E>] {
        &self.owner_evidence
    }
}

/// Bounded readout of one live mutation lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeMutationReadout {
    binding: RuntimeMutationBinding,
    next_expected_step: Option<u64>,
    last_applied_step: Option<SimulationStep>,
    invalidated_admission_count: u64,
    evicted_receipt_count: u64,
    disposed: bool,
}

impl RuntimeMutationReadout {
    pub const fn binding(self) -> RuntimeMutationBinding {
        self.binding
    }

    pub const fn next_expected_step(self) -> Option<u64> {
        self.next_expected_step
    }

    pub const fn last_applied_step(self) -> Option<SimulationStep> {
        self.last_applied_step
    }

    pub const fn invalidated_admission_count(self) -> u64 {
        self.invalidated_admission_count
    }

    pub const fn evicted_receipt_count(self) -> u64 {
        self.evicted_receipt_count
    }

    pub const fn disposed(self) -> bool {
        self.disposed
    }
}

#[derive(Debug)]
struct AppliedMutationRecord<G, E> {
    receipt: MutationReceipt<G, E>,
}

/// One instance-owned mutation lane. It stores static catalog and progression,
/// never a planner or any product service.
#[derive(Debug)]
pub struct RuntimeMutation<A, E>
where
    A: MutationAuthority,
    A::Guard: Clone,
    E: Clone,
{
    catalog: CompiledMutationCatalog,
    binding: RuntimeMutationBinding,
    next_expected_step: Option<u64>,
    last_applied_step: Option<SimulationStep>,
    invalidated_admission_count: u64,
    evicted_receipt_count: u64,
    receipt_history: VecDeque<AppliedMutationRecord<A::Guard, E>>,
    disposed: bool,
    authority: PhantomData<A>,
}

impl<A, E> RuntimeMutation<A, E>
where
    A: MutationAuthority,
    A::Guard: Clone,
    E: Clone,
{
    /// Binds a compiled catalog to a fresh running lifecycle before any step
    /// is admitted. Binding a progressed lifecycle would make the lane's
    /// exactly-once cursor ambiguous.
    pub fn bind(
        catalog: CompiledMutationCatalog,
        lifecycle: &RuntimeLifecycle,
    ) -> Result<Self, RuntimeMutationError<()>> {
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeMutationError::LifecycleNotRunning);
        }
        let admitted_steps = lifecycle.readout().admitted_simulation_steps();
        if admitted_steps != 0 {
            return Err(RuntimeMutationError::AlreadyAdvanced { admitted_steps });
        }
        Ok(Self {
            catalog,
            binding: RuntimeMutationBinding::new(
                lifecycle.instance_id(),
                lifecycle.generation(),
                lifecycle.control_revision(),
            ),
            next_expected_step: Some(0),
            last_applied_step: None,
            invalidated_admission_count: 0,
            evicted_receipt_count: 0,
            receipt_history: VecDeque::with_capacity(MAX_MUTATION_RECEIPTS),
            disposed: false,
            authority: PhantomData,
        })
    }

    pub fn catalog(&self) -> &CompiledMutationCatalog {
        &self.catalog
    }

    pub fn inspection(&self) -> &crate::RuntimeMutationInspection {
        self.catalog.inspection()
    }

    pub const fn binding(&self) -> RuntimeMutationBinding {
        self.binding
    }

    pub const fn readout(&self) -> RuntimeMutationReadout {
        RuntimeMutationReadout {
            binding: self.binding,
            next_expected_step: self.next_expected_step,
            last_applied_step: self.last_applied_step,
            invalidated_admission_count: self.invalidated_admission_count,
            evicted_receipt_count: self.evicted_receipt_count,
            disposed: self.disposed,
        }
    }

    pub fn last_receipt(&self) -> Option<&MutationReceipt<A::Guard, E>> {
        self.receipt_history.back().map(|record| &record.receipt)
    }

    /// Reads a retained receipt in the current binding's bounded history.
    /// Evicted receipts and receipts from a prior lifecycle binding are not
    /// available here; neither case permits the old step to be republished.
    pub fn receipt_for_step(&self, step: SimulationStep) -> Option<&MutationReceipt<A::Guard, E>> {
        self.receipt_history
            .iter()
            .find(|record| record.receipt.step() == step)
            .map(|record| &record.receipt)
    }

    /// Applies one nonempty batch for the exact lifecycle Mutation token.
    ///
    /// All Result-producing checks, receipt construction, and bounded history
    /// replacement occur before the one publication assignment. The guarantee
    /// is conditional Result-failure atomicity for a trusted Product Assembly;
    /// planner/interior-state effects, panic, allocation failure, and Drop
    /// behavior are outside the guarantee.
    pub fn apply_batch<P>(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        authority: &mut A,
        planner: &mut P,
        batch: MutationBatch,
    ) -> Result<MutationReceipt<A::Guard, E>, RuntimeMutationError<P::Error>>
    where
        P: MutationPlanner<A, E>,
    {
        self.validate_token(lifecycle, token)?;
        let step = token.simulation().step();

        self.validate_authority_domain(authority)?;
        if let Some(record) = self
            .receipt_history
            .iter()
            .find(|record| record.receipt.batch_id() == batch.id())
        {
            if record.receipt.batch_fingerprint() == batch.fingerprint() {
                if record.receipt.step() == step {
                    return Ok(record.receipt.clone());
                }
                return Err(RuntimeMutationError::BatchAlreadyApplied {
                    batch_id: batch.id().as_str().to_owned(),
                    applied_step: record.receipt.step(),
                    received_step: step,
                });
            }
            return Err(RuntimeMutationError::BatchIdentityConflict {
                step: record.receipt.step(),
                batch_id: batch.id().as_str().to_owned(),
                fingerprint: batch.fingerprint(),
                catalog_identity: self.catalog.catalog_identity(),
            });
        }
        if let Some(record) = self
            .receipt_history
            .iter()
            .find(|record| record.receipt.step() == step)
        {
            return Err(RuntimeMutationError::BatchIdentityConflict {
                step,
                batch_id: batch.id().as_str().to_owned(),
                fingerprint: batch.fingerprint(),
                catalog_identity: record.receipt.catalog_identity(),
            });
        }
        if self.next_expected_step != Some(step.value()) {
            return Err(RuntimeMutationError::StepOutOfOrder {
                expected: self.next_expected_step,
                received: step,
            });
        }
        let resolved = self.resolve_batch(&batch)?;
        let observed_guard = authority.guard();
        let staged = planner
            .stage(authority, &resolved)
            .map_err(RuntimeMutationError::Planner)?;
        let (candidate, owner_evidence) = staged.into_parts();
        self.validate_owner_evidence::<P::Error, E>(&resolved, &owner_evidence)?;
        self.validate_authority_domain(&candidate)?;
        let committed_guard = candidate.guard();
        if observed_guard != authority.guard() {
            return Err(RuntimeMutationError::AuthorityGuardChanged);
        }
        self.revalidate_before_commit(lifecycle, token, authority)?;
        let next_expected_step = next_step_after(step.value());
        let receipt = MutationReceipt::new(
            self.binding,
            step,
            &resolved,
            self.catalog.catalog_identity(),
            observed_guard,
            committed_guard,
            owner_evidence,
        );
        let stored_receipt = receipt.clone();
        let record = AppliedMutationRecord {
            receipt: stored_receipt,
        };
        let next_evicted_receipt_count = if self.receipt_history.len() == MAX_MUTATION_RECEIPTS {
            self.evicted_receipt_count
                .checked_add(1)
                .ok_or(RuntimeMutationError::ReceiptEvictionOverflow)?
        } else {
            self.evicted_receipt_count
        };
        // Replace generic receipt/batch state and update only primitive lane
        // bookkeeping before publication. The bounded queue is preallocated
        // at bind; no lane bookkeeping remains after the assignment.
        if self.receipt_history.len() == MAX_MUTATION_RECEIPTS {
            self.receipt_history.pop_front();
        }
        self.receipt_history.push_back(record);
        self.last_applied_step = Some(step);
        self.next_expected_step = next_expected_step;
        self.evicted_receipt_count = next_evicted_receipt_count;
        // This is the sole publication assignment. Do not add fallible work
        // below it: projection, persistence, and outbox/external status belong
        // to downstream owners after Applied.
        *authority = candidate;
        Ok(receipt)
    }

    /// Reconciles a lifecycle control revision or generation reset. An
    /// admitted step not yet successfully committed is counted as invalidated;
    /// it is never reported as completed or replayed automatically.
    pub fn rebind(&mut self, lifecycle: &RuntimeLifecycle) -> Result<(), RuntimeMutationError<()>> {
        if self.disposed {
            return Err(RuntimeMutationError::Disposed);
        }
        if lifecycle.instance_id() != self.binding.instance_id() {
            return Err(RuntimeMutationError::RebindForeignInstance);
        }
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeMutationError::RebindNotRunning);
        }
        let newer_generation = lifecycle.generation().value() > self.binding.generation().value();
        let newer_same_generation = lifecycle.generation() == self.binding.generation()
            && lifecycle.control_revision().value() > self.binding.control_revision().value();
        if !newer_generation && !newer_same_generation {
            return Err(RuntimeMutationError::RebindRegression);
        }
        let (next_expected_step, invalidated_admission_count) = if newer_generation {
            (Some(0), 0)
        } else {
            reconcile_admitted_steps(
                self.next_expected_step,
                lifecycle.readout().admitted_simulation_steps(),
                self.invalidated_admission_count,
            )?
        };
        self.binding = RuntimeMutationBinding::new(
            lifecycle.instance_id(),
            lifecycle.generation(),
            lifecycle.control_revision(),
        );
        self.next_expected_step = next_expected_step;
        self.invalidated_admission_count = invalidated_admission_count;
        if newer_generation {
            self.receipt_history.clear();
            self.evicted_receipt_count = 0;
            self.last_applied_step = None;
        }
        Ok(())
    }

    pub fn synchronize(
        &mut self,
        lifecycle: &RuntimeLifecycle,
    ) -> Result<(), RuntimeMutationError<()>> {
        self.rebind(lifecycle)
    }

    pub fn dispose(&mut self) {
        self.receipt_history.clear();
        self.last_applied_step = None;
        self.next_expected_step = None;
        self.disposed = true;
    }

    fn validate_token<PError>(
        &self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
    ) -> Result<(), RuntimeMutationError<PError>> {
        if self.disposed {
            return Err(RuntimeMutationError::Disposed);
        }
        if token.phase() != RuntimePhase::Mutation {
            return Err(RuntimeMutationError::WrongPhase {
                expected: RuntimePhase::Mutation,
                received: token.phase(),
            });
        }
        if lifecycle.instance_id() != self.binding.instance_id() {
            return Err(RuntimeMutationError::ForeignInstance {
                expected: self.binding.instance_id(),
                received: lifecycle.instance_id(),
            });
        }
        let simulation = token.simulation();
        if simulation.generation() != self.binding.generation()
            || simulation.control_revision() != self.binding.control_revision()
        {
            return Err(RuntimeMutationError::StaleBinding {
                expected_generation: self.binding.generation(),
                expected_control_revision: self.binding.control_revision(),
                received_generation: simulation.generation(),
                received_control_revision: simulation.control_revision(),
            });
        }
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeMutationError::LifecycleNotRunning);
        }
        lifecycle
            .validate_phase_token(token, RuntimePhase::Mutation)
            .map_err(RuntimeMutationError::Lifecycle)
    }

    fn revalidate_before_commit<PError>(
        &self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        authority: &A,
    ) -> Result<(), RuntimeMutationError<PError>> {
        self.validate_token(lifecycle, token)?;
        self.validate_authority_domain(authority)
    }

    fn validate_authority_domain<PError>(
        &self,
        authority: &A,
    ) -> Result<(), RuntimeMutationError<PError>> {
        if let Some(expected) = self.catalog.publication_domain() {
            if authority.publication_domain() != expected {
                return Err(RuntimeMutationError::DomainMismatch {
                    expected: expected.to_owned(),
                    received: authority.publication_domain().to_owned(),
                });
            }
        }
        Ok(())
    }

    fn resolve_batch<PError>(
        &self,
        batch: &MutationBatch,
    ) -> Result<MutationResolvedBatch, RuntimeMutationError<PError>> {
        if batch.operations().is_empty() {
            return Err(RuntimeMutationError::EmptyBatch);
        }
        if batch.operations().len() > MAX_MUTATION_BATCH_OPERATIONS {
            return Err(RuntimeMutationError::BoundsExceeded("mutation operations"));
        }
        let mut operation_ids = BTreeSet::new();
        let mut operations = Vec::with_capacity(batch.operations().len());
        for (index, operation) in batch.operations().iter().enumerate() {
            if !operation_ids.insert(operation.id()) {
                return Err(RuntimeMutationError::DuplicateOperationId(operation.id()));
            }
            let capability = self
                .catalog
                .capability(operation.binding_id())
                .ok_or_else(|| RuntimeMutationError::UnknownOperationBinding {
                    binding: operation.binding_id().to_owned(),
                    target: operation.target().to_owned(),
                })?;
            if capability.target() != operation.target() {
                return Err(RuntimeMutationError::OperationTargetMismatch {
                    binding: operation.binding_id().to_owned(),
                    expected: capability.target().to_owned(),
                    received: operation.target().to_owned(),
                });
            }
            let payload_bytes = validate_payload(operation.payload()).map_err(|error| {
                RuntimeMutationError::InvalidOperationPayload {
                    operation: operation.id(),
                    error,
                }
            })?;
            if payload_bytes > capability.maximum_payload_bytes() {
                return Err(RuntimeMutationError::OperationPayloadTooLarge {
                    operation: operation.id(),
                    actual: payload_bytes,
                    maximum: capability.maximum_payload_bytes(),
                });
            }
            operations.push(MutationResolvedOperation::new(
                index,
                operation,
                MutationResolvedMetadata {
                    binding_index: capability.binding_index(),
                    resolved_target: capability.resolved_target().to_owned(),
                    kind: capability.kind().to_owned(),
                    publication_domain: capability.publication_domain().to_owned(),
                    owner: capability.owner().to_owned(),
                    provenance_source: capability.provenance_source().to_owned(),
                    provenance_path: capability.provenance_path().to_owned(),
                },
            ));
        }
        Ok(MutationResolvedBatch::new(batch, operations))
    }

    fn validate_owner_evidence<PError, Evidence>(
        &self,
        batch: &MutationResolvedBatch,
        evidence: &[MutationOwnerEvidence<Evidence>],
    ) -> Result<(), RuntimeMutationError<PError>> {
        if evidence.len() != batch.operations().len() {
            return Err(RuntimeMutationError::OwnerEvidenceCount {
                expected: batch.operations().len(),
                received: evidence.len(),
            });
        }
        for (index, (expected, received)) in
            batch.operations().iter().zip(evidence.iter()).enumerate()
        {
            if expected.id() != received.operation_id() {
                return Err(RuntimeMutationError::OwnerEvidenceMismatch {
                    index,
                    reason: "operation identity or order",
                });
            }
            if expected.binding_id() != received.binding_id()
                || expected.target() != received.target()
                || expected.resolved_target() != received.resolved_target()
                || expected.publication_domain() != received.publication_domain()
                || expected.owner() != received.owner()
            {
                return Err(RuntimeMutationError::OwnerEvidenceMismatch {
                    index,
                    reason: "binding target, resolved target, publication domain, or named owner",
                });
            }
        }
        Ok(())
    }
}

fn next_step_after(step: u64) -> Option<u64> {
    step.checked_add(1)
}

fn reconcile_admitted_steps(
    next_expected_step: Option<u64>,
    admitted_steps: u64,
    invalidated_admission_count: u64,
) -> Result<(Option<u64>, u64), RuntimeMutationError<()>> {
    let expected_admissions = next_expected_step.unwrap_or(u64::MAX);
    if admitted_steps < expected_admissions {
        return Err(RuntimeMutationError::RebindAdmissionRegression {
            expected_next_step: next_expected_step,
            admitted_steps,
        });
    }
    let newly_invalidated = admitted_steps - expected_admissions;
    let invalidated_admission_count = invalidated_admission_count
        .checked_add(newly_invalidated)
        .ok_or(RuntimeMutationError::InvalidatedAdmissionOverflow)?;
    let next_expected_step = if admitted_steps == u64::MAX {
        None
    } else {
        Some(admitted_steps)
    };
    Ok((next_expected_step, invalidated_admission_count))
}
