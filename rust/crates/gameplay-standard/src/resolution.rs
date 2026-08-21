//! Typed standard leaves for the existing resolution traversal.
//!
//! These types deliberately do not implement a policy or transaction. A downstream policy keeps
//! its intent, facts, product operations, rejections, and transaction owner; it selects a
//! `Program<StandardPredicate, ComposedOperation<ProductOperation>>` and calls `plan` for the
//! standard leaves. Planning captures role bindings, exact evaluation and catalog provenance,
//! plus component revisions. A product transaction may execute a `StandardMechanicsEffect`
//! against its private candidate before one product-owned publication.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use core_ids::EntityId;
use entity_state::{ComponentAccessError, ComponentRevision, EntityState};
use gameplay_mechanics::{
    ActiveEffectsComponent, DamagePart, DamageReceipt, DamageRequest, DamageService,
    EffectApplyRequest, EffectInstanceId, EffectMutationReceipt, EffectRemovalRequest,
    EffectService, EquipmentComponent, IntrinsicSourcesComponent, ItemComponent, MechanicsCatalog,
    MechanicsComponentKind, MechanicsError, OperationId, RequestSource, SourceInstanceIdentity,
    StatsComponent, TrackId, TrackMutationReceipt, TrackMutationRequest, TrackService,
    TracksComponent, MAX_DAMAGE_PARTS, MAX_DAMAGE_REQUEST_SOURCES,
};

use crate::{
    CapabilityRequirementId, CapabilityRoleId, ExactComparison, ExactEvaluationError,
    ExactEvaluator, ExactExpr, ExactExprLimits, ExactExprRequirements, ExactInputBundle,
    ExactInputReference, RoleRequirement, StandardDefinitionIdentity,
    EXACT_EVALUATOR_SEMANTICS_VERSION, MAX_CAPABILITY_REQUIREMENTS_PER_ROLE,
};

/// Capability required to change an admitted mechanics track.
pub const STANDARD_TRACK_CAPABILITY: &str = "mechanics.track";
/// Capability required to submit a typed mechanics damage request.
pub const STANDARD_DAMAGE_CAPABILITY: &str = "mechanics.damage";
/// Capability required to apply or remove an admitted mechanics effect.
pub const STANDARD_EFFECT_CAPABILITY: &str = "mechanics.effect";
/// Maximum independently bound roles admitted for one standard program execution.
pub const MAX_CAPABILITY_ROLE_BINDINGS: usize = 16;

fn capability(value: &'static str) -> CapabilityRequirementId {
    CapabilityRequirementId::parse(value).expect("fixed standard capability is valid")
}

/// One explicit product-supplied binding from a declared capability role to an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityRoleBinding {
    role: CapabilityRoleId,
    entity: EntityId,
    capabilities: Vec<CapabilityRequirementId>,
}

impl CapabilityRoleBinding {
    pub fn new(
        role: CapabilityRoleId,
        entity: EntityId,
        capabilities: Vec<CapabilityRequirementId>,
    ) -> Result<Self, StandardRoleAdmissionError> {
        if capabilities.len() > MAX_CAPABILITY_REQUIREMENTS_PER_ROLE {
            return Err(StandardRoleAdmissionError::CapabilityQuotaExceeded {
                actual: capabilities.len(),
                maximum: MAX_CAPABILITY_REQUIREMENTS_PER_ROLE,
            });
        }
        Ok(Self {
            role,
            entity,
            capabilities: capabilities
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        })
    }

    pub fn role(&self) -> &CapabilityRoleId {
        &self.role
    }
    pub const fn entity(&self) -> EntityId {
        self.entity
    }
    pub fn capabilities(&self) -> &[CapabilityRequirementId] {
        &self.capabilities
    }
}

/// Admitted typed role bindings. This is not an actor/target model and performs no lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityRoleBindings {
    bindings: BTreeMap<CapabilityRoleId, CapabilityRoleBinding>,
}

