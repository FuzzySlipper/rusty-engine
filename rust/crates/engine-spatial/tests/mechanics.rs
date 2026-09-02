use core_ids::EntityId;
use core_math::Vec3;
use core_time::TickDelta;
use engine_spatial::{
    decode_trigger_snapshot, encode_trigger_snapshot, integrate_kinematic,
    integrate_kinematic_with_query, EntityMotionCommand, EntityMotionOutcome, EntityMotionService,
    KinematicBody, KinematicShape, KinematicTriggerDefinition, MaterialVoxel, PhysicsError,
    PhysicsStep, PhysicsWorld, TriggerGeometrySource, TriggerOverlapFactKind,
    TriggerReconcileCause, TriggerVolumeDiagnosticCode, TriggerVolumeSystem, VoxelCollisionScene,
};
use entity_state::{
    EntityAuthoringService, EntityCommand, EntityCommandBatch, EntityDefinition, EntityState,
    EntityTransform,
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
                address,
                material_slot,
            }),
    )
    .unwrap();

    assert_eq!(scene.solid_voxel_count(), tunnel.voxels.len());
    assert!(scene.contains_point([0.5, 0.5, 0.5]));
    assert!(!scene.contains_point([2.5, 2.5, 2.5]));
    assert_eq!(scene.resident_chunk_count(), 1);
    assert!(!scene.mesh_chunks().is_empty());
    assert!(scene.navigation_cell_count() > 0);
}

#[test]
fn trigger_enter_continue_and_exit_are_reconciled_once() {
    let (mut entities, mut triggers, trigger, subject) = trigger_fixture();
    let empty = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Scheduled)
        .unwrap();
    assert!(empty.facts.is_empty());

    move_entity(&mut entities, subject, Vec3::ZERO);
    let entered = triggers
        .reconcile(&entities, 2, TriggerReconcileCause::Teleport)
        .unwrap();
    assert_eq!(entered.facts.len(), 1);
    assert_eq!(entered.facts[0].kind, TriggerOverlapFactKind::Enter);
    assert_eq!(entered.facts[0].pair.trigger_id(), trigger);

    let continued = triggers
        .reconcile(&entities, 3, TriggerReconcileCause::Scheduled)
        .unwrap();
    assert!(continued.facts.is_empty());
    assert_eq!(continued.continued, entered.active_overlaps);
    assert_eq!(continued.revision, entered.revision);

    move_entity(&mut entities, subject, Vec3::new(2.0, 0.0, 0.0));
    let exited = triggers
        .reconcile(&entities, 4, TriggerReconcileCause::Teleport)
        .unwrap();
    assert_eq!(exited.facts.len(), 1);
    assert_eq!(exited.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(exited.active_overlaps.is_empty());
}

#[test]
fn trigger_endpoint_activation_lifecycle_and_face_touching_semantics_are_explicit() {
    let (mut entities, mut triggers, trigger, subject) = trigger_fixture();
    move_entity(&mut entities, subject, Vec3::new(1.0, 0.0, 0.0));
    let touching = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Teleport)
        .unwrap();
    assert!(touching.active_overlaps.is_empty());

    move_entity(&mut entities, subject, Vec3::new(-2.0, 0.0, 0.0));
    triggers
        .reconcile(&entities, 2, TriggerReconcileCause::Teleport)
        .unwrap();
    move_entity(&mut entities, subject, Vec3::new(2.0, 0.0, 0.0));
    let through = triggers
        .reconcile(&entities, 3, TriggerReconcileCause::Teleport)
        .unwrap();
    assert!(
        through.facts.is_empty(),
        "teleports sample endpoint geometry"
    );

    move_entity(&mut entities, subject, Vec3::ZERO);
    triggers
        .reconcile(&entities, 4, TriggerReconcileCause::Spawn)
        .unwrap();
    entities
        .apply_batch(EntityCommandBatch::new([
            EntityCommand::SetCollisionEnabled {
                entity: trigger,
                enabled: false,
            },
        ]))
        .unwrap();
    let inactive = triggers
        .reconcile(&entities, 5, TriggerReconcileCause::ActivationChanged)
        .unwrap();
    assert_eq!(inactive.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert_eq!(
        inactive.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::InactiveCollision
    );

    entities
        .apply_batch(EntityCommandBatch::new([
            EntityCommand::SetCollisionEnabled {
                entity: trigger,
                enabled: true,
            },
        ]))
        .unwrap();
    let reactivated = triggers
        .reconcile(&entities, 6, TriggerReconcileCause::ActivationChanged)
        .unwrap();
    assert_eq!(reactivated.facts[0].kind, TriggerOverlapFactKind::Enter);

    let entity_revision = entities.revision();
    EntityAuthoringService
        .destroy(&mut entities, entity_revision, subject)
        .unwrap();
    let destroyed = triggers
        .reconcile(&entities, 7, TriggerReconcileCause::LifecycleChanged)
        .unwrap();
    assert_eq!(destroyed.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(destroyed.active_overlaps.is_empty());
}

#[test]
fn trigger_geometry_uses_entity_state_world_transform_composition() {
    let parent = EntityId::new(1);
    let trigger = EntityId::new(10);
    let subject = EntityId::new(20);
    let entities = EntityState::from_definitions([
        EntityDefinition::new(parent, "zone parent")
            .with_full_transform(EntityTransform::at(Vec3::new(5.0, 0.0, 0.0))),
        EntityDefinition::new(trigger, "child zone")
            .with_transform(Vec3::new(-5.0, 0.0, 0.0))
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, true)
            .with_transform_parent(parent),
        EntityDefinition::new(subject, "subject")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.25), Vec3::splat(0.25))
            .with_collision(true, false),
    ])
    .unwrap();
    let mut triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        trigger,
        "zone.parented",
        ["zone"],
    )])
    .unwrap();

    let receipt = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Spawn)
        .unwrap();
    assert_eq!(receipt.facts[0].kind, TriggerOverlapFactKind::Enter);
    assert_eq!(receipt.facts[0].pair.subject_id(), subject);
}

