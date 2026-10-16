//! Vegetation fields (#10016): the product samples tiles of ground when the
//! Engine asks and decides what grows on them; the Engine keeps the tiles
//! about a focus, places each layer's copies within its budgets on worker
//! threads as each call finishes, and draws them as retained scatter
//! patches. The mechanism is `render_projection::VegetationFields`.

use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use render_model::{RenderDiff, RenderFrameDiff, ShadowCasting};
use render_projection::{
    VegetationError, VegetationFields, VegetationLayer, VegetationLayout, VegetationReadout,
    VegetationRecord, VegetationTileContent,
};
use svc_mesh::heightfield::TileSamples;

use crate::{
    appearance::RuntimeAppearanceBridge,
    composition::{borrowed_slice, ABI_OK},
    operation_diagnostics::{clear_receipt, refuse, BorrowedResult},
    spatial::SpatialCollisionSource,
    CsharpEngineServicesError,
};

fn refused(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_VEGETATION", message)
}

/// What the tiles centre on: a camera (by handle), or a world point.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    Camera(u64),
    Point([f64; 3]),
}

#[derive(Debug, Clone)]
struct RetainedField {
    /// The spatial session whose world it grows in (0: the scene's space).
    session: u64,
    focus: Focus,
    layers: BTreeMap<u32, RetainedLayer>,
}

#[derive(Debug, Clone)]
struct RetainedLayer {
    /// The layer as `SetLayer` gave it, with no levels or thinning.
    base: VegetationLayer,
    /// Its farther levels and thinning (`SetLayerDetail`).
    lods: Vec<render_model::ScatterLod>,
    lod_band: f32,
    thinning: render_model::ScatterThinning,
    /// The appearance it grows, held while it grows, and its levels'.
    appearance: u64,
    lod_appearances: Vec<u64>,
}

impl RetainedLayer {
    fn layer(&self) -> VegetationLayer {
        VegetationLayer {
            lods: self.lods.clone(),
            lod_band: self.lod_band,
            thinning: self.thinning.clone(),
            ..self.base.clone()
        }
    }

    fn held(&self) -> impl Iterator<Item = u64> + '_ {
        std::iter::once(self.appearance).chain(self.lod_appearances.iter().copied())
    }
}

impl RetainedField {
    /// Where the world's origin stands in the scene now.
    fn placement(&self, spatial: &SpatialCollisionSource) -> [f64; 3] {
        if self.session == 0 {
            return [0.0; 3];
        }
        let origin = spatial
            .world_origin(NativeSpatialSessionHandle {
                value: self.session,
            })
            .unwrap_or(core_space::WorldOrigin::ZERO);
        origin.cell().map(|cell| -(cell as f64))
    }
}

#[derive(Debug, Clone, Default)]
struct VegetationState {
    fields: BTreeMap<u64, RetainedField>,
    tiles: VegetationFields,
    next: u64,
}

pub(crate) struct RuntimeVegetationCall {
    state: VegetationState,
    pub(crate) frames: Vec<RenderFrameDiff>,
    /// Removals of patches made before this call, applied ahead of the
    /// call's Graphics work: the appearance a removed layer held may be
    /// disposed in the same call.
    pub(crate) early: Vec<RenderDiff>,
}

pub(crate) struct RuntimeVegetationBridge {
    spatial: SpatialCollisionSource,
    state: VegetationState,
    staged: Option<RuntimeVegetationCall>,
    appearance: Option<*mut RuntimeAppearanceBridge>,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
    borrowed: BorrowedResult,
}

impl RuntimeVegetationBridge {
    pub(crate) fn new(spatial: SpatialCollisionSource) -> Self {
        Self {
            spatial,
            state: VegetationState {
                next: 1,
                ..VegetationState::default()
            },
            staged: None,
            appearance: None,
            operation_diagnostics: Default::default(),
            borrowed: Default::default(),
        }
    }