impl CapabilityRoleBindings {
    pub fn admit(
        requirements: &[RoleRequirement],
        bindings: Vec<CapabilityRoleBinding>,
    ) -> Result<Self, StandardRoleAdmissionError> {
        if bindings.len() > MAX_CAPABILITY_ROLE_BINDINGS {
            return Err(StandardRoleAdmissionError::RoleBindingQuotaExceeded {
                actual: bindings.len(),
                maximum: MAX_CAPABILITY_ROLE_BINDINGS,
            });
        }
        let mut admitted = BTreeMap::new();
        for binding in bindings {
            if admitted.insert(binding.role.clone(), binding).is_some() {
                return Err(StandardRoleAdmissionError::DuplicateRole);
            }
        }
        for requirement in requirements {
            let binding = admitted.get(requirement.role()).ok_or_else(|| {
                StandardRoleAdmissionError::MissingRole {
                    role: requirement.role().clone(),
                }
            })?;
            for needed in requirement.capabilities() {
                if binding.capabilities.binary_search(needed).is_err() {
                    return Err(StandardRoleAdmissionError::MissingCapability {
                        role: requirement.role().clone(),
                        capability: needed.clone(),
                    });
                }
            }
        }
        Ok(Self { bindings: admitted })
    }

    pub fn entity(&self, role: &CapabilityRoleId) -> Result<EntityId, StandardRoleBindingsError> {
        self.bindings
            .get(role)
            .map(CapabilityRoleBinding::entity)
            .ok_or_else(|| StandardRoleBindingsError::MissingRole { role: role.clone() })
    }

    fn require(
        &self,
        role: &CapabilityRoleId,
        required: CapabilityRequirementId,
    ) -> Result<EntityId, StandardRoleBindingsError> {
        let binding = self
            .bindings
            .get(role)
            .ok_or_else(|| StandardRoleBindingsError::MissingRole { role: role.clone() })?;
        if binding.capabilities.binary_search(&required).is_err() {
            return Err(StandardRoleBindingsError::MissingCapability {
                role: role.clone(),
                capability: required,
            });
        }
        Ok(binding.entity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardRoleAdmissionError {
    DuplicateRole,
    RoleBindingQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    CapabilityQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    MissingRole {
        role: CapabilityRoleId,
    },
    MissingCapability {
        role: CapabilityRoleId,
        capability: CapabilityRequirementId,
    },
}
impl fmt::Display for StandardRoleAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "standard role admission failed: {self:?}")
    }
}
impl std::error::Error for StandardRoleAdmissionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardRoleBindingsError {
    MissingRole {
        role: CapabilityRoleId,
    },
    MissingCapability {
        role: CapabilityRoleId,
        capability: CapabilityRequirementId,
    },
}
impl fmt::Display for StandardRoleBindingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "standard role binding rejected: {self:?}")
    }
}
impl std::error::Error for StandardRoleBindingsError {}

/// A standard predicate is only an exact comparison. Branching remains `Program::When`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardPredicate {
    Exact(ExactComparison),
}
impl StandardPredicate {
    pub fn evaluate(&self, inputs: &ExactInputBundle) -> Result<bool, ExactEvaluationError> {
        match self {
            Self::Exact(comparison) => {
                ExactEvaluator::evaluate_predicate(comparison, inputs, ExactExprLimits::default())
            }
        }
    }
}

/// One standard mechanics operation selected by a downstream program.
#[derive(Debug, Clone)]
pub enum StandardOperation {
    SpendTrack {
        role: CapabilityRoleId,
        track: TrackId,
        amount: StandardExactOperand,
    },
    RestoreTrack {
        role: CapabilityRoleId,
        track: TrackId,
        amount: StandardExactOperand,
    },
    SubmitDamage {
        actor: Option<CapabilityRoleId>,
        target: CapabilityRoleId,
        target_track: TrackId,
        parts: Vec<(StandardExactOperand, gameplay_mechanics::DamageKindId)>,
        request_sources: Vec<RequestSource>,
    },
    ApplyEffect {
        role: CapabilityRoleId,
        instance: EffectInstanceId,
        definition: gameplay_mechanics::EffectDefinitionId,
        stacks: u16,
    },
    RemoveEffect {
        role: CapabilityRoleId,
        instance: EffectInstanceId,
    },
}

