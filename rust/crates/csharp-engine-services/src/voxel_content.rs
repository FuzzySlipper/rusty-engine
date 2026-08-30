//! Retained typed voxel artifacts behind the generated NativeAOT table.
//!
//! The bridge delegates validation and bounded runtime admission to the
//! existing voxel artifact owners. An explicit asset-to-Spatial operation
//! feeds a retained asset into the canonical scene; presentation and material
//! resource realization remain separate Engine-owned services.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    sync::Arc,
};

use csharp_engine_abi::*;
use render_model::{RenderFrameDiff, RenderMaterialDescriptor, RenderMetadata, Transform};
use render_projection::{VoxelObjectProjectionInstance, VoxelObjectRenderProjector};
use voxel_annotation::{
    decode_annotation_layer, query_annotation_layer, validate_annotation_layer,
    VoxelAnnotationBounds, VoxelAnnotationEditCommand, VoxelAnnotationEditService,
    VoxelAnnotationEditTransaction, VoxelAnnotationKind, VoxelAnnotationLayer,
    VoxelAnnotationLimits, VoxelAnnotationQuery, VoxelAnnotationQueryMode,
    VoxelAnnotationRegionReadout,
};
use voxel_asset::{decode_voxel_asset, represented_voxel_count, VoxelAsset, VoxelFrame};
use voxel_object_runtime::{
    admit_voxel_object_json, AdmittedVoxelObject, VoxelObjectLoopMode, VoxelObjectPlaybackPosture,
    VoxelObjectPlaybackRate, VoxelObjectPlaybackReadout, VoxelObjectPlaybackStatus,
    VoxelObjectPlayer,
};

use crate::{
    appearance::RuntimeAppearanceBridge,
    composition::{borrowed_slice, borrowed_utf8, ABI_OK},
    spatial::{RuntimeSpatialBridge, VoxelAssetSpatialPublishFacts},
    CsharpEngineServicesError,
};

#[derive(Debug, Clone, Copy)]
struct ObjectFrameSelection {
    runtime_frame: u32,
    clip_index: u32,
    clip_frame_index: u32,
    is_default: bool,
}

#[derive(Debug)]
struct RetainedVoxelObject {
    object: Arc<AdmittedVoxelObject>,
    selected: ObjectFrameSelection,
    selection_revision: u64,
}

/// The player owns an Arc to the admitted object. Destroying the object's
/// product handle only removes direct object access; it cannot invalidate a
/// still-retained player.
#[derive(Debug)]
struct RetainedVoxelObjectPlayer {
    object: Arc<AdmittedVoxelObject>,
    player: VoxelObjectPlayer,
}

#[derive(Debug, Clone)]
struct RetainedVoxelObjectPresentation {
    object: Arc<AdmittedVoxelObject>,
    runtime_frame: u32,
    transform: Transform,
    visible: bool,
    materials: BTreeMap<String, RenderMaterialDescriptor>,
}

#[derive(Debug, Clone)]
struct VoxelObjectPresentationState {
    projector: VoxelObjectRenderProjector,
    presentations: BTreeMap<u64, RetainedVoxelObjectPresentation>,
    next_presentation: u64,
}

#[derive(Debug)]
pub(crate) struct RuntimeVoxelContentCall {
    state: VoxelObjectPresentationState,
    pub(crate) frames: Vec<RenderFrameDiff>,
}

#[derive(Debug)]
struct RetainedVoxelAnnotation {
    /// Retaining this Arc makes the annotation's admitted target independent of
    /// the product's direct asset-handle lifetime.
    _asset: Arc<VoxelAsset>,
    layer: VoxelAnnotationLayer,
    revision: u64,
}

#[derive(Debug)]
struct RetainedVoxelAnnotationRegion {
    region_id: String,
    label: String,
    kind: NativeVoxelAnnotationKind,
    parent_region_id: Option<String>,
    bounds: NativeVoxelAnnotationBounds,
    assigned_cell_count: u64,
}

#[derive(Debug)]
struct RetainedVoxelAnnotationRegionLease {
    _regions: Vec<RetainedVoxelAnnotationRegion>,
    _readout: Vec<NativeVoxelAnnotationRegionReadout>,
}

#[derive(Debug)]
struct RetainedVoxelAnnotationEditLease {
    _ids: Vec<String>,
    _readout: Vec<NativeVoxelAnnotationAffectedId>,
}

#[derive(Debug)]
struct RetainedVoxelAssetSpatialPaletteRow {
    material_asset_id: String,
    display_name: String,
}

#[derive(Debug)]
struct RetainedVoxelAssetSpatialPublishLease {
    _palette: Vec<RetainedVoxelAssetSpatialPaletteRow>,
    _readout: Vec<NativeVoxelAssetSpatialPaletteRow>,
}

pub(crate) struct RuntimeVoxelContentBridge {
    assets: BTreeMap<u64, Arc<VoxelAsset>>,
    objects: BTreeMap<u64, RetainedVoxelObject>,
    players: BTreeMap<u64, RetainedVoxelObjectPlayer>,
    annotations: BTreeMap<u64, RetainedVoxelAnnotation>,
    annotation_region_leases: BTreeMap<u64, RetainedVoxelAnnotationRegionLease>,
    annotation_edit_leases: BTreeMap<u64, RetainedVoxelAnnotationEditLease>,
    asset_spatial_publish_leases: BTreeMap<u64, RetainedVoxelAssetSpatialPublishLease>,
    next_asset: u64,
    next_object: u64,
    next_player: u64,
    next_annotation: u64,
    next_annotation_region_lease: u64,
    next_annotation_edit_lease: u64,
    next_asset_spatial_publish_lease: u64,
    presentation: VoxelObjectPresentationState,
    staged_presentation: Option<RuntimeVoxelContentCall>,
    appearance: Option<*mut RuntimeAppearanceBridge>,
    spatial: Option<*mut RuntimeSpatialBridge>,
}

impl RuntimeVoxelContentBridge {
    pub(crate) fn new() -> Self {
        Self {
            assets: BTreeMap::new(),
            objects: BTreeMap::new(),
            players: BTreeMap::new(),
            annotations: BTreeMap::new(),
            annotation_region_leases: BTreeMap::new(),
            annotation_edit_leases: BTreeMap::new(),
            asset_spatial_publish_leases: BTreeMap::new(),
            next_asset: 1,
            next_object: 1,
            next_player: 1,
            next_annotation: 1,
            next_annotation_region_lease: 1,
            next_annotation_edit_lease: 1,
            next_asset_spatial_publish_lease: 1,
            presentation: VoxelObjectPresentationState {
                projector: VoxelObjectRenderProjector::new(),
                presentations: BTreeMap::new(),
                next_presentation: 1,
            },
            staged_presentation: None,
            appearance: None,
            spatial: None,
        }
    }

    pub(crate) fn bind_spatial(&mut self, spatial: &mut RuntimeSpatialBridge) {
        self.spatial = Some(spatial as *mut RuntimeSpatialBridge);
    }

    fn insert_asset(&mut self, asset: VoxelAsset) -> Option<NativeVoxelAssetHandle> {
        let value = self.next_asset;
        self.next_asset = value.checked_add(1)?;
        self.assets.insert(value, Arc::new(asset));
        Some(NativeVoxelAssetHandle { value })
    }

    fn insert_object(&mut self, object: AdmittedVoxelObject) -> Option<NativeVoxelObjectHandle> {
        let value = self.next_object;
        self.next_object = value.checked_add(1)?;
        self.objects.insert(
            value,
            RetainedVoxelObject {
                object: Arc::new(object),
                selected: ObjectFrameSelection {
                    runtime_frame: 0,
                    clip_index: u32::MAX,
                    clip_frame_index: 0,
                    is_default: true,
                },
                selection_revision: 0,
            },
        );
        Some(NativeVoxelObjectHandle { value })
    }

    fn insert_player(
        &mut self,
        object: Arc<AdmittedVoxelObject>,
    ) -> Option<NativeVoxelObjectPlayerHandle> {
        let value = self.next_player;
        self.next_player = value.checked_add(1)?;
        self.players.insert(
            value,
            RetainedVoxelObjectPlayer {
                object,
                player: VoxelObjectPlayer::new(),
            },
        );
        Some(NativeVoxelObjectPlayerHandle { value })
    }