    pub(crate) fn bind_appearance(&mut self, appearance: &mut RuntimeAppearanceBridge) {
        // The sibling bridge is retained by EngineServiceSet; the pointer is
        // refreshed while assembling the call table and used only during a
        // synchronous callback.
        self.appearance = Some(appearance as *mut RuntimeAppearanceBridge);
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeVegetationCall {
            state: std::mem::take(&mut self.state),
            frames: Vec::new(),
            early: Vec::new(),
        });
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeVegetationCall, CsharpEngineServicesError> {
        self.staged
            .take()
            .ok_or_else(|| refused("vegetation was read outside a product call"))
    }

    pub(crate) fn commit_call(&mut self, call: RuntimeVegetationCall) {
        self.state = call.state;
    }

    fn staged_mut(&mut self) -> Result<&mut RuntimeVegetationCall, CsharpEngineServicesError> {
        self.staged
            .as_mut()
            .ok_or_else(|| refused("vegetation was called outside a product call"))
    }

    fn appearance_mut(
        &mut self,
    ) -> Result<&mut RuntimeAppearanceBridge, CsharpEngineServicesError> {
        let appearance = self
            .appearance
            .ok_or_else(|| refused("vegetation has no Engine appearance owner"))?;
        // SAFETY: bind_appearance points at the sibling owner retained by
        // EngineServiceSet; callbacks do not retain it beyond this call.
        Ok(unsafe { &mut *appearance })
    }

    fn retained(
        &mut self,
        handle: NativeVegetationFieldHandle,
    ) -> Result<(&mut RetainedField, &mut VegetationFields), CsharpEngineServicesError> {
        let state = &mut self.staged_mut()?.state;
        let field = state
            .fields
            .get_mut(&handle.value)
            .ok_or_else(|| refused("the vegetation field does not live"))?;
        Ok((field, &mut state.tiles))
    }

    fn readout(
        &mut self,
        handle: NativeVegetationFieldHandle,
    ) -> Result<NativeVegetationReadout, CsharpEngineServicesError> {
        let (_, tiles) = self.retained(handle)?;
        Ok(native_readout(
            tiles.readout(handle.value).unwrap_or_default(),
        ))
    }

    fn create(
        &mut self,
        request: NativeVegetationFieldCreateRequest,
    ) -> Result<NativeVegetationFieldHandle, CsharpEngineServicesError> {
        let session = request.session.session;
        if session.value != 0 && self.spatial.world_origin(session).is_none() {
            return Err(refused(
                "the vegetation field names a spatial session that does not live",
            ));
        }
        let focus = focus_of(request.focus)?;
        let staged = self.staged_mut()?;
        let value = staged.state.next;
        staged
            .state
            .tiles
            .create(
                value,
                VegetationLayout {
                    tile_size: request.tile_size,
                    tile_cells: request.tile_cells,
                },
                request.placements_per_call,
            )
            .map_err(|_| {
                refused(
                    "a vegetation tile is finite and above 0 metres, with 1 to 256 cells a side",
                )
            })?;
        staged.state.next += 1;
        staged.state.fields.insert(
            value,
            RetainedField {
                session: session.value,
                focus,
                layers: BTreeMap::new(),
            },
        );
        Ok(NativeVegetationFieldHandle { value })
    }

    fn destroy(
        &mut self,
        handle: NativeVegetationFieldHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let Some(retained) = staged.state.fields.remove(&handle.value) else {
            return Ok(());
        };
        let removals = staged.state.tiles.destroy(handle.value);
        staged.early.extend(removals);
        let appearance = self.appearance_mut()?;
        for held in retained.layers.values().flat_map(RetainedLayer::held) {
            appearance.hold_for_scatter(held, false)?;
        }
        Ok(())
    }

    fn set_focus(
        &mut self,
        request: NativeVegetationFocusRequest,
    ) -> Result<NativeVegetationReadout, CsharpEngineServicesError> {
        let focus = focus_of(request.focus)?;
        self.retained(request.field)?.0.focus = focus;
        self.readout(request.field)
    }

