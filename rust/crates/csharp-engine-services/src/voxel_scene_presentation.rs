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
use render_model::{RenderFrameDiff, RenderMaterialDescriptor, Transform};
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
    projector: VoxelRenderProjector,
}

#[derive(Debug, Clone, Default)]
struct VoxelScenePresentationState {
    presentations: BTreeMap<u64, RetainedVoxelScenePresentation>,
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
        let materials =
            self.materials_for_scene(&scene, request.materials, request.materials_len)?;
        let staged = self.staged_mut()?;
        let value = staged.state.next_presentation;
        staged.state.next_presentation = value.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                "voxel scene presentation handle space overflowed",
            )
        })?;
        staged.state.presentations.insert(
            value,
            RetainedVoxelScenePresentation {
                session: request.session,
                materials,
                projector: VoxelRenderProjector::new(),
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
            .get_mut(&handle.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                    "voxel scene presentation handle is not retained",
                )
            })?;
        let (frame, readout) = project_presentation(handle, presentation, &spatial)?;
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
        let materials =
            self.materials_for_scene(&scene, request.materials, request.materials_len)?;
        let staged = self.staged_mut()?;
        staged
            .state
            .presentations
            .get_mut(&request.presentation.value)
            .expect("presentation existence was checked before material resolution")
            .materials = materials;
        self.refresh(request.presentation)
    }

    fn destroy(
        &mut self,
        handle: NativeVoxelScenePresentationHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let spatial = self.spatial.clone();
        let staged = self.staged_mut()?;
        let mut presentation = staged
            .state
            .presentations
            .remove(&handle.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_SCENE_PRESENTATION_HANDLE",
                    "voxel scene presentation handle is not retained",
                )
            })?;
        let frame = clear_presentation(&mut presentation, &spatial)?;
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
        for presentation in staged.state.presentations.values_mut() {
            staged
                .frames
                .push(clear_presentation(presentation, &spatial)?);
        }
        staged.state.presentations.clear();
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
    ) -> Result<BTreeMap<u16, RenderMaterialDescriptor>, CsharpEngineServicesError> {
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
        slots
            .into_iter()
            .map(|(slot, material)| {
                let mut descriptor = appearance.voxel_material_descriptor(material)?;
                descriptor.id = voxel_material_id(slot);
                Ok((slot, descriptor))
            })
            .collect()
    }
}

fn project_presentation(
    handle: NativeVoxelScenePresentationHandle,
    presentation: &mut RetainedVoxelScenePresentation,
    spatial: &SpatialCollisionSource,
) -> Result<(RenderFrameDiff, NativeVoxelScenePresentationReadout), CsharpEngineServicesError> {
    let scene = spatial.scene(presentation.session)?;
    let instance_id = format!("csharp-voxel-scene-presentation-{}", handle.value);
    let asset_id = format!("spatial-session-{}", presentation.session.value);
    let result = presentation
        .projector
        .project(
            &[VoxelProjectionInstance {
                instance_id,
                asset_id,
                transform: Transform::IDENTITY,
                scene: &scene,
            }],
            &presentation.materials,
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_VOXEL_SCENE_PRESENTATION", format!("{error:?}"))
        })?;
    Ok((
        result.frame,
        NativeVoxelScenePresentationReadout {
            present: true,
            source_revision: scene.source_revision().raw(),
            mesh_revision: scene.projection_revisions().mesh().raw(),
            chunk_count: u64::try_from(result.readout.chunk_count).map_err(|_| {
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
        },
    ))
}

fn clear_presentation(
    presentation: &mut RetainedVoxelScenePresentation,
    _spatial: &SpatialCollisionSource,
) -> Result<RenderFrameDiff, CsharpEngineServicesError> {
    presentation
        .projector
        .project(&[], &BTreeMap::new())
        .map(|result| result.frame)
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_VOXEL_SCENE_PRESENTATION", format!("{error:?}"))
        })
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
                        reserved: 0,
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
        let mut handle = NativeMaterialHandle::default();
        assert_eq!(
            unsafe {
                crate::appearance::create_material(
                    (appearance as *mut RuntimeAppearanceBridge).cast(),
                    NativeMaterialRequest {
                        color: NativeColor {
                            r: 0.25,
                            g: 0.5,
                            b: 0.75,
                            a: 1.0,
                        },
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
}
