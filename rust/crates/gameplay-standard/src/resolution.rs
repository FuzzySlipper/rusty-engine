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
    EffectApplyRequest, EffectInstanceId, EffectMutationReceipt, EffectRefreshRequest,
    EffectRemovalRequest, EffectReplaceRequest, EffectService, EffectStackingPolicy,
    EquipmentComponent, EquipmentEquipRequest, EquipmentMutationReceipt, EquipmentService,
    EquipmentSlotId, EquipmentSwapRequest, EquipmentUnequipRequest, IntrinsicSourcesComponent,
    InventoryComponent, InventoryMutationReceipt, InventoryMutationRequest, InventoryService,
    InventoryTransferReceipt, InventoryTransferRequest, ItemComponent, ItemDefinitionId, ItemKind,
    ItemTransferReceipt, ItemTransferRequest, MechanicsCatalog, MechanicsComponentKind,
    MechanicsError, OperationId, RequestSource, SourceInstanceIdentity, StatsComponent, TrackId,
    TrackMutationReceipt, TrackMutationRequest, TrackService, TracksComponent, MAX_DAMAGE_PARTS,
    MAX_DAMAGE_REQUEST_SOURCES, MAX_EQUIPMENT_ASSIGNMENTS,
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
/// Capability required to apply, remove, refresh, or replace an admitted mechanics effect.
pub const STANDARD_EFFECT_CAPABILITY: &str = "mechanics.effect";
/// Capability required to mutate a fungible inventory stack through `InventoryService`.
pub const STANDARD_INVENTORY_CAPABILITY: &str = "mechanics.inventory";
/// Capability required to change a caller-supplied unique item's equipment assignment.
pub const STANDARD_EQUIPMENT_CAPABILITY: &str = "mechanics.equipment";
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
    /// Refresh an existing effect instance selected by a downstream policy.
    ///
    /// The existing instance retains its admitted definition; only Refresh-policy definitions
    /// are eligible. The operation context supplies the new provenance and correlation.
    RefreshEffect {
        role: CapabilityRoleId,
        instance: EffectInstanceId,
        stacks: u16,
    },
    /// Replace every active effect in the selected definition's stacking group.
    ///
    /// A requested instance identity may be reused when its old instance is in that removed
    /// group, but not when an unrelated group still owns it.
    ReplaceEffect {
        role: CapabilityRoleId,
        instance: EffectInstanceId,
        definition: gameplay_mechanics::EffectDefinitionId,
        stacks: u16,
    },
    RemoveEffect {
        role: CapabilityRoleId,
        instance: EffectInstanceId,
    },
    /// Grant a bounded quantity of one admitted fungible item to a bound inventory owner.
    GrantStack {
        role: CapabilityRoleId,
        item: ItemDefinitionId,
        quantity: u64,
    },
    /// Consume a bounded quantity of one admitted fungible item from a bound inventory owner.
    ConsumeStack {
        role: CapabilityRoleId,
        item: ItemDefinitionId,
        quantity: u64,
    },
    /// Transfer a bounded quantity of one admitted fungible item between distinct bound owners.
    TransferStack {
        from: CapabilityRoleId,
        to: CapabilityRoleId,
        item: ItemDefinitionId,
        quantity: u64,
    },
    /// Transfer one caller-supplied unique item between distinct bound inventory owners.
    TransferUniqueItem {
        from: CapabilityRoleId,
        to: CapabilityRoleId,
        item: EntityId,
    },
    /// Assign one caller-supplied unique item to caller-supplied equipment slots.
    EquipUniqueItem {
        role: CapabilityRoleId,
        item: EntityId,
        slots: Vec<gameplay_mechanics::EquipmentSlotId>,
    },
    /// Remove one caller-supplied unique item from all of its current equipment slots.
    UnequipUniqueItem {
        role: CapabilityRoleId,
        item: EntityId,
    },
    /// Replace one caller-supplied equipped item with another in caller-supplied slots.
    SwapUniqueItem {
        role: CapabilityRoleId,
        outgoing_item: EntityId,
        incoming_item: EntityId,
        incoming_slots: Vec<gameplay_mechanics::EquipmentSlotId>,
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
    RefreshEffect(EffectRefreshRequest),
    ReplaceEffect(EffectReplaceRequest),
    RemoveEffect(EffectRemovalRequest),
    GrantStack(InventoryMutationRequest),
    ConsumeStack(InventoryMutationRequest),
    TransferStack(InventoryTransferRequest),
    TransferUniqueItem(ItemTransferRequest),
    EquipUniqueItem(EquipmentEquipRequest),
    UnequipUniqueItem(EquipmentUnequipRequest),
    SwapUniqueItem(EquipmentSwapRequest),
}

