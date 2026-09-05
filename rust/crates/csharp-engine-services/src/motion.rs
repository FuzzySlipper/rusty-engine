use std::ffi::c_void;

use core_ids::EntityId;
use csharp_engine_abi::*;
use engine_spatial::{
    EntityMotionCommand, EntityMotionOutcome, EntityMotionService, MotionSpatialEntity,
};
use entity_state::{BoundsComponent, EntityTransform};

use crate::composition::{
    borrowed_slice, native_quat, native_quat_value, native_vec3, native_vec3_value,
    CsharpEngineServicesError, ABI_OK,
};

const MAX_MOTION_ENTITIES: usize = 1_024;

pub(crate) fn api() -> NativeMotionApi {
    NativeMotionApi {
        context: std::ptr::null_mut(),
        resolve,
    }
}

fn resolve_motion(
    request: &NativeMotionResolveRequest,
) -> Result<NativeMotionResolveReceipt, CsharpEngineServicesError> {
    let rows =
        unsafe { borrowed_slice(request.entities, request.entities_len, "motion entity rows") }?;
    if rows.len() > MAX_MOTION_ENTITIES {
        return Err(motion_error(
            "CSHARP_MOTION_ENTITIES",
            format!("motion request exceeded its {MAX_MOTION_ENTITIES}-entity bound"),
        ));
    }
    let view = rows.iter().map(native_spatial_entity).collect::<Vec<_>>();
    let resolution = EntityMotionService
        .resolve_spatial_view(
            &view,
            EntityMotionCommand {
                entity: EntityId::new(request.target_entity_id),
                delta: native_vec3_value(request.delta),
            },
        )
        .map_err(|error| motion_error("CSHARP_MOTION_RESOLVE", error))?;
    let current = view
        .iter()
        .find(|entity| entity.entity == resolution.entity)
        .expect("successful motion resolution retains target transform")
        .transform;
    let (outcome, blocked_axes) = match resolution.outcome {
        EntityMotionOutcome::Moved { .. } => (NativeMotionOutcome::Moved, [false; 3]),
        EntityMotionOutcome::Blocked { .. } => (
            NativeMotionOutcome::Blocked,
            [
                request.delta.x != 0.0,
                request.delta.y != 0.0,
                request.delta.z != 0.0,
            ],
        ),
        EntityMotionOutcome::Slid { blocked_axes, .. } => (NativeMotionOutcome::Slid, blocked_axes),
    };
    let candidate_transform = EntityTransform {
        translation: resolution.resolved_position(),
        ..current
    };
    Ok(NativeMotionResolveReceipt {
        outcome,
        blocked_x: blocked_axes[0],
        blocked_y: blocked_axes[1],
        blocked_z: blocked_axes[2],
        has_hit: resolution.hit.is_some(),
        hit_entity_id: resolution.hit.map(EntityId::raw).unwrap_or_default(),
        from: native_vec3(resolution.from),
        resolved_position: native_vec3(resolution.resolved_position()),
        candidate_transform: native_transform(candidate_transform),
    })
}

fn native_spatial_entity(row: &NativeMotionSpatialEntity) -> MotionSpatialEntity {
    MotionSpatialEntity {
        entity: EntityId::new(row.entity_id),
        transform: native_transform_value(row.transform),
        bounds: BoundsComponent {
            min: native_vec3_value(row.bounds_min),
            max: native_vec3_value(row.bounds_max),
        },
        collision_enabled: row.collision_enabled,
        collision_static: row.collision_static,
        has_transform_parent: row.has_transform_parent,
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

unsafe extern "C" fn resolve(
    _context: *mut c_void,
    request: *const NativeMotionResolveRequest,
    receipt: *mut NativeMotionResolveReceipt,
) -> i32 {
    if request.is_null() || receipt.is_null() {
        return 0;
    }
    match resolve_motion(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

fn motion_error(code: &'static str, detail: impl std::fmt::Display) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform(x: f32, y: f32) -> NativeTransform {
        NativeTransform {
            translation: NativeVec3 { x, y, z: 0.0 },
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

    fn row(entity_id: u64, x: f32, y: f32, static_collider: bool) -> NativeMotionSpatialEntity {
        NativeMotionSpatialEntity {
            entity_id,
            transform: transform(x, y),
            bounds_min: NativeVec3 {
                x: -0.5,
                y: -0.5,
                z: -0.5,
            },
            bounds_max: NativeVec3 {
                x: 0.5,
                y: 0.5,
                z: 0.5,
            },
            collision_enabled: true,
            collision_static: static_collider,
            ..Default::default()
        }
    }

    fn resolve(
        rows: &[NativeMotionSpatialEntity],
        delta: NativeVec3,
    ) -> NativeMotionResolveReceipt {
        resolve_motion(&NativeMotionResolveRequest {
            target_entity_id: 1,
            delta,
            entities: rows.as_ptr(),
            entities_len: rows.len(),
        })
        .unwrap()
    }

    #[test]
    fn motion_resolution_is_call_local_for_moved_blocked_and_slid_outcomes() {
        let mover = row(1, 0.0, 0.0, false);
        let wall = row(2, 2.0, 0.0, true);
        let moved = resolve(
            &[mover],
            NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        );
        assert_eq!(moved.outcome, NativeMotionOutcome::Moved);
        assert_eq!(moved.resolved_position.x, 1.0);

        let blocked = resolve(
            &[mover, wall],
            NativeVec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
        );
        assert_eq!(blocked.outcome, NativeMotionOutcome::Blocked);
        assert!(blocked.blocked_x && blocked.has_hit && blocked.hit_entity_id == 2);
        assert_eq!(blocked.candidate_transform.translation.x, 0.0);

        let slid = resolve(
            &[mover, wall],
            NativeVec3 {
                x: 2.0,
                y: 1.0,
                z: 0.0,
            },
        );
        assert_eq!(slid.outcome, NativeMotionOutcome::Slid);
        assert!(slid.blocked_x && !slid.blocked_y);
        assert_eq!(slid.resolved_position.y, 1.0);
    }

    #[test]
    fn rejected_motion_does_not_retain_or_publish_call_rows() {
        let mover = row(1, 0.0, 0.0, false);
        let rejected_rows = [mover];
        let rejected = resolve_motion(&NativeMotionResolveRequest {
            target_entity_id: 1,
            delta: NativeVec3 {
                x: f32::NAN,
                y: 0.0,
                z: 0.0,
            },
            entities: rejected_rows.as_ptr(),
            entities_len: 1,
        });
        assert!(rejected.is_err());
        let moved = resolve(
            &[mover],
            NativeVec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        );
        assert_eq!(moved.resolved_position.x, 1.0);
    }
}
