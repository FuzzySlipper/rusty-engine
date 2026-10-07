use core_ids::EntityId;
use core_math::Vec3;
use core_time::TickDelta;
use engine_spatial::{
    integrate_kinematic, integrate_kinematic_with_query, KinematicBody, KinematicShape,
    KinematicTriggerDefinition, MaterialVoxel, PhysicsError, PhysicsStep, PhysicsWorld,
    TriggerCollider, TriggerGeometrySource, TriggerOverlapFactKind, TriggerReconcileCause,
    TriggerVolumeDiagnosticCode, TriggerVolumeSystem, VoxelCollisionScene,
};
use environment_authoring::{generate_tunnel, TunnelGeneratorConfig};

#[test]
fn fixed_step_integration_preserves_donor_velocity_acceleration_and_gravity_behavior() {
    let body = KinematicBody::stationary(Vec3::new(1.0, 2.0, 3.0))
        .with_velocity(Vec3::new(2.0, 0.0, -1.0))
        .with_acceleration(Vec3::new(0.0, 4.0, 0.0))
        .with_gravity_scale(0.0);
    let step = PhysicsStep::new(TickDelta::new(2), 0.25).unwrap();
    let result = integrate_kinematic(body, PhysicsWorld::ZERO_GRAVITY, step).unwrap();

    assert_eq!(result.elapsed_seconds, 0.5);
    assert_eq!(result.next_velocity, Vec3::new(2.0, 2.0, -1.0));
    assert_eq!(result.next_position, Vec3::new(2.0, 3.0, 2.5));
    assert_eq!(
        integrate_kinematic(body, PhysicsWorld::ZERO_GRAVITY, step).unwrap(),
        result
    );

    let falling = KinematicBody::stationary(Vec3::ZERO).with_gravity_scale(0.5);
    let gravity = integrate_kinematic(
        falling,
        PhysicsWorld::Y_DOWN_GRAVITY,
        PhysicsStep::new(TickDelta::new(1), 1.0).unwrap(),
    )
    .unwrap();
    assert_eq!(gravity.next_velocity, Vec3::new(0.0, -4.9, 0.0));
    assert_eq!(gravity.next_position, Vec3::new(0.0, -4.9, 0.0));
}