/// Caller-owned correlation and provenance supplied for one selected standard leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardOperationContext {
    operation: OperationId,
    source: SourceInstanceIdentity,
}
impl StandardOperationContext {
    pub fn new(
        operation: OperationId,
        source: SourceInstanceIdentity,
    ) -> Result<Self, StandardOperationContextError> {
        if let SourceInstanceIdentity::Request {
            operation: claimed, ..
        } = &source
        {
            if claimed != &operation {
                return Err(StandardOperationContextError::RequestOperationMismatch {
                    context: operation,
                    source: claimed.clone(),
                });
            }
        }
        Ok(Self { operation, source })
    }
    pub fn operation(&self) -> &OperationId {
        &self.operation
    }
    pub fn source(&self) -> &SourceInstanceIdentity {
        &self.source
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardOperationContextError {
    RequestOperationMismatch {
        context: OperationId,
        source: OperationId,
    },
}
impl fmt::Display for StandardOperationContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "standard operation context rejected: {self:?}")
    }
}
impl std::error::Error for StandardOperationContextError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardExactOperand {
    expression: ExactExpr,
    definition_identity: Option<StandardDefinitionIdentity>,
    admitted_roles: Option<Box<[RoleRequirement]>>,
}
impl StandardExactOperand {
    /// Builds a direct product expression. Its inputs require bound roles, but it carries no
    /// package-declared capability contract; product code owns that direct-expression policy.
    pub fn new(expression: ExactExpr) -> Self {
        Self {
            expression,
            definition_identity: None,
            admitted_roles: None,
        }
    }
    /// Preserves the admitted definition's canonical role/capability requirements so planning
    /// cannot treat a supplied input role as authorization on its own.
    pub fn from_admitted(definition: &crate::AdmittedExactDefinition) -> Self {
        Self {
            expression: definition.definition().expression().clone(),
            definition_identity: Some(definition.identity().clone()),
            admitted_roles: Some(definition.definition().roles().to_vec().into_boxed_slice()),
        }
    }
    pub fn expression(&self) -> &ExactExpr {
        &self.expression
    }
    pub fn definition_identity(&self) -> Option<&StandardDefinitionIdentity> {
        self.definition_identity.as_ref()
    }
    pub fn admitted_roles(&self) -> Option<&[RoleRequirement]> {
        self.admitted_roles.as_deref()
    }
}
impl From<ExactExpr> for StandardExactOperand {
    fn from(expression: ExactExpr) -> Self {
        Self::new(expression)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardExactEvaluation {
    semantics_version: u32,
    definition_identity: Option<StandardDefinitionIdentity>,
    expression: ExactExpr,
    requirements: ExactExprRequirements,
    inputs: Vec<(ExactInputReference, gameplay_mechanics::MechanicsScalar)>,
    result: gameplay_mechanics::MechanicsScalar,
}
impl StandardExactEvaluation {
    pub const fn semantics_version(&self) -> u32 {
        self.semantics_version
    }
    pub fn definition_identity(&self) -> Option<&StandardDefinitionIdentity> {
        self.definition_identity.as_ref()
    }
    pub fn expression(&self) -> &ExactExpr {
        &self.expression
    }
    pub fn requirements(&self) -> &ExactExprRequirements {
        &self.requirements
    }
    pub fn inputs(&self) -> &[(ExactInputReference, gameplay_mechanics::MechanicsScalar)] {
        &self.inputs
    }
    pub const fn result(&self) -> gameplay_mechanics::MechanicsScalar {
        self.result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardCatalogProvenance {
    version: gameplay_mechanics::CatalogVersion,
    fingerprint: String,
}
impl StandardCatalogProvenance {
    fn capture(catalog: &MechanicsCatalog) -> Self {
        Self {
            version: catalog.version().clone(),
            fingerprint: catalog.fingerprint().to_string(),
        }
    }
    pub fn version(&self) -> &gameplay_mechanics::CatalogVersion {
        &self.version
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// A staged, typed mechanics request. It cannot mutate until the product transaction elects to
/// execute it against a private candidate.
#[derive(Debug, Clone)]
pub enum StandardMechanicsEffect {
    SpendTrack(TrackMutationRequest),
    RestoreTrack(TrackMutationRequest),
    SubmitDamage(DamageRequest),
    ApplyEffect(EffectApplyRequest),
    RemoveEffect(EffectRemovalRequest),
}

/// Result returned by explicit candidate execution, preserving the mechanics receipt unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardMechanicsReceipt {
    Track(TrackMutationReceipt),
    Damage(DamageReceipt),
    Effect(EffectMutationReceipt),
}

impl StandardMechanicsEffect {
    /// Executes only against a product-owned candidate. It is intentionally not a transaction.
    pub fn apply_to_candidate(
        &self,
        candidate: &mut EntityState,
        catalog: &MechanicsCatalog,
    ) -> Result<StandardMechanicsReceipt, MechanicsError> {
        match self {
            Self::SpendTrack(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<TracksComponent>(request.entity)?);
                TrackService::spend(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Track)
            }
            Self::RestoreTrack(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<TracksComponent>(request.entity)?);
                TrackService::restore(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Track)
            }
            Self::SubmitDamage(request) => {
                let mut request = request.clone();
                request.expected_tracks_revision =
                    Some(candidate.component_revision::<TracksComponent>(request.target)?);
                DamageService::apply(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Damage)
            }
            Self::ApplyEffect(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<ActiveEffectsComponent>(request.entity)?);
                EffectService::apply(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Effect)
            }
            Self::RemoveEffect(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<ActiveEffectsComponent>(request.entity)?);
                EffectService::remove(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Effect)
            }
        }
    }
}

/// One mechanics component slot guarded while planning an operation.
///
/// Standard plans intentionally snapshot a conservative superset of the slots an operation may
/// read. Product transactions validate that source snapshot against authoritative state before
/// cloning a private candidate and rebasing the candidate-private mutation guard.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StandardObservedComponentRevision {
    entity: EntityId,
    component: MechanicsComponentKind,
    revision: u64,
}

