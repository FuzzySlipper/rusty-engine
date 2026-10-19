use std::ffi::c_void;

use crate::*;

/// A vegetation field: grass, plants and placed things the Engine grows,
/// draws and streams in tiles over ground the product samples, with no
/// voxel or collision residency behind them.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NativeVegetationFieldHandle {
    pub value: u64,
}

/// What the tiles centre on: a camera's position, measured as each call
/// finishes, or with no camera a world point the product moves.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVegetationFocus {
    pub camera: NativeOptionalCamera,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A new field over a spatial session's world (none: the scene's own
/// space), in square tiles of `tile_size` metres sampled `tile_cells`
/// cells a side (1 to 256). At most `placements_per_call` tile layers are
/// placed each call; the rest wait, counted in the readout.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVegetationFieldCreateRequest {
    pub session: NativeOptionalSpatialSession,
    pub tile_size: f64,
    pub tile_cells: u32,
    pub placements_per_call: u32,
    pub focus: NativeVegetationFocus,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVegetationFocusRequest {
    pub field: NativeVegetationFieldHandle,
    pub focus: NativeVegetationFocus,
}

/// Draw a field or not, its copies' shadows included. A hidden field keeps
/// its tiles and holds its focus where it was, asking for and placing
/// nothing new as its camera moves, so showing it again draws at once.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVegetationVisibilityRequest {
    pub field: NativeVegetationFieldHandle,
    pub visible: bool,
}

/// One kind of thing that grows, named `layer` within the field: setting
/// it again replaces it, and its copies are placed again as calls allow,
/// drawing as they were until then. Copies stand `density` to the square
/// metre (0: only the tiles' placement records) on vertices whose ground
/// class (0 to 63) has its bit in `grounds`, no steeper than
/// `slope_limit_degrees`, on tiles within `reach` metres of the focus,
/// shrinking away over its last `fade` metres; nearer than `near` they are
/// not drawn, growing back over the next `near_fade` (a voxel scene scatter
/// with the same `layer` key, `seed` and density, fading out over that band,
/// hands its copies over one for one). Each draws `appearance` (a static
/// mesh appearance) with `material` (`default`: each mesh slot with the
/// mesh's own material) at a scale between `scale_min` and
/// `scale_max`, tinted between `tint_low` and `tint_high`, turned at random
/// about its up axis and leaning `align` of the way onto the ground's
/// normal. The nearest tiles' copies come first within
/// `maximum_instances`. The appearance and material must stay live while the
/// layer grows them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVegetationLayerRequest {
    pub field: NativeVegetationFieldHandle,
    pub layer: u32,
    pub appearance: NativeAppearanceHandle,
    pub material: NativeOptionalMaterial,
    pub density: f32,
    pub grounds: u64,
    pub slope_limit_degrees: f32,
    pub align: f32,
    pub scale_min: f32,
    pub scale_max: f32,
    /// Linear RGB.
    pub tint_low: NativeVec3,
    /// Linear RGB.
    pub tint_high: NativeVec3,
    pub seed: u32,
    pub reach: f32,
    pub fade: f32,
    pub near: f32,
    pub near_fade: f32,
    pub maximum_instances: u32,
    pub casts_shadows: bool,
}

/// A farther level of detail of a layer: from `start` metres its copies draw
/// `appearance` (a static mesh appearance) with `material` (`default`: each
/// mesh slot with the mesh's own material), crossfading with the level
/// before over the layer's `lod_band`. With `impostor`, the level is a
/// camera-facing card showing `appearance` from the nearest of the views the
/// renderer bakes of it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVegetationLod {
    pub appearance: NativeAppearanceHandle,
    pub material: NativeOptionalMaterial,
    pub start: f32,
    pub impostor: bool,
}

/// A thinning point: at `distance` metres from the camera, the share of a
/// layer's copies kept (0 to 1, above 0).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVegetationThinningPoint {
    pub distance: f32,
    pub share: f32,
}

/// A layer's farther levels of detail (at most 3, starting farther each,
/// each past the band) and its coverage-preserving thinning: up to 4 points
/// in increasing distance, the share linear between them and level beyond;
/// a kept copy grows by the inverse square root of the share, at most
/// `max_scale` (1 to 4). Replaces the layer's earlier detail; setting the
/// layer again keeps it. The levels' appearances and materials must stay
/// live while the layer grows them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVegetationLayerDetailRequest {
    pub field: NativeVegetationFieldHandle,
    pub layer: u32,
    pub lods: *const NativeVegetationLod,
    pub lods_len: usize,
    pub lod_band: f32,
    pub thinning: *const NativeVegetationThinningPoint,
    pub thinning_len: usize,
    pub max_scale: f32,
}

/// Stops growing one layer: its copies are removed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVegetationLayerRemoval {
    pub field: NativeVegetationFieldHandle,
    pub layer: u32,
}

/// The tiles waiting for samples, at most `max_requests` (0: all).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVegetationRequestsRequest {
    pub field: NativeVegetationFieldHandle,
    pub max_requests: u32,
}

/// A tile to sample, as a heightfield tile is sampled: `tile_cells + 3`
/// heights a side from (`origin_x` − `cell_size`, `origin_z` −
/// `cell_size`), and `tile_cells + 1` ground classes or cover weights a
/// side from the origin, x fastest, in world metres. Commit it with its
/// `revision`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVegetationTileRequest {
    pub tile_cells: u32,
    pub x: i64,
    pub z: i64,
    pub revision: u32,
    pub origin_x: f64,
    pub origin_z: f64,
    pub cell_size: f64,
}