    /// A mesh appearance's static mesh, and the overrides that draw every
    /// slot with `material` (defined as `id`), or with the mesh's own
    /// bindings for a default material; the operations defining it.
    fn projected(
        &mut self,
        appearance: NativeAppearanceHandle,
        material: NativeOptionalMaterial,
        id: String,
    ) -> Result<
        (String, Vec<render_model::MeshMaterialSlot>, Vec<RenderDiff>),
        CsharpEngineServicesError,
    > {
        let bridge = self.appearance_mut()?;
        let (mesh, slots) = bridge.scatter_mesh(appearance)?;
        if material.material.value == 0 {
            return Ok((mesh, Vec::new(), Vec::new()));
        }
        let (mut material, textures, shader) =
            bridge.voxel_material_projection(material.material)?;
        material.id = id;
        let overrides = slots
            .into_iter()
            .map(|slot| render_model::MeshMaterialSlot {
                slot,
                material: material.id.clone(),
            })
            .collect();
        let operations = textures
            .into_iter()
            .map(|texture| RenderDiff::DefineTexture { texture })
            .chain(shader.map(|shader| RenderDiff::DefineShader { shader }))
            .chain(std::iter::once(RenderDiff::DefineMaterial { material }))
            .collect();
        Ok((mesh, overrides, operations))
    }

    /// Give a layer to the field's tiles, its definitions to the call's
    /// frames, and keep it; the appearances it no longer holds are released.
    fn keep_layer(
        &mut self,
        field: NativeVegetationFieldHandle,
        key: u32,
        retained_layer: RetainedLayer,
        operations: Vec<RenderDiff>,
    ) -> Result<NativeVegetationReadout, CsharpEngineServicesError> {
        let layer = retained_layer.layer();
        layer
            .validate()
            .map_err(|error| refused(format!("the vegetation layer is invalid: {error:?}")))?;
        let appearance = self.appearance_mut()?;
        for held in retained_layer.held() {
            appearance.hold_for_scatter(held, true)?;
        }
        let staged = self.staged_mut()?;
        if !operations.is_empty() {
            staged.frames.push(RenderFrameDiff::from_ops(operations));
        }
        staged
            .state
            .tiles
            .set_layer(field.value, key, layer)
            .map_err(vegetation_refusal)?;
        let replaced = staged
            .state
            .fields
            .get_mut(&field.value)
            .expect("the field was checked")
            .layers
            .insert(key, retained_layer);
        if let Some(replaced) = replaced {
            let appearance = self.appearance_mut()?;
            for held in replaced.held() {
                appearance.hold_for_scatter(held, false)?;
            }
        }
        self.readout(field)
    }

    fn set_layer(
        &mut self,
        request: NativeVegetationLayerRequest,
    ) -> Result<NativeVegetationReadout, CsharpEngineServicesError> {
        let existing = self
            .retained(request.field)?
            .0
            .layers
            .get(&request.layer)
            .cloned();
        let (mesh, material_overrides, operations) = self.projected(
            request.appearance,
            request.material,
            layer_material_id(request.field.value, request.layer, None),
        )?;
        let base = VegetationLayer {
            mesh,
            material_overrides,
            shadow_casting: if request.casts_shadows {
                ShadowCasting::Cast
            } else {
                ShadowCasting::None
            },
            density: request.density,
            grounds: request.grounds,
            slope_limit_degrees: request.slope_limit_degrees,
            align: request.align,
            scale: [request.scale_min, request.scale_max],
            tints: [
                [request.tint_low.x, request.tint_low.y, request.tint_low.z],
                [
                    request.tint_high.x,
                    request.tint_high.y,
                    request.tint_high.z,
                ],
            ],
            // As a voxel scene scatter keyed the same draws its spots.
            seed: (u64::from(request.seed) << 32) | u64::from(request.layer),
            reach: request.reach,
            fade: request.fade,
            near: request.near,
            near_fade: request.near_fade,
            maximum_instances: request.maximum_instances,
            lods: Vec::new(),
            lod_band: 0.0,
            thinning: Default::default(),
        };
        // Setting a layer again keeps its levels and thinning.
        let retained_layer = match existing {
            Some(existing) => RetainedLayer {
                base,
                appearance: request.appearance.value,
                ..existing
            },
            None => RetainedLayer {
                base,
                lods: Vec::new(),
                lod_band: 0.0,
                thinning: Default::default(),
                appearance: request.appearance.value,
                lod_appearances: Vec::new(),
            },
        };
        self.keep_layer(request.field, request.layer, retained_layer, operations)
    }