#[test]
fn trigger_snapshot_restore_preserves_pairs_without_duplicate_enter() {
    let (mut entities, mut triggers, trigger, subject) = trigger_fixture();
    move_entity(&mut entities, subject, Vec3::ZERO);
    triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Teleport)
        .unwrap();
    let encoded = encode_trigger_snapshot(&triggers).unwrap();
    let mut restored = decode_trigger_snapshot(&encoded).unwrap();

    assert_eq!(
        restored.current_overlaps(trigger, 1).unwrap().subjects,
        vec![subject]
    );
    let receipt = restored
        .reconcile(&entities, 2, TriggerReconcileCause::Restore)
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
fn trigger_lifecycle_retirement_is_revision_guarded_and_reactivation_is_deliberate() {
    let (mut entities, mut triggers, trigger, subject) = trigger_fixture();
    move_entity(&mut entities, subject, Vec3::ZERO);
    let entered = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Movement)
        .unwrap();
    assert_eq!(entered.revision, 1);

    let retired = triggers.set_active(trigger, 1, false, 2).unwrap();
    assert!(!retired.active);
    assert_eq!((retired.revision_before, retired.revision_after), (1, 2));
    assert_eq!(retired.removed_overlaps.len(), 1);
    assert_eq!(retired.facts.len(), 1);
    assert_eq!(retired.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(triggers
        .current_overlaps(trigger, 1)
        .unwrap()
        .subjects
        .is_empty());
    let restored_inactive = decode_trigger_snapshot(&encode_trigger_snapshot(&triggers).unwrap())
        .expect("inactive lifecycle state round-trips");
    assert!(!restored_inactive.is_active(trigger).unwrap());

    let unchanged = triggers.clone();
    let duplicate = triggers.set_active(trigger, 2, false, 3).unwrap_err();
    assert_eq!(
        duplicate.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::DuplicateLifecycle
    );
    assert_eq!(triggers, unchanged);
    let stale = triggers.set_active(trigger, 1, true, 3).unwrap_err();
    assert_eq!(
        stale.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::StaleRevision
    );
    assert_eq!(triggers, unchanged);
    let unknown = triggers
        .set_active(EntityId::new(999), 2, false, 3)
        .unwrap_err();
    assert_eq!(
        unknown.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::MissingDefinition
    );
    assert_eq!(triggers, unchanged);

    let reactivated = triggers.set_active(trigger, 2, true, 4).unwrap();
    assert!(reactivated.active);
    assert!(reactivated.facts.is_empty());
    let reentered = triggers
        .reconcile(&entities, 5, TriggerReconcileCause::Movement)
        .unwrap();
    assert_eq!(reentered.facts.len(), 1);
    assert_eq!(reentered.facts[0].kind, TriggerOverlapFactKind::Enter);
}

