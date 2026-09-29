//! Generated direct bridge for the canonical runtime voxel scene.
//!
//! This family works on the Spatial session's scene rather than owning a
//! second voxel store. Edits and residency changes mutate that scene in place,
//! so collision, character motion and Dynamics see them directly.

use std::{ffi::c_void, sync::Arc, time::Instant};

use csharp_engine_abi::*;
use engine_spatial::{
    VoxelChunkIdentity, VoxelChunkPayload, VoxelChunkResidencyApplyError,
    VoxelChunkResidencyOperation, VoxelChunkResidencyRejection, VoxelChunkResidencyService,
    VoxelEdit, VoxelEditApplyError, VoxelEditRejection,
};

use crate::{
    composition::{CsharpEngineServicesError, ABI_OK},
    spatial::RuntimeSpatialBridge,
};

const VOXEL_SERVICE: &[u8] = b"Voxel";
const APPLY_EDITS_OPERATION: &[u8] = b"ApplyEdits";
const APPLY_RESIDENCY_OPERATION: &[u8] = b"ApplyResidency";

/// Voxel mutation failures retain their original Engine diagnostic until the
/// generated managed call has copied it and released this exact lease.
pub(crate) struct VoxelOperationDiagnosticLease {
    _code: Box<str>,
    _message: Box<str>,
    _source: Box<str>,
    diagnostic: NativeEngineDiagnostic,
}

impl VoxelOperationDiagnosticLease {
    fn new(error: &CsharpEngineServicesError) -> Self {
        let code: Box<str> = error.code().into();
        let message: Box<str> = error.detail().into();
        let source: Box<str> = "".into();
        let diagnostic = NativeEngineDiagnostic {
            code: native_utf8(code.as_bytes()),
            message: native_utf8(message.as_bytes()),
            source: native_utf8(source.as_bytes()),
        };
        Self {
            _code: code,
            _message: message,
            _source: source,
            diagnostic,
        }
    }
}

