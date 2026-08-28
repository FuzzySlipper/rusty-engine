//! Direct, call-local bridge for Engine kinematic integration.
//!
//! The only collision source is the immutable snapshot owned by a named
//! Spatial session. This module deliberately exposes neither a collision query
//! callback nor a backend representation.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
};

use core_ids::EntityId;
use core_time::TickDelta;
use csharp_engine_abi::*;
use engine_spatial::{
    integrate_kinematic, integrate_kinematic_with_query, CollisionMode, IntegrationResult,
    KinematicBody, KinematicMotionSystem, KinematicShape, MotionAxis, MotionFact, MotionPhaseError,
    PhysicsError, PhysicsStep, PhysicsWorld,
};
use entity_state::{EntityDefinition, EntityState, EntityTransform};

use crate::{
    composition::{
        borrowed_slice, native_quat, native_quat_value, native_vec3, native_vec3_value, ABI_OK,
    },
    spatial::RuntimeSpatialBridge,
};

const MAX_KINEMATIC_MOTION_ROWS: usize = 1_024;
const MAX_KINEMATIC_MOTION_SELECTION: usize = 1_024;

pub(crate) struct KinematicMotionLeaseBacking {
    _candidates: Box<[NativeKinematicMotionCandidate]>,
    _facts: Box<[NativeKinematicMotionFact]>,
}

fn native_body(value: NativeKinematicBody) -> KinematicBody {
    KinematicBody {
        position: native_vec3_value(value.position),
        velocity: native_vec3_value(value.velocity),
        acceleration: native_vec3_value(value.acceleration),
        gravity_scale: value.gravity_scale,
        collision_mode: match value.collision_mode {
            NativeKinematicCollisionMode::None => CollisionMode::None,
            NativeKinematicCollisionMode::SpatialSession => CollisionMode::QueryRequired,
        },
    }
}

fn native_step(value: NativePhysicsStep) -> Result<PhysicsStep, PhysicsError> {
    PhysicsStep::new(TickDelta::new(value.ticks), value.seconds_per_tick)
}

fn native_world(value: NativePhysicsWorld) -> PhysicsWorld {
    PhysicsWorld {
        gravity: native_vec3_value(value.gravity),
    }
}

fn native_shape(value: NativeKinematicShape) -> Result<KinematicShape, PhysicsError> {
    KinematicShape::new(native_vec3_value(value.half_extents))
}

fn native_result(value: IntegrationResult) -> NativeIntegrationResult {
    NativeIntegrationResult {
        previous_position: native_vec3(value.previous_position),
        next_position: native_vec3(value.next_position),
        previous_velocity: native_vec3(value.previous_velocity),
        next_velocity: native_vec3(value.next_velocity),
        elapsed_seconds: value.elapsed_seconds,
        blocked_x: value.collision.blocked_axes[0],
        blocked_y: value.collision.blocked_axes[1],
        blocked_z: value.collision.blocked_axes[2],
    }
}

fn physics_status(error: PhysicsError) -> i32 {
    match error {
        PhysicsError::InvalidStep { .. } => NativeKinematicErrorStatus::InvalidStep as i32,
        PhysicsError::StepOverflow => NativeKinematicErrorStatus::StepOverflow as i32,
        PhysicsError::CollisionQueryRequired => {
            NativeKinematicErrorStatus::CollisionQueryRequired as i32
        }
        PhysicsError::InvalidShape => NativeKinematicErrorStatus::InvalidShape as i32,
        PhysicsError::NonFiniteInput => NativeKinematicErrorStatus::NonFiniteInput as i32,
    }
}

fn motion_status(error: MotionPhaseError) -> i32 {
    match error {
        MotionPhaseError::InvalidDeltaSeconds { .. } => {
            NativeKinematicErrorStatus::InvalidMotionDelta as i32
        }
        MotionPhaseError::EntityBatch(_) => NativeKinematicErrorStatus::MotionBatchRejected as i32,
    }
}

