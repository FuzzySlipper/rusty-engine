//! Retained renderer projection for canonical Spatial voxel scenes.
//!
//! The product selects live Appearance materials and a Spatial session.  The
//! Engine resolves the current `VoxelCollisionScene`, projects it through the
//! normal incremental voxel projector, and stages renderer work.  No mesh or
//! renderer object is ever admitted from C#.

use runtime_diagnostics::RuntimeUpdateAttribution;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    time::Instant,
};

use core_space::Direction6;
use csharp_engine_abi::*;
use engine_spatial::{SurfaceMode, VoxelCollisionScene};
use render_model::{
    RenderDiff, RenderFrameDiff, RenderMaterialDescriptor, TextureDescriptor, Transform,
};
use render_projection::{
    voxel_material_id, VoxelMaterialSlotMapping, VoxelProjectionInstance, VoxelRenderProjector,
};

use crate::{
    appearance::RuntimeAppearanceBridge,
    composition::{borrowed_slice, ABI_OK},
    spatial::SpatialCollisionSource,
    CsharpEngineServicesError,
};

#[derive(Debug, Clone)]
struct RetainedVoxelScenePresentation {
    session: NativeSpatialSessionHandle,
    base_materials: BTreeMap<u16, RenderMaterialDescriptor>,
    face_materials: BTreeMap<(u16, Direction6), RenderMaterialDescriptor>,
    textures: BTreeMap<String, TextureDescriptor>,
    base_renderer_slots: BTreeMap<u16, u16>,
    face_renderer_slots: BTreeMap<(u16, Direction6), u16>,
    base_material_provenance: BTreeMap<u16, u64>,
    face_material_provenance: BTreeMap<(u16, Direction6), u64>,
    base_material_count: u32,
}

#[derive(Debug, Clone, Default)]
struct VoxelScenePresentationState {
    presentations: BTreeMap<u64, RetainedVoxelScenePresentation>,
    projector: VoxelRenderProjector,
    next_presentation: u64,
    material_mapping_leases: BTreeMap<u64, Box<[NativeVoxelSceneMaterialMappingRow]>>,
    next_material_mapping_lease: u64,
}

pub(crate) struct RuntimeVoxelScenePresentationCall {
    state: VoxelScenePresentationState,
    pub(crate) frames: Vec<RenderFrameDiff>,
}

/// The Engine retains only presentation identity and copied material
/// descriptors.  The canonical scene stays owned by Spatial and is resolved
/// synchronously for each explicit projection update.
pub(crate) struct RuntimeVoxelScenePresentationBridge {
    spatial: SpatialCollisionSource,
    state: VoxelScenePresentationState,
    staged: Option<RuntimeVoxelScenePresentationCall>,
    appearance: Option<*mut RuntimeAppearanceBridge>,
    update_attribution: RuntimeUpdateAttribution,
}

impl RuntimeVoxelScenePresentationBridge {
    pub(crate) fn new(spatial: SpatialCollisionSource) -> Self {
        Self {
            spatial,
            state: VoxelScenePresentationState {
                presentations: BTreeMap::new(),
                projector: VoxelRenderProjector::new(),
                next_presentation: 1,
                material_mapping_leases: BTreeMap::new(),
                next_material_mapping_lease: 1,
            },
            staged: None,
            appearance: None,
            update_attribution: RuntimeUpdateAttribution::default(),
        }
    }

    pub(crate) fn bind_appearance(&mut self, appearance: &mut RuntimeAppearanceBridge) {
        // The sibling bridge is retained by EngineServiceSet.  This pointer is
        // refreshed while assembling the call table and used only during the
        // synchronous generated callback.
        self.appearance = Some(appearance as *mut RuntimeAppearanceBridge);
    }

    pub(crate) fn reset_update_attribution(&mut self) {
        self.update_attribution = RuntimeUpdateAttribution::default();
    }

    pub(crate) fn update_attribution(&self) -> RuntimeUpdateAttribution {
        self.update_attribution
    }

