use core_ids::EntityId;
use entity_state::{
    apply_relationship, EntityAuthoringService, EntityComponent, EntityDefinition, EntityState,
    RelationshipCommand,
};
use gameplay_mechanics::{
    CapacityMetricDefinition, CapacityMetricId, CatalogVersion, EquipmentComponent,
    EquipmentExclusivityId, EquipmentSlotDefinition, EquipmentSlotId, InventoryCapacityLimit,
    InventoryComponent, ItemCapacityCost, ItemClassificationId, ItemComponent, ItemDefinition,
    ItemDefinitionId, ItemEquipmentPolicy, ItemKind, MechanicsCatalog, MechanicsCatalogDefinition,
    OperationId, SourceInstanceId, SourceInstanceIdentity, MAX_EQUIPMENT_ASSIGNMENTS,
};
use gameplay_standard::{
    CapabilityRequirementId, CapabilityRoleBinding, CapabilityRoleBindings, CapabilityRoleId,
    ExactInputBundle, StandardMechanicsReceipt, StandardOperation, StandardOperationContext,
    StandardPlanningError, STANDARD_EQUIPMENT_CAPABILITY, STANDARD_INVENTORY_CAPABILITY,
};

const OWNER: EntityId = EntityId::new(1);
const OTHER: EntityId = EntityId::new(2);
const RIFLE: EntityId = EntityId::new(3);
const PISTOL: EntityId = EntityId::new(4);

fn role(value: &str) -> CapabilityRoleId {
    CapabilityRoleId::parse(value).unwrap()
}
fn capability(value: &str) -> CapabilityRequirementId {
    CapabilityRequirementId::parse(value).unwrap()
}
fn item(value: &str) -> ItemDefinitionId {
    ItemDefinitionId::parse(value).unwrap()
}
fn slot(value: &str) -> EquipmentSlotId {
    EquipmentSlotId::parse(value).unwrap()
}
fn version() -> CatalogVersion {
    CatalogVersion::parse("unique.v1").unwrap()
}

fn catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: version(),
        stats: vec![],
        tracks: vec![],
        sources: vec![],
        damage_kinds: vec![],
        effects: vec![],
        capacity_metrics: vec![CapacityMetricDefinition {
            id: CapacityMetricId::parse("mass").unwrap(),
        }],
        items: ["rifle", "pistol"]
            .into_iter()
            .map(|id| ItemDefinition {
                id: item(id),
                kind: ItemKind::Unique,
                maximum_quantity: 1,
                classifications: vec![ItemClassificationId::parse("weapon").unwrap()],
                capacity_costs: vec![ItemCapacityCost {
                    metric: CapacityMetricId::parse("mass").unwrap(),
                    units: 1,
                }],
                equipment: Some(ItemEquipmentPolicy {
                    required_slots: 1,
                    exclusive_group: Some(EquipmentExclusivityId::parse("weapon").unwrap()),
                }),
                sources: vec![],
            })
            .collect(),
        equipment_slots: vec![EquipmentSlotDefinition {
            id: slot("hand"),
            allowed_classifications: vec![ItemClassificationId::parse("weapon").unwrap()],
        }],
    })
    .unwrap()
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    EntityAuthoringService
        .attach_component(
            state,
            state.component_revision::<T>(entity).unwrap(),
            entity,
            value,
        )
        .unwrap();
}

