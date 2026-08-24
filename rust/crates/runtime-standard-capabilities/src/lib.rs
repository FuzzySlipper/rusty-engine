//! Closed, host-neutral Runtime Composition capability mechanisms.
//!
//! This crate deliberately implements one inspectable observation shape rather
//! than a query language: typed observer/target facts, distance and facing,
//! center-ray occlusion, a deterministic target reduction, and a caller-owned
//! Product Kernel operation publication through `runtime-mutation`.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use core_ids::EntityId;
use core_math::Vec3;
use engine_spatial::{SpatialOcclusionQuery, SpatialOcclusionService, VoxelCollisionScene};
use entity_state::{ComponentTypeId, EntityComponent, EntityLifecycle, EntityState};
use product_model::{LinkedCapabilityTarget, LinkedProductComposition, SchedulePhase};
use runtime_mutation::{
    CompiledMutationCatalog, MutationBatch, MutationBatchId, MutationCausation, MutationDataError,
    MutationOperation, MutationOperationId, MutationProvenance,
};
use runtime_schedule::CompiledSystem;
use serde::Deserialize;
use serde_json::json;

pub const OBSERVE_PAIRS_TARGET: &str = "engine.runtime.observe-pairs";
pub const OBSERVE_PAIRS_PLAN_KIND: &str = "engine.runtime.observe-pairs.v1";
pub const OBSERVE_PAIRS_RESULT_KIND: &str = "engine.runtime.observe-pairs.result.v1";
pub const MAX_OBSERVERS: usize = 64;
pub const MAX_TARGETS: usize = 256;
pub const MAX_CANDIDATE_PAIRS: usize = 1_024;
pub const MAX_AGGREGATE_ENTRIES: usize = 256;

/// Rust-owned cross-language descriptor for the one currently admitted
/// standard capability. It is intentionally a current contract, not a
/// version family: TypeScript generation consumes this exact JSON.
pub fn encode_runtime_standard_capabilities_contract_descriptor() -> String {
    serde_json::to_string_pretty(&json!({
        "artifact": "runtime-standard-capabilities",
        "observePairs": {
            "target": OBSERVE_PAIRS_TARGET,
            "kind": "system",
            "maximumCompactJsonPayloadBytes": 16_384,
            "access": {
                "reads": ["entity-state.components", "entity-state.transforms", "engine-spatial.occlusion"],
                "writes": ["runtime-mutation.operations"]
            },
            "payload": {
                "kind": OBSERVE_PAIRS_PLAN_KIND,
                "visibility": "center-ray",
                "resultKind": OBSERVE_PAIRS_RESULT_KIND,
                "fields": ["kind", "observerRole", "targetRole", "operationBinding", "operationType", "quotas"],
                "quotaFields": ["observers", "targets", "pairs", "aggregates"]
            },
            "quotas": { "observers": MAX_OBSERVERS, "targets": MAX_TARGETS, "pairs": MAX_CANDIDATE_PAIRS, "aggregates": MAX_AGGREGATE_ENTRIES }
        }
    })).expect("static standard capability contract is valid JSON") + "\n"
}

/// Compile-time adapter for an inert observer component. Product components
/// retain their own meaning; this exports only neutral sensing facts.
pub trait ObservePairsObserver: EntityComponent {
    fn facts(&self) -> ObservePairsObserverFacts;
}

/// Compile-time adapter for an inert target marker/fact.
pub trait ObservePairsTarget: EntityComponent {
    fn local_center(&self) -> Vec3;
}

/// Product-authored, typed sensing facts. They are read through a static Rust
/// component adapter rather than authored as a dynamic query expression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservePairsObserverFacts {
    pub local_origin: Vec3,
    pub local_forward: Vec3,
    pub maximum_distance: f32,
    pub minimum_facing_cosine: f32,
    pub evidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObservePairsPlan {
    observer_role: String,
    target_role: String,
    operation_binding: String,
    operation_target: String,
    operation_type: String,
    observers: usize,
    targets: usize,
    pairs: usize,
    aggregates: usize,
    cadence_every_steps: u32,
    cadence_offset_steps: u32,
    inspection: ObservePairsInspection,
}