    fn create_player(
        &mut self,
        object_handle: NativeVoxelObjectHandle,
    ) -> Result<NativeVoxelObjectPlayerHandle, CsharpEngineServicesError> {
        let object = Arc::clone(&self.object(object_handle)?.object);
        self.insert_player(object).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_CONTENT_PLAYER",
                "voxel object player handle space overflowed",
            )
        })
    }

    /// Internal retained-owner resolution for the later annotation bridge.
    /// This intentionally has no ABI callback: annotations remain a separate
    /// capability and cannot turn an asset handle into scene mutation.
    pub(crate) fn resolve_asset(
        &self,
        handle: NativeVoxelAssetHandle,
    ) -> Result<&VoxelAsset, CsharpEngineServicesError> {
        self.assets
            .get(&handle.value)
            .map(Arc::as_ref)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_CONTENT_ASSET",
                    "voxel asset handle is not admitted",
                )
            })
    }

    fn asset_arc(
        &self,
        handle: NativeVoxelAssetHandle,
    ) -> Result<Arc<VoxelAsset>, CsharpEngineServicesError> {
        self.assets.get(&handle.value).cloned().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_CONTENT_ASSET",
                "voxel asset handle is not admitted",
            )
        })
    }

    fn prepare_asset_spatial_publish_lease(
        &self,
        value: u64,
        asset: &VoxelAsset,
        facts: VoxelAssetSpatialPublishFacts,
        voxel_data_hash: NativeVoxelContentHash,
        content_hash: NativeVoxelContentHash,
    ) -> (
        NativeVoxelAssetSpatialPublishLease,
        RetainedVoxelAssetSpatialPublishLease,
    ) {
        let palette = asset
            .material_palette
            .iter()
            .map(|binding| RetainedVoxelAssetSpatialPaletteRow {
                material_asset_id: binding.material_asset_id.clone(),
                display_name: binding.display_name.clone().unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        let readout = palette
            .iter()
            .zip(&asset.material_palette)
            .map(|(row, binding)| NativeVoxelAssetSpatialPaletteRow {
                material_slot: u32::from(binding.material_slot),
                material_asset_id: native_utf8(&row.material_asset_id),
                display_name: native_utf8(&row.display_name),
            })
            .collect::<Vec<_>>();
        let lease = NativeVoxelAssetSpatialPublishLease {
            handle: NativeVoxelAssetSpatialPublishLeaseHandle { value },
            palette: readout.as_ptr(),
            palette_len: readout.len(),
            revision_before: facts.revision_before,
            revision_after: facts.revision_after,
            voxel_size: facts.voxel_size,
            chunk_size: facts.chunk_size,
            solid_voxel_count: facts.solid_voxel_count,
            resident_chunk_count: facts.resident_chunk_count,
            authority_hash: facts.authority_hash,
            projection_version: facts.projection_version,
            collision_revision: facts.collision_revision,
            navigation_revision: facts.navigation_revision,
            mesh_revision: facts.mesh_revision,
            navigation_cell_count: facts.navigation_cell_count,
            voxel_data_hash,
            content_hash,
        };
        (
            lease,
            RetainedVoxelAssetSpatialPublishLease {
                _palette: palette,
                _readout: readout,
            },
        )
    }

    fn insert_annotation(
        &mut self,
        asset: Arc<VoxelAsset>,
        layer: VoxelAnnotationLayer,
    ) -> Option<NativeVoxelAnnotationHandle> {
        let value = self.next_annotation;
        self.next_annotation = value.checked_add(1)?;
        self.annotations.insert(
            value,
            RetainedVoxelAnnotation {
                _asset: asset,
                layer,
                revision: 0,
            },
        );
        Some(NativeVoxelAnnotationHandle { value })
    }

    fn annotation(
        &self,
        handle: NativeVoxelAnnotationHandle,
    ) -> Result<&RetainedVoxelAnnotation, CsharpEngineServicesError> {
        self.annotations.get(&handle.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_ANNOTATION",
                "voxel annotation handle is not admitted",
            )
        })
    }

    fn annotation_mut(
        &mut self,
        handle: NativeVoxelAnnotationHandle,
    ) -> Result<&mut RetainedVoxelAnnotation, CsharpEngineServicesError> {
        self.annotations.get_mut(&handle.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_ANNOTATION",
                "voxel annotation handle is not admitted",
            )
        })
    }

    fn insert_region_lease(
        &mut self,
        regions: Vec<VoxelAnnotationRegionReadout>,
        total_layer_regions: usize,
        truncated: bool,
        revision: u64,
        layer_hash: NativeVoxelContentHash,
    ) -> Result<NativeVoxelAnnotationRegionLease, CsharpEngineServicesError> {
        let value = self.next_annotation_region_lease;
        self.next_annotation_region_lease = value.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_ANNOTATION_LEASE",
                "annotation region lease handle space overflowed",
            )
        })?;
        let retained: Vec<_> = regions
            .into_iter()
            .map(|region| RetainedVoxelAnnotationRegion {
                region_id: region.region_id,
                label: region.label,
                kind: native_annotation_kind(region.kind),
                parent_region_id: region.parent_region_id,
                bounds: native_annotation_bounds(region.bounds),
                assigned_cell_count: region.assigned_cell_count,
            })
            .collect();
        let readout = retained
            .iter()
            .map(|region| NativeVoxelAnnotationRegionReadout {
                region_id: native_utf8(&region.region_id),
                label: native_utf8(&region.label),
                kind: region.kind,
                has_parent_region_id: region.parent_region_id.is_some(),
                parent_region_id: region
                    .parent_region_id
                    .as_deref()
                    .map(native_utf8)
                    .unwrap_or(NativeUtf8Slice {
                        bytes: std::ptr::null(),
                        len: 0,
                    }),
                bounds: region.bounds,
                assigned_cell_count: region.assigned_cell_count,
            })
            .collect::<Vec<_>>();
        let lease = NativeVoxelAnnotationRegionLease {
            handle: NativeVoxelAnnotationRegionLeaseHandle { value },
            regions: readout.as_ptr(),
            regions_len: readout.len(),
            total_layer_regions: narrow(total_layer_regions)?,
            truncated,
            revision,
            layer_hash,
        };
        self.annotation_region_leases.insert(
            value,
            RetainedVoxelAnnotationRegionLease {
                _regions: retained,
                _readout: readout,
            },
        );
        Ok(lease)
    }

    fn prepare_edit_lease(
        value: u64,
        receipt: voxel_annotation::VoxelAnnotationEditReceipt,
        revision: u64,
    ) -> Result<
        (
            NativeVoxelAnnotationEditLease,
            RetainedVoxelAnnotationEditLease,
        ),
        CsharpEngineServicesError,
    > {
        let ids = receipt.affected_region_ids;
        let readout = ids
            .iter()
            .map(|region_id| NativeVoxelAnnotationAffectedId {
                region_id: native_utf8(region_id),
            })
            .collect::<Vec<_>>();
        let lease = NativeVoxelAnnotationEditLease {
            handle: NativeVoxelAnnotationEditLeaseHandle { value },
            affected_ids: readout.as_ptr(),
            affected_ids_len: readout.len(),
            layer_hash_before: hash(&receipt.layer_hash_before)?,
            layer_hash_after: hash(&receipt.layer_hash_after)?,
            membership_hash_before: hash(&receipt.membership_hash_before)?,
            membership_hash_after: hash(&receipt.membership_hash_after)?,
            revision,
            command_count: narrow(receipt.command_count)?,
            region_count: narrow(receipt.region_count)?,
            assigned_cell_count: receipt.assigned_cell_count,
        };
        Ok((
            lease,
            RetainedVoxelAnnotationEditLease {
                _ids: ids,
                _readout: readout,
            },
        ))
    }

    fn apply_annotation_edit(
        &mut self,
        handle: NativeVoxelAnnotationHandle,
        expected_layer_hash: String,
        command: VoxelAnnotationEditCommand,
    ) -> Result<NativeVoxelAnnotationEditLease, CsharpEngineServicesError> {
        let (next_revision, mut candidate) = {
            let annotation = self.annotation(handle)?;
            (
                annotation.revision.checked_add(1).ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_VOXEL_ANNOTATION_REVISION",
                        "annotation revision overflowed",
                    )
                })?,
                annotation.layer.clone(),
            )
        };
        let next_lease = self.next_annotation_edit_lease;
        let next_lease_after = next_lease.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_ANNOTATION_LEASE",
                "annotation edit lease handle space overflowed",
            )
        })?;
        let transaction = VoxelAnnotationEditTransaction {
            expected_layer_hash,
            commands: vec![command],
        };
        // Preflight the owner transaction and construct every fallible ABI
        // receipt value before mutating the retained layer. This preserves the
        // owner's all-or-nothing guarantee through lease construction too.
        let candidate_receipt =
            VoxelAnnotationEditService::apply(&mut candidate, transaction.clone()).map_err(
                |_| {
                    CsharpEngineServicesError::new(
                        "CSHARP_VOXEL_ANNOTATION_EDIT",
                        "voxel annotation metadata edit was rejected",
                    )
                },
            )?;
        let (lease, retained_lease) =
            Self::prepare_edit_lease(next_lease, candidate_receipt, next_revision)?;
        let receipt =
            VoxelAnnotationEditService::apply(&mut self.annotation_mut(handle)?.layer, transaction)
                .map_err(|_| {
                    CsharpEngineServicesError::new(
                        "CSHARP_VOXEL_ANNOTATION_EDIT",
                        "annotation metadata changed during its atomic commit",
                    )
                })?;
        debug_assert_eq!(
            lease.layer_hash_after,
            hash(&receipt.layer_hash_after).unwrap_or_default()
        );
        let annotation = self.annotation_mut(handle)?;
        annotation.revision = next_revision;
        self.next_annotation_edit_lease = next_lease_after;
        self.annotation_edit_leases
            .insert(next_lease, retained_lease);
        Ok(lease)
    }

    fn select_default(
        &mut self,
        handle: NativeVoxelObjectHandle,
    ) -> Result<NativeVoxelObjectFrameReadout, CsharpEngineServicesError> {
        let retained = self.object_mut(handle)?;
        retained.selected = ObjectFrameSelection {
            runtime_frame: 0,
            clip_index: u32::MAX,
            clip_frame_index: 0,
            is_default: true,
        };
        retained.selection_revision =
            retained.selection_revision.checked_add(1).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_CONTENT_SELECTION",
                    "selection revision overflowed",
                )
            })?;
        native_frame_readout(&retained.object, retained.selected)
    }

    fn select_clip_frame(
        &mut self,
        handle: NativeVoxelObjectHandle,
        clip_id: &str,
        frame_index: u32,
    ) -> Result<NativeVoxelObjectFrameReadout, CsharpEngineServicesError> {
        let retained = self.object_mut(handle)?;
        let clip_index = retained
            .object
            .clips()
            .iter()
            .position(|clip| clip.id == clip_id)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_CONTENT_CLIP",
                    "voxel object clip is not admitted",
                )
            })?;
        let clip = &retained.object.clips()[clip_index];
        let runtime_frame = *clip
            .frame_indices
            .get(frame_index as usize)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_CONTENT_FRAME",
                    "voxel object clip frame is not admitted",
                )
            })?;
        retained.selected = ObjectFrameSelection {
            runtime_frame,
            clip_index: narrow(clip_index)?,
            clip_frame_index: frame_index,
            is_default: false,
        };
        retained.selection_revision =
            retained.selection_revision.checked_add(1).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_CONTENT_SELECTION",
                    "selection revision overflowed",
                )
            })?;
        native_frame_readout(&retained.object, retained.selected)
    }

    fn object(
        &self,
        handle: NativeVoxelObjectHandle,
    ) -> Result<&RetainedVoxelObject, CsharpEngineServicesError> {
        self.objects.get(&handle.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_CONTENT_OBJECT",
                "voxel object handle is not admitted",
            )
        })
    }

    fn object_mut(
        &mut self,
        handle: NativeVoxelObjectHandle,
    ) -> Result<&mut RetainedVoxelObject, CsharpEngineServicesError> {
        self.objects.get_mut(&handle.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_CONTENT_OBJECT",
                "voxel object handle is not admitted",
            )
        })
    }

    fn player(
        &self,
        handle: NativeVoxelObjectPlayerHandle,
    ) -> Result<&RetainedVoxelObjectPlayer, CsharpEngineServicesError> {
        self.players.get(&handle.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_CONTENT_PLAYER",
                "voxel object player handle is not retained",
            )
        })
    }

    fn player_mut(
        &mut self,
        handle: NativeVoxelObjectPlayerHandle,
    ) -> Result<&mut RetainedVoxelObjectPlayer, CsharpEngineServicesError> {
        self.players.get_mut(&handle.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_CONTENT_PLAYER",
                "voxel object player handle is not retained",
            )
        })
    }

    pub(crate) fn bind_appearance(&mut self, appearance: &mut RuntimeAppearanceBridge) {
        // This raw pointer is only used synchronously by a generated callback
        // while `EngineServiceSet` retains both bridges. It is refreshed every
        // time the Engine table is assembled rather than stored by C#.
        self.appearance = Some(appearance as *mut RuntimeAppearanceBridge);
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged_presentation = Some(RuntimeVoxelContentCall {
            state: self.presentation.clone(),
            frames: Vec::new(),
        });
    }

    pub(crate) fn discard_call(&mut self) {
        self.staged_presentation = None;
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeVoxelContentCall, CsharpEngineServicesError> {
        self.staged_presentation.take().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_CALL",
                "voxel presentation was read outside a product call",
            )
        })
    }

    pub(crate) fn commit_call(&mut self, call: RuntimeVoxelContentCall) {
        self.presentation = call.state;
    }

    fn staged_presentation_mut(
        &mut self,
    ) -> Result<&mut RuntimeVoxelContentCall, CsharpEngineServicesError> {
        self.staged_presentation.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_CALL",
                "voxel presentation was called outside a product call",
            )
        })
    }

    fn appearance_mut(
        &mut self,
    ) -> Result<&mut RuntimeAppearanceBridge, CsharpEngineServicesError> {
        let appearance = self.appearance.ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_APPEARANCE",
                "voxel presentation has no Engine appearance owner",
            )
        })?;
        // SAFETY: `bind_appearance` receives the sibling bridge retained by
        // `EngineServiceSet`; callbacks are synchronous and never retain it.
        Ok(unsafe { &mut *appearance })
    }

    fn project_object(
        &mut self,
        request: NativeProjectVoxelObjectRequest,
    ) -> Result<NativeVoxelObjectPresentationHandle, CsharpEngineServicesError> {
        let object = Arc::clone(&self.object(request.object)?.object);
        let presentation = RetainedVoxelObjectPresentation {
            object,
            runtime_frame: request.runtime_frame,
            transform: presentation_transform(request.transform)?,
            visible: request.visible,
            materials: self.presentation_materials(
                request.materials,
                request.materials_len,
                request.object,
            )?,
        };
        let next = self
            .staged_presentation
            .as_ref()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_PRESENTATION_CALL",
                    "voxel presentation was called outside a product call",
                )
            })?
            .state
            .next_presentation;
        let next_after = next.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_HANDLE",
                "voxel presentation handle space overflowed",
            )
        })?;
        let mut candidate = self.staged_presentation_state()?.clone();
        candidate.next_presentation = next_after;
        candidate.presentations.insert(next, presentation);
        let frame = project_presentation_state(&mut candidate)?;
        self.commit_presentation_operation(candidate, frame)?;
        Ok(NativeVoxelObjectPresentationHandle { value: next })
    }

    fn update_object_presentation(
        &mut self,
        request: NativeUpdateVoxelObjectPresentationRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let materials = self.presentation_materials_for_object(
            request.materials,
            request.materials_len,
            request.presentation,
        )?;
        let transform = presentation_transform(request.transform)?;
        let mut candidate = self.staged_presentation_state()?.clone();
        let presentation = candidate
            .presentations
            .get_mut(&request.presentation.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_PRESENTATION_HANDLE",
                    "voxel presentation handle is not retained",
                )
            })?;
        if presentation.object.frame(request.runtime_frame).is_none() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_FRAME",
                "voxel presentation frame is not admitted by its retained object",
            ));
        }
        presentation.runtime_frame = request.runtime_frame;
        presentation.transform = transform;
        presentation.visible = request.visible;
        presentation.materials = materials;
        let frame = project_presentation_state(&mut candidate)?;
        self.commit_presentation_operation(candidate, frame)
    }

    fn destroy_object_presentation(
        &mut self,
        handle: NativeVoxelObjectPresentationHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let mut candidate = self.staged_presentation_state()?.clone();
        let removed = candidate.presentations.remove(&handle.value);
        if removed.is_none() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_HANDLE",
                "voxel presentation handle is not retained",
            ));
        }
        let frame = project_presentation_state(&mut candidate)?;
        self.commit_presentation_operation(candidate, frame)
    }

    fn presentation_materials(
        &mut self,
        pointer: *const NativeVoxelObjectMaterialBinding,
        len: usize,
        object: NativeVoxelObjectHandle,
    ) -> Result<BTreeMap<String, RenderMaterialDescriptor>, CsharpEngineServicesError> {
        let object = Arc::clone(&self.object(object)?.object);
        self.presentation_materials_for(&object, pointer, len)
    }

    fn presentation_materials_for_object(
        &mut self,
        pointer: *const NativeVoxelObjectMaterialBinding,
        len: usize,
        presentation: NativeVoxelObjectPresentationHandle,
    ) -> Result<BTreeMap<String, RenderMaterialDescriptor>, CsharpEngineServicesError> {
        let object = Arc::clone(
            &self
                .staged_presentation
                .as_ref()
                .and_then(|call| call.state.presentations.get(&presentation.value))
                .ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_VOXEL_PRESENTATION_HANDLE",
                        "voxel presentation handle is not retained",
                    )
                })?
                .object,
        );
        self.presentation_materials_for(&object, pointer, len)
    }

    fn presentation_materials_for(
        &mut self,
        object: &AdmittedVoxelObject,
        pointer: *const NativeVoxelObjectMaterialBinding,
        len: usize,
    ) -> Result<BTreeMap<String, RenderMaterialDescriptor>, CsharpEngineServicesError> {
        // SAFETY: generated C# pins this bounded typed array for the direct
        // callback. Every descriptor below is copied before return.
        let bindings = unsafe { borrowed_slice(pointer, len, "voxel-object material bindings")? };
        let expected = object
            .source()
            .material_palette
            .iter()
            .map(|binding| binding.material_slot)
            .collect::<BTreeSet<_>>();
        let actual = bindings
            .iter()
            .map(|binding| u16::try_from(binding.material_slot))
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_PRESENTATION_MATERIALS",
                    "voxel presentation material slot exceeded the admitted object slot range",
                )
            })?;
        if actual.len() != bindings.len() || actual != expected {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_VOXEL_PRESENTATION_MATERIALS",
                "voxel presentation requires exactly one live material binding for every object palette slot",
            ));
        }
        let by_slot = bindings
            .iter()
            .map(|binding| {
                u16::try_from(binding.material_slot)
                    .map(|slot| (slot, binding.material))
                    .map_err(|_| {
                        CsharpEngineServicesError::new(
                            "CSHARP_VOXEL_PRESENTATION_MATERIALS",
                            "voxel presentation material slot exceeded the admitted object slot range",
                        )
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let appearance = self.appearance_mut()?;
        object
            .source()
            .material_palette
            .iter()
            .map(|palette| {
                let material = by_slot[&palette.material_slot];
                let mut descriptor = appearance.voxel_material_descriptor(material)?;
                descriptor.id = palette.material_asset_id.clone();
                Ok((descriptor.id.clone(), descriptor))
            })
            .collect()
    }

    fn staged_presentation_state(
        &self,
    ) -> Result<&VoxelObjectPresentationState, CsharpEngineServicesError> {
        self.staged_presentation
            .as_ref()
            .map(|call| &call.state)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_PRESENTATION_CALL",
                    "voxel presentation was called outside a product call",
                )
            })
    }

    fn commit_presentation_operation(
        &mut self,
        state: VoxelObjectPresentationState,
        frame: RenderFrameDiff,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_presentation_mut()?;
        staged.state = state;
        staged.frames.push(frame);
        Ok(())
    }
}