impl RuntimeSpatialBridge {
    fn retain_voxel_operation_diagnostic(
        &mut self,
        error: &CsharpEngineServicesError,
    ) -> Option<NativeEngineDiagnosticLease> {
        let value = self.next_voxel_operation_diagnostic_lease;
        self.next_voxel_operation_diagnostic_lease = value.checked_add(1)?;
        let lease = VoxelOperationDiagnosticLease::new(error);
        self.voxel_operation_diagnostic_leases.insert(value, lease);
        let lease = self.voxel_operation_diagnostic_leases.get(&value)?;
        let diagnostics = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value },
            diagnostics: std::ptr::from_ref(&lease.diagnostic),
            diagnostics_len: 1,
        };
        Some(diagnostics)
    }

    fn destroy_voxel_operation_diagnostic_lease(
        &mut self,
        handle: NativeEngineDiagnosticLeaseHandle,
    ) -> bool {
        handle.value != 0
            && self
                .voxel_operation_diagnostic_leases
                .remove(&handle.value)
                .is_some()
    }

    fn read_voxel_scene(
        &mut self,
        request: NativeVoxelSceneReadRequest,
    ) -> Result<NativeVoxelSceneReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let scene = session.scene.as_ref();
        let update = scene.mesh_update();
        Ok(NativeVoxelSceneReadout {
            present: true,
            voxel_size: scene.voxel_size(),
            chunk_size: scene.chunk_size(),
            source_revision: scene.source_revision().raw(),
            authority_hash: scene.authority_hash(),
            collision_revision: scene.projection_revisions().collision().raw(),
            navigation_revision: scene.projection_revisions().navigation().raw(),
            mesh_revision: scene.projection_revisions().mesh().raw(),
            projection_version: scene.projection_version(),
            resident_chunk_count: scene.resident_chunk_count() as u64,
            collider_chunk_count: scene.collider_chunk_count() as u64,
            solid_voxel_count: scene.solid_voxel_count() as u64,
            navigation_cell_count: scene.navigation_cell_count() as u64,
            navigation_hash: scene.navigation_hash(),
            dirty_chunk_count: narrow(update.dirty_chunks.len()),
            rebuilt_mesh_chunks: narrow(update.rebuilt_chunks),
            reused_mesh_chunks: narrow(update.reused_chunks),
            removed_mesh_chunks: narrow(update.removed_chunks),
        })
    }

    fn read_voxel(
        &mut self,
        request: NativeVoxelReadRequest,
    ) -> Result<NativeVoxelReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let address = address(request.address);
        Ok(session
            .scene
            .material_voxel(address)
            .map(|voxel| NativeVoxelReadout {
                present: true,
                address: request.address,
                material_slot: u32::from(voxel.material_slot),
                state: u32::from(voxel.state),
            })
            .unwrap_or(NativeVoxelReadout {
                present: false,
                address: request.address,
                ..Default::default()
            }))
    }

    fn read_voxel_chunk(
        &mut self,
        request: NativeVoxelChunkReadRequest,
    ) -> Result<NativeVoxelChunkReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(VoxelChunkResidencyService::resident_chunk(
            session.scene.as_ref(),
            chunk_identity(request.chunk),
        )
        .map(native_chunk_readout)
        .unwrap_or(NativeVoxelChunkReadout {
            chunk: request.chunk,
            ..Default::default()
        }))
    }

    fn read_resident_chunk_at(
        &mut self,
        request: NativeVoxelResidentChunkAtRequest,
    ) -> Result<NativeVoxelChunkReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(
            VoxelChunkResidencyService::resident_chunks(session.scene.as_ref())
                .get(request.index as usize)
                .copied()
                .map(native_chunk_readout)
                .unwrap_or_default(),
        )
    }

    fn apply_voxel_edits(
        &mut self,
        request: &NativeVoxelEditTransaction,
    ) -> Result<NativeVoxelEditReceipt, CsharpEngineServicesError> {
        let edits = unsafe {
            crate::composition::borrowed_slice(request.edits, request.edits_len, "voxel edits")
        }?;
        let edits = edits
            .iter()
            .copied()
            .map(native_edit)
            .collect::<Result<Vec<_>, _>>()?;
        self.edit_scene(request.session, |session| {
            let scene = Arc::make_mut(&mut session.scene);
            match engine_spatial::VoxelEditService::apply(scene, &edits) {
                Ok(receipt) => {
                    session.last_voxel_dirty_chunks = receipt.dirty_mesh_chunks.clone();
                    Ok(native_edit_receipt(&receipt))
                }
                Err(VoxelEditApplyError::Rejected(VoxelEditRejection::NoChanges)) => {
                    let revision = scene.source_revision().raw();
                    Ok(NativeVoxelEditReceipt {
                        status: NativeVoxelEditStatus::NoChanges,
                        revision_before: revision,
                        accepted_revision: revision,
                        ..Default::default()
                    })
                }
                Err(error) => Err(voxel_error("CSHARP_VOXEL_EDIT", error.to_string())),
            }
        })?
    }

    fn read_dirty_chunk_at(
        &mut self,
        request: NativeVoxelDirtyChunkAtRequest,
    ) -> Result<NativeVoxelDirtyChunkAtReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(session
            .last_voxel_dirty_chunks
            .get(request.index as usize)
            .copied()
            .map(|chunk| NativeVoxelDirtyChunkAtReceipt {
                present: true,
                chunk: native_chunk(chunk),
            })
            .unwrap_or_default())
    }

    fn apply_voxel_residency(
        &mut self,
        request: &NativeVoxelResidencyTransaction,
    ) -> Result<NativeVoxelResidencyReceipt, CsharpEngineServicesError> {
        let chunk_size = self.session_mut(request.session)?.scene.chunk_size();
        let operations = translate_residency(request, chunk_size)?;
        self.edit_scene(request.session, |session| {
            let scene = Arc::make_mut(&mut session.scene);
            match VoxelChunkResidencyService::apply(scene, &operations) {
                Ok(receipt) => {
                    session.last_voxel_dirty_chunks = receipt
                        .dirty_chunks
                        .iter()
                        .map(|chunk| chunk.to_array())
                        .collect();
                    Ok(native_residency_receipt(&receipt))
                }
                // A batch that changes nothing is an ordinary outcome.
                Err(VoxelChunkResidencyApplyError::Rejected(
                    VoxelChunkResidencyRejection::NoChanges { retained },
                )) => {
                    let revision = scene.source_revision().raw();
                    Ok(NativeVoxelResidencyReceipt {
                        revision_before: revision,
                        accepted_revision: revision,
                        retained_count: narrow(retained.len()),
                        resident_chunk_count: scene.resident_chunk_count() as u64,
                        resident_solid_voxel_count: scene.solid_voxel_count() as u64,
                        authority_hash: scene.authority_hash(),
                        collision_revision: revision,
                        navigation_revision: revision,
                        mesh_revision: revision,
                        ..Default::default()
                    })
                }
                Err(error) => Err(voxel_error("CSHARP_VOXEL_RESIDENCY", error.to_string())),
            }
        })?
    }
}

