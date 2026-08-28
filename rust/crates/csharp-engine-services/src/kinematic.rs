//! Direct, call-local bridge for Engine kinematic integration.
//!
//! The only collision source is the immutable snapshot owned by a named
//! Spatial session. This module deliberately exposes neither a collision query
//! callback nor a backend representation.

use std::ffi::c_void;

use core_time::TickDelta;
use csharp_engine_abi::*;
use engine_spatial::{
    integrate_kinematic, integrate_kinematic_with_query, CollisionMode, IntegrationResult,
    KinematicBody, KinematicShape, PhysicsError, PhysicsStep, PhysicsWorld,
};

use crate::{
    composition::{native_vec3, native_vec3_value, ABI_OK},
    spatial::RuntimeSpatialBridge,
};

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

pub(crate) fn api(bridge: &mut RuntimeSpatialBridge) -> NativeKinematicApi {
    NativeKinematicApi {
        context: (bridge as *mut RuntimeSpatialBridge).cast(),
        integrate,
        integrate_spatial,
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
}