#[test]
fn collision_required_integration_fails_closed_without_query_and_resolves_with_spatial_query() {
    let body = KinematicBody::stationary(Vec3::new(0.0, 0.5, 0.5))
        .with_velocity(Vec3::new(2.0, 0.0, 1.0))
        .requiring_collision_query();
    let step = PhysicsStep::new(TickDelta::new(1), 0.5).unwrap();
    assert_eq!(
        integrate_kinematic(body, PhysicsWorld::ZERO_GRAVITY, step).unwrap_err(),
        PhysicsError::CollisionQueryRequired
    );

    let scene = VoxelCollisionScene::from_material_voxels(
        1.0,
        8,
        [MaterialVoxel {
            state: 0,
            address: [1, 0, 0],
            material_slot: 1,
        }],
    )
    .unwrap();
    let result = integrate_kinematic_with_query(
        body,
        PhysicsWorld::ZERO_GRAVITY,
        step,
        KinematicShape::new(Vec3::splat(0.4)).unwrap(),
        &scene,
    )
    .unwrap();

    assert_eq!(result.next_position, Vec3::new(0.0, 0.5, 1.0));
    assert_eq!(result.next_velocity, Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(result.collision.blocked_axes, [true, false, false]);
}

#[test]
fn zero_steps_and_invalid_inputs_are_typed() {
    let body = KinematicBody::stationary(Vec3::new(3.0, 4.0, 5.0))
        .with_velocity(Vec3::new(10.0, 0.0, 0.0));
    let result = integrate_kinematic(
        body,
        PhysicsWorld::Y_DOWN_GRAVITY,
        PhysicsStep::new(TickDelta::ZERO, 0.25).unwrap(),
    )
    .unwrap();
    assert_eq!(result.next_position, body.position);
    assert_eq!(result.next_velocity, body.velocity);
    assert!(matches!(
        PhysicsStep::new(TickDelta::new(1), 0.0),
        Err(PhysicsError::InvalidStep { .. })
    ));
    assert_eq!(
        integrate_kinematic(
            KinematicBody::stationary(Vec3::new(f32::INFINITY, 0.0, 0.0)),
            PhysicsWorld::ZERO_GRAVITY,
            PhysicsStep::new(TickDelta::new(1), 1.0).unwrap(),
        )
        .unwrap_err()
        .code(),
        "non-finite-physics-input"
    );
    let overflow =
        KinematicBody::stationary(Vec3::ZERO).with_velocity(Vec3::new(f32::MAX, 0.0, 0.0));
    assert_eq!(
        integrate_kinematic(
            overflow,
            PhysicsWorld::ZERO_GRAVITY,
            PhysicsStep::new(TickDelta::new(2), 1.0).unwrap(),
        )
        .unwrap_err(),
        PhysicsError::NonFiniteInput
    );
}

#[test]
fn generated_tunnel_cells_feed_existing_collision_navigation_and_mesh_authority() {
    let tunnel = generate_tunnel(TunnelGeneratorConfig::tiny_enclosed(19)).unwrap();
    let scene = VoxelCollisionScene::from_material_voxels(
        tunnel.config.voxel_size,
        tunnel.config.chunk_size,
        tunnel
            .spatial_cells()
            .map(|(address, material_slot)| MaterialVoxel {
                state: 0,
                address,
                material_slot,
            }),
    )
    .unwrap();

    assert_eq!(scene.solid_voxel_count(), tunnel.voxels.len());
    assert!(scene.contains_point([0.5, 0.5, 0.5]));
    assert!(!scene.contains_point([2.5, 2.5, 2.5]));
    assert_eq!(scene.resident_chunk_count(), 1);
    assert!(scene.mesh_chunks().len() != 0);
}

#[test]
fn trigger_enter_continue_and_exit_are_reconciled_once() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    let empty = triggers.reconcile(colliders.clone(), 1, TriggerReconcileCause::Scheduled);
    assert!(empty.facts.is_empty());

    move_collider(&mut colliders, subject, Vec3::ZERO);
    let entered = triggers.reconcile(colliders.clone(), 2, TriggerReconcileCause::Teleport);
    assert_eq!(entered.facts.len(), 1);
    assert_eq!(entered.facts[0].kind, TriggerOverlapFactKind::Enter);
    assert_eq!(entered.facts[0].pair.trigger_id(), trigger);

    let continued = triggers.reconcile(colliders.clone(), 3, TriggerReconcileCause::Scheduled);
    assert!(continued.facts.is_empty());
    assert_eq!(continued.continued, entered.active_overlaps);

    move_collider(&mut colliders, subject, Vec3::new(2.0, 0.0, 0.0));
    let exited = triggers.reconcile(colliders, 4, TriggerReconcileCause::Teleport);
    assert_eq!(exited.facts.len(), 1);
    assert_eq!(exited.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(exited.active_overlaps.is_empty());
}

#[test]
fn trigger_endpoint_activation_lifecycle_and_face_touching_semantics_are_explicit() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    move_collider(&mut colliders, subject, Vec3::new(1.0, 0.0, 0.0));
    let touching = triggers.reconcile(colliders.clone(), 1, TriggerReconcileCause::Teleport);
    assert!(touching.active_overlaps.is_empty());

    move_collider(&mut colliders, subject, Vec3::new(-2.0, 0.0, 0.0));
    triggers.reconcile(colliders.clone(), 2, TriggerReconcileCause::Teleport);
    move_collider(&mut colliders, subject, Vec3::new(2.0, 0.0, 0.0));
    let through = triggers.reconcile(colliders.clone(), 3, TriggerReconcileCause::Teleport);
    assert!(
        through.facts.is_empty(),
        "teleports sample endpoint geometry"
    );

    move_collider(&mut colliders, subject, Vec3::ZERO);
    triggers.reconcile(colliders.clone(), 4, TriggerReconcileCause::Spawn);
    set_collision(&mut colliders, trigger, false);
    let inactive = triggers.reconcile(
        colliders.clone(),
        5,
        TriggerReconcileCause::ActivationChanged,
    );
    assert_eq!(inactive.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert_eq!(
        inactive.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::InactiveCollision
    );

    set_collision(&mut colliders, trigger, true);
    let reactivated = triggers.reconcile(
        colliders.clone(),
        6,
        TriggerReconcileCause::ActivationChanged,
    );
    assert_eq!(reactivated.facts[0].kind, TriggerOverlapFactKind::Enter);

    colliders.retain(|collider| collider.entity != subject);
    let destroyed = triggers.reconcile(colliders, 7, TriggerReconcileCause::LifecycleChanged);
    assert_eq!(destroyed.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(destroyed.active_overlaps.is_empty());
}

#[test]
fn trigger_lifecycle_retirement_and_reactivation_are_deliberate() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    move_collider(&mut colliders, subject, Vec3::ZERO);
    let entered = triggers.reconcile(colliders.clone(), 1, TriggerReconcileCause::Movement);
    assert_eq!(entered.facts.len(), 1);

    let retired = triggers.set_active(trigger, false, 2).unwrap();
    assert!(!retired.active);
    assert_eq!(retired.removed_overlaps.len(), 1);
    assert_eq!(retired.facts.len(), 1);
    assert_eq!(retired.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(triggers
        .current_overlaps(trigger)
        .unwrap()
        .subjects
        .is_empty());
    assert!(!triggers.is_active(trigger).unwrap());

    let unchanged = triggers.clone();
    let duplicate = triggers.set_active(trigger, false, 3).unwrap_err();
    assert_eq!(
        duplicate.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::DuplicateLifecycle
    );
    assert_eq!(triggers, unchanged);
    let unknown = triggers
        .set_active(EntityId::new(999), false, 3)
        .unwrap_err();
    assert_eq!(
        unknown.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::MissingDefinition
    );
    assert_eq!(triggers, unchanged);

    let reactivated = triggers.set_active(trigger, true, 4).unwrap();
    assert!(reactivated.active);
    assert!(reactivated.facts.is_empty());
    let reentered = triggers.reconcile(colliders, 5, TriggerReconcileCause::Movement);
    assert_eq!(reentered.facts.len(), 1);
    assert_eq!(reentered.facts[0].kind, TriggerOverlapFactKind::Enter);
}

#[test]
fn trigger_restore_rebases_active_set_and_overlaps_without_edges() {
    let trigger_a = EntityId::new(10);
    let trigger_b = EntityId::new(11);
    let subject = EntityId::new(20);
    let colliders = [
        collider(trigger_a, Vec3::ZERO, 0.5, true),
        collider(trigger_b, Vec3::ZERO, 0.5, true),
        collider(subject, Vec3::ZERO, 0.25, true),
    ];
    let mut triggers = TriggerVolumeSystem::new([
        KinematicTriggerDefinition::new(trigger_a, "zone.a", ["zone"]),
        KinematicTriggerDefinition::new(trigger_b, "zone.b", ["zone"]),
    ])
    .unwrap();
    let entered = triggers.reconcile(colliders, 1, TriggerReconcileCause::Spawn);
    assert_eq!(entered.facts.len(), 2);

    let restored = triggers.restore(&[trigger_b], colliders).unwrap();
    assert_eq!(restored.registered_count, 2);
    assert_eq!(restored.active_count, 1);
    assert_eq!(restored.active_overlaps.len(), 1);
    assert_eq!(restored.active_overlaps[0].trigger_id(), trigger_b);
    assert!(!triggers.is_active(trigger_a).unwrap());
    assert!(triggers.is_active(trigger_b).unwrap());
    let after = triggers.reconcile(colliders, 2, TriggerReconcileCause::Restore);
    assert!(after.facts.is_empty());
    assert_eq!(after.continued.len(), 1);

    let unchanged = triggers.clone();
    let duplicate = triggers
        .restore(&[trigger_b, trigger_b], colliders)
        .unwrap_err();
    assert_eq!(
        duplicate.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::DuplicateLifecycle
    );
    assert_eq!(triggers, unchanged);
    let unknown = triggers
        .restore(&[EntityId::new(999)], colliders)
        .unwrap_err();
    assert_eq!(
        unknown.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::MissingDefinition
    );
    assert_eq!(triggers, unchanged);
}

#[test]
fn malformed_definitions_stale_entities_and_unknown_reads_are_typed() {
    let invalid = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        EntityId::new(1),
        "bad scope",
        ["ok"],
    )])
    .unwrap_err();
    assert_eq!(
        invalid.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::InvalidIdentifier
    );

    let mut stale = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        EntityId::new(99),
        "zone.stale",
        ["zone"],
    )])
    .unwrap();
    let receipt = stale.reconcile([], 1, TriggerReconcileCause::Scheduled);
    assert_eq!(
        receipt.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::StaleEntity
    );

    assert_eq!(
        stale
            .current_overlaps(EntityId::new(98))
            .unwrap_err()
            .diagnostics[0]
            .code,
        TriggerVolumeDiagnosticCode::MissingDefinition
    );
}