#[test]
fn trigger_restore_rebases_active_set_and_overlaps_without_edges() {
    let trigger_a = EntityId::new(10);
    let trigger_b = EntityId::new(11);
    let subject = EntityId::new(20);
    let entities = EntityState::from_definitions([
        EntityDefinition::new(trigger_a, "zone a")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, true),
        EntityDefinition::new(trigger_b, "zone b")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, true),
        EntityDefinition::new(subject, "subject")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.25), Vec3::splat(0.25))
            .with_collision(true, false),
    ])
    .unwrap();
    let mut triggers = TriggerVolumeSystem::new([
        KinematicTriggerDefinition::new(trigger_a, "zone.a", ["zone"]),
        KinematicTriggerDefinition::new(trigger_b, "zone.b", ["zone"]),
    ])
    .unwrap();
    let entered = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Spawn)
        .unwrap();
    assert_eq!(entered.facts.len(), 2);

    let restored = triggers.restore(&[trigger_b], &entities, 1).unwrap();
    assert_eq!((restored.revision_before, restored.revision_after), (1, 2));
    assert_eq!(restored.registered_count, 2);
    assert_eq!(restored.active_count, 1);
    assert_eq!(restored.active_overlaps.len(), 1);
    assert_eq!(restored.active_overlaps[0].trigger_id(), trigger_b);
    assert!(!triggers.is_active(trigger_a).unwrap());
    assert!(triggers.is_active(trigger_b).unwrap());
    let after = triggers
        .reconcile(&entities, 2, TriggerReconcileCause::Restore)
        .unwrap();
    assert!(after.facts.is_empty());
    assert_eq!(after.continued.len(), 1);

    let unchanged = triggers.clone();
    let duplicate = triggers
        .restore(&[trigger_b, trigger_b], &entities, 2)
        .unwrap_err();
    assert_eq!(
        duplicate.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::DuplicateLifecycle
    );
    assert_eq!(triggers, unchanged);
    let unknown = triggers
        .restore(&[EntityId::new(999)], &entities, 2)
        .unwrap_err();
    assert_eq!(
        unknown.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::MissingDefinition
    );
    assert_eq!(triggers, unchanged);
}

#[test]
fn malformed_definitions_stale_entities_and_read_quotas_are_typed() {
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
        .reconcile(&EntityState::default(), 1, TriggerReconcileCause::Scheduled)
        .unwrap();
    assert_eq!(
        receipt.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::StaleEntity
    );

    let (mut entities, mut triggers, trigger, subject) = trigger_fixture();
    move_entity(&mut entities, subject, Vec3::ZERO);
    triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Teleport)
        .unwrap();
    assert_eq!(
        triggers
            .current_overlaps(trigger, 0)
            .unwrap_err()
            .diagnostics[0]
            .code,
        TriggerVolumeDiagnosticCode::QuotaExceeded
    );
}

#[test]
fn trigger_overlap_pages_report_total_empty_final_page_and_stale_revision() {
    let (mut entities, mut triggers, trigger, subject) = trigger_fixture();
    move_entity(&mut entities, subject, Vec3::ZERO);
    let reconcile = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Teleport)
        .unwrap();

    let first = triggers.current_overlaps_page(trigger, None, 0, 1).unwrap();
    assert_eq!(first.revision, reconcile.revision);
    assert_eq!(first.total, 1);
    assert_eq!(first.subjects, vec![subject]);
    assert_eq!(first.next_cursor, None);

    let final_empty = triggers
        .current_overlaps_page(trigger, Some(first.revision), 1, 1)
        .unwrap();
    assert!(final_empty.subjects.is_empty());
    assert_eq!(final_empty.total, 1);
    assert_eq!(final_empty.next_cursor, None);
    assert_eq!(
        triggers
            .current_overlaps_page(trigger, Some(first.revision + 1), 1, 1)
            .unwrap_err()
            .diagnostics[0]
            .code,
        TriggerVolumeDiagnosticCode::StaleRevision
    );
}

#[test]
fn non_solid_trigger_senses_kinematic_traversal_with_one_enter_and_one_exit() {
    let trigger = EntityId::new(10);
    let subject = EntityId::new(20);
    let mut entities = EntityState::from_definitions([
        // A registered trigger volume with bounds and transform but no collision
        // component: it must never become a solid motion obstacle.
        EntityDefinition::new(trigger, "sensor zone")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5)),
        EntityDefinition::new(subject, "player")
            .with_transform(Vec3::new(-2.0, 0.0, 0.0))
            .with_bounds(Vec3::splat(-0.25), Vec3::splat(0.25))
            .with_collision(true, false),
    ])
    .unwrap();
    let mut triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        trigger,
        "zone.sensor",
        ["zone"],
    )
    .with_geometry_source(TriggerGeometrySource::EntityBounds)])
    .unwrap();

    let outside = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Spawn)
        .unwrap();
    assert!(outside.facts.is_empty());
    assert!(outside.diagnostics.is_empty());

    // Motion treats only active collision as solid: the subject enters the
    // registered non-solid trigger volume without being blocked.
    let revision = entities.revision();
    let entered_motion = EntityMotionService
        .apply(
            &mut entities,
            revision,
            EntityMotionCommand {
                entity: subject,
                delta: Vec3::new(1.5, 0.0, 0.0),
            },
        )
        .unwrap();
    assert_eq!(
        entered_motion.resolution.outcome,
        EntityMotionOutcome::Moved {
            to: Vec3::new(-0.5, 0.0, 0.0)
        }
    );
    let entered = triggers
        .reconcile(&entities, 2, TriggerReconcileCause::Movement)
        .unwrap();
    assert_eq!(entered.facts.len(), 1);
    assert_eq!(entered.facts[0].kind, TriggerOverlapFactKind::Enter);
    assert_eq!(entered.facts[0].pair.trigger_id(), trigger);
    assert_eq!(entered.facts[0].pair.subject_id(), subject);
    assert!(entered.diagnostics.is_empty());

    // The traversal continues through the volume and out the other side,
    // again unblocked, producing exactly one exit.
    let revision = entities.revision();
    let exited_motion = EntityMotionService
        .apply(
            &mut entities,
            revision,
            EntityMotionCommand {
                entity: subject,
                delta: Vec3::new(2.0, 0.0, 0.0),
            },
        )
        .unwrap();
    assert!(matches!(
        exited_motion.resolution.outcome,
        EntityMotionOutcome::Moved { .. }
    ));
    let exited = triggers
        .reconcile(&entities, 3, TriggerReconcileCause::Movement)
        .unwrap();
    assert_eq!(exited.facts.len(), 1);
    assert_eq!(exited.facts[0].kind, TriggerOverlapFactKind::Exit);
    assert!(exited.active_overlaps.is_empty());
}

