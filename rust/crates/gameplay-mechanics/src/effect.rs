use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use entity_state::{ComponentRevision, EntityAuthoringService, EntityComponent, EntityState};
use serde::{Deserialize, Serialize};

use crate::{
    source::ensure_catalog_version, CatalogVersion, EffectDefinition, EffectDefinitionId,
    EffectInstanceId, EffectStackingPolicy, MechanicsCatalog, MechanicsComponentDataError,
    MechanicsComponentKind, MechanicsError, ObservedComponentRevision, OperationId,
    SourceCollectionCost, SourceDefinitionId, SourceInstanceIdentity, StackingGroupId,
    MAX_EFFECT_STACKS,
};

pub const MAX_ACTIVE_EFFECT_INSTANCES: usize = 64;
pub const MAX_EFFECT_SOURCE_ACTIVATIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ActiveEffectInstance {
    instance: EffectInstanceId,
    definition: EffectDefinitionId,
    provenance: SourceInstanceIdentity,
    stacks: u16,
}

impl ActiveEffectInstance {
    pub fn new(
        instance: EffectInstanceId,
        definition: EffectDefinitionId,
        provenance: SourceInstanceIdentity,
        stacks: u16,
    ) -> Result<Self, MechanicsComponentDataError> {
        if stacks == 0 || stacks > MAX_EFFECT_STACKS {
            return Err(MechanicsComponentDataError::InvalidEffectStacks {
                instance,
                stacks,
                maximum: MAX_EFFECT_STACKS,
            });
        }
        Ok(Self {
            instance,
            definition,
            provenance,
            stacks,
        })
    }

    pub const fn instance(&self) -> &EffectInstanceId {
        &self.instance
    }

    pub const fn definition(&self) -> &EffectDefinitionId {
        &self.definition
    }

    pub const fn provenance(&self) -> &SourceInstanceIdentity {
        &self.provenance
    }

