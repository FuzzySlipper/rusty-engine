//! Generated direct bridge for the canonical runtime voxel scene.
//!
//! This family deliberately aliases the Spatial session rather than owning a
//! second voxel store. Every accepted edit or residency publication swaps the
//! same scene Arc used by collision, character motion, and Dynamics bindings.

use std::{ffi::c_void, sync::Arc, time::Instant};

use csharp_engine_abi::*;
use engine_spatial::{
    decode_voxel_edit_history, encode_voxel_edit_history, VoxelChunkContentHash,
    VoxelChunkIdentity, VoxelChunkLeaseId, VoxelChunkPayload, VoxelChunkResidencyOperation,
    VoxelChunkResidencyService, VoxelChunkResidencyTransaction, VoxelEdit, VoxelEditHistoryLimits,
    VoxelEditHistoryRevertReceipt, VoxelResidencyHistoryPolicy, MAX_VOXEL_EDIT_HISTORY_BYTES,
    VOXEL_EDIT_HISTORY_SCHEMA_VERSION,
};

use crate::{
    composition::{CsharpEngineServicesError, ABI_OK},
    spatial::RuntimeSpatialBridge,
};

const LEASE_ID_MASK: u64 = u32::MAX as u64;
const VOXEL_SERVICE: &[u8] = b"Voxel";
const APPLY_EDITS_OPERATION: &[u8] = b"ApplyEdits";
const APPLY_RESIDENCY_OPERATION: &[u8] = b"ApplyResidency";
const UNDO_OPERATION: &[u8] = b"Undo";
const REDO_OPERATION: &[u8] = b"Redo";

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
            .material_voxels()
            .iter()
            .find(|voxel| voxel.address == address)
            .map(|voxel| NativeVoxelReadout {
                present: true,
                address: request.address,
                material_slot: u32::from(voxel.material_slot),
            })
            .unwrap_or(NativeVoxelReadout {
                present: false,
                address: request.address,
                ..Default::default()
            }))
    }

    fn read_voxel_at(
        &mut self,
        request: NativeVoxelAtRequest,
    ) -> Result<NativeVoxelAtReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(session
            .scene
            .material_voxels()
            .get(request.index as usize)
            .map(|voxel| NativeVoxelAtReceipt {
                present: true,
                address: native_address(voxel.address),
                material_slot: u32::from(voxel.material_slot),
            })
            .unwrap_or_default())
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
        let (scene, receipt) = {
            let session = self.session_mut(request.session)?;
            if session.scene.source_revision().raw() != request.expected_revision {
                return Err(voxel_error(
                    "CSHARP_VOXEL_EDIT",
                    format!(
                        "stale voxel revision: expected {}, actual {}",
                        request.expected_revision,
                        session.scene.source_revision().raw()
                    ),
                ));
            }
            let mut scene = (*session.scene).clone();
            let receipt = session
                .voxel_history
                .apply(&mut scene, &edits)
                .map_err(|error| voxel_error("CSHARP_VOXEL_EDIT", error.to_string()))?;
            let edit = native_edit_receipt(&receipt.edit);
            session.last_voxel_dirty_chunks = receipt.edit.dirty_mesh_chunks.clone();
            let scene = Arc::new(scene);
            session.scene = Arc::clone(&scene);
            (scene, edit)
        };
        self.publish_scene(request.session, scene);
        Ok(receipt)
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
        let session = self.session_mut(request.session)?;
        let chunk_size = session.scene.chunk_size();
        let mut translated = Vec::with_capacity(operations.len());
        for operation in operations {
            let chunk = chunk_identity(operation.chunk);
            let translated_operation = match operation.kind {
                NativeVoxelResidencyOperationKind::Admit => {
                    let payload = native_payload(*operation, chunk_size, material_slots)?;
                    VoxelChunkResidencyOperation::Admit { chunk, payload }
                }
                NativeVoxelResidencyOperationKind::Replace => {
                    let payload = native_payload(*operation, chunk_size, material_slots)?;
                    VoxelChunkResidencyOperation::Replace {
                        chunk,
                        expected_content_hash: VoxelChunkContentHash::new(
                            operation.expected_content_hash,
                        ),
                        payload,
                    }
                }
                NativeVoxelResidencyOperationKind::Evict => VoxelChunkResidencyOperation::Evict {
                    chunk,
                    expected_content_hash: VoxelChunkContentHash::new(
                        operation.expected_content_hash,
                    ),
                },
            };
            translated.push(translated_operation);
        }
        let policy = match request.history_policy {
            NativeVoxelResidencyHistoryPolicy::RejectIfNonEmpty => {
                VoxelResidencyHistoryPolicy::RejectIfNonEmpty
            }
            NativeVoxelResidencyHistoryPolicy::ResetToPublishedAuthority => {
                VoxelResidencyHistoryPolicy::ResetToPublishedAuthority
            }
        };
        let mut scene = (*session.scene).clone();
        let receipt = VoxelChunkResidencyService::apply_with_history(
            &mut scene,
            &session.voxel_leases,
            &mut session.voxel_history,
            policy,
            VoxelChunkResidencyTransaction {
                expected_scene_source_revision: engine_spatial::VoxelSourceRevision::new(
                    request.expected_revision,
                ),
                operations: &translated,
            },
        )
        .map_err(|error| voxel_error("CSHARP_VOXEL_RESIDENCY", error.to_string()))?;
        let native = native_residency_receipt(&receipt);
        session.last_voxel_dirty_chunks = receipt
            .dirty_chunks
            .iter()
            .map(|chunk| chunk.to_array())
            .collect();
        let scene = Arc::new(scene);
        session.scene = Arc::clone(&scene);
        self.publish_scene(request.session, scene);
        Ok(native)
    }

    fn acquire_chunk_lease(
        &mut self,
        request: NativeVoxelChunkLeaseRequest,
    ) -> Result<NativeVoxelChunkLeaseHandle, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let evidence = session
            .voxel_leases
            .acquire(&session.scene, chunk_identity(request.chunk))
            .map_err(|error| voxel_error("CSHARP_VOXEL_LEASE", error.to_string()))?;
        Ok(NativeVoxelChunkLeaseHandle {
            value: encode_lease(request.session, evidence.lease_id),
        })
    }

    fn destroy_chunk_lease(
        &mut self,
        handle: NativeVoxelChunkLeaseHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let (session_handle, lease_id) = decode_lease(handle)?;
        let session = self.session_mut(session_handle)?;
        session
            .voxel_leases
            .release(lease_id)
            .map_err(|error| voxel_error("CSHARP_VOXEL_LEASE", error.to_string()))?;
        Ok(())
    }

    fn read_chunk_lease(
        &mut self,
        request: NativeVoxelChunkLeaseReadRequest,
    ) -> Result<NativeVoxelChunkLeaseReadout, CsharpEngineServicesError> {
        let (session_handle, lease_id) = decode_lease(request.lease)?;
        let session = self.session_mut(session_handle)?;
        let evidence = session.voxel_leases.evidence_for_lease(lease_id);
        Ok(evidence
            .map(|evidence| NativeVoxelChunkLeaseReadout {
                present: true,
                chunk: native_chunk(evidence.chunk.to_array()),
                acquired_content_hash: evidence.acquired_content_hash.raw(),
            })
            .unwrap_or_default())
    }

    fn read_history_cursor(
        &mut self,
        request: NativeVoxelHistoryCursorReadRequest,
    ) -> Result<NativeVoxelHistoryCursorReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(native_history_cursor(
            &session.voxel_history.cursor(),
            session.voxel_history.entries().len(),
        ))
    }

    fn read_history_entry_at(
        &mut self,
        request: NativeVoxelHistoryEntryAtRequest,
    ) -> Result<NativeVoxelHistoryEntryReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(session
            .voxel_history
            .entries()
            .get(request.index as usize)
            .map(native_history_entry)
            .unwrap_or_default())
    }

    fn read_history_delta_at(
        &mut self,
        request: NativeVoxelHistoryDeltaAtRequest,
    ) -> Result<NativeVoxelHistoryDeltaReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(session
            .voxel_history
            .entries()
            .get(request.entry_index as usize)
            .and_then(|entry| entry.deltas.get(request.delta_index as usize))
            .map(|delta| NativeVoxelHistoryDeltaReadout {
                present: true,
                address: native_address(delta.address),
                before_material_present: delta.before_material.is_some(),
                before_material: u32::from(delta.before_material.unwrap_or_default()),
                after_material_present: delta.after_material.is_some(),
                after_material: u32::from(delta.after_material.unwrap_or_default()),
            })
            .unwrap_or_default())
    }

    fn undo_voxel(
        &mut self,
        request: NativeVoxelHistoryActionRequest,
    ) -> Result<NativeVoxelHistoryReceipt, CsharpEngineServicesError> {
        self.apply_history_action(request.session, true)
    }

    fn redo_voxel(
        &mut self,
        request: NativeVoxelHistoryActionRequest,
    ) -> Result<NativeVoxelHistoryReceipt, CsharpEngineServicesError> {
        self.apply_history_action(request.session, false)
    }

    fn apply_history_action(
        &mut self,
        handle: NativeSpatialSessionHandle,
        undo: bool,
    ) -> Result<NativeVoxelHistoryReceipt, CsharpEngineServicesError> {
        let (scene, receipt) = {
            let session = self.session_mut(handle)?;
            let mut scene = (*session.scene).clone();
            let receipt = if undo {
                session.voxel_history.undo_one(&mut scene)
            } else {
                session.voxel_history.redo_one(&mut scene)
            }
            .map_err(|error| voxel_error("CSHARP_VOXEL_HISTORY", error.to_string()))?;
            let native = native_history_receipt(&receipt);
            session.last_voxel_dirty_chunks = scene.mesh_update().dirty_chunks.clone();
            let scene = Arc::new(scene);
            session.scene = Arc::clone(&scene);
            (scene, native)
        };
        self.publish_scene(handle, scene);
        Ok(receipt)
    }

    fn read_history_codec_info(&mut self) -> NativeVoxelHistoryCodecInfo {
        NativeVoxelHistoryCodecInfo {
            schema_version: VOXEL_EDIT_HISTORY_SCHEMA_VERSION,
            max_encoded_bytes: MAX_VOXEL_EDIT_HISTORY_BYTES as u64,
        }
    }

    fn export_history(
        &mut self,
        request: NativeVoxelHistoryExportRequest,
    ) -> Result<NativeByteLease, CsharpEngineServicesError> {
        let encoded = {
            let session = self.session_mut(request.session)?;
            let encoded = encode_voxel_edit_history(&session.voxel_history)
                .map_err(|error| voxel_error("CSHARP_VOXEL_HISTORY_EXPORT", error.to_string()))?;
            Arc::<[u8]>::from(encoded.into_bytes())
        };
        if encoded.len() > MAX_VOXEL_EDIT_HISTORY_BYTES {
            return Err(voxel_error(
                "CSHARP_VOXEL_HISTORY_EXPORT",
                "encoded history exceeded the owner byte limit",
            ));
        }
        let handle = self.next_voxel_history_export;
        self.next_voxel_history_export = self
            .next_voxel_history_export
            .checked_add(1)
            .ok_or_else(|| voxel_error("CSHARP_VOXEL_HISTORY_EXPORT", "export leases exhausted"))?;
        let replaced = self.voxel_history_exports.insert(handle, encoded);
        debug_assert!(replaced.is_none());
        let bytes: &[u8] = self
            .voxel_history_exports
            .get(&handle)
            .expect("inserted history export");
        Ok(NativeByteLease {
            handle: NativeByteLeaseHandle { value: handle },
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        })
    }

    fn destroy_history_export(
        &mut self,
        handle: NativeByteLeaseHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        if handle.value == 0 || self.voxel_history_exports.remove(&handle.value).is_none() {
            return Err(voxel_error(
                "CSHARP_VOXEL_HISTORY_EXPORT",
                "unknown or released history export lease",
            ));
        }
        Ok(())
    }

    fn restore_history(
        &mut self,
        request: &NativeVoxelHistoryRestoreRequest,
    ) -> Result<NativeVoxelHistoryRestoreReceipt, CsharpEngineServicesError> {
        let payload = unsafe {
            crate::composition::borrowed_slice(
                request.payload.bytes,
                request.payload.len,
                "voxel history payload",
            )
        }?;
        if payload.len() > MAX_VOXEL_EDIT_HISTORY_BYTES {
            return Err(voxel_error(
                "CSHARP_VOXEL_HISTORY_RESTORE",
                "history payload exceeded the owner byte limit",
            ));
        }
        let payload = std::str::from_utf8(payload).map_err(|_| {
            voxel_error(
                "CSHARP_VOXEL_HISTORY_RESTORE",
                "history payload was not UTF-8",
            )
        })?;
        let live_scene = Arc::clone(&self.session_mut(request.session)?.scene);
        // Decode and rebuild before taking the mutable session so every codec
        // failure leaves the live scene, history cursor, and published spatial
        // projection intact. The owner-side rebuild retains static collision
        // and rebase context from the live spatial scene.
        let restored = decode_voxel_edit_history(payload, VoxelEditHistoryLimits::default())
            .map_err(|error| voxel_error("CSHARP_VOXEL_HISTORY_RESTORE", error.to_string()))?;
        let scene = Arc::new(
            restored
                .scene_preserving_runtime_context(&live_scene)
                .map_err(|error| voxel_error("CSHARP_VOXEL_HISTORY_RESTORE", error.to_string()))?,
        );
        let receipt = {
            let session = self.session_mut(request.session)?;
            session.voxel_history = restored.history;
            session.last_voxel_dirty_chunks = scene.mesh_update().dirty_chunks.clone();
            session.scene = Arc::clone(&scene);
            NativeVoxelHistoryRestoreReceipt {
                cursor: native_history_cursor(
                    &session.voxel_history.cursor(),
                    session.voxel_history.entries().len(),
                ),
                source_revision: scene.source_revision().raw(),
            }
        };
        self.publish_scene(request.session, scene);
        Ok(receipt)
    }
}

