use core_ids::EntityId;
use core_math::Vec3;
use core_time::TickDelta;
use engine_spatial::{
    decode_trigger_snapshot, encode_trigger_snapshot, integrate_kinematic,
    integrate_kinematic_with_query, KinematicBody, KinematicShape, KinematicTriggerDefinition,
    MaterialVoxel, PhysicsError, PhysicsStep, PhysicsWorld, TriggerCollider, TriggerGeometrySource,
    TriggerOverlapFactKind, TriggerReconcileCause, TriggerVolumeDiagnosticCode,
    TriggerVolumeSystem, VoxelCollisionScene,
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
    assert!(scene.navigation_cell_count() > 0);
}

#[test]
fn trigger_enter_continue_and_exit_are_reconciled_once() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    let empty = triggers
        .reconcile(colliders.clone(), 1, TriggerReconcileCause::Scheduled)
        .unwrap();
    assert!(empty.facts.is_empty());

    move_collider(&mut colliders, subject, Vec3::ZERO);
    let entered = triggers
        .reconcile(colliders.clone(), 2, TriggerReconcileCause::Teleport)
        .unwrap();
    assert_eq!(entered.facts.len(), 1);
    assert_eq!(entered.facts[0].kind, TriggerOverlapFactKind::Enter);
    assert_eq!(entered.facts[0].pair.trigger_id(), trigger);

    let continued = triggers
        .reconcile(colliders.clone(), 3, TriggerReconcileCause::Scheduled)
        .unwrap();
    assert!(continued.facts.is_empty());
    assert_eq!(continued.continued, entered.active_overlaps);
    assert_eq!(continued.revision, entered.revision);

    move_collider(&mut colliders, subject, Vec3::new(2.0, 0.0, 0.0));
    let exited = triggers
        .reconcile(colliders, 4, TriggerReconcileCause::Teleport)
        .unwrap();
    assert_eq!(exited.facts.len(), 1);
    assert_eq!(exited.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(exited.active_overlaps.is_empty());
}

#[test]
fn trigger_endpoint_activation_lifecycle_and_face_touching_semantics_are_explicit() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    move_collider(&mut colliders, subject, Vec3::new(1.0, 0.0, 0.0));
    let touching = triggers
        .reconcile(colliders.clone(), 1, TriggerReconcileCause::Teleport)
        .unwrap();
    assert!(touching.active_overlaps.is_empty());

    move_collider(&mut colliders, subject, Vec3::new(-2.0, 0.0, 0.0));
    triggers
        .reconcile(colliders.clone(), 2, TriggerReconcileCause::Teleport)
        .unwrap();
    move_collider(&mut colliders, subject, Vec3::new(2.0, 0.0, 0.0));
    let through = triggers
        .reconcile(colliders.clone(), 3, TriggerReconcileCause::Teleport)
        .unwrap();
    assert!(
        through.facts.is_empty(),
        "teleports sample endpoint geometry"
    );

    move_collider(&mut colliders, subject, Vec3::ZERO);
    triggers
        .reconcile(colliders.clone(), 4, TriggerReconcileCause::Spawn)
        .unwrap();
    set_collision(&mut colliders, trigger, false);
    let inactive = triggers
        .reconcile(
            colliders.clone(),
            5,
            TriggerReconcileCause::ActivationChanged,
        )
        .unwrap();
    assert_eq!(inactive.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert_eq!(
        inactive.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::InactiveCollision
    );

    set_collision(&mut colliders, trigger, true);
    let reactivated = triggers
        .reconcile(
            colliders.clone(),
            6,
            TriggerReconcileCause::ActivationChanged,
        )
        .unwrap();
    assert_eq!(reactivated.facts[0].kind, TriggerOverlapFactKind::Enter);

    colliders.retain(|collider| collider.entity != subject);
    let destroyed = triggers
        .reconcile(colliders, 7, TriggerReconcileCause::LifecycleChanged)
        .unwrap();
    assert_eq!(destroyed.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(destroyed.active_overlaps.is_empty());
}

#[test]
fn trigger_snapshot_restore_preserves_pairs_without_duplicate_enter() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    move_collider(&mut colliders, subject, Vec3::ZERO);
    triggers
        .reconcile(colliders.clone(), 1, TriggerReconcileCause::Teleport)
        .unwrap();
    let encoded = encode_trigger_snapshot(&triggers).unwrap();
    let mut restored = decode_trigger_snapshot(&encoded).unwrap();

    assert_eq!(
        restored.current_overlaps(trigger).unwrap().subjects,
        vec![subject]
    );
    let receipt = restored
        .reconcile(colliders, 2, TriggerReconcileCause::Restore)
        .unwrap();
    assert!(receipt.facts.is_empty());
    assert_eq!(receipt.continued.len(), 1);
    assert_eq!(restored, triggers);

    let mut noncanonical = triggers.snapshot();
    noncanonical.definitions[0].tags.push("exit".to_string());
    assert_eq!(
        TriggerVolumeSystem::from_snapshot(noncanonical)
            .unwrap_err()
            .diagnostics[0]
            .code,
        TriggerVolumeDiagnosticCode::SnapshotInvariant
    );

    let unknown = encoded.replacen("\"revision\": 1", "\"revision\": 1, \"mystery\": true", 1);
    assert_eq!(
        decode_trigger_snapshot(&unknown).unwrap_err().diagnostics[0].code,
        TriggerVolumeDiagnosticCode::SnapshotDecode
    );
}

