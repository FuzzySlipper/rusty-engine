//! Read-only adapters over the existing inspection leaf.

use core_ids::EntityId;
use engine_inspector::{
    inspect_entity as inspect_entity_owner, inspect_mechanics_entity_structural,
    inspect_standard_borrowed_evidence, inspect_standard_plan as inspect_standard_plan_owner,
    EntityInspection, MechanicsEvaluationReadoutInspection, MechanicsStructuralEntityInspection,
    StandardBorrowedEvidence, StandardBorrowedEvidenceParts, StandardInspection,
};
use entity_state::EntityState;
use gameplay_mechanics::{MechanicsCatalog, MechanicsError};
use gameplay_standard::StandardOperationPlan;

/// Reads one entity through the existing `engine-inspector` projection.
pub fn inspect_entity(state: &EntityState, entity: EntityId) -> Option<EntityInspection> {
    inspect_entity_owner(state, entity.raw())
}

/// Reads structural mechanics facts without evaluating or mutating them.
pub fn inspect_mechanics(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
) -> Result<MechanicsStructuralEntityInspection, MechanicsError> {
    inspect_mechanics_entity_structural(state, catalog, entity)
}

/// Composes a standard plan with supplied structural mechanics evidence.
pub fn inspect_standard_plan<'a>(
    mechanics: &'a MechanicsStructuralEntityInspection,
    plan: &'a StandardOperationPlan,
) -> StandardInspection<'a> {
    inspect_standard_plan_owner(mechanics, plan)
}

/// Composes complete caller-supplied standard evidence without reevaluating a
/// definition, reading live state, mutating, or serializing a product explanation.
pub fn inspect_standard_evidence<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
    Explanation,
>(
    mechanics: &'a MechanicsStructuralEntityInspection,
    evaluations: Option<&'a MechanicsEvaluationReadoutInspection>,
    parts: StandardBorrowedEvidenceParts<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
        Explanation,
    >,
) -> StandardBorrowedEvidence<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
    Explanation,
> {
    inspect_standard_borrowed_evidence(mechanics, evaluations, parts)
}