/// The waiting tiles, nearest the focus first; borrowed until the next
/// Vegetation call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVegetationRequestsResult {
    pub requests: *const NativeVegetationTileRequest,
    pub requests_len: usize,
}

/// One thing the product places itself, in layer `layer`: x and z from the
/// tile's origin, y the world height, turned `yaw` radians about the
/// vertical and leaning onto the ground as its layer does.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVegetationRecord {
    pub layer: u32,
    pub position: NativeVec3,
    pub yaw: f32,
    pub scale: f32,
}

/// One tile's samples (`NativeVegetationTileRequest`): heights in world
/// metres (NaN: no ground there); optionally a ground class per vertex
/// (none: class 0), a cover weight 0 to 1 per vertex thinning every
/// layer's spots (none: 1), and placement records. A commit whose revision
/// is no longer the tile's (it was invalidated, or left reach) is ignored.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVegetationTileCommit {
    pub field: NativeVegetationFieldHandle,
    pub x: i64,
    pub z: i64,
    pub revision: u32,
    pub heights: *const f32,
    pub heights_len: usize,
    pub grounds: *const u32,
    pub grounds_len: usize,
    pub cover: *const f32,
    pub cover_len: usize,
    pub records: *const NativeVegetationRecord,
    pub records_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVegetationCommitReceipt {
    /// The tile took the samples; false for a stale revision.
    pub accepted: bool,
}

/// Every tile overlapping the rectangle, in world metres, is sampled again
/// and placed again; each keeps drawing until its new samples land.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVegetationInvalidateRequest {
    pub field: NativeVegetationFieldHandle,
    pub min_x: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_z: f64,
}

/// A field as the last call left it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVegetationReadout {
    /// Whether it is drawn (`set_visible`).
    pub visible: bool,
    /// Tiles holding samples, and tiles in reach waiting for current ones.
    pub tiles: u32,
    pub pending_tiles: u32,
    /// Patches drawn (one per tile and layer) and the copies they hold.
    pub patches: u32,
    pub instances: u64,
    /// Tile layers in reach a layer's `maximum_instances` left bare.
    pub over_budget: u32,
    /// Tile layers in reach with samples waiting for a placement.
    pub deferred: u32,
    /// The last call's placements (tile layers) and the copies they made;
    /// its time placing (on workers, end to end), choosing what is drawn,
    /// and turning that into renderer work. The renderer's upload of the
    /// copies is in the renderer's own frame readout.
    pub placed: u32,
    pub placed_instances: u64,
    pub placement_microseconds: u64,
    pub selection_microseconds: u64,
    pub publication_microseconds: u64,
    /// Since the field was made: tiles invalidated, and patches placed
    /// again on a tile's new samples.
    pub invalidated_tiles: u64,
    pub replaced_patches: u64,
    /// The samples and records held, in bytes.
    pub sample_bytes: u64,
    /// Since the field was made: calls that had nothing to choose again (no
    /// change, the focus within a sixteenth of a tile of the last choice),
    /// and calls that chose again.
    pub settles_skipped: u64,
    pub settles_reselected: u64,
}

pub type NativeCreateVegetationField = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeVegetationFieldCreateRequest,
    result: *mut NativeVegetationFieldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeDestroyVegetationField = unsafe extern "C" fn(
    context: *mut c_void,
    field: NativeVegetationFieldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetVegetationFocus = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeVegetationFocusRequest,
    readout: *mut NativeVegetationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetVegetationVisible = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeVegetationVisibilityRequest,
    readout: *mut NativeVegetationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetVegetationLayer = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeVegetationLayerRequest,
    readout: *mut NativeVegetationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetVegetationLayerDetail = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeVegetationLayerDetailRequest,
    readout: *mut NativeVegetationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeRemoveVegetationLayer = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeVegetationLayerRemoval,
    readout: *mut NativeVegetationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadVegetationRequests = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeVegetationRequestsRequest,
    result: *mut NativeVegetationRequestsResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeCommitVegetationTile = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeVegetationTileCommit,
    receipt: *mut NativeVegetationCommitReceipt,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeInvalidateVegetation = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeVegetationInvalidateRequest,
    readout: *mut NativeVegetationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadVegetation = unsafe extern "C" fn(
    context: *mut c_void,
    field: NativeVegetationFieldHandle,
    readout: *mut NativeVegetationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

/// Vegetation fields: the product samples tiles of ground and places what
/// it chooses; the Engine keeps the tiles' residency about a focus, places
/// each layer's copies within its budgets, and draws them as retained
/// instanced patches.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVegetationApi {
    pub context: *mut c_void,
    pub create: NativeCreateVegetationField,
    pub destroy: NativeDestroyVegetationField,
    pub set_focus: NativeSetVegetationFocus,
    pub set_visible: NativeSetVegetationVisible,
    pub set_layer: NativeSetVegetationLayer,
    pub set_layer_detail: NativeSetVegetationLayerDetail,
    pub remove_layer: NativeRemoveVegetationLayer,
    pub read_requests: NativeReadVegetationRequests,
    pub commit_tile: NativeCommitVegetationTile,
    pub invalidate: NativeInvalidateVegetation,
    pub read: NativeReadVegetation,
}