    fn record_presentation_attribution(&mut self, duration_us: u64) {
        self.update_attribution.voxel_scene_presentation_calls = self
            .update_attribution
            .voxel_scene_presentation_calls
            .saturating_add(1);
        self.update_attribution.voxel_scene_presentation_duration_us = self
            .update_attribution
            .voxel_scene_presentation_duration_us
            .saturating_add(duration_us);
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeVoxelScenePresentationCall {
            state: self.state.clone(),
            frames: Vec::new(),
        });
    }

    pub(crate) fn begin_attach_call(&mut self) {
        self.begin_call();
        let staged = self
            .staged
            .as_mut()
            .expect("attach begins a voxel scene presentation stage");
        // A fresh renderer has no publication revision, handles, materials,
        // or mesh payloads. Reset only the detached clone so RefreshScene
        // emits a complete baseline without changing the active runtime's
        // retained projector history.
        staged.state.projector = VoxelRenderProjector::new();
    }

    pub(crate) fn discard_call(&mut self) {
        self.staged = None;
    }

    /// Reprojects the retained active presentation state from Spatial without
    /// adopting any state staged by the failed product call. This is the one
    /// narrow recovery path for an immediate voxel mutation that outlived a
    /// later callback failure.
    ///
    /// The clone intentionally retains the active projector's publication
    /// history so the frame can repair an already attached renderer. It is
    /// never committed: recovery must not advance retained projector history
    /// or make a later ordinary call observe a failed callback's staging.
    pub(crate) fn recover_from_canonical(
        &self,
    ) -> Result<Vec<RenderFrameDiff>, CsharpEngineServicesError> {
        let mut recovery = self.state.clone();
        let frame = project_all_presentations(&mut recovery, &self.spatial)?;
        Ok((!frame.ops.is_empty())
            .then_some(frame)
            .into_iter()
            .collect())
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeVoxelScenePresentationCall, CsharpEngineServicesError> {
        self.staged.take().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_CALL",
                "voxel scene presentation was read outside a product call",
            )
        })
    }

    pub(crate) fn commit_call(&mut self, call: RuntimeVoxelScenePresentationCall) {
        self.state = call.state;
    }

    fn staged_mut(
        &mut self,
    ) -> Result<&mut RuntimeVoxelScenePresentationCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_CALL",
                "voxel scene presentation was called outside a product call",
            )
        })
    }

    fn appearance_mut(
        &mut self,
    ) -> Result<&mut RuntimeAppearanceBridge, CsharpEngineServicesError> {
        let appearance = self.appearance.ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_APPEARANCE",
                "voxel scene presentation has no Engine appearance owner",
            )
        })?;
        // SAFETY: bind_appearance points at the sibling owner retained by
        // EngineServiceSet. Callbacks do not retain it beyond this call.
        Ok(unsafe { &mut *appearance })
    }

    fn project_scene(
        &mut self,
        request: NativeProjectVoxelSceneRequest,
    ) -> Result<NativeVoxelScenePresentationHandle, CsharpEngineServicesError> {
        self.project_scene_directional(NativeProjectVoxelSceneDirectionalRequest {
            session: request.session,
            materials: request.materials,
            materials_len: request.materials_len,
            face_materials: std::ptr::null(),
            face_materials_len: 0,
        })
    }

    fn project_scene_directional(
        &mut self,
        request: NativeProjectVoxelSceneDirectionalRequest,
    ) -> Result<NativeVoxelScenePresentationHandle, CsharpEngineServicesError> {
        let scene = self.spatial.scene(request.session)?;
        let resolved = self.materials_for_scene(
            &scene,
            request.materials,
            request.materials_len,
            request.face_materials,
            request.face_materials_len,
        )?;
        let staged = self.staged_mut()?;
        let value = staged.state.next_presentation;
        staged.state.next_presentation = value.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                "voxel scene presentation handle space overflowed",
            )
        })?;
        let renderer_slots = allocate_renderer_slots(
            resolved.base_materials.keys().map(|slot| (*slot, *slot)),
            staged
                .state
                .presentations
                .values()
                .flat_map(|presentation| {
                    presentation
                        .base_renderer_slots
                        .values()
                        .chain(presentation.face_renderer_slots.values())
                        .copied()
                }),
        )?;
        let face_renderer_slots = allocate_renderer_slots(
            resolved
                .face_materials
                .keys()
                .map(|(slot, direction)| ((*slot, *direction), *slot)),
            staged
                .state
                .presentations
                .values()
                .flat_map(|presentation| {
                    presentation
                        .base_renderer_slots
                        .values()
                        .chain(presentation.face_renderer_slots.values())
                        .copied()
                })
                .chain(renderer_slots.values().copied()),
        )?;
        staged.state.presentations.insert(
            value,
            RetainedVoxelScenePresentation {
                session: request.session,
                base_materials: resolved.base_materials,
                face_materials: resolved.face_materials,
                textures: resolved.textures,
                base_renderer_slots: renderer_slots,
                face_renderer_slots,
                base_material_provenance: resolved.base_material_provenance,
                face_material_provenance: resolved.face_material_provenance,
                base_material_count: resolved.base_material_count,
            },
        );
        self.refresh(NativeVoxelScenePresentationHandle { value })?;
        Ok(NativeVoxelScenePresentationHandle { value })
    }

    fn refresh(
        &mut self,
        handle: NativeVoxelScenePresentationHandle,
    ) -> Result<NativeVoxelScenePresentationReadout, CsharpEngineServicesError> {
        let spatial = self.spatial.clone();
        let staged = self.staged_mut()?;
        let presentation = staged
            .state
            .presentations
            .get(&handle.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                    "voxel scene presentation handle is not retained",
                )
            })?;
        let readout = presentation_readout(presentation, &spatial)?;
        let frame = project_all_presentations(&mut staged.state, &spatial)?;
        staged.frames.push(frame);
        Ok(readout)
    }

    fn update(
        &mut self,
        request: NativeUpdateVoxelScenePresentationRequest,
    ) -> Result<NativeVoxelScenePresentationReadout, CsharpEngineServicesError> {
        self.update_directional(NativeUpdateVoxelScenePresentationDirectionalRequest {
            presentation: request.presentation,
            materials: request.materials,
            materials_len: request.materials_len,
            face_materials: std::ptr::null(),
            face_materials_len: 0,
        })
    }

    fn update_directional(
        &mut self,
        request: NativeUpdateVoxelScenePresentationDirectionalRequest,
    ) -> Result<NativeVoxelScenePresentationReadout, CsharpEngineServicesError> {
        let session = self
            .staged
            .as_ref()
            .and_then(|call| call.state.presentations.get(&request.presentation.value))
            .map(|presentation| presentation.session)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                    "voxel scene presentation handle is not retained",
                )
            })?;
        let scene = self.spatial.scene(session)?;
        let resolved = self.materials_for_scene(
            &scene,
            request.materials,
            request.materials_len,
            request.face_materials,
            request.face_materials_len,
        )?;
        let staged = self.staged_mut()?;
        let retained_base_slots = staged
            .state
            .presentations
            .get(&request.presentation.value)
            .expect("presentation existence was checked before material resolution")
            .base_renderer_slots
            .iter()
            .filter(|(source_slot, _)| resolved.base_materials.contains_key(source_slot))
            .map(|(source_slot, renderer_slot)| (*source_slot, *renderer_slot))
            .collect::<BTreeMap<_, _>>();
        let missing_base_slots = resolved
            .base_materials
            .keys()
            .copied()
            .filter(|slot| !retained_base_slots.contains_key(slot))
            .collect::<Vec<_>>();
        let retained_face_slots = staged
            .state
            .presentations
            .get(&request.presentation.value)
            .expect("presentation existence was checked before material resolution")
            .face_renderer_slots
            .iter()
            .filter(|(key, _)| resolved.face_materials.contains_key(key))
            .map(|(key, renderer_slot)| (*key, *renderer_slot))
            .collect::<BTreeMap<_, _>>();
        let occupied = staged
            .state
            .presentations
            .iter()
            .filter(|(handle, _)| **handle != request.presentation.value)
            .flat_map(|(_, presentation)| {
                presentation
                    .base_renderer_slots
                    .values()
                    .chain(presentation.face_renderer_slots.values())
                    .copied()
            })
            .chain(retained_base_slots.values().copied())
            .chain(retained_face_slots.values().copied())
            .collect::<Vec<_>>();
        let new_base_slots = allocate_renderer_slots(
            missing_base_slots.into_iter().map(|slot| (slot, slot)),
            occupied,
        )?;
        let missing_face_slots = resolved
            .face_materials
            .keys()
            .copied()
            .filter(|key| !retained_face_slots.contains_key(key));
        let occupied_faces = staged
            .state
            .presentations
            .iter()
            .filter(|(handle, _)| **handle != request.presentation.value)
            .flat_map(|(_, presentation)| {
                presentation
                    .base_renderer_slots
                    .values()
                    .chain(presentation.face_renderer_slots.values())
                    .copied()
            })
            .chain(retained_base_slots.values().copied())
            .chain(new_base_slots.values().copied())
            .chain(retained_face_slots.values().copied());
        let new_face_slots = allocate_renderer_slots(
            missing_face_slots.map(|(slot, direction)| ((slot, direction), slot)),
            occupied_faces,
        )?;
        let presentation = staged
            .state
            .presentations
            .get_mut(&request.presentation.value)
            .expect("presentation existence was checked before material resolution");
        presentation.base_materials = resolved.base_materials;
        presentation.face_materials = resolved.face_materials;
        presentation.textures = resolved.textures;
        presentation.base_renderer_slots = retained_base_slots;
        presentation.base_renderer_slots.extend(new_base_slots);
        presentation.face_renderer_slots = retained_face_slots;
        presentation.face_renderer_slots.extend(new_face_slots);
        presentation.base_material_provenance = resolved.base_material_provenance;
        presentation.face_material_provenance = resolved.face_material_provenance;
        presentation.base_material_count = resolved.base_material_count;
        self.refresh(request.presentation)
    }

    fn destroy(
        &mut self,
        handle: NativeVoxelScenePresentationHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let spatial = self.spatial.clone();
        let staged = self.staged_mut()?;
        staged
            .state
            .presentations
            .remove(&handle.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                    "voxel scene presentation handle is not retained",
                )
            })?;
        let frame = project_all_presentations(&mut staged.state, &spatial)?;
        staged.frames.push(frame);
        Ok(())
    }

    fn read_material_mapping(
        &mut self,
        handle: NativeVoxelScenePresentationHandle,
    ) -> Result<NativeVoxelSceneMaterialMappingLease, CsharpEngineServicesError> {
        let spatial = self.spatial.clone();
        let staged = self.staged_mut()?;
        let presentation = staged
            .state
            .presentations
            .get(&handle.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                    "voxel scene presentation handle is not retained",
                )
            })?;
        let scene = spatial.scene(presentation.session)?;
        let rows = presentation
            .base_materials
            .keys()
            .flat_map(|source_slot| {
                Direction6::ALL
                    .into_iter()
                    .map(move |direction| (*source_slot, direction))
            })
            .map(|(source_slot, direction)| {
                let key = (source_slot, direction);
                let overridden = presentation.face_materials.contains_key(&key);
                Ok(NativeVoxelSceneMaterialMappingRow {
                    source_slot: u32::from(source_slot),
                    face: native_face(direction),
                    material_value: *(if overridden {
                        presentation.face_material_provenance.get(&key)
                    } else {
                        presentation.base_material_provenance.get(&source_slot)
                    })
                    .ok_or_else(|| {
                        CsharpEngineServicesError::new(
                            "CSHARP_VOXEL_SCENE_PRESENTATION_MAPPING",
                            "voxel scene material selection lost its provenance",
                        )
                    })?,
                    renderer_slot: u32::from(
                        *(if overridden {
                            presentation.face_renderer_slots.get(&key)
                        } else {
                            presentation.base_renderer_slots.get(&source_slot)
                        })
                        .ok_or_else(|| {
                            CsharpEngineServicesError::new(
                                "CSHARP_VOXEL_SCENE_PRESENTATION_MAPPING",
                                "voxel scene material selection lost its renderer slot",
                            )
                        })?,
                    ),
                    overridden,
                })
            })
            .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?
            .into_boxed_slice();
        let value = staged.state.next_material_mapping_lease;
        staged.state.next_material_mapping_lease = value.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_MAPPING",
                "voxel scene material mapping lease space overflowed",
            )
        })?;
        let lease = NativeVoxelSceneMaterialMappingLease {
            handle: NativeVoxelSceneMaterialMappingLeaseHandle { value },
            mappings: rows.as_ptr(),
            mappings_len: rows.len(),
            source_revision: scene.source_revision().raw(),
            mesh_revision: scene.projection_revisions().mesh().raw(),
        };
        staged.state.material_mapping_leases.insert(value, rows);
        Ok(lease)
    }

    fn destroy_material_mapping_lease(
        &mut self,
        handle: NativeVoxelSceneMaterialMappingLeaseHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        if handle.value == 0
            || staged
                .state
                .material_mapping_leases
                .remove(&handle.value)
                .is_none()
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_MAPPING",
                "voxel scene material mapping lease is not retained",
            ));
        }
        Ok(())
    }

    fn clear(
        &mut self,
    ) -> Result<NativeVoxelScenePresentationClearReceipt, CsharpEngineServicesError> {
        let spatial = self.spatial.clone();
        let staged = self.staged_mut()?;
        let cleared_count = u32::try_from(staged.state.presentations.len()).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_CLEAR",
                "voxel scene presentation count exceeded the C# receipt range",
            )
        })?;
        staged.state.presentations.clear();
        staged
            .frames
            .push(project_all_presentations(&mut staged.state, &spatial)?);
        Ok(NativeVoxelScenePresentationClearReceipt {
            cleared_count,
            retained_count: 0,
        })
    }

    fn materials_for_scene(
        &mut self,
        scene: &VoxelCollisionScene,
        pointer: *const NativeVoxelSceneMaterialBinding,
        len: usize,
        face_pointer: *const NativeVoxelSceneFaceMaterialBinding,
        face_len: usize,
    ) -> Result<ResolvedSceneMaterials, CsharpEngineServicesError> {
        // SAFETY: generated C# pins this bounded typed array for the direct
        // callback. Resolved descriptors are copied before returning.
        let bindings = unsafe { borrowed_slice(pointer, len, "voxel scene material bindings")? };
        let face_bindings = unsafe {
            borrowed_slice(face_pointer, face_len, "voxel scene face material bindings")?
        };
        let expected = scene
            .mesh_chunks()
            .iter()
            .flat_map(|chunk| chunk.groups.iter().map(|group| group.material_slot))
            .collect::<BTreeSet<_>>();
        let slots = bindings
            .iter()
            .map(|binding| {
                u16::try_from(binding.material_slot).map(|slot| (slot, binding.material))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_MATERIALS",
                    "voxel scene material slot exceeded the admitted scene slot range",
                )
            })?;
        if slots.len() != bindings.len()
            || slots.keys().copied().collect::<BTreeSet<_>>() != expected
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_MATERIALS",
                "voxel scene presentation requires exactly one live material binding for every used scene material slot",
            ));
        }
        if !face_bindings.is_empty()
            && scene
                .mesh_chunks()
                .iter()
                .any(|chunk| chunk.surface_mode != SurfaceMode::GreedyCubes)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_DIRECTIONAL",
                "face material overrides require a GreedyCubes voxel surface",
            ));
        }
        let mut overrides = BTreeMap::new();
        for binding in face_bindings {
            let slot = u16::try_from(binding.material_slot).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_MATERIALS",
                    "face material slot exceeded the admitted scene slot range",
                )
            })?;
            let direction = native_direction(binding.face)?;
            if !expected.contains(&slot) {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_DIRECTIONAL",
                    "face material override referenced an unused scene material slot",
                ));
            }
            if overrides
                .insert((slot, direction), binding.material)
                .is_some()
            {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_DIRECTIONAL",
                    "face material overrides contained a duplicate source slot and direction",
                ));
            }
        }
        let appearance = self.appearance_mut()?;
        let mut resolved_by_handle = BTreeMap::new();
        for material in slots.values().copied().chain(overrides.values().copied()) {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                resolved_by_handle.entry(material.value)
            {
                entry.insert(appearance.voxel_material_projection(material)?);
            }
        }
        let base_materials = slots
            .iter()
            .map(|(slot, material)| {
                let (descriptor, _) = resolved_by_handle
                    .get(&material.value)
                    .expect("all admitted material handles were resolved");
                (*slot, descriptor.clone())
            })
            .collect();
        let face_materials = overrides
            .iter()
            .map(|(key, material)| {
                let (descriptor, _) = resolved_by_handle
                    .get(&material.value)
                    .expect("all admitted material handles were resolved");
                (*key, descriptor.clone())
            })
            .collect();
        let textures = resolved_by_handle
            .into_values()
            .filter_map(|(_, texture)| texture.map(|texture| (texture.id.clone(), texture)))
            .collect();
        let base_material_count = u32::try_from(slots.len()).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_MATERIALS",
                "voxel scene material count exceeded the C# range",
            )
        })?;
        Ok(ResolvedSceneMaterials {
            base_materials,
            face_materials,
            textures,
            base_material_provenance: slots
                .into_iter()
                .map(|(slot, material)| (slot, material.value))
                .collect(),
            face_material_provenance: overrides
                .into_iter()
                .map(|(key, material)| (key, material.value))
                .collect(),
            base_material_count,
        })
    }
}