#[test]
fn current_overlaps_report_every_subject() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    move_collider(&mut colliders, subject, Vec3::ZERO);
    triggers.reconcile(colliders, 1, TriggerReconcileCause::Teleport);

    let readout = triggers.current_overlaps(trigger).unwrap();
    assert_eq!(readout.subjects, vec![subject]);
}

#[test]
fn entity_bounds_trigger_senses_traversal_with_one_enter_and_one_exit() {
    let trigger = EntityId::new(10);
    let subject = EntityId::new(20);
    // The sensor's collision is disabled, so it is never a solid obstacle.
    let mut colliders = vec![
        collider(trigger, Vec3::ZERO, 0.5, false),
        collider(subject, Vec3::new(-2.0, 0.0, 0.0), 0.25, true),
    ];
    let mut triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        trigger,
        "zone.sensor",
        ["zone"],
    )
    .with_geometry_source(TriggerGeometrySource::EntityBounds)])
    .unwrap();

    let outside = triggers.reconcile(colliders.clone(), 1, TriggerReconcileCause::Spawn);
    assert!(outside.facts.is_empty());
    assert!(outside.diagnostics.is_empty());

    move_collider(&mut colliders, subject, Vec3::new(-0.5, 0.0, 0.0));
    let entered = triggers.reconcile(colliders.clone(), 2, TriggerReconcileCause::Movement);
    assert_eq!(entered.facts.len(), 1);
    assert_eq!(entered.facts[0].kind, TriggerOverlapFactKind::Enter);
    assert_eq!(entered.facts[0].pair.trigger_id(), trigger);
    assert_eq!(entered.facts[0].pair.subject_id(), subject);
    assert!(entered.diagnostics.is_empty());

    move_collider(&mut colliders, subject, Vec3::new(1.5, 0.0, 0.0));
    let exited = triggers.reconcile(colliders, 3, TriggerReconcileCause::Movement);
    assert_eq!(exited.facts.len(), 1);
    assert_eq!(exited.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(exited.active_overlaps.is_empty());
}