impl StandardObservedComponentRevision {
    pub const fn entity(&self) -> EntityId {
        self.entity
    }

    pub const fn component(&self) -> MechanicsComponentKind {
        self.component
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    fn current(&self, state: &EntityState) -> Result<ComponentRevision, ComponentAccessError> {
        match self.component {
            MechanicsComponentKind::Stats => {
                state.component_revision::<StatsComponent>(self.entity)
            }
            MechanicsComponentKind::Tracks => {
                state.component_revision::<TracksComponent>(self.entity)
            }
            MechanicsComponentKind::IntrinsicSources => {
                state.component_revision::<IntrinsicSourcesComponent>(self.entity)
            }
            MechanicsComponentKind::ActiveEffects => {
                state.component_revision::<ActiveEffectsComponent>(self.entity)
            }
            MechanicsComponentKind::Inventory => {
                state.component_revision::<gameplay_mechanics::InventoryComponent>(self.entity)
            }
            MechanicsComponentKind::Item => state.component_revision::<ItemComponent>(self.entity),
            MechanicsComponentKind::Equipment => {
                state.component_revision::<EquipmentComponent>(self.entity)
            }
        }
    }
}

/// Planning readout retaining the complete mechanics source read-set.
#[derive(Debug, Clone)]
pub struct StandardOperationPlan {
    effect: StandardMechanicsEffect,
    observed_revisions: Vec<StandardObservedComponentRevision>,
    catalog: StandardCatalogProvenance,
    exact_evaluations: Vec<StandardExactEvaluation>,
}
impl StandardOperationPlan {
    pub fn effect(&self) -> &StandardMechanicsEffect {
        &self.effect
    }
    pub fn observed_revisions(&self) -> &[StandardObservedComponentRevision] {
        &self.observed_revisions
    }
    pub fn into_effect(self) -> StandardMechanicsEffect {
        self.effect
    }
    pub fn catalog(&self) -> &StandardCatalogProvenance {
        &self.catalog
    }
    pub fn exact_evaluations(&self) -> &[StandardExactEvaluation] {
        &self.exact_evaluations
    }
    pub fn validate_source_state(
        &self,
        state: &EntityState,
        catalog: &MechanicsCatalog,
    ) -> Result<(), StandardPlanValidationError> {
        if self.catalog.version != *catalog.version()
            || self.catalog.fingerprint != catalog.fingerprint()
        {
            return Err(StandardPlanValidationError::CatalogChanged {
                expected: self.catalog.clone(),
                actual: StandardCatalogProvenance::capture(catalog),
            });
        }
        for expected in &self.observed_revisions {
            let actual = expected
                .current(state)
                .map_err(StandardPlanValidationError::Component)?;
            if actual.revision() != expected.revision {
                return Err(StandardPlanValidationError::StaleComponentRevision {
                    expected: expected.clone(),
                    actual,
                });
            }
        }
        Ok(())
    }
}

fn conservative_source_read_set(
    effect: &StandardMechanicsEffect,
    state: &EntityState,
) -> Result<Vec<StandardObservedComponentRevision>, ComponentAccessError> {
    // Do not execute a mutation against a planning clone: source-state feasibility is not
    // candidate-state feasibility, and valid sequential private-candidate programs must plan.
    // Standard leaves instead take a deliberately conservative source snapshot. Every mechanics
    // slot on each participating entity is guarded whether it is presently occupied or absent;
    // currently equipped item entities contribute their Item slots as well.
    let mut entities = BTreeSet::new();
    match effect {
        StandardMechanicsEffect::SpendTrack(request)
        | StandardMechanicsEffect::RestoreTrack(request) => {
            entities.insert(request.entity);
        }
        StandardMechanicsEffect::SubmitDamage(request) => {
            entities.insert(request.target);
            if let Some(actor) = request.actor {
                entities.insert(actor);
            }
        }
        StandardMechanicsEffect::ApplyEffect(request) => {
            entities.insert(request.entity);
        }
        StandardMechanicsEffect::RemoveEffect(request) => {
            entities.insert(request.entity);
        }
    }

    let mut read_set = Vec::new();
    let mut equipped_items = BTreeSet::new();
    for entity in entities {
        for component in MechanicsComponentKind::ALL {
            read_set.push(snapshot_slot(state, entity, component)?);
        }
        if let Some(equipment) = state.component::<EquipmentComponent>(entity)? {
            equipped_items.extend(
                equipment
                    .assignments()
                    .iter()
                    .map(|assignment| assignment.item),
            );
        }
    }
    for item in equipped_items {
        read_set.push(snapshot_slot(state, item, MechanicsComponentKind::Item)?);
    }
    read_set.sort();
    read_set.dedup();
    Ok(read_set)
}

fn snapshot_slot(
    state: &EntityState,
    entity: EntityId,
    component: MechanicsComponentKind,
) -> Result<StandardObservedComponentRevision, ComponentAccessError> {
    let revision = match component {
        MechanicsComponentKind::Stats => state.component_revision::<StatsComponent>(entity)?,
        MechanicsComponentKind::Tracks => state.component_revision::<TracksComponent>(entity)?,
        MechanicsComponentKind::IntrinsicSources => {
            state.component_revision::<IntrinsicSourcesComponent>(entity)?
        }
        MechanicsComponentKind::ActiveEffects => {
            state.component_revision::<ActiveEffectsComponent>(entity)?
        }
        MechanicsComponentKind::Inventory => {
            state.component_revision::<gameplay_mechanics::InventoryComponent>(entity)?
        }
        MechanicsComponentKind::Item => state.component_revision::<ItemComponent>(entity)?,
        MechanicsComponentKind::Equipment => {
            state.component_revision::<EquipmentComponent>(entity)?
        }
    };
    Ok(StandardObservedComponentRevision {
        entity,
        component,
        revision: revision.revision(),
    })
}

impl StandardOperation {
    pub fn requirements(&self) -> Vec<RoleRequirement> {
        let mut roles: BTreeMap<CapabilityRoleId, BTreeSet<CapabilityRequirementId>> =
            BTreeMap::new();
        let mut add = |role: &CapabilityRoleId, required: &'static str| {
            roles
                .entry(role.clone())
                .or_default()
                .insert(capability(required));
        };
        match self {
            Self::SpendTrack { role, .. } | Self::RestoreTrack { role, .. } => {
                add(role, STANDARD_TRACK_CAPABILITY)
            }
            Self::SubmitDamage { actor, target, .. } => {
                if let Some(actor) = actor {
                    add(actor, STANDARD_DAMAGE_CAPABILITY);
                }
                add(target, STANDARD_DAMAGE_CAPABILITY);
            }
            Self::ApplyEffect { role, .. } | Self::RemoveEffect { role, .. } => {
                add(role, STANDARD_EFFECT_CAPABILITY)
            }
        }
        roles
            .into_iter()
            .map(|(role, capabilities)| {
                RoleRequirement::new(role, capabilities.into_iter().collect())
                    .expect("fixed capability count fits")
            })
            .collect()
    }