fn native_edit(value: NativeVoxelEdit) -> Result<VoxelEdit, CsharpEngineServicesError> {
    let native_address_value = value.address;
    match value.kind {
        NativeVoxelEditKind::Set => Ok(VoxelEdit::SetState {
            state: u16::try_from(value.state)
                .map_err(|_| voxel_error("CSHARP_VOXEL_STATE", "state exceeded u16"))?,
            address: address(native_address_value),
            material_slot: u16::try_from(value.material_slot)
                .map_err(|_| voxel_error("CSHARP_VOXEL_EDIT", "material slot exceeded u16"))?,
        }),
        NativeVoxelEditKind::Clear => Ok(VoxelEdit::Clear {
            address: address(native_address_value),
        }),
    }
}

fn native_payload(
    operation: NativeVoxelResidencyOperation,
    chunk_size: u32,
    material_slots: &[u32],
    states: &[u32],
) -> Result<VoxelChunkPayload, CsharpEngineServicesError> {
    let start = usize::try_from(operation.material_offset)
        .map_err(|_| voxel_error("CSHARP_VOXEL_RESIDENCY", "material offset exceeded usize"))?;
    let count = usize::try_from(operation.material_count)
        .map_err(|_| voxel_error("CSHARP_VOXEL_RESIDENCY", "material count exceeded usize"))?;
    let end = start
        .checked_add(count)
        .ok_or_else(|| voxel_error("CSHARP_VOXEL_RESIDENCY", "material range overflowed"))?;
    let values = material_slots.get(start..end).ok_or_else(|| {
        voxel_error(
            "CSHARP_VOXEL_RESIDENCY",
            "material range exceeded the transaction span",
        )
    })?;
    let values = values
        .iter()
        .copied()
        .map(|value| {
            u16::try_from(value)
                .map_err(|_| voxel_error("CSHARP_VOXEL_RESIDENCY", "material slot exceeded u16"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut payload = VoxelChunkPayload::new([chunk_size; 3], values);
    if !states.is_empty() {
        payload.states = states
            .get(start..end)
            .ok_or_else(|| voxel_error("CSHARP_VOXEL_STATE", "state range exceeded span"))?
            .iter()
            .map(|s| {
                u16::try_from(*s)
                    .map_err(|_| voxel_error("CSHARP_VOXEL_STATE", "state exceeded u16"))
            })
            .collect::<Result<_, _>>()?;
    }
    Ok(payload)
}

fn native_edit_receipt(receipt: &engine_spatial::VoxelEditReceipt) -> NativeVoxelEditReceipt {
    NativeVoxelEditReceipt {
        revision_before: receipt.revision_before.raw(),
        accepted_revision: receipt.accepted_revision.raw(),
        solid_voxel_count: receipt.solid_voxel_count as u64,
        authority_hash: receipt.authority_hash,
        collision_revision: receipt.projections.collision().raw(),
        navigation_revision: receipt.projections.navigation().raw(),
        mesh_revision: receipt.projections.mesh().raw(),
        changed_voxels: narrow(receipt.fact.changed_voxels),
        changed_min: native_address(receipt.fact.changed_min),
        changed_max_inclusive: native_address(receipt.fact.changed_max_inclusive),
        dirty_chunk_count: narrow(receipt.dirty_mesh_chunks.len()),
        rebuilt_mesh_chunks: narrow(receipt.rebuilt_mesh_chunks),
        reused_mesh_chunks: narrow(receipt.reused_mesh_chunks),
        removed_mesh_chunks: narrow(receipt.removed_mesh_chunks),
        status: NativeVoxelEditStatus::Accepted,
    }
}

fn native_residency_receipt(
    receipt: &engine_spatial::VoxelChunkResidencyReceipt,
) -> NativeVoxelResidencyReceipt {
    NativeVoxelResidencyReceipt {
        revision_before: receipt.revision_before.raw(),
        accepted_revision: receipt.accepted_revision.raw(),
        admitted_count: narrow(receipt.admitted.len()),
        replaced_count: narrow(receipt.replaced.len()),
        evicted_count: narrow(receipt.evicted.len()),
        retained_count: narrow(receipt.retained.len()),
        resident_chunk_count: receipt.resident_chunk_count as u64,
        resident_solid_voxel_count: receipt.resident_solid_voxel_count as u64,
        authority_hash: receipt.authority_hash,
        collision_revision: receipt.projections.collision().raw(),
        navigation_revision: receipt.projections.navigation().raw(),
        mesh_revision: receipt.projections.mesh().raw(),
        dirty_chunk_count: narrow(receipt.dirty_chunks.len()),
        rebuilt_mesh_chunks: narrow(receipt.rebuilt_mesh_chunks),
        reused_mesh_chunks: narrow(receipt.reused_mesh_chunks),
        removed_mesh_chunks: narrow(receipt.removed_mesh_chunks),
    }
}

fn native_chunk_readout(chunk: engine_spatial::ResidentVoxelChunk) -> NativeVoxelChunkReadout {
    NativeVoxelChunkReadout {
        present: true,
        chunk: native_chunk(chunk.chunk.to_array()),
        content_hash: chunk.content_hash.raw(),
        solid_voxel_count: chunk.solid_voxel_count as u64,
    }
}

fn native_address(value: [i64; 3]) -> NativeVoxelAddress {
    NativeVoxelAddress {
        x: value[0],
        y: value[1],
        z: value[2],
    }
}

fn native_chunk(value: [i64; 3]) -> NativeVoxelChunkIdentity {
    NativeVoxelChunkIdentity {
        x: value[0],
        y: value[1],
        z: value[2],
    }
}

fn address(value: NativeVoxelAddress) -> [i64; 3] {
    [value.x, value.y, value.z]
}

fn chunk_identity(value: NativeVoxelChunkIdentity) -> VoxelChunkIdentity {
    VoxelChunkIdentity::new(value.x, value.y, value.z)
}

fn narrow(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn voxel_error(code: &'static str, detail: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, detail)
}

fn native_utf8(value: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: if value.is_empty() {
            std::ptr::null()
        } else {
            value.as_ptr()
        },
        len: value.len(),
    }
}

unsafe extern "C" fn read_scene(
    context: *mut c_void,
    request: NativeVoxelSceneReadRequest,
    output: *mut NativeVoxelSceneReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_voxel_scene(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    request: NativeVoxelReadRequest,
    output: *mut NativeVoxelReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_voxel(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_chunk(
    context: *mut c_void,
    request: NativeVoxelChunkReadRequest,
    output: *mut NativeVoxelChunkReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_voxel_chunk(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_resident_chunk_at(
    context: *mut c_void,
    request: NativeVoxelResidentChunkAtRequest,
    output: *mut NativeVoxelChunkReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_resident_chunk_at(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn configure_material_collision(
    context: *mut c_void,
    request: *const NativeVoxelMaterialCollisionRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    let request = unsafe { &*request };
    let result = (|| {
        let materials = unsafe {
            crate::composition::borrowed_slice(
                request.materials,
                request.materials_len,
                "voxel material collision declarations",
            )
        }?;
        let mut declared = std::collections::BTreeSet::new();
        let mut excluded = std::collections::BTreeSet::new();
        for material in materials {
            let slot = u16::try_from(material.material_slot).ok().ok_or_else(|| {
                voxel_error(
                    "CSHARP_VOXEL_MATERIAL_COLLISION",
                    "material slot must be in 0..=65535",
                )
            })?;
            if !declared.insert(slot) {
                return Err(voxel_error(
                    "CSHARP_VOXEL_MATERIAL_COLLISION",
                    "duplicate material slot",
                ));
            }
            if !material.collidable {
                excluded.insert(slot);
            }
        }
        bridge.edit_scene(request.session, |session| {
            Arc::make_mut(&mut session.scene).set_noncollidable_materials(excluded);
        })
    })();
    match result {
        Ok(()) => ABI_OK,
        Err(error) => {
            retain_voxel_operation_error(bridge, &error, receipt, b"ConfigureMaterialCollision");
            0
        }
    }
}

unsafe extern "C" fn apply_edits(
    context: *mut c_void,
    request: *const NativeVoxelEditTransaction,
    output: *mut NativeVoxelEditReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: this borrowed receipt starts empty for every direct callback.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.apply_voxel_edits(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            retain_voxel_operation_error(bridge, &error, receipt, APPLY_EDITS_OPERATION);
            0
        }
    }
}

unsafe extern "C" fn read_dirty_chunk_at(
    context: *mut c_void,
    request: NativeVoxelDirtyChunkAtRequest,
    output: *mut NativeVoxelDirtyChunkAtReceipt,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_dirty_chunk_at(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn apply_residency(
    context: *mut c_void,
    request: *const NativeVoxelResidencyTransaction,
    output: *mut NativeVoxelResidencyReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: this borrowed receipt starts empty for every direct callback.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    let started = Instant::now();
    let result = bridge.apply_voxel_residency(unsafe { &*request });
    let duration_us = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
    bridge.record_voxel_residency_attribution(duration_us);
    match result {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            retain_voxel_operation_error(bridge, &error, receipt, APPLY_RESIDENCY_OPERATION);
            0
        }
    }
}

fn retain_voxel_operation_error(
    bridge: &mut RuntimeSpatialBridge,
    error: &CsharpEngineServicesError,
    receipt: *mut NativeOperationErrorReceipt,
    operation: &'static [u8],
) {
    if let Some(diagnostics) = bridge.retain_voxel_operation_diagnostic(error) {
        // SAFETY: receipt was checked by the direct callback and names only
        // this independently retained Voxel diagnostic lease.
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                service: native_utf8(VOXEL_SERVICE),
                operation: native_utf8(operation),
                status: 0,
                diagnostics,
            };
        }
    }
}

unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    i32::from(bridge.destroy_voxel_operation_diagnostic_lease(handle))
}

pub(crate) fn api(bridge: &mut RuntimeSpatialBridge) -> NativeVoxelApi {
    NativeVoxelApi {
        context: (bridge as *mut RuntimeSpatialBridge).cast(),
        configure_material_collision,
        read_scene,
        read,
        sample_direct_lighting,
        read_chunk,
        read_resident_chunk_at,
        apply_edits,
        read_dirty_chunk_at,
        apply_residency,
        destroy_operation_diagnostic_lease,
    }
}

fn translate_residency(
    request: &NativeVoxelResidencyTransaction,
    chunk_size: u32,
) -> Result<Vec<VoxelChunkResidencyOperation>, CsharpEngineServicesError> {
    let operations = unsafe {
        crate::composition::borrowed_slice(
            request.operations,
            request.operations_len,
            "voxel residency operations",
        )
    }?;
    let material_slots = unsafe {
        crate::composition::borrowed_slice(
            request.material_slots,
            request.material_slots_len,
            "voxel residency material slots",
        )
    }?;
    let states = unsafe {
        crate::composition::borrowed_slice(request.states, request.states_len, "voxel states")
    }?;
    if !states.is_empty() && states.len() != material_slots.len() {
        return Err(voxel_error(
            "CSHARP_VOXEL_STATE",
            "states must be empty or match material slots",
        ));
    }
    let mut translated = Vec::with_capacity(operations.len());
    for operation in operations {
        let chunk = chunk_identity(operation.chunk);
        let translated_operation = match operation.kind {
            NativeVoxelResidencyOperationKind::Admit => {
                let payload = native_payload(*operation, chunk_size, material_slots, states)?;
                VoxelChunkResidencyOperation::Admit { chunk, payload }
            }
            NativeVoxelResidencyOperationKind::Replace => {
                let payload = native_payload(*operation, chunk_size, material_slots, states)?;
                VoxelChunkResidencyOperation::Replace { chunk, payload }
            }
            NativeVoxelResidencyOperationKind::Evict => {
                VoxelChunkResidencyOperation::Evict { chunk }
            }
        };
        translated.push(translated_operation);
    }

    Ok(translated)
}

unsafe extern "C" fn sample_direct_lighting(
    context: *mut c_void,
    request: *const NativeVoxelLightSampleRequest,
    output: *mut NativeVoxelLightSample,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() || error.is_null() {
        return 0;
    }
    unsafe { *error = std::mem::zeroed() };
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    let request = unsafe { &*request };
    let result = (|| {
        let offset = [request.offset.x, request.offset.y, request.offset.z];
        let mut normal = [request.normal.x, request.normal.y, request.normal.z].map(f64::from);
        if !offset.iter().all(|v| v.is_finite())
            || !normal.iter().all(|v| v.is_finite())
            || !request.directional_distance.is_finite()
            || request.directional_distance <= 0.0
        {
            return Err(voxel_error(
                "CSHARP_VOXEL_LIGHT_SAMPLE",
                "sample coordinates and directional horizon must be finite and horizon positive",
            ));
        }
        let length = normal.iter().map(|v| v * v).sum::<f64>().sqrt();
        if length > 0.0 {
            normal = normal.map(|v| v / length);
        }
        let input = unsafe {
            crate::composition::borrowed_slice(
                request.lights,
                request.lights_len,
                "voxel light descriptors",
            )
        }?;
        let lights = input
            .iter()
            .copied()
            .map(crate::appearance::native_light_descriptor)
            .collect::<Result<Vec<_>, _>>()?;
        let session = bridge.session_mut(request.session)?;
        let scene = &session.scene;
        let address = address(request.address);
        let origin = scene.world_origin().cell();
        let position = std::array::from_fn(|i| {
            (address[i] as f64 + f64::from(offset[i])) * scene.voxel_size() - origin[i] as f64
        });
        let sample = render_model::sample_direct_lighting(
            position,
            normal,
            f64::from(request.directional_distance),
            &lights,
            |direction, distance| scene.raycast_world(position, direction, distance).is_some(),
        );
        let [r, g, b] = sample.irradiance;
        Ok(NativeVoxelLightSample {
            irradiance: NativeVec3 { x: r, y: g, z: b },
            luminance: 0.2126 * r + 0.7152 * g + 0.0722 * b,
            contributing_lights: sample.contributing_lights,
            occluded_lights: sample.occluded_lights,
            source_revision: scene.source_revision().raw(),
            collision_revision: scene.projection_revisions().collision().raw(),
            static_collision_revision: scene.static_mesh_collision_revision(),
            rebase_revision: scene.rebase_revision(),
        })
    })();
    match result {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(e) => {
            retain_voxel_operation_error(bridge, &e, error, b"SampleDirectLighting");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_session(bridge: &mut RuntimeSpatialBridge) -> NativeSpatialSessionHandle {
        let spatial = crate::spatial::api(bridge);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial.create_session)(
                    spatial.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        session
    }

    #[test]
    fn material_collision_configuration_survives_edits_and_can_change_later() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = create_session(&mut bridge);
        let api = api(&mut bridge);
        let materials = [NativeVoxelMaterialCollision {
            material_slot: 11,
            collidable: false,
        }];
        let request = NativeVoxelMaterialCollisionRequest {
            session,
            materials: materials.as_ptr(),
            materials_len: materials.len(),
        };
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { (api.configure_material_collision)(api.context, &request, &mut receipt) },
            ABI_OK
        );
        let edits = [
            NativeVoxelEdit {
                kind: NativeVoxelEditKind::Set,
                address: NativeVoxelAddress { x: 0, y: 0, z: 0 },
                material_slot: 1,
                state: 0,
            },
            NativeVoxelEdit {
                kind: NativeVoxelEditKind::Set,
                address: NativeVoxelAddress { x: 0, y: 1, z: 0 },
                material_slot: 11,
                state: 0,
            },
        ];
        bridge
            .apply_voxel_edits(&NativeVoxelEditTransaction {
                session,
                edits: edits.as_ptr(),
                edits_len: edits.len(),
            })
            .unwrap();
        let scene = &bridge.session_mut(session).unwrap().scene;
        assert_eq!(scene.material_voxels().len(), 2);
        assert_eq!(
            scene
                .raycast([0.5, 3.0, 0.5], [0.0, -1.0, 0.0], 5.0)
                .unwrap()
                .voxel,
            [0, 0, 0]
        );
        // Collision can be reconfigured after edits: slot 11 now collides.
        let collidable = [NativeVoxelMaterialCollision {
            material_slot: 11,
            collidable: true,
        }];
        let request = NativeVoxelMaterialCollisionRequest {
            session,
            materials: collidable.as_ptr(),
            materials_len: collidable.len(),
        };
        assert_eq!(
            unsafe { (api.configure_material_collision)(api.context, &request, &mut receipt) },
            ABI_OK
        );
        let scene = &bridge.session_mut(session).unwrap().scene;
        assert!(scene.noncollidable_materials().is_empty());
        assert_eq!(
            scene
                .raycast([0.5, 3.0, 0.5], [0.0, -1.0, 0.0], 5.0)
                .unwrap()
                .voxel,
            [0, 1, 0]
        );
    }

    fn set(address: NativeVoxelAddress, material_slot: u32) -> NativeVoxelEdit {
        NativeVoxelEdit {
            state: 0,
            kind: NativeVoxelEditKind::Set,
            address,
            material_slot,
        }
    }

    fn copied_utf8(value: NativeUtf8Slice) -> String {
        let bytes = unsafe { std::slice::from_raw_parts(value.bytes, value.len) };
        std::str::from_utf8(bytes).unwrap().to_owned()
    }

    #[test]
    fn direct_light_sample_uses_live_voxel_occlusion_and_reports_revision() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = create_session(&mut bridge);
        let api = api(&mut bridge);
        let edits = [set(NativeVoxelAddress { x: 0, y: 0, z: 0 }, 1)];
        bridge
            .apply_voxel_edits(&NativeVoxelEditTransaction {
                session,
                edits: edits.as_ptr(),
                edits_len: 1,
            })
            .unwrap();
        let light = NativeLightDescriptor {
            kind: NativeLightKind::Point,
            color: NativeVec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            intensity: 10.0,
            enabled: true,
            position: NativeVec3 {
                x: 2.5,
                y: 0.5,
                z: 0.5,
            },
            direction: NativeVec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            has_range: true,
            range: 10.0,
            decay: 2.0,
            outer_angle_radians: 0.0,
            penumbra: 0.0,
            shadow_intent: NativeLightShadowIntent::Requested,
        };
        let mut request = NativeVoxelLightSampleRequest {
            session,
            address: NativeVoxelAddress { x: -1, y: 0, z: 0 },
            offset: NativeVec3 {
                x: 0.5,
                y: 0.5,
                z: 0.5,
            },
            normal: NativeVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            directional_distance: 20.0,
            lights: &light,
            lights_len: 1,
        };
        let mut result: NativeVoxelLightSample = unsafe { std::mem::zeroed() };
        let mut error: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { (api.sample_direct_lighting)(api.context, &request, &mut result, &mut error) },
            ABI_OK
        );
        assert_eq!(result.luminance, 0.0);
        assert_eq!(result.occluded_lights, 1);
        assert!(result.source_revision > 0);
        request.address.x = 1;
        assert_eq!(
            unsafe { (api.sample_direct_lighting)(api.context, &request, &mut result, &mut error) },
            ABI_OK
        );
        assert!(result.luminance > 0.0);
        assert_eq!(result.occluded_lights, 0);
        request.directional_distance = f32::NAN;
        assert_eq!(
            unsafe { (api.sample_direct_lighting)(api.context, &request, &mut result, &mut error) },
            0
        );
        assert_ne!(error.diagnostics.handle.value, 0);
        bridge.destroy_voxel_operation_diagnostic_lease(error.diagnostics.handle);
    }

    #[test]
    fn mutation_callbacks_return_typed_no_change_outcomes() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = create_session(&mut bridge);
        let api = api(&mut bridge);
        let edit = [set(NativeVoxelAddress { x: 1, y: 0, z: 0 }, 2)];
        let mut accepted = NativeVoxelEditReceipt::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (api.apply_edits)(
                    api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        edits: edit.as_ptr(),
                        edits_len: edit.len(),
                    },
                    &mut accepted,
                    &mut error,
                )
            },
            ABI_OK
        );
        assert_eq!(error.diagnostics.handle.value, 0);
        assert_eq!(accepted.status, NativeVoxelEditStatus::Accepted);

        let mut no_change = NativeVoxelEditReceipt::default();
        let mut no_change_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (api.apply_edits)(
                    api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        edits: edit.as_ptr(),
                        edits_len: edit.len(),
                    },
                    &mut no_change,
                    &mut no_change_error,
                )
            },
            ABI_OK
        );
        assert_eq!(no_change_error.diagnostics.handle.value, 0);
        assert_eq!(no_change.status, NativeVoxelEditStatus::NoChanges);
        assert_eq!(no_change.accepted_revision, accepted.accepted_revision);

        let invalid = [NativeVoxelEdit {
            state: 0,
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 2, y: 0, z: 0 },
            material_slot: u32::from(u16::MAX) + 1,
        }];
        let mut invalid_receipt = NativeVoxelEditReceipt::default();
        let mut invalid_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (api.apply_edits)(
                    api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        edits: invalid.as_ptr(),
                        edits_len: invalid.len(),
                    },
                    &mut invalid_receipt,
                    &mut invalid_error,
                )
            },
            0
        );
        assert_eq!(invalid_error.status, 0);
        assert_eq!(copied_utf8(invalid_error.service), "Voxel");
        assert_eq!(copied_utf8(invalid_error.operation), "ApplyEdits");
        assert_eq!(invalid_error.diagnostics.diagnostics_len, 1);
        let diagnostic = unsafe { *invalid_error.diagnostics.diagnostics };
        assert_eq!(copied_utf8(diagnostic.code), "CSHARP_VOXEL_EDIT");
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(
                    api.context,
                    invalid_error.diagnostics.handle,
                )
            },
            ABI_OK
        );
    }
}