#[test]
fn entity_bounds_trigger_keeps_subject_eligibility() {
    let trigger = EntityId::new(10);
    let subject = EntityId::new(20);
    // Subjects still need enabled collision even when the trigger does not.
    let colliders = [
        collider(trigger, Vec3::ZERO, 0.5, false),
        collider(subject, Vec3::ZERO, 0.25, false),
    ];
    let mut triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        trigger,
        "zone.sensor",
        ["zone"],
    )
    .with_geometry_source(TriggerGeometrySource::EntityBounds)])
    .unwrap();

    let receipt = triggers.reconcile(colliders, 1, TriggerReconcileCause::Spawn);
    assert!(receipt.facts.is_empty());
    assert!(receipt.active_overlaps.is_empty());
    assert!(receipt.diagnostics.is_empty());
}

#[test]
fn triggers_containing_a_point_read_the_last_reconciled_geometry_of_active_triggers() {
    let outer = EntityId::new(1);
    let inner = EntityId::new(2);
    let beside = EntityId::new(3);
    let bounds_only = EntityId::new(4);
    let mut triggers = TriggerVolumeSystem::new([
        KinematicTriggerDefinition::new(outer, "pool", ["water"]),
        KinematicTriggerDefinition::new(inner, "pool.deep", ["water"]),
        KinematicTriggerDefinition::new(beside, "shore", ["land"]),
        KinematicTriggerDefinition::new(bounds_only, "mist", ["air"])
            .with_geometry_source(TriggerGeometrySource::EntityBounds),
    ])
    .unwrap();
    // Nothing has been reconciled: no geometry, no containment.
    assert!(triggers.triggers_containing(Vec3::ZERO).is_empty());

    // A nested pair, an adjacent box sharing the face x = 4, and a bounds-only
    // trigger with disabled collision.
    let colliders = vec![
        collider(outer, Vec3::ZERO, 4.0, true),
        collider(inner, Vec3::ZERO, 1.0, true),
        collider(beside, Vec3::new(8.0, 0.0, 0.0), 4.0, true),
        collider(bounds_only, Vec3::new(0.0, 10.0, 0.0), 1.0, false),
    ];
    triggers.reconcile(colliders.clone(), 1, TriggerReconcileCause::Scheduled);
    assert_eq!(triggers.triggers_containing(Vec3::ZERO), [outer, inner]);
    assert_eq!(
        triggers.triggers_containing(Vec3::new(2.0, 0.0, 0.0)),
        [outer]
    );
    // The shared face belongs to neither, as a subject touching it overlaps
    // neither; just inside is one or the other.
    assert!(triggers
        .triggers_containing(Vec3::new(4.0, 0.0, 0.0))
        .is_empty());
    assert_eq!(
        triggers.triggers_containing(Vec3::new(3.999, 0.0, 0.0)),
        [outer]
    );
    assert_eq!(
        triggers.triggers_containing(Vec3::new(4.001, 0.0, 0.0)),
        [beside]
    );
    // A bounds trigger senses with its collision disabled.
    assert_eq!(
        triggers.triggers_containing(Vec3::new(0.0, 10.0, 0.0)),
        [bounds_only]
    );

    // An inactive trigger contains nothing; reactivated, it does again.
    triggers.set_active(inner, false, 2).unwrap();
    assert_eq!(triggers.triggers_containing(Vec3::ZERO), [outer]);
    triggers.set_active(inner, true, 3).unwrap();
    assert_eq!(triggers.triggers_containing(Vec3::ZERO), [outer, inner]);

    // A trigger sensing from active collision needs it enabled, as a
    // reconcile requires.
    let mut disabled = colliders.clone();
    set_collision(&mut disabled, outer, false);
    triggers.reconcile(disabled, 4, TriggerReconcileCause::ActivationChanged);
    assert_eq!(triggers.triggers_containing(Vec3::ZERO), [inner]);

    // The geometry is the last reconcile's: a moved volume answers from where
    // it moved to, and a restore replaces it too.
    let mut moved = colliders.clone();
    move_collider(&mut moved, inner, Vec3::new(0.0, 0.0, 20.0));
    triggers.reconcile(moved, 5, TriggerReconcileCause::Movement);
    assert_eq!(triggers.triggers_containing(Vec3::ZERO), [outer]);
    assert_eq!(
        triggers.triggers_containing(Vec3::new(0.0, 0.0, 20.0)),
        [inner]
    );
    triggers
        .restore(&[outer, inner, beside], colliders)
        .unwrap();
    assert_eq!(triggers.triggers_containing(Vec3::ZERO), [outer, inner]);
    assert!(triggers
        .triggers_containing(Vec3::new(0.0, 10.0, 0.0))
        .is_empty());
}