fn native_motion_axis(axis: MotionAxis) -> NativeKinematicMotionAxis {
    match axis {
        MotionAxis::X => NativeKinematicMotionAxis::X,
        MotionAxis::Y => NativeKinematicMotionAxis::Y,
        MotionAxis::Z => NativeKinematicMotionAxis::Z,
    }
}

fn native_motion_fact(fact: MotionFact) -> NativeKinematicMotionFact {
    match fact {
        MotionFact::Moved {
            entity,
            before,
            after,
        } => NativeKinematicMotionFact {
            entity_id: entity.raw(),
            kind: NativeKinematicMotionFactKind::Moved,
            axis: NativeKinematicMotionAxis::X,
            before: native_vec3(before),
            after: native_vec3(after),
            attempted_delta: 0.0,
        },
        MotionFact::Blocked {
            entity,
            axis,
            attempted_delta,
        } => NativeKinematicMotionFact {
            entity_id: entity.raw(),
            kind: NativeKinematicMotionFactKind::Blocked,
            axis: native_motion_axis(axis),
            before: NativeVec3::default(),
            after: NativeVec3::default(),
            attempted_delta,
        },
    }
}

fn native_transform_value(value: NativeTransform) -> EntityTransform {
    EntityTransform {
        translation: native_vec3_value(value.translation),
        rotation: native_quat_value(value.rotation),
        scale: native_vec3_value(value.scale),
    }
}

fn native_transform(value: EntityTransform) -> NativeTransform {
    NativeTransform {
        translation: native_vec3(value.translation),
        rotation: native_quat(value.rotation),
        scale: native_vec3(value.scale),
    }
}

fn native_motion_entity(row: &NativeKinematicMotionEntityRow) -> EntityDefinition {
    EntityDefinition::new(
        EntityId::new(row.entity_id),
        format!("product-{}", row.entity_id),
    )
    .with_full_transform(native_transform_value(row.transform))
    .with_collision(row.collision_enabled, row.collision_static)
    .with_kinematic(
        native_vec3_value(row.half_extents),
        native_vec3_value(row.velocity),
    )
}