struct ResolvedSceneMaterials {
    base_materials: BTreeMap<u16, RenderMaterialDescriptor>,
    face_materials: BTreeMap<(u16, Direction6), RenderMaterialDescriptor>,
    textures: BTreeMap<String, TextureDescriptor>,
    base_material_provenance: BTreeMap<u16, u64>,
    face_material_provenance: BTreeMap<(u16, Direction6), u64>,
    base_material_count: u32,
}

fn native_direction(value: NativeSpatialFace) -> Result<Direction6, CsharpEngineServicesError> {
    match value {
        NativeSpatialFace::PosX => Ok(Direction6::PosX),
        NativeSpatialFace::NegX => Ok(Direction6::NegX),
        NativeSpatialFace::PosY => Ok(Direction6::PosY),
        NativeSpatialFace::NegY => Ok(Direction6::NegY),
        NativeSpatialFace::PosZ => Ok(Direction6::PosZ),
        NativeSpatialFace::NegZ => Ok(Direction6::NegZ),
        NativeSpatialFace::None => Err(CsharpEngineServicesError::new(
            "CSHARP_VOXEL_SCENE_PRESENTATION_DIRECTIONAL",
            "face material override must name one cube face",
        )),
    }
}

fn native_face(value: Direction6) -> NativeSpatialFace {
    match value {
        Direction6::PosX => NativeSpatialFace::PosX,
        Direction6::NegX => NativeSpatialFace::NegX,
        Direction6::PosY => NativeSpatialFace::PosY,
        Direction6::NegY => NativeSpatialFace::NegY,
        Direction6::PosZ => NativeSpatialFace::PosZ,
        Direction6::NegZ => NativeSpatialFace::NegZ,
    }
}