fn state() -> EntityState {
    let mut state = EntityState::from_definitions_with_registry(
        gameplay_mechanics::gameplay_component_registry().unwrap(),
        [
            EntityDefinition::new(OWNER, "owner"),
            EntityDefinition::new(OTHER, "other"),
            EntityDefinition::new(RIFLE, "rifle").with_containment(OWNER),
            EntityDefinition::new(PISTOL, "pistol").with_containment(OWNER),
        ],
    )
    .unwrap();
    for owner in [OWNER, OTHER] {
        attach(
            &mut state,
            owner,
            InventoryComponent::with_capacity_limits(
                version(),
                vec![],
                vec![InventoryCapacityLimit::new(
                    CapacityMetricId::parse("mass").unwrap(),
                    2,
                )],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            owner,
            EquipmentComponent::new(version(), vec![]).unwrap(),
        );
    }
    attach(
        &mut state,
        RIFLE,
        ItemComponent::new(version(), item("rifle")),
    );
    attach(
        &mut state,
        PISTOL,
        ItemComponent::new(version(), item("pistol")),
    );
    state
}

fn context() -> StandardOperationContext {
    let operation = OperationId::parse("unique-operation").unwrap();
    StandardOperationContext::new(
        operation.clone(),
        SourceInstanceIdentity::Request {
            operation,
            instance: SourceInstanceId::parse("unique-source").unwrap(),
        },
    )
    .unwrap()
}

fn bindings(operation: &StandardOperation) -> CapabilityRoleBindings {
    CapabilityRoleBindings::admit(
        &operation.requirements(),
        vec![
            CapabilityRoleBinding::new(
                role("owner"),
                OWNER,
                vec![
                    capability(STANDARD_INVENTORY_CAPABILITY),
                    capability(STANDARD_EQUIPMENT_CAPABILITY),
                ],
            )
            .unwrap(),
            CapabilityRoleBinding::new(
                role("other"),
                OTHER,
                vec![capability(STANDARD_INVENTORY_CAPABILITY)],
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn unique_operations_are_explicit_candidate_leaves_with_complete_guards() {
    let catalog = catalog();
    let source = state();
    let transfer = StandardOperation::TransferUniqueItem {
        from: role("owner"),
        to: role("other"),
        item: RIFLE,
    };
    let equip = StandardOperation::EquipUniqueItem {
        role: role("owner"),
        item: RIFLE,
        slots: vec![slot("hand")],
    };
    let unequip = StandardOperation::UnequipUniqueItem {
        role: role("owner"),
        item: RIFLE,
    };
    let swap = StandardOperation::SwapUniqueItem {
        role: role("owner"),
        outgoing_item: RIFLE,
        incoming_item: PISTOL,
        incoming_slots: vec![slot("hand")],
    };
    assert_eq!(transfer.requirements().len(), 2);
    assert_eq!(
        equip.requirements()[0].capabilities(),
        &[capability(STANDARD_EQUIPMENT_CAPABILITY)]
    );

    let transfer_plan = transfer
        .plan(
            &bindings(&transfer),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let equip_plan = equip
        .plan(
            &bindings(&equip),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let unequip_plan = unequip
        .plan(
            &bindings(&unequip),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let swap_plan = swap
        .plan(
            &bindings(&swap),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    for plan in [&transfer_plan, &equip_plan, &unequip_plan, &swap_plan] {
        assert_eq!(plan.observed_state_revision(), Some(source.revision()));
    }
    assert!(swap_plan
        .observed_revisions()
        .iter()
        .any(|entry| entry.entity() == RIFLE
            && entry.component() == gameplay_mechanics::MechanicsComponentKind::Item));
    assert!(swap_plan
        .observed_revisions()
        .iter()
        .any(|entry| entry.entity() == PISTOL
            && entry.component() == gameplay_mechanics::MechanicsComponentKind::Item));

    let mut transfer_candidate = state();
    assert!(
        matches!(transfer_plan.effect().apply_to_candidate(&mut transfer_candidate, &catalog), Ok(StandardMechanicsReceipt::UniqueItemTransfer(receipt)) if receipt.item == RIFLE)
    );
    assert_eq!(transfer_candidate.contained_in(RIFLE), Some(OTHER));
    assert_eq!(source.contained_in(RIFLE), Some(OWNER));

    let mut candidate = state();
    assert!(
        matches!(equip_plan.effect().apply_to_candidate(&mut candidate, &catalog), Ok(StandardMechanicsReceipt::Equipment(receipt)) if receipt.item == RIFLE)
    );
    assert!(
        matches!(unequip_plan.effect().apply_to_candidate(&mut candidate, &catalog), Ok(StandardMechanicsReceipt::Equipment(receipt)) if receipt.item == RIFLE)
    );
    assert!(matches!(
        equip_plan
            .effect()
            .apply_to_candidate(&mut candidate, &catalog),
        Ok(StandardMechanicsReceipt::Equipment(_))
    ));
    assert!(
        matches!(swap_plan.effect().apply_to_candidate(&mut candidate, &catalog), Ok(StandardMechanicsReceipt::Equipment(receipt)) if receipt.item == PISTOL && receipt.replaced_item == Some(RIFLE))
    );
}

#[test]
fn unique_plans_reject_stale_item_and_containment_without_publication() {
    let catalog = catalog();
    let mut source = state();
    let equip = StandardOperation::EquipUniqueItem {
        role: role("owner"),
        item: RIFLE,
        slots: vec![slot("hand")],
    };
    let plan = equip
        .plan(
            &bindings(&equip),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let revision = source.component_revision::<ItemComponent>(RIFLE).unwrap();
    EntityAuthoringService
        .detach_component::<ItemComponent>(&mut source, revision, RIFLE)
        .unwrap();
    assert!(matches!(
        plan.validate_source_state(&source, &catalog),
        Err(gameplay_standard::StandardPlanValidationError::StaleStateRevision { .. })
    ));

    let mut containment_source = state();
    let containment_plan = equip
        .plan(
            &bindings(&equip),
            &ExactInputBundle::empty(),
            &containment_source,
            &catalog,
            &context(),
        )
        .unwrap();
    let containment_revision = containment_source.revision();
    apply_relationship(
        &mut containment_source,
        containment_revision,
        RelationshipCommand::SetContainment {
            child: RIFLE,
            container: OTHER,
        },
    )
    .unwrap();
    assert!(matches!(
        containment_plan.validate_source_state(&containment_source, &catalog),
        Err(gameplay_standard::StandardPlanValidationError::StaleStateRevision { .. })
    ));
}

#[test]
fn unique_equipment_authoring_rejects_structural_slots_before_candidate_execution() {
    let catalog = catalog();
    let source = state();
    let source_revision = source.revision();
    let candidate = state();
    let candidate_revision = candidate.revision();
    let slots: Vec<_> = (0..MAX_EQUIPMENT_ASSIGNMENTS)
        .map(|index| EquipmentSlotId::parse(format!("slot-{index}")).unwrap())
        .collect();
    let exact_limit = StandardOperation::EquipUniqueItem {
        role: role("owner"),
        item: RIFLE,
        slots: slots.clone(),
    };
    assert!(exact_limit
        .plan(
            &bindings(&exact_limit),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .is_ok());

    let mut limit_plus_one_slots = slots;
    limit_plus_one_slots.push(slot("one-too-many"));
    let limit_plus_one = StandardOperation::EquipUniqueItem {
        role: role("owner"),
        item: RIFLE,
        slots: limit_plus_one_slots,
    };
    assert!(matches!(
        limit_plus_one.plan(
            &bindings(&limit_plus_one),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::EquipmentSlots {
            actual,
            maximum: MAX_EQUIPMENT_ASSIGNMENTS,
        }) if actual == MAX_EQUIPMENT_ASSIGNMENTS + 1
    ));

    let duplicate = StandardOperation::EquipUniqueItem {
        role: role("owner"),
        item: RIFLE,
        slots: vec![slot("hand"), slot("hand")],
    };
    assert!(matches!(
        duplicate.plan(
            &bindings(&duplicate),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::DuplicateEquipmentSlot { .. })
    ));

    let self_swap = StandardOperation::SwapUniqueItem {
        role: role("owner"),
        outgoing_item: RIFLE,
        incoming_item: RIFLE,
        incoming_slots: vec![slot("hand")],
    };
    assert!(matches!(
        self_swap.plan(
            &bindings(&self_swap),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::EquipmentSwapSameItem { item: RIFLE })
    ));
    assert_eq!(source.revision(), source_revision);
    assert_eq!(candidate.revision(), candidate_revision);
}

#[test]
fn unique_sequence_keeps_authority_unchanged_until_one_candidate_publication() {
    let catalog = catalog();
    let authority = state();
    let equip = StandardOperation::EquipUniqueItem {
        role: role("owner"),
        item: RIFLE,
        slots: vec![slot("hand")],
    };
    let transfer = StandardOperation::TransferUniqueItem {
        from: role("owner"),
        to: role("other"),
        item: RIFLE,
    };
    let equip_plan = equip
        .plan(
            &bindings(&equip),
            &ExactInputBundle::empty(),
            &authority,
            &catalog,
            &context(),
        )
        .unwrap();
    let transfer_plan = transfer
        .plan(
            &bindings(&transfer),
            &ExactInputBundle::empty(),
            &authority,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut failed_candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&authority).unwrap(),
        &catalog,
    )
    .unwrap();
    equip_plan
        .effect()
        .apply_to_candidate(&mut failed_candidate, &catalog)
        .unwrap();
    assert!(matches!(
        transfer_plan
            .effect()
            .apply_to_candidate(&mut failed_candidate, &catalog),
        Err(gameplay_mechanics::MechanicsError::ItemEquipped { .. })
    ));
    assert!(authority
        .component::<EquipmentComponent>(OWNER)
        .unwrap()
        .unwrap()
        .assignments()
        .is_empty());
    assert_eq!(authority.contained_in(RIFLE), Some(OWNER));

    let swap = StandardOperation::SwapUniqueItem {
        role: role("owner"),
        outgoing_item: RIFLE,
        incoming_item: PISTOL,
        incoming_slots: vec![slot("hand")],
    };
    let swap_plan = swap
        .plan(
            &bindings(&swap),
            &ExactInputBundle::empty(),
            &authority,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&authority).unwrap(),
        &catalog,
    )
    .unwrap();
    equip_plan
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    swap_plan
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    assert!(authority
        .component::<EquipmentComponent>(OWNER)
        .unwrap()
        .unwrap()
        .assignments()
        .is_empty());
    let authority = candidate; // the product's one explicit publication point
    assert_eq!(
        authority
            .component::<EquipmentComponent>(OWNER)
            .unwrap()
            .unwrap()
            .assignments()[0]
            .item,
        PISTOL
    );
}