fn build_motion_lease(
    bridge: &mut RuntimeSpatialBridge,
    mut entities: EntityState,
    scene: &engine_spatial::VoxelCollisionScene,
    request: &NativeKinematicMotionRequest,
    selected: &BTreeSet<EntityId>,
) -> Result<NativeKinematicMotionLease, i32> {
    let before: BTreeMap<_, _> = entities
        .kinematic_bodies()
        .map(|body| {
            (
                body.entity,
                (
                    entities
                        .transform(body.entity)
                        .expect("kinematic body retains a transform")
                        .transform(),
                    body.velocity,
                ),
            )
        })
        .collect();
    let receipt = if request.selection_present {
        KinematicMotionSystem::run_selected(&mut entities, scene, request.delta_seconds, selected)
    } else {
        KinematicMotionSystem::run(&mut entities, scene, request.delta_seconds)
    }
    .map_err(motion_status)?;
    let candidates = before
        .into_iter()
        .filter_map(|(entity, (before_transform, before_velocity))| {
            let after_transform = entities.transform(entity)?.transform();
            let after_velocity = entities.kinematic(entity)?.velocity;
            (after_transform != before_transform || after_velocity != before_velocity).then_some(
                NativeKinematicMotionCandidate {
                    entity_id: entity.raw(),
                    before_transform: native_transform(before_transform),
                    after_transform: native_transform(after_transform),
                    before_velocity: native_vec3(before_velocity),
                    after_velocity: native_vec3(after_velocity),
                },
            )
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let facts = receipt
        .facts
        .into_iter()
        .map(native_motion_fact)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let handle_value = bridge.next_kinematic_motion_lease;
    bridge.next_kinematic_motion_lease = handle_value.checked_add(1).ok_or(0)?;
    let lease = NativeKinematicMotionLease {
        handle: NativeKinematicMotionLeaseHandle {
            value: handle_value,
        },
        candidates: candidates.as_ptr(),
        candidates_len: candidates.len(),
        facts: facts.as_ptr(),
        facts_len: facts.len(),
        bodies_considered: u64::try_from(receipt.bodies_considered).map_err(|_| 0)?,
        moved_bodies: u64::try_from(receipt.moved_bodies).map_err(|_| 0)?,
        blocked_axes: u64::try_from(receipt.blocked_axes).map_err(|_| 0)?,
        revision_before: receipt.revision_before,
        revision_after: receipt.revision_after,
    };
    bridge.kinematic_motion_leases.insert(
        handle_value,
        KinematicMotionLeaseBacking {
            _candidates: candidates,
            _facts: facts,
        },
    );
    Ok(lease)
}

unsafe extern "C" fn integrate(
    _context: *mut c_void,
    request: NativeKinematicIntegrationRequest,
    result: *mut NativeIntegrationResult,
) -> i32 {
    if result.is_null() {
        return 0;
    }
    let value = native_step(request.step).and_then(|step| {
        integrate_kinematic(native_body(request.body), native_world(request.world), step)
    });
    match value {
        Ok(value) => {
            // SAFETY: result is an out pointer borrowed for this ABI call.
            unsafe { *result = native_result(value) };
            ABI_OK
        }
        Err(error) => physics_status(error),
    }
}

unsafe extern "C" fn integrate_spatial(
    context: *mut c_void,
    request: NativeKinematicSpatialIntegrationRequest,
    result: *mut NativeIntegrationResult,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: this exact context is supplied by `api` for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    let scene = match bridge.collision_source().scene(request.session) {
        Ok(scene) => scene,
        Err(_) => return 0,
    };
    let value = native_step(request.step).and_then(|step| {
        native_shape(request.shape).and_then(|shape| {
            integrate_kinematic_with_query(
                native_body(request.body),
                native_world(request.world),
                step,
                shape,
                scene.as_ref(),
            )
        })
    });
    match value {
        Ok(value) => {
            // SAFETY: result is an out pointer borrowed for this ABI call.
            unsafe { *result = native_result(value) };
            ABI_OK
        }
        Err(error) => physics_status(error),
    }
}

unsafe extern "C" fn run_motion(
    context: *mut c_void,
    request: *const NativeKinematicMotionRequest,
    result: *mut NativeKinematicMotionLease,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: request is borrowed for this direct ABI call after its null check.
    let request = unsafe { &*request };
    let rows =
        match unsafe { borrowed_slice(request.rows, request.rows_len, "kinematic motion rows") } {
            Ok(rows) if rows.len() <= MAX_KINEMATIC_MOTION_ROWS => rows,
            _ => return 0,
        };
    let selected_ids = match unsafe {
        borrowed_slice(
            request.selected_entity_ids,
            request.selected_entity_ids_len,
            "kinematic motion selected entity ids",
        )
    } {
        Ok(ids) if ids.len() <= MAX_KINEMATIC_MOTION_SELECTION => ids,
        _ => return 0,
    };
    if !request.selection_present && !selected_ids.is_empty() {
        return 0;
    }
    let selected = selected_ids.iter().copied().map(EntityId::new).collect();
    let entities = match EntityState::from_definitions(rows.iter().map(native_motion_entity)) {
        Ok(entities) => entities,
        Err(_) => return NativeKinematicErrorStatus::InvalidMotionRows as i32,
    };
    // SAFETY: this exact context is supplied by `api` for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    let scene = match bridge.collision_source().scene(request.session) {
        Ok(scene) => scene,
        Err(_) => return 0,
    };
    match build_motion_lease(bridge, entities, scene.as_ref(), request, &selected) {
        Ok(value) => {
            // SAFETY: result is an out pointer borrowed for this ABI call.
            unsafe { *result = value };
            ABI_OK
        }
        Err(status) => status,
    }
}

unsafe extern "C" fn destroy_motion_lease(
    context: *mut c_void,
    handle: NativeKinematicMotionLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    // SAFETY: this exact context is supplied by `api` for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    i32::from(
        bridge
            .kinematic_motion_leases
            .remove(&handle.value)
            .is_some(),
    )
}