    pub const fn stacks(&self) -> u16 {
        self.stacks
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ActiveEffectsComponent {
    catalog_version: CatalogVersion,
    effects: Vec<ActiveEffectInstance>,
}

impl ActiveEffectsComponent {
    pub const LABEL: &'static str = "ActiveEffectsComponent";

    pub fn new(
        catalog_version: CatalogVersion,
        mut effects: Vec<ActiveEffectInstance>,
    ) -> Result<Self, MechanicsComponentDataError> {
        effects.sort_by(|left, right| left.instance.cmp(&right.instance));
        if effects.len() > MAX_ACTIVE_EFFECT_INSTANCES {
            return Err(MechanicsComponentDataError::QuotaExceeded {
                field: "activeEffects",
                actual: effects.len(),
                maximum: MAX_ACTIVE_EFFECT_INSTANCES,
            });
        }
        for pair in effects.windows(2) {
            if pair[0].instance == pair[1].instance {
                return Err(MechanicsComponentDataError::DuplicateIdentity {
                    field: "activeEffects",
                    identity: pair[0].instance.to_string(),
                });
            }
        }
        if let Some(effect) = effects
            .iter()
            .find(|effect| effect.stacks == 0 || effect.stacks > MAX_EFFECT_STACKS)
        {
            return Err(MechanicsComponentDataError::InvalidEffectStacks {
                instance: effect.instance.clone(),
                stacks: effect.stacks,
                maximum: MAX_EFFECT_STACKS,
            });
        }
        Ok(Self {
            catalog_version,
            effects,
        })
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn effects(&self) -> &[ActiveEffectInstance] {
        &self.effects
    }
}

impl EntityComponent for ActiveEffectsComponent {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectMutationKind {
    Apply,
    Refresh,
    Replace,
    Remove,
    Expire,
}

#[derive(Debug, Clone)]
pub struct EffectApplyRequest {
    pub operation: OperationId,
    pub entity: EntityId,
    pub instance: EffectInstanceId,
    pub definition: EffectDefinitionId,
    pub provenance: SourceInstanceIdentity,
    pub stacks: u16,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone)]
pub struct EffectRefreshRequest {
    pub operation: OperationId,
    pub entity: EntityId,
    pub instance: EffectInstanceId,
    pub provenance: SourceInstanceIdentity,
    pub stacks: u16,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone)]
pub struct EffectReplaceRequest {
    pub operation: OperationId,
    pub entity: EntityId,
    pub instance: EffectInstanceId,
    pub definition: EffectDefinitionId,
    pub provenance: SourceInstanceIdentity,
    pub stacks: u16,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone)]
pub struct EffectRemovalRequest {
    pub operation: OperationId,
    pub entity: EntityId,
    pub instance: EffectInstanceId,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSourceActivation {
    pub identity: SourceInstanceIdentity,
    pub definition: SourceDefinitionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectMutationReceipt {
    pub catalog_version: CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub entity: EntityId,
    pub kind: EffectMutationKind,
    pub removed: Vec<ActiveEffectInstance>,
    pub current: Option<ActiveEffectInstance>,
    pub activated_sources: Vec<EffectSourceActivation>,
    pub observed_effects_revision: u64,
    pub committed_effects_revision: u64,
    pub observed_revisions: Vec<ObservedComponentRevision>,
    pub tracks_validated: usize,
    pub source_cost: SourceCollectionCost,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EffectService;

impl EffectService {
    pub fn apply(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EffectApplyRequest,
    ) -> Result<EffectMutationReceipt, MechanicsError> {
        let loaded = LoadedEffects::read(
            state,
            catalog,
            request.entity,
            request.expected_revision.as_ref(),
        )?;
        let definition = require_effect(catalog, &request.definition)?;
        let current = ActiveEffectInstance::new(
            request.instance.clone(),
            request.definition,
            request.provenance,
            request.stacks,
        )?;
        validate_effect_stacks(definition, &current)?;
        if loaded
            .component
            .effects
            .binary_search_by(|effect| effect.instance.cmp(&request.instance))
            .is_ok()
        {
            return Err(MechanicsError::DuplicateEffectInstance {
                entity: request.entity,
                instance: request.instance,
            });
        }

        let matching: Vec<_> = loaded
            .component
            .effects
            .iter()
            .filter(|effect| {
                catalog
                    .effect(&effect.definition)
                    .is_some_and(|active| active.stacking_group == definition.stacking_group)
            })
            .collect();
        match definition.stacking {
            EffectStackingPolicy::IndependentByProvenance { maximum_instances } => {
                if matching
                    .iter()
                    .any(|effect| effect.provenance == current.provenance)
                {
                    return Err(MechanicsError::EffectProvenanceConflict {
                        entity: request.entity,
                        group: definition.stacking_group.clone(),
                        provenance: current.provenance.clone(),
                    });
                }
                if matching.len() >= usize::from(maximum_instances) {
                    return Err(MechanicsError::EffectGroupLimitExceeded {
                        entity: request.entity,
                        group: definition.stacking_group.clone(),
                        actual: matching.len() + 1,
                        maximum: maximum_instances,
                    });
                }
            }
            EffectStackingPolicy::Refresh | EffectStackingPolicy::Replace
                if !matching.is_empty() =>
            {
                return Err(MechanicsError::EffectStackingConflict {
                    entity: request.entity,
                    group: definition.stacking_group.clone(),
                    policy: definition.stacking,
                });
            }
            EffectStackingPolicy::Refresh | EffectStackingPolicy::Replace => {}
        }

        let mut effects = loaded.component.effects.clone();
        effects.push(current.clone());
        Self::publish(
            state,
            catalog,
            request.operation,
            request.entity,
            EffectMutationKind::Apply,
            loaded,
            effects,
            Vec::new(),
            Some(current),
        )
    }

    pub fn refresh(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EffectRefreshRequest,
    ) -> Result<EffectMutationReceipt, MechanicsError> {
        let loaded = LoadedEffects::read(
            state,
            catalog,
            request.entity,
            request.expected_revision.as_ref(),
        )?;
        let index = loaded
            .component
            .effects
            .binary_search_by(|effect| effect.instance.cmp(&request.instance))
            .map_err(|_| MechanicsError::MissingEffectInstance {
                entity: request.entity,
                instance: request.instance.clone(),
            })?;
        let previous = loaded.component.effects[index].clone();
        let definition = require_effect(catalog, &previous.definition)?;
        if definition.stacking != EffectStackingPolicy::Refresh {
            return Err(MechanicsError::EffectPolicyMismatch {
                effect: previous.definition.clone(),
                expected: "refresh",
                actual: definition.stacking,
            });
        }
        let current = ActiveEffectInstance::new(
            previous.instance.clone(),
            previous.definition.clone(),
            request.provenance,
            request.stacks,
        )?;
        validate_effect_stacks(definition, &current)?;
        let mut effects = loaded.component.effects.clone();
        effects[index] = current.clone();
        Self::publish(
            state,
            catalog,
            request.operation,
            request.entity,
            EffectMutationKind::Refresh,
            loaded,
            effects,
            vec![previous],
            Some(current),
        )
    }

    pub fn replace(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EffectReplaceRequest,
    ) -> Result<EffectMutationReceipt, MechanicsError> {
        let loaded = LoadedEffects::read(
            state,
            catalog,
            request.entity,
            request.expected_revision.as_ref(),
        )?;
        let definition = require_effect(catalog, &request.definition)?;
        if definition.stacking != EffectStackingPolicy::Replace {
            return Err(MechanicsError::EffectPolicyMismatch {
                effect: request.definition,
                expected: "replace",
                actual: definition.stacking,
            });
        }
        let current = ActiveEffectInstance::new(
            request.instance,
            definition.id.clone(),
            request.provenance,
            request.stacks,
        )?;
        validate_effect_stacks(definition, &current)?;

        let mut removed = Vec::new();
        let mut effects = Vec::with_capacity(loaded.component.effects.len() + 1);
        for effect in &loaded.component.effects {
            let active_definition = require_effect(catalog, &effect.definition)?;
            if active_definition.stacking_group == definition.stacking_group {
                removed.push(effect.clone());
            } else {
                effects.push(effect.clone());
            }
        }
        if effects
            .iter()
            .any(|effect| effect.instance == current.instance)
        {
            return Err(MechanicsError::DuplicateEffectInstance {
                entity: request.entity,
                instance: current.instance.clone(),
            });
        }
        effects.push(current.clone());
        Self::publish(
            state,
            catalog,
            request.operation,
            request.entity,
            EffectMutationKind::Replace,
            loaded,
            effects,
            removed,
            Some(current),
        )
    }

    pub fn remove(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EffectRemovalRequest,
    ) -> Result<EffectMutationReceipt, MechanicsError> {
        Self::remove_with_kind(state, catalog, request, EffectMutationKind::Remove)
    }

    pub fn expire(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EffectRemovalRequest,
    ) -> Result<EffectMutationReceipt, MechanicsError> {
        Self::remove_with_kind(state, catalog, request, EffectMutationKind::Expire)
    }

    fn remove_with_kind(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EffectRemovalRequest,
        kind: EffectMutationKind,
    ) -> Result<EffectMutationReceipt, MechanicsError> {
        let loaded = LoadedEffects::read(
            state,
            catalog,
            request.entity,
            request.expected_revision.as_ref(),
        )?;
        let index = loaded
            .component
            .effects
            .binary_search_by(|effect| effect.instance.cmp(&request.instance))
            .map_err(|_| MechanicsError::MissingEffectInstance {
                entity: request.entity,
                instance: request.instance,
            })?;
        let removed = loaded.component.effects[index].clone();
        let mut effects = loaded.component.effects.clone();
        effects.remove(index);
        Self::publish(
            state,
            catalog,
            request.operation,
            request.entity,
            kind,
            loaded,
            effects,
            vec![removed],
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn publish(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        operation: OperationId,
        entity: EntityId,
        kind: EffectMutationKind,
        loaded: LoadedEffects,
        effects: Vec<ActiveEffectInstance>,
        mut removed: Vec<ActiveEffectInstance>,
        current: Option<ActiveEffectInstance>,
    ) -> Result<EffectMutationReceipt, MechanicsError> {
        let candidate = ActiveEffectsComponent::new(catalog.version().clone(), effects)
            .map_err(MechanicsError::from)?;
        validate_active_effects_against_catalog(entity, &candidate, catalog)?;
        let (tracks_validated, mut observed_revisions, source_cost) =
            crate::stat::validate_tracks_with_effects_override(
                state, catalog, entity, &operation, &candidate,
            )
            .map_err(map_bound_reconciliation_error)?;
        let activated_sources = current
            .as_ref()
            .map(|effect| effect_source_activations(entity, effect, catalog))
            .transpose()?
            .unwrap_or_default();

        EntityAuthoringService.replace_component(
            state,
            loaded.publish_revision,
            entity,
            candidate,
        )?;
        let committed_revision = state.component_revision::<ActiveEffectsComponent>(entity)?;
        observed_revisions.push(ObservedComponentRevision {
            entity,
            component: MechanicsComponentKind::ActiveEffects,
            revision: loaded.actual_revision.revision(),
        });
        canonicalize_observed_revisions(&mut observed_revisions);
        removed.sort_by(|left, right| left.instance.cmp(&right.instance));

        Ok(EffectMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation,
            entity,
            kind,
            removed,
            current,
            activated_sources,
            observed_effects_revision: loaded.actual_revision.revision(),
            committed_effects_revision: committed_revision.revision(),
            observed_revisions,
            tracks_validated,
            source_cost,
        })
    }
}

struct LoadedEffects {
    actual_revision: ComponentRevision,
    publish_revision: ComponentRevision,
    component: ActiveEffectsComponent,
}

impl LoadedEffects {
    fn read(
        state: &EntityState,
        catalog: &MechanicsCatalog,
        entity: EntityId,
        expected_revision: Option<&ComponentRevision>,
    ) -> Result<Self, MechanicsError> {
        let actual_revision = state.component_revision::<ActiveEffectsComponent>(entity)?;
        if let Some(expected) = expected_revision {
            crate::track::ensure_revision(expected, &actual_revision)?;
        }
        let component = state.component::<ActiveEffectsComponent>(entity)?.ok_or(
            MechanicsError::MissingComponent {
                entity,
                component: ActiveEffectsComponent::LABEL,
            },
        )?;
        ensure_catalog_version(
            catalog,
            entity,
            ActiveEffectsComponent::LABEL,
            component.catalog_version(),
        )?;
        validate_active_effects_against_catalog(entity, component, catalog)?;
        Ok(Self {
            publish_revision: expected_revision
                .cloned()
                .unwrap_or_else(|| actual_revision.clone()),
            actual_revision,
            component: component.clone(),
        })
    }
}

pub(crate) fn validate_active_effects_against_catalog(
    entity: EntityId,
    component: &ActiveEffectsComponent,
    catalog: &MechanicsCatalog,
) -> Result<(), MechanicsError> {
    let mut group_counts: BTreeMap<&StackingGroupId, usize> = BTreeMap::new();
    let mut independent_provenance = BTreeSet::new();
    let mut source_activations = 0_usize;

    for effect in component.effects() {
        let definition = require_effect(catalog, effect.definition())?;
        validate_effect_stacks(definition, effect)?;
        let count = group_counts
            .entry(&definition.stacking_group)
            .and_modify(|value| *value += 1)
            .or_insert(1);
        match definition.stacking {
            EffectStackingPolicy::IndependentByProvenance { maximum_instances } => {
                if *count > usize::from(maximum_instances) {
                    return Err(MechanicsError::EffectGroupLimitExceeded {
                        entity,
                        group: definition.stacking_group.clone(),
                        actual: *count,
                        maximum: maximum_instances,
                    });
                }
                if !independent_provenance.insert((&definition.stacking_group, effect.provenance()))
                {
                    return Err(MechanicsError::EffectProvenanceConflict {
                        entity,
                        group: definition.stacking_group.clone(),
                        provenance: effect.provenance().clone(),
                    });
                }
            }
            EffectStackingPolicy::Refresh | EffectStackingPolicy::Replace => {
                if *count > 1 {
                    return Err(MechanicsError::EffectStackingConflict {
                        entity,
                        group: definition.stacking_group.clone(),
                        policy: definition.stacking,
                    });
                }
            }
        }
        let additional = usize::from(effect.stacks())
            .checked_mul(definition.sources.len())
            .ok_or(MechanicsError::EffectSourceQuotaExceeded {
                actual: usize::MAX,
                maximum: MAX_EFFECT_SOURCE_ACTIVATIONS,
            })?;
        source_activations = source_activations.checked_add(additional).ok_or(
            MechanicsError::EffectSourceQuotaExceeded {
                actual: usize::MAX,
                maximum: MAX_EFFECT_SOURCE_ACTIVATIONS,
            },
        )?;
        if source_activations > MAX_EFFECT_SOURCE_ACTIVATIONS {
            return Err(MechanicsError::EffectSourceQuotaExceeded {
                actual: source_activations,
                maximum: MAX_EFFECT_SOURCE_ACTIVATIONS,
            });
        }
    }
    Ok(())
}

pub(crate) fn effect_source_activations(
    entity: EntityId,
    effect: &ActiveEffectInstance,
    catalog: &MechanicsCatalog,
) -> Result<Vec<EffectSourceActivation>, MechanicsError> {
    let definition = require_effect(catalog, effect.definition())?;
    let expected = usize::from(effect.stacks())
        .checked_mul(definition.sources.len())
        .ok_or(MechanicsError::EffectSourceQuotaExceeded {
            actual: usize::MAX,
            maximum: MAX_EFFECT_SOURCE_ACTIVATIONS,
        })?;
    if expected > MAX_EFFECT_SOURCE_ACTIVATIONS {
        return Err(MechanicsError::EffectSourceQuotaExceeded {
            actual: expected,
            maximum: MAX_EFFECT_SOURCE_ACTIVATIONS,
        });
    }
    let mut activations = Vec::with_capacity(expected);
    for stack in 1..=effect.stacks() {
        for source in &definition.sources {
            activations.push(EffectSourceActivation {
                identity: SourceInstanceIdentity::Effect {
                    entity,
                    effect: effect.instance().clone(),
                    stack,
                    source: source.clone(),
                },
                definition: source.clone(),
            });
        }
    }
    Ok(activations)
}

fn require_effect<'a>(
    catalog: &'a MechanicsCatalog,
    effect: &EffectDefinitionId,
) -> Result<&'a EffectDefinition, MechanicsError> {
    catalog
        .effect(effect)
        .ok_or_else(|| MechanicsError::UnknownEffect {
            effect: effect.clone(),
        })
}

fn validate_effect_stacks(
    definition: &EffectDefinition,
    effect: &ActiveEffectInstance,
) -> Result<(), MechanicsError> {
    if effect.stacks() == 0 || effect.stacks() > definition.maximum_stacks {
        return Err(MechanicsError::EffectStackLimitExceeded {
            effect: effect.definition().clone(),
            stacks: effect.stacks(),
            maximum: definition.maximum_stacks,
        });
    }
    Ok(())
}

fn map_bound_reconciliation_error(error: MechanicsError) -> MechanicsError {
    match error {
        MechanicsError::TrackOutOfBounds {
            entity,
            track,
            attempted,
            minimum,
            maximum,
        } => MechanicsError::EffectWouldInvalidateTrack {
            entity,
            track,
            current: attempted,
            prospective_minimum: minimum,
            prospective_maximum: maximum,
        },
        other => other,
    }
}

fn canonicalize_observed_revisions(revisions: &mut Vec<ObservedComponentRevision>) {
    revisions.sort_by_key(|value| (value.entity, value.component));
    revisions.dedup();
}
