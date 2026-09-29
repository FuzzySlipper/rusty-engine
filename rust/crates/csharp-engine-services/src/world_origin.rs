use std::{ffi::c_void, sync::Arc};

use core_ids::EntityId;
use core_space::{GlobalPosition, WorldOrigin};
use csharp_engine_abi::*;
use engine_spatial::{
    PreparedWorldOriginRebase, WorldOriginRebaseRequest, WorldOriginRebaseService,
};

use crate::{
    composition::{
        borrowed_slice, native_quat, native_quat_value, native_vec3, native_vec3_value,
        CsharpEngineServicesError, ABI_OK,
    },
    spatial::RuntimeSpatialBridge,
};

/// Disposable native ownership retained between the product's explicit
/// prepare/read/commit calls: the target origin and the rebased local
/// transforms. It holds no scene and no product entity state.
pub(crate) struct PreparedWorldOriginOwner {
    pub(crate) session: u64,
    candidate: PreparedWorldOriginRebase,
}

impl RuntimeSpatialBridge {
    fn prepare_world_origin(
        &mut self,
        request: &NativeWorldOriginPrepareRequest,
    ) -> Result<NativeWorldOriginPreparedHandle, CsharpEngineServicesError> {
        let rows = unsafe {
            borrowed_slice(
                request.entities,
                request.entities_len,
                "world-origin entity rows",
            )
        }?;
        let entities = rows
            .iter()
            .map(|row| {
                Ok(engine_spatial::WorldOriginEntity {
                    entity: EntityId::new(row.entity_id),
                    transform: native_entity_transform(row.local_transform),
                    global_position: native_global_position(row.global_position)?,
                })
            })
            .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?;
        let session = self.session_mut(request.session)?;
        let candidate = WorldOriginRebaseService
            .prepare(
                &session.world_origin,
                WorldOriginRebaseRequest {
                    target_origin: WorldOrigin::new([
                        request.target_cell_x,
                        request.target_cell_y,
                        request.target_cell_z,
                    ]),
                    entities,
                },
            )
            .map_err(|error| world_origin_error("CSHARP_WORLD_ORIGIN_PREPARE", error))?;
        let value = self.next_world_origin_prepared;
        self.next_world_origin_prepared = self
            .next_world_origin_prepared
            .checked_add(1)
            .ok_or_else(|| {
                world_origin_error(
                    "CSHARP_WORLD_ORIGIN_PREPARE",
                    "world-origin prepared handles exhausted",
                )
            })?;
        self.prepared_world_origins.insert(
            value,
            PreparedWorldOriginOwner {
                session: request.session.value,
                candidate,
            },
        );
        Ok(NativeWorldOriginPreparedHandle { value })
    }