/// Immutable linkage, access, and quota readout for the closed standard
/// system. It is data for inspection, not a runtime dispatch registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservePairsInspection {
    pub system_binding: String,
    pub system_owner: String,
    pub system_provenance_source: String,
    pub system_provenance_path: String,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub maximum_payload_bytes: usize,
    pub operation_binding: String,
    pub operation_target: String,
    pub operation_owner: String,
    pub operation_provenance_source: String,
    pub operation_provenance_path: String,
    pub operation_type: String,
    pub visibility: String,
    pub maximum_observers: usize,
    pub maximum_targets: usize,
    pub maximum_candidate_pairs: usize,
    pub maximum_aggregate_entries: usize,
}

impl ObservePairsPlan {
    /// Compiles with the complete linked composition when it is available at
    /// assembly time, additionally proving the selected operation resolves to
    /// a Product Kernel owner.
    pub fn compile(
        linked: &LinkedProductComposition,
        system: &CompiledSystem,
        mutations: &CompiledMutationCatalog,
    ) -> Result<Self, ObservePairsError> {
        let binding = linked
            .capability_binding(system.capability().binding_index())
            .ok_or(ObservePairsError::UnknownSystemBinding)?;
        if binding.target() != OBSERVE_PAIRS_TARGET
            || system.capability().target() != OBSERVE_PAIRS_TARGET
        {
            return Err(ObservePairsError::WrongSystemTarget);
        }
        let plan = Self::compile_system(system, mutations)?;
        let operation_binding = linked
            .capability_bindings()
            .iter()
            .find(|binding| binding.id() == plan.operation_binding())
            .ok_or(ObservePairsError::UnknownOperationBinding)?;
        if !matches!(
            operation_binding.resolved_target(),
            LinkedCapabilityTarget::ProductKernel(_)
        ) {
            return Err(ObservePairsError::OperationNotProductKernel);
        }
        Ok(plan)
    }

    /// Compiles from the immutable schedule and mutation artifacts retained by
    /// `runtime-composition`. This is the generated Product Assembly seam: no
    /// linked composition, registry, or dynamic owner lookup is required once
    /// both owning compilers have retained exact binding metadata.
    pub fn compile_system(
        system: &CompiledSystem,
        mutations: &CompiledMutationCatalog,
    ) -> Result<Self, ObservePairsError> {
        if system.capability().target() != OBSERVE_PAIRS_TARGET {
            return Err(ObservePairsError::WrongSystemTarget);
        }
        if system.phase() != SchedulePhase::Simulation {
            return Err(ObservePairsError::WrongSchedulePhase);
        }
        let wire: ObservePairsWire = serde_json::from_value(system.payload().clone())
            .map_err(|error| ObservePairsError::InvalidPlan(error.to_string()))?;
        if wire.kind != OBSERVE_PAIRS_PLAN_KIND {
            return Err(ObservePairsError::WrongPlanKind);
        }
        validate_identity(&wire.observer_role, "observer role")?;
        validate_identity(&wire.target_role, "target role")?;
        validate_identity(&wire.operation_binding, "operation binding")?;
        validate_identity(&wire.operation_type, "operation type")?;
        if wire.operation_type != OBSERVE_PAIRS_RESULT_KIND {
            return Err(ObservePairsError::OperationTypeMismatch);
        }
        let quotas = wire.quotas;
        validate_quota("observers", quotas.observers, MAX_OBSERVERS)?;
        validate_quota("targets", quotas.targets, MAX_TARGETS)?;
        validate_quota("pairs", quotas.pairs, MAX_CANDIDATE_PAIRS)?;
        validate_quota("aggregates", quotas.aggregates, MAX_AGGREGATE_ENTRIES)?;
        let operation = mutations
            .capability(&wire.operation_binding)
            .ok_or(ObservePairsError::UnknownOperationBinding)?;
        if !operation.is_product_kernel_target() {
            return Err(ObservePairsError::OperationNotProductKernel);
        }
        if operation.operation_type() != wire.operation_type {
            return Err(ObservePairsError::OperationTypeMismatch);
        }
        Ok(Self {
            observer_role: wire.observer_role,
            target_role: wire.target_role,
            operation_binding: wire.operation_binding.clone(),
            operation_target: operation.target().to_owned(),
            operation_type: wire.operation_type.clone(),
            observers: quotas.observers,
            targets: quotas.targets,
            pairs: quotas.pairs,
            aggregates: quotas.aggregates,
            cadence_every_steps: system.cadence().every_steps(),
            cadence_offset_steps: system.cadence().offset_steps(),
            inspection: ObservePairsInspection {
                system_binding: system.capability().id().to_owned(),
                system_owner: system.capability().resolved_owner().to_owned(),
                system_provenance_source: system.capability().provenance_source().to_owned(),
                system_provenance_path: system.capability().provenance_path().to_owned(),
                reads: system.capability().reads().to_vec(),
                writes: system.capability().writes().to_vec(),
                maximum_payload_bytes: system.capability().maximum_compact_json_payload_bytes(),
                operation_binding: wire.operation_binding.clone(),
                operation_target: operation.target().to_owned(),
                operation_owner: operation.owner().to_owned(),
                operation_provenance_source: operation.provenance_source().to_owned(),
                operation_provenance_path: operation.provenance_path().to_owned(),
                operation_type: wire.operation_type.clone(),
                visibility: "center-ray".to_owned(),
                maximum_observers: quotas.observers,
                maximum_targets: quotas.targets,
                maximum_candidate_pairs: quotas.pairs,
                maximum_aggregate_entries: quotas.aggregates,
            },
        })
    }