    pub fn plan(
        &self,
        roles: &CapabilityRoleBindings,
        inputs: &ExactInputBundle,
        state: &EntityState,
        catalog: &MechanicsCatalog,
        context: &StandardOperationContext,
    ) -> Result<StandardOperationPlan, StandardPlanningError> {
        let mut evaluations = Vec::new();
        let mut value = |operand: &StandardExactOperand| {
            if let Some(admitted_roles) = operand.admitted_roles() {
                for requirement in admitted_roles {
                    for required in requirement.capabilities() {
                        roles
                            .require(requirement.role(), required.clone())
                            .map_err(StandardPlanningError::Roles)?;
                    }
                }
            }
            let requirements = ExactExprRequirements::inspect(operand.expression())
                .map_err(StandardPlanningError::Expression)?;
            for input in requirements.inputs() {
                roles
                    .entity(input.role())
                    .map_err(StandardPlanningError::Roles)?;
            }
            let result =
                ExactEvaluator::evaluate(operand.expression(), inputs, ExactExprLimits::default())
                    .map_err(StandardPlanningError::Expression)?;
            let values = requirements
                .inputs()
                .iter()
                .filter_map(|input| inputs.get(input).map(|value| (input.clone(), value)))
                .collect();
            evaluations.push(StandardExactEvaluation {
                semantics_version: EXACT_EVALUATOR_SEMANTICS_VERSION,
                definition_identity: operand.definition_identity().cloned(),
                expression: operand.expression().clone(),
                requirements,
                inputs: values,
                result,
            });
            Ok(result)
        };
        let track_revision = |entity| {
            state
                .component_revision::<TracksComponent>(entity)
                .map_err(StandardPlanningError::Component)
        };
        let effect_revision = |entity| {
            state
                .component_revision::<ActiveEffectsComponent>(entity)
                .map_err(StandardPlanningError::Component)
        };
        let planned = match self {
            Self::SpendTrack {
                role,
                track,
                amount,
            } => {
                let entity = roles
                    .require(role, capability(STANDARD_TRACK_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = track_revision(entity)?;
                (
                    StandardMechanicsEffect::SpendTrack(TrackMutationRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        entity,
                        track: track.clone(),
                        amount: value(amount)?,
                        kind: gameplay_mechanics::TrackAdjustmentKind::Spend,
                        expected_revision: Some(revision.clone()),
                    }),
                    vec![revision],
                )
            }
            Self::RestoreTrack {
                role,
                track,
                amount,
            } => {
                let entity = roles
                    .require(role, capability(STANDARD_TRACK_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = track_revision(entity)?;
                (
                    StandardMechanicsEffect::RestoreTrack(TrackMutationRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        entity,
                        track: track.clone(),
                        amount: value(amount)?,
                        kind: gameplay_mechanics::TrackAdjustmentKind::Restore,
                        expected_revision: Some(revision.clone()),
                    }),
                    vec![revision],
                )
            }
            Self::SubmitDamage {
                actor,
                target,
                target_track,
                parts,
                request_sources,
            } => {
                if parts.is_empty() || parts.len() > MAX_DAMAGE_PARTS {
                    return Err(StandardPlanningError::DamageParts {
                        actual: parts.len(),
                        maximum: MAX_DAMAGE_PARTS,
                    });
                }
                if request_sources.len() > MAX_DAMAGE_REQUEST_SOURCES {
                    return Err(StandardPlanningError::DamageRequestSources {
                        actual: request_sources.len(),
                        maximum: MAX_DAMAGE_REQUEST_SOURCES,
                    });
                }
                let target_entity = roles
                    .require(target, capability(STANDARD_DAMAGE_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let actor = actor
                    .as_ref()
                    .map(|role| roles.require(role, capability(STANDARD_DAMAGE_CAPABILITY)))
                    .transpose()
                    .map_err(StandardPlanningError::Roles)?;
                let revision = track_revision(target_entity)?;
                let mut planned_parts = Vec::with_capacity(parts.len());
                for (amount, kind) in parts {
                    planned_parts.push(DamagePart {
                        amount: value(amount)?,
                        kind: kind.clone(),
                    });
                }
                (
                    StandardMechanicsEffect::SubmitDamage(DamageRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        actor,
                        target: target_entity,
                        target_track: target_track.clone(),
                        parts: planned_parts,
                        request_sources: request_sources.clone(),
                        expected_tracks_revision: Some(revision.clone()),
                    }),
                    vec![revision],
                )
            }
            Self::ApplyEffect {
                role,
                instance,
                definition,
                stacks,
            } => {
                let admitted = catalog.effect(definition).ok_or_else(|| {
                    StandardPlanningError::UnknownEffect {
                        definition: definition.clone(),
                    }
                })?;
                if *stacks == 0 || *stacks > admitted.maximum_stacks {
                    return Err(StandardPlanningError::EffectStacks {
                        actual: *stacks,
                        maximum: admitted.maximum_stacks,
                    });
                }
                let entity = roles
                    .require(role, capability(STANDARD_EFFECT_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = effect_revision(entity)?;
                (
                    StandardMechanicsEffect::ApplyEffect(EffectApplyRequest {
                        operation: context.operation().clone(),
                        entity,
                        instance: instance.clone(),
                        definition: definition.clone(),
                        provenance: context.source().clone(),
                        stacks: *stacks,
                        expected_revision: Some(revision.clone()),
                    }),
                    vec![revision],
                )
            }
            Self::RemoveEffect { role, instance } => {
                let entity = roles
                    .require(role, capability(STANDARD_EFFECT_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = effect_revision(entity)?;
                (
                    StandardMechanicsEffect::RemoveEffect(EffectRemovalRequest {
                        operation: context.operation().clone(),
                        entity,
                        instance: instance.clone(),
                        expected_revision: Some(revision.clone()),
                    }),
                    vec![revision],
                )
            }
        };
        let observed_revisions = conservative_source_read_set(&planned.0, state)
            .map_err(StandardPlanningError::Component)?;
        Ok(StandardOperationPlan {
            effect: planned.0,
            observed_revisions,
            catalog: StandardCatalogProvenance::capture(catalog),
            exact_evaluations: evaluations,
        })
    }
}

#[derive(Debug)]
pub enum StandardPlanningError {
    Roles(StandardRoleBindingsError),
    Expression(ExactEvaluationError),
    Component(ComponentAccessError),
    DamageParts {
        actual: usize,
        maximum: usize,
    },
    DamageRequestSources {
        actual: usize,
        maximum: usize,
    },
    UnknownEffect {
        definition: gameplay_mechanics::EffectDefinitionId,
    },
    EffectStacks {
        actual: u16,
        maximum: u16,
    },
}
impl fmt::Display for StandardPlanningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "standard operation planning failed: {self:?}")
    }
}
impl std::error::Error for StandardPlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardPlanValidationError {
    CatalogChanged {
        expected: StandardCatalogProvenance,
        actual: StandardCatalogProvenance,
    },
    StaleComponentRevision {
        expected: StandardObservedComponentRevision,
        actual: ComponentRevision,
    },
    Component(ComponentAccessError),
}
impl fmt::Display for StandardPlanValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "standard operation plan source validation failed: {self:?}"
        )
    }
}
impl std::error::Error for StandardPlanValidationError {}

/// Closed composition seam that preserves a downstream product leaf's type and error identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposedPredicate<ProductPredicate> {
    Standard(StandardPredicate),
    Product(ProductPredicate),
}
/// Closed composition seam that preserves a downstream product operation's type and identity.
#[derive(Debug, Clone)]
pub enum ComposedOperation<ProductOperation> {
    Standard(StandardOperation),
    Product(ProductOperation),
}