    fn read_world_origin(
        &mut self,
        request: NativeWorldOriginReadRequest,
    ) -> Result<NativeWorldOriginReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(native_readout(
            session.world_origin.readout(),
            session.scene.as_ref(),
        ))
    }

    fn read_prepared_world_origin(
        &mut self,
        request: NativeWorldOriginPreparedReadRequest,
    ) -> Result<NativeWorldOriginPreparedReadout, CsharpEngineServicesError> {
        let owner = self.prepared_world_origin(request.prepared)?;
        let target = owner.candidate.target_origin().cell();
        let local_envelope = self
            .sessions
            .get(&owner.session)
            .map_or(0.0, |session| session.world_origin.local_envelope());
        Ok(NativeWorldOriginPreparedReadout {
            present: true,
            target_cell_x: target[0],
            target_cell_y: target[1],
            target_cell_z: target[2],
            affected_entity_count: owner.candidate.affected_transforms().len() as u32,
            local_envelope,
        })
    }

    fn read_world_origin_affected_at(
        &mut self,
        request: NativeWorldOriginAffectedAtRequest,
    ) -> Result<NativeWorldOriginAffectedAtReceipt, CsharpEngineServicesError> {
        let owner = self.prepared_world_origin(request.prepared)?;
        Ok(owner
            .candidate
            .affected_transforms()
            .get(request.index as usize)
            .copied()
            .map(|value| NativeWorldOriginAffectedAtReceipt {
                present: true,
                entity_id: value.entity.raw(),
                local_transform: native_transform(value.transform),
            })
            .unwrap_or_default())
    }

    fn commit_world_origin(
        &mut self,
        request: NativeWorldOriginCommitRequest,
    ) -> Result<NativeWorldOriginCommitReceipt, CsharpEngineServicesError> {
        let session_id = self.prepared_world_origin(request.prepared)?.session;
        let handle = NativeSpatialSessionHandle { value: session_id };
        let (scene, receipt) = {
            let (sessions, prepared) = (&mut self.sessions, &self.prepared_world_origins);
            let owner = prepared.get(&request.prepared.value).ok_or_else(|| {
                world_origin_error(
                    "CSHARP_WORLD_ORIGIN_PREPARED",
                    "C# used an unknown, committed, or disposed prepared world-origin candidate",
                )
            })?;
            let session = sessions.get_mut(&session_id).ok_or_else(|| {
                world_origin_error(
                    "CSHARP_SPATIAL_SESSION",
                    "C# used an unknown or disposed spatial session",
                )
            })?;
            let (scene, receipt) = WorldOriginRebaseService
                .commit(&mut session.world_origin, &session.scene, &owner.candidate)
                .map_err(|error| world_origin_error("CSHARP_WORLD_ORIGIN_COMMIT", error))?;
            let scene = Arc::new(scene);
            session.scene = Arc::clone(&scene);
            (scene, receipt)
        };
        self.publish_scene(handle, scene);
        self.prepared_world_origins.remove(&request.prepared.value);
        Ok(NativeWorldOriginCommitReceipt {
            revision_before: receipt.revision_before,
            revision_after: receipt.revision_after,
            origin_before_cell_x: receipt.origin_before.cell()[0],
            origin_before_cell_y: receipt.origin_before.cell()[1],
            origin_before_cell_z: receipt.origin_before.cell()[2],
            origin_after_cell_x: receipt.origin_after.cell()[0],
            origin_after_cell_y: receipt.origin_after.cell()[1],
            origin_after_cell_z: receipt.origin_after.cell()[2],
            voxel_source_revision: receipt.voxel_source_revision,
            static_mesh_revision: receipt.static_mesh_revision,
            affected_entity_count: receipt.entity_count as u32,
            local_envelope: receipt.local_envelope,
        })
    }

    fn destroy_prepared_world_origin(&mut self, handle: NativeWorldOriginPreparedHandle) {
        self.prepared_world_origins.remove(&handle.value);
    }

    fn prepared_world_origin(
        &self,
        handle: NativeWorldOriginPreparedHandle,
    ) -> Result<&PreparedWorldOriginOwner, CsharpEngineServicesError> {
        self.prepared_world_origins
            .get(&handle.value)
            .ok_or_else(|| {
                world_origin_error(
                    "CSHARP_WORLD_ORIGIN_PREPARED",
                    "C# used an unknown, committed, or disposed prepared world-origin candidate",
                )
            })
    }
}

fn native_global_position(
    value: NativeWorldOriginGlobalPosition,
) -> Result<GlobalPosition, CsharpEngineServicesError> {
    GlobalPosition::new(
        [value.cell_x, value.cell_y, value.cell_z],
        [value.offset_x, value.offset_y, value.offset_z],
    )
    .map_err(|error| world_origin_error("CSHARP_WORLD_ORIGIN_POSITION", error))
}

fn native_entity_transform(value: NativeTransform) -> entity_state::EntityTransform {
    entity_state::EntityTransform {
        translation: native_vec3_value(value.translation),
        rotation: native_quat_value(value.rotation),
        scale: native_vec3_value(value.scale),
    }
}

fn native_transform(value: entity_state::EntityTransform) -> NativeTransform {
    NativeTransform {
        translation: native_vec3(value.translation),
        rotation: native_quat(value.rotation),
        scale: native_vec3(value.scale),
    }
}

fn native_readout(
    origin: engine_spatial::WorldOriginReadout,
    scene: &engine_spatial::VoxelCollisionScene,
) -> NativeWorldOriginReadout {
    NativeWorldOriginReadout {
        cell_x: origin.origin.cell()[0],
        cell_y: origin.origin.cell()[1],
        cell_z: origin.origin.cell()[2],
        revision: origin.revision,
        local_envelope: origin.local_envelope,
        voxel_source_revision: scene.source_revision().raw(),
        static_mesh_revision: scene.static_mesh_collision_revision(),
    }
}

