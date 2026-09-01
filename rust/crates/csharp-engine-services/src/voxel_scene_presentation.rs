//! Retained renderer projection for canonical Spatial voxel scenes.
//!
//! The product selects live Appearance materials and a Spatial session.  The
//! Engine resolves the current `VoxelCollisionScene`, projects it through the
//! normal incremental voxel projector, and stages renderer work.  No mesh or
//! renderer object is ever admitted from C#.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
};

use csharp_engine_abi::*;
use engine_spatial::VoxelCollisionScene;
use render_model::{
    RenderDiff, RenderFrameDiff, RenderMaterialDescriptor, TextureDescriptor, Transform,
};
use render_projection::{voxel_material_id, VoxelProjectionInstance, VoxelRenderProjector};

use crate::{
    appearance::RuntimeAppearanceBridge,
    composition::{borrowed_slice, ABI_OK},
    spatial::SpatialCollisionSource,
    CsharpEngineServicesError,
};

#[derive(Debug, Clone)]
struct RetainedVoxelScenePresentation {
    session: NativeSpatialSessionHandle,
    materials: BTreeMap<u16, RenderMaterialDescriptor>,
    textures: BTreeMap<String, TextureDescriptor>,
    renderer_slots: BTreeMap<u16, u16>,
}

#[derive(Debug, Clone, Default)]
struct VoxelScenePresentationState {
    presentations: BTreeMap<u64, RetainedVoxelScenePresentation>,
    projector: VoxelRenderProjector,
    next_presentation: u64,
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
}

impl RuntimeVoxelScenePresentationBridge {
    pub(crate) fn new(spatial: SpatialCollisionSource) -> Self {
        Self {
            spatial,
            state: VoxelScenePresentationState {
                presentations: BTreeMap::new(),
                projector: VoxelRenderProjector::new(),
                next_presentation: 1,
            },
            staged: None,
            appearance: None,
        }
    }

    pub(crate) fn bind_appearance(&mut self, appearance: &mut RuntimeAppearanceBridge) {
        // The sibling bridge is retained by EngineServiceSet.  This pointer is
        // refreshed while assembling the call table and used only during the
        // synchronous generated callback.
        self.appearance = Some(appearance as *mut RuntimeAppearanceBridge);
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
        let scene = self.spatial.scene(request.session)?;
        let (materials, textures) =
            self.materials_for_scene(&scene, request.materials, request.materials_len)?;
        let staged = self.staged_mut()?;
        let value = staged.state.next_presentation;
        staged.state.next_presentation = value.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                "voxel scene presentation handle space overflowed",
            )
        })?;
        let renderer_slots = allocate_renderer_slots(
            materials.keys().copied(),
            staged
                .state
                .presentations
                .values()
                .flat_map(|presentation| presentation.renderer_slots.values().copied()),
        )?;
        staged.state.presentations.insert(
            value,
            RetainedVoxelScenePresentation {
                session: request.session,
                materials,
                textures,
                renderer_slots,
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
        let (materials, textures) =
            self.materials_for_scene(&scene, request.materials, request.materials_len)?;
        let staged = self.staged_mut()?;
        let retained_slots = staged
            .state
            .presentations
            .get(&request.presentation.value)
            .expect("presentation existence was checked before material resolution")
            .renderer_slots
            .iter()
            .filter(|(source_slot, _)| materials.contains_key(source_slot))
            .map(|(source_slot, renderer_slot)| (*source_slot, *renderer_slot))
            .collect::<BTreeMap<_, _>>();
        let missing_slots = materials
            .keys()
            .copied()
            .filter(|slot| !retained_slots.contains_key(slot))
            .collect::<Vec<_>>();
        let occupied = staged
            .state
            .presentations
            .iter()
            .filter(|(handle, _)| **handle != request.presentation.value)
            .flat_map(|(_, presentation)| presentation.renderer_slots.values().copied())
            .chain(retained_slots.values().copied())
            .collect::<Vec<_>>();
        let new_slots = allocate_renderer_slots(missing_slots, occupied)?;
        let presentation = staged
            .state
            .presentations
            .get_mut(&request.presentation.value)
            .expect("presentation existence was checked before material resolution");
        presentation.materials = materials;
        presentation.textures = textures;
        presentation.renderer_slots = retained_slots;
        presentation.renderer_slots.extend(new_slots);
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
    ) -> Result<
        (
            BTreeMap<u16, RenderMaterialDescriptor>,
            BTreeMap<String, TextureDescriptor>,
        ),
        CsharpEngineServicesError,
    > {
        // SAFETY: generated C# pins this bounded typed array for the direct
        // callback. Resolved descriptors are copied before returning.
        let bindings = unsafe { borrowed_slice(pointer, len, "voxel scene material bindings")? };
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
        let appearance = self.appearance_mut()?;
        let resolved = slots
            .into_iter()
            .map(|(slot, material)| {
                let (descriptor, texture) = appearance.voxel_material_projection(material)?;
                Ok((slot, descriptor, texture))
            })
            .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?;
        let materials = resolved
            .iter()
            .map(|(slot, material, _)| (*slot, material.clone()))
            .collect();
        let textures = resolved
            .into_iter()
            .filter_map(|(_, _, texture)| texture.map(|texture| (texture.id.clone(), texture)))
            .collect();
        Ok((materials, textures))
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
                presentation.renderer_slots.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let materials = renderer_materials(&state.presentations)?;
    let textures = presentation_textures(&state.presentations);
    let result = state
        .projector
        .project_mapped(&instances, &materials, &material_slots)
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
        material_count: u32::try_from(presentation.materials.len()).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION",
                "voxel scene material count exceeded the C# receipt range",
            )
        })?,
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
                .materials
                .iter()
                .map(|(source_slot, descriptor)| {
                    (
                        presentation.renderer_slots.get(source_slot).copied(),
                        descriptor,
                    )
                })
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

fn allocate_renderer_slots(
    source_slots: impl IntoIterator<Item = u16>,
    occupied_slots: impl IntoIterator<Item = u16>,
) -> Result<BTreeMap<u16, u16>, CsharpEngineServicesError> {
    let mut occupied = occupied_slots.into_iter().collect::<BTreeSet<_>>();
    let mut mappings = BTreeMap::new();
    for source_slot in source_slots {
        if mappings.contains_key(&source_slot) {
            continue;
        }
        let renderer_slot = (!occupied.contains(&source_slot))
            .then_some(source_slot)
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
    match bridge.project_scene(unsafe { *request }) {
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
    match bridge.refresh(handle) {
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
    match bridge.update(unsafe { *request }) {
        Ok(readout) => {
            unsafe { *output = readout };
            ABI_OK
        }
        Err(_) => 0,
    }
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
        let spatial_api = crate::spatial::api(spatial);
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (spatial_api.create_session)(
                    spatial_api.context,
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
        let voxel_api = crate::voxel::api(spatial);
        let edits = [NativeVoxelEdit {
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 0, y: 0, z: 0 },
            material_slot: 1,
        }];
        let mut receipt = NativeVoxelEditReceipt::default();
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
            "same source slot is remapped per presentation"
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
        assert!(allocate_renderer_slots([1], 0..=u16::MAX).is_err());
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
        let second_bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material: second_material,
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
                2,
                "the detached baseline preserves the disjoint material slots"
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