    pub fn operation_binding(&self) -> &str {
        &self.operation_binding
    }
    pub fn operation_type(&self) -> &str {
        &self.operation_type
    }

    /// The existing `runtime-schedule` owns due-step admission; this readout
    /// lets a named dispatcher verify which compiled cadence selected the
    /// capability without this crate creating another timer.
    pub const fn cadence_every_steps(&self) -> u32 {
        self.cadence_every_steps
    }
    pub const fn cadence_offset_steps(&self) -> u32 {
        self.cadence_offset_steps
    }
    pub fn inspection(&self) -> &ObservePairsInspection {
        &self.inspection
    }

    /// Converts one complete readout into exactly one ordered Product Kernel
    /// operation. The payload is neutral evidence data; threshold, alert, and
    /// consequence meaning remain in the Product Kernel planner.
    pub fn mutation_batch(
        &self,
        readout: &ObservePairsReadout,
        batch_id: MutationBatchId,
        causation: MutationCausation,
        provenance: MutationProvenance,
        operation_id: MutationOperationId,
    ) -> Result<MutationBatch, ObservePairsError> {
        self.validate_readout(readout)?;
        let results = readout
            .aggregates
            .iter()
            .map(|entry| {
                json!({
                    "target": entry.target.raw(),
                    "visibleObserverCount": entry.visible_observer_count,
                    "evidenceTotal": entry.evidence_total,
                })
            })
            .collect::<Vec<_>>();
        let operation = MutationOperation::new(
            operation_id,
            self.operation_binding.clone(),
            self.operation_target.clone(),
            json!({
                "kind": OBSERVE_PAIRS_RESULT_KIND,
                "operationType": self.operation_type,
                "results": results,
            }),
        )
        .map_err(ObservePairsError::MutationData)?;
        MutationBatch::new(batch_id, causation, provenance, vec![operation])
            .map_err(ObservePairsError::MutationData)
    }

