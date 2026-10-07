//! Generated direct bridge for the canonical runtime voxel scene.
//!
//! This family works on the Spatial session's scene rather than owning a
//! second voxel store. Edits and residency changes mutate that scene in place,
//! so collision, character motion and Dynamics see them directly.

use std::{ffi::c_void, sync::Arc, time::Instant};

use crate::operation_diagnostics::{clear_receipt, refuse};
use csharp_engine_abi::*;
use engine_spatial::{
    MaterialSurface, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions, TerrainLayers,
    VertexPlacement, VoxelChunkIdentity, VoxelChunkPayload, VoxelChunkResidencyApplyError,
    VoxelChunkResidencyOperation, VoxelChunkResidencyRejection, VoxelChunkResidencyService,
    VoxelDensityApplyError, VoxelDensityEdit, VoxelDensityEditService, VoxelDensityOperation,
    VoxelDensityRejection, VoxelDensityShape, VoxelEdit, VoxelEditApplyError, VoxelEditRejection,
};

use crate::{
    composition::{CsharpEngineServicesError, ABI_OK},
    spatial::RuntimeSpatialBridge,
};

impl RuntimeSpatialBridge {
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
            mesh_revision: scene.projection_revisions().mesh().raw(),
            projection_version: scene.projection_version(),
            resident_chunk_count: scene.resident_chunk_count() as u64,
            collider_chunk_count: scene.collider_chunk_count() as u64,
            solid_voxel_count: scene.solid_voxel_count() as u64,
            dirty_chunk_count: narrow(update.dirty_chunks.len()),
            rebuilt_mesh_chunks: narrow(update.rebuilt_chunks),
            reused_mesh_chunks: narrow(update.reused_chunks),
            removed_mesh_chunks: narrow(update.removed_chunks),
            mesh_microseconds: update.mesh_microseconds,
        })
    }

    fn configure_voxel_material_surfaces(
        &mut self,
        request: &NativeVoxelMaterialSurfaceRequest,
    ) -> Result<NativeVoxelSceneReadout, CsharpEngineServicesError> {
        let materials = unsafe {
            crate::composition::borrowed_slice(
                request.materials,
                request.materials_len,
                "voxel material surfaces",
            )
        }?;
        let invalid = |message: &str| voxel_error("CSHARP_VOXEL_MATERIAL_SURFACE", message);
        let entries = materials
            .iter()
            .map(|material| {
                let slot = u16::try_from(material.material_slot)
                    .map_err(|_| invalid("material slot must be in 0..=65535"))?;
                Ok((
                    slot,
                    MaterialSurface {
                        mode: crate::spatial::surface_mode(material.mode),
                        character: surface_character(material.character),
                    },
                ))
            })
            .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?;
        let materials =
            SurfaceMaterials::new(entries).map_err(|error| invalid(&error.to_string()))?;
        let session = self.session_mut(request.session)?;
        let options = SurfaceMeshOptions {
            mode: crate::spatial::surface_mode(request.mode),
            materials,
            ..session.scene.mesh_options().clone()
        };
        // The surface hash in the collision navigation key changes, so the
        // next collision navigation publication derives everything.
        self.edit_scene(request.session, |session| {
            Arc::make_mut(&mut session.scene).set_mesh_options(options)
        })?
        .map_err(|error| invalid(&error.to_string()))?;
        self.read_voxel_scene(NativeVoxelSceneReadRequest {
            session: request.session,
        })
    }

    fn configure_voxel_terrain_layers(
        &mut self,
        request: &NativeVoxelTerrainLayerRequest,
    ) -> Result<NativeVoxelSceneReadout, CsharpEngineServicesError> {
        let slots = unsafe {
            crate::composition::borrowed_slice(
                request.slots,
                request.slots_len,
                "voxel terrain layer slots",
            )
        }?;
        let layers = unsafe {
            crate::composition::borrowed_slice(
                request.layers,
                request.layers_len,
                "voxel terrain layer indices",
            )
        }?;
        let invalid = |message: &str| voxel_error("CSHARP_VOXEL_TERRAIN_LAYERS", message);
        let terrain_layers = if slots.is_empty() && layers.is_empty() {
            None
        } else {
            let slots = slots
                .iter()
                .map(|slot| {
                    u16::try_from(*slot).map_err(|_| invalid("material slot must be in 0..=65535"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let cells = u8::try_from(request.transition_cells)
                .map_err(|_| invalid("transition must be 1 to 4 voxels"))?;
            let terrain_layers = if layers.is_empty() {
                TerrainLayers::new(slots, cells)
            } else {
                let layers = layers
                    .iter()
                    .map(|layer| u8::try_from(*layer).map_err(|_| invalid("layer must be 0 to 3")))
                    .collect::<Result<Vec<_>, _>>()?;
                TerrainLayers::mapped(slots, layers, cells)
            };
            Some(terrain_layers.map_err(|error| invalid(&error.to_string()))?)
        };
        let session = self.session_mut(request.session)?;
        let chunk_size = session.scene.chunk_size();
        if terrain_layers
            .as_ref()
            .is_some_and(|layers| u32::from(layers.transition_cells()) + 1 >= chunk_size)
        {
            return Err(invalid(
                "a transition must be shorter than the chunk size less one voxel",
            ));
        }
        let options = SurfaceMeshOptions {
            terrain_layers,
            ..session.scene.mesh_options().clone()
        };
        self.edit_scene(request.session, |session| {
            Arc::make_mut(&mut session.scene).set_mesh_options(options)
        })?
        .map_err(|error| invalid(&error.to_string()))?;
        self.read_voxel_scene(NativeVoxelSceneReadRequest {
            session: request.session,
        })
    }

    fn configure_voxel_vertex_occlusion(
        &mut self,
        request: &NativeVoxelVertexOcclusionRequest,
    ) -> Result<NativeVoxelSceneReadout, CsharpEngineServicesError> {
        if !request.strength.is_finite() || !(0.0..=1.0).contains(&request.strength) {
            return Err(voxel_error(
                "CSHARP_VOXEL_VERTEX_OCCLUSION",
                "vertex occlusion strength must be within 0 to 1",
            ));
        }
        let session = self.session_mut(request.session)?;
        let options = SurfaceMeshOptions {
            vertex_occlusion: request.strength,
            ..session.scene.mesh_options().clone()
        };
        self.edit_scene(request.session, |session| {
            Arc::make_mut(&mut session.scene).set_mesh_options(options)
        })?
        .map_err(|error| voxel_error("CSHARP_VOXEL_VERTEX_OCCLUSION", error.to_string()))?;
        self.read_voxel_scene(NativeVoxelSceneReadRequest {
            session: request.session,
        })
    }

    fn apply_voxel_density_edits(
        &mut self,
        request: &NativeVoxelDensityTransaction,
    ) -> Result<NativeVoxelDensityReceipt, CsharpEngineServicesError> {
        let edits = unsafe {
            crate::composition::borrowed_slice(
                request.edits,
                request.edits_len,
                "voxel density edits",
            )
        }?;
        let densities = unsafe {
            crate::composition::borrowed_slice(
                request.densities,
                request.densities_len,
                "voxel densities",
            )
        }?;
        let materials = unsafe {
            crate::composition::borrowed_slice(
                request.materials,
                request.materials_len,
                "voxel density materials",
            )
        }?;
        let edits = edits
            .iter()
            .map(|edit| native_density_edit(edit, densities, materials))
            .collect::<Result<Vec<_>, _>>()?;
        self.edit_scene(request.session, |session| {
            session.change_collision(
                |scene| match VoxelDensityEditService::apply(scene, &edits) {
                    Ok(receipt) => {
                        let changed = collision_reach(
                            scene,
                            voxel_box(scene, receipt.changed_min, receipt.changed_max_inclusive),
                        );
                        (Ok(native_density_receipt(scene, &receipt)), vec![changed])
                    }
                    Err(VoxelDensityApplyError::Rejected(VoxelDensityRejection::NoChanges)) => {
                        let revision = scene.source_revision().raw();
                        (
                            Ok(NativeVoxelDensityReceipt {
                                status: NativeVoxelEditStatus::NoChanges,
                                revision_before: revision,
                                accepted_revision: revision,
                                solid_voxel_count: scene.solid_voxel_count() as u64,
                                authority_hash: scene.authority_hash(),
                                ..Default::default()
                            }),
                            Vec::new(),
                        )
                    }
                    Err(error) => (
                        Err(voxel_error("CSHARP_VOXEL_DENSITY_EDIT", error.to_string())),
                        Vec::new(),
                    ),
                },
            )
        })?
    }

    fn read_voxel_densities(
        &mut self,
        request: NativeVoxelDensityReadRequest,
    ) -> Result<NativeVoxelDensityResult, CsharpEngineServicesError> {
        let count = [request.size_x, request.size_y, request.size_z]
            .iter()
            .map(|value| u64::from(*value))
            .product::<u64>();
        if count == 0 || count > engine_spatial::MAX_DENSITY_EDIT_VOXELS {
            return Err(voxel_error(
                "CSHARP_VOXEL_DENSITY_READ",
                "a density read covers 1 to 16,777,216 voxels",
            ));
        }
        let min = address(request.min);
        let session = self.session_mut(request.session)?;
        let scene = session.scene.as_ref();
        let mut samples = Vec::with_capacity(count as usize);
        for z in 0..i64::from(request.size_z) {
            for y in 0..i64::from(request.size_y) {
                for x in 0..i64::from(request.size_x) {
                    let at = [min[0] + x, min[1] + y, min[2] + z];
                    samples.push(match scene.density(at) {
                        Some(density) => NativeVoxelDensitySample {
                            density,
                            material_slot: scene
                                .material_voxel(at)
                                .map_or(0, |voxel| u32::from(voxel.material_slot)),
                            resident: true,
                        },
                        None => NativeVoxelDensitySample {
                            density: engine_spatial::DEFAULT_DENSITY_MAGNITUDE,
                            material_slot: 0,
                            resident: false,
                        },
                    });
                }
            }
        }
        let result = NativeVoxelDensityResult {
            samples: samples.as_ptr(),
            samples_len: samples.len(),
        };
        self.borrowed.hold(samples);
        Ok(result)
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
            session.change_collision(|scene| {
                match engine_spatial::VoxelEditService::apply(scene, &edits) {
                    Ok(receipt) => {
                        let changed = collision_reach(
                            scene,
                            voxel_box(
                                scene,
                                receipt.fact.changed_min,
                                receipt.fact.changed_max_inclusive,
                            ),
                        );
                        let mut native = native_edit_receipt(&receipt);
                        native.mesh_microseconds = scene.mesh_update().mesh_microseconds;
                        (Ok(native), vec![changed])
                    }
                    Err(VoxelEditApplyError::Rejected(VoxelEditRejection::NoChanges)) => {
                        let revision = scene.source_revision().raw();
                        let receipt = NativeVoxelEditReceipt {
                            status: NativeVoxelEditStatus::NoChanges,
                            revision_before: revision,
                            accepted_revision: revision,
                            ..Default::default()
                        };
                        (Ok(receipt), Vec::new())
                    }
                    Err(error) => (
                        Err(voxel_error("CSHARP_VOXEL_EDIT", error.to_string())),
                        Vec::new(),
                    ),
                }
            })
        })?
    }

    fn apply_voxel_residency(
        &mut self,
        request: &NativeVoxelResidencyTransaction,
    ) -> Result<NativeVoxelResidencyReceipt, CsharpEngineServicesError> {
        let chunk_size = self.session_mut(request.session)?.scene.chunk_size();
        let operations = translate_residency(request, chunk_size)?;
        self.edit_scene(request.session, |session| {
            session.change_collision(|scene| {
                match VoxelChunkResidencyService::apply(scene, &operations) {
                    Ok(receipt) => {
                        let changed = receipt
                            .admitted
                            .iter()
                            .chain(&receipt.replaced)
                            .chain(&receipt.evicted)
                            .map(|chunk| collision_reach(scene, chunk_box(scene, *chunk)))
                            .collect();
                        let mut native = native_residency_receipt(&receipt);
                        native.mesh_microseconds = scene.mesh_update().mesh_microseconds;
                        (Ok(native), changed)
                    }
                    // A batch that changes nothing is an ordinary outcome.
                    Err(VoxelChunkResidencyApplyError::Rejected(
                        VoxelChunkResidencyRejection::NoChanges { retained },
                    )) => {
                        let revision = scene.source_revision().raw();
                        let receipt = NativeVoxelResidencyReceipt {
                            revision_before: revision,
                            accepted_revision: revision,
                            retained_count: narrow(retained.len()),
                            resident_chunk_count: scene.resident_chunk_count() as u64,
                            resident_solid_voxel_count: scene.solid_voxel_count() as u64,
                            authority_hash: scene.authority_hash(),
                            collision_revision: revision,
                            mesh_revision: revision,
                            ..Default::default()
                        };
                        (Ok(receipt), Vec::new())
                    }
                    Err(error) => (
                        Err(voxel_error("CSHARP_VOXEL_RESIDENCY", error.to_string())),
                        Vec::new(),
                    ),
                }
            })
        })?
    }
}

/// World bounds of an inclusive voxel address range.
pub(crate) fn voxel_box(
    scene: &engine_spatial::VoxelCollisionScene,
    min: [i64; 3],
    max_inclusive: [i64; 3],
) -> ([f64; 3], [f64; 3]) {
    let grid = scene.voxel_world().grid();
    let min = grid.voxel_min_world(core_space::VoxelCoord::new(min[0], min[1], min[2]));
    let (_, max) = grid.voxel_bounds_world(core_space::VoxelCoord::new(
        max_inclusive[0],
        max_inclusive[1],
        max_inclusive[2],
    ));
    ([min.x, min.y, min.z], [max.x, max.y, max.z])
}

/// A changed box grown by the reach of a reconstructed surface: its
/// triangles move with the samples within two voxels of them.
pub(crate) fn collision_reach(
    scene: &engine_spatial::VoxelCollisionScene,
    (min, max): ([f64; 3], [f64; 3]),
) -> ([f64; 3], [f64; 3]) {
    if scene.mesh_options().all_greedy() {
        return (min, max);
    }
    let reach = 2.0 * scene.voxel_size();
    (
        min.map(|value| value - reach),
        max.map(|value| value + reach),
    )
}

fn surface_character(value: NativeSurfaceCharacter) -> SurfaceCharacter {
    SurfaceCharacter {
        placement: match value.placement {
            NativeVertexPlacement::Sharp => VertexPlacement::Sharp,
            NativeVertexPlacement::Smooth => VertexPlacement::Smooth,
            NativeVertexPlacement::Blocky => VertexPlacement::Blocky,
        },
        crease_angle_degrees: value.crease_angle_degrees,
        roughness: value.roughness,
    }
}

fn native_density_edit(
    edit: &NativeVoxelDensityEdit,
    densities: &[f32],
    materials: &[u32],
) -> Result<VoxelDensityEdit, CsharpEngineServicesError> {
    let invalid = |message: &str| voxel_error("CSHARP_VOXEL_DENSITY_EDIT", message);
    let span = |offset: u32, count: u32, len: usize| {
        let start = offset as usize;
        let end = start
            .checked_add(count as usize)
            .filter(|end| *end <= len)
            .ok_or_else(|| invalid("an edit's range exceeds the transaction's span"))?;
        Ok::<_, CsharpEngineServicesError>(start..end)
    };
    let material_slot =
        u16::try_from(edit.material_slot).map_err(|_| invalid("material slot exceeded u16"))?;
    Ok(match edit.kind {
        NativeVoxelDensityEditKind::Region => VoxelDensityEdit::Region {
            min: address(edit.min),
            size: [edit.size_x, edit.size_y, edit.size_z],
            densities: densities[span(edit.density_offset, edit.density_count, densities.len())?]
                .to_vec(),
            materials: materials[span(edit.material_offset, edit.material_count, materials.len())?]
                .iter()
                .map(|slot| u16::try_from(*slot).map_err(|_| invalid("material slot exceeded u16")))
                .collect::<Result<_, _>>()?,
        },
        NativeVoxelDensityEditKind::Brush => {
            let vector = |value: NativeVec3| [value.x, value.y, value.z].map(f64::from);
            VoxelDensityEdit::Brush {
                shape: match edit.shape {
                    NativeVoxelDensityShape::Sphere => VoxelDensityShape::Sphere {
                        center: vector(edit.center),
                        radius: f64::from(edit.radius),
                    },
                    NativeVoxelDensityShape::Box => VoxelDensityShape::Box {
                        min: vector(edit.box_min),
                        max: vector(edit.box_max),
                    },
                },
                operation: match edit.operation {
                    NativeVoxelDensityOperation::Add => VoxelDensityOperation::Add,
                    NativeVoxelDensityOperation::Subtract => VoxelDensityOperation::Subtract,
                    NativeVoxelDensityOperation::Smooth => VoxelDensityOperation::Smooth {
                        strength: edit.strength,
                    },
                    NativeVoxelDensityOperation::Paint => VoxelDensityOperation::Paint,
                },
                material_slot,
            }
        }
    })
}

fn native_density_receipt(
    scene: &engine_spatial::VoxelCollisionScene,
    receipt: &engine_spatial::VoxelDensityReceipt,
) -> NativeVoxelDensityReceipt {
    let revisions = scene.projection_revisions();
    NativeVoxelDensityReceipt {
        status: NativeVoxelEditStatus::Accepted,
        revision_before: receipt.revision_before.raw(),
        accepted_revision: receipt.accepted_revision.raw(),
        changed_voxels: narrow(receipt.changed_voxels),
        solidity_changes: narrow(receipt.solidity_changes),
        changed_min: native_address(receipt.changed_min),
        changed_max_inclusive: native_address(receipt.changed_max_inclusive),
        solid_voxel_count: receipt.solid_voxel_count as u64,
        authority_hash: receipt.authority_hash,
        collision_revision: revisions.collision().raw(),
        mesh_revision: revisions.mesh().raw(),
        dirty_chunk_count: narrow(receipt.dirty_mesh_chunks.len()),
        rebuilt_mesh_chunks: narrow(receipt.rebuilt_mesh_chunks),
        reused_mesh_chunks: narrow(receipt.reused_mesh_chunks),
        removed_mesh_chunks: narrow(receipt.removed_mesh_chunks),
        mesh_microseconds: receipt.mesh_microseconds,
    }
}

/// World bounds of one voxel chunk.
fn chunk_box(
    scene: &engine_spatial::VoxelCollisionScene,
    chunk: engine_spatial::VoxelChunkIdentity,
) -> ([f64; 3], [f64; 3]) {
    let size = i64::from(scene.chunk_size());
    let min = [chunk.x * size, chunk.y * size, chunk.z * size];
    voxel_box(scene, min, min.map(|value| value + size - 1))
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
    densities: &[f32],
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
    if operation.density_count != 0 {
        let start = operation.density_offset as usize;
        payload.densities = start
            .checked_add(operation.density_count as usize)
            .and_then(|end| densities.get(start..end))
            .ok_or_else(|| {
                voxel_error(
                    "CSHARP_VOXEL_RESIDENCY",
                    "density range exceeded the transaction span",
                )
            })?
            .to_vec();
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
        mesh_revision: receipt.projections.mesh().raw(),
        changed_voxels: narrow(receipt.fact.changed_voxels),
        changed_min: native_address(receipt.fact.changed_min),
        changed_max_inclusive: native_address(receipt.fact.changed_max_inclusive),
        dirty_chunk_count: narrow(receipt.dirty_mesh_chunks.len()),
        rebuilt_mesh_chunks: narrow(receipt.rebuilt_mesh_chunks),
        reused_mesh_chunks: narrow(receipt.reused_mesh_chunks),
        removed_mesh_chunks: narrow(receipt.removed_mesh_chunks),
        status: NativeVoxelEditStatus::Accepted,
        mesh_microseconds: 0,
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
        mesh_revision: receipt.projections.mesh().raw(),
        dirty_chunk_count: narrow(receipt.dirty_chunks.len()),
        rebuilt_mesh_chunks: narrow(receipt.rebuilt_mesh_chunks),
        reused_mesh_chunks: narrow(receipt.reused_mesh_chunks),
        removed_mesh_chunks: narrow(receipt.removed_mesh_chunks),
        mesh_microseconds: 0,
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

unsafe extern "C" fn read_scene(
    context: *mut c_void,
    request: NativeVoxelSceneReadRequest,
    output: *mut NativeVoxelSceneReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_voxel_scene(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    request: NativeVoxelReadRequest,
    output: *mut NativeVoxelReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_voxel(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

unsafe extern "C" fn read_chunk(
    context: *mut c_void,
    request: NativeVoxelChunkReadRequest,
    output: *mut NativeVoxelChunkReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_voxel_chunk(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
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
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
    }
}

unsafe extern "C" fn configure_material_occlusion(
    context: *mut c_void,
    request: *const NativeVoxelMaterialOcclusionRequest,
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
                "voxel material occlusion declarations",
            )
        }?;
        let mut non_occluding = std::collections::BTreeSet::new();
        for material in materials.iter().filter(|material| !material.occludes) {
            non_occluding.insert(u16::try_from(material.material_slot).map_err(|_| {
                voxel_error(
                    "CSHARP_VOXEL_MATERIAL_OCCLUSION",
                    "material slot must be in 0..=65535",
                )
            })?);
        }
        let options = SurfaceMeshOptions {
            non_occluding,
            ..bridge
                .session_mut(request.session)?
                .scene
                .mesh_options()
                .clone()
        };
        bridge
            .edit_scene(request.session, |session| {
                Arc::make_mut(&mut session.scene).set_mesh_options(options)
            })?
            .map_err(|error| voxel_error("CSHARP_VOXEL_MATERIAL_OCCLUSION", error.to_string()))
    })();
    match result {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, receipt);
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
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
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
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
    }
}

pub(crate) fn api(bridge: &mut RuntimeSpatialBridge) -> NativeVoxelApi {
    NativeVoxelApi {
        context: (bridge as *mut RuntimeSpatialBridge).cast(),
        configure_material_collision,
        configure_material_occlusion,
        read_scene,
        read,
        sample_direct_lighting,
        read_chunk,
        apply_edits,
        apply_residency,
        configure_material_surfaces,
        apply_density_edits,
        read_densities,
        configure_terrain_layers,
        configure_vertex_occlusion,
    }
}

unsafe extern "C" fn configure_vertex_occlusion(
    context: *mut c_void,
    request: *const NativeVoxelVertexOcclusionRequest,
    output: *mut NativeVoxelSceneReadout,
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
    match bridge.configure_voxel_vertex_occlusion(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
    }
}

unsafe extern "C" fn configure_material_surfaces(
    context: *mut c_void,
    request: *const NativeVoxelMaterialSurfaceRequest,
    output: *mut NativeVoxelSceneReadout,
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
    match bridge.configure_voxel_material_surfaces(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
    }
}

unsafe extern "C" fn configure_terrain_layers(
    context: *mut c_void,
    request: *const NativeVoxelTerrainLayerRequest,
    output: *mut NativeVoxelSceneReadout,
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
    match bridge.configure_voxel_terrain_layers(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
    }
}

unsafe extern "C" fn apply_density_edits(
    context: *mut c_void,
    request: *const NativeVoxelDensityTransaction,
    output: *mut NativeVoxelDensityReceipt,
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
    match bridge.apply_voxel_density_edits(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
    }
}

unsafe extern "C" fn read_densities(
    context: *mut c_void,
    request: NativeVoxelDensityReadRequest,
    output: *mut NativeVoxelDensityResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_voxel_densities(request) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
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
    let densities = unsafe {
        crate::composition::borrowed_slice(
            request.densities,
            request.densities_len,
            "voxel residency densities",
        )
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
                let payload =
                    native_payload(*operation, chunk_size, material_slots, states, densities)?;
                VoxelChunkResidencyOperation::Admit { chunk, payload }
            }
            NativeVoxelResidencyOperationKind::Replace => {
                let payload =
                    native_payload(*operation, chunk_size, material_slots, states, densities)?;
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
            bridge.operation_diagnostics.retain(&e, error);
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
                    std::ptr::null_mut(),
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

    #[test]
    fn material_occlusion_shows_the_bed_under_water_through_edits() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = create_session(&mut bridge);
        let api = api(&mut bridge);
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        let mut configure = |bridge: &mut RuntimeSpatialBridge, occludes| {
            let materials = [NativeVoxelMaterialOcclusion {
                material_slot: 11,
                occludes,
            }];
            let request = NativeVoxelMaterialOcclusionRequest {
                session,
                materials: materials.as_ptr(),
                materials_len: materials.len(),
            };
            assert_eq!(
                unsafe { (api.configure_material_occlusion)(api.context, &request, &mut receipt) },
                ABI_OK
            );
            bridge
                .session_mut(session)
                .unwrap()
                .scene
                .mesh_chunks()
                .map(|chunk| chunk.quads)
                .sum::<u32>()
        };
        let edit = |bridge: &mut RuntimeSpatialBridge, x, y, material_slot| {
            let edits = [set(NativeVoxelAddress { x, y, z: 0 }, material_slot)];
            bridge
                .apply_voxel_edits(&NativeVoxelEditTransaction {
                    session,
                    edits: edits.as_ptr(),
                    edits_len: edits.len(),
                })
                .unwrap();
            bridge
                .session_mut(session)
                .unwrap()
                .scene
                .mesh_chunks()
                .map(|chunk| chunk.quads)
                .sum::<u32>()
        };
        // Stone under two water voxels: five stone and five water quads,
        // the stone top hidden.
        edit(&mut bridge, 0, 0, 1);
        edit(&mut bridge, 0, 1, 11);
        assert_eq!(edit(&mut bridge, 0, 2, 11), 10);
        // Declared non-occluding, the water shows the stone top but still
        // hides its own inner face and draws nothing against the stone.
        assert_eq!(configure(&mut bridge, false), 11);
        // The declaration survives later edits.
        assert_eq!(edit(&mut bridge, 0, 3, 11), 11);
        assert_eq!(configure(&mut bridge, true), 10);
    }

    #[test]
    fn terrain_layers_weigh_reconstructed_vertices_and_leave_geometry() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = create_session(&mut bridge);
        let api = api(&mut bridge);
        // Sand (1) then rock (2) across a 16 x 2 x 16 floor, dual contoured.
        let floor: Vec<_> = (0..16)
            .flat_map(|x| (0..16).flat_map(move |z| (0..2).map(move |y| (x, y, z))))
            .map(|(x, y, z)| set(NativeVoxelAddress { x, y, z }, if x < 8 { 1 } else { 2 }))
            .collect();
        bridge
            .apply_voxel_edits(&NativeVoxelEditTransaction {
                session,
                edits: floor.as_ptr(),
                edits_len: floor.len(),
            })
            .unwrap();
        let mut readout = NativeVoxelSceneReadout::default();
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.configure_material_surfaces)(
                    api.context,
                    &NativeVoxelMaterialSurfaceRequest {
                        session,
                        mode: NativeVoxelSurfaceMode::DualContouring,
                        materials: std::ptr::null(),
                        materials_len: 0,
                    },
                    &mut readout,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        let meshes = |bridge: &mut RuntimeSpatialBridge| {
            bridge
                .session_mut(session)
                .unwrap()
                .scene
                .mesh_chunks()
                .cloned()
                .collect::<Vec<_>>()
        };
        let plain = meshes(&mut bridge);
        let configure_mapped = |slots: &[u32], layers: &[u32], transition_cells: u32| {
            let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
            unsafe {
                (api.configure_terrain_layers)(
                    api.context,
                    &NativeVoxelTerrainLayerRequest {
                        session,
                        slots: slots.as_ptr(),
                        slots_len: slots.len(),
                        transition_cells,
                        layers: layers.as_ptr(),
                        layers_len: layers.len(),
                    },
                    &mut NativeVoxelSceneReadout::default(),
                    &mut receipt,
                )
            }
        };
        let configure =
            |slots: &[u32], transition_cells: u32| configure_mapped(slots, &[], transition_cells);
        // Malformed sets and a transition as long as the 8-voxel chunk are
        // refused, leaving the meshes as they were.
        for (slots, cells) in [
            (vec![1, 2, 3, 4, 5], 1),
            (vec![1, 1], 1),
            (vec![1, 2], 0),
            (vec![1, 2], 7),
            (vec![70_000], 1),
        ] {
            assert_eq!(configure(&slots, cells), 0, "{slots:?} {cells}");
        }
        // So are duplicate or conflicting slot mappings, a layer past 3, a
        // layer count that does not match the slots, and too many slots.
        let many: Vec<u32> = (1..=17).collect();
        for (slots, layers) in [
            (vec![1, 2, 1], vec![0, 1, 0]),
            (vec![1, 2, 1], vec![0, 1, 1]),
            (vec![1, 2], vec![0, 4]),
            (vec![1, 2], vec![0, 256]),
            (vec![1, 2], vec![0]),
            (vec![], vec![0]),
            (many.clone(), vec![0; many.len()]),
        ] {
            assert_eq!(
                configure_mapped(&slots, &layers, 1),
                0,
                "{slots:?} {layers:?}"
            );
        }
        assert_eq!(meshes(&mut bridge), plain);
        assert_eq!(configure(&[1, 2], 2), ABI_OK);
        let layered = meshes(&mut bridge);
        for (plain, layered) in plain.iter().zip(&layered) {
            assert_eq!(plain.positions, layered.positions);
            assert_eq!(plain.indices, layered.indices);
            assert_eq!(plain.groups, layered.groups);
            assert_eq!(layered.layer_weights.len(), layered.positions.len() / 3 * 4);
        }
        assert!(layered
            .iter()
            .flat_map(|chunk| chunk.layer_weights.chunks(4))
            .any(|weights| weights[0] > 0.0 && weights[1] > 0.0));
        // Sand and an unused slot 3 mapped onto layer 1, rock onto layer 3:
        // the same blend lands on layers 1 and 3, geometry still unchanged.
        assert_eq!(configure_mapped(&[1, 3, 2], &[1, 1, 3], 2), ABI_OK);
        let mapped = meshes(&mut bridge);
        for (layered, mapped) in layered.iter().zip(&mapped) {
            assert_eq!(layered.positions, mapped.positions);
            assert_eq!(layered.groups, mapped.groups);
            for (layered, mapped) in layered
                .layer_weights
                .chunks(4)
                .zip(mapped.layer_weights.chunks(4))
            {
                assert_eq!([0.0, layered[0], 0.0, layered[1]], mapped);
            }
        }
        // No slots removes the weights.
        assert_eq!(configure(&[], 0), ABI_OK);
        assert_eq!(meshes(&mut bridge), plain);
    }

    #[test]
    fn material_surfaces_and_density_edits_reshape_what_collision_meets() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = create_session(&mut bridge);
        let api = api(&mut bridge);
        // A 16 x 2 x 16 stone floor.
        let floor: Vec<_> = (0..16)
            .flat_map(|x| (0..16).flat_map(move |z| (0..2).map(move |y| (x, y, z))))
            .map(|(x, y, z)| set(NativeVoxelAddress { x, y, z }, 1))
            .collect();
        bridge
            .apply_voxel_edits(&NativeVoxelEditTransaction {
                session,
                edits: floor.as_ptr(),
                edits_len: floor.len(),
            })
            .unwrap();
        let surfaces = [NativeVoxelMaterialSurface {
            material_slot: 1,
            mode: NativeVoxelSurfaceMode::DualContouring,
            character: NativeSurfaceCharacter {
                placement: NativeVertexPlacement::Smooth,
                crease_angle_degrees: 180.0,
                roughness: 0.0,
            },
        }];
        let mut readout = NativeVoxelSceneReadout::default();
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.configure_material_surfaces)(
                    api.context,
                    &NativeVoxelMaterialSurfaceRequest {
                        session,
                        mode: NativeVoxelSurfaceMode::GreedyCubes,
                        materials: surfaces.as_ptr(),
                        materials_len: surfaces.len(),
                    },
                    &mut readout,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(readout.solid_voxel_count, 16 * 16 * 2);
        // Blast a crater; collision follows its drawn floor.
        let edits = [NativeVoxelDensityEdit {
            kind: NativeVoxelDensityEditKind::Brush,
            shape: NativeVoxelDensityShape::Sphere,
            operation: NativeVoxelDensityOperation::Subtract,
            center: NativeVec3 {
                x: 8.0,
                y: 2.0,
                z: 8.0,
            },
            radius: 1.5,
            material_slot: 1,
            ..Default::default()
        }];
        let mut density = NativeVoxelDensityReceipt::default();
        assert_eq!(
            unsafe {
                (api.apply_density_edits)(
                    api.context,
                    &NativeVoxelDensityTransaction {
                        session,
                        edits: edits.as_ptr(),
                        edits_len: edits.len(),
                        densities: std::ptr::null(),
                        densities_len: 0,
                        materials: std::ptr::null(),
                        materials_len: 0,
                    },
                    &mut density,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(density.status, NativeVoxelEditStatus::Accepted);
        assert!(density.solidity_changes > 0);
        assert!(density.mesh_microseconds > 0);
        let scene = &bridge.session_mut(session).unwrap().scene;
        let floor = scene
            .raycast([8.0, 5.0, 8.0], [0.0, -1.0, 0.0], 10.0)
            .unwrap();
        assert!(
            floor.point[1] < 1.2 && floor.point[1] > 0.0,
            "crater floor {:?}",
            floor.point
        );
        let mut samples = unsafe { std::mem::zeroed::<NativeVoxelDensityResult>() };
        assert_eq!(
            unsafe {
                (api.read_densities)(
                    api.context,
                    NativeVoxelDensityReadRequest {
                        session,
                        min: NativeVoxelAddress { x: 7, y: 1, z: 8 },
                        size_x: 2,
                        size_y: 1,
                        size_z: 1,
                    },
                    &mut samples,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        let samples = unsafe { std::slice::from_raw_parts(samples.samples, samples.samples_len) };
        assert_eq!(samples.len(), 2);
        assert!(samples.iter().all(|sample| sample.resident));
        assert!(samples[1].density >= 0.0 && samples[1].material_slot == 0);
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
            shadow_resolution: 0,
            shadow_priority: 0,
            shadow_soft: false,
            ground_color: Default::default(),
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
        assert_ne!(error.diagnostics_len, 0);
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
        assert_eq!(error.diagnostics_len, 0);

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
        assert_eq!(no_change_error.diagnostics_len, 0);
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
        assert_eq!(invalid_error.diagnostics_len, 1);
        let diagnostic = unsafe { *invalid_error.diagnostics };
        assert_eq!(copied_utf8(diagnostic.code), "CSHARP_VOXEL_EDIT");
    }
}