#[test]
fn trigger_lifecycle_retirement_and_reactivation_are_deliberate() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    move_collider(&mut colliders, subject, Vec3::ZERO);
    let entered = triggers
        .reconcile(colliders.clone(), 1, TriggerReconcileCause::Movement)
        .unwrap();
    assert_eq!(entered.revision, 1);

    let retired = triggers.set_active(trigger, false, 2).unwrap();
    assert!(!retired.active);
    assert_eq!((retired.revision_before, retired.revision_after), (1, 2));
    assert_eq!(retired.removed_overlaps.len(), 1);
    assert_eq!(retired.facts.len(), 1);
    assert_eq!(retired.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(triggers
        .current_overlaps(trigger)
        .unwrap()
        .subjects
        .is_empty());
    let restored_inactive = decode_trigger_snapshot(&encode_trigger_snapshot(&triggers).unwrap())
        .expect("inactive lifecycle state round-trips");
    assert!(!restored_inactive.is_active(trigger).unwrap());

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
    let reentered = triggers
        .reconcile(colliders, 5, TriggerReconcileCause::Movement)
        .unwrap();
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
    let entered = triggers
        .reconcile(colliders, 1, TriggerReconcileCause::Spawn)
        .unwrap();
    assert_eq!(entered.facts.len(), 2);

    let restored = triggers.restore(&[trigger_b], colliders).unwrap();
    assert_eq!((restored.revision_before, restored.revision_after), (1, 2));
    assert_eq!(restored.registered_count, 2);
    assert_eq!(restored.active_count, 1);
    assert_eq!(restored.active_overlaps.len(), 1);
    assert_eq!(restored.active_overlaps[0].trigger_id(), trigger_b);
    assert!(!triggers.is_active(trigger_a).unwrap());
    assert!(triggers.is_active(trigger_b).unwrap());
    let after = triggers
        .reconcile(colliders, 2, TriggerReconcileCause::Restore)
        .unwrap();
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
    let receipt = stale
        .reconcile([], 1, TriggerReconcileCause::Scheduled)
        .unwrap();
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
fn current_overlaps_report_every_subject_and_the_revision() {
    let (mut colliders, mut triggers, trigger, subject) = trigger_fixture();
    move_collider(&mut colliders, subject, Vec3::ZERO);
    let reconcile = triggers
        .reconcile(colliders, 1, TriggerReconcileCause::Teleport)
        .unwrap();

    let readout = triggers.current_overlaps(trigger).unwrap();
    assert_eq!(readout.revision, reconcile.revision);
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

    let outside = triggers
        .reconcile(colliders.clone(), 1, TriggerReconcileCause::Spawn)
        .unwrap();
    assert!(outside.facts.is_empty());
    assert!(outside.diagnostics.is_empty());

    move_collider(&mut colliders, subject, Vec3::new(-0.5, 0.0, 0.0));
    let entered = triggers
        .reconcile(colliders.clone(), 2, TriggerReconcileCause::Movement)
        .unwrap();
    assert_eq!(entered.facts.len(), 1);
    assert_eq!(entered.facts[0].kind, TriggerOverlapFactKind::Enter);
    assert_eq!(entered.facts[0].pair.trigger_id(), trigger);
    assert_eq!(entered.facts[0].pair.subject_id(), subject);
    assert!(entered.diagnostics.is_empty());

    move_collider(&mut colliders, subject, Vec3::new(1.5, 0.0, 0.0));
    let exited = triggers
        .reconcile(colliders, 3, TriggerReconcileCause::Movement)
        .unwrap();
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

    let receipt = triggers
        .reconcile(colliders, 1, TriggerReconcileCause::Spawn)
        .unwrap();
    assert!(receipt.facts.is_empty());
    assert!(receipt.active_overlaps.is_empty());
    assert!(receipt.diagnostics.is_empty());
}

#[test]
fn trigger_snapshots_preserve_geometry_source_and_decode_legacy_definitions() {
    // Snapshots written before the geometry seam existed carry no geometry
    // field; they decode as the historical active-collision behavior.
    let legacy = r#"{
  "schemaVersion": 1,
  "revision": 0,
  "definitions": [
    {
      "trigger": 10,
      "scope": "zone.exit",
      "tags": [
        "exit"
      ]
    }
  ],
  "activeOverlaps": []
}
"#;
    let restored = decode_trigger_snapshot(legacy).unwrap();
    let definition = restored.definitions().next().unwrap();
    assert_eq!(
        definition.geometry_source(),
        TriggerGeometrySource::ActiveCollision
    );

    // New snapshots round-trip the geometry source exactly.
    let system = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        EntityId::new(10),
        "zone.sensor",
        ["zone"],
    )
    .with_geometry_source(TriggerGeometrySource::EntityBounds)])
    .unwrap();
    let encoded = encode_trigger_snapshot(&system).unwrap();
    assert!(encoded.contains("\"geometry\": \"entityBounds\""));
    let restored = decode_trigger_snapshot(&encoded).unwrap();
    assert_eq!(restored, system);
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
