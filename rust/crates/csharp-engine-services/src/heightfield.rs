//! Heightfield terrain (#9842): the product samples tiles of its height
//! field when the Engine asks; the Engine keeps their residency, meshes them
//! (in parallel, as each call finishes), draws them in rings about a focus
//! with every ring geomorphing onto the next, and picks the drawn surface.
//! The mechanism is `svc_mesh::heightfield_tiles`; nodes are projected by
//! `render_projection::HeightfieldProjector`.

use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use render_model::{RenderFrameDiff, RenderLayer, ShadowCasting};
use render_projection::{HeightfieldPlacement, HeightfieldProjector};
use svc_mesh::heightfield::{
    HeightfieldLayout, HeightfieldLayoutError, HeightfieldRingSpec, TileKey, TileSamples,
    TileSamplesError,
};
use svc_mesh::heightfield_tiles::HeightfieldTiles;

use crate::{
    appearance::RuntimeAppearanceBridge,
    composition::{borrowed_slice, ABI_OK},
    operation_diagnostics::{clear_receipt, refuse, BorrowedResult},
    spatial::SpatialCollisionSource,
    CsharpEngineServicesError,
};

fn refused(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_HEIGHTFIELD", message)
}

/// What the rings centre on: a camera (by handle), or a point of the
/// heightfield.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    Camera(u64),
    Point([f64; 2]),
}

#[derive(Debug, Clone)]
struct RetainedHeightfield {
    tiles: HeightfieldTiles,
    /// Material index → (material handle, catalog id).
    materials: BTreeMap<u32, (u64, String)>,
    layer: RenderLayer,
    shadow_casting: ShadowCasting,
    /// The spatial session the anchor resolves in (0: none, `origin` as given).
    session: u64,
    anchor: core_space::GlobalPosition,
    origin: [f64; 3],
    scale: f64,
    focus: Focus,
    /// Drawn (`SetVisible`); hidden, a camera focus holds where it was.
    visible: bool,
}

impl RetainedHeightfield {
    /// Where the heightfield's (0, 0, 0) stands in its layer now.
    fn placement(&self, spatial: &SpatialCollisionSource) -> [f64; 3] {
        if self.session == 0 {
            return self.origin;
        }
        let origin = spatial
            .world_origin(NativeSpatialSessionHandle {
                value: self.session,
            })
            .unwrap_or(core_space::WorldOrigin::ZERO);
        let cell = origin.cell();
        std::array::from_fn(|axis| {
            (i128::from(self.anchor.cell()[axis]) - i128::from(cell[axis])) as f64
                + self.anchor.offset()[axis]
        })
    }

    /// The focus in heightfield units, from a layer point.
    fn heightfield_point(&self, placement: [f64; 3], layer_point: [f64; 3]) -> [f64; 2] {
        [
            (layer_point[0] - placement[0]) / self.scale,
            (layer_point[2] - placement[2]) / self.scale,
        ]
    }