fn project_all_presentations(
    state: &mut VoxelScenePresentationState,
    spatial: &SpatialCollisionSource,
) -> Result<RenderFrameDiff, CsharpEngineServicesError> {
    let scenes = state
        .presentations
        .iter()
        .map(|(handle, presentation)| {
            spatial
                .scene(presentation.session)
                .map(|scene| (*handle, presentation.session, scene))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let instances = scenes
        .iter()
        .map(|(handle, session, scene)| VoxelProjectionInstance {
            instance_id: presentation_instance_id(*handle),
            asset_id: format!("spatial-session-{}", session.value),
            transform: Transform::IDENTITY,
            scene,
        })
        .collect::<Vec<_>>();
    let material_slots = state
        .presentations
        .iter()
        .map(|(handle, presentation)| {
            (
                presentation_instance_id(*handle),
                VoxelMaterialSlotMapping {
                    base: presentation.base_renderer_slots.clone(),
                    directional: presentation.face_renderer_slots.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let materials = renderer_materials(&state.presentations)?;
    let textures = presentation_textures(&state.presentations);
    let result = state
        .projector
        .project_mapped_directional(&instances, &materials, &material_slots)
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_VOXEL_SCENE_PRESENTATION", format!("{error:?}"))
        })?;
    let mut frame = result.frame;
    let used_textures = frame
        .ops
        .iter()
        .filter_map(|operation| match operation {
            RenderDiff::DefineMaterial { material } => material.texture.clone(),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if !used_textures.is_empty() {
        let mut operations = used_textures
            .into_iter()
            .map(|identity| {
                textures
                    .get(&identity)
                    .cloned()
                    .map(|texture| RenderDiff::DefineTexture { texture })
                    .ok_or_else(|| {
                        CsharpEngineServicesError::new(
                            "CSHARP_VOXEL_SCENE_PRESENTATION_TEXTURE",
                            "projected voxel material referenced an unavailable texture descriptor",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let publication = frame.publication.take();
        operations.append(&mut frame.ops);
        frame = if let Some(publication) = publication {
            RenderFrameDiff::try_from_published_ops(
                publication.stream,
                publication.base_revision,
                publication.revision,
                operations,
            )
        } else {
            RenderFrameDiff::try_from_ops(operations)
        }
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_VOXEL_SCENE_PRESENTATION", format!("{error:?}"))
        })?;
    }
    Ok(frame)
}

fn presentation_readout(
    presentation: &RetainedVoxelScenePresentation,
    spatial: &SpatialCollisionSource,
) -> Result<NativeVoxelScenePresentationReadout, CsharpEngineServicesError> {
    let scene = spatial.scene(presentation.session)?;
    Ok(NativeVoxelScenePresentationReadout {
        present: true,
        source_revision: scene.source_revision().raw(),
        mesh_revision: scene.projection_revisions().mesh().raw(),
        chunk_count: u64::try_from(scene.mesh_chunks().len()).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION",
                "voxel scene chunk count exceeded the C# receipt range",
            )
        })?,
        material_count: presentation.base_material_count,
    })
}

fn presentation_instance_id(handle: u64) -> String {
    format!("csharp-voxel-scene-presentation-{handle}")
}

fn renderer_materials(
    presentations: &BTreeMap<u64, RetainedVoxelScenePresentation>,
) -> Result<BTreeMap<u16, RenderMaterialDescriptor>, CsharpEngineServicesError> {
    presentations
        .values()
        .flat_map(|presentation| {
            presentation
                .base_materials
                .iter()
                .map(|(source_slot, descriptor)| {
                    (
                        presentation.base_renderer_slots.get(source_slot).copied(),
                        descriptor,
                    )
                })
                .chain(
                    presentation
                        .face_materials
                        .iter()
                        .map(|(source_slot, descriptor)| {
                            (
                                presentation.face_renderer_slots.get(source_slot).copied(),
                                descriptor,
                            )
                        }),
                )
        })
        .map(|(renderer_slot, descriptor)| {
            let slot = renderer_slot.ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_MATERIAL_SLOTS",
                    "voxel scene material had no retained renderer slot",
                )
            })?;
            let mut descriptor = descriptor.clone();
            descriptor.id = voxel_material_id(slot);
            Ok((slot, descriptor))
        })
        .collect()
}

fn presentation_textures(
    presentations: &BTreeMap<u64, RetainedVoxelScenePresentation>,
) -> BTreeMap<String, TextureDescriptor> {
    presentations
        .values()
        .flat_map(|presentation| presentation.textures.iter())
        .map(|(identity, texture)| (identity.clone(), texture.clone()))
        .collect()
}

fn allocate_renderer_slots<K: Ord + Copy>(
    source_slots: impl IntoIterator<Item = (K, u16)>,
    occupied_slots: impl IntoIterator<Item = u16>,
) -> Result<BTreeMap<K, u16>, CsharpEngineServicesError> {
    let mut occupied = occupied_slots.into_iter().collect::<BTreeSet<_>>();
    let mut mappings = BTreeMap::new();
    for (source_slot, preferred_slot) in source_slots {
        if mappings.contains_key(&source_slot) {
            continue;
        }
        let renderer_slot = (!occupied.contains(&preferred_slot))
            .then_some(preferred_slot)
            .or_else(|| (0..=u16::MAX).find(|candidate| !occupied.contains(candidate)))
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_MATERIAL_SLOTS",
                    "voxel scene renderer material slot space is exhausted",
                )
            })?;
        occupied.insert(renderer_slot);
        mappings.insert(source_slot, renderer_slot);
    }
    Ok(mappings)
}

pub(crate) fn api(
    bridge: &mut RuntimeVoxelScenePresentationBridge,
    appearance: &mut RuntimeAppearanceBridge,
) -> NativeVoxelScenePresentationApi {
    bridge.bind_appearance(appearance);
    NativeVoxelScenePresentationApi {
        context: (bridge as *mut RuntimeVoxelScenePresentationBridge).cast(),
        project_scene,
        refresh_scene,
        update_scene,
        destroy_scene,
        clear,
        project_scene_directional,
        update_scene_directional,
        read_material_mapping,
        destroy_material_mapping_lease,
    }
}