unsafe extern "C" fn prepare(
    context: *mut c_void,
    request: *const NativeWorldOriginPrepareRequest,
    handle: *mut NativeWorldOriginPreparedHandle,
) -> i32 {
    if context.is_null() || request.is_null() || handle.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .prepare_world_origin(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    request: NativeWorldOriginReadRequest,
    readout: *mut NativeWorldOriginReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_world_origin(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_prepared(
    context: *mut c_void,
    request: NativeWorldOriginPreparedReadRequest,
    readout: *mut NativeWorldOriginPreparedReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .read_prepared_world_origin(request)
    {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_affected_at(
    context: *mut c_void,
    request: NativeWorldOriginAffectedAtRequest,
    receipt: *mut NativeWorldOriginAffectedAtReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .read_world_origin_affected_at(request)
    {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn commit(
    context: *mut c_void,
    request: NativeWorldOriginCommitRequest,
    receipt: *mut NativeWorldOriginCommitReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.commit_world_origin(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_prepared(
    context: *mut c_void,
    handle: NativeWorldOriginPreparedHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.destroy_prepared_world_origin(handle);
    ABI_OK
}

pub(crate) fn api(bridge: &mut RuntimeSpatialBridge) -> NativeWorldOriginApi {
    NativeWorldOriginApi {
        context: (bridge as *mut RuntimeSpatialBridge).cast(),
        prepare,
        read,
        read_prepared,
        read_affected_at,
        commit,
        destroy_prepared,
    }
}

fn world_origin_error(
    code: &'static str,
    detail: impl std::fmt::Display,
) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial;

    fn session(api: &NativeSpatialApi) -> NativeSpatialSessionHandle {
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (api.create_session)(
                    api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 16,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        session
    }

    fn transform(x: f32) -> NativeTransform {
        NativeTransform {
            translation: NativeVec3 { x, y: 2.0, z: -3.0 },
            rotation: NativeQuat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            scale: NativeVec3 {
                x: 2.0,
                y: 3.0,
                z: 4.0,
            },
        }
    }

    fn row(entity_id: u64, local_x: f32, global_x: i64) -> NativeWorldOriginEntityRow {
        NativeWorldOriginEntityRow {
            entity_id,
            local_transform: transform(local_x),
            global_position: NativeWorldOriginGlobalPosition {
                cell_x: global_x,
                cell_y: 2,
                cell_z: -3,
                ..Default::default()
            },
        }
    }

    fn prepare_request(
        session: NativeSpatialSessionHandle,
        target_x: i64,
        rows: &[NativeWorldOriginEntityRow],
    ) -> NativeWorldOriginPrepareRequest {
        NativeWorldOriginPrepareRequest {
            session,
            target_cell_x: target_x,
            target_cell_y: 0,
            target_cell_z: 0,
            entities: rows.as_ptr(),
            entities_len: rows.len(),
        }
    }

    #[test]
    fn prepared_world_origin_commits_scene_and_exact_copied_transforms() {
        let mut bridge = RuntimeSpatialBridge::new();
        let spatial_api = spatial::api(&mut bridge);
        let world_origin_api = api(&mut bridge);
        let session = session(&spatial_api);
        let rows = [row(41, 100.0, 100)];
        let request = prepare_request(session, 100, &rows);
        let mut prepared = NativeWorldOriginPreparedHandle::default();
        assert_eq!(
            unsafe {
                (world_origin_api.prepare)(world_origin_api.context, &request, &mut prepared)
            },
            ABI_OK
        );

        let mut summary = NativeWorldOriginPreparedReadout::default();
        assert_eq!(
            unsafe {
                (world_origin_api.read_prepared)(
                    world_origin_api.context,
                    NativeWorldOriginPreparedReadRequest { prepared },
                    &mut summary,
                )
            },
            ABI_OK
        );
        assert_eq!(summary.target_cell_x, 100);
        assert_eq!(summary.local_envelope, 16_384.0);
        assert_eq!(summary.affected_entity_count, 1);

        let mut affected = NativeWorldOriginAffectedAtReceipt::default();
        assert_eq!(
            unsafe {
                (world_origin_api.read_affected_at)(
                    world_origin_api.context,
                    NativeWorldOriginAffectedAtRequest { prepared, index: 0 },
                    &mut affected,
                )
            },
            ABI_OK
        );
        assert!(affected.present);
        assert_eq!(affected.entity_id, 41);
        assert_eq!(affected.local_transform.translation.x, 0.0);
        assert_eq!(affected.local_transform.translation.y, 2.0);
        assert_eq!(affected.local_transform.translation.z, -3.0);
        assert_eq!(affected.local_transform.scale.x, 2.0);
        assert_eq!(affected.local_transform.rotation.w, 1.0);

        let mut receipt = NativeWorldOriginCommitReceipt::default();
        assert_eq!(
            unsafe {
                (world_origin_api.commit)(
                    world_origin_api.context,
                    NativeWorldOriginCommitRequest { prepared },
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(receipt.revision_before, 0);
        assert_eq!(receipt.revision_after, 1);
        assert_eq!(receipt.origin_after_cell_x, 100);
        assert_eq!(receipt.affected_entity_count, 1);
        let scene = bridge.collision_source().scene(session).unwrap();
        assert_eq!(scene.world_origin().cell(), [100, 0, 0]);
        assert_eq!(scene.rebase_revision(), 1);
    }

    #[test]
    fn commit_rebases_the_live_scene_and_keeps_edits_made_after_prepare() {
        let mut bridge = RuntimeSpatialBridge::new();
        let spatial_api = spatial::api(&mut bridge);
        let voxel_api = crate::voxel::api(&mut bridge);
        let world_origin_api = api(&mut bridge);
        let session = session(&spatial_api);
        let prepare = |target_x, rows: &[NativeWorldOriginEntityRow]| {
            let mut prepared = NativeWorldOriginPreparedHandle::default();
            assert_eq!(
                unsafe {
                    (world_origin_api.prepare)(
                        world_origin_api.context,
                        &prepare_request(session, target_x, rows),
                        &mut prepared,
                    )
                },
                ABI_OK
            );
            prepared
        };
        let commit = |prepared| {
            let mut receipt = NativeWorldOriginCommitReceipt::default();
            assert_eq!(
                unsafe {
                    (world_origin_api.commit)(
                        world_origin_api.context,
                        NativeWorldOriginCommitRequest { prepared },
                        &mut receipt,
                    )
                },
                ABI_OK
            );
            receipt
        };

        // Two prepared rebases commit in either order; the last one wins.
        let rows = [row(7, 50.0, 50)];
        let to_50 = prepare(50, &rows);
        let to_75 = prepare(75, &rows);
        assert_eq!(commit(to_75).origin_after_cell_x, 75);
        let last = commit(to_50);
        assert_eq!((last.revision_before, last.revision_after), (1, 2));
        assert_eq!(last.origin_before_cell_x, 75);
        assert_eq!(last.origin_after_cell_x, 50);

        // A voxel edit between prepare and commit survives the rebase.
        let to_60 = prepare(60, &rows);
        let edits = [NativeVoxelEdit {
            state: 0,
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 51, y: 0, z: 0 },
            material_slot: 1,
        }];
        let mut voxel_receipt = NativeVoxelEditReceipt::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (voxel_api.apply_edits)(
                    voxel_api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        edits: edits.as_ptr(),
                        edits_len: edits.len(),
                    },
                    &mut voxel_receipt,
                    &mut error,
                )
            },
            ABI_OK
        );
        let receipt = commit(to_60);
        assert_eq!(
            receipt.voxel_source_revision,
            voxel_receipt.accepted_revision
        );
        let scene = bridge.collision_source().scene(session).unwrap();
        assert_eq!(scene.world_origin().cell(), [60, 0, 0]);
        assert_eq!(
            scene
                .raycast([-8.5, 3.0, 0.5], [0.0, -1.0, 0.0], 4.0)
                .unwrap()
                .voxel,
            [51, 0, 0]
        );
    }
}