    fn materials(&self) -> BTreeMap<u32, String> {
        self.materials
            .iter()
            .map(|(index, (_, id))| (*index, id.clone()))
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
struct HeightfieldState {
    heightfields: BTreeMap<u64, RetainedHeightfield>,
    projector: HeightfieldProjector,
    next: u64,
}

pub(crate) struct RuntimeHeightfieldCall {
    state: HeightfieldState,
    pub(crate) frames: Vec<RenderFrameDiff>,
    /// Removals of nodes and meshes made before this call, applied ahead of
    /// the call's Graphics work: a material a heightfield held may be
    /// released in the same call.
    pub(crate) early: Vec<render_model::RenderDiff>,
}

pub(crate) struct RuntimeHeightfieldBridge {
    spatial: SpatialCollisionSource,
    state: HeightfieldState,
    staged: Option<RuntimeHeightfieldCall>,
    appearance: Option<*mut RuntimeAppearanceBridge>,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
    borrowed: BorrowedResult,
}

impl RuntimeHeightfieldBridge {
    pub(crate) fn new(spatial: SpatialCollisionSource) -> Self {
        Self {
            spatial,
            state: HeightfieldState {
                next: 1,
                ..HeightfieldState::default()
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
        let mut state = std::mem::take(&mut self.state);
        state.projector.begin_call();
        self.staged = Some(RuntimeHeightfieldCall {
            state,
            frames: Vec::new(),
            early: Vec::new(),
        });
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeHeightfieldCall, CsharpEngineServicesError> {
        self.staged
            .take()
            .ok_or_else(|| refused("heightfields were read outside a product call"))
    }

    pub(crate) fn commit_call(&mut self, call: RuntimeHeightfieldCall) {
        self.state = call.state;
    }

    fn staged_mut(&mut self) -> Result<&mut RuntimeHeightfieldCall, CsharpEngineServicesError> {
        self.staged
            .as_mut()
            .ok_or_else(|| refused("heightfields were called outside a product call"))
    }

    fn appearance_mut(
        &mut self,
    ) -> Result<&mut RuntimeAppearanceBridge, CsharpEngineServicesError> {
        let appearance = self
            .appearance
            .ok_or_else(|| refused("heightfields have no Engine appearance owner"))?;
        // SAFETY: bind_appearance points at the sibling owner retained by
        // EngineServiceSet; callbacks do not retain it beyond this call.
        Ok(unsafe { &mut *appearance })
    }

    fn retained(
        &mut self,
        handle: NativeHeightfieldHandle,
    ) -> Result<&mut RetainedHeightfield, CsharpEngineServicesError> {
        self.staged_mut()?
            .state
            .heightfields
            .get_mut(&handle.value)
            .ok_or_else(|| refused("the heightfield does not live"))
    }

    fn create(
        &mut self,
        request: &NativeHeightfieldCreateRequest,
    ) -> Result<NativeHeightfieldHandle, CsharpEngineServicesError> {
        let rings =
            unsafe { borrowed_slice(request.rings, request.rings_len, "heightfield rings")? };
        let bindings = unsafe {
            borrowed_slice(
                request.materials,
                request.materials_len,
                "heightfield materials",
            )?
        };
        let specs: Vec<HeightfieldRingSpec> = rings
            .iter()
            .map(|ring| HeightfieldRingSpec {
                cell_size: ring.cell_size,
                reach: ring.reach,
            })
            .collect();
        let layout = HeightfieldLayout::new(request.tile_cells, &specs).map_err(layout_refusal)?;
        let layer = match request.layer {
            NativeRenderLayer::Scene => RenderLayer::Scene,
            NativeRenderLayer::Backdrop => RenderLayer::Backdrop,
            _ => {
                return Err(refused(
                    "a heightfield draws in the Scene or the Backdrop layer",
                ))
            }
        };
        let anchor = core_space::GlobalPosition::new(
            [
                request.anchor.cell_x,
                request.anchor.cell_y,
                request.anchor.cell_z,
            ],
            [
                request.anchor.offset_x,
                request.anchor.offset_y,
                request.anchor.offset_z,
            ],
        )
        .map_err(|_| refused("the heightfield's anchor must be a finite world position"))?;
        let origin = [request.origin.x, request.origin.y, request.origin.z].map(f64::from);
        let scale = f64::from(request.scale);
        if !origin.iter().all(|value| value.is_finite()) || !scale.is_finite() || scale <= 0.0 {
            return Err(refused(
                "the heightfield's origin must be finite and its scale above 0",
            ));
        }
        let session = request.session.session;
        if session.value != 0 && self.spatial.world_origin(session).is_none() {
            return Err(refused(
                "the heightfield's anchor names a spatial session that does not live",
            ));
        }
        let focus = focus_of(request.focus)?;
        if bindings.is_empty() {
            return Err(refused(
                "a heightfield needs a material for index 0 at least",
            ));
        }
        let mut indices = std::collections::BTreeSet::new();
        if !bindings.iter().all(|binding| indices.insert(binding.index)) {
            return Err(refused("a heightfield binds each material index once"));
        }
        let appearance = self.appearance_mut()?;
        let mut materials = BTreeMap::new();
        for binding in bindings {
            match appearance.hold_for_engine_surface(binding.material.value, true) {
                Ok(id) => {
                    materials.insert(binding.index, (binding.material.value, id));
                }
                Err(error) => {
                    for (handle, _) in materials.values() {
                        let _ = appearance.hold_for_engine_surface(*handle, false);
                    }
                    return Err(error);
                }
            }
        }
        let shadow_casting = match request.shadow_casting {
            NativeShadowCasting::Cast => ShadowCasting::Cast,
            NativeShadowCasting::None => ShadowCasting::None,
        };
        let mut retained = RetainedHeightfield {
            tiles: HeightfieldTiles::new(layout),
            materials,
            layer,
            shadow_casting,
            session: session.value,
            anchor,
            origin,
            scale,
            focus,
            visible: true,
        };
        let spatial = self.spatial.clone();
        let placement = retained.placement(&spatial);
        if let Focus::Point(point) = focus {
            retained.tiles.set_focus(Some(point));
        }
        let staged = self.staged_mut()?;
        let value = staged.state.next;
        staged.state.next += 1;
        let operations = staged
            .state
            .projector
            .place(value, render_placement(&retained, placement))
            .map_err(|error| refused(format!("heightfield node: {error:?}")))?;
        push_frame(&mut staged.frames, operations)?;
        staged.state.heightfields.insert(value, retained);
        Ok(NativeHeightfieldHandle { value })
    }

    fn destroy(
        &mut self,
        handle: NativeHeightfieldHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let Some(retained) = staged.state.heightfields.remove(&handle.value) else {
            return Ok(());
        };
        let removals = staged.state.projector.remove(handle.value);
        staged.early.extend(removals.early);
        push_frame(&mut staged.frames, removals.late)?;
        let appearance = self.appearance_mut()?;
        for (material, _) in retained.materials.values() {
            appearance.hold_for_engine_surface(*material, false)?;
        }
        Ok(())
    }

    fn set_focus(
        &mut self,
        request: NativeHeightfieldFocusRequest,
    ) -> Result<NativeHeightfieldReadout, CsharpEngineServicesError> {
        let focus = focus_of(request.focus)?;
        let retained = self.retained(request.heightfield)?;
        retained.focus = focus;
        // A camera focus is measured as the call finishes.
        if let Focus::Point(point) = focus {
            retained.tiles.set_focus(Some(point));
        }
        Ok(readout(retained))
    }

    fn set_visible(
        &mut self,
        request: NativeHeightfieldVisibilityRequest,
    ) -> Result<NativeHeightfieldReadout, CsharpEngineServicesError> {
        let retained = self.retained(request.heightfield)?;
        // The node shows or hides as the call settles.
        retained.visible = request.visible;
        Ok(readout(retained))
    }

    fn read_requests(
        &mut self,
        request: NativeHeightfieldRequestsRequest,
    ) -> Result<NativeHeightfieldRequestsResult, CsharpEngineServicesError> {
        let retained = self.retained(request.heightfield)?;
        let layout = retained.tiles.layout();
        let requests: Vec<NativeHeightfieldTileRequest> = retained
            .tiles
            .requests(request.max_requests as usize)
            .into_iter()
            .map(|waiting| {
                let origin = layout.origin(waiting.key);
                NativeHeightfieldTileRequest {
                    tile_cells: layout.tile_cells,
                    ring: u32::from(waiting.key.ring),
                    x: waiting.key.x,
                    z: waiting.key.z,
                    revision: waiting.revision,
                    origin_x: origin[0],
                    origin_z: origin[1],
                    cell_size: layout.rings[usize::from(waiting.key.ring)].cell_size,
                }
            })
            .collect();
        let result = NativeHeightfieldRequestsResult {
            requests: requests.as_ptr(),
            requests_len: requests.len(),
        };
        self.borrowed.hold(requests);
        Ok(result)
    }

    fn commit_tile(
        &mut self,
        request: &NativeHeightfieldTileCommit,
    ) -> Result<NativeHeightfieldCommitReceipt, CsharpEngineServicesError> {
        let heights =
            unsafe { borrowed_slice(request.heights, request.heights_len, "heightfield heights")? };
        let colors =
            unsafe { borrowed_slice(request.colors, request.colors_len, "heightfield colours")? };
        let weights = unsafe {
            borrowed_slice(
                request.layer_weights,
                request.layer_weights_len,
                "heightfield layer weights",
            )?
        };
        let materials = unsafe {
            borrowed_slice(
                request.materials,
                request.materials_len,
                "heightfield materials",
            )?
        };
        let retained = self.retained(request.heightfield)?;
        let rings = retained.tiles.layout().rings.len();
        let ring = u8::try_from(request.ring)
            .ok()
            .filter(|ring| usize::from(*ring) < rings)
            .ok_or_else(|| refused("the tile's ring is not one of the heightfield's"))?;
        if let Some(index) = materials
            .iter()
            .find(|index| !retained.materials.contains_key(*index))
        {
            return Err(refused(format!(
                "the tile uses material index {index}, which the heightfield does not bind"
            )));
        }
        let samples = TileSamples {
            heights: heights.to_vec(),
            colors: (!colors.is_empty())
                .then(|| colors.iter().map(|c| [c.r, c.g, c.b, c.a]).collect()),
            layer_weights: (!weights.is_empty())
                .then(|| weights.iter().map(|w| [w.x, w.y, w.z, w.w]).collect()),
            materials: (!materials.is_empty()).then(|| materials.to_vec()),
        };
        samples
            .validate(retained.tiles.layout())
            .map_err(samples_refusal)?;
        let key = TileKey {
            ring,
            x: request.x,
            z: request.z,
        };
        Ok(NativeHeightfieldCommitReceipt {
            accepted: retained.tiles.commit(key, request.revision, samples),
        })
    }

    fn invalidate(
        &mut self,
        request: NativeHeightfieldInvalidateRequest,
    ) -> Result<NativeHeightfieldReadout, CsharpEngineServicesError> {
        let bounds = [request.min_x, request.min_z, request.max_x, request.max_z];
        if !bounds.iter().all(|value| value.is_finite())
            || request.min_x > request.max_x
            || request.min_z > request.max_z
        {
            return Err(refused(
                "an invalidated rectangle must be finite, its minimum at most its maximum",
            ));
        }
        let retained = self.retained(request.heightfield)?;
        retained.tiles.invalidate(
            [request.min_x, request.min_z],
            [request.max_x, request.max_z],
        );
        Ok(readout(retained))
    }

    fn pick(
        &mut self,
        request: NativeHeightfieldPickRequest,
    ) -> Result<NativeHeightfieldPickResult, CsharpEngineServicesError> {
        let origin = [request.origin.x, request.origin.y, request.origin.z].map(f64::from);
        let direction = [
            request.direction.x,
            request.direction.y,
            request.direction.z,
        ]
        .map(f64::from);
        let length = direction
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        if !origin
            .iter()
            .chain(&direction)
            .all(|value| value.is_finite())
            || length <= 0.0
            || !request.max_distance.is_finite()
            || request.max_distance < 0.0
        {
            return Err(refused(
                "a pick needs a finite origin, a nonzero direction and a distance of 0 or more",
            ));
        }
        let spatial = self.spatial.clone();
        let retained = self.retained(request.heightfield)?;
        let placement = retained.placement(&spatial);
        let scale = retained.scale;
        let local = [
            (origin[0] - placement[0]) / scale,
            (origin[1] - placement[1]) / scale,
            (origin[2] - placement[2]) / scale,
        ];
        let toward = direction.map(|value| value / scale);
        let Some((key, hit)) = retained.tiles.pick(local, toward) else {
            return Ok(NativeHeightfieldPickResult::default());
        };
        let distance = hit.distance * length;
        if request.max_distance > 0.0 && distance > f64::from(request.max_distance) {
            return Ok(NativeHeightfieldPickResult::default());
        }
        let vector = |value: [f64; 3]| NativeVec3 {
            x: value[0] as f32,
            y: value[1] as f32,
            z: value[2] as f32,
        };
        Ok(NativeHeightfieldPickResult {
            hit: true,
            position: vector(std::array::from_fn(|axis| {
                origin[axis] + direction[axis] * hit.distance
            })),
            normal: vector(hit.normal),
            distance: distance as f32,
            x: hit.point[0],
            z: hit.point[2],
            height: hit.point[1],
            ring: u32::from(key.ring),
        })
    }

    fn read(
        &mut self,
        handle: NativeHeightfieldHandle,
    ) -> Result<NativeHeightfieldReadout, CsharpEngineServicesError> {
        Ok(readout(self.retained(handle)?))
    }

    /// As a call finishes: resolve every heightfield's place (a rebase moves
    /// it) and focus (`camera` gives a camera's position in the world, or
    /// with `true` where it stands in the backdrop), mesh what waits, and
    /// project the changes.
    pub(crate) fn settle(
        &mut self,
        call: &mut RuntimeHeightfieldCall,
        camera: impl Fn(u64, bool) -> Option<[f64; 3]>,
    ) -> Result<(), CsharpEngineServicesError> {
        let state = &mut call.state;
        let mut operations = Vec::new();
        let early = &mut call.early;
        for (handle, retained) in &mut state.heightfields {
            let placement = retained.placement(&self.spatial);
            if let (Focus::Camera(camera_handle), true) = (retained.focus, retained.visible) {
                let eye = camera(camera_handle, retained.layer == RenderLayer::Backdrop);
                let point = eye.map(|eye| retained.heightfield_point(placement, eye));
                retained.tiles.set_focus(point);
            }
            retained.tiles.mesh();
            let changes = retained.tiles.take_changes();
            let projector = &mut state.projector;
            let failed = |error| refused(format!("heightfield node: {error:?}"));
            operations.extend(
                projector
                    .place(*handle, render_placement(retained, placement))
                    .map_err(failed)?,
            );
            operations.extend(projector.focus(*handle, retained.tiles.focus()));
            let mut removals = render_projection::HeightfieldRemovals::default();
            for key in changes.evicted {
                removals.extend(projector.remove_tile(*handle, key));
            }
            let materials = retained.materials();
            for key in changes.meshed {
                let origin = retained.tiles.layout().origin(key);
                match retained.tiles.mesh_of(key) {
                    Some(mesh) => operations.extend(
                        projector
                            .mesh(
                                *handle,
                                key,
                                origin,
                                mesh,
                                &materials,
                                retained.tiles.is_drawn(key),
                            )
                            .map_err(failed)?,
                    ),
                    // Samples that left nothing to draw (all holes).
                    None => removals.extend(projector.remove_tile(*handle, key)),
                }
            }
            early.extend(removals.early);
            operations.extend(removals.late);
            // A tile can be shown and hidden again within one span: each
            // takes the visibility it ends with.
            for key in changes.shown.into_iter().chain(changes.hidden) {
                operations.extend(projector.show(*handle, key, retained.tiles.is_drawn(key)));
            }
        }
        push_frame(&mut call.frames, operations)
    }
}

fn render_placement(retained: &RetainedHeightfield, placement: [f64; 3]) -> HeightfieldPlacement {
    HeightfieldPlacement {
        origin: placement.map(|value| value as f32),
        scale: retained.scale as f32,
        layer: retained.layer,
        shadow_casting: retained.shadow_casting,
        visible: retained.visible,
    }
}

fn focus_of(focus: NativeHeightfieldFocus) -> Result<Focus, CsharpEngineServicesError> {
    if focus.camera.camera.value != 0 {
        return Ok(Focus::Camera(focus.camera.camera.value));
    }
    if !(focus.x.is_finite() && focus.z.is_finite()) {
        return Err(refused("a heightfield's focus must be finite"));
    }
    Ok(Focus::Point([focus.x, focus.z]))
}

fn readout(retained: &RetainedHeightfield) -> NativeHeightfieldReadout {
    let tiles = &retained.tiles;
    let footprint = tiles.footprint();
    let focus = tiles.focus();
    NativeHeightfieldReadout {
        visible: retained.visible,
        focused: focus.is_some(),
        focus_x: focus.map_or(0.0, |focus| focus[0]),
        focus_z: focus.map_or(0.0, |focus| focus[1]),
        resident_tiles: tiles.resident() as u32,
        drawn_tiles: tiles.drawn().count() as u32,
        pending_tiles: tiles.requests(0).len() as u32,
        settled: tiles.settled(),
        vertices: footprint.vertices,
        indices: footprint.indices,
        sample_bytes: footprint.sample_bytes,
        mesh_bytes: footprint.mesh_bytes,
        mesh_microseconds: tiles.mesh_microseconds,
        meshed_tiles: tiles.meshed_tiles,
    }
}

fn push_frame(
    frames: &mut Vec<RenderFrameDiff>,
    operations: Vec<render_model::RenderDiff>,
) -> Result<(), CsharpEngineServicesError> {
    if operations.is_empty() {
        return Ok(());
    }
    frames.push(RenderFrameDiff::from_ops(operations));
    Ok(())
}

fn layout_refusal(error: HeightfieldLayoutError) -> CsharpEngineServicesError {
    refused(match error {
        HeightfieldLayoutError::RingCount => "a heightfield has 1 to 8 rings".to_owned(),
        HeightfieldLayoutError::TileCells => "a tile has 1 to 256 cells a side".to_owned(),
        HeightfieldLayoutError::Ring(ring) => {
            format!("ring {ring}'s cell size and reach must be finite and above 0")
        }
        HeightfieldLayoutError::NotNested(ring) => format!(
            "ring {ring}'s tile must be a whole number, 2 or more, of ring {}'s",
            ring - 1
        ),
        HeightfieldLayoutError::ReachTooShort(ring) => format!(
            "ring {ring} must reach farther past ring {}'s region to morph onto it",
            ring - 1
        ),
    })
}

fn samples_refusal(error: TileSamplesError) -> CsharpEngineServicesError {
    refused(match error {
        TileSamplesError::HeightCount { expected, actual } => {
            format!("a tile has {expected} heights (tile cells + 3, squared), not {actual}")
        }
        TileSamplesError::StreamCount { expected, actual } => format!(
            "a tile's colours, layer weights and materials number {expected} (tile cells + 1, squared), not {actual}"
        ),
        TileSamplesError::ColorsAndWeights => {
            "a tile has colours or layer weights, not both".to_owned()
        }
        TileSamplesError::Value => {
            "heights are finite or NaN, colours and weights 0 to 1".to_owned()
        }
    })
}

pub(crate) fn api(
    bridge: &mut RuntimeHeightfieldBridge,
    appearance: &mut RuntimeAppearanceBridge,
) -> NativeHeightfieldApi {
    bridge.bind_appearance(appearance);
    NativeHeightfieldApi {
        context: (bridge as *mut RuntimeHeightfieldBridge).cast(),
        create,
        destroy,
        set_focus,
        set_visible,
        read_requests,
        commit_tile,
        invalidate,
        pick,
        read,
    }
}

/// Runs one operation: clears the receipt, then writes the result or the
/// refusal.
unsafe fn operate<T>(
    context: *mut c_void,
    output: *mut T,
    error: *mut NativeOperationErrorReceipt,
    operation: impl FnOnce(&mut RuntimeHeightfieldBridge) -> Result<T, CsharpEngineServicesError>,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeHeightfieldBridge>() };
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
    request: *const NativeHeightfieldCreateRequest,
    result: *mut NativeHeightfieldHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe { operate(context, result, error, |bridge| bridge.create(&*request)) }
}

unsafe extern "C" fn destroy(
    context: *mut c_void,
    heightfield: NativeHeightfieldHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeHeightfieldBridge>() };
    match bridge.destroy(heightfield) {
        Ok(()) => ABI_OK,
        Err(failure) => refuse(&failure, error),
    }
}

unsafe extern "C" fn set_focus(
    context: *mut c_void,
    request: NativeHeightfieldFocusRequest,
    readout: *mut NativeHeightfieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.set_focus(request)) }
}

unsafe extern "C" fn set_visible(
    context: *mut c_void,
    request: NativeHeightfieldVisibilityRequest,
    readout: *mut NativeHeightfieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        operate(context, readout, error, |bridge| {
            bridge.set_visible(request)
        })
    }
}

unsafe extern "C" fn read_requests(
    context: *mut c_void,
    request: NativeHeightfieldRequestsRequest,
    result: *mut NativeHeightfieldRequestsResult,
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
    request: *const NativeHeightfieldTileCommit,
    receipt: *mut NativeHeightfieldCommitReceipt,
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
    request: NativeHeightfieldInvalidateRequest,
    readout: *mut NativeHeightfieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.invalidate(request)) }
}

unsafe extern "C" fn pick(
    context: *mut c_void,
    request: NativeHeightfieldPickRequest,
    result: *mut NativeHeightfieldPickResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, result, error, |bridge| bridge.pick(request)) }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    heightfield: NativeHeightfieldHandle,
    readout: *mut NativeHeightfieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.read(heightfield)) }
}

#[cfg(test)]
#[path = "heightfield_tests.rs"]
mod tests;