    fn set_layer_detail(
        &mut self,
        request: &NativeVegetationLayerDetailRequest,
    ) -> Result<NativeVegetationReadout, CsharpEngineServicesError> {
        // SAFETY: generated C# pins these bounded typed arrays for the
        // direct callback; they are copied before returning.
        let levels =
            unsafe { borrowed_slice(request.lods, request.lods_len, "vegetation levels")? };
        let points = unsafe {
            borrowed_slice(
                request.thinning,
                request.thinning_len,
                "vegetation thinning",
            )?
        };
        let Some(existing) = self
            .retained(request.field)?
            .0
            .layers
            .get(&request.layer)
            .cloned()
        else {
            return Err(refused("set the vegetation layer before its detail"));
        };
        let mut operations = Vec::new();
        let mut lods = Vec::with_capacity(levels.len());
        for (index, level) in levels.iter().enumerate() {
            let (asset, material_overrides, defined) = self.projected(
                level.appearance,
                level.material,
                layer_material_id(request.field.value, request.layer, Some(index)),
            )?;
            operations.extend(defined);
            lods.push(render_model::ScatterLod {
                asset,
                material_overrides,
                start: level.start,
                impostor: level.impostor,
            });
        }
        let retained_layer = RetainedLayer {
            lods,
            lod_band: request.lod_band,
            thinning: render_model::ScatterThinning {
                points: points
                    .iter()
                    .map(|point| [point.distance, point.share])
                    .collect(),
                max_scale: request.max_scale,
            },
            lod_appearances: levels.iter().map(|level| level.appearance.value).collect(),
            ..existing
        };
        self.keep_layer(request.field, request.layer, retained_layer, operations)
    }

    fn remove_layer(
        &mut self,
        request: NativeVegetationLayerRemoval,
    ) -> Result<NativeVegetationReadout, CsharpEngineServicesError> {
        let (retained, tiles) = self.retained(request.field)?;
        let removed = retained.layers.remove(&request.layer);
        let removals = tiles
            .remove_layer(request.field.value, request.layer)
            .map_err(vegetation_refusal)?;
        self.staged_mut()?.early.extend(removals);
        if let Some(removed) = removed {
            let appearance = self.appearance_mut()?;
            for held in removed.held() {
                appearance.hold_for_scatter(held, false)?;
            }
        }
        self.readout(request.field)
    }

    fn read_requests(
        &mut self,
        request: NativeVegetationRequestsRequest,
    ) -> Result<NativeVegetationRequestsResult, CsharpEngineServicesError> {
        let (_, tiles) = self.retained(request.field)?;
        let layout = tiles
            .layout(request.field.value)
            .expect("a live field has tiles");
        let requests: Vec<NativeVegetationTileRequest> = tiles
            .requests(request.field.value, request.max_requests as usize)
            .into_iter()
            .map(|waiting| NativeVegetationTileRequest {
                tile_cells: layout.tile_cells,
                x: waiting.x,
                z: waiting.z,
                revision: waiting.revision,
                origin_x: waiting.origin[0],
                origin_z: waiting.origin[1],
                cell_size: layout.tile_size / f64::from(layout.tile_cells),
            })
            .collect();
        let result = NativeVegetationRequestsResult {
            requests: requests.as_ptr(),
            requests_len: requests.len(),
        };
        self.borrowed.hold(requests);
        Ok(result)
    }

    fn commit_tile(
        &mut self,
        request: &NativeVegetationTileCommit,
    ) -> Result<NativeVegetationCommitReceipt, CsharpEngineServicesError> {
        // SAFETY: generated C# pins these bounded typed arrays for the
        // direct callback; they are copied before returning.
        let heights =
            unsafe { borrowed_slice(request.heights, request.heights_len, "vegetation heights")? };
        let grounds =
            unsafe { borrowed_slice(request.grounds, request.grounds_len, "vegetation grounds")? };
        let cover =
            unsafe { borrowed_slice(request.cover, request.cover_len, "vegetation cover")? };
        let records =
            unsafe { borrowed_slice(request.records, request.records_len, "vegetation records")? };
        let content = VegetationTileContent {
            samples: TileSamples {
                heights: heights.to_vec(),
                colors: None,
                layer_weights: None,
                materials: (!grounds.is_empty()).then(|| grounds.to_vec()),
            },
            cover: (!cover.is_empty()).then(|| cover.to_vec()),
            records: records
                .iter()
                .map(|record| VegetationRecord {
                    layer: record.layer,
                    position: [record.position.x, record.position.y, record.position.z],
                    yaw: record.yaw,
                    scale: record.scale,
                })
                .collect(),
        };
        let (_, tiles) = self.retained(request.field)?;
        let accepted = tiles
            .commit(
                request.field.value,
                (request.x, request.z),
                request.revision,
                content,
            )
            .map_err(vegetation_refusal)?;
        Ok(NativeVegetationCommitReceipt { accepted })
    }