unsafe extern "C" fn project_scene(
    context: *mut c_void,
    request: *const NativeProjectVoxelSceneRequest,
    output: *mut NativeVoxelScenePresentationHandle,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    let started = Instant::now();
    let result = bridge.project_scene(unsafe { *request });
    bridge.record_presentation_attribution(
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
    );
    match result {
        Ok(handle) => {
            unsafe { *output = handle };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn project_scene_directional(
    context: *mut c_void,
    request: *const NativeProjectVoxelSceneDirectionalRequest,
    output: *mut NativeVoxelScenePresentationHandle,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    let started = Instant::now();
    let result = bridge.project_scene_directional(unsafe { *request });
    bridge.record_presentation_attribution(
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
    );
    match result {
        Ok(handle) => {
            unsafe { *output = handle };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn refresh_scene(
    context: *mut c_void,
    handle: NativeVoxelScenePresentationHandle,
    output: *mut NativeVoxelScenePresentationReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    let started = Instant::now();
    let result = bridge.refresh(handle);
    let duration_us = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
    bridge.record_presentation_attribution(duration_us);
    match result {
        Ok(readout) => {
            unsafe { *output = readout };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn update_scene(
    context: *mut c_void,
    request: *const NativeUpdateVoxelScenePresentationRequest,
    output: *mut NativeVoxelScenePresentationReadout,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    let started = Instant::now();
    let result = bridge.update(unsafe { *request });
    bridge.record_presentation_attribution(
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
    );
    match result {
        Ok(readout) => {
            unsafe { *output = readout };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn update_scene_directional(
    context: *mut c_void,
    request: *const NativeUpdateVoxelScenePresentationDirectionalRequest,
    output: *mut NativeVoxelScenePresentationReadout,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    let started = Instant::now();
    let result = bridge.update_directional(unsafe { *request });
    bridge.record_presentation_attribution(
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
    );
    match result {
        Ok(readout) => {
            unsafe { *output = readout };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_material_mapping(
    context: *mut c_void,
    handle: NativeVoxelScenePresentationHandle,
    output: *mut NativeVoxelSceneMaterialMappingLease,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    match bridge.read_material_mapping(handle) {
        Ok(lease) => {
            unsafe { *output = lease };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_material_mapping_lease(
    context: *mut c_void,
    handle: NativeVoxelSceneMaterialMappingLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    bridge
        .destroy_material_mapping_lease(handle)
        .map_or(0, |_| ABI_OK)
}

unsafe extern "C" fn destroy_scene(
    context: *mut c_void,
    handle: NativeVoxelScenePresentationHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    bridge.destroy(handle).map_or(0, |_| ABI_OK)
}

unsafe extern "C" fn clear(
    context: *mut c_void,
    output: *mut NativeVoxelScenePresentationClearReceipt,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelScenePresentationBridge>() };
    match bridge.clear() {
        Ok(receipt) => {
            unsafe { *output = receipt };
            ABI_OK
        }
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{appearance::RuntimeAppearanceBridge, spatial::RuntimeSpatialBridge};
    use render_model::RenderDiff;
    use render_projection::RuntimeAppearanceCatalog;
    use std::sync::Arc;

    fn session_with_voxel(spatial: &mut RuntimeSpatialBridge) -> NativeSpatialSessionHandle {
        session_with_voxel_mode(spatial, NativeVoxelSurfaceMode::GreedyCubes)
    }

    fn session_with_voxel_mode(
        spatial: &mut RuntimeSpatialBridge,
        surface_mode: NativeVoxelSurfaceMode,
    ) -> NativeSpatialSessionHandle {
        let spatial_api = crate::spatial::api(spatial);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial_api.create_session)(
                    spatial_api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: surface_mode,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        let voxel_api = crate::voxel::api(spatial);
        let edits = [NativeVoxelEdit {
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 0, y: 0, z: 0 },
            material_slot: 1,
        }];
        let mut receipt = NativeVoxelEditReceipt::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
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
                    &mut receipt,
                    &mut error,
                )
            },
            ABI_OK
        );
        assert_eq!(receipt.changed_voxels, 1);
        session
    }

    fn material(appearance: &mut RuntimeAppearanceBridge) -> NativeMaterialHandle {
        material_with_color(
            appearance,
            NativeColor {
                r: 0.25,
                g: 0.5,
                b: 0.75,
                a: 1.0,
            },
        )
    }

    fn material_with_color(
        appearance: &mut RuntimeAppearanceBridge,
        color: NativeColor,
    ) -> NativeMaterialHandle {
        let mut handle = NativeMaterialHandle::default();
        assert_eq!(
            unsafe {
                crate::appearance::create_material(
                    (appearance as *mut RuntimeAppearanceBridge).cast(),
                    NativeMaterialRequest {
                        color,
                        texture: NativeRenderResourceHandle::default(),
                        roughness: 1.0,
                        texture_tint: NativeColor {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                        emission_color: NativeVec3::default(),
                        emission_intensity: 0.0,
                        double_sided: false,
                    },
                    &mut handle,
                )
            },
            ABI_OK
        );
        handle
    }

    #[test]
    fn projects_the_canonical_spatial_scene_incrementally_and_disposes_its_renderer_identity() {
        let mut spatial = RuntimeSpatialBridge::new();
        let session = session_with_voxel(&mut spatial);
        let mut bridge = RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());

        appearance.begin_call();
        bridge.begin_call();
        let material = material(&mut appearance);
        let api = super::api(&mut bridge, &mut appearance);
        let bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material,
        }];
        let mut presentation = NativeVoxelScenePresentationHandle::default();
        assert_eq!(
            unsafe {
                (api.project_scene)(
                    api.context,
                    &NativeProjectVoxelSceneRequest {
                        session,
                        materials: bindings.as_ptr(),
                        materials_len: bindings.len(),
                    },
                    &mut presentation,
                )
            },
            ABI_OK
        );
        let attribution = bridge.update_attribution();
        assert_eq!(attribution.voxel_scene_presentation_calls, 1);
        let staged = bridge
            .take_staged_call()
            .expect("staged voxel scene projection");
        assert!(staged.frames.iter().any(|frame| {
            frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::DefineMaterial { .. }))
                && frame
                    .ops
                    .iter()
                    .any(|operation| matches!(operation, RenderDiff::ReplaceMeshPayload { .. }))
        }));
        assert_eq!(
            staged
                .frames
                .iter()
                .flat_map(|frame| frame.ops.iter())
                .filter(|operation| matches!(operation, RenderDiff::DefineMaterial { .. }))
                .count(),
            1,
            "base-only projection retains one material definition"
        );
        bridge.commit_call(staged);
        let staged_appearance = appearance.take_staged_call().expect("staged material");
        appearance.commit(staged_appearance);

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        let mut readout = NativeVoxelScenePresentationReadout::default();
        assert_eq!(
            unsafe { (api.refresh_scene)(api.context, presentation, &mut readout) },
            ABI_OK
        );
        assert!(readout.present);
        assert_eq!(readout.material_count, 1);
        let staged = bridge
            .take_staged_call()
            .expect("staged incremental refresh");
        assert!(staged.frames.iter().all(|frame| frame.ops.is_empty()));
        bridge.commit_call(staged);
        let staged_appearance = appearance
            .take_staged_call()
            .expect("staged appearance refresh");
        appearance.commit(staged_appearance);

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        assert_eq!(
            unsafe { (api.destroy_scene)(api.context, presentation) },
            ABI_OK
        );
        let staged = bridge.take_staged_call().expect("staged disposal");
        assert!(staged.frames.iter().any(|frame| {
            frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::Destroy { .. }))
        }));
    }

    #[test]
    fn canonical_recovery_repairs_a_discarded_voxel_refresh_without_advancing_active_history() {
        let mut spatial = RuntimeSpatialBridge::new();
        let session = session_with_voxel(&mut spatial);
        let mut bridge = RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());

        appearance.begin_call();
        bridge.begin_call();
        let material = material(&mut appearance);
        let api = super::api(&mut bridge, &mut appearance);
        let bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material,
        }];
        let mut presentation = NativeVoxelScenePresentationHandle::default();
        assert_eq!(
            unsafe {
                (api.project_scene)(
                    api.context,
                    &NativeProjectVoxelSceneRequest {
                        session,
                        materials: bindings.as_ptr(),
                        materials_len: bindings.len(),
                    },
                    &mut presentation,
                )
            },
            ABI_OK
        );
        let initial = bridge.take_staged_call().expect("initial projection");
        bridge.commit_call(initial);
        let initial_appearance = appearance.take_staged_call().expect("initial material");
        appearance.commit(initial_appearance);
        assert!(
            bridge
                .recover_from_canonical()
                .expect("matching canonical scene")
                .is_empty(),
            "a recovery with no retained/canonical drift must not emit a frame"
        );

        // The accepted mutation changes canonical Spatial immediately. The
        // following RefreshScene is deliberately left staged, as it would be
        // when later C# work in the same callback fails.
        let clear = [NativeVoxelEdit {
            kind: NativeVoxelEditKind::Clear,
            address: NativeVoxelAddress { x: 0, y: 0, z: 0 },
            material_slot: 0,
        }];
        let mut receipt = NativeVoxelEditReceipt::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        let voxel_api = crate::voxel::api(&mut spatial);
        assert_eq!(
            unsafe {
                (voxel_api.apply_edits)(
                    voxel_api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        expected_revision: 1,
                        edits: clear.as_ptr(),
                        edits_len: clear.len(),
                    },
                    &mut receipt,
                    &mut error,
                )
            },
            ABI_OK
        );
        assert_eq!(receipt.accepted_revision, 2);
        assert_eq!(receipt.solid_voxel_count, 0);
        let before_refresh_failure = bridge
            .recover_from_canonical()
            .expect("canonical recovery before RefreshScene");
        assert!(before_refresh_failure[0]
            .ops
            .iter()
            .any(|operation| matches!(operation, RenderDiff::Destroy { .. })));

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        let mut readout = NativeVoxelScenePresentationReadout::default();
        assert_eq!(
            unsafe { (api.refresh_scene)(api.context, presentation, &mut readout) },
            ABI_OK
        );
        assert_eq!(readout.source_revision, receipt.accepted_revision);
        let failed_stage = bridge.take_staged_call().expect("staged refresh");
        assert!(failed_stage.frames.iter().any(|frame| {
            frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::Destroy { .. }))
        }));
        bridge.discard_call();
        appearance.discard_call();

        let recovered = bridge
            .recover_from_canonical()
            .expect("canonical voxel recovery");
        assert_eq!(recovered.len(), 1);
        assert!(recovered[0]
            .ops
            .iter()
            .any(|operation| matches!(operation, RenderDiff::Destroy { .. })));
        assert_eq!(
            bridge
                .recover_from_canonical()
                .expect("repeated canonical recovery"),
            before_refresh_failure,
            "recovery is detached and must not advance the active projector"
        );

        // No second canonical change occurred, but the active projector still
        // has the pre-failure history. An ordinary refresh therefore emits the
        // same repair rather than silently adopting recovery's detached state.
        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        assert_eq!(
            unsafe { (api.refresh_scene)(api.context, presentation, &mut readout) },
            ABI_OK
        );
        let ordinary = bridge.take_staged_call().expect("ordinary repair refresh");
        assert!(ordinary.frames[0]
            .ops
            .iter()
            .any(|operation| matches!(operation, RenderDiff::Destroy { .. })));
    }

    #[test]
    fn directional_materials_keep_complete_defaults_and_copy_effective_mapping() {
        let mut spatial = RuntimeSpatialBridge::new();
        let session = session_with_voxel(&mut spatial);
        let mut bridge = RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        appearance.begin_call();
        bridge.begin_call();
        let side = material(&mut appearance);
        let top = material_with_color(
            &mut appearance,
            NativeColor {
                r: 0.1,
                g: 0.9,
                b: 0.2,
                a: 1.0,
            },
        );
        let zero = material_with_color(
            &mut appearance,
            NativeColor {
                r: 0.8,
                g: 0.3,
                b: 0.1,
                a: 1.0,
            },
        );
        let api = super::api(&mut bridge, &mut appearance);
        let bases = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material: side,
        }];
        let overrides = [NativeVoxelSceneFaceMaterialBinding {
            material_slot: 1,
            face: NativeSpatialFace::PosY,
            material: top,
        }];
        let mut presentation = NativeVoxelScenePresentationHandle::default();
        assert_eq!(
            unsafe {
                (api.project_scene_directional)(
                    api.context,
                    &NativeProjectVoxelSceneDirectionalRequest {
                        session,
                        materials: bases.as_ptr(),
                        materials_len: bases.len(),
                        face_materials: overrides.as_ptr(),
                        face_materials_len: overrides.len(),
                    },
                    &mut presentation,
                )
            },
            ABI_OK
        );
        let directional_material_count = bridge
            .staged
            .as_ref()
            .unwrap()
            .frames
            .iter()
            .flat_map(|frame| frame.ops.iter())
            .filter(|operation| matches!(operation, RenderDiff::DefineMaterial { .. }))
            .count();
        assert_eq!(
            directional_material_count, 2,
            "one base material plus one +Y override are realized"
        );
        let mut mapping = NativeVoxelSceneMaterialMappingLease {
            handle: NativeVoxelSceneMaterialMappingLeaseHandle::default(),
            mappings: std::ptr::null(),
            mappings_len: 0,
            source_revision: 0,
            mesh_revision: 0,
        };
        assert_eq!(
            unsafe { (api.read_material_mapping)(api.context, presentation, &mut mapping) },
            ABI_OK
        );
        let rows = unsafe { std::slice::from_raw_parts(mapping.mappings, mapping.mappings_len) };
        assert_eq!(rows.len(), 6);
        let top_row = rows
            .iter()
            .find(|row| row.face == NativeSpatialFace::PosY)
            .unwrap();
        let side_row = rows
            .iter()
            .find(|row| row.face == NativeSpatialFace::PosX)
            .unwrap();
        assert!(top_row.overridden);
        assert!(!side_row.overridden);
        assert_eq!(top_row.material_value, top.value);
        assert_eq!(side_row.material_value, side.value);
        assert_ne!(top_row.renderer_slot, side_row.renderer_slot);
        assert_eq!(
            unsafe { (api.destroy_material_mapping_lease)(api.context, mapping.handle) },
            ABI_OK
        );

        // Reserve a retained +Y face slot that matches the preferred renderer
        // slot of a newly used, valid source slot. This exercises the update
        // allocator's base-before-face collision path directly.
        bridge
            .staged
            .as_mut()
            .unwrap()
            .state
            .presentations
            .get_mut(&presentation.value)
            .unwrap()
            .face_renderer_slots
            .insert((1, Direction6::PosY), 2);
        let voxel_api = crate::voxel::api(&mut spatial);
        let add_slot = [NativeVoxelEdit {
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 2, y: 0, z: 0 },
            material_slot: 2,
        }];
        let mut voxel_receipt = NativeVoxelEditReceipt::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (voxel_api.apply_edits)(
                    voxel_api.context,
                    &NativeVoxelEditTransaction {
                        session,
                        expected_revision: 1,
                        edits: add_slot.as_ptr(),
                        edits_len: add_slot.len(),
                    },
                    &mut voxel_receipt,
                    &mut error,
                )
            },
            ABI_OK
        );
        let updated_bases = [
            NativeVoxelSceneMaterialBinding {
                material_slot: 1,
                material: side,
            },
            NativeVoxelSceneMaterialBinding {
                material_slot: 2,
                material: zero,
            },
        ];
        let mut updated_readout = NativeVoxelScenePresentationReadout::default();
        assert_eq!(
            unsafe {
                (api.update_scene_directional)(
                    api.context,
                    &NativeUpdateVoxelScenePresentationDirectionalRequest {
                        presentation,
                        materials: updated_bases.as_ptr(),
                        materials_len: updated_bases.len(),
                        face_materials: overrides.as_ptr(),
                        face_materials_len: overrides.len(),
                    },
                    &mut updated_readout,
                )
            },
            ABI_OK
        );
        let mut updated_mapping = NativeVoxelSceneMaterialMappingLease {
            handle: NativeVoxelSceneMaterialMappingLeaseHandle::default(),
            mappings: std::ptr::null(),
            mappings_len: 0,
            source_revision: 0,
            mesh_revision: 0,
        };
        assert_eq!(
            unsafe { (api.read_material_mapping)(api.context, presentation, &mut updated_mapping) },
            ABI_OK
        );
        let updated_rows = unsafe {
            std::slice::from_raw_parts(updated_mapping.mappings, updated_mapping.mappings_len)
        };
        let retained_top = updated_rows
            .iter()
            .find(|row| row.source_slot == 1 && row.face == NativeSpatialFace::PosY)
            .unwrap();
        let new_base = updated_rows
            .iter()
            .find(|row| row.source_slot == 2 && row.face == NativeSpatialFace::PosX)
            .unwrap();
        assert_ne!(
            retained_top.renderer_slot, new_base.renderer_slot,
            "new base allocation must reserve the retained face override slot"
        );
        assert_eq!(new_base.material_value, zero.value);
        assert!(bridge
            .staged
            .as_ref()
            .unwrap()
            .frames
            .iter()
            .flat_map(|frame| frame.ops.iter())
            .any(|operation| matches!(
                operation,
                RenderDiff::DefineMaterial { material }
                    if material.id == voxel_material_id(u16::try_from(new_base.renderer_slot).unwrap())
            )));
        assert_eq!(
            unsafe { (api.destroy_material_mapping_lease)(api.context, updated_mapping.handle) },
            ABI_OK
        );

        // A duplicate face override is rejected before the retained mapping or
        // renderer payload can be replaced.
        let duplicate = [overrides[0], overrides[0]];
        let mut readout = NativeVoxelScenePresentationReadout::default();
        assert_eq!(
            unsafe {
                (api.update_scene_directional)(
                    api.context,
                    &NativeUpdateVoxelScenePresentationDirectionalRequest {
                        presentation,
                        materials: updated_bases.as_ptr(),
                        materials_len: updated_bases.len(),
                        face_materials: duplicate.as_ptr(),
                        face_materials_len: duplicate.len(),
                    },
                    &mut readout,
                )
            },
            0
        );
        let none = [NativeVoxelSceneFaceMaterialBinding {
            material_slot: 1,
            face: NativeSpatialFace::None,
            material: top,
        }];
        assert_eq!(
            unsafe {
                (api.update_scene_directional)(
                    api.context,
                    &NativeUpdateVoxelScenePresentationDirectionalRequest {
                        presentation,
                        materials: updated_bases.as_ptr(),
                        materials_len: updated_bases.len(),
                        face_materials: none.as_ptr(),
                        face_materials_len: none.len(),
                    },
                    &mut readout,
                )
            },
            0
        );
        let unknown = [NativeVoxelSceneFaceMaterialBinding {
            material_slot: 3,
            face: NativeSpatialFace::PosY,
            material: top,
        }];
        assert_eq!(
            unsafe {
                (api.update_scene_directional)(
                    api.context,
                    &NativeUpdateVoxelScenePresentationDirectionalRequest {
                        presentation,
                        materials: updated_bases.as_ptr(),
                        materials_len: updated_bases.len(),
                        face_materials: unknown.as_ptr(),
                        face_materials_len: unknown.len(),
                    },
                    &mut readout,
                )
            },
            0
        );
        let mut after = NativeVoxelSceneMaterialMappingLease {
            handle: NativeVoxelSceneMaterialMappingLeaseHandle::default(),
            mappings: std::ptr::null(),
            mappings_len: 0,
            source_revision: 0,
            mesh_revision: 0,
        };
        assert_eq!(
            unsafe { (api.read_material_mapping)(api.context, presentation, &mut after) },
            ABI_OK
        );
        let after_rows = unsafe { std::slice::from_raw_parts(after.mappings, after.mappings_len) };
        assert_eq!(
            after_rows
                .iter()
                .find(|row| row.face == NativeSpatialFace::PosY)
                .unwrap()
                .material_value,
            top.value
        );
        assert_eq!(
            unsafe { (api.destroy_material_mapping_lease)(api.context, after.handle) },
            ABI_OK
        );
    }

    #[test]
    fn reconstructed_groups_use_base_renderer_slots_and_reject_face_overrides() {
        let mut spatial = RuntimeSpatialBridge::new();
        let first_session =
            session_with_voxel_mode(&mut spatial, NativeVoxelSurfaceMode::MarchingCubes);
        let second_session =
            session_with_voxel_mode(&mut spatial, NativeVoxelSurfaceMode::MarchingCubes);
        let mut bridge = RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        appearance.begin_call();
        bridge.begin_call();
        let first_material = material(&mut appearance);
        let second_material = material_with_color(
            &mut appearance,
            NativeColor {
                r: 0.8,
                g: 0.1,
                b: 0.2,
                a: 1.0,
            },
        );
        let api = super::api(&mut bridge, &mut appearance);
        let first = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material: first_material,
        }];
        let second = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material: second_material,
        }];
        let mut first_presentation = NativeVoxelScenePresentationHandle::default();
        let mut second_presentation = NativeVoxelScenePresentationHandle::default();
        for (session, bindings, output) in [
            (first_session, &first[..], &mut first_presentation),
            (second_session, &second[..], &mut second_presentation),
        ] {
            assert_eq!(
                unsafe {
                    (api.project_scene)(
                        api.context,
                        &NativeProjectVoxelSceneRequest {
                            session,
                            materials: bindings.as_ptr(),
                            materials_len: bindings.len(),
                        },
                        output,
                    )
                },
                ABI_OK
            );
        }
        let staged = bridge
            .staged
            .as_ref()
            .expect("reconstructed projections are staged");
        let slots = staged
            .frames
            .iter()
            .flat_map(|frame| frame.ops.iter())
            .filter_map(|operation| match operation {
                RenderDiff::ReplaceMeshPayload { payload, .. } => {
                    payload.groups.first().map(|group| group.material_slot)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            slots,
            BTreeSet::from([0, 1]),
            "directionless groups retain their per-presentation base renderer remaps"
        );

        let face = [NativeVoxelSceneFaceMaterialBinding {
            material_slot: 1,
            face: NativeSpatialFace::PosY,
            material: first_material,
        }];
        let mut rejected = NativeVoxelScenePresentationHandle::default();
        assert_eq!(
            unsafe {
                (api.project_scene_directional)(
                    api.context,
                    &NativeProjectVoxelSceneDirectionalRequest {
                        session: first_session,
                        materials: first.as_ptr(),
                        materials_len: first.len(),
                        face_materials: face.as_ptr(),
                        face_materials_len: face.len(),
                    },
                    &mut rejected,
                )
            },
            0
        );
    }

    #[test]
    fn overlapping_presentations_keep_renderer_handles_and_material_slots_distinct() {
        let mut spatial = RuntimeSpatialBridge::new();
        let first_session = session_with_voxel(&mut spatial);
        let second_session = session_with_voxel(&mut spatial);
        let mut bridge = RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());

        appearance.begin_call();
        bridge.begin_call();
        let first_material = material_with_color(
            &mut appearance,
            NativeColor {
                r: 0.9,
                g: 0.1,
                b: 0.1,
                a: 1.0,
            },
        );
        let second_material = material_with_color(
            &mut appearance,
            NativeColor {
                r: 0.1,
                g: 0.9,
                b: 0.1,
                a: 1.0,
            },
        );
        let api = super::api(&mut bridge, &mut appearance);
        let first_bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material: first_material,
        }];
        let second_bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material: second_material,
        }];
        let mut first = NativeVoxelScenePresentationHandle::default();
        let mut second = NativeVoxelScenePresentationHandle::default();
        for (session, bindings, output) in [
            (first_session, &first_bindings[..], &mut first),
            (second_session, &second_bindings[..], &mut second),
        ] {
            assert_eq!(
                unsafe {
                    (api.project_scene)(
                        api.context,
                        &NativeProjectVoxelSceneRequest {
                            session,
                            materials: bindings.as_ptr(),
                            materials_len: bindings.len(),
                        },
                        output,
                    )
                },
                ABI_OK
            );
        }
        let staged = bridge.take_staged_call().expect("overlapping projections");
        let create_handles = staged
            .frames
            .iter()
            .flat_map(|frame| frame.ops.iter())
            .filter_map(|operation| match operation {
                RenderDiff::Create { handle, .. } => Some(*handle),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            create_handles.len(),
            4,
            "two roots and two chunks are distinct"
        );
        let material_ids = staged
            .frames
            .iter()
            .flat_map(|frame| frame.ops.iter())
            .filter_map(|operation| match operation {
                RenderDiff::DefineMaterial { material } => Some(material.id.clone()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            material_ids.len(),
            2,
            "base-only presentations retain one renderer material per source slot"
        );
        let payload_slots = staged
            .frames
            .iter()
            .flat_map(|frame| frame.ops.iter())
            .filter_map(|operation| match operation {
                RenderDiff::ReplaceMeshPayload { payload, .. } => {
                    payload.groups.first().map(|group| group.material_slot)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(payload_slots, BTreeSet::from([0, 1]));
        bridge.commit_call(staged);
        let staged_appearance = appearance.take_staged_call().expect("staged materials");
        appearance.commit(staged_appearance);

        let first_root = bridge
            .state
            .projector
            .root_handle(&presentation_instance_id(first.value))
            .expect("first root");
        let second_root = bridge
            .state
            .projector
            .root_handle(&presentation_instance_id(second.value))
            .expect("second root");

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        assert_eq!(unsafe { (api.destroy_scene)(api.context, first) }, ABI_OK);
        let staged = bridge.take_staged_call().expect("first cleanup");
        let destroyed = staged.frames[0]
            .ops
            .iter()
            .filter_map(|operation| match operation {
                RenderDiff::Destroy { handle } => Some(*handle),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert!(destroyed.contains(&first_root));
        assert!(!destroyed.contains(&second_root));
        bridge.commit_call(staged);
        let staged_appearance = appearance.take_staged_call().expect("appearance cleanup");
        appearance.commit(staged_appearance);

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        let mut readout = NativeVoxelScenePresentationReadout::default();
        assert_eq!(
            unsafe { (api.refresh_scene)(api.context, second, &mut readout) },
            ABI_OK
        );
        let staged = bridge.take_staged_call().expect("survivor refresh");
        assert!(staged.frames[0].ops.is_empty());
        bridge.commit_call(staged);
        let staged_appearance = appearance.take_staged_call().expect("appearance refresh");
        appearance.commit(staged_appearance);

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        assert_eq!(unsafe { (api.destroy_scene)(api.context, second) }, ABI_OK);
        let staged = bridge.take_staged_call().expect("second cleanup");
        assert!(staged.frames[0]
            .ops
            .iter()
            .any(|operation| matches!(operation, RenderDiff::Destroy { handle } if *handle == second_root)));
    }

    #[test]
    fn renderer_slot_exhaustion_is_an_explicit_admission_error() {
        assert!(allocate_renderer_slots([(1, 1)], 0..=u16::MAX).is_err());
    }

    #[test]
    fn fresh_attachment_rebases_voxel_projection_without_mutating_active_history() {
        let mut spatial = RuntimeSpatialBridge::new();
        let session = session_with_voxel(&mut spatial);
        let second_session = session_with_voxel(&mut spatial);
        let mut bridge = RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());

        appearance.begin_call();
        bridge.begin_call();
        let material = material(&mut appearance);
        let second_material = material_with_color(
            &mut appearance,
            NativeColor {
                r: 0.8,
                g: 0.2,
                b: 0.1,
                a: 1.0,
            },
        );
        let api = super::api(&mut bridge, &mut appearance);
        let bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material,
        }];
        let mut presentation = NativeVoxelScenePresentationHandle::default();
        let mut second_presentation = NativeVoxelScenePresentationHandle::default();
        let top_override = [NativeVoxelSceneFaceMaterialBinding {
            material_slot: 1,
            face: NativeSpatialFace::PosY,
            material: second_material,
        }];
        assert_eq!(
            unsafe {
                (api.project_scene_directional)(
                    api.context,
                    &NativeProjectVoxelSceneDirectionalRequest {
                        session,
                        materials: bindings.as_ptr(),
                        materials_len: bindings.len(),
                        face_materials: top_override.as_ptr(),
                        face_materials_len: top_override.len(),
                    },
                    &mut presentation,
                )
            },
            ABI_OK
        );
        let second_bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material,
        }];
        assert_eq!(
            unsafe {
                (api.project_scene)(
                    api.context,
                    &NativeProjectVoxelSceneRequest {
                        session: second_session,
                        materials: second_bindings.as_ptr(),
                        materials_len: second_bindings.len(),
                    },
                    &mut second_presentation,
                )
            },
            ABI_OK
        );
        let initial_voxel = bridge.take_staged_call().expect("initial voxel projection");
        bridge.commit_call(initial_voxel);
        let initial_appearance = appearance
            .take_staged_call()
            .expect("initial material projection");
        appearance.commit(initial_appearance);

        let attach_frame = |bridge: &mut RuntimeVoxelScenePresentationBridge,
                            appearance: &mut RuntimeAppearanceBridge| {
            appearance.begin_attach_call();
            bridge.begin_attach_call();
            let api = super::api(bridge, appearance);
            let mut readout = NativeVoxelScenePresentationReadout::default();
            assert_eq!(
                unsafe { (api.refresh_scene)(api.context, presentation, &mut readout) },
                ABI_OK
            );
            assert!(readout.present);
            let staged = bridge.take_staged_call().expect("detached voxel baseline");
            assert_eq!(staged.frames.len(), 1);
            let frame = staged.frames.into_iter().next().expect("one voxel frame");
            assert!(frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::DefineMaterial { .. })));
            assert!(frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::Create { .. })));
            assert!(frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::ReplaceMeshPayload { .. })));
            assert_eq!(
                frame
                    .ops
                    .iter()
                    .filter(|operation| matches!(operation, RenderDiff::Create { .. }))
                    .count(),
                4,
                "the detached baseline contains both retained presentations"
            );
            assert_eq!(
                frame
                    .ops
                    .iter()
                    .filter(|operation| matches!(operation, RenderDiff::DefineMaterial { .. }))
                    .count(),
                3,
                "the detached baseline reproduces the retained directional override"
            );
            bridge.discard_call();
            appearance.discard_call();
            frame
        };

        let first_attach = attach_frame(&mut bridge, &mut appearance);

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        let mut readout = NativeVoxelScenePresentationReadout::default();
        assert_eq!(
            unsafe { (api.refresh_scene)(api.context, presentation, &mut readout) },
            ABI_OK
        );
        let active = bridge
            .take_staged_call()
            .expect("active incremental refresh");
        assert_eq!(active.frames.len(), 1);
        assert!(active.frames[0].ops.is_empty());
        bridge.discard_call();
        appearance.discard_call();

        let second_attach = attach_frame(&mut bridge, &mut appearance);
        assert_eq!(second_attach, first_attach);
    }

    #[test]
    fn open_texture_material_projects_through_voxel_scene_presentation() {
        // Keep this test independent from any product-specific rendering
        // fixture. The bridge only needs a valid PNG resource to exercise the
        // normal appearance/material path.
        const TEXTURE: &[u8] = &[
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 244, 34, 127, 138, 0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            0, 68, 255, 25, 26, 0, 16, 121, 3, 126, 153, 113, 48, 89, 0, 0, 0, 0, 73, 69, 78, 68,
            174, 66, 96, 130,
        ];
        let mut spatial = RuntimeSpatialBridge::new();
        let session = session_with_voxel(&mut spatial);
        let mut bridge = RuntimeVoxelScenePresentationBridge::new(spatial.collision_source());
        let mut resources = BTreeMap::new();
        resources.insert("surface.png".to_owned(), Arc::from(TEXTURE));
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), resources);

        appearance.begin_call();
        bridge.begin_call();
        let path = b"surface.png";
        let mut resource = NativeRenderResourceInfo::default();
        assert_eq!(
            unsafe {
                crate::appearance::open_render_resource(
                    (&mut appearance as *mut RuntimeAppearanceBridge).cast(),
                    &NativeRenderResourceRequest {
                        path: NativeUtf8Slice {
                            bytes: path.as_ptr(),
                            len: path.len(),
                        },
                    },
                    &mut resource,
                )
            },
            ABI_OK
        );
        let mut material = NativeMaterialHandle::default();
        assert_eq!(
            unsafe {
                crate::appearance::create_material(
                    (&mut appearance as *mut RuntimeAppearanceBridge).cast(),
                    NativeMaterialRequest {
                        color: NativeColor {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                        texture: resource.handle,
                        roughness: 1.0,
                        texture_tint: NativeColor {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                        emission_color: NativeVec3::default(),
                        emission_intensity: 0.0,
                        double_sided: false,
                    },
                    &mut material,
                )
            },
            ABI_OK
        );

        let api = super::api(&mut bridge, &mut appearance);
        let bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material,
        }];
        let mut presentation = NativeVoxelScenePresentationHandle::default();
        assert_eq!(
            unsafe {
                (api.project_scene)(
                    api.context,
                    &NativeProjectVoxelSceneRequest {
                        session,
                        materials: bindings.as_ptr(),
                        materials_len: bindings.len(),
                    },
                    &mut presentation,
                )
            },
            ABI_OK
        );
        let staged = bridge.take_staged_call().expect("textured projection");
        let frame = staged
            .frames
            .iter()
            .find(|frame| {
                frame
                    .ops
                    .iter()
                    .any(|operation| matches!(operation, RenderDiff::DefineTexture { .. }))
            })
            .expect("voxel scene frame defines the selected texture");
        let (texture_index, texture) = frame
            .ops
            .iter()
            .enumerate()
            .find_map(|(index, operation)| match operation {
                RenderDiff::DefineTexture { texture } => Some((index, texture)),
                _ => None,
            })
            .expect("matching texture descriptor");
        let (material_index, material_texture) = frame
            .ops
            .iter()
            .enumerate()
            .find_map(|(index, operation)| match operation {
                RenderDiff::DefineMaterial { material } => {
                    material.texture.as_deref().map(|texture| (index, texture))
                }
                _ => None,
            })
            .expect("textured material descriptor");
        let texture_id = texture.id.clone();
        assert_eq!(material_texture, texture_id);
        assert!(texture_index < material_index);
        let payload_resource = match texture.payload.as_ref().map(|payload| &payload.source) {
            Some(render_model::TexturePayloadSource::Resource { resource }) => resource.clone(),
            _ => panic!("texture descriptor did not retain a resource payload"),
        };
        assert!(payload_resource.starts_with("texture-resource/"));
        let staged_appearance = appearance.take_staged_call().expect("staged appearance");
        appearance.commit(staged_appearance);
        let selected = appearance
            .state
            .render_resources
            .first()
            .expect("selected texture resource");
        assert_eq!(selected.identity(), payload_resource);
        assert_eq!(selected.asset_identity(), texture_id);
    }
}