fn project_presentation_state(
    state: &mut VoxelObjectPresentationState,
) -> Result<RenderFrameDiff, CsharpEngineServicesError> {
    let instances = state
        .presentations
        .iter()
        .map(|(handle, presentation)| VoxelObjectProjectionInstance {
            instance_id: format!("csharp-voxel-presentation-{handle}"),
            object: presentation.object.as_ref(),
            frame: presentation.runtime_frame,
            transform: presentation.transform,
            visible: presentation.visible,
            material_overrides: Vec::new(),
            metadata: RenderMetadata {
                source_entity: None,
                source_scene_node: None,
                tags: vec!["csharp-voxel-object".to_owned()],
                label: Some(format!("Voxel object presentation {handle}")),
            },
        })
        .collect::<Vec<_>>();
    let mut materials = BTreeMap::new();
    for presentation in state.presentations.values() {
        for (id, descriptor) in &presentation.materials {
            if let Some(existing) = materials.insert(id.clone(), descriptor.clone()) {
                if existing != *descriptor {
                    return Err(CsharpEngineServicesError::new(
                        "CSHARP_VOXEL_PRESENTATION_MATERIALS",
                        "one retained voxel material identity resolved to conflicting descriptors",
                    ));
                }
            }
        }
    }
    let result = state
        .projector
        .project(&instances, &materials)
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_VOXEL_PRESENTATION", format!("{error:?}"))
        })?;
    Ok(result.frame)
}

fn presentation_transform(value: NativeTransform) -> Result<Transform, CsharpEngineServicesError> {
    let transform = Transform {
        translation: [
            value.translation.x,
            value.translation.y,
            value.translation.z,
        ],
        rotation: [
            value.rotation.x,
            value.rotation.y,
            value.rotation.z,
            value.rotation.w,
        ],
        scale: [value.scale.x, value.scale.y, value.scale.z],
    };
    transform.validate().map_err(|error| {
        CsharpEngineServicesError::new("CSHARP_VOXEL_PRESENTATION_TRANSFORM", format!("{error:?}"))
    })?;
    Ok(transform)
}

#[cfg(test)]
pub(crate) fn api(
    bridge: &mut RuntimeVoxelContentBridge,
    appearance: &mut RuntimeAppearanceBridge,
) -> NativeVoxelContentApi {
    bridge.bind_appearance(appearance);
    api_impl(bridge)
}

pub(crate) fn api_with_spatial(
    bridge: &mut RuntimeVoxelContentBridge,
    appearance: &mut RuntimeAppearanceBridge,
    spatial: &mut RuntimeSpatialBridge,
) -> NativeVoxelContentApi {
    bridge.bind_appearance(appearance);
    bridge.bind_spatial(spatial);
    api_impl(bridge)
}

fn api_impl(bridge: &mut RuntimeVoxelContentBridge) -> NativeVoxelContentApi {
    NativeVoxelContentApi {
        context: (bridge as *mut RuntimeVoxelContentBridge).cast(),
        admit_asset,
        destroy_asset,
        read_asset,
        publish_asset_to_spatial,
        destroy_asset_spatial_publish_lease,
        admit_object,
        destroy_object,
        read_object,
        select_default_object_frame,
        select_object_clip_frame,
        read_selected_object_frame,
        create_object_player,
        destroy_object_player,
        play_object_player,
        scrub_object_player,
        pause_object_player,
        resume_object_player,
        stop_object_player,
        read_object_player,
        sample_object_player,
        project_object,
        update_object_presentation,
        destroy_object_presentation,
        admit_annotation,
        destroy_annotation,
        query_annotation,
        destroy_annotation_region_lease,
        set_annotation_label,
        set_annotation_kind,
        set_annotation_parent,
        set_annotation_bounds,
        set_annotation_tags,
        destroy_annotation_edit_lease,
    }
}

unsafe extern "C" fn admit_asset(
    context: *mut c_void,
    request: *const NativeAdmitVoxelAssetRequest,
    output: *mut NativeVoxelAssetHandle,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let body = match unsafe { borrowed_json(request.bytes) } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let asset = match decode_voxel_asset(body) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.insert_asset(asset) {
        Some(handle) => {
            unsafe { *output = handle };
            ABI_OK
        }
        None => 0,
    }
}