    fn invalidate(
        &mut self,
        request: NativeVegetationInvalidateRequest,
    ) -> Result<NativeVegetationReadout, CsharpEngineServicesError> {
        let bounds = [request.min_x, request.min_z, request.max_x, request.max_z];
        if !bounds.iter().all(|value| value.is_finite())
            || request.min_x > request.max_x
            || request.min_z > request.max_z
        {
            return Err(refused(
                "an invalidated rectangle must be finite, its minimum at most its maximum",
            ));
        }
        let (_, tiles) = self.retained(request.field)?;
        tiles
            .invalidate(
                request.field.value,
                [request.min_x, request.min_z],
                [request.max_x, request.max_z],
            )
            .map_err(vegetation_refusal)?;
        self.readout(request.field)
    }

    /// As a call finishes: resolve every field's place (a rebase moves its
    /// root) and focus (`camera` gives a camera's position in the scene),
    /// then place, choose and project.
    pub(crate) fn settle(
        &mut self,
        call: &mut RuntimeVegetationCall,
        camera: impl Fn(u64) -> Option<[f64; 3]>,
    ) -> Result<(), CsharpEngineServicesError> {
        let state = &mut call.state;
        let mut operations = Vec::new();
        for (handle, retained) in &state.fields {
            let placement = retained.placement(&self.spatial);
            let focus = match retained.focus {
                Focus::Camera(camera_handle) => camera(camera_handle)
                    .map(|eye| std::array::from_fn(|axis| eye[axis] - placement[axis])),
                Focus::Point(point) => Some(point),
            };
            operations.extend(
                state
                    .tiles
                    .settle(*handle, placement, focus)
                    .map_err(vegetation_refusal)?,
            );
        }
        if !operations.is_empty() {
            call.frames.push(RenderFrameDiff::from_ops(operations));
        }
        Ok(())
    }
}

/// The material a layer (or its level `lod`) draws every slot with.
fn layer_material_id(field: u64, layer: u32, lod: Option<usize>) -> String {
    match lod {
        None => format!("material/vegetation-{field}-{layer}"),
        Some(lod) => format!("material/vegetation-{field}-{layer}-lod{lod}"),
    }
}

fn focus_of(focus: NativeVegetationFocus) -> Result<Focus, CsharpEngineServicesError> {
    if focus.camera.camera.value != 0 {
        return Ok(Focus::Camera(focus.camera.camera.value));
    }
    if !(focus.x.is_finite() && focus.y.is_finite() && focus.z.is_finite()) {
        return Err(refused("a vegetation field's focus must be finite"));
    }
    Ok(Focus::Point([focus.x, focus.y, focus.z]))
}

fn native_readout(readout: VegetationReadout) -> NativeVegetationReadout {
    NativeVegetationReadout {
        tiles: readout.tiles,
        pending_tiles: readout.pending_tiles,
        patches: readout.patches,
        instances: readout.instances,
        over_budget: readout.over_budget,
        deferred: readout.deferred,
        placed: readout.placed,
        placed_instances: readout.placed_instances,
        placement_microseconds: readout.placement_microseconds,
        selection_microseconds: readout.selection_microseconds,
        publication_microseconds: readout.publication_microseconds,
        invalidated_tiles: readout.invalidated_tiles,
        replaced_patches: readout.replaced_patches,
        sample_bytes: readout.sample_bytes,
        settles_skipped: readout.settles_skipped,
        settles_reselected: readout.settles_reselected,
    }
}

