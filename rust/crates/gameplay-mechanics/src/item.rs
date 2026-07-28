use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use entity_state::{
    ComponentReplacement, ComponentRevision, EntityAuthoringService, EntityState,
    RelationshipCommand, RelationshipError,
};

use crate::{
    CapacityMetricId, CatalogVersion, EquipmentAssignment, EquipmentComponent, EquipmentSlotId,
    InventoryComponent, ItemComponent, ItemDefinition, ItemDefinitionId, ItemKind, ItemStack,
    MechanicsCatalog, MechanicsComponentKind, MechanicsError, ObservedComponentRevision,
    OperationId, SourceCollectionCost, SourceInstanceIdentity, MAX_EQUIPMENT_ASSIGNMENTS,
};

pub const MAX_CONTAINED_ENTITIES_PER_INVENTORY: usize = 256;
pub const MAX_EQUIPMENT_SOURCE_ACTIVATIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityUsage {
    pub metric: CapacityMetricId,
    pub used: u64,
    pub maximum: Option<u64>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InventoryReadCost {
    pub stack_entries_visited: usize,
    pub containment_entries_visited: usize,
    pub item_components_read: usize,
    pub capacity_limits_visited: usize,
    pub capacity_costs_visited: usize,
}

impl InventoryReadCost {
    fn include(&mut self, other: Self) {
        self.stack_entries_visited += other.stack_entries_visited;
        self.containment_entries_visited += other.containment_entries_visited;
        self.item_components_read += other.item_components_read;
        self.capacity_limits_visited += other.capacity_limits_visited;
        self.capacity_costs_visited += other.capacity_costs_visited;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueInventoryItem {
    pub entity: EntityId,
    pub definition: ItemDefinitionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryView {
    owner: EntityId,
    revision: ComponentRevision,
    relationship_revision: u64,
    catalog_version: CatalogVersion,
    stacks: Vec<ItemStack>,
    unique_items: Vec<UniqueInventoryItem>,
    capacity: Vec<CapacityUsage>,
    read_cost: InventoryReadCost,
}

impl InventoryView {
    pub const fn owner(&self) -> EntityId {
        self.owner
    }

    pub const fn revision(&self) -> &ComponentRevision {
        &self.revision
    }

    pub const fn relationship_revision(&self) -> u64 {
        self.relationship_revision
    }

    pub const fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn stacks(&self) -> &[ItemStack] {
        &self.stacks
    }

    pub fn unique_items(&self) -> &[UniqueInventoryItem] {
        &self.unique_items
    }

    pub fn capacity(&self) -> &[CapacityUsage] {
        &self.capacity
    }

    pub const fn read_cost(&self) -> InventoryReadCost {
        self.read_cost
    }
}

#[derive(Debug, Clone)]
pub struct InventoryMutationRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub owner: EntityId,
    pub item: ItemDefinitionId,
    pub quantity: u64,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryMutationKind {
    Grant,
    Consume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryMutationReceipt {
    pub catalog_version: CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub kind: InventoryMutationKind,
    pub owner: EntityId,
    pub item: ItemDefinitionId,
    pub requested_quantity: u64,
    pub before_quantity: u64,
    pub after_quantity: u64,
    pub observed_inventory_revision: u64,
    pub committed_inventory_revision: u64,
    pub capacity_before: Vec<CapacityUsage>,
    pub capacity_after: Vec<CapacityUsage>,
    pub read_cost: InventoryReadCost,
}

#[derive(Debug, Clone)]
pub struct InventoryTransferRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub from_owner: EntityId,
    pub to_owner: EntityId,
    pub item: ItemDefinitionId,
    pub quantity: u64,
    pub expected_from_revision: Option<ComponentRevision>,
    pub expected_to_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryTransferReceipt {
    pub catalog_version: CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub from_owner: EntityId,
    pub to_owner: EntityId,
    pub item: ItemDefinitionId,
    pub quantity: u64,
    pub from_before: u64,
    pub from_after: u64,
    pub to_before: u64,
    pub to_after: u64,
    pub observed_from_revision: u64,
    pub committed_from_revision: u64,
    pub observed_to_revision: u64,
    pub committed_to_revision: u64,
    pub from_capacity_before: Vec<CapacityUsage>,
    pub from_capacity_after: Vec<CapacityUsage>,
    pub to_capacity_before: Vec<CapacityUsage>,
    pub to_capacity_after: Vec<CapacityUsage>,
    pub read_cost: InventoryReadCost,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct InventoryService;

impl InventoryService {
    pub fn view(
        state: &EntityState,
        catalog: &MechanicsCatalog,
        owner: EntityId,
    ) -> Result<InventoryView, MechanicsError> {
        let component = inventory_component(state, catalog, owner)?;
        let evaluation = evaluate_inventory(state, catalog, owner, component.stacks(), None, None)?;
        Ok(InventoryView {
            owner,
            revision: state.component_revision::<InventoryComponent>(owner)?,
            relationship_revision: state.revision(),
            catalog_version: component.catalog_version().clone(),
            stacks: component.stacks().to_vec(),
            unique_items: evaluation.unique_items,
            capacity: evaluation.capacity,
            read_cost: evaluation.cost,
        })
    }

    pub fn grant(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: InventoryMutationRequest,
    ) -> Result<InventoryMutationReceipt, MechanicsError> {
        Self::mutate(state, catalog, request, InventoryMutationKind::Grant)
    }

    pub fn consume(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: InventoryMutationRequest,
    ) -> Result<InventoryMutationReceipt, MechanicsError> {
        Self::mutate(state, catalog, request, InventoryMutationKind::Consume)
    }

    fn mutate(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: InventoryMutationRequest,
        kind: InventoryMutationKind,
    ) -> Result<InventoryMutationReceipt, MechanicsError> {
        require_positive_quantity(&request.item, request.quantity)?;
        let definition = fungible_definition(catalog, &request.item)?;
        let actual_revision = state.component_revision::<InventoryComponent>(request.owner)?;
        if let Some(expected) = &request.expected_revision {
            crate::track::ensure_revision(expected, &actual_revision)?;
        }
        let publish_revision = request
            .expected_revision
            .clone()
            .unwrap_or_else(|| actual_revision.clone());
        let component = inventory_component(state, catalog, request.owner)?;
        let before_evaluation = evaluate_inventory(
            state,
            catalog,
            request.owner,
            component.stacks(),
            None,
            None,
        )?;
        let before = stack_quantity(component.stacks(), &request.item);
        let after = match kind {
            InventoryMutationKind::Grant => {
                before.checked_add(request.quantity).ok_or_else(|| {
                    MechanicsError::InventoryQuantityLimitExceeded {
                        item: request.item.clone(),
                        attempted: u64::MAX,
                        maximum: definition.maximum_quantity,
                    }
                })?
            }
            InventoryMutationKind::Consume => {
                before.checked_sub(request.quantity).ok_or_else(|| {
                    MechanicsError::InventoryInsufficientQuantity {
                        owner: request.owner,
                        item: request.item.clone(),
                        requested: request.quantity,
                        available: before,
                    }
                })?
            }
        };
        if after > definition.maximum_quantity {
            return Err(MechanicsError::InventoryQuantityLimitExceeded {
                item: request.item.clone(),
                attempted: after,
                maximum: definition.maximum_quantity,
            });
        }
        let stacks = replace_stack_quantity(component.stacks(), request.item.clone(), after);
        let candidate = InventoryComponent::with_capacity_limits(
            catalog.version().clone(),
            stacks,
            component.capacity_limits().to_vec(),
        )?;
        let after_evaluation = evaluate_inventory(
            state,
            catalog,
            request.owner,
            candidate.stacks(),
            None,
            None,
        )?;
        let mut read_cost = before_evaluation.cost;
        read_cost.include(after_evaluation.cost);
        EntityAuthoringService.replace_component(
            state,
            publish_revision,
            request.owner,
            candidate,
        )?;
        let committed = state.component_revision::<InventoryComponent>(request.owner)?;
        Ok(InventoryMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            source: request.source,
            kind,
            owner: request.owner,
            item: request.item,
            requested_quantity: request.quantity,
            before_quantity: before,
            after_quantity: after,
            observed_inventory_revision: actual_revision.revision(),
            committed_inventory_revision: committed.revision(),
            capacity_before: before_evaluation.capacity,
            capacity_after: after_evaluation.capacity,
            read_cost,
        })
    }

    pub fn transfer(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: InventoryTransferRequest,
    ) -> Result<InventoryTransferReceipt, MechanicsError> {
        if request.from_owner == request.to_owner {
            return Err(MechanicsError::InventoryOwnerConflict {
                owner: request.from_owner,
            });
        }
        require_positive_quantity(&request.item, request.quantity)?;
        let definition = fungible_definition(catalog, &request.item)?;
        let from_actual = state.component_revision::<InventoryComponent>(request.from_owner)?;
        let to_actual = state.component_revision::<InventoryComponent>(request.to_owner)?;
        if let Some(expected) = &request.expected_from_revision {
            crate::track::ensure_revision(expected, &from_actual)?;
        }
        if let Some(expected) = &request.expected_to_revision {
            crate::track::ensure_revision(expected, &to_actual)?;
        }
        let from_publish = request
            .expected_from_revision
            .clone()
            .unwrap_or_else(|| from_actual.clone());
        let to_publish = request
            .expected_to_revision
            .clone()
            .unwrap_or_else(|| to_actual.clone());
        let from_component = inventory_component(state, catalog, request.from_owner)?;
        let to_component = inventory_component(state, catalog, request.to_owner)?;
        let from_before_evaluation = evaluate_inventory(
            state,
            catalog,
            request.from_owner,
            from_component.stacks(),
            None,
            None,
        )?;
        let to_before_evaluation = evaluate_inventory(
            state,
            catalog,
            request.to_owner,
            to_component.stacks(),
            None,
            None,
        )?;
        let from_before = stack_quantity(from_component.stacks(), &request.item);
        let from_after = from_before.checked_sub(request.quantity).ok_or_else(|| {
            MechanicsError::InventoryInsufficientQuantity {
                owner: request.from_owner,
                item: request.item.clone(),
                requested: request.quantity,
                available: from_before,
            }
        })?;
        let to_before = stack_quantity(to_component.stacks(), &request.item);
        let to_after = to_before.checked_add(request.quantity).ok_or_else(|| {
            MechanicsError::InventoryQuantityLimitExceeded {
                item: request.item.clone(),
                attempted: u64::MAX,
                maximum: definition.maximum_quantity,
            }
        })?;
        if to_after > definition.maximum_quantity {
            return Err(MechanicsError::InventoryQuantityLimitExceeded {
                item: request.item.clone(),
                attempted: to_after,
                maximum: definition.maximum_quantity,
            });
        }
        let from_candidate = InventoryComponent::with_capacity_limits(
            catalog.version().clone(),
            replace_stack_quantity(from_component.stacks(), request.item.clone(), from_after),
            from_component.capacity_limits().to_vec(),
        )?;
        let to_candidate = InventoryComponent::with_capacity_limits(
            catalog.version().clone(),
            replace_stack_quantity(to_component.stacks(), request.item.clone(), to_after),
            to_component.capacity_limits().to_vec(),
        )?;
        let from_after_evaluation = evaluate_inventory(
            state,
            catalog,
            request.from_owner,
            from_candidate.stacks(),
            None,
            None,
        )?;
        let to_after_evaluation = evaluate_inventory(
            state,
            catalog,
            request.to_owner,
            to_candidate.stacks(),
            None,
            None,
        )?;
        let mut read_cost = from_before_evaluation.cost;
        read_cost.include(to_before_evaluation.cost);
        read_cost.include(from_after_evaluation.cost);
        read_cost.include(to_after_evaluation.cost);
        EntityAuthoringService.replace_components(
            state,
            vec![
                ComponentReplacement {
                    expected_revision: from_publish,
                    entity: request.from_owner,
                    component: from_candidate,
                },
                ComponentReplacement {
                    expected_revision: to_publish,
                    entity: request.to_owner,
                    component: to_candidate,
                },
            ],
        )?;
        let from_committed = state.component_revision::<InventoryComponent>(request.from_owner)?;
        let to_committed = state.component_revision::<InventoryComponent>(request.to_owner)?;
        Ok(InventoryTransferReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            source: request.source,
            from_owner: request.from_owner,
            to_owner: request.to_owner,
            item: request.item,
            quantity: request.quantity,
            from_before,
            from_after,
            to_before,
            to_after,
            observed_from_revision: from_actual.revision(),
            committed_from_revision: from_committed.revision(),
            observed_to_revision: to_actual.revision(),
            committed_to_revision: to_committed.revision(),
            from_capacity_before: from_before_evaluation.capacity,
            from_capacity_after: from_after_evaluation.capacity,
            to_capacity_before: to_before_evaluation.capacity,
            to_capacity_after: to_after_evaluation.capacity,
            read_cost,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentMutationKind {
    Equip,
    Unequip,
    Swap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipmentSlotChange {
    pub slot: EquipmentSlotId,
    pub before: Option<EntityId>,
    pub after: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipmentMutationReceipt {
    pub catalog_version: CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub kind: EquipmentMutationKind,
    pub owner: EntityId,
    pub item: EntityId,
    pub replaced_item: Option<EntityId>,
    pub changes: Vec<EquipmentSlotChange>,
    pub observed_state_revision: u64,
    pub committed_state_revision: u64,
    pub observed_equipment_revision: u64,
    pub committed_equipment_revision: u64,
    pub observed_item_revisions: Vec<ObservedComponentRevision>,
    pub source_activations: usize,
    pub tracks_validated: usize,
    pub observed_revisions: Vec<ObservedComponentRevision>,
    pub source_cost: SourceCollectionCost,
}

#[derive(Debug, Clone)]
pub struct EquipmentEquipRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub owner: EntityId,
    pub item: EntityId,
    pub slots: Vec<EquipmentSlotId>,
    pub expected_equipment_revision: Option<ComponentRevision>,
    pub expected_state_revision: u64,
}

#[derive(Debug, Clone)]
pub struct EquipmentUnequipRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub owner: EntityId,
    pub item: EntityId,
    pub expected_equipment_revision: Option<ComponentRevision>,
    pub expected_state_revision: u64,
}

#[derive(Debug, Clone)]
pub struct EquipmentSwapRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub owner: EntityId,
    pub outgoing_item: EntityId,
    pub incoming_item: EntityId,
    pub incoming_slots: Vec<EquipmentSlotId>,
    pub expected_equipment_revision: Option<ComponentRevision>,
    pub expected_state_revision: u64,
}

#[derive(Debug, Clone)]
pub struct ItemTransferRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub item: EntityId,
    pub from_owner: EntityId,
    pub to_owner: EntityId,
    pub expected_relationship_revision: u64,
    pub expected_from_inventory_revision: Option<ComponentRevision>,
    pub expected_to_inventory_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTransferReceipt {
    pub catalog_version: CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub item: EntityId,
    pub from_owner: EntityId,
    pub to_owner: EntityId,
    pub revision_before: u64,
    pub revision_after: u64,
    pub observed_from_inventory_revision: u64,
    pub observed_to_inventory_revision: u64,
    pub from_capacity_before: Vec<CapacityUsage>,
    pub from_capacity_after: Vec<CapacityUsage>,
    pub to_capacity_before: Vec<CapacityUsage>,
    pub to_capacity_after: Vec<CapacityUsage>,
    pub read_cost: InventoryReadCost,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EquipmentService;

impl EquipmentService {
    pub fn equip(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EquipmentEquipRequest,
    ) -> Result<EquipmentMutationReceipt, MechanicsError> {
        let prepared = prepare_equipment(
            state,
            catalog,
            request.owner,
            request.expected_equipment_revision,
            request.expected_state_revision,
        )?;
        if prepared
            .component
            .assignments()
            .iter()
            .any(|assignment| assignment.item == request.item)
        {
            return Err(MechanicsError::EquipmentItemAlreadyAssigned {
                owner: request.owner,
                item: request.item,
            });
        }
        validate_requested_equipment_item(
            state,
            catalog,
            request.owner,
            request.item,
            &request.slots,
        )?;
        let mut assignments = prepared.component.assignments().to_vec();
        for slot in &request.slots {
            if let Some(assignment) = prepared.component.assignment(slot) {
                return Err(MechanicsError::EquipmentSlotOccupied {
                    owner: request.owner,
                    slot: slot.clone(),
                    item: assignment.item,
                });
            }
            assignments.push(EquipmentAssignment {
                slot: slot.clone(),
                item: request.item,
            });
        }
        let candidate = EquipmentComponent::new(catalog.version().clone(), assignments)?;
        finish_equipment_mutation(
            state,
            catalog,
            prepared,
            candidate,
            EquipmentMutationContext {
                operation: request.operation,
                source: request.source,
                kind: EquipmentMutationKind::Equip,
                owner: request.owner,
                item: request.item,
                replaced_item: None,
            },
        )
    }

    pub fn unequip(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EquipmentUnequipRequest,
    ) -> Result<EquipmentMutationReceipt, MechanicsError> {
        let prepared = prepare_equipment(
            state,
            catalog,
            request.owner,
            request.expected_equipment_revision,
            request.expected_state_revision,
        )?;
        if !prepared
            .component
            .assignments()
            .iter()
            .any(|assignment| assignment.item == request.item)
        {
            return Err(MechanicsError::EquipmentItemNotAssigned {
                owner: request.owner,
                item: request.item,
            });
        }
        let assignments = prepared
            .component
            .assignments()
            .iter()
            .filter(|assignment| assignment.item != request.item)
            .cloned()
            .collect();
        let candidate = EquipmentComponent::new(catalog.version().clone(), assignments)?;
        finish_equipment_mutation(
            state,
            catalog,
            prepared,
            candidate,
            EquipmentMutationContext {
                operation: request.operation,
                source: request.source,
                kind: EquipmentMutationKind::Unequip,
                owner: request.owner,
                item: request.item,
                replaced_item: None,
            },
        )
    }

    pub fn swap(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: EquipmentSwapRequest,
    ) -> Result<EquipmentMutationReceipt, MechanicsError> {
        if request.outgoing_item == request.incoming_item {
            return Err(MechanicsError::EquipmentItemAlreadyAssigned {
                owner: request.owner,
                item: request.incoming_item,
            });
        }
        let prepared = prepare_equipment(
            state,
            catalog,
            request.owner,
            request.expected_equipment_revision,
            request.expected_state_revision,
        )?;
        if !prepared
            .component
            .assignments()
            .iter()
            .any(|assignment| assignment.item == request.outgoing_item)
        {
            return Err(MechanicsError::EquipmentItemNotAssigned {
                owner: request.owner,
                item: request.outgoing_item,
            });
        }
        if prepared
            .component
            .assignments()
            .iter()
            .any(|assignment| assignment.item == request.incoming_item)
        {
            return Err(MechanicsError::EquipmentItemAlreadyAssigned {
                owner: request.owner,
                item: request.incoming_item,
            });
        }
        validate_requested_equipment_item(
            state,
            catalog,
            request.owner,
            request.incoming_item,
            &request.incoming_slots,
        )?;
        let mut assignments = prepared
            .component
            .assignments()
            .iter()
            .filter(|assignment| assignment.item != request.outgoing_item)
            .cloned()
            .collect::<Vec<_>>();
        for slot in &request.incoming_slots {
            if let Some(assignment) = assignments
                .iter()
                .find(|assignment| &assignment.slot == slot)
            {
                return Err(MechanicsError::EquipmentSlotOccupied {
                    owner: request.owner,
                    slot: slot.clone(),
                    item: assignment.item,
                });
            }
            assignments.push(EquipmentAssignment {
                slot: slot.clone(),
                item: request.incoming_item,
            });
        }
        let candidate = EquipmentComponent::new(catalog.version().clone(), assignments)?;
        finish_equipment_mutation(
            state,
            catalog,
            prepared,
            candidate,
            EquipmentMutationContext {
                operation: request.operation,
                source: request.source,
                kind: EquipmentMutationKind::Swap,
                owner: request.owner,
                item: request.incoming_item,
                replaced_item: Some(request.outgoing_item),
            },
        )
    }

    pub fn transfer_unique_item(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: ItemTransferRequest,
    ) -> Result<ItemTransferReceipt, MechanicsError> {
        if request.from_owner == request.to_owner {
            return Err(MechanicsError::InventoryOwnerConflict {
                owner: request.from_owner,
            });
        }
        ensure_state_revision(state, request.expected_relationship_revision)?;
        unique_item_definition(state, catalog, request.item)?;
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
        let from_actual = state.component_revision::<InventoryComponent>(request.from_owner)?;
        let to_actual = state.component_revision::<InventoryComponent>(request.to_owner)?;
        if let Some(expected) = &request.expected_from_inventory_revision {
            crate::track::ensure_revision(expected, &from_actual)?;
        }
        if let Some(expected) = &request.expected_to_inventory_revision {
            crate::track::ensure_revision(expected, &to_actual)?;
        }
        let from_component = inventory_component(state, catalog, request.from_owner)?;
        let to_component = inventory_component(state, catalog, request.to_owner)?;
        let from_before = evaluate_inventory(
            state,
            catalog,
            request.from_owner,
            from_component.stacks(),
            None,
            None,
        )?;
        let to_before = evaluate_inventory(
            state,
            catalog,
            request.to_owner,
            to_component.stacks(),
            None,
            None,
        )?;
        let from_after = evaluate_inventory(
            state,
            catalog,
            request.from_owner,
            from_component.stacks(),
            Some(request.item),
            None,
        )?;
        let to_after = evaluate_inventory(
            state,
            catalog,
            request.to_owner,
            to_component.stacks(),
            None,
            Some(request.item),
        )?;
        let mut read_cost = from_before.cost;
        read_cost.include(to_before.cost);
        read_cost.include(from_after.cost);
        read_cost.include(to_after.cost);
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
            observed_from_inventory_revision: from_actual.revision(),
            observed_to_inventory_revision: to_actual.revision(),
            from_capacity_before: from_before.capacity,
            from_capacity_after: from_after.capacity,
            to_capacity_before: to_before.capacity,
            to_capacity_after: to_after.capacity,
            read_cost,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ItemDestroyRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub item: EntityId,
    pub expected_state_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDestroyReceipt {
    pub catalog_version: CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub item: EntityId,
    pub former_owner: Option<EntityId>,
    pub revision_before: u64,
    pub revision_after: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ItemService;

impl ItemService {
    pub fn destroy_unique(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: ItemDestroyRequest,
    ) -> Result<ItemDestroyReceipt, MechanicsError> {
        ensure_state_revision(state, request.expected_state_revision)?;
        unique_item_definition(state, catalog, request.item)?;
        let owner = state.contained_in(request.item);
        if let Some(owner) = owner {
            if let Some(equipment) = state.component::<EquipmentComponent>(owner)? {
                crate::source::ensure_catalog_version(
                    catalog,
                    owner,
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
                        owner,
                        slot: assignment.slot.clone(),
                    });
                }
            }
        }
        let receipt =
            EntityAuthoringService.destroy(state, request.expected_state_revision, request.item)?;
        Ok(ItemDestroyReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            source: request.source,
            item: request.item,
            former_owner: owner,
            revision_before: receipt.revision_before,
            revision_after: receipt.revision_after,
        })
    }
}

struct InventoryEvaluation {
    unique_items: Vec<UniqueInventoryItem>,
    capacity: Vec<CapacityUsage>,
    cost: InventoryReadCost,
}

pub(crate) fn validate_inventory_state(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    owner: EntityId,
    component: &InventoryComponent,
) -> Result<(), MechanicsError> {
    crate::source::ensure_catalog_version(
        catalog,
        owner,
        InventoryComponent::LABEL,
        component.catalog_version(),
    )?;
    evaluate_inventory(state, catalog, owner, component.stacks(), None, None)?;
    Ok(())
}

fn evaluate_inventory(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    owner: EntityId,
    stacks: &[ItemStack],
    excluded_item: Option<EntityId>,
    included_item: Option<EntityId>,
) -> Result<InventoryEvaluation, MechanicsError> {
    let component =
        state
            .component::<InventoryComponent>(owner)?
            .ok_or(MechanicsError::MissingComponent {
                entity: owner,
                component: InventoryComponent::LABEL,
            })?;
    crate::source::ensure_catalog_version(
        catalog,
        owner,
        InventoryComponent::LABEL,
        component.catalog_version(),
    )?;
    let mut adjusted_count = state.contained_entity_count(owner);
    if excluded_item.is_some_and(|item| state.contained_in(item) == Some(owner)) {
        adjusted_count = adjusted_count.saturating_sub(1);
    }
    if included_item.is_some_and(|item| state.contained_in(item) != Some(owner)) {
        adjusted_count = adjusted_count.saturating_add(1);
    }
    if adjusted_count > MAX_CONTAINED_ENTITIES_PER_INVENTORY {
        return Err(MechanicsError::InventoryContainmentQuotaExceeded {
            owner,
            actual: adjusted_count,
            maximum: MAX_CONTAINED_ENTITIES_PER_INVENTORY,
        });
    }

    let mut cost = InventoryReadCost::default();
    let mut capacity: BTreeMap<CapacityMetricId, (u128, Option<u64>)> = BTreeMap::new();
    for limit in component.capacity_limits() {
        cost.capacity_limits_visited += 1;
        if catalog.capacity_metric(limit.metric()).is_none() {
            return Err(MechanicsError::UnknownCapacityMetric {
                metric: limit.metric().clone(),
            });
        }
        capacity.insert(limit.metric().clone(), (0, Some(limit.maximum())));
    }
    for stack in stacks {
        cost.stack_entries_visited += 1;
        let definition = fungible_definition(catalog, &stack.definition)?;
        if stack.quantity == 0 || stack.quantity > definition.maximum_quantity {
            return Err(MechanicsError::InventoryQuantityLimitExceeded {
                item: stack.definition.clone(),
                attempted: stack.quantity,
                maximum: definition.maximum_quantity,
            });
        }
        include_capacity_costs(
            &mut capacity,
            &mut cost,
            definition,
            u128::from(stack.quantity),
        )?;
    }

    let mut unique_items = Vec::new();
    for item in state.contained_entities(owner) {
        if excluded_item == Some(item) {
            continue;
        }
        cost.containment_entries_visited += 1;
        include_unique_item(
            state,
            catalog,
            item,
            &mut unique_items,
            &mut capacity,
            &mut cost,
        )?;
    }
    if let Some(item) = included_item {
        if state.contained_in(item) != Some(owner) {
            cost.containment_entries_visited += 1;
            include_unique_item(
                state,
                catalog,
                item,
                &mut unique_items,
                &mut capacity,
                &mut cost,
            )?;
        }
    }
    unique_items.sort_by_key(|item| item.entity);

    let mut capacity_receipt = Vec::with_capacity(capacity.len());
    for (metric, (used, maximum)) in capacity {
        let used = u64::try_from(used).map_err(|_| MechanicsError::CapacityArithmeticOverflow {
            metric: metric.clone(),
        })?;
        if let Some(maximum) = maximum {
            if used > maximum {
                return Err(MechanicsError::InventoryCapacityExceeded {
                    owner,
                    metric,
                    attempted: used,
                    maximum,
                });
            }
        }
        capacity_receipt.push(CapacityUsage {
            metric,
            used,
            maximum,
        });
    }
    Ok(InventoryEvaluation {
        unique_items,
        capacity: capacity_receipt,
        cost,
    })
}

fn include_unique_item(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    item: EntityId,
    unique_items: &mut Vec<UniqueInventoryItem>,
    capacity: &mut BTreeMap<CapacityMetricId, (u128, Option<u64>)>,
    cost: &mut InventoryReadCost,
) -> Result<(), MechanicsError> {
    let Some(component) = state.component::<ItemComponent>(item)? else {
        return Ok(());
    };
    cost.item_components_read += 1;
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
    unique_items.push(UniqueInventoryItem {
        entity: item,
        definition: component.definition().clone(),
    });
    include_capacity_costs(capacity, cost, definition, 1)
}

fn include_capacity_costs(
    capacity: &mut BTreeMap<CapacityMetricId, (u128, Option<u64>)>,
    read_cost: &mut InventoryReadCost,
    definition: &ItemDefinition,
    quantity: u128,
) -> Result<(), MechanicsError> {
    for item_cost in &definition.capacity_costs {
        read_cost.capacity_costs_visited += 1;
        let amount = u128::from(item_cost.units)
            .checked_mul(quantity)
            .ok_or_else(|| MechanicsError::CapacityArithmeticOverflow {
                metric: item_cost.metric.clone(),
            })?;
        let entry = capacity
            .entry(item_cost.metric.clone())
            .or_insert((0, None));
        entry.0 = entry.0.checked_add(amount).ok_or_else(|| {
            MechanicsError::CapacityArithmeticOverflow {
                metric: item_cost.metric.clone(),
            }
        })?;
    }
    Ok(())
}

fn inventory_component<'a>(
    state: &'a EntityState,
    catalog: &MechanicsCatalog,
    owner: EntityId,
) -> Result<&'a InventoryComponent, MechanicsError> {
    let component =
        state
            .component::<InventoryComponent>(owner)?
            .ok_or(MechanicsError::MissingComponent {
                entity: owner,
                component: InventoryComponent::LABEL,
            })?;
    crate::source::ensure_catalog_version(
        catalog,
        owner,
        InventoryComponent::LABEL,
        component.catalog_version(),
    )?;
    for limit in component.capacity_limits() {
        if catalog.capacity_metric(limit.metric()).is_none() {
            return Err(MechanicsError::UnknownCapacityMetric {
                metric: limit.metric().clone(),
            });
        }
    }
    Ok(component)
}

fn fungible_definition<'a>(
    catalog: &'a MechanicsCatalog,
    item: &ItemDefinitionId,
) -> Result<&'a ItemDefinition, MechanicsError> {
    let definition = catalog
        .item(item)
        .ok_or_else(|| MechanicsError::UnknownItem { item: item.clone() })?;
    if definition.kind != ItemKind::Fungible {
        return Err(MechanicsError::InventoryItemKindMismatch {
            item: item.clone(),
            expected: ItemKind::Fungible,
            actual: definition.kind,
        });
    }
    Ok(definition)
}

fn require_positive_quantity(item: &ItemDefinitionId, quantity: u64) -> Result<(), MechanicsError> {
    if quantity == 0 {
        return Err(MechanicsError::InvalidInventoryQuantity {
            item: item.clone(),
            quantity,
        });
    }
    Ok(())
}

fn stack_quantity(stacks: &[ItemStack], item: &ItemDefinitionId) -> u64 {
    stacks
        .binary_search_by(|stack| stack.definition.cmp(item))
        .ok()
        .map_or(0, |index| stacks[index].quantity)
}

fn replace_stack_quantity(
    stacks: &[ItemStack],
    item: ItemDefinitionId,
    quantity: u64,
) -> Vec<ItemStack> {
    let mut candidate = stacks.to_vec();
    match candidate.binary_search_by(|stack| stack.definition.cmp(&item)) {
        Ok(index) if quantity == 0 => {
            candidate.remove(index);
        }
        Ok(index) => candidate[index].quantity = quantity,
        Err(index) if quantity > 0 => candidate.insert(
            index,
            ItemStack {
                definition: item,
                quantity,
            },
        ),
        Err(_) => {}
    }
    candidate
}

struct PreparedEquipment {
    actual_revision: ComponentRevision,
    publish_revision: ComponentRevision,
    observed_state_revision: u64,
    component: EquipmentComponent,
}

struct EquipmentMutationContext {
    operation: OperationId,
    source: SourceInstanceIdentity,
    kind: EquipmentMutationKind,
    owner: EntityId,
    item: EntityId,
    replaced_item: Option<EntityId>,
}

fn prepare_equipment(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    owner: EntityId,
    expected_equipment_revision: Option<ComponentRevision>,
    expected_state_revision: u64,
) -> Result<PreparedEquipment, MechanicsError> {
    ensure_state_revision(state, expected_state_revision)?;
    let actual_revision = state.component_revision::<EquipmentComponent>(owner)?;
    if let Some(expected) = &expected_equipment_revision {
        crate::track::ensure_revision(expected, &actual_revision)?;
    }
    let publish_revision = expected_equipment_revision.unwrap_or_else(|| actual_revision.clone());
    let component =
        state
            .component::<EquipmentComponent>(owner)?
            .ok_or(MechanicsError::MissingComponent {
                entity: owner,
                component: EquipmentComponent::LABEL,
            })?;
    crate::source::ensure_catalog_version(
        catalog,
        owner,
        EquipmentComponent::LABEL,
        component.catalog_version(),
    )?;
    Ok(PreparedEquipment {
        actual_revision,
        publish_revision,
        observed_state_revision: expected_state_revision,
        component: component.clone(),
    })
}

fn finish_equipment_mutation(
    state: &mut EntityState,
    catalog: &MechanicsCatalog,
    prepared: PreparedEquipment,
    candidate: EquipmentComponent,
    context: EquipmentMutationContext,
) -> Result<EquipmentMutationReceipt, MechanicsError> {
    let validation = validate_equipment_state(state, catalog, context.owner, &candidate)?;
    let (tracks_validated, observed_revisions, source_cost) =
        crate::stat::validate_tracks_with_equipment_override(
            state,
            catalog,
            context.owner,
            &context.operation,
            &candidate,
        )
        .map_err(map_equipment_bound_reconciliation_error)?;
    let changes = equipment_changes(&prepared.component, &candidate);
    EntityAuthoringService.replace_component(
        state,
        prepared.publish_revision,
        context.owner,
        candidate,
    )?;
    let committed = state.component_revision::<EquipmentComponent>(context.owner)?;
    Ok(EquipmentMutationReceipt {
        catalog_version: catalog.version().clone(),
        catalog_fingerprint: catalog.fingerprint().to_string(),
        operation: context.operation,
        source: context.source,
        kind: context.kind,
        owner: context.owner,
        item: context.item,
        replaced_item: context.replaced_item,
        changes,
        observed_state_revision: prepared.observed_state_revision,
        committed_state_revision: state.revision(),
        observed_equipment_revision: prepared.actual_revision.revision(),
        committed_equipment_revision: committed.revision(),
        observed_item_revisions: validation.observed_items,
        source_activations: validation.source_activations,
        tracks_validated,
        observed_revisions,
        source_cost,
    })
}

fn map_equipment_bound_reconciliation_error(error: MechanicsError) -> MechanicsError {
    match error {
        MechanicsError::TrackOutOfBounds {
            entity,
            track,
            attempted,
            minimum,
            maximum,
        } => MechanicsError::EquipmentWouldInvalidateTrack {
            owner: entity,
            track,
            current: attempted,
            prospective_minimum: minimum,
            prospective_maximum: maximum,
        },
        other => other,
    }
}

fn equipment_changes(
    before: &EquipmentComponent,
    after: &EquipmentComponent,
) -> Vec<EquipmentSlotChange> {
    let before = before
        .assignments()
        .iter()
        .map(|assignment| (assignment.slot.clone(), assignment.item))
        .collect::<BTreeMap<_, _>>();
    let after = after
        .assignments()
        .iter()
        .map(|assignment| (assignment.slot.clone(), assignment.item))
        .collect::<BTreeMap<_, _>>();
    before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|slot| {
            let old = before.get(&slot).copied();
            let new = after.get(&slot).copied();
            (old != new).then_some(EquipmentSlotChange {
                slot,
                before: old,
                after: new,
            })
        })
        .collect()
}

pub(crate) struct EquipmentValidation {
    pub(crate) observed_items: Vec<ObservedComponentRevision>,
    pub(crate) source_activations: usize,
}

pub(crate) fn validate_equipment_state(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    owner: EntityId,
    component: &EquipmentComponent,
) -> Result<EquipmentValidation, MechanicsError> {
    crate::source::ensure_catalog_version(
        catalog,
        owner,
        EquipmentComponent::LABEL,
        component.catalog_version(),
    )?;
    let mut items: BTreeMap<EntityId, Vec<EquipmentSlotId>> = BTreeMap::new();
    for assignment in component.assignments() {
        if catalog.equipment_slot(&assignment.slot).is_none() {
            return Err(MechanicsError::UnknownEquipmentSlot {
                slot: assignment.slot.clone(),
            });
        }
        items
            .entry(assignment.item)
            .or_default()
            .push(assignment.slot.clone());
    }
    let mut exclusivity = BTreeMap::new();
    let mut observed_items = Vec::with_capacity(items.len());
    let mut source_activations = 0_usize;
    for (item, slots) in items {
        let (component, definition) = unique_item_definition(state, catalog, item)?;
        let actual_owner = state.contained_in(item);
        if actual_owner != Some(owner) {
            return Err(MechanicsError::ItemNotContained {
                item,
                expected_owner: owner,
                actual_owner,
            });
        }
        let policy =
            definition
                .equipment
                .as_ref()
                .ok_or_else(|| MechanicsError::ItemNotEquippable {
                    item,
                    definition: definition.id.clone(),
                })?;
        if slots.len() != usize::from(policy.required_slots) {
            return Err(MechanicsError::EquipmentSlotCountMismatch {
                item,
                expected: policy.required_slots,
                actual: slots.len(),
            });
        }
        for slot in &slots {
            let slot_definition = catalog
                .equipment_slot(slot)
                .expect("slot existence checked while grouping");
            if !slot_definition.allowed_classifications.is_empty()
                && !definition.classifications.iter().any(|classification| {
                    slot_definition
                        .allowed_classifications
                        .binary_search(classification)
                        .is_ok()
                })
            {
                return Err(MechanicsError::EquipmentSlotClassificationMismatch {
                    item,
                    slot: slot.clone(),
                });
            }
        }
        if let Some(group) = &policy.exclusive_group {
            if let Some(existing) = exclusivity.insert(group.clone(), item) {
                return Err(MechanicsError::EquipmentExclusivityConflict {
                    owner,
                    group: group.clone(),
                    existing,
                    requested: item,
                });
            }
        }
        source_activations = source_activations
            .checked_add(definition.sources.len())
            .ok_or(MechanicsError::EquipmentSourceQuotaExceeded {
                actual: usize::MAX,
                maximum: MAX_EQUIPMENT_SOURCE_ACTIVATIONS,
            })?;
        if source_activations > MAX_EQUIPMENT_SOURCE_ACTIVATIONS {
            return Err(MechanicsError::EquipmentSourceQuotaExceeded {
                actual: source_activations,
                maximum: MAX_EQUIPMENT_SOURCE_ACTIVATIONS,
            });
        }
        observed_items.push(ObservedComponentRevision {
            entity: item,
            component: MechanicsComponentKind::Item,
            revision: state.component_revision::<ItemComponent>(item)?.revision(),
        });
        let _ = component;
    }
    Ok(EquipmentValidation {
        observed_items,
        source_activations,
    })
}

fn unique_item_definition<'a>(
    state: &'a EntityState,
    catalog: &'a MechanicsCatalog,
    item: EntityId,
) -> Result<(&'a ItemComponent, &'a ItemDefinition), MechanicsError> {
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
    Ok((component, definition))
}

fn validate_requested_equipment_item(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    owner: EntityId,
    item: EntityId,
    slots: &[EquipmentSlotId],
) -> Result<(), MechanicsError> {
    ensure_unique_slots(slots)?;
    let (_, definition) = unique_item_definition(state, catalog, item)?;
    let actual_owner = state.contained_in(item);
    if actual_owner != Some(owner) {
        return Err(MechanicsError::ItemNotContained {
            item,
            expected_owner: owner,
            actual_owner,
        });
    }
    let policy =
        definition
            .equipment
            .as_ref()
            .ok_or_else(|| MechanicsError::ItemNotEquippable {
                item,
                definition: definition.id.clone(),
            })?;
    if slots.len() != usize::from(policy.required_slots) {
        return Err(MechanicsError::EquipmentSlotCountMismatch {
            item,
            expected: policy.required_slots,
            actual: slots.len(),
        });
    }
    Ok(())
}

fn ensure_unique_slots(slots: &[EquipmentSlotId]) -> Result<(), MechanicsError> {
    if slots.len() > MAX_EQUIPMENT_ASSIGNMENTS {
        return Err(MechanicsError::RequestQuotaExceeded {
            field: "equipmentSlots",
            actual: slots.len(),
            maximum: MAX_EQUIPMENT_ASSIGNMENTS,
        });
    }
    let mut unique = BTreeSet::new();
    for slot in slots {
        if !unique.insert(slot) {
            return Err(MechanicsError::RequestQuotaExceeded {
                field: "duplicateEquipmentSlots",
                actual: slots.len(),
                maximum: unique.len(),
            });
        }
    }
    Ok(())
}

fn ensure_state_revision(state: &EntityState, expected: u64) -> Result<(), MechanicsError> {
    if state.revision() != expected {
        return Err(MechanicsError::Relationship(
            RelationshipError::StaleRevision {
                expected,
                actual: state.revision(),
            },
        ));
    }
    Ok(())
}