pub(crate) fn api(bridge: &mut RuntimeSpatialBridge) -> NativeKinematicApi {
    NativeKinematicApi {
        context: (bridge as *mut RuntimeSpatialBridge).cast(),
        integrate,
        integrate_spatial,
        run_motion,
        destroy_motion_lease,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{spatial, voxel};

    fn session(api: &NativeSpatialApi) -> NativeSpatialSessionHandle {
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (api.create_session)(
                    api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        reserved: 0,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        session
    }

    fn body(
        position: NativeVec3,
        velocity: NativeVec3,
        collision_mode: NativeKinematicCollisionMode,
    ) -> NativeKinematicBody {
        NativeKinematicBody {
            position,
            velocity,
            acceleration: NativeVec3::default(),
            gravity_scale: 1.0,
            collision_mode,
        }
    }

    fn step(ticks: u64, seconds_per_tick: f32) -> NativePhysicsStep {
        NativePhysicsStep {
            ticks,
            seconds_per_tick,
        }
    }

    fn assert_vec3(actual: NativeVec3, expected: NativeVec3) {
        assert_eq!(
            (actual.x, actual.y, actual.z),
            (expected.x, expected.y, expected.z)
        );
    }

    fn transform(position: NativeVec3) -> NativeTransform {
        NativeTransform {
            translation: position,
            rotation: NativeQuat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            scale: NativeVec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        }
    }

    fn motion_row(
        entity_id: u64,
        position: NativeVec3,
        velocity: NativeVec3,
    ) -> NativeKinematicMotionEntityRow {
        NativeKinematicMotionEntityRow {
            entity_id,
            transform: transform(position),
            half_extents: NativeVec3 {
                x: 0.4,
                y: 0.4,
                z: 0.4,
            },
            velocity,
            collision_enabled: true,
            collision_static: false,
        }
    }

    #[test]
    fn gravity_integration_is_call_local_and_complete() {
        let mut spatial = RuntimeSpatialBridge::new();
        let api = super::api(&mut spatial);
        let mut result = NativeIntegrationResult::default();
        assert_eq!(
            unsafe {
                (api.integrate)(
                    api.context,
                    NativeKinematicIntegrationRequest {
                        body: body(
                            NativeVec3::default(),
                            NativeVec3::default(),
                            NativeKinematicCollisionMode::None,
                        ),
                        world: NativePhysicsWorld {
                            gravity: NativeVec3 {
                                x: 0.0,
                                y: -9.8,
                                z: 0.0,
                            },
                        },
                        step: step(1, 0.5),
                    },
                    &mut result,
                )
            },
            ABI_OK
        );
        assert_vec3(result.previous_position, NativeVec3::default());
        assert_vec3(result.previous_velocity, NativeVec3::default());
        assert_vec3(
            result.next_velocity,
            NativeVec3 {
                x: 0.0,
                y: -4.9,
                z: 0.0,
            },
        );
        assert_vec3(
            result.next_position,
            NativeVec3 {
                x: 0.0,
                y: -2.45,
                z: 0.0,
            },
        );
        assert_eq!(result.elapsed_seconds, 0.5);
        assert!(!result.blocked_x && !result.blocked_y && !result.blocked_z);
    }

    #[test]
    fn spatial_session_snapshot_reports_the_blocked_axis() {
        let mut spatial = RuntimeSpatialBridge::new();
        let spatial_api = spatial::api(&mut spatial);
        let voxel_api = voxel::api(&mut spatial);
        let kinematic_api = super::api(&mut spatial);
        let session = session(&spatial_api);
        let edits = [NativeVoxelEdit {
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 1, y: 0, z: 0 },
            material_slot: 1,
        }];
        let mut edit_receipt = NativeVoxelEditReceipt::default();
        assert_eq!(
            unsafe {
                (voxel_api.apply_edits)(
                    voxel_api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        expected_revision: 0,
                        edits: edits.as_ptr(),
                        edits_len: edits.len(),
                    },
                    &mut edit_receipt,
                )
            },
            ABI_OK
        );
        let mut result = NativeIntegrationResult::default();
        assert_eq!(
            unsafe {
                (kinematic_api.integrate_spatial)(
                    kinematic_api.context,
                    NativeKinematicSpatialIntegrationRequest {
                        session,
                        body: body(
                            NativeVec3 {
                                x: 0.0,
                                y: 0.5,
                                z: 0.5,
                            },
                            NativeVec3 {
                                x: 2.0,
                                y: 0.0,
                                z: 1.0,
                            },
                            NativeKinematicCollisionMode::SpatialSession,
                        ),
                        world: NativePhysicsWorld::default(),
                        step: step(1, 0.5),
                        shape: NativeKinematicShape {
                            half_extents: NativeVec3 {
                                x: 0.4,
                                y: 0.4,
                                z: 0.4,
                            },
                        },
                    },
                    &mut result,
                )
            },
            ABI_OK
        );
        assert!(result.blocked_x && !result.blocked_y && !result.blocked_z);
        assert_vec3(
            result.next_position,
            NativeVec3 {
                x: 0.0,
                y: 0.5,
                z: 1.0,
            },
        );
        assert_vec3(
            result.next_velocity,
            NativeVec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        );
    }

    #[test]
    fn physics_errors_have_distinct_stable_statuses() {
        let mut spatial = RuntimeSpatialBridge::new();
        let spatial_api = spatial::api(&mut spatial);
        let api = super::api(&mut spatial);
        let session = session(&spatial_api);
        let request = NativeKinematicIntegrationRequest {
            body: body(
                NativeVec3::default(),
                NativeVec3::default(),
                NativeKinematicCollisionMode::None,
            ),
            world: NativePhysicsWorld::default(),
            step: step(1, 1.0),
        };
        let mut result = NativeIntegrationResult::default();
        let invalid_step = NativeKinematicIntegrationRequest {
            step: step(1, 0.0),
            ..request
        };
        assert_eq!(
            unsafe { (api.integrate)(api.context, invalid_step, &mut result) },
            NativeKinematicErrorStatus::InvalidStep as i32
        );
        let overflow = NativeKinematicIntegrationRequest {
            step: step(u64::MAX, f32::MAX),
            ..request
        };
        assert_eq!(
            unsafe { (api.integrate)(api.context, overflow, &mut result) },
            NativeKinematicErrorStatus::StepOverflow as i32
        );
        let query_required = NativeKinematicIntegrationRequest {
            body: body(
                NativeVec3::default(),
                NativeVec3::default(),
                NativeKinematicCollisionMode::SpatialSession,
            ),
            ..request
        };
        assert_eq!(
            unsafe { (api.integrate)(api.context, query_required, &mut result) },
            NativeKinematicErrorStatus::CollisionQueryRequired as i32
        );
        let non_finite = NativeKinematicIntegrationRequest {
            body: body(
                NativeVec3 {
                    x: f32::INFINITY,
                    y: 0.0,
                    z: 0.0,
                },
                NativeVec3::default(),
                NativeKinematicCollisionMode::None,
            ),
            ..request
        };
        assert_eq!(
            unsafe { (api.integrate)(api.context, non_finite, &mut result) },
            NativeKinematicErrorStatus::NonFiniteInput as i32
        );
        let invalid_shape = NativeKinematicSpatialIntegrationRequest {
            session,
            body: body(
                NativeVec3::default(),
                NativeVec3::default(),
                NativeKinematicCollisionMode::SpatialSession,
            ),
            world: NativePhysicsWorld::default(),
            step: step(1, 1.0),
            shape: NativeKinematicShape {
                half_extents: NativeVec3::default(),
            },
        };
        assert_eq!(
            unsafe { (api.integrate_spatial)(api.context, invalid_shape, &mut result) },
            NativeKinematicErrorStatus::InvalidShape as i32
        );
    }

    #[test]
    fn selected_motion_returns_deterministic_changed_candidates_and_facts() {
        let mut spatial = RuntimeSpatialBridge::new();
        let spatial_api = spatial::api(&mut spatial);
        let api = super::api(&mut spatial);
        let session = session(&spatial_api);
        let rows = [
            motion_row(
                1,
                NativeVec3::default(),
                NativeVec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 2.0,
                },
            ),
            // This selected body overlaps the mover's X sweep, but selected
            // bodies are deliberately excluded from the dynamic blocker set.
            motion_row(
                2,
                NativeVec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                NativeVec3::default(),
            ),
            // This unselected enabled body blocks the mover after its X move.
            motion_row(
                3,
                NativeVec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 1.0,
                },
                NativeVec3::default(),
            ),
        ];
        let selected = [1_u64, 2_u64];
        let request = NativeKinematicMotionRequest {
            session,
            delta_seconds: 1.0,
            rows: rows.as_ptr(),
            rows_len: rows.len(),
            selection_present: true,
            selected_entity_ids: selected.as_ptr(),
            selected_entity_ids_len: selected.len(),
        };
        let mut first = NativeKinematicMotionLease {
            handle: NativeKinematicMotionLeaseHandle::default(),
            candidates: std::ptr::null(),
            candidates_len: 0,
            facts: std::ptr::null(),
            facts_len: 0,
            bodies_considered: 0,
            moved_bodies: 0,
            blocked_axes: 0,
            revision_before: 0,
            revision_after: 0,
        };
        assert_eq!(
            unsafe { (api.run_motion)(api.context, &request, &mut first) },
            ABI_OK
        );
        assert_eq!(first.bodies_considered, 2);
        assert_eq!(first.moved_bodies, 1);
        assert_eq!(first.blocked_axes, 1);
        assert_eq!((first.revision_before, first.revision_after), (0, 1));
        let candidates =
            unsafe { std::slice::from_raw_parts(first.candidates, first.candidates_len) };
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].entity_id, 1);
        assert_vec3(
            candidates[0].after_transform.translation,
            NativeVec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
        );
        assert_vec3(
            candidates[0].after_velocity,
            NativeVec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
        );
        let facts = unsafe { std::slice::from_raw_parts(first.facts, first.facts_len) };
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].kind, NativeKinematicMotionFactKind::Blocked);
        assert_eq!(facts[0].entity_id, 1);
        assert_eq!(facts[0].axis, NativeKinematicMotionAxis::Z);
        assert_eq!(facts[1].kind, NativeKinematicMotionFactKind::Moved);
        assert_eq!(facts[1].entity_id, 1);

        let mut second = first;
        assert_eq!(
            unsafe { (api.run_motion)(api.context, &request, &mut second) },
            ABI_OK
        );
        let second_facts = unsafe { std::slice::from_raw_parts(second.facts, second.facts_len) };
        assert_eq!(
            facts
                .iter()
                .map(|fact| (fact.kind, fact.entity_id, fact.axis, fact.attempted_delta))
                .collect::<Vec<_>>(),
            second_facts
                .iter()
                .map(|fact| (fact.kind, fact.entity_id, fact.axis, fact.attempted_delta))
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            unsafe { (api.destroy_motion_lease)(api.context, first.handle) },
            1
        );
        assert_eq!(
            unsafe { (api.destroy_motion_lease)(api.context, second.handle) },
            1
        );
    }
}
