use core_ids::EntityId;
use entity_state::EntityState;
use serde::{Deserialize, Serialize};

use crate::{
    ActiveEffectsComponent, CatalogVersion, EffectInstanceId, EquipmentComponent,
    IntrinsicSourcesComponent, ItemComponent, ItemKind, MechanicsCatalog, MechanicsComponentKind,
    MechanicsError, ObservedComponentRevision, OperationId, SourceDefinitionId, SourceInstanceId,
};

pub const MAX_INTRINSIC_SOURCE_BINDINGS: usize = 64;
pub const MAX_ACTIVE_EFFECT_INSTANCES: usize = 64;
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
                    instance: binding.instance.clone(),
                },
                binding.definition.clone(),
                maximum_sources,
            )?;
        }
    }

    if let Some(component) = state.component::<ActiveEffectsComponent>(entity)? {
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
        for active in component.effects() {
            cost.effect_entries_visited += 1;
            let definition = catalog.effect(&active.definition).ok_or_else(|| {
                MechanicsError::UnknownEffect {
                    effect: active.definition.clone(),
                }
            })?;
            for source in &definition.sources {
                push_source(
                    catalog,
                    &mut collected,
                    SourceInstanceIdentity::Effect {
                        entity,
                        effect: active.instance.clone(),
                        source: source.clone(),
                    },
                    source.clone(),
                    maximum_sources,
                )?;
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
        for assignment in component.assignments() {
            cost.equipment_entries_visited += 1;
            if catalog.equipment_slot(&assignment.slot).is_none() {
                return Err(MechanicsError::UnknownEquipmentSlot {
                    slot: assignment.slot.clone(),
                });
            }
            let actual_owner = state.contained_in(assignment.item);
            if actual_owner != Some(entity) {
                return Err(MechanicsError::ItemNotContained {
                    item: assignment.item,
                    expected_owner: entity,
                    actual_owner,
                });
            }
            let item = state.component::<ItemComponent>(assignment.item)?.ok_or(
                MechanicsError::MissingComponent {
                    entity: assignment.item,
                    component: ItemComponent::LABEL,
                },
            )?;
            observed_revisions.push(ObservedComponentRevision {
                entity: assignment.item,
                component: MechanicsComponentKind::Item,
                revision: state
                    .component_revision::<ItemComponent>(assignment.item)?
                    .revision(),
            });
            cost.item_components_read += 1;
            ensure_catalog_version(
                catalog,
                assignment.item,
                ItemComponent::LABEL,
                item.catalog_version(),
            )?;
            let item_definition =
                catalog
                    .item(item.definition())
                    .ok_or_else(|| MechanicsError::UnknownItem {
                        item: item.definition().clone(),
                    })?;
            if item_definition.kind != ItemKind::Unique {
                return Err(MechanicsError::IncompatibleItemKind {
                    item: assignment.item,
                    definition: item.definition().clone(),
                });
            }
            for source in &item_definition.sources {
                push_source(
                    catalog,
                    &mut collected,
                    SourceInstanceIdentity::EquippedItem {
                        owner: entity,
                        item: assignment.item,
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
