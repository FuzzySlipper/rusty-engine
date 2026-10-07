use std::{ffi::c_void, sync::Arc};

use crate::operation_diagnostics::{clear_receipt, refuse};
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
                    exclude_outside_envelope: request.exclude_outside_envelope,
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
    ) -> Result<NativeWorldOriginPreparedResult, CsharpEngineServicesError> {
        let owner = self.prepared_world_origin(request.prepared)?;
        let target = owner.candidate.target_origin().cell();
        let local_envelope = self
            .sessions
            .get(&owner.session)
            .map_or(0.0, |session| session.world_origin.local_envelope());
        let affected = owner
            .candidate
            .affected_transforms()
            .iter()
            .map(|value| NativeWorldOriginAffectedTransform {
                entity_id: value.entity.raw(),
                local_transform: native_transform(value.transform),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let excluded = owner
            .candidate
            .excluded()
            .iter()
            .map(|entity| NativeWorldOriginExcludedEntity {
                entity_id: entity.raw(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let result = NativeWorldOriginPreparedResult {
            affected: affected.as_ptr(),
            affected_len: affected.len(),
            excluded: excluded.as_ptr(),
            excluded_len: excluded.len(),
            target_cell_x: target[0],
            target_cell_y: target[1],
            target_cell_z: target[2],
            local_envelope,
        };
        self.borrowed.hold((affected, excluded));
        Ok(result)
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
            session.rebase_collision(
                Arc::clone(&scene),
                receipt.origin_before.cell(),
                receipt.origin_after.cell(),
            );
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
            excluded_entity_count: receipt.excluded_count as u32,
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
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
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
        Err(refusal) => refuse(&refusal, error),
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    request: NativeWorldOriginReadRequest,
    readout: *mut NativeWorldOriginReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_world_origin(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

unsafe extern "C" fn read_prepared(
    context: *mut c_void,
    request: NativeWorldOriginPreparedReadRequest,
    result: *mut NativeWorldOriginPreparedResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .read_prepared_world_origin(request)
    {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

unsafe extern "C" fn commit(
    context: *mut c_void,
    request: NativeWorldOriginCommitRequest,
    receipt: *mut NativeWorldOriginCommitReceipt,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.commit_world_origin(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
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
                    std::ptr::null_mut(),
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
            exclude_outside_envelope: false,
        }
    }

    #[test]
    fn a_prepare_may_exclude_the_rows_outside_the_envelope_and_names_them() {
        let mut bridge = RuntimeSpatialBridge::new();
        let spatial_api = spatial::api(&mut bridge);
        let world_origin_api = api(&mut bridge);
        let session = session(&spatial_api);
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        // A root 20 km from the target is outside the 16,384 m envelope.
        let rows = [row(1, 0.0, 100), row(2, 0.0, 20_100), row(3, 0.0, 101)];
        let mut request = prepare_request(session, 100, &rows);
        let mut prepared = NativeWorldOriginPreparedHandle::default();
        // Without opting in, one such row refuses the whole request.
        assert_ne!(
            unsafe {
                (world_origin_api.prepare)(
                    world_origin_api.context,
                    &request,
                    &mut prepared,
                    &mut error,
                )
            },
            ABI_OK
        );
        request.exclude_outside_envelope = true;
        assert_eq!(
            unsafe {
                (world_origin_api.prepare)(
                    world_origin_api.context,
                    &request,
                    &mut prepared,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut summary: NativeWorldOriginPreparedResult = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (world_origin_api.read_prepared)(
                    world_origin_api.context,
                    NativeWorldOriginPreparedReadRequest { prepared },
                    &mut summary,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let affected =
            unsafe { std::slice::from_raw_parts(summary.affected, summary.affected_len) }.to_vec();
        let excluded =
            unsafe { std::slice::from_raw_parts(summary.excluded, summary.excluded_len) }.to_vec();
        assert_eq!(
            affected.iter().map(|row| row.entity_id).collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(affected[1].local_transform.translation.x, 1.0);
        assert_eq!(excluded, [NativeWorldOriginExcludedEntity { entity_id: 2 }]);
        let mut receipt = NativeWorldOriginCommitReceipt::default();
        assert_eq!(
            unsafe {
                (world_origin_api.commit)(
                    world_origin_api.context,
                    NativeWorldOriginCommitRequest { prepared },
                    &mut receipt,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(receipt.origin_after_cell_x, 100);
        assert_eq!(receipt.affected_entity_count, 2);
        assert_eq!(receipt.excluded_entity_count, 1);

        // Every row excluded still moves the origin.
        let far = [row(7, 0.0, 50_000), row(8, 0.0, -50_000)];
        let mut request = prepare_request(session, 200, &far);
        request.exclude_outside_envelope = true;
        assert_eq!(
            unsafe {
                (world_origin_api.prepare)(
                    world_origin_api.context,
                    &request,
                    &mut prepared,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (world_origin_api.read_prepared)(
                    world_origin_api.context,
                    NativeWorldOriginPreparedReadRequest { prepared },
                    &mut summary,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!((summary.affected_len, summary.excluded_len), (0, 2));
        assert_eq!(
            unsafe {
                (world_origin_api.commit)(
                    world_origin_api.context,
                    NativeWorldOriginCommitRequest { prepared },
                    &mut receipt,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(receipt.origin_after_cell_x, 200);
        assert_eq!(
            (receipt.affected_entity_count, receipt.excluded_entity_count),
            (0, 2)
        );
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
                (world_origin_api.prepare)(
                    world_origin_api.context,
                    &request,
                    &mut prepared,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );

        let mut summary: NativeWorldOriginPreparedResult = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (world_origin_api.read_prepared)(
                    world_origin_api.context,
                    NativeWorldOriginPreparedReadRequest { prepared },
                    &mut summary,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(summary.target_cell_x, 100);
        assert_eq!(summary.local_envelope, 16_384.0);
        // Copy the borrowed rows before the next call, as generated C# does.
        let affected =
            unsafe { std::slice::from_raw_parts(summary.affected, summary.affected_len) }.to_vec();
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].entity_id, 41);
        assert_eq!(affected[0].local_transform.translation.x, 0.0);
        assert_eq!(affected[0].local_transform.translation.y, 2.0);
        assert_eq!(affected[0].local_transform.translation.z, -3.0);
        assert_eq!(affected[0].local_transform.scale.x, 2.0);
        assert_eq!(affected[0].local_transform.rotation.w, 1.0);

        let mut receipt = NativeWorldOriginCommitReceipt::default();
        assert_eq!(
            unsafe {
                (world_origin_api.commit)(
                    world_origin_api.context,
                    NativeWorldOriginCommitRequest { prepared },
                    &mut receipt,
                    std::ptr::null_mut(),
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
                        std::ptr::null_mut(),
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
                        std::ptr::null_mut(),
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
