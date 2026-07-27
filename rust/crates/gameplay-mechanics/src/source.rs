use core_ids::EntityId;
use entity_state::EntityState;
use serde::{Deserialize, Serialize};

use crate::{
    ActiveEffectsComponent, CatalogVersion, EffectInstanceId, EquipmentComponent,
    IntrinsicSourcesComponent, ItemComponent, MechanicsCatalog, MechanicsComponentKind,
    MechanicsError, ObservedComponentRevision, OperationId, SourceDefinitionId, SourceInstanceId,
};

pub const MAX_INTRINSIC_SOURCE_BINDINGS: usize = 64;
pub const MAX_EQUIPMENT_ASSIGNMENTS: usize = 32;
pub const MAX_INVENTORY_STACKS: usize = 128;
pub const MAX_REQUEST_SOURCES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SourceInstanceIdentity {
    Intrinsic {
        #[serde(with = "entity_id_serde")]
        entity: EntityId,
        instance: SourceInstanceId,
    },
    Effect {
        #[serde(with = "entity_id_serde")]
        entity: EntityId,
        effect: EffectInstanceId,
        stack: u16,
        source: SourceDefinitionId,
    },
    EquippedItem {
        #[serde(with = "entity_id_serde")]
        owner: EntityId,
        #[serde(with = "entity_id_serde")]
        item: EntityId,
        source: SourceDefinitionId,
    },
    Request {
        operation: OperationId,
        instance: SourceInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSource {
    pub instance: SourceInstanceId,
    pub definition: SourceDefinitionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSource {
    pub identity: SourceInstanceIdentity,
    pub definition: SourceDefinitionId,
    pub priority: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionOutcome {
    Applied,
    Suppressed,
    Inapplicable,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SourceCollectionCost {
    pub intrinsic_entries_visited: usize,
    pub effect_entries_visited: usize,
    pub effect_source_activations_visited: usize,
    pub equipment_entries_visited: usize,
    pub item_components_read: usize,
    pub request_entries_visited: usize,
}

pub(crate) fn collect_active_sources(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
    operation: &OperationId,
    request_sources: &[RequestSource],
    maximum_sources: usize,
) -> Result<
    (
        Vec<ActiveSource>,
        SourceCollectionCost,
        Vec<ObservedComponentRevision>,
    ),
    MechanicsError,
> {
    collect_active_sources_with_effects_override(
        state,
        catalog,
        entity,
        operation,
        request_sources,
        maximum_sources,
        None,
    )
}

pub(crate) fn collect_active_sources_with_effects_override(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
    operation: &OperationId,
    request_sources: &[RequestSource],
    maximum_sources: usize,
    active_effects_override: Option<&ActiveEffectsComponent>,
) -> Result<
    (
        Vec<ActiveSource>,
        SourceCollectionCost,
        Vec<ObservedComponentRevision>,
    ),
    MechanicsError,
> {
    let mut collected = Vec::new();
    let mut cost = SourceCollectionCost::default();
    let mut observed_revisions = Vec::new();

    if let Some(component) = state.component::<IntrinsicSourcesComponent>(entity)? {
        observed_revisions.push(ObservedComponentRevision {
            entity,
            component: MechanicsComponentKind::IntrinsicSources,
            revision: state
                .component_revision::<IntrinsicSourcesComponent>(entity)?
                .revision(),
        });
        ensure_catalog_version(
            catalog,
            entity,
            IntrinsicSourcesComponent::LABEL,
            component.catalog_version(),
        )?;
        for binding in component.bindings() {
            cost.intrinsic_entries_visited += 1;
            push_source(
                catalog,
                &mut collected,
                SourceInstanceIdentity::Intrinsic {
                    entity,
                    instance: binding.instance().clone(),
                },
                binding.definition().clone(),
                maximum_sources,
            )?;
        }
    }

    let stored_effects = if active_effects_override.is_none() {
        state.component::<ActiveEffectsComponent>(entity)?
    } else {
        None
    };
    if let Some(component) = active_effects_override.or(stored_effects) {
        observed_revisions.push(ObservedComponentRevision {
            entity,
            component: MechanicsComponentKind::ActiveEffects,
            revision: state
                .component_revision::<ActiveEffectsComponent>(entity)?
                .revision(),
        });
        ensure_catalog_version(
            catalog,
            entity,
            ActiveEffectsComponent::LABEL,
            component.catalog_version(),
        )?;
        crate::effect::validate_active_effects_against_catalog(entity, component, catalog)?;
        for active in component.effects() {
            cost.effect_entries_visited += 1;
            let definition = catalog.effect(active.definition()).ok_or_else(|| {
                MechanicsError::UnknownEffect {
                    effect: active.definition().clone(),
                }
            })?;
            let additional = usize::from(active.stacks())
                .checked_mul(definition.sources.len())
                .ok_or(MechanicsError::ReceiptQuotaExceeded {
                    actual: usize::MAX,
                    maximum: maximum_sources,
                })?;
            ensure_receipt_capacity(collected.len(), additional, maximum_sources)?;
            for stack in 1..=active.stacks() {
                for source in &definition.sources {
                    cost.effect_source_activations_visited += 1;
                    push_source(
                        catalog,
                        &mut collected,
                        SourceInstanceIdentity::Effect {
                            entity,
                            effect: active.instance().clone(),
                            stack,
                            source: source.clone(),
                        },
                        source.clone(),
                        maximum_sources,
                    )?;
                }
            }
        }
    }

    if let Some(component) = state.component::<EquipmentComponent>(entity)? {
        observed_revisions.push(ObservedComponentRevision {
            entity,
            component: MechanicsComponentKind::Equipment,
            revision: state
                .component_revision::<EquipmentComponent>(entity)?
                .revision(),
        });
        ensure_catalog_version(
            catalog,
            entity,
            EquipmentComponent::LABEL,
            component.catalog_version(),
        )?;
        let validation = crate::item::validate_equipment_state(state, catalog, entity, component)?;
        cost.equipment_entries_visited += component.assignments().len();
        cost.item_components_read += validation.observed_items.len();
        observed_revisions.extend(validation.observed_items);
        let items = component
            .assignments()
            .iter()
            .map(|assignment| assignment.item)
            .collect::<std::collections::BTreeSet<_>>();
        for item_entity in items {
            let item = state
                .component::<ItemComponent>(item_entity)?
                .expect("equipment validation requires item components");
            let item_definition = catalog
                .item(item.definition())
                .expect("equipment validation requires admitted item definitions");
            for source in &item_definition.sources {
                push_source(
                    catalog,
                    &mut collected,
                    SourceInstanceIdentity::EquippedItem {
                        owner: entity,
                        item: item_entity,
                        source: source.clone(),
                    },
                    source.clone(),
                    maximum_sources,
                )?;
            }
        }
    }

    for request in request_sources {
        cost.request_entries_visited += 1;
        push_source(
            catalog,
            &mut collected,
            SourceInstanceIdentity::Request {
                operation: operation.clone(),
                instance: request.instance.clone(),
            },
            request.definition.clone(),
            maximum_sources,
        )?;
    }

    let mut identities = std::collections::BTreeSet::new();
    for source in &collected {
        if !identities.insert(&source.identity) {
            return Err(MechanicsError::DuplicateSource {
                source: source.identity.clone(),
            });
        }
    }
    collected.sort_by(|left, right| {
        (left.priority, &left.identity, &left.definition).cmp(&(
            right.priority,
            &right.identity,
            &right.definition,
        ))
    });

    observed_revisions.sort_by_key(|value| (value.entity, value.component));
    observed_revisions.dedup();

    Ok((collected, cost, observed_revisions))
}

pub(crate) fn ensure_catalog_version(
    catalog: &MechanicsCatalog,
    entity: EntityId,
    component: &'static str,
    actual: &CatalogVersion,
) -> Result<(), MechanicsError> {
    if actual != catalog.version() {
        return Err(MechanicsError::CatalogVersionMismatch {
            entity,
            component,
            expected: catalog.version().clone(),
            actual: actual.clone(),
        });
    }
    Ok(())
}

fn push_source(
    catalog: &MechanicsCatalog,
    collected: &mut Vec<ActiveSource>,
    identity: SourceInstanceIdentity,
    definition: SourceDefinitionId,
    maximum_sources: usize,
) -> Result<(), MechanicsError> {
    ensure_receipt_capacity(collected.len(), 1, maximum_sources)?;
    let admitted = catalog
        .source(&definition)
        .ok_or_else(|| MechanicsError::UnknownSource {
            source: definition.clone(),
        })?;
    collected.push(ActiveSource {
        identity,
        definition,
        priority: admitted.priority,
    });
    Ok(())
}

pub(crate) fn ensure_receipt_capacity(
    current: usize,
    additional: usize,
    maximum: usize,
) -> Result<(), MechanicsError> {
    let actual = current.saturating_add(additional);
    if actual > maximum {
        return Err(MechanicsError::ReceiptQuotaExceeded { actual, maximum });
    }
    Ok(())
}

pub(crate) mod entity_id_serde {
    use core_ids::EntityId;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &EntityId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(value.raw())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<EntityId, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(EntityId::new(u64::deserialize(deserializer)?))
    }
}
