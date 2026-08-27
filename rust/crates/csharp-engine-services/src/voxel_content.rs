//! Retained typed voxel artifacts behind the generated NativeAOT table.
//!
//! The bridge delegates validation and bounded runtime admission to the
//! existing voxel artifact owners. It does not feed their contents into the
//! canonical Spatial scene, collision, renderer, or a product-side store.

use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use voxel_asset::{decode_voxel_asset, represented_voxel_count, VoxelAsset, VoxelFrame};
use voxel_object_runtime::{admit_voxel_object_json, AdmittedVoxelObject};

use crate::{
    composition::{borrowed_utf8, ABI_OK},
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
    object: AdmittedVoxelObject,
    selected: ObjectFrameSelection,
    selection_revision: u64,
}

pub(crate) struct RuntimeVoxelContentBridge {
    assets: BTreeMap<u64, VoxelAsset>,
    objects: BTreeMap<u64, RetainedVoxelObject>,
    next_asset: u64,
    next_object: u64,
}

impl RuntimeVoxelContentBridge {
    pub(crate) fn new() -> Self {
        Self {
            assets: BTreeMap::new(),
            objects: BTreeMap::new(),
            next_asset: 1,
            next_object: 1,
        }
    }

    fn insert_asset(&mut self, asset: VoxelAsset) -> Option<NativeVoxelAssetHandle> {
        let value = self.next_asset;
        self.next_asset = value.checked_add(1)?;
        self.assets.insert(value, asset);
        Some(NativeVoxelAssetHandle { value })
    }

    fn insert_object(&mut self, object: AdmittedVoxelObject) -> Option<NativeVoxelObjectHandle> {
        let value = self.next_object;
        self.next_object = value.checked_add(1)?;
        self.objects.insert(
            value,
            RetainedVoxelObject {
                object,
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

    /// Internal retained-owner resolution for the later annotation bridge.
    /// This intentionally has no ABI callback: annotations remain a separate
    /// capability and cannot turn an asset handle into scene mutation.
    pub(crate) fn resolve_asset(
        &self,
        handle: NativeVoxelAssetHandle,
    ) -> Result<&VoxelAsset, CsharpEngineServicesError> {
        self.assets.get(&handle.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VOXEL_CONTENT_ASSET",
                "voxel asset handle is not admitted",
            )
        })
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
}

pub(crate) fn api(bridge: &mut RuntimeVoxelContentBridge) -> NativeVoxelContentApi {
    NativeVoxelContentApi {
        context: (bridge as *mut RuntimeVoxelContentBridge).cast(),
        admit_asset,
        destroy_asset,
        read_asset,
        admit_object,
        destroy_object,
        read_object,
        select_default_object_frame,
        select_object_clip_frame,
        read_selected_object_frame,
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
    use crate::spatial::RuntimeSpatialBridge;
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
                        reserved: 0,
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
        let api = super::api(&mut bridge);
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