#[test]
fn entity_bounds_trigger_ignores_collision_state_and_keeps_geometry_diagnostics() {
    let sensor = EntityId::new(10);
    let unbounded = EntityId::new(11);
    let subject = EntityId::new(20);
    let entities = EntityState::from_definitions([
        // Collision component present but disabled: irrelevant for an
        // entity-bounds trigger and no longer reported as a diagnostic.
        EntityDefinition::new(sensor, "disabled-collision sensor")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(false, true),
        // Missing bounds still fail closed with an actionable diagnostic.
        EntityDefinition::new(unbounded, "unbounded sensor")
            .with_transform(Vec3::new(10.0, 0.0, 0.0)),
        EntityDefinition::new(subject, "player")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.25), Vec3::splat(0.25))
            .with_collision(true, false),
    ])
    .unwrap();
    let mut triggers = TriggerVolumeSystem::new([
        KinematicTriggerDefinition::new(sensor, "zone.sensor", ["zone"])
            .with_geometry_source(TriggerGeometrySource::EntityBounds),
        KinematicTriggerDefinition::new(unbounded, "zone.unbounded", ["zone"])
            .with_geometry_source(TriggerGeometrySource::EntityBounds),
    ])
    .unwrap();

    let receipt = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Spawn)
        .unwrap();
    assert_eq!(receipt.facts.len(), 1);
    assert_eq!(receipt.facts[0].kind, TriggerOverlapFactKind::Enter);
    assert_eq!(receipt.facts[0].pair.trigger_id(), sensor);
    assert_eq!(receipt.diagnostics.len(), 1);
    assert_eq!(
        receipt.diagnostics[0].code,
        TriggerVolumeDiagnosticCode::MissingBounds
    );
    assert_eq!(receipt.diagnostics[0].entity, Some(unbounded));
}

#[test]
fn entity_bounds_trigger_keeps_canonical_subject_eligibility() {
    let trigger = EntityId::new(10);
    let subject = EntityId::new(20);
    let entities = EntityState::from_definitions([
        EntityDefinition::new(trigger, "sensor zone")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5)),
        // Subjects still require active collision even when the trigger does
        // not: a subject with disabled collision is not sensed.
        EntityDefinition::new(subject, "inactive subject")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.25), Vec3::splat(0.25))
            .with_collision(false, false),
    ])
    .unwrap();
    let mut triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        trigger,
        "zone.sensor",
        ["zone"],
    )
    .with_geometry_source(TriggerGeometrySource::EntityBounds)])
    .unwrap();

    let receipt = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Spawn)
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

fn trigger_fixture() -> (EntityState, TriggerVolumeSystem, EntityId, EntityId) {
    let trigger = EntityId::new(10);
    let subject = EntityId::new(20);
    let entities = EntityState::from_definitions([
        EntityDefinition::new(trigger, "exit zone")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, true),
        EntityDefinition::new(subject, "player")
            .with_transform(Vec3::new(2.0, 0.0, 0.0))
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, false),
    ])
    .unwrap();
    let triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        trigger,
        "zone.exit",
        ["door", "exit"],
    )])
    .unwrap();
    (entities, triggers, trigger, subject)
}

fn move_entity(entities: &mut EntityState, entity: EntityId, translation: Vec3) {
    entities
        .apply_batch(EntityCommandBatch::new([EntityCommand::SetTranslation {
            entity,
            translation,
        }]))
        .unwrap();
}