fn trigger_fixture() -> (
    Vec<TriggerCollider>,
    TriggerVolumeSystem,
    EntityId,
    EntityId,
) {
    let trigger = EntityId::new(10);
    let subject = EntityId::new(20);
    let colliders = vec![
        collider(trigger, Vec3::ZERO, 0.5, true),
        collider(subject, Vec3::new(2.0, 0.0, 0.0), 0.5, true),
    ];
    let triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        trigger,
        "zone.exit",
        ["door", "exit"],
    )])
    .unwrap();
    (colliders, triggers, trigger, subject)
}

fn collider(entity: EntityId, center: Vec3, half: f32, collision_enabled: bool) -> TriggerCollider {
    TriggerCollider {
        entity,
        min: center - Vec3::splat(half),
        max: center + Vec3::splat(half),
        collision_enabled,
    }
}

fn move_collider(colliders: &mut [TriggerCollider], entity: EntityId, center: Vec3) {
    let value = colliders
        .iter_mut()
        .find(|collider| collider.entity == entity)
        .unwrap();
    let half = (value.max - value.min) * 0.5;
    value.min = center - half;
    value.max = center + half;
}

fn set_collision(colliders: &mut [TriggerCollider], entity: EntityId, enabled: bool) {
    colliders
        .iter_mut()
        .find(|collider| collider.entity == entity)
        .unwrap()
        .collision_enabled = enabled;
}

#[test]
fn registers_more_triggers_than_the_former_cap() {
    // 5,000 definitions: past the former 4,096 cap.
    let system = TriggerVolumeSystem::new(
        (1..=5_000)
            .map(|raw| KinematicTriggerDefinition::new(EntityId::new(raw), "zone", ["zone"])),
    )
    .unwrap();
    assert_eq!(system.definitions().count(), 5_000);
}