unsafe extern "C" fn destroy_asset(context: *mut c_void, handle: NativeVoxelAssetHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    if bridge.assets.remove(&handle.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn read_asset(
    context: *mut c_void,
    handle: NativeVoxelAssetHandle,
    output: *mut NativeVoxelAssetReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let asset = match bridge.resolve_asset(handle) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    match native_asset_readout(asset) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn publish_asset_to_spatial(
    context: *mut c_void,
    request: *const NativePublishVoxelAssetToSpatialRequest,
    output: *mut NativeVoxelAssetSpatialPublishLease,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let next_lease = match bridge.next_asset_spatial_publish_lease.checked_add(1) {
        Some(value) => value,
        None => return 0,
    };
    let asset = match bridge.asset_arc(request.asset) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let voxel_data_hash = match hash(&asset.voxel_data_hash) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let content_hash = match hash(&asset.content_hash) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let spatial = match bridge.spatial {
        Some(value) if !value.is_null() => unsafe { &mut *value },
        _ => return 0,
    };
    let prepared = match spatial.prepare_voxel_asset(request.session, &asset) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let facts = prepared.facts();
    let (lease, retained) = bridge.prepare_asset_spatial_publish_lease(
        bridge.next_asset_spatial_publish_lease,
        &asset,
        facts,
        voxel_data_hash,
        content_hash,
    );
    if spatial.commit_voxel_asset(prepared).is_err() {
        return 0;
    }
    bridge.next_asset_spatial_publish_lease = next_lease;
    let replaced = bridge
        .asset_spatial_publish_leases
        .insert(lease.handle.value, retained);
    debug_assert!(replaced.is_none());
    unsafe { *output = lease };
    ABI_OK
}

unsafe extern "C" fn destroy_asset_spatial_publish_lease(
    context: *mut c_void,
    handle: NativeVoxelAssetSpatialPublishLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    if bridge
        .asset_spatial_publish_leases
        .remove(&handle.value)
        .is_some()
    {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn admit_object(
    context: *mut c_void,
    request: *const NativeAdmitVoxelObjectRequest,
    output: *mut NativeVoxelObjectHandle,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let body = match unsafe { borrowed_json(request.bytes) } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let object = match admit_voxel_object_json(body, Default::default()) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.insert_object(object) {
        Some(handle) => {
            unsafe { *output = handle };
            ABI_OK
        }
        None => 0,
    }
}

unsafe extern "C" fn admit_annotation(
    context: *mut c_void,
    request: *const NativeAdmitVoxelAnnotationRequest,
    output: *mut NativeVoxelAnnotationHandle,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let body = match unsafe { borrowed_json(request.bytes) } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let layer = match decode_annotation_layer(body) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let asset = match bridge.asset_arc(request.asset) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    if validate_annotation_layer(&layer, Some(&asset), Default::default()).is_err() {
        return 0;
    }
    match bridge.insert_annotation(asset, layer) {
        Some(handle) => {
            unsafe { *output = handle };
            ABI_OK
        }
        None => 0,
    }
}

unsafe extern "C" fn destroy_annotation(
    context: *mut c_void,
    handle: NativeVoxelAnnotationHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    if bridge.annotations.remove(&handle.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn query_annotation(
    context: *mut c_void,
    request: *const NativeVoxelAnnotationQueryRequest,
    output: *mut NativeVoxelAnnotationRegionLease,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let expected_layer_hash = if request.has_expected_layer_hash {
        Some(hash_string(request.expected_layer_hash))
    } else {
        None
    };
    let mode = match annotation_query_mode(request) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let (readout, revision) = {
        let annotation = match bridge.annotation(request.annotation) {
            Ok(value) => value,
            Err(_) => return 0,
        };
        let readout = match query_annotation_layer(
            &annotation.layer,
            &VoxelAnnotationQuery {
                expected_layer_hash,
                mode,
                max_results: request.max_results as usize,
            },
        ) {
            Ok(value) => value,
            Err(_) => return 0,
        };
        (readout, annotation.revision)
    };
    let layer_hash = match hash(&readout.layer_hash) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    match bridge.insert_region_lease(
        readout.matched_regions,
        readout.total_layer_regions,
        readout.truncated,
        revision,
        layer_hash,
    ) {
        Ok(lease) => {
            unsafe { *output = lease };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_annotation_region_lease(
    context: *mut c_void,
    handle: NativeVoxelAnnotationRegionLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    i32::from(
        bridge
            .annotation_region_leases
            .remove(&handle.value)
            .is_some(),
    )
}

unsafe extern "C" fn set_annotation_label(
    context: *mut c_void,
    request: *const NativeSetVoxelAnnotationLabelRequest,
    output: *mut NativeVoxelAnnotationEditLease,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let expected = hash_string(request.expected_layer_hash);
    let region_id = match unsafe {
        borrowed_utf8(
            request.region_id.bytes,
            request.region_id.len,
            "annotation region id",
        )
    } {
        Ok(value) => value.to_owned(),
        Err(_) => return 0,
    };
    let label = match unsafe {
        borrowed_utf8(request.label.bytes, request.label.len, "annotation label")
    } {
        Ok(value) => value.to_owned(),
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.apply_annotation_edit(
        request.annotation,
        expected,
        VoxelAnnotationEditCommand::SetLabel { region_id, label },
    ) {
        Ok(lease) => {
            unsafe { *output = lease };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn set_annotation_kind(
    context: *mut c_void,
    request: *const NativeSetVoxelAnnotationKindRequest,
    output: *mut NativeVoxelAnnotationEditLease,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let expected = hash_string(request.expected_layer_hash);
    let region_id = match unsafe {
        borrowed_utf8(
            request.region_id.bytes,
            request.region_id.len,
            "annotation region id",
        )
    } {
        Ok(value) => value.to_owned(),
        Err(_) => return 0,
    };
    let kind = match annotation_kind(request.kind) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.apply_annotation_edit(
        request.annotation,
        expected,
        VoxelAnnotationEditCommand::SetKind {
            region_id,
            annotation_kind: kind,
        },
    ) {
        Ok(lease) => {
            unsafe { *output = lease };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn set_annotation_parent(
    context: *mut c_void,
    request: *const NativeSetVoxelAnnotationParentRequest,
    output: *mut NativeVoxelAnnotationEditLease,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let expected = hash_string(request.expected_layer_hash);
    let region_id = match unsafe {
        borrowed_utf8(
            request.region_id.bytes,
            request.region_id.len,
            "annotation region id",
        )
    } {
        Ok(value) => value.to_owned(),
        Err(_) => return 0,
    };
    let parent_region_id = if request.has_parent_region_id {
        match unsafe {
            borrowed_utf8(
                request.parent_region_id.bytes,
                request.parent_region_id.len,
                "annotation parent region id",
            )
        } {
            Ok(value) => Some(value.to_owned()),
            Err(_) => return 0,
        }
    } else {
        None
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.apply_annotation_edit(
        request.annotation,
        expected,
        VoxelAnnotationEditCommand::SetParent {
            region_id,
            parent_region_id,
        },
    ) {
        Ok(lease) => {
            unsafe { *output = lease };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn set_annotation_bounds(
    context: *mut c_void,
    request: *const NativeSetVoxelAnnotationBoundsRequest,
    output: *mut NativeVoxelAnnotationEditLease,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let expected = hash_string(request.expected_layer_hash);
    let region_id = match unsafe {
        borrowed_utf8(
            request.region_id.bytes,
            request.region_id.len,
            "annotation region id",
        )
    } {
        Ok(value) => value.to_owned(),
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.apply_annotation_edit(
        request.annotation,
        expected,
        VoxelAnnotationEditCommand::SetBounds {
            region_id,
            bounds: annotation_bounds(request.bounds),
        },
    ) {
        Ok(lease) => {
            unsafe { *output = lease };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn set_annotation_tags(
    context: *mut c_void,
    request: *const NativeSetVoxelAnnotationTagsRequest,
    output: *mut NativeVoxelAnnotationEditLease,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let expected = hash_string(request.expected_layer_hash);
    let region_id = match unsafe {
        borrowed_utf8(
            request.region_id.bytes,
            request.region_id.len,
            "annotation region id",
        )
    } {
        Ok(value) => value.to_owned(),
        Err(_) => return 0,
    };
    let tags = match unsafe { borrowed_annotation_tags(request.tags, request.tags_len) } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.apply_annotation_edit(
        request.annotation,
        expected,
        VoxelAnnotationEditCommand::SetTags { region_id, tags },
    ) {
        Ok(lease) => {
            unsafe { *output = lease };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_annotation_edit_lease(
    context: *mut c_void,
    handle: NativeVoxelAnnotationEditLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    i32::from(
        bridge
            .annotation_edit_leases
            .remove(&handle.value)
            .is_some(),
    )
}

unsafe extern "C" fn destroy_object(context: *mut c_void, handle: NativeVoxelObjectHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    if bridge.objects.remove(&handle.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn read_object(
    context: *mut c_void,
    handle: NativeVoxelObjectHandle,
    output: *mut NativeVoxelObjectReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let retained = match bridge.object(handle) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    match native_object_readout(retained) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn select_default_object_frame(
    context: *mut c_void,
    handle: NativeVoxelObjectHandle,
    output: *mut NativeVoxelObjectFrameReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.select_default(handle) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn select_object_clip_frame(
    context: *mut c_void,
    request: *const NativeSelectVoxelObjectClipFrameRequest,
    output: *mut NativeVoxelObjectFrameReadout,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let clip =
        match unsafe { borrowed_utf8(request.clip.bytes, request.clip.len, "voxel object clip") } {
            Ok(value) => value,
            Err(_) => return 0,
        };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.select_clip_frame(request.object_handle, clip, request.frame_index) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_selected_object_frame(
    context: *mut c_void,
    handle: NativeVoxelObjectHandle,
    output: *mut NativeVoxelObjectFrameReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let retained = match bridge.object(handle) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    match native_frame_readout(&retained.object, retained.selected) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn create_object_player(
    context: *mut c_void,
    object_handle: NativeVoxelObjectHandle,
    output: *mut NativeVoxelObjectPlayerHandle,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.create_player(object_handle) {
        Ok(handle) => {
            unsafe { *output = handle };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_object_player(
    context: *mut c_void,
    handle: NativeVoxelObjectPlayerHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    if bridge.players.remove(&handle.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn play_object_player(
    context: *mut c_void,
    request: *const NativePlayVoxelObjectPlayerRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let clip = match unsafe {
        borrowed_utf8(
            request.clip.bytes,
            request.clip.len,
            "voxel object player clip",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let loop_mode = loop_mode(request.loop_mode);
    let rate = match VoxelObjectPlaybackRate::new(request.rate_numerator, request.rate_denominator)
    {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let retained = match bridge.player_mut(request.player_handle) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    if retained
        .player
        .play(&retained.object, clip, loop_mode, rate, request.now_micros)
        .is_ok()
    {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn scrub_object_player(
    context: *mut c_void,
    request: *const NativeScrubVoxelObjectPlayerRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let clip = match unsafe {
        borrowed_utf8(
            request.clip.bytes,
            request.clip.len,
            "voxel object player clip",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let loop_mode = loop_mode(request.loop_mode);
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let retained = match bridge.player_mut(request.player_handle) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    if retained
        .player
        .scrub(&retained.object, clip, request.clip_frame, loop_mode)
        .is_ok()
    {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn pause_object_player(
    context: *mut c_void,
    request: NativeVoxelObjectPlayerTimeRequest,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.player_mut(request.player_handle) {
        Ok(retained) => {
            if retained.player.pause(request.now_micros).is_ok() {
                ABI_OK
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn resume_object_player(
    context: *mut c_void,
    request: NativeVoxelObjectPlayerTimeRequest,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.player_mut(request.player_handle) {
        Ok(retained) => {
            if retained.player.resume(request.now_micros).is_ok() {
                ABI_OK
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn stop_object_player(
    context: *mut c_void,
    handle: NativeVoxelObjectPlayerHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.player_mut(handle) {
        Ok(retained) => {
            retained.player.stop();
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_object_player(
    context: *mut c_void,
    request: NativeVoxelObjectPlayerTimeRequest,
    output: *mut NativeVoxelObjectPlayerReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let retained = match bridge.player(request.player_handle) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    match retained.player.posture_at(request.now_micros) {
        Ok(posture) => {
            unsafe { *output = native_player_readout(&posture) };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn sample_object_player(
    context: *mut c_void,
    request: NativeVoxelObjectPlayerTimeRequest,
    output: *mut NativeVoxelObjectPlayerSampleReadout,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    let retained = match bridge.player(request.player_handle) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    match retained
        .player
        .sample_at(&retained.object, request.now_micros)
    {
        Ok(sample) => {
            unsafe { *output = native_player_sample_readout(sample) };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn project_object(
    context: *mut c_void,
    request: *const NativeProjectVoxelObjectRequest,
    output: *mut NativeVoxelObjectPresentationHandle,
) -> i32 {
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    match bridge.project_object(unsafe { *request }) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn update_object_presentation(
    context: *mut c_void,
    request: *const NativeUpdateVoxelObjectPresentationRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    if bridge
        .update_object_presentation(unsafe { *request })
        .is_ok()
    {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn destroy_object_presentation(
    context: *mut c_void,
    handle: NativeVoxelObjectPresentationHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVoxelContentBridge>() };
    if bridge.destroy_object_presentation(handle).is_ok() {
        ABI_OK
    } else {
        0
    }
}

unsafe fn borrowed_json<'a>(value: NativeByteSlice) -> Result<&'a str, ()> {
    if value.len > 0 && value.bytes.is_null() {
        return Err(());
    }
    let bytes = if value.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(value.bytes, value.len) }
    };
    std::str::from_utf8(bytes).map_err(|_| ())
}

/// Copies a bounded, synchronous tag input before an edit can reach the
/// retained annotation owner. No product pointer is preserved after return.
unsafe fn borrowed_annotation_tags(
    tags: *const NativeVoxelAnnotationTag,
    tags_len: usize,
) -> Result<Vec<String>, ()> {
    let limits = VoxelAnnotationLimits::default();
    if tags_len > limits.max_tags_per_region {
        return Err(());
    }
    if tags_len == 0 {
        return Ok(Vec::new());
    }
    if tags.is_null() {
        return Err(());
    }
    unsafe { std::slice::from_raw_parts(tags, tags_len) }
        .iter()
        .map(|tag| {
            unsafe { borrowed_utf8(tag.value.bytes, tag.value.len, "annotation tag") }
                .map(str::to_owned)
                .map_err(|_| ())
        })
        .collect()
}

fn native_utf8(value: &str) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_ptr(),
        len: value.len(),
    }
}

fn native_annotation_bounds(value: VoxelAnnotationBounds) -> NativeVoxelAnnotationBounds {
    NativeVoxelAnnotationBounds {
        min_x: value.min[0],
        min_y: value.min[1],
        min_z: value.min[2],
        max_x: value.max[0],
        max_y: value.max[1],
        max_z: value.max[2],
    }
}

fn annotation_bounds(value: NativeVoxelAnnotationBounds) -> VoxelAnnotationBounds {
    VoxelAnnotationBounds {
        min: [value.min_x, value.min_y, value.min_z],
        max: [value.max_x, value.max_y, value.max_z],
    }
}

fn native_annotation_kind(value: VoxelAnnotationKind) -> NativeVoxelAnnotationKind {
    match value {
        VoxelAnnotationKind::Selection => NativeVoxelAnnotationKind::Selection,
        VoxelAnnotationKind::Room => NativeVoxelAnnotationKind::Room,
        VoxelAnnotationKind::Portal => NativeVoxelAnnotationKind::Portal,
        VoxelAnnotationKind::SpawnArea => NativeVoxelAnnotationKind::SpawnArea,
        VoxelAnnotationKind::Cover => NativeVoxelAnnotationKind::Cover,
        VoxelAnnotationKind::Hazard => NativeVoxelAnnotationKind::Hazard,
        VoxelAnnotationKind::NavigationHint => NativeVoxelAnnotationKind::NavigationHint,
        VoxelAnnotationKind::Custom => NativeVoxelAnnotationKind::Custom,
    }
}

fn annotation_kind(
    value: NativeVoxelAnnotationKind,
) -> Result<VoxelAnnotationKind, CsharpEngineServicesError> {
    Ok(match value {
        NativeVoxelAnnotationKind::Selection => VoxelAnnotationKind::Selection,
        NativeVoxelAnnotationKind::Room => VoxelAnnotationKind::Room,
        NativeVoxelAnnotationKind::Portal => VoxelAnnotationKind::Portal,
        NativeVoxelAnnotationKind::SpawnArea => VoxelAnnotationKind::SpawnArea,
        NativeVoxelAnnotationKind::Cover => VoxelAnnotationKind::Cover,
        NativeVoxelAnnotationKind::Hazard => VoxelAnnotationKind::Hazard,
        NativeVoxelAnnotationKind::NavigationHint => VoxelAnnotationKind::NavigationHint,
        NativeVoxelAnnotationKind::Custom => VoxelAnnotationKind::Custom,
    })
}

fn annotation_query_mode(
    request: &NativeVoxelAnnotationQueryRequest,
) -> Result<VoxelAnnotationQueryMode, CsharpEngineServicesError> {
    Ok(match request.mode {
        NativeVoxelAnnotationQueryMode::Cell => VoxelAnnotationQueryMode::Cell {
            coordinate: [
                request.coordinate_x,
                request.coordinate_y,
                request.coordinate_z,
            ],
        },
        NativeVoxelAnnotationQueryMode::Bounds => VoxelAnnotationQueryMode::Bounds {
            bounds: annotation_bounds(request.bounds),
        },
        NativeVoxelAnnotationQueryMode::Region => VoxelAnnotationQueryMode::Region {
            region_id: unsafe {
                borrowed_utf8(
                    request.region_id.bytes,
                    request.region_id.len,
                    "annotation region id",
                )
            }
            .map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_VOXEL_ANNOTATION_QUERY",
                    "annotation region id was not valid UTF-8",
                )
            })?
            .to_owned(),
        },
        NativeVoxelAnnotationQueryMode::LayerSummary => VoxelAnnotationQueryMode::LayerSummary,
    })
}

fn native_asset_readout(
    asset: &VoxelAsset,
) -> Result<NativeVoxelAssetReadout, CsharpEngineServicesError> {
    Ok(NativeVoxelAssetReadout {
        schema_version: asset.schema_version,
        represented_voxel_count: represented_voxel_count(&VoxelFrame::from(asset)) as u64,
        material_palette_count: narrow(asset.material_palette.len())?,
        material_mapping_count: narrow(asset.material_map.len())?,
        voxel_data_hash: hash(&asset.voxel_data_hash)?,
        content_hash: hash(&asset.content_hash)?,
    })
}

fn native_object_readout(
    retained: &RetainedVoxelObject,
) -> Result<NativeVoxelObjectReadout, CsharpEngineServicesError> {
    let source = retained.object.source();
    Ok(NativeVoxelObjectReadout {
        schema_version: source.schema_version,
        frame_count: narrow(retained.object.frames().len())?,
        clip_count: narrow(retained.object.clips().len())?,
        material_palette_count: narrow(source.material_palette.len())?,
        material_mapping_count: narrow(source.material_map.len())?,
        default_runtime_frame: 0,
        selected_runtime_frame: retained.selected.runtime_frame,
        selection_revision: retained.selection_revision,
        content_hash: hash(retained.object.content_hash())?,
    })
}

fn native_frame_readout(
    object: &AdmittedVoxelObject,
    selected: ObjectFrameSelection,
) -> Result<NativeVoxelObjectFrameReadout, CsharpEngineServicesError> {
    let frame = object.frame(selected.runtime_frame).ok_or_else(|| {
        CsharpEngineServicesError::new(
            "CSHARP_VOXEL_CONTENT_FRAME",
            "selected voxel object frame is not admitted",
        )
    })?;
    Ok(NativeVoxelObjectFrameReadout {
        runtime_frame: selected.runtime_frame,
        clip_index: selected.clip_index,
        clip_frame_index: selected.clip_frame_index,
        is_default: selected.is_default,
        cell_count: frame.cells.len() as u64,
        anchor_count: narrow(frame.anchors.len())?,
        has_collision: frame.collision.is_some(),
        voxel_data_hash: hash(&frame.voxel_data_hash)?,
    })
}

fn loop_mode(value: NativeVoxelObjectLoopMode) -> VoxelObjectLoopMode {
    match value {
        NativeVoxelObjectLoopMode::Once => VoxelObjectLoopMode::Once,
        NativeVoxelObjectLoopMode::Repeat => VoxelObjectLoopMode::Repeat,
        NativeVoxelObjectLoopMode::PingPong => VoxelObjectLoopMode::PingPong,
    }
}

fn native_loop_mode(value: VoxelObjectLoopMode) -> NativeVoxelObjectLoopMode {
    match value {
        VoxelObjectLoopMode::Once => NativeVoxelObjectLoopMode::Once,
        VoxelObjectLoopMode::Repeat => NativeVoxelObjectLoopMode::Repeat,
        VoxelObjectLoopMode::PingPong => NativeVoxelObjectLoopMode::PingPong,
    }
}

fn native_status(value: VoxelObjectPlaybackStatus) -> NativeVoxelObjectPlaybackStatus {
    match value {
        VoxelObjectPlaybackStatus::Stopped => NativeVoxelObjectPlaybackStatus::Stopped,
        VoxelObjectPlaybackStatus::Playing => NativeVoxelObjectPlaybackStatus::Playing,
        VoxelObjectPlaybackStatus::Paused => NativeVoxelObjectPlaybackStatus::Paused,
    }
}

fn native_player_readout(posture: &VoxelObjectPlaybackPosture) -> NativeVoxelObjectPlayerReadout {
    NativeVoxelObjectPlayerReadout {
        status: native_status(posture.status),
        loop_mode: native_loop_mode(posture.loop_mode),
        rate_numerator: posture.rate.numerator,
        rate_denominator: posture.rate.denominator,
        elapsed_micros: posture.elapsed_micros,
    }
}

fn native_player_sample_readout(
    sample: VoxelObjectPlaybackReadout<'_>,
) -> NativeVoxelObjectPlayerSampleReadout {
    NativeVoxelObjectPlayerSampleReadout {
        status: native_status(sample.status),
        loop_mode: native_loop_mode(sample.loop_mode),
        rate_numerator: sample.rate.numerator,
        rate_denominator: sample.rate.denominator,
        elapsed_micros: sample.elapsed_micros,
        runtime_frame: sample.frame,
        has_clip_frame: sample.clip_frame.is_some(),
        clip_frame: sample.clip_frame.unwrap_or(0),
        ended: sample.ended,
    }
}

fn hash(value: &str) -> Result<NativeVoxelContentHash, CsharpEngineServicesError> {
    let value = value.strip_prefix("sha256:").unwrap_or(value);
    if value.len() != 64 || !value.is_ascii() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_VOXEL_CONTENT_HASH",
            "admitted voxel digest was not SHA-256",
        ));
    }
    let word = |offset| {
        u64::from_str_radix(&value[offset..offset + 16], 16).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_CONTENT_HASH",
                "admitted voxel digest was not hexadecimal",
            )
        })
    };
    Ok(NativeVoxelContentHash {
        word0: word(0)?,
        word1: word(16)?,
        word2: word(32)?,
        word3: word(48)?,
    })
}

fn hash_string(value: NativeVoxelContentHash) -> String {
    format!(
        "sha256:{:016x}{:016x}{:016x}{:016x}",
        value.word0, value.word1, value.word2, value.word3
    )
}

fn narrow(value: usize) -> Result<u32, CsharpEngineServicesError> {
    u32::try_from(value).map_err(|_| {
        CsharpEngineServicesError::new(
            "CSHARP_VOXEL_CONTENT_LIMIT",
            "voxel owner count exceeds the C# ABI range",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{appearance::RuntimeAppearanceBridge, spatial::RuntimeSpatialBridge};
    use render_model::RenderDiff;
    use render_projection::RuntimeAppearanceCatalog;
    use std::collections::BTreeMap;
    use voxel_annotation::{
        encode_annotation_layer, finalize_annotation_draft, VoxelAnnotationLayerDraft,
        VoxelAnnotationRegion, VoxelAnnotationSelection, VoxelAnnotationSparseRun,
    };
    use voxel_asset::{
        encode_voxel_asset, encode_voxel_object, with_computed_content_hash,
        with_computed_voxel_object_hashes, VoxelAssetBounds, VoxelAssetGrid,
        VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelAssetProvenance,
        VoxelAssetProvenanceKind, VoxelCoordinateSystem, VoxelFrame, VoxelObjectAnimationFrame,
        VoxelObjectAsset, VoxelObjectClip, VoxelObjectGrid, VoxelObjectProvenance,
        VoxelObjectProvenanceKind, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
        VOXEL_ASSET_SCHEMA_VERSION, VOXEL_OBJECT_SCHEMA_VERSION,
    };

    #[test]
    fn projects_retained_voxel_objects_through_staged_renderer_frames_and_releases_them() {
        let mut bridge = RuntimeVoxelContentBridge::new();
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);

        let body = encode_voxel_object(&object())
            .expect("canonical object")
            .into_bytes();
        let mut object_handle = NativeVoxelObjectHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_object)(
                    api.context,
                    &NativeAdmitVoxelObjectRequest {
                        bytes: NativeByteSlice {
                            bytes: body.as_ptr(),
                            len: body.len(),
                        },
                    },
                    &mut object_handle,
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
                    &mut material,
                )
            },
            ABI_OK
        );
        let bindings = [NativeVoxelObjectMaterialBinding {
            material_slot: 1,
            material,
        }];
        let mut presentation = NativeVoxelObjectPresentationHandle::default();
        assert_eq!(
            unsafe {
                (api.project_object)(
                    api.context,
                    &NativeProjectVoxelObjectRequest {
                        object: object_handle,
                        runtime_frame: 0,
                        transform: NativeTransform {
                            translation: NativeVec3::default(),
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
                        },
                        visible: true,
                        materials: bindings.as_ptr(),
                        materials_len: bindings.len(),
                    },
                    &mut presentation,
                )
            },
            ABI_OK
        );
        let staged = bridge.take_staged_call().expect("staged projection");
        assert!(staged.frames.iter().any(|frame| {
            frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::DefineVoxelObject { .. }))
                && frame.ops.iter().any(|operation| {
                    matches!(operation, RenderDiff::CreateVoxelObjectInstance { .. })
                })
        }));
        bridge.commit_call(staged);
        let staged_appearance = appearance.take_staged_call().expect("staged material");
        appearance.commit(staged_appearance);

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        let mut conflicting_material = NativeMaterialHandle::default();
        assert_eq!(
            unsafe {
                crate::appearance::create_material(
                    (&mut appearance as *mut RuntimeAppearanceBridge).cast(),
                    NativeMaterialRequest {
                        color: NativeColor {
                            r: 0.75,
                            g: 0.25,
                            b: 0.5,
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
                    &mut conflicting_material,
                )
            },
            ABI_OK
        );
        let conflicting_bindings = [NativeVoxelObjectMaterialBinding {
            material_slot: 1,
            material: conflicting_material,
        }];
        let mut rejected = NativeVoxelObjectPresentationHandle::default();
        assert_eq!(
            unsafe {
                (api.project_object)(
                    api.context,
                    &NativeProjectVoxelObjectRequest {
                        object: object_handle,
                        runtime_frame: 0,
                        transform: NativeTransform {
                            translation: NativeVec3::default(),
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
                        },
                        visible: true,
                        materials: conflicting_bindings.as_ptr(),
                        materials_len: conflicting_bindings.len(),
                    },
                    &mut rejected,
                )
            },
            0,
            "a conflicting retained material identity rejects atomically"
        );
        let rejected_call = bridge.take_staged_call().expect("rejected projection call");
        assert!(
            rejected_call.frames.is_empty(),
            "rejected projection emits no frame"
        );
        assert_eq!(
            rejected_call.state.presentations.len(),
            1,
            "rejected projection does not retain a second presentation"
        );
        bridge.commit_call(rejected_call);
        let staged_appearance = appearance
            .take_staged_call()
            .expect("staged conflicting material");
        appearance.commit(staged_appearance);

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        assert_eq!(
            unsafe {
                (api.update_object_presentation)(
                    api.context,
                    &NativeUpdateVoxelObjectPresentationRequest {
                        presentation,
                        runtime_frame: 1,
                        transform: NativeTransform {
                            translation: NativeVec3 {
                                x: 2.0,
                                y: 0.0,
                                z: 0.0,
                            },
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
                        },
                        visible: false,
                        materials: bindings.as_ptr(),
                        materials_len: bindings.len(),
                    },
                )
            },
            ABI_OK
        );
        let staged = bridge.take_staged_call().expect("staged frame update");
        assert!(staged.frames.iter().any(|frame| {
            frame.ops.iter().any(|operation| {
                matches!(operation, RenderDiff::SetVoxelObjectFrame { frame: 1, .. })
            }) && frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::Update { .. }))
        }));
        bridge.commit_call(staged);
        let staged_appearance = appearance
            .take_staged_call()
            .expect("staged update material");
        appearance.commit(staged_appearance);

        appearance.begin_call();
        bridge.begin_call();
        let api = super::api(&mut bridge, &mut appearance);
        assert_eq!(
            unsafe { (api.destroy_object_presentation)(api.context, presentation) },
            ABI_OK
        );
        let staged = bridge.take_staged_call().expect("staged release");
        assert!(staged.frames.iter().any(|frame| {
            frame
                .ops
                .iter()
                .any(|operation| matches!(operation, RenderDiff::Destroy { .. }))
                && frame
                    .ops
                    .iter()
                    .any(|operation| matches!(operation, RenderDiff::ReleaseVoxelObject { .. }))
        }));
        assert_eq!(
            unsafe { (api.destroy_object_presentation)(api.context, presentation) },
            0,
            "each retained presentation has one exact release"
        );
    }

    #[test]
    fn admits_owned_typed_artifacts_selects_a_local_frame_and_leaves_spatial_unchanged() {
        let mut spatial = RuntimeSpatialBridge::new();
        let spatial_api = crate::spatial::api(&mut spatial);
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
        let voxel_api = crate::voxel::api(&mut spatial);
        let mut before = NativeVoxelSceneReadout::default();
        assert_eq!(
            unsafe {
                (voxel_api.read_scene)(
                    voxel_api.context,
                    NativeVoxelSceneReadRequest { session },
                    &mut before,
                )
            },
            ABI_OK
        );

        let mut bridge = RuntimeVoxelContentBridge::new();
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        let api = super::api(&mut bridge, &mut appearance);
        let mut asset_body = encode_voxel_asset(&asset())
            .expect("canonical asset")
            .into_bytes();
        let asset_request = NativeAdmitVoxelAssetRequest {
            bytes: NativeByteSlice {
                bytes: asset_body.as_ptr(),
                len: asset_body.len(),
            },
        };
        let mut asset_handle = NativeVoxelAssetHandle::default();
        assert_eq!(
            unsafe { (api.admit_asset)(api.context, &asset_request, &mut asset_handle) },
            ABI_OK
        );
        asset_body.fill(0);
        let mut asset_readout = NativeVoxelAssetReadout::default();
        assert_eq!(
            unsafe { (api.read_asset)(api.context, asset_handle, &mut asset_readout) },
            ABI_OK
        );
        assert_eq!(asset_readout.represented_voxel_count, 1);
        assert_ne!(
            asset_readout.content_hash,
            NativeVoxelContentHash::default()
        );

        let invalid = b"not a voxel artifact";
        let invalid_request = NativeAdmitVoxelAssetRequest {
            bytes: NativeByteSlice {
                bytes: invalid.as_ptr(),
                len: invalid.len(),
            },
        };
        let mut invalid_handle = NativeVoxelAssetHandle::default();
        assert_eq!(
            unsafe { (api.admit_asset)(api.context, &invalid_request, &mut invalid_handle) },
            0
        );

        let mut object_body = encode_voxel_object(&object())
            .expect("canonical object")
            .into_bytes();
        let object_request = NativeAdmitVoxelObjectRequest {
            bytes: NativeByteSlice {
                bytes: object_body.as_ptr(),
                len: object_body.len(),
            },
        };
        let mut object_handle = NativeVoxelObjectHandle::default();
        assert_eq!(
            unsafe { (api.admit_object)(api.context, &object_request, &mut object_handle) },
            ABI_OK
        );
        object_body.fill(0);
        let mut object_readout = NativeVoxelObjectReadout::default();
        assert_eq!(
            unsafe { (api.read_object)(api.context, object_handle, &mut object_readout) },
            ABI_OK
        );
        assert_eq!(object_readout.frame_count, 2);
        assert_eq!(object_readout.clip_count, 1);

        let clip = b"walk";
        let selection_request = NativeSelectVoxelObjectClipFrameRequest {
            object_handle,
            clip: NativeUtf8Slice {
                bytes: clip.as_ptr(),
                len: clip.len(),
            },
            frame_index: 0,
        };
        let mut selected = NativeVoxelObjectFrameReadout::default();
        assert_eq!(
            unsafe {
                (api.select_object_clip_frame)(api.context, &selection_request, &mut selected)
            },
            ABI_OK
        );
        assert!(!selected.is_default);
        assert_eq!(selected.clip_index, 0);
        assert_eq!(selected.cell_count, 1);

        let mut after = NativeVoxelSceneReadout::default();
        assert_eq!(
            unsafe {
                (voxel_api.read_scene)(
                    voxel_api.context,
                    NativeVoxelSceneReadRequest { session },
                    &mut after,
                )
            },
            ABI_OK
        );
        assert_eq!(after.source_revision, before.source_revision);
        assert_eq!(after.solid_voxel_count, before.solid_voxel_count);
        assert_eq!(after.authority_hash, before.authority_hash);

        assert_eq!(
            unsafe { (api.destroy_asset)(api.context, asset_handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.read_asset)(api.context, asset_handle, &mut asset_readout) },
            0
        );
        assert_eq!(
            unsafe { (api.destroy_object)(api.context, object_handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.read_object)(api.context, object_handle, &mut object_readout) },
            0
        );
    }

    #[test]
    fn publishes_admitted_asset_into_fresh_spatial_session_with_palette_lease() {
        let mut spatial = RuntimeSpatialBridge::new();
        let spatial_api = crate::spatial::api(&mut spatial);
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

        let mut bridge = RuntimeVoxelContentBridge::new();
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        let api = super::api_with_spatial(&mut bridge, &mut appearance, &mut spatial);
        let body = encode_voxel_asset(&asset())
            .expect("canonical asset")
            .into_bytes();
        let mut asset_handle = NativeVoxelAssetHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_asset)(
                    api.context,
                    &NativeAdmitVoxelAssetRequest {
                        bytes: NativeByteSlice {
                            bytes: body.as_ptr(),
                            len: body.len(),
                        },
                    },
                    &mut asset_handle,
                )
            },
            ABI_OK
        );

        let mut lease = unsafe { std::mem::zeroed::<NativeVoxelAssetSpatialPublishLease>() };
        assert_eq!(
            unsafe {
                (api.publish_asset_to_spatial)(
                    api.context,
                    &NativePublishVoxelAssetToSpatialRequest {
                        asset: asset_handle,
                        session,
                    },
                    &mut lease,
                )
            },
            ABI_OK
        );
        assert_eq!(lease.revision_before, 0);
        assert_eq!(lease.revision_after, 0);
        assert_eq!(lease.solid_voxel_count, 1);
        assert_eq!(lease.resident_chunk_count, 1);
        assert_ne!(lease.authority_hash, 0);
        assert_eq!(lease.palette_len, 1);
        let palette_row = unsafe { &*lease.palette };
        assert_eq!(palette_row.material_slot, 1);
        assert_eq!(
            unsafe {
                std::str::from_utf8(std::slice::from_raw_parts(
                    palette_row.material_asset_id.bytes,
                    palette_row.material_asset_id.len,
                ))
            }
            .expect("leased material id"),
            "material/bridge-test"
        );

        let voxel_api = crate::voxel::api(&mut spatial);
        let mut scene = NativeVoxelSceneReadout::default();
        assert_eq!(
            unsafe {
                (voxel_api.read_scene)(
                    voxel_api.context,
                    NativeVoxelSceneReadRequest { session },
                    &mut scene,
                )
            },
            ABI_OK
        );
        assert_eq!(scene.solid_voxel_count, lease.solid_voxel_count);
        assert_eq!(scene.resident_chunk_count, lease.resident_chunk_count);
        assert_eq!(scene.authority_hash, lease.authority_hash);
        assert_eq!(scene.navigation_revision, lease.navigation_revision);

        assert_eq!(
            unsafe { (api.destroy_asset_spatial_publish_lease)(api.context, lease.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_asset_spatial_publish_lease)(api.context, lease.handle) },
            0,
            "publish leases have one exact release"
        );

        let before_rejected = scene;
        let mut rejected = unsafe { std::mem::zeroed::<NativeVoxelAssetSpatialPublishLease>() };
        assert_eq!(
            unsafe {
                (api.publish_asset_to_spatial)(
                    api.context,
                    &NativePublishVoxelAssetToSpatialRequest {
                        asset: asset_handle,
                        session,
                    },
                    &mut rejected,
                )
            },
            0,
            "a nonempty session cannot be initialized a second time"
        );
        let mut after_rejected = NativeVoxelSceneReadout::default();
        assert_eq!(
            unsafe {
                (voxel_api.read_scene)(
                    voxel_api.context,
                    NativeVoxelSceneReadRequest { session },
                    &mut after_rejected,
                )
            },
            ABI_OK
        );
        assert_eq!(after_rejected, before_rejected);
    }

    #[test]
    fn retains_explicit_time_object_players_after_object_release_and_rejects_bad_times() {
        let mut bridge = RuntimeVoxelContentBridge::new();
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        let api = super::api(&mut bridge, &mut appearance);
        let object_body = encode_voxel_object(&object())
            .expect("canonical object")
            .into_bytes();
        let request = NativeAdmitVoxelObjectRequest {
            bytes: NativeByteSlice {
                bytes: object_body.as_ptr(),
                len: object_body.len(),
            },
        };
        let mut object_handle = NativeVoxelObjectHandle::default();
        assert_eq!(
            unsafe { (api.admit_object)(api.context, &request, &mut object_handle) },
            ABI_OK
        );

        let mut player = NativeVoxelObjectPlayerHandle::default();
        let mut overflow_player = NativeVoxelObjectPlayerHandle::default();
        assert_eq!(
            unsafe { (api.create_object_player)(api.context, object_handle, &mut player) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.create_object_player)(api.context, object_handle, &mut overflow_player) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_object)(api.context, object_handle) },
            ABI_OK
        );
        let mut object_readout = NativeVoxelObjectReadout::default();
        assert_eq!(
            unsafe { (api.read_object)(api.context, object_handle, &mut object_readout) },
            0
        );

        let clip = b"walk";
        let play = NativePlayVoxelObjectPlayerRequest {
            player_handle: player,
            clip: NativeUtf8Slice {
                bytes: clip.as_ptr(),
                len: clip.len(),
            },
            loop_mode: NativeVoxelObjectLoopMode::Once,
            rate_numerator: 1,
            rate_denominator: 1,
            now_micros: 10,
        };
        assert_eq!(
            unsafe { (api.play_object_player)(api.context, &play) },
            ABI_OK
        );

        let mut sample = NativeVoxelObjectPlayerSampleReadout::default();
        assert_eq!(
            unsafe {
                (api.sample_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: player,
                        now_micros: 10,
                    },
                    &mut sample,
                )
            },
            ABI_OK
        );
        assert_eq!(sample.status, NativeVoxelObjectPlaybackStatus::Playing);
        assert_eq!(sample.runtime_frame, 1);
        assert!(sample.has_clip_frame);
        assert_eq!(sample.clip_frame, 0);
        assert!(!sample.ended);

        assert_eq!(
            unsafe {
                (api.sample_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: player,
                        now_micros: 9,
                    },
                    &mut sample,
                )
            },
            0,
            "backward explicit time must fail deterministically"
        );
        assert_eq!(
            unsafe {
                (api.pause_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: player,
                        now_micros: 50,
                    },
                )
            },
            ABI_OK
        );
        let mut readout = NativeVoxelObjectPlayerReadout::default();
        assert_eq!(
            unsafe {
                (api.read_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: player,
                        now_micros: 500,
                    },
                    &mut readout,
                )
            },
            ABI_OK
        );
        assert_eq!(readout.status, NativeVoxelObjectPlaybackStatus::Paused);
        assert_eq!(readout.elapsed_micros, 40);
        assert_eq!(
            unsafe {
                (api.resume_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: player,
                        now_micros: 100,
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.sample_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: player,
                        now_micros: 83_393,
                    },
                    &mut sample,
                )
            },
            ABI_OK
        );
        assert!(sample.ended);

        let scrub = NativeScrubVoxelObjectPlayerRequest {
            player_handle: player,
            clip: NativeUtf8Slice {
                bytes: clip.as_ptr(),
                len: clip.len(),
            },
            clip_frame: 0,
            loop_mode: NativeVoxelObjectLoopMode::Repeat,
        };
        assert_eq!(
            unsafe { (api.scrub_object_player)(api.context, &scrub) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.stop_object_player)(api.context, player) },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.sample_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: player,
                        now_micros: 0,
                    },
                    &mut sample,
                )
            },
            ABI_OK
        );
        assert_eq!(sample.status, NativeVoxelObjectPlaybackStatus::Stopped);
        assert_eq!(sample.runtime_frame, 0);
        assert!(!sample.has_clip_frame);

        let overflow_play = NativePlayVoxelObjectPlayerRequest {
            player_handle: overflow_player,
            clip: NativeUtf8Slice {
                bytes: clip.as_ptr(),
                len: clip.len(),
            },
            loop_mode: NativeVoxelObjectLoopMode::Once,
            rate_numerator: 2,
            rate_denominator: 1,
            now_micros: 0,
        };
        assert_eq!(
            unsafe { (api.play_object_player)(api.context, &overflow_play) },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.pause_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: overflow_player,
                        now_micros: u64::MAX,
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.sample_object_player)(
                    api.context,
                    NativeVoxelObjectPlayerTimeRequest {
                        player_handle: overflow_player,
                        now_micros: u64::MAX,
                    },
                    &mut sample,
                )
            },
            0,
            "rate scaling overflow must fail deterministically"
        );
        assert_eq!(
            unsafe { (api.destroy_object_player)(api.context, player) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_object_player)(api.context, overflow_player) },
            ABI_OK
        );
        assert_eq!(unsafe { (api.stop_object_player)(api.context, player) }, 0);
    }

    #[test]
    fn retains_target_bound_annotations_queries_bounded_leases_and_keeps_metadata_edits_atomic() {
        let mut bridge = RuntimeVoxelContentBridge::new();
        let mut appearance =
            RuntimeAppearanceBridge::new(RuntimeAppearanceCatalog::default(), BTreeMap::new());
        let api = super::api(&mut bridge, &mut appearance);
        let asset = annotation_asset();
        let asset_body = encode_voxel_asset(&asset)
            .expect("canonical asset")
            .into_bytes();
        let mut asset_handle = NativeVoxelAssetHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_asset)(
                    api.context,
                    &NativeAdmitVoxelAssetRequest {
                        bytes: NativeByteSlice {
                            bytes: asset_body.as_ptr(),
                            len: asset_body.len(),
                        },
                    },
                    &mut asset_handle,
                )
            },
            ABI_OK
        );

        let annotation = annotation(&asset);
        let annotation_body = encode_annotation_layer(&annotation)
            .expect("canonical annotation")
            .into_bytes();
        let mut annotation_handle = NativeVoxelAnnotationHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_annotation)(
                    api.context,
                    &NativeAdmitVoxelAnnotationRequest {
                        asset: asset_handle,
                        bytes: NativeByteSlice {
                            bytes: annotation_body.as_ptr(),
                            len: annotation_body.len(),
                        },
                    },
                    &mut annotation_handle,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_asset)(api.context, asset_handle) },
            ABI_OK,
            "annotation retains its admitted target after the direct asset release"
        );

        let initial_hash = hash(&annotation.content_hashes.canonical_layer).expect("valid hash");
        let region_a = b"region/bridge-a";
        let bounds = NativeVoxelAnnotationBounds {
            min_x: 0,
            min_y: 0,
            min_z: 0,
            max_x: 0,
            max_y: 0,
            max_z: 0,
        };
        for mode in [
            NativeVoxelAnnotationQueryMode::Cell,
            NativeVoxelAnnotationQueryMode::Bounds,
            NativeVoxelAnnotationQueryMode::Region,
            NativeVoxelAnnotationQueryMode::LayerSummary,
        ] {
            let mut lease = unsafe { std::mem::zeroed::<NativeVoxelAnnotationRegionLease>() };
            let max_results = if mode == NativeVoxelAnnotationQueryMode::LayerSummary {
                1
            } else {
                8
            };
            assert_eq!(
                unsafe {
                    (api.query_annotation)(
                        api.context,
                        &NativeVoxelAnnotationQueryRequest {
                            annotation: annotation_handle,
                            mode,
                            coordinate_x: 0,
                            coordinate_y: 0,
                            coordinate_z: 0,
                            bounds,
                            region_id: NativeUtf8Slice {
                                bytes: region_a.as_ptr(),
                                len: region_a.len(),
                            },
                            has_expected_layer_hash: true,
                            expected_layer_hash: initial_hash,
                            max_results,
                        },
                        &mut lease,
                    )
                },
                ABI_OK
            );
            assert_eq!(lease.total_layer_regions, 2);
            assert_eq!(lease.revision, 0);
            assert_eq!(lease.layer_hash, initial_hash);
            assert_eq!(
                lease.truncated,
                mode == NativeVoxelAnnotationQueryMode::LayerSummary
            );
            assert!(!lease.regions.is_null());
            assert!(lease.regions_len >= 1);
            assert_eq!(
                unsafe {
                    std::str::from_utf8(std::slice::from_raw_parts(
                        (*lease.regions).region_id.bytes,
                        (*lease.regions).region_id.len,
                    ))
                }
                .expect("leased region id"),
                "region/bridge-a"
            );
            assert_eq!(
                unsafe { (api.destroy_annotation_region_lease)(api.context, lease.handle) },
                ABI_OK
            );
            assert_eq!(
                unsafe { (api.destroy_annotation_region_lease)(api.context, lease.handle) },
                0,
                "each query lease has one exact release"
            );
        }

        let label = b"renamed";
        let mut label_edit = unsafe { std::mem::zeroed::<NativeVoxelAnnotationEditLease>() };
        assert_eq!(
            unsafe {
                (api.set_annotation_label)(
                    api.context,
                    &NativeSetVoxelAnnotationLabelRequest {
                        annotation: annotation_handle,
                        expected_layer_hash: initial_hash,
                        region_id: NativeUtf8Slice {
                            bytes: region_a.as_ptr(),
                            len: region_a.len(),
                        },
                        label: NativeUtf8Slice {
                            bytes: label.as_ptr(),
                            len: label.len(),
                        },
                    },
                    &mut label_edit,
                )
            },
            ABI_OK
        );
        assert_eq!(label_edit.revision, 1);
        let label_hash = label_edit.layer_hash_after;
        assert_eq!(label_edit.affected_ids_len, 1);
        assert_eq!(
            unsafe {
                std::str::from_utf8(std::slice::from_raw_parts(
                    (*label_edit.affected_ids).region_id.bytes,
                    (*label_edit.affected_ids).region_id.len,
                ))
            }
            .expect("leased affected id"),
            "region/bridge-a"
        );
        assert_eq!(
            unsafe { (api.destroy_annotation_edit_lease)(api.context, label_edit.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_annotation_edit_lease)(api.context, label_edit.handle) },
            0,
            "each edit lease has one exact release"
        );

        let mut stale_kind = unsafe { std::mem::zeroed::<NativeVoxelAnnotationEditLease>() };
        assert_eq!(
            unsafe {
                (api.set_annotation_kind)(
                    api.context,
                    &NativeSetVoxelAnnotationKindRequest {
                        annotation: annotation_handle,
                        expected_layer_hash: initial_hash,
                        region_id: NativeUtf8Slice {
                            bytes: region_a.as_ptr(),
                            len: region_a.len(),
                        },
                        kind: NativeVoxelAnnotationKind::Room,
                    },
                    &mut stale_kind,
                )
            },
            0,
            "stale edits fail before the owner commits"
        );

        let mut kind_edit = unsafe { std::mem::zeroed::<NativeVoxelAnnotationEditLease>() };
        assert_eq!(
            unsafe {
                (api.set_annotation_kind)(
                    api.context,
                    &NativeSetVoxelAnnotationKindRequest {
                        annotation: annotation_handle,
                        expected_layer_hash: label_hash,
                        region_id: NativeUtf8Slice {
                            bytes: region_a.as_ptr(),
                            len: region_a.len(),
                        },
                        kind: NativeVoxelAnnotationKind::Room,
                    },
                    &mut kind_edit,
                )
            },
            ABI_OK
        );
        assert_eq!(kind_edit.revision, 2);
        let kind_hash = kind_edit.layer_hash_after;
        assert_eq!(
            unsafe { (api.destroy_annotation_edit_lease)(api.context, kind_edit.handle) },
            ABI_OK
        );

        let region_b = b"region/bridge-b";
        let mut parent_edit = unsafe { std::mem::zeroed::<NativeVoxelAnnotationEditLease>() };
        assert_eq!(
            unsafe {
                (api.set_annotation_parent)(
                    api.context,
                    &NativeSetVoxelAnnotationParentRequest {
                        annotation: annotation_handle,
                        expected_layer_hash: kind_hash,
                        region_id: NativeUtf8Slice {
                            bytes: region_b.as_ptr(),
                            len: region_b.len(),
                        },
                        has_parent_region_id: true,
                        parent_region_id: NativeUtf8Slice {
                            bytes: region_a.as_ptr(),
                            len: region_a.len(),
                        },
                    },
                    &mut parent_edit,
                )
            },
            ABI_OK
        );
        assert_eq!(parent_edit.revision, 3);
        let parent_hash = parent_edit.layer_hash_after;
        assert_eq!(
            unsafe { (api.destroy_annotation_edit_lease)(api.context, parent_edit.handle) },
            ABI_OK
        );

        let mut bounds_edit = unsafe { std::mem::zeroed::<NativeVoxelAnnotationEditLease>() };
        assert_eq!(
            unsafe {
                (api.set_annotation_bounds)(
                    api.context,
                    &NativeSetVoxelAnnotationBoundsRequest {
                        annotation: annotation_handle,
                        expected_layer_hash: parent_hash,
                        region_id: NativeUtf8Slice {
                            bytes: region_a.as_ptr(),
                            len: region_a.len(),
                        },
                        bounds: NativeVoxelAnnotationBounds { max_x: 1, ..bounds },
                    },
                    &mut bounds_edit,
                )
            },
            ABI_OK
        );
        assert_eq!(bounds_edit.revision, 4);
        let bounds_hash = bounds_edit.layer_hash_after;
        assert_eq!(
            unsafe { (api.destroy_annotation_edit_lease)(api.context, bounds_edit.handle) },
            ABI_OK
        );

        let mut tag_one = "café".as_bytes().to_vec();
        let mut tag_two = b"combat".to_vec();
        let tags = [
            NativeVoxelAnnotationTag {
                value: NativeUtf8Slice {
                    bytes: tag_one.as_ptr(),
                    len: tag_one.len(),
                },
            },
            NativeVoxelAnnotationTag {
                value: NativeUtf8Slice {
                    bytes: tag_two.as_ptr(),
                    len: tag_two.len(),
                },
            },
        ];
        let mut tags_edit = unsafe { std::mem::zeroed::<NativeVoxelAnnotationEditLease>() };
        assert_eq!(
            unsafe {
                (api.set_annotation_tags)(
                    api.context,
                    &NativeSetVoxelAnnotationTagsRequest {
                        annotation: annotation_handle,
                        expected_layer_hash: bounds_hash,
                        region_id: NativeUtf8Slice {
                            bytes: region_a.as_ptr(),
                            len: region_a.len(),
                        },
                        tags: tags.as_ptr(),
                        tags_len: tags.len(),
                    },
                    &mut tags_edit,
                )
            },
            ABI_OK
        );
        assert_eq!(tags_edit.revision, 5);
        assert_eq!(
            unsafe { (api.destroy_annotation_edit_lease)(api.context, tags_edit.handle) },
            ABI_OK
        );
        tag_one.fill(b'x');
        tag_two.fill(b'y');
        let copied_tags = &bridge
            .annotation(annotation_handle)
            .expect("retained annotation")
            .layer
            .regions
            .iter()
            .find(|region| region.region_id == "region/bridge-a")
            .expect("retained region")
            .tags;
        assert_eq!(
            copied_tags,
            &["café", "combat"],
            "tag input is copied before the callback returns"
        );

        let stale_tags = [NativeVoxelAnnotationTag {
            value: NativeUtf8Slice {
                bytes: b"stale".as_ptr(),
                len: b"stale".len(),
            },
        }];
        let mut stale_tags_edit = unsafe { std::mem::zeroed::<NativeVoxelAnnotationEditLease>() };
        assert_eq!(
            unsafe {
                (api.set_annotation_tags)(
                    api.context,
                    &NativeSetVoxelAnnotationTagsRequest {
                        annotation: annotation_handle,
                        expected_layer_hash: bounds_hash,
                        region_id: NativeUtf8Slice {
                            bytes: region_a.as_ptr(),
                            len: region_a.len(),
                        },
                        tags: stale_tags.as_ptr(),
                        tags_len: stale_tags.len(),
                    },
                    &mut stale_tags_edit,
                )
            },
            0,
            "a stale expected hash does not commit tag replacement"
        );
        assert_eq!(
            bridge
                .annotation(annotation_handle)
                .expect("retained annotation")
                .layer
                .regions
                .iter()
                .find(|region| region.region_id == "region/bridge-a")
                .expect("retained region")
                .tags,
            vec!["café", "combat"]
        );

        let mut after = unsafe { std::mem::zeroed::<NativeVoxelAnnotationRegionLease>() };
        assert_eq!(
            unsafe {
                (api.query_annotation)(
                    api.context,
                    &NativeVoxelAnnotationQueryRequest {
                        annotation: annotation_handle,
                        mode: NativeVoxelAnnotationQueryMode::Region,
                        coordinate_x: 0,
                        coordinate_y: 0,
                        coordinate_z: 0,
                        bounds,
                        region_id: NativeUtf8Slice {
                            bytes: region_a.as_ptr(),
                            len: region_a.len(),
                        },
                        has_expected_layer_hash: false,
                        expected_layer_hash: NativeVoxelContentHash::default(),
                        max_results: 1,
                    },
                    &mut after,
                )
            },
            ABI_OK
        );
        assert_eq!(after.revision, 5);
        assert_eq!(
            unsafe { (*after.regions).kind },
            NativeVoxelAnnotationKind::Room
        );
        assert_eq!(
            unsafe {
                std::str::from_utf8(std::slice::from_raw_parts(
                    (*after.regions).label.bytes,
                    (*after.regions).label.len,
                ))
            }
            .expect("leased label"),
            "renamed",
            "stale failure did not partially apply kind"
        );
        assert_eq!(unsafe { (*after.regions).bounds.max_x }, 1);
        assert_eq!(
            unsafe { (api.destroy_annotation_region_lease)(api.context, after.handle) },
            ABI_OK
        );

        assert_eq!(
            unsafe { (api.destroy_annotation)(api.context, annotation_handle) },
            ABI_OK
        );
    }

    fn asset() -> VoxelAsset {
        with_computed_content_hash(VoxelAsset {
            schema_version: VOXEL_ASSET_SCHEMA_VERSION,
            asset_id: "voxel-volume/bridge-test".to_owned(),
            grid: VoxelAssetGrid {
                coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
                cell_size: 1.0,
                chunk_size: 8,
                origin: [0, 0, 0],
            },
            bounds: VoxelAssetBounds {
                min: [0, 0, 0],
                max: [0, 0, 0],
            },
            representation: representation([0, 0, 0]),
            material_palette: palette(),
            material_map: mapping(),
            provenance: asset_provenance(),
            voxel_data_hash: String::new(),
            content_hash: String::new(),
        })
        .expect("valid asset")
    }

    fn annotation_asset() -> VoxelAsset {
        let mut value = asset();
        value.bounds.max[0] = 1;
        value.representation.sparse_runs.push(VoxelSparseRun {
            start: [1, 0, 0],
            length: 1,
            material_slot: 1,
        });
        with_computed_content_hash(value).expect("valid annotation target asset")
    }

    fn annotation(asset: &VoxelAsset) -> VoxelAnnotationLayer {
        finalize_annotation_draft(
            VoxelAnnotationLayerDraft {
                layer_id: "voxel-annotation/bridge-test".to_owned(),
                target_voxel_asset_id: asset.asset_id.clone(),
                target_voxel_data_hash: asset.voxel_data_hash.clone(),
                target_bounds: VoxelAnnotationBounds {
                    min: asset.bounds.min,
                    max: asset.bounds.max,
                },
                regions: vec![
                    VoxelAnnotationRegion {
                        region_id: "region/bridge-a".to_owned(),
                        label: "original".to_owned(),
                        kind: VoxelAnnotationKind::Selection,
                        tags: vec![],
                        parent_region_id: None,
                        bounds: VoxelAnnotationBounds {
                            min: [0, 0, 0],
                            max: [0, 0, 0],
                        },
                        selection: VoxelAnnotationSelection {
                            sparse_runs: vec![VoxelAnnotationSparseRun {
                                start: [0, 0, 0],
                                length: 1,
                            }],
                        },
                    },
                    VoxelAnnotationRegion {
                        region_id: "region/bridge-b".to_owned(),
                        label: "second".to_owned(),
                        kind: VoxelAnnotationKind::Selection,
                        tags: vec![],
                        parent_region_id: None,
                        bounds: VoxelAnnotationBounds {
                            min: [1, 0, 0],
                            max: [1, 0, 0],
                        },
                        selection: VoxelAnnotationSelection {
                            sparse_runs: vec![VoxelAnnotationSparseRun {
                                start: [1, 0, 0],
                                length: 1,
                            }],
                        },
                    },
                ],
                provenance: vec![],
            },
            asset,
            Default::default(),
        )
        .expect("valid annotation")
    }

    fn object() -> VoxelObjectAsset {
        with_computed_voxel_object_hashes(VoxelObjectAsset {
            schema_version: VOXEL_OBJECT_SCHEMA_VERSION,
            asset_id: "voxel-object/bridge-test".to_owned(),
            grid: VoxelObjectGrid {
                coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
                cell_size: 1.0,
                chunk_size: 8,
                pivot: [0.0, 0.0, 0.0],
            },
            bounds: VoxelAssetBounds {
                min: [0, 0, 0],
                max: [1, 0, 0],
            },
            default_frame: frame([0, 0, 0]),
            clips: vec![VoxelObjectClip {
                id: "walk".to_owned(),
                name: None,
                frames_per_second: 12.0,
                frames: vec![VoxelObjectAnimationFrame {
                    duration_seconds: None,
                    anchors: vec![],
                    collision: None,
                    frame: frame([1, 0, 0]),
                }],
            }],
            default_clip: Some("walk".to_owned()),
            material_palette: palette(),
            material_map: mapping(),
            provenance: VoxelObjectProvenance {
                kind: VoxelObjectProvenanceKind::Authored,
                source_path: "bridge-test.voxel-object.json".to_owned(),
                source_sha256: digest('a'),
                source_byte_count: 1,
                converter: "bridge-test".to_owned(),
                settings_sha256: digest('b'),
                license_path: None,
                source_clips: vec![],
            },
            content_hash: String::new(),
        })
        .expect("valid object")
    }

    fn frame(start: [i64; 3]) -> VoxelFrame {
        VoxelFrame {
            bounds: VoxelAssetBounds {
                min: start,
                max: start,
            },
            representation: representation(start),
            voxel_data_hash: String::new(),
        }
    }

    fn representation(start: [i64; 3]) -> VoxelRepresentation {
        VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs: vec![VoxelSparseRun {
                start,
                length: 1,
                material_slot: 1,
            }],
        }
    }

    fn palette() -> Vec<VoxelAssetMaterialBinding> {
        vec![VoxelAssetMaterialBinding {
            material_slot: 1,
            material_asset_id: "material/bridge-test".to_owned(),
            display_name: None,
        }]
    }

    fn mapping() -> Vec<VoxelAssetMaterialMapping> {
        vec![VoxelAssetMaterialMapping {
            source_material_slot: 0,
            source_material_name: None,
            voxel_material_slot: 1,
        }]
    }

    fn asset_provenance() -> VoxelAssetProvenance {
        VoxelAssetProvenance {
            kind: VoxelAssetProvenanceKind::Authored,
            source_path: "bridge-test.voxel.json".to_owned(),
            source_sha256: digest('a'),
            source_byte_count: 1,
            converter: "bridge-test".to_owned(),
            settings_sha256: digest('b'),
            license_path: None,
        }
    }

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }
}