fn native_edit(value: NativeVoxelEdit) -> Result<VoxelEdit, CsharpEngineServicesError> {
    let native_address_value = value.address;
    match value.kind {
        NativeVoxelEditKind::Set => Ok(VoxelEdit::Set {
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
    Ok(VoxelChunkPayload::new([chunk_size; 3], values))
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
    }
}

fn native_residency_receipt(
    receipt: &engine_spatial::VoxelChunkResidencyReceipt,
) -> NativeVoxelResidencyReceipt {
    let history_reset = receipt.history_reset;
    NativeVoxelResidencyReceipt {
        revision_before: receipt.revision_before.raw(),
        accepted_revision: receipt.accepted_revision.raw(),
        admitted_count: narrow(receipt.admitted.len()),
        replaced_count: narrow(receipt.replaced.len()),
        evicted_count: narrow(receipt.evicted.len()),
        retained_count: narrow(receipt.retained.len()),
        resident_chunk_count: receipt.resident_chunk_count as u64,
        resident_solid_voxel_count: receipt.resident_solid_voxel_count as u64,
        residency_hash: receipt.residency_hash,
        authority_hash: receipt.authority_hash,
        collision_revision: receipt.projections.collision().raw(),
        navigation_revision: receipt.projections.navigation().raw(),
        mesh_revision: receipt.projections.mesh().raw(),
        dirty_chunk_count: narrow(receipt.dirty_chunks.len()),
        rebuilt_mesh_chunks: narrow(receipt.rebuilt_mesh_chunks),
        reused_mesh_chunks: narrow(receipt.reused_mesh_chunks),
        removed_mesh_chunks: narrow(receipt.removed_mesh_chunks),
        history_reset: history_reset.is_some(),
        history_invalidated_entries: history_reset
            .map_or(0, |reset| reset.invalidated_entries as u64),
        history_invalidated_redo_entries: history_reset
            .map_or(0, |reset| reset.invalidated_redo_entries as u64),
    }
}

fn native_history_cursor(
    cursor: &engine_spatial::VoxelEditHistoryCursor,
    entry_count: usize,
) -> NativeVoxelHistoryCursorReadout {
    NativeVoxelHistoryCursorReadout {
        present: true,
        index: cursor.index as u64,
        entry_count: entry_count as u64,
        applied_transaction_present: cursor.applied_transaction_id.is_some(),
        applied_transaction_id: cursor.applied_transaction_id.unwrap_or_default(),
        undo_depth: cursor.undo_depth as u64,
        redo_depth: cursor.redo_depth as u64,
        authority_hash: cursor.authority_hash,
        history_hash: cursor.history_hash,
    }
}

fn native_history_entry(
    entry: &engine_spatial::VoxelEditHistoryEntry,
) -> NativeVoxelHistoryEntryReadout {
    NativeVoxelHistoryEntryReadout {
        present: true,
        transaction_id: entry.transaction_id,
        parent_transaction_present: entry.parent_transaction_id.is_some(),
        parent_transaction_id: entry.parent_transaction_id.unwrap_or_default(),
        before_hash: entry.before_hash,
        after_hash: entry.after_hash,
        delta_count: narrow(entry.deltas.len()),
    }
}

fn native_history_receipt(receipt: &VoxelEditHistoryRevertReceipt) -> NativeVoxelHistoryReceipt {
    let bounds = receipt.diff.bounds;
    NativeVoxelHistoryReceipt {
        applied: receipt.applied,
        cursor_before: receipt.cursor_before.index as u64,
        cursor_after: receipt.cursor_after.index as u64,
        undo_depth: receipt.cursor_after.undo_depth as u64,
        redo_depth: receipt.cursor_after.redo_depth as u64,
        authority_hash: receipt.cursor_after.authority_hash,
        history_hash: receipt.cursor_after.history_hash,
        revision_before: receipt.revision_before.raw(),
        revision_after: receipt.revision_after.raw(),
        changed_voxels: narrow(receipt.diff.changed_voxels),
        bounds_present: bounds.is_some(),
        changed_min: native_address(bounds.map_or([0; 3], |bounds| bounds.min)),
        changed_max_inclusive: native_address(bounds.map_or([0; 3], |bounds| bounds.max)),
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

fn encode_lease(session: NativeSpatialSessionHandle, lease: VoxelChunkLeaseId) -> u64 {
    ((session.value & LEASE_ID_MASK) << 32) | (lease.raw() & LEASE_ID_MASK)
}

fn decode_lease(
    handle: NativeVoxelChunkLeaseHandle,
) -> Result<(NativeSpatialSessionHandle, VoxelChunkLeaseId), CsharpEngineServicesError> {
    let session = handle.value >> 32;
    let lease = handle.value & LEASE_ID_MASK;
    if session == 0 || lease == 0 {
        return Err(voxel_error(
            "CSHARP_VOXEL_LEASE",
            "invalid voxel chunk lease",
        ));
    }
    Ok((
        NativeSpatialSessionHandle { value: session },
        VoxelChunkLeaseId::from_raw(lease),
    ))
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

unsafe extern "C" fn read_at(
    context: *mut c_void,
    request: NativeVoxelAtRequest,
    output: *mut NativeVoxelAtReceipt,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_voxel_at(request) {
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

unsafe extern "C" fn acquire_chunk_lease(
    context: *mut c_void,
    request: NativeVoxelChunkLeaseRequest,
    output: *mut NativeVoxelChunkLeaseHandle,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.acquire_chunk_lease(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_chunk_lease(
    context: *mut c_void,
    handle: NativeVoxelChunkLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.destroy_chunk_lease(handle) {
        Ok(()) => ABI_OK,
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_chunk_lease(
    context: *mut c_void,
    request: NativeVoxelChunkLeaseReadRequest,
    output: *mut NativeVoxelChunkLeaseReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_chunk_lease(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_history_cursor(
    context: *mut c_void,
    request: NativeVoxelHistoryCursorReadRequest,
    output: *mut NativeVoxelHistoryCursorReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_history_cursor(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_history_entry_at(
    context: *mut c_void,
    request: NativeVoxelHistoryEntryAtRequest,
    output: *mut NativeVoxelHistoryEntryReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_history_entry_at(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_history_delta_at(
    context: *mut c_void,
    request: NativeVoxelHistoryDeltaAtRequest,
    output: *mut NativeVoxelHistoryDeltaReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_history_delta_at(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn undo(
    context: *mut c_void,
    request: NativeVoxelHistoryActionRequest,
    output: *mut NativeVoxelHistoryReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: this borrowed receipt starts empty for every direct callback.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.undo_voxel(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            retain_voxel_operation_error(bridge, &error, receipt, UNDO_OPERATION);
            0
        }
    }
}

unsafe extern "C" fn redo(
    context: *mut c_void,
    request: NativeVoxelHistoryActionRequest,
    output: *mut NativeVoxelHistoryReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: this borrowed receipt starts empty for every direct callback.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.redo_voxel(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            retain_voxel_operation_error(bridge, &error, receipt, REDO_OPERATION);
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

unsafe extern "C" fn read_history_codec_info(
    context: *mut c_void,
    output: *mut NativeVoxelHistoryCodecInfo,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    unsafe { *output = bridge.read_history_codec_info() };
    ABI_OK
}

unsafe extern "C" fn export_history(
    context: *mut c_void,
    request: NativeVoxelHistoryExportRequest,
    output: *mut NativeByteLease,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.export_history(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_history_export_lease(
    context: *mut c_void,
    handle: NativeByteLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.destroy_history_export(handle) {
        Ok(()) => ABI_OK,
        Err(_) => 0,
    }
}

unsafe extern "C" fn restore_history(
    context: *mut c_void,
    request: *const NativeVoxelHistoryRestoreRequest,
    output: *mut NativeVoxelHistoryRestoreReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .restore_history(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

pub(crate) fn api(bridge: &mut RuntimeSpatialBridge) -> NativeVoxelApi {
    NativeVoxelApi {
        context: (bridge as *mut RuntimeSpatialBridge).cast(),
        read_scene,
        read,
        read_at,
        read_chunk,
        read_resident_chunk_at,
        apply_edits,
        read_dirty_chunk_at,
        apply_residency,
        acquire_chunk_lease,
        destroy_chunk_lease,
        read_chunk_lease,
        read_history_cursor,
        read_history_entry_at,
        read_history_delta_at,
        undo,
        redo,
        destroy_operation_diagnostic_lease,
        read_history_codec_info,
        export_history,
        destroy_history_export_lease,
        restore_history,
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

    fn set(address: NativeVoxelAddress, material_slot: u32) -> NativeVoxelEdit {
        NativeVoxelEdit {
            kind: NativeVoxelEditKind::Set,
            address,
            material_slot,
        }
    }

    fn copied_utf8(value: NativeUtf8Slice) -> String {
        let bytes = unsafe { std::slice::from_raw_parts(value.bytes, value.len) };
        std::str::from_utf8(bytes).unwrap().to_owned()
    }

    fn assert_edit_rejection(
        api: &NativeVoxelApi,
        receipt: &mut NativeOperationErrorReceipt,
        expected_message: &str,
    ) {
        assert_eq!(receipt.status, 0);
        assert_eq!(copied_utf8(receipt.service), "Voxel");
        assert_eq!(copied_utf8(receipt.operation), "ApplyEdits");
        assert_eq!(receipt.diagnostics.diagnostics_len, 1);
        let diagnostic = unsafe { *receipt.diagnostics.diagnostics };
        assert_eq!(copied_utf8(diagnostic.code), "CSHARP_VOXEL_EDIT");
        assert!(copied_utf8(diagnostic.message).contains(expected_message));
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            0
        );
    }

    #[test]
    fn mutation_callbacks_retain_copyable_diagnostics_for_no_change_and_stale_edits() {
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
                        expected_revision: 0,
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

        let mut no_change = NativeVoxelEditReceipt::default();
        let mut no_change_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (api.apply_edits)(
                    api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        expected_revision: accepted.accepted_revision,
                        edits: edit.as_ptr(),
                        edits_len: edit.len(),
                    },
                    &mut no_change,
                    &mut no_change_error,
                )
            },
            0
        );
        assert_edit_rejection(&api, &mut no_change_error, "NoChanges");

        let mut stale = NativeVoxelEditReceipt::default();
        let mut stale_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (api.apply_edits)(
                    api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        expected_revision: 0,
                        edits: edit.as_ptr(),
                        edits_len: edit.len(),
                    },
                    &mut stale,
                    &mut stale_error,
                )
            },
            0
        );
        assert_edit_rejection(&api, &mut stale_error, "stale voxel revision");
    }

    #[test]
    fn history_export_restore_releases_exact_bytes_and_retains_cursor_and_redo() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = create_session(&mut bridge);
        let first = [set(NativeVoxelAddress { x: 1, y: 0, z: 0 }, 2)];
        let second = [set(NativeVoxelAddress { x: 2, y: 0, z: 0 }, 3)];
        for (expected_revision, edits) in [(0, first.as_slice()), (1, second.as_slice())] {
            bridge
                .apply_voxel_edits(&NativeVoxelEditTransaction {
                    session,
                    expected_revision,
                    edits: edits.as_ptr(),
                    edits_len: edits.len(),
                })
                .unwrap();
        }
        bridge
            .undo_voxel(NativeVoxelHistoryActionRequest { session })
            .unwrap();

        let codec = bridge.read_history_codec_info();
        let cursor = bridge
            .read_history_cursor(NativeVoxelHistoryCursorReadRequest { session })
            .unwrap();
        let export = bridge
            .export_history(NativeVoxelHistoryExportRequest { session })
            .unwrap();
        assert_eq!(codec.schema_version, VOXEL_EDIT_HISTORY_SCHEMA_VERSION);
        assert_eq!(codec.max_encoded_bytes, MAX_VOXEL_EDIT_HISTORY_BYTES as u64);
        assert!(export.len <= MAX_VOXEL_EDIT_HISTORY_BYTES);
        assert_eq!(cursor.index, 1);
        assert_eq!(cursor.redo_depth, 1);
        let bytes = unsafe { std::slice::from_raw_parts(export.bytes, export.len) }.to_vec();
        bridge.destroy_history_export(export.handle).unwrap();
        assert!(bridge.destroy_history_export(export.handle).is_err());

        // This is the same direct composition used by the managed helper:
        // product-selected scope/key and persistence revision carry opaque
        // codec bytes, while Voxel remains the sole history owner.
        let persistence_root = tempfile::tempdir().unwrap();
        let mut persistence = crate::persistence::RuntimePersistenceBridge::new(Some(
            persistence_root.path().to_path_buf(),
        ));
        let persistence_api = crate::persistence::api(&mut persistence);
        let scope = b"runtime";
        let key = b"voxel.history";
        let mut store = NativePersistenceStoreHandle::default();
        assert_eq!(
            unsafe {
                (persistence_api.open_store)(
                    persistence_api.context,
                    &NativePersistenceOpenRequest {
                        scope: NativeUtf8Slice {
                            bytes: scope.as_ptr(),
                            len: scope.len(),
                        },
                    },
                    &mut store,
                )
            },
            ABI_OK
        );
        let mut save = NativePersistenceSaveReceipt::default();
        assert_eq!(
            unsafe {
                (persistence_api.save)(
                    persistence_api.context,
                    &NativePersistenceSaveRequest {
                        store,
                        key: NativeUtf8Slice {
                            bytes: key.as_ptr(),
                            len: key.len(),
                        },
                        schema_version: codec.schema_version,
                        revision_guard: NativePersistenceRevisionGuard::Absent,
                        expected_revision: 0,
                        payload: NativeByteSlice {
                            bytes: bytes.as_ptr(),
                            len: bytes.len(),
                        },
                    },
                    &mut save,
                )
            },
            ABI_OK
        );
        assert_eq!(save.revision, 1);

        let mutation = [set(NativeVoxelAddress { x: 9, y: 0, z: 0 }, 4)];
        bridge
            .apply_voxel_edits(&NativeVoxelEditTransaction {
                session,
                expected_revision: 3,
                edits: mutation.as_ptr(),
                edits_len: mutation.len(),
            })
            .unwrap();

        let mut blob = NativePersistenceBlobHandle::default();
        assert_eq!(
            unsafe {
                (persistence_api.load)(
                    persistence_api.context,
                    &NativePersistenceLoadRequest {
                        store,
                        key: NativeUtf8Slice {
                            bytes: key.as_ptr(),
                            len: key.len(),
                        },
                    },
                    &mut blob,
                )
            },
            ABI_OK
        );
        let mut blob_info = NativePersistenceBlobInfo {
            present: false,
            schema_version: 0,
            revision: 0,
            payload_len: 0,
        };
        assert_eq!(
            unsafe {
                (persistence_api.describe_blob)(persistence_api.context, blob, &mut blob_info)
            },
            ABI_OK
        );
        assert_eq!(
            (
                blob_info.present,
                blob_info.schema_version,
                blob_info.revision
            ),
            (true, codec.schema_version, 1)
        );
        let mut restored_bytes = vec![0; blob_info.payload_len];
        assert_eq!(
            unsafe {
                (persistence_api.copy_blob)(
                    persistence_api.context,
                    &NativePersistenceCopyBlobRequest {
                        blob,
                        destination: NativeWritableByteSlice {
                            bytes: restored_bytes.as_mut_ptr(),
                            len: restored_bytes.len(),
                        },
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { (persistence_api.destroy_blob)(persistence_api.context, blob) },
            ABI_OK
        );

        let restore = bridge
            .restore_history(&NativeVoxelHistoryRestoreRequest {
                session,
                payload: NativeByteSlice {
                    bytes: restored_bytes.as_ptr(),
                    len: restored_bytes.len(),
                },
            })
            .unwrap();
        assert_eq!(restore.cursor.index, 1);
        assert_eq!(restore.cursor.undo_depth, 1);
        assert_eq!(restore.cursor.redo_depth, 1);
        assert_eq!(restore.source_revision, 3);

        let restored_scene = bridge
            .read_voxel(NativeVoxelReadRequest {
                session,
                address: NativeVoxelAddress { x: 9, y: 0, z: 0 },
            })
            .unwrap();
        assert!(!restored_scene.present);
        bridge
            .redo_voxel(NativeVoxelHistoryActionRequest { session })
            .unwrap();
        assert!(
            bridge
                .read_voxel(NativeVoxelReadRequest {
                    session,
                    address: NativeVoxelAddress { x: 2, y: 0, z: 0 },
                })
                .unwrap()
                .present
        );

        let before_failure = bridge
            .read_history_cursor(NativeVoxelHistoryCursorReadRequest { session })
            .unwrap();
        assert!(bridge
            .restore_history(&NativeVoxelHistoryRestoreRequest {
                session,
                payload: NativeByteSlice {
                    bytes: b"not voxel history".as_ptr(),
                    len: b"not voxel history".len(),
                },
            })
            .is_err());
        assert_eq!(
            bridge
                .read_history_cursor(NativeVoxelHistoryCursorReadRequest { session })
                .unwrap(),
            before_failure
        );
    }
}
