use core_ids::EntityId;
use entity_state::{
    ComponentRevision, EntityAuthoringService, EntityState, RelationshipCommand, RelationshipError,
};

use crate::{
    EquipmentAssignment, EquipmentComponent, EquipmentSlotId, ItemComponent, ItemKind,
    MechanicsCatalog, MechanicsError, OperationId, SourceInstanceIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipmentMutationReceipt {
    pub catalog_version: crate::CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub owner: EntityId,
    pub slot: EquipmentSlotId,
    pub item: EntityId,
    pub equipped: bool,
    pub observed_equipment_revision: u64,
    pub committed_equipment_revision: u64,
}

#[derive(Debug, Clone)]
pub struct ItemTransferRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub item: EntityId,
    pub from_owner: EntityId,
    pub to_owner: EntityId,
    pub expected_relationship_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTransferReceipt {
    pub catalog_version: crate::CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub item: EntityId,
    pub from_owner: EntityId,
    pub to_owner: EntityId,
    pub revision_before: u64,
    pub revision_after: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EquipmentService;

impl EquipmentService {
    #[allow(clippy::too_many_arguments)]
    pub fn equip(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        operation: OperationId,
        source: SourceInstanceIdentity,
        owner: EntityId,
        slot: EquipmentSlotId,
        item: EntityId,
        expected_revision: Option<ComponentRevision>,
    ) -> Result<EquipmentMutationReceipt, MechanicsError> {
        if catalog.equipment_slot(&slot).is_none() {
            return Err(MechanicsError::UnknownEquipmentSlot { slot });
        }
        validate_unique_item(state, catalog, item)?;
        let actual_owner = state.contained_in(item);
        if actual_owner != Some(owner) {
            return Err(MechanicsError::ItemNotContained {
                item,
                expected_owner: owner,
                actual_owner,
            });
        }
        let actual_revision = state.component_revision::<EquipmentComponent>(owner)?;
        if let Some(expected) = &expected_revision {
            crate::track::ensure_revision(expected, &actual_revision)?;
        }
        let publish_revision = expected_revision.unwrap_or_else(|| actual_revision.clone());
        let equipment = state.component::<EquipmentComponent>(owner)?.ok_or(
            MechanicsError::MissingComponent {
                entity: owner,
                component: EquipmentComponent::LABEL,
            },
        )?;
        crate::source::ensure_catalog_version(
            catalog,
            owner,
            EquipmentComponent::LABEL,
            equipment.catalog_version(),
        )?;
        if let Some(assignment) = equipment.assignment(&slot) {
            return Err(MechanicsError::EquipmentSlotOccupied {
                owner,
                slot,
                item: assignment.item,
            });
        }
        if equipment
            .assignments()
            .iter()
            .any(|assignment| assignment.item == item)
        {
            return Err(MechanicsError::EquipmentItemAlreadyAssigned { owner, item });
        }
        let mut assignments = equipment.assignments().to_vec();
        assignments.push(EquipmentAssignment {
            slot: slot.clone(),
            item,
        });
        let candidate = EquipmentComponent::new(catalog.version().clone(), assignments)?;
        EntityAuthoringService.replace_component(state, publish_revision, owner, candidate)?;
        let committed = state.component_revision::<EquipmentComponent>(owner)?;
        Ok(EquipmentMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation,
            source,
            owner,
            slot,
            item,
            equipped: true,
            observed_equipment_revision: actual_revision.revision(),
            committed_equipment_revision: committed.revision(),
        })
    }

    pub fn unequip(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        operation: OperationId,
        source: SourceInstanceIdentity,
        owner: EntityId,
        slot: EquipmentSlotId,
        expected_revision: Option<ComponentRevision>,
    ) -> Result<EquipmentMutationReceipt, MechanicsError> {
        if catalog.equipment_slot(&slot).is_none() {
            return Err(MechanicsError::UnknownEquipmentSlot { slot });
        }
        let actual_revision = state.component_revision::<EquipmentComponent>(owner)?;
        if let Some(expected) = &expected_revision {
            crate::track::ensure_revision(expected, &actual_revision)?;
        }
        let publish_revision = expected_revision.unwrap_or_else(|| actual_revision.clone());
        let equipment = state.component::<EquipmentComponent>(owner)?.ok_or(
            MechanicsError::MissingComponent {
                entity: owner,
                component: EquipmentComponent::LABEL,
            },
        )?;
        crate::source::ensure_catalog_version(
            catalog,
            owner,
            EquipmentComponent::LABEL,
            equipment.catalog_version(),
        )?;
        let assignment =
            equipment
                .assignment(&slot)
                .ok_or_else(|| MechanicsError::EquipmentSlotEmpty {
                    owner,
                    slot: slot.clone(),
                })?;
        let item = assignment.item;
        let assignments = equipment
            .assignments()
            .iter()
            .filter(|assignment| assignment.slot != slot)
            .cloned()
            .collect();
        let candidate = EquipmentComponent::new(catalog.version().clone(), assignments)?;
        EntityAuthoringService.replace_component(state, publish_revision, owner, candidate)?;
        let committed = state.component_revision::<EquipmentComponent>(owner)?;
        Ok(EquipmentMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation,
            source,
            owner,
            slot,
            item,
            equipped: false,
            observed_equipment_revision: actual_revision.revision(),
            committed_equipment_revision: committed.revision(),
        })
    }

    pub fn transfer_unique_item(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: ItemTransferRequest,
    ) -> Result<ItemTransferReceipt, MechanicsError> {
        if state.revision() != request.expected_relationship_revision {
            return Err(MechanicsError::Relationship(
                RelationshipError::StaleRevision {
                    expected: request.expected_relationship_revision,
                    actual: state.revision(),
                },
            ));
        }
        validate_unique_item(state, catalog, request.item)?;
        let actual_owner = state.contained_in(request.item);
        if actual_owner != Some(request.from_owner) {
            return Err(MechanicsError::ItemNotContained {
                item: request.item,
                expected_owner: request.from_owner,
                actual_owner,
            });
        }
        if let Some(equipment) = state.component::<EquipmentComponent>(request.from_owner)? {
            crate::source::ensure_catalog_version(
                catalog,
                request.from_owner,
                EquipmentComponent::LABEL,
                equipment.catalog_version(),
            )?;
            if let Some(assignment) = equipment
                .assignments()
                .iter()
                .find(|assignment| assignment.item == request.item)
            {
                return Err(MechanicsError::ItemEquipped {
                    item: request.item,
                    owner: request.from_owner,
                    slot: assignment.slot.clone(),
                });
            }
        }

        let relationship = state.apply_relationship(
            request.expected_relationship_revision,
            RelationshipCommand::SetContainment {
                child: request.item,
                container: request.to_owner,
            },
        )?;
        Ok(ItemTransferReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            source: request.source,
            item: request.item,
            from_owner: request.from_owner,
            to_owner: request.to_owner,
            revision_before: relationship.revision_before,
            revision_after: relationship.revision_after,
        })
    }
}

fn validate_unique_item(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    item: EntityId,
) -> Result<(), MechanicsError> {
    let component =
        state
            .component::<ItemComponent>(item)?
            .ok_or(MechanicsError::MissingComponent {
                entity: item,
                component: ItemComponent::LABEL,
            })?;
    crate::source::ensure_catalog_version(
        catalog,
        item,
        ItemComponent::LABEL,
        component.catalog_version(),
    )?;
    let definition =
        catalog
            .item(component.definition())
            .ok_or_else(|| MechanicsError::UnknownItem {
                item: component.definition().clone(),
            })?;
    if definition.kind != ItemKind::Unique {
        return Err(MechanicsError::IncompatibleItemKind {
            item,
            definition: component.definition().clone(),
        });
    }
    Ok(())
}