/// Result returned by explicit candidate execution, preserving the mechanics receipt unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardMechanicsReceipt {
    Track(TrackMutationReceipt),
    Damage(DamageReceipt),
    Effect(EffectMutationReceipt),
    Inventory(InventoryMutationReceipt),
    InventoryTransfer(InventoryTransferReceipt),
    UniqueItemTransfer(ItemTransferReceipt),
    Equipment(EquipmentMutationReceipt),
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
            Self::RefreshEffect(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<ActiveEffectsComponent>(request.entity)?);
                EffectService::refresh(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Effect)
            }
            Self::ReplaceEffect(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<ActiveEffectsComponent>(request.entity)?);
                EffectService::replace(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Effect)
            }
            Self::RemoveEffect(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<ActiveEffectsComponent>(request.entity)?);
                EffectService::remove(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Effect)
            }
            Self::GrantStack(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<InventoryComponent>(request.owner)?);
                InventoryService::grant(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Inventory)
            }
            Self::ConsumeStack(request) => {
                let mut request = request.clone();
                request.expected_revision =
                    Some(candidate.component_revision::<InventoryComponent>(request.owner)?);
                InventoryService::consume(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Inventory)
            }
            Self::TransferStack(request) => {
                let mut request = request.clone();
                request.expected_from_revision =
                    Some(candidate.component_revision::<InventoryComponent>(request.from_owner)?);
                request.expected_to_revision =
                    Some(candidate.component_revision::<InventoryComponent>(request.to_owner)?);
                InventoryService::transfer(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::InventoryTransfer)
            }
            Self::TransferUniqueItem(request) => {
                let mut request = request.clone();
                request.expected_relationship_revision = candidate.revision();
                request.expected_from_inventory_revision =
                    Some(candidate.component_revision::<InventoryComponent>(request.from_owner)?);
                request.expected_to_inventory_revision =
                    Some(candidate.component_revision::<InventoryComponent>(request.to_owner)?);
                EquipmentService::transfer_unique_item(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::UniqueItemTransfer)
            }
            Self::EquipUniqueItem(request) => {
                let mut request = request.clone();
                request.expected_state_revision = candidate.revision();
                request.expected_equipment_revision =
                    Some(candidate.component_revision::<EquipmentComponent>(request.owner)?);
                EquipmentService::equip(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Equipment)
            }
            Self::UnequipUniqueItem(request) => {
                let mut request = request.clone();
                request.expected_state_revision = candidate.revision();
                request.expected_equipment_revision =
                    Some(candidate.component_revision::<EquipmentComponent>(request.owner)?);
                EquipmentService::unequip(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Equipment)
            }
            Self::SwapUniqueItem(request) => {
                let mut request = request.clone();
                request.expected_state_revision = candidate.revision();
                request.expected_equipment_revision =
                    Some(candidate.component_revision::<EquipmentComponent>(request.owner)?);
                EquipmentService::swap(candidate, catalog, request)
                    .map(StandardMechanicsReceipt::Equipment)
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
    observed_state_revision: Option<u64>,
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
    /// The relationship/index revision observed by inventory and equipment operations whose
    /// semantics read containment or equipment relationships. Mechanics leaves that do not read
    /// those relationships intentionally omit this global guard.
    pub const fn observed_state_revision(&self) -> Option<u64> {
        self.observed_state_revision
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
        if let Some(expected) = self.observed_state_revision {
            let actual = state.revision();
            if actual != expected {
                return Err(StandardPlanValidationError::StaleStateRevision { expected, actual });
            }
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
    // currently equipped item entities contribute their Item slots as well. Inventory capacity
    // additionally reads directly contained unique-item slots, but that expansion belongs only
    // to inventory operations rather than every standard mechanics leaf.
    let mut entities = BTreeSet::new();
    let mut inventory_owners = BTreeSet::new();
    let mut explicit_items = BTreeSet::new();
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
        StandardMechanicsEffect::RefreshEffect(request) => {
            entities.insert(request.entity);
        }
        StandardMechanicsEffect::ReplaceEffect(request) => {
            entities.insert(request.entity);
        }
        StandardMechanicsEffect::RemoveEffect(request) => {
            entities.insert(request.entity);
        }
        StandardMechanicsEffect::GrantStack(request)
        | StandardMechanicsEffect::ConsumeStack(request) => {
            entities.insert(request.owner);
            inventory_owners.insert(request.owner);
        }
        StandardMechanicsEffect::TransferStack(request) => {
            entities.insert(request.from_owner);
            entities.insert(request.to_owner);
            inventory_owners.insert(request.from_owner);
            inventory_owners.insert(request.to_owner);
        }
        StandardMechanicsEffect::TransferUniqueItem(request) => {
            entities.insert(request.from_owner);
            entities.insert(request.to_owner);
            inventory_owners.insert(request.from_owner);
            inventory_owners.insert(request.to_owner);
            explicit_items.insert(request.item);
        }
        StandardMechanicsEffect::EquipUniqueItem(request) => {
            entities.insert(request.owner);
            explicit_items.insert(request.item);
        }
        StandardMechanicsEffect::UnequipUniqueItem(request) => {
            entities.insert(request.owner);
            explicit_items.insert(request.item);
        }
        StandardMechanicsEffect::SwapUniqueItem(request) => {
            entities.insert(request.owner);
            explicit_items.insert(request.outgoing_item);
            explicit_items.insert(request.incoming_item);
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
        if inventory_owners.contains(&entity) {
            // Inventory capacity reads the Item slots of every directly contained unique item.
            // Capture absent slots too: attaching or removing one changes capacity semantics.
            equipped_items.extend(state.contained_entities(entity));
        }
    }
    for item in equipped_items {
        read_set.push(snapshot_slot(state, item, MechanicsComponentKind::Item)?);
    }
    for item in explicit_items {
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
            Self::ApplyEffect { role, .. }
            | Self::RefreshEffect { role, .. }
            | Self::ReplaceEffect { role, .. }
            | Self::RemoveEffect { role, .. } => add(role, STANDARD_EFFECT_CAPABILITY),
            Self::GrantStack { role, .. } | Self::ConsumeStack { role, .. } => {
                add(role, STANDARD_INVENTORY_CAPABILITY)
            }
            Self::TransferStack { from, to, .. } => {
                add(from, STANDARD_INVENTORY_CAPABILITY);
                add(to, STANDARD_INVENTORY_CAPABILITY);
            }
            Self::TransferUniqueItem { from, to, .. } => {
                add(from, STANDARD_INVENTORY_CAPABILITY);
                add(to, STANDARD_INVENTORY_CAPABILITY);
            }
            Self::EquipUniqueItem { role, .. }
            | Self::UnequipUniqueItem { role, .. }
            | Self::SwapUniqueItem { role, .. } => add(role, STANDARD_EQUIPMENT_CAPABILITY),
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
        let inventory_revision = |entity| {
            state
                .component_revision::<InventoryComponent>(entity)
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
            Self::RefreshEffect {
                role,
                instance,
                stacks,
            } => {
                let entity = roles
                    .require(role, capability(STANDARD_EFFECT_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = effect_revision(entity)?;
                let existing = active_effect(state, entity, instance)?;
                let definition = standard_effect_definition(catalog, existing.definition())?;
                ensure_effect_policy(definition, "refresh", EffectStackingPolicy::Refresh)?;
                validate_effect_stacks(definition, *stacks)?;
                (
                    StandardMechanicsEffect::RefreshEffect(EffectRefreshRequest {
                        operation: context.operation().clone(),
                        entity,
                        instance: instance.clone(),
                        provenance: context.source().clone(),
                        stacks: *stacks,
                        expected_revision: Some(revision.clone()),
                    }),
                    vec![revision],
                )
            }
            Self::ReplaceEffect {
                role,
                instance,
                definition,
                stacks,
            } => {
                let entity = roles
                    .require(role, capability(STANDARD_EFFECT_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = effect_revision(entity)?;
                let replacement = standard_effect_definition(catalog, definition)?;
                ensure_effect_policy(replacement, "replace", EffectStackingPolicy::Replace)?;
                validate_effect_stacks(replacement, *stacks)?;
                let active = active_effects(state, entity)?;
                for existing in active.effects() {
                    let active_definition =
                        standard_effect_definition(catalog, existing.definition())?;
                    if existing.instance() == instance
                        && active_definition.stacking_group != replacement.stacking_group
                    {
                        return Err(StandardPlanningError::EffectInstanceConflict {
                            entity,
                            instance: instance.clone(),
                        });
                    }
                }
                (
                    StandardMechanicsEffect::ReplaceEffect(EffectReplaceRequest {
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
            Self::GrantStack {
                role,
                item,
                quantity,
            } => {
                validate_fungible_stack(catalog, item, *quantity)?;
                let entity = roles
                    .require(role, capability(STANDARD_INVENTORY_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = inventory_revision(entity)?;
                (
                    StandardMechanicsEffect::GrantStack(InventoryMutationRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        owner: entity,
                        item: item.clone(),
                        quantity: *quantity,
                        expected_revision: Some(revision.clone()),
                    }),
                    vec![revision],
                )
            }
            Self::ConsumeStack {
                role,
                item,
                quantity,
            } => {
                validate_fungible_stack(catalog, item, *quantity)?;
                let entity = roles
                    .require(role, capability(STANDARD_INVENTORY_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = inventory_revision(entity)?;
                (
                    StandardMechanicsEffect::ConsumeStack(InventoryMutationRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        owner: entity,
                        item: item.clone(),
                        quantity: *quantity,
                        expected_revision: Some(revision.clone()),
                    }),
                    vec![revision],
                )
            }
            Self::TransferStack {
                from,
                to,
                item,
                quantity,
            } => {
                validate_fungible_stack(catalog, item, *quantity)?;
                let from_owner = roles
                    .require(from, capability(STANDARD_INVENTORY_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let to_owner = roles
                    .require(to, capability(STANDARD_INVENTORY_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                if from_owner == to_owner {
                    return Err(StandardPlanningError::InventoryOwnerConflict {
                        owner: from_owner,
                    });
                }
                let from_revision = inventory_revision(from_owner)?;
                let to_revision = inventory_revision(to_owner)?;
                (
                    StandardMechanicsEffect::TransferStack(InventoryTransferRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        from_owner,
                        to_owner,
                        item: item.clone(),
                        quantity: *quantity,
                        expected_from_revision: Some(from_revision.clone()),
                        expected_to_revision: Some(to_revision.clone()),
                    }),
                    vec![from_revision, to_revision],
                )
            }
            Self::TransferUniqueItem { from, to, item } => {
                let from_owner = roles
                    .require(from, capability(STANDARD_INVENTORY_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let to_owner = roles
                    .require(to, capability(STANDARD_INVENTORY_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                if from_owner == to_owner {
                    return Err(StandardPlanningError::InventoryOwnerConflict {
                        owner: from_owner,
                    });
                }
                let from_revision = inventory_revision(from_owner)?;
                let to_revision = inventory_revision(to_owner)?;
                (
                    StandardMechanicsEffect::TransferUniqueItem(ItemTransferRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        item: *item,
                        from_owner,
                        to_owner,
                        expected_relationship_revision: state.revision(),
                        expected_from_inventory_revision: Some(from_revision.clone()),
                        expected_to_inventory_revision: Some(to_revision.clone()),
                    }),
                    vec![from_revision, to_revision],
                )
            }
            Self::EquipUniqueItem { role, item, slots } => {
                validate_equipment_slots(slots)?;
                let owner = roles
                    .require(role, capability(STANDARD_EQUIPMENT_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = state
                    .component_revision::<EquipmentComponent>(owner)
                    .map_err(StandardPlanningError::Component)?;
                (
                    StandardMechanicsEffect::EquipUniqueItem(EquipmentEquipRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        owner,
                        item: *item,
                        slots: slots.clone(),
                        expected_equipment_revision: Some(revision.clone()),
                        expected_state_revision: state.revision(),
                    }),
                    vec![revision],
                )
            }
            Self::UnequipUniqueItem { role, item } => {
                let owner = roles
                    .require(role, capability(STANDARD_EQUIPMENT_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = state
                    .component_revision::<EquipmentComponent>(owner)
                    .map_err(StandardPlanningError::Component)?;
                (
                    StandardMechanicsEffect::UnequipUniqueItem(EquipmentUnequipRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        owner,
                        item: *item,
                        expected_equipment_revision: Some(revision.clone()),
                        expected_state_revision: state.revision(),
                    }),
                    vec![revision],
                )
            }
            Self::SwapUniqueItem {
                role,
                outgoing_item,
                incoming_item,
                incoming_slots,
            } => {
                if outgoing_item == incoming_item {
                    return Err(StandardPlanningError::EquipmentSwapSameItem {
                        item: *incoming_item,
                    });
                }
                validate_equipment_slots(incoming_slots)?;
                let owner = roles
                    .require(role, capability(STANDARD_EQUIPMENT_CAPABILITY))
                    .map_err(StandardPlanningError::Roles)?;
                let revision = state
                    .component_revision::<EquipmentComponent>(owner)
                    .map_err(StandardPlanningError::Component)?;
                (
                    StandardMechanicsEffect::SwapUniqueItem(EquipmentSwapRequest {
                        operation: context.operation().clone(),
                        source: context.source().clone(),
                        owner,
                        outgoing_item: *outgoing_item,
                        incoming_item: *incoming_item,
                        incoming_slots: incoming_slots.clone(),
                        expected_equipment_revision: Some(revision.clone()),
                        expected_state_revision: state.revision(),
                    }),
                    vec![revision],
                )
            }
        };
        let observes_relationships = matches!(
            &planned.0,
            StandardMechanicsEffect::GrantStack(_)
                | StandardMechanicsEffect::ConsumeStack(_)
                | StandardMechanicsEffect::TransferStack(_)
                | StandardMechanicsEffect::TransferUniqueItem(_)
                | StandardMechanicsEffect::EquipUniqueItem(_)
                | StandardMechanicsEffect::UnequipUniqueItem(_)
                | StandardMechanicsEffect::SwapUniqueItem(_)
        );
        let observed_revisions = conservative_source_read_set(&planned.0, state)
            .map_err(StandardPlanningError::Component)?;
        Ok(StandardOperationPlan {
            effect: planned.0,
            observed_revisions,
            catalog: StandardCatalogProvenance::capture(catalog),
            exact_evaluations: evaluations,
            observed_state_revision: observes_relationships.then(|| state.revision()),
        })
    }
}

fn validate_fungible_stack(
    catalog: &MechanicsCatalog,
    item: &ItemDefinitionId,
    quantity: u64,
) -> Result<(), StandardPlanningError> {
    let definition = catalog
        .item(item)
        .ok_or_else(|| StandardPlanningError::UnknownItem { item: item.clone() })?;
    if definition.kind != ItemKind::Fungible {
        return Err(StandardPlanningError::InventoryItemKind {
            item: item.clone(),
            actual: definition.kind,
        });
    }
    if quantity == 0 || quantity > definition.maximum_quantity {
        return Err(StandardPlanningError::InventoryQuantity {
            item: item.clone(),
            quantity,
            maximum: definition.maximum_quantity,
        });
    }
    Ok(())
}

fn active_effects(
    state: &EntityState,
    entity: EntityId,
) -> Result<&ActiveEffectsComponent, StandardPlanningError> {
    state
        .component::<ActiveEffectsComponent>(entity)
        .map_err(StandardPlanningError::Component)?
        .ok_or(StandardPlanningError::MissingActiveEffects { entity })
}

fn active_effect<'a>(
    state: &'a EntityState,
    entity: EntityId,
    instance: &EffectInstanceId,
) -> Result<&'a gameplay_mechanics::ActiveEffectInstance, StandardPlanningError> {
    active_effects(state, entity)?
        .effects()
        .iter()
        .find(|effect| effect.instance() == instance)
        .ok_or_else(|| StandardPlanningError::MissingEffectInstance {
            entity,
            instance: instance.clone(),
        })
}

fn standard_effect_definition<'a>(
    catalog: &'a MechanicsCatalog,
    definition: &gameplay_mechanics::EffectDefinitionId,
) -> Result<&'a gameplay_mechanics::EffectDefinition, StandardPlanningError> {
    catalog
        .effect(definition)
        .ok_or_else(|| StandardPlanningError::UnknownEffect {
            definition: definition.clone(),
        })
}

fn ensure_effect_policy(
    definition: &gameplay_mechanics::EffectDefinition,
    expected: &'static str,
    policy: EffectStackingPolicy,
) -> Result<(), StandardPlanningError> {
    if definition.stacking != policy {
        return Err(StandardPlanningError::EffectPolicyMismatch {
            effect: definition.id.clone(),
            expected,
            actual: definition.stacking,
        });
    }
    Ok(())
}

fn validate_effect_stacks(
    definition: &gameplay_mechanics::EffectDefinition,
    stacks: u16,
) -> Result<(), StandardPlanningError> {
    if stacks == 0 || stacks > definition.maximum_stacks {
        return Err(StandardPlanningError::EffectStacks {
            actual: stacks,
            maximum: definition.maximum_stacks,
        });
    }
    Ok(())
}

fn validate_equipment_slots(slots: &[EquipmentSlotId]) -> Result<(), StandardPlanningError> {
    if slots.len() > MAX_EQUIPMENT_ASSIGNMENTS {
        return Err(StandardPlanningError::EquipmentSlots {
            actual: slots.len(),
            maximum: MAX_EQUIPMENT_ASSIGNMENTS,
        });
    }
    let mut unique = BTreeSet::new();
    for slot in slots {
        if !unique.insert(slot) {
            return Err(StandardPlanningError::DuplicateEquipmentSlot { slot: slot.clone() });
        }
    }
    Ok(())
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
    MissingActiveEffects {
        entity: EntityId,
    },
    MissingEffectInstance {
        entity: EntityId,
        instance: EffectInstanceId,
    },
    EffectPolicyMismatch {
        effect: gameplay_mechanics::EffectDefinitionId,
        expected: &'static str,
        actual: EffectStackingPolicy,
    },
    EffectInstanceConflict {
        entity: EntityId,
        instance: EffectInstanceId,
    },
    EffectStacks {
        actual: u16,
        maximum: u16,
    },
    UnknownItem {
        item: ItemDefinitionId,
    },
    InventoryItemKind {
        item: ItemDefinitionId,
        actual: ItemKind,
    },
    InventoryQuantity {
        item: ItemDefinitionId,
        quantity: u64,
        maximum: u64,
    },
    InventoryOwnerConflict {
        owner: EntityId,
    },
    EquipmentSlots {
        actual: usize,
        maximum: usize,
    },
    DuplicateEquipmentSlot {
        slot: EquipmentSlotId,
    },
    EquipmentSwapSameItem {
        item: EntityId,
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
    StaleStateRevision {
        expected: u64,
        actual: u64,
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