fn vegetation_refusal(error: VegetationError) -> CsharpEngineServicesError {
    refused(match error {
        VegetationError::Layout => {
            "a vegetation tile is finite and above 0 metres, with 1 to 256 cells a side".to_owned()
        }
        VegetationError::Field => "the vegetation field does not live".to_owned(),
        VegetationError::SampleCount { expected, actual } => format!(
            "a tile has {expected} heights (tile cells + 3, squared) or grounds and cover \
             (tile cells + 1, squared), not {actual}"
        ),
        VegetationError::Streams => "a vegetation tile has no colours or layer weights".to_owned(),
        VegetationError::Value => "heights are finite or NaN, cover 0 to 1, ground classes 0 to \
             63, and records finite with a scale above 0"
            .to_owned(),
        VegetationError::Handle(error) => format!("vegetation node: {error:?}"),
    })
}

pub(crate) fn api(
    bridge: &mut RuntimeVegetationBridge,
    appearance: &mut RuntimeAppearanceBridge,
) -> NativeVegetationApi {
    bridge.bind_appearance(appearance);
    NativeVegetationApi {
        context: (bridge as *mut RuntimeVegetationBridge).cast(),
        create,
        destroy,
        set_focus,
        set_layer,
        set_layer_detail,
        remove_layer,
        read_requests,
        commit_tile,
        invalidate,
        read,
    }
}

/// Runs one operation: clears the receipt, then writes the result or the
/// refusal.
unsafe fn operate<T>(
    context: *mut c_void,
    output: *mut T,
    error: *mut NativeOperationErrorReceipt,
    operation: impl FnOnce(&mut RuntimeVegetationBridge) -> Result<T, CsharpEngineServicesError>,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVegetationBridge>() };
    match operation(bridge) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(failure) => {
            bridge.operation_diagnostics.retain(&failure, error);
            0
        }
    }
}

unsafe extern "C" fn create(
    context: *mut c_void,
    request: NativeVegetationFieldCreateRequest,
    result: *mut NativeVegetationFieldHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, result, error, |bridge| bridge.create(request)) }
}

unsafe extern "C" fn destroy(
    context: *mut c_void,
    field: NativeVegetationFieldHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVegetationBridge>() };
    match bridge.destroy(field) {
        Ok(()) => ABI_OK,
        Err(failure) => refuse(&failure, error),
    }
}

unsafe extern "C" fn set_focus(
    context: *mut c_void,
    request: NativeVegetationFocusRequest,
    readout: *mut NativeVegetationReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.set_focus(request)) }
}

unsafe extern "C" fn set_layer(
    context: *mut c_void,
    request: NativeVegetationLayerRequest,
    readout: *mut NativeVegetationReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.set_layer(request)) }
}

unsafe extern "C" fn set_layer_detail(
    context: *mut c_void,
    request: *const NativeVegetationLayerDetailRequest,
    readout: *mut NativeVegetationReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe {
        operate(context, readout, error, |bridge| {
            bridge.set_layer_detail(&*request)
        })
    }
}

unsafe extern "C" fn remove_layer(
    context: *mut c_void,
    request: NativeVegetationLayerRemoval,
    readout: *mut NativeVegetationReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        operate(context, readout, error, |bridge| {
            bridge.remove_layer(request)
        })
    }
}

unsafe extern "C" fn read_requests(
    context: *mut c_void,
    request: NativeVegetationRequestsRequest,
    result: *mut NativeVegetationRequestsResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        operate(context, result, error, |bridge| {
            bridge.read_requests(request)
        })
    }
}

unsafe extern "C" fn commit_tile(
    context: *mut c_void,
    request: *const NativeVegetationTileCommit,
    receipt: *mut NativeVegetationCommitReceipt,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe {
        operate(context, receipt, error, |bridge| {
            bridge.commit_tile(&*request)
        })
    }
}

unsafe extern "C" fn invalidate(
    context: *mut c_void,
    request: NativeVegetationInvalidateRequest,
    readout: *mut NativeVegetationReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.invalidate(request)) }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    field: NativeVegetationFieldHandle,
    readout: *mut NativeVegetationReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.readout(field)) }
}

#[cfg(test)]
#[path = "vegetation_tests.rs"]
mod tests;