    fn validate_readout(&self, readout: &ObservePairsReadout) -> Result<(), ObservePairsError> {
        enforce("observers", readout.selected_observers, self.observers)?;
        enforce("targets", readout.selected_targets, self.targets)?;
        enforce("pairs", readout.pairs_examined, self.pairs)?;
        enforce("aggregates", readout.aggregates.len(), self.aggregates)?;
        let mut previous = None;
        let mut visible_pairs = 0usize;
        for entry in &readout.aggregates {
            if previous.is_some_and(|entity| entity >= entry.target)
                || entry.visible_observer_count == 0
                || !entry.evidence_total.is_finite()
            {
                return Err(ObservePairsError::InvalidReadout);
            }
            previous = Some(entry.target);
            let entry_visible = usize::try_from(entry.visible_observer_count)
                .map_err(|_| ObservePairsError::InvalidReadout)?;
            visible_pairs = visible_pairs
                .checked_add(entry_visible)
                .ok_or(ObservePairsError::InvalidReadout)?;
        }
        let maximum_comparisons = readout
            .selected_observers
            .checked_mul(readout.selected_targets)
            .ok_or(ObservePairsError::InvalidReadout)?;
        if visible_pairs != readout.visible_pairs
            || readout.selection_comparisons > maximum_comparisons
            || readout.distance_rejects.checked_add(readout.pairs_examined)
                != Some(readout.selection_comparisons)
            || readout.facing_rejects.checked_add(readout.visibility_casts)
                != Some(readout.pairs_examined)
            || readout.occlusion_rejects.checked_add(readout.visible_pairs)
                != Some(readout.visibility_casts)
        {
            return Err(ObservePairsError::InvalidReadout);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObservePairsReadout {
    pub selected_observers: usize,
    pub selected_targets: usize,
    /// All bounded observer/target comparisons performed before distance.
    pub selection_comparisons: usize,
    /// Distance-qualified candidate pairs charged to the authored pair quota.
    pub pairs_examined: usize,
    pub distance_rejects: usize,
    pub facing_rejects: usize,
    pub visibility_casts: usize,
    pub occlusion_rejects: usize,
    pub visible_pairs: usize,
    pub aggregates: Vec<ObservePairsAggregate>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservePairsAggregate {
    pub target: EntityId,
    pub visible_observer_count: u64,
    pub evidence_total: f64,
}

/// Product-owned correlation facts for one standard-capability publication.
/// The generated concrete adapter constructs this value directly; Engine does
/// not derive product causation or provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservePairsBatchIdentity {
    pub batch_id: MutationBatchId,
    pub causation: MutationCausation,
    pub provenance: MutationProvenance,
    pub operation_id: MutationOperationId,
}

/// One fully evaluated standard system result ready for the existing
/// `ProductRuntimeAdapter::prepare_mutation` path.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservePairsEmission {
    pub readout: ObservePairsReadout,
    pub batch: MutationBatch,
}

impl ObservePairsPlan {
    /// Evaluates a fully typed, compile-time selected observation plan. It
    /// produces no mutation and has no callbacks; its caller publishes an
    /// ordered operation through the separately-owned mutation lane.
    pub fn evaluate<O: ObservePairsObserver, T: ObservePairsTarget>(
        &self,
        entities: &EntityState,
        scene: &VoxelCollisionScene,
    ) -> Result<ObservePairsReadout, ObservePairsError> {
        check_role::<O>(entities, &self.observer_role)?;
        check_role::<T>(entities, &self.target_role)?;
        let observers = entities
            .components::<O>()
            .map_err(|_| ObservePairsError::UnregisteredRole)?
            .filter(|(id, _)| entities.lifecycle(*id) == Some(EntityLifecycle::Active))
            .collect::<Vec<_>>();
        let targets = entities
            .components::<T>()
            .map_err(|_| ObservePairsError::UnregisteredRole)?
            .filter(|(id, _)| entities.lifecycle(*id) == Some(EntityLifecycle::Active))
            .collect::<Vec<_>>();
        enforce("observers", observers.len(), self.observers)?;
        enforce("targets", targets.len(), self.targets)?;
        for (target, facts) in &targets {
            let transform = entities
                .world_transform(*target)
                .ok_or(ObservePairsError::MissingTransform(*target))?;
            if !finite_vec3(transform.transform_point(facts.local_center())) {
                return Err(ObservePairsError::InvalidTargetFacts(*target));
            }
        }
        let mut pairs = 0usize;
        let mut selection_comparisons = 0usize;
        let mut distance_rejects = 0usize;
        let mut facing_rejects = 0usize;
        let mut casts = 0usize;
        let mut occlusion_rejects = 0usize;
        let mut reductions: BTreeMap<EntityId, (u64, f64)> = BTreeMap::new();
        for (observer, fact) in &observers {
            let transform = entities
                .world_transform(*observer)
                .ok_or(ObservePairsError::MissingTransform(*observer))?;
            let facts = fact.facts();
            validate_observer_facts(*observer, facts)?;
            let origin = transform.transform_point(facts.local_origin);
            let forward = transform.transform_direction(facts.local_forward);
            for (target, target_facts) in &targets {
                selection_comparisons = selection_comparisons
                    .checked_add(1)
                    .ok_or(ObservePairsError::QuotaExceeded("selection comparisons"))?;
                let target_transform = entities
                    .world_transform(*target)
                    .ok_or(ObservePairsError::MissingTransform(*target))?;
                let target_center = target_transform.transform_point(target_facts.local_center());
                if !finite_vec3(target_center) {
                    return Err(ObservePairsError::InvalidTargetFacts(*target));
                }
                let delta = target_center - origin;
                let distance_squared = delta.length_squared();
                if !distance_squared.is_finite()
                    || distance_squared > facts.maximum_distance.powi(2)
                {
                    distance_rejects += 1;
                    continue;
                }
                pairs = pairs
                    .checked_add(1)
                    .ok_or(ObservePairsError::QuotaExceeded("pairs"))?;
                enforce("pairs", pairs, self.pairs)?;
                let length = distance_squared.sqrt();
                if length <= 0.0 {
                    facing_rejects += 1;
                    continue;
                }
                let cosine = forward.dot(delta) / (forward.length() * length);
                if !cosine.is_finite() || cosine < facts.minimum_facing_cosine {
                    facing_rejects += 1;
                    continue;
                }
                casts += 1;
                let hit = SpatialOcclusionService
                    .cast_ray(
                        scene,
                        entities,
                        SpatialOcclusionQuery {
                            origin: origin.to_array().map(f64::from),
                            direction: delta.to_array().map(f64::from),
                            max_distance: f64::from(length),
                            ignored_entities: &[*observer, *target],
                        },
                    )
                    .map_err(ObservePairsError::Occlusion)?;
                if hit.is_some() {
                    occlusion_rejects += 1;
                    continue;
                }
                let entry = reductions.entry(*target).or_insert((0, 0.0));
                entry.0 = entry
                    .0
                    .checked_add(1)
                    .ok_or(ObservePairsError::QuotaExceeded("visible observers"))?;
                entry.1 += facts.evidence;
                if !entry.1.is_finite() {
                    return Err(ObservePairsError::NonFiniteEvidence);
                }
            }
        }
        enforce("aggregates", reductions.len(), self.aggregates)?;
        let aggregates = reductions
            .into_iter()
            .map(
                |(target, (visible_observer_count, evidence_total))| ObservePairsAggregate {
                    target,
                    visible_observer_count,
                    evidence_total,
                },
            )
            .collect::<Vec<_>>();
        Ok(ObservePairsReadout {
            selected_observers: observers.len(),
            selected_targets: targets.len(),
            selection_comparisons,
            pairs_examined: pairs,
            distance_rejects,
            facing_rejects,
            visibility_casts: casts,
            occlusion_rejects,
            visible_pairs: aggregates
                .iter()
                .map(|entry| entry.visible_observer_count as usize)
                .sum(),
            aggregates,
        })
    }

    /// Closed end-to-end standard schedule adapter. Concrete product source
    /// selects `O` and `T` at compile time and retains the returned batch for
    /// the composition root's one mutation publication call.
    pub fn evaluate_and_batch<O: ObservePairsObserver, T: ObservePairsTarget>(
        &self,
        entities: &EntityState,
        scene: &VoxelCollisionScene,
        identity: ObservePairsBatchIdentity,
    ) -> Result<ObservePairsEmission, ObservePairsError> {
        let readout = self.evaluate::<O, T>(entities, scene)?;
        let batch = self.mutation_batch(
            &readout,
            identity.batch_id,
            identity.causation,
            identity.provenance,
            identity.operation_id,
        )?;
        Ok(ObservePairsEmission { readout, batch })
    }
}

fn finite_vec3(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}
fn validate_observer_facts(
    entity: EntityId,
    facts: ObservePairsObserverFacts,
) -> Result<(), ObservePairsError> {
    if !finite_vec3(facts.local_origin)
        || !finite_vec3(facts.local_forward)
        || !facts.maximum_distance.is_finite()
        || facts.maximum_distance <= 0.0
        || !facts.minimum_facing_cosine.is_finite()
        || !(-1.0..=1.0).contains(&facts.minimum_facing_cosine)
        || !facts.evidence.is_finite()
        || facts.local_forward.length_squared() <= 0.0
    {
        Err(ObservePairsError::InvalidObserverFacts(entity))
    } else {
        Ok(())
    }
}
fn check_role<T: EntityComponent>(
    entities: &EntityState,
    expected: &str,
) -> Result<(), ObservePairsError> {
    let actual = entities
        .component_type_id::<T>()
        .map_err(|_| ObservePairsError::UnregisteredRole)?;
    if actual.as_str() == expected {
        Ok(())
    } else {
        Err(ObservePairsError::RoleIdentityMismatch {
            expected: expected.into(),
            actual: actual.as_str().into(),
        })
    }
}
fn validate_identity(value: &str, field: &str) -> Result<(), ObservePairsError> {
    ComponentTypeId::parse(value)
        .map(|_| ())
        .map_err(|_| ObservePairsError::InvalidPlan(format!("invalid {field}")))
}
fn validate_quota(
    name: &'static str,
    received: usize,
    maximum: usize,
) -> Result<(), ObservePairsError> {
    if received == 0 || received > maximum {
        Err(ObservePairsError::InvalidQuota {
            name,
            received,
            maximum,
        })
    } else {
        Ok(())
    }
}
fn enforce(name: &'static str, received: usize, maximum: usize) -> Result<(), ObservePairsError> {
    if received > maximum {
        Err(ObservePairsError::QuotaExceeded(name))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum ObservePairsError {
    UnknownSystemBinding,
    WrongSystemTarget,
    WrongSchedulePhase,
    WrongPlanKind,
    InvalidPlan(String),
    InvalidQuota {
        name: &'static str,
        received: usize,
        maximum: usize,
    },
    UnknownOperationBinding,
    OperationNotProductKernel,
    OperationTypeMismatch,
    UnregisteredRole,
    RoleIdentityMismatch {
        expected: String,
        actual: String,
    },
    MissingTransform(EntityId),
    QuotaExceeded(&'static str),
    InvalidObserverFacts(EntityId),
    InvalidTargetFacts(EntityId),
    NonFiniteEvidence,
    InvalidReadout,
    MutationData(MutationDataError),
    Occlusion(engine_spatial::SpatialOcclusionError),
}
impl std::fmt::Display for ObservePairsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "observe-pairs rejected: {self:?}")
    }
}
impl std::error::Error for ObservePairsError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservePairsWire {
    kind: String,
    observer_role: String,
    target_role: String,
    operation_binding: String,
    operation_type: String,
    quotas: ObservePairsQuotas,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservePairsQuotas {
    observers: usize,
    targets: usize,
    pairs: usize,
    aggregates: usize,
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use entity_state::{ComponentRegistration, EntityAuthoringService, EntityDefinition};

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Vision {
        facts: ObservePairsObserverFacts,
    }

    impl EntityComponent for Vision {}

    impl ObservePairsObserver for Vision {
        fn facts(&self) -> ObservePairsObserverFacts {
            self.facts
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Target {
        center: Vec3,
    }

    impl EntityComponent for Target {}

    impl ObservePairsTarget for Target {
        fn local_center(&self) -> Vec3 {
            self.center
        }
    }

    fn plan() -> ObservePairsPlan {
        ObservePairsPlan {
            observer_role: "product.observer".into(),
            target_role: "product.target".into(),
            operation_binding: "kernel.alert".into(),
            operation_target: "kernel.alert-state".into(),
            operation_type: "product.alert-observation.v1".into(),
            observers: 64,
            targets: 256,
            pairs: 1024,
            aggregates: 256,
            cadence_every_steps: 6,
            cadence_offset_steps: 0,
            inspection: ObservePairsInspection {
                system_binding: "observe-pairs".into(),
                system_owner: "rusty-engine.runtime-standard-capabilities".into(),
                system_provenance_source: "test".into(),
                system_provenance_path: "test".into(),
                reads: vec![],
                writes: vec![],
                maximum_payload_bytes: 16_384,
                operation_binding: "kernel.alert".into(),
                operation_target: "kernel.alert-state".into(),
                operation_owner: "product.kernel".into(),
                operation_provenance_source: "test".into(),
                operation_provenance_path: "test".into(),
                operation_type: "engine.runtime.observe-pairs.result.v1".into(),
                visibility: "center-ray".into(),
                maximum_observers: 64,
                maximum_targets: 256,
                maximum_candidate_pairs: 1024,
                maximum_aggregate_entries: 256,
            },
        }
    }

    fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
        let revision = state
            .component_revision::<T>(entity)
            .expect("registered component revision");
        EntityAuthoringService
            .attach_component(state, revision, entity, value)
            .expect("attach typed pressure-test fact");
    }

    #[test]
    fn contract_is_closed_and_names_center_ray() {
        let value: Value =
            serde_json::from_str(&encode_runtime_standard_capabilities_contract_descriptor())
                .expect("contract json");
        assert_eq!(value["observePairs"]["target"], OBSERVE_PAIRS_TARGET);
        assert_eq!(value["observePairs"]["payload"]["visibility"], "center-ray");
        assert_eq!(
            value["observePairs"]["quotas"]["pairs"],
            MAX_CANDIDATE_PAIRS
        );
    }

    #[test]
    fn cross_language_stealth_fixture_links_schedule_operation_and_fixed_type() {
        let composition = product_model::decode_compiled_composition(include_bytes!(
            "../../../../fixtures/runtime-standard-capabilities/stealth.observe-pairs.compiled-composition.json"
        ))
        .expect("TypeScript-authored stealth composition decodes in Rust");
        let manifest_source = include_str!("../../../../fixtures/product-model/minimum.rusty.toml")
            .replace("example.product", "stealth.pressure");
        let manifest =
            product_model::decode_product_manifest(&manifest_source).expect("pressure manifest");
        let admitted = product_model::admit_checked_product_composition(&manifest, composition)
            .expect("pressure composition admission");
        let kernel = [product_model::ProductKernelCapabilityDescriptor::new(
            "advance-alert",
            product_model::CapabilityMetadata::new(
                product_model::CapabilityKind::Operation,
                product_model::CapabilityUses::SCHEDULE,
                product_model::CapabilityAvailability::Linkable,
                product_model::CapabilityAccess::new(&[], &[]),
                product_model::CapabilityBudget::new(16_384),
                product_model::CapabilityProvenance::new(
                    "stealth.product.kernel",
                    "kernel/alert.rs",
                    "advance_alert",
                ),
            ),
        )];
        let linked = product_model::link_admitted_product_composition(admitted, &kernel)
            .expect("closed Engine and Product Kernel linkage");
        let mutations = CompiledMutationCatalog::compile(
            &linked,
            &[runtime_mutation::MutationCapabilityDescriptor::new(
                "stealth.advance-alert",
                "kernel.advance-alert",
                "stealth.world",
                "stealth.product.kernel",
                OBSERVE_PAIRS_RESULT_KIND,
            )],
        )
        .expect("typed operation catalog");
        let schedule = runtime_schedule::CompiledRuntimeSchedule::compile(&linked)
            .expect("compiled cadence schedule");
        let system = &schedule
            .phase(product_model::SchedulePhase::Simulation)
            .systems()[0];
        let compiled =
            ObservePairsPlan::compile(&linked, system, &mutations).expect("closed standard plan");
        assert_eq!(compiled.cadence_every_steps(), 6);
        assert_eq!(compiled.cadence_offset_steps(), 0);
        assert_eq!(compiled.operation_type(), OBSERVE_PAIRS_RESULT_KIND);
        assert_eq!(
            compiled.inspection().operation_owner,
            "stealth.product.kernel"
        );
    }

    #[test]
    fn readout_becomes_exactly_one_kernel_operation() {
        let readout = ObservePairsReadout {
            selected_observers: 1,
            selected_targets: 1,
            selection_comparisons: 1,
            pairs_examined: 1,
            distance_rejects: 0,
            facing_rejects: 0,
            visibility_casts: 1,
            occlusion_rejects: 0,
            visible_pairs: 1,
            aggregates: vec![ObservePairsAggregate {
                target: EntityId::new(9),
                visible_observer_count: 1,
                evidence_total: 0.25,
            }],
        };
        let batch = plan()
            .mutation_batch(
                &readout,
                MutationBatchId::new("observe-step-6").expect("batch id"),
                MutationCausation::new("runtime-schedule").expect("causation"),
                MutationProvenance::new("product.kernel").expect("provenance"),
                MutationOperationId::new(6),
            )
            .expect("one operation batch");
        assert_eq!(batch.operations().len(), 1);
        assert_eq!(batch.operations()[0].binding_id(), "kernel.alert");
        assert_eq!(batch.operations()[0].target(), "kernel.alert-state");
        assert_eq!(batch.operations()[0].payload()["results"][0]["target"], 9);

        let mut impossible = readout;
        impossible.selection_comparisons = usize::MAX;
        impossible.distance_rejects = usize::MAX;
        assert!(matches!(
            plan().mutation_batch(
                &impossible,
                MutationBatchId::new("impossible").expect("batch id"),
                MutationCausation::new("runtime-schedule").expect("causation"),
                MutationProvenance::new("product.kernel").expect("provenance"),
                MutationOperationId::new(7),
            ),
            Err(ObservePairsError::InvalidReadout)
        ));
    }

    #[test]
    fn stealth_pressure_selects_typed_roles_and_reports_bounded_geometry_costs() {
        let observer = EntityId::new(30);
        let clear_target = EntityId::new(10);
        let blocked_target = EntityId::new(20);
        let role_like_only = EntityId::new(40);
        let blocker = EntityId::new(50);
        let mut entities = EntityState::from_definitions([
            EntityDefinition::new(blocker, "door")
                .with_transform(Vec3::new(6.0, 0.0, 0.0))
                .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
                .with_collision(true, true),
            EntityDefinition::new(role_like_only, "stealth target")
                .with_transform(Vec3::new(2.0, 0.0, 0.0)),
            EntityDefinition::new(blocked_target, "blocked")
                .with_transform(Vec3::new(8.0, 0.0, 0.0)),
            EntityDefinition::new(clear_target, "clear").with_transform(Vec3::new(4.0, 0.0, 0.0)),
            EntityDefinition::new(observer, "observer").with_transform(Vec3::ZERO),
        ])
        .expect("pressure entities");
        entities
            .register_component(ComponentRegistration::<Vision>::runtime_only(
                ComponentTypeId::parse("product.observer").expect("observer id"),
            ))
            .expect("observer registration");
        entities
            .register_component(ComponentRegistration::<Target>::runtime_only(
                ComponentTypeId::parse("product.target").expect("target id"),
            ))
            .expect("target registration");
        attach(
            &mut entities,
            observer,
            Vision {
                facts: ObservePairsObserverFacts {
                    local_origin: Vec3::ZERO,
                    local_forward: Vec3::new(1.0, 0.0, 0.0),
                    maximum_distance: 12.0,
                    minimum_facing_cosine: 0.5,
                    evidence: 0.25,
                },
            },
        );
        attach(&mut entities, clear_target, Target { center: Vec3::ZERO });
        attach(&mut entities, blocked_target, Target { center: Vec3::ZERO });

        let readout = plan()
            .evaluate::<Vision, Target>(
                &entities,
                &VoxelCollisionScene::from_solid_voxels(1.0, 8, []).expect("empty voxel scene"),
            )
            .expect("closed pressure evaluation");
        assert_eq!(readout.selected_observers, 1);
        assert_eq!(readout.selected_targets, 2);
        assert_eq!(readout.pairs_examined, 2);
        assert_eq!(readout.visibility_casts, 2);
        assert_eq!(readout.occlusion_rejects, 1);
        assert_eq!(readout.visible_pairs, 1);
        assert_eq!(readout.aggregates.len(), 1);
        assert_eq!(readout.aggregates[0].target, clear_target);
        assert_eq!(readout.aggregates[0].evidence_total, 0.25);

        let mut invalid = entities.clone();
        let revision = invalid
            .component_revision::<Vision>(observer)
            .expect("vision revision");
        let invalid_facts = ObservePairsObserverFacts {
            local_origin: Vec3::ZERO,
            local_forward: Vec3::ZERO,
            maximum_distance: 12.0,
            minimum_facing_cosine: 0.5,
            evidence: 0.25,
        };
        EntityAuthoringService
            .replace_component(
                &mut invalid,
                revision,
                observer,
                Vision {
                    facts: invalid_facts,
                },
            )
            .expect("replace with malformed typed fact");
        assert!(matches!(
            plan().evaluate::<Vision, Target>(
                &invalid,
                &VoxelCollisionScene::from_solid_voxels(1.0, 8, [])
                    .expect("empty voxel scene"),
            ),
            Err(ObservePairsError::InvalidObserverFacts(entity)) if entity == observer
        ));
    }
}
