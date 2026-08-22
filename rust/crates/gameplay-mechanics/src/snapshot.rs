use core_ids::EntityId;
use entity_state::{decode_snapshot_with_registry, ComponentRegistry, EntityState};

use crate::{
    gameplay_component_registry, source::ensure_catalog_version, stat::track_bounds,
    ActiveEffectsComponent, EquipmentComponent, InventoryComponent, ItemComponent, ItemKind,
    MechanicsCatalog, MechanicsError, MechanicsSnapshotError, OperationId, StatsComponent,
    TracksComponent,
};

pub fn decode_snapshot_with_catalog(
    input: &str,
    catalog: &MechanicsCatalog,
) -> Result<EntityState, MechanicsSnapshotError> {
    let registry = gameplay_component_registry()
        .expect("fixed gameplay component registrations are internally consistent");
    let state = decode_snapshot_with_registry(input, registry)?;
    validate_state_against_catalog(&state, catalog)?;
    Ok(state)
}

pub fn decode_snapshot_with_catalog_and_registry(
    input: &str,
    registry: ComponentRegistry,
    catalog: &MechanicsCatalog,
) -> Result<EntityState, MechanicsSnapshotError> {
    let state = decode_snapshot_with_registry(input, registry)?;
    validate_state_against_catalog(&state, catalog)?;
    Ok(state)
}

pub fn validate_state_against_catalog(
    state: &EntityState,
    catalog: &MechanicsCatalog,
) -> Result<(), MechanicsError> {
    for (entity, component) in state.components::<StatsComponent>()? {
        ensure_catalog_version(
            catalog,
            entity,
            StatsComponent::LABEL,
            component.catalog_version(),
        )?;
        for value in component.values() {
            let definition = catalog.stat(value.stat()).ok_or_else(|| {
                MechanicsError::InvalidCatalogReference {
                    entity,
                    component: StatsComponent::LABEL,
                    namespace: "stat",
                    reference: value.stat().to_string(),
                }
            })?;
            if value.base() < definition.minimum || value.base() > definition.maximum {
                return Err(MechanicsError::StatOutOfBounds {
                    entity,
                    stat: value.stat().clone(),
                    attempted: value.base().get(),
                    minimum: definition.minimum.get(),
                    maximum: definition.maximum.get(),
                });
            }
        }
    }
    for (entity, component) in state.components::<crate::IntrinsicSourcesComponent>()? {
        ensure_catalog_version(
            catalog,
            entity,
            crate::IntrinsicSourcesComponent::LABEL,
            component.catalog_version(),
        )?;
        for binding in component.bindings() {
            ensure_reference(
                catalog.source(binding.definition()).is_some(),
                entity,
                crate::IntrinsicSourcesComponent::LABEL,
                "source",
                binding.definition().to_string(),
            )?;
        }
    }
    for (entity, component) in state.components::<ActiveEffectsComponent>()? {
        ensure_catalog_version(
            catalog,
            entity,
            ActiveEffectsComponent::LABEL,
            component.catalog_version(),
        )?;
        for effect in component.effects() {
            ensure_reference(
                catalog.effect(effect.definition()).is_some(),
                entity,
                ActiveEffectsComponent::LABEL,
                "effect",
                effect.definition().to_string(),
            )?;
        }
        crate::effect::validate_active_effects_against_catalog(entity, component, catalog)?;
    }
    for (entity, component) in state.components::<ItemComponent>()? {
        ensure_catalog_version(
            catalog,
            entity,
            ItemComponent::LABEL,
            component.catalog_version(),
        )?;
        let definition = catalog.item(component.definition()).ok_or_else(|| {
            MechanicsError::InvalidCatalogReference {
                entity,
                component: ItemComponent::LABEL,
                namespace: "item",
                reference: component.definition().to_string(),
            }
        })?;
        ensure_reference(
            definition.kind == ItemKind::Unique,
            entity,
            ItemComponent::LABEL,
            "unique item",
            component.definition().to_string(),
        )?;
    }
    for (entity, component) in state.components::<InventoryComponent>()? {
        crate::item::validate_inventory_catalog_compatibility(catalog, entity, component)?;
        crate::item::validate_inventory_state(state, catalog, entity, component)?;
    }
    for (owner, component) in state.components::<EquipmentComponent>()? {
        ensure_catalog_version(
            catalog,
            owner,
            EquipmentComponent::LABEL,
            component.catalog_version(),
        )?;
        for assignment in component.assignments() {
            ensure_reference(
                catalog.equipment_slot(&assignment.slot).is_some(),
                owner,
                EquipmentComponent::LABEL,
                "equipment slot",
                assignment.slot.to_string(),
            )?;
            if state.contained_in(assignment.item) != Some(owner) {
                return Err(MechanicsError::ItemNotContained {
                    item: assignment.item,
                    expected_owner: owner,
                    actual_owner: state.contained_in(assignment.item),
                });
            }
            let item = state.component::<ItemComponent>(assignment.item)?.ok_or(
                MechanicsError::MissingComponent {
                    entity: assignment.item,
                    component: ItemComponent::LABEL,
                },
            )?;
            ensure_reference(
                catalog
                    .item(item.definition())
                    .is_some_and(|definition| definition.kind == ItemKind::Unique),
                assignment.item,
                ItemComponent::LABEL,
                "unique item",
                item.definition().to_string(),
            )?;
        }
        crate::item::validate_equipment_state(state, catalog, owner, component)?;
    }

    let validation_operation =
        OperationId::parse("snapshot_validation").expect("fixed operation identity");
    for (entity, component) in state.components::<TracksComponent>()? {
        ensure_catalog_version(
            catalog,
            entity,
            TracksComponent::LABEL,
            component.catalog_version(),
        )?;
        for value in component.values() {
            let (minimum, maximum, _, _) =
                track_bounds(state, catalog, entity, value.track(), &validation_operation)?;
            if value.current() < minimum || value.current() > maximum {
                return Err(MechanicsError::TrackOutOfBounds {
                    entity,
                    track: value.track().clone(),
                    attempted: value.current().get(),
                    minimum: minimum.get(),
                    maximum: maximum.get(),
                });
            }
        }
    }
    Ok(())
}

fn ensure_reference(
    valid: bool,
    entity: EntityId,
    component: &'static str,
    namespace: &'static str,
    reference: String,
) -> Result<(), MechanicsError> {
    if !valid {
        return Err(MechanicsError::InvalidCatalogReference {
            entity,
            component,
            namespace,
            reference,
        });
    }
    Ok(())
}
