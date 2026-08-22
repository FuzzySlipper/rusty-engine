//! Adapters that select a resolver mode while leaving policy and transaction ownership downstream.

use entity_state::EntityState;
use gameplay_mechanics::MechanicsCatalog;
use gameplay_resolution::{
    ResolutionMode, ResolutionPolicy, ResolutionReceipt, ResolutionRequest, ResolutionTransaction,
    StandardResolver,
};
use gameplay_standard::{StandardOperationPlan, StandardPlanValidationError};

type Receipt<Policy, Transaction> = ResolutionReceipt<
    <Policy as ResolutionPolicy>::RawIntent,
    <Policy as ResolutionPolicy>::Intent,
    <Policy as ResolutionPolicy>::Facts,
    <Policy as ResolutionPolicy>::Evidence,
    <Policy as ResolutionPolicy>::Effect,
    <Policy as ResolutionPolicy>::Event,
    <Policy as ResolutionPolicy>::Rejection,
    <Policy as ResolutionPolicy>::Fault,
    <Policy as ResolutionPolicy>::Suspension,
    <Policy as ResolutionPolicy>::TraceDetail,
    <Transaction as ResolutionTransaction>::Error,
>;

/// Product-provided ordinary attempt input. The product owns every type here,
/// including intent/evidence and its policy and transaction construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardAttempt<RawIntent, Evidence> {
    pub identity: gameplay_resolution::ResolutionIdentity,
    pub intent: RawIntent,
    pub evidence: Vec<Evidence>,
}

impl<RawIntent, Evidence> StandardAttempt<RawIntent, Evidence> {
    pub const fn new(
        identity: gameplay_resolution::ResolutionIdentity,
        intent: RawIntent,
        evidence: Vec<Evidence>,
    ) -> Self {
        Self {
            identity,
            intent,
            evidence,
        }
    }

    pub fn request(self, mode: ResolutionMode) -> ResolutionRequest<RawIntent, Evidence> {
        ResolutionRequest::new(self.identity, mode, self.intent, self.evidence)
    }
}

/// Resolves a product policy through the exact existing resolver in preview mode.
/// `StandardResolver` stages then aborts the supplied transaction; this adapter
/// does not create a candidate, transaction, queue, or publication path.
pub fn preview_standard_attempt<Policy, Transaction>(
    resolver: &StandardResolver,
    policy: &mut Policy,
    transaction: &mut Transaction,
    attempt: StandardAttempt<Policy::RawIntent, Policy::Evidence>,
) -> Receipt<Policy, Transaction>
where
    Policy: ResolutionPolicy,
    Transaction: ResolutionTransaction<Effect = Policy::Effect>,
{
    resolver.resolve(
        policy,
        transaction,
        attempt.request(ResolutionMode::Preview),
    )
}

/// Resolves a product policy through the exact existing resolver in apply mode.
/// The product-supplied transaction remains the only publication owner.
pub fn execute_standard_attempt<Policy, Transaction>(
    resolver: &StandardResolver,
    policy: &mut Policy,
    transaction: &mut Transaction,
    attempt: StandardAttempt<Policy::RawIntent, Policy::Evidence>,
) -> Receipt<Policy, Transaction>
where
    Policy: ResolutionPolicy,
    Transaction: ResolutionTransaction<Effect = Policy::Effect>,
{
    resolver.resolve(policy, transaction, attempt.request(ResolutionMode::Apply))
}

/// Preserves existing plan staleness validation rather than making a command-specific guard.
pub fn validate_standard_plan(
    plan: &StandardOperationPlan,
    state: &EntityState,
    catalog: &MechanicsCatalog,
) -> Result<(), StandardPlanValidationError> {
    plan.validate_source_state(state, catalog)
}
