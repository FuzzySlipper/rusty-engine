use std::ffi::c_void;

use crate::*;

/// A heightfield terrain: rings of product-sampled tiles about a focus,
/// meshed, drawn, geomorphed and picked by the Engine.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NativeHeightfieldHandle {
    pub value: u64,
}

/// One ring, finest first: a cell's width and how far from the focus the
/// ring reaches, in heightfield units. Each ring's tile (`tile_cells`
/// cells) must be a whole number, two or more, of the finer ring's, and it
/// must reach far enough past the finer ring to morph onto it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeHeightfieldRing {
    pub cell_size: f64,
    pub reach: f64,
}

/// The material a tile sample's material index draws with. A terrain
/// layer material (`create_terrain_layer_material`) blends its first four
/// layers by the samples' layer weights; any other draws its triangles
/// whole, tinted by linear vertex colours if the samples have them.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeHeightfieldMaterial {
    pub index: u32,
    pub material: NativeMaterialHandle,
}

/// What the rings centre on: a camera's position (in the backdrop layer,
/// where it stands in the backdrop under its link), or with no camera a
/// point (x, z) of the heightfield the product moves.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeHeightfieldFocus {
    pub camera: NativeOptionalCamera,
    pub x: f64,
    pub z: f64,
}

/// A new heightfield. Its point (0, 0, 0) stands at `origin` in `layer`
/// (Scene or Backdrop), or with a `session` at the world point `anchor`
/// resolved into that session's frame, again after every world-origin
/// rebase; one heightfield unit is `scale` layer units, and it is never
/// turned. The materials bind every material index its samples use, and
/// are held while it lives.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHeightfieldCreateRequest {
    /// Cells along a tile's side, 1 to 256.
    pub tile_cells: u32,
    pub rings: *const NativeHeightfieldRing,
    pub rings_len: usize,
    pub materials: *const NativeHeightfieldMaterial,
    pub materials_len: usize,
    pub layer: NativeRenderLayer,
    pub session: NativeOptionalSpatialSession,
    pub anchor: NativeWorldOriginGlobalPosition,
    pub origin: NativeVec3,
    pub scale: f32,
    pub focus: NativeHeightfieldFocus,
    pub shadow_casting: NativeShadowCasting,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeHeightfieldFocusRequest {
    pub heightfield: NativeHeightfieldHandle,
    pub focus: NativeHeightfieldFocus,
}

/// The tiles waiting for samples, at most `max_requests` (0: all).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeHeightfieldRequestsRequest {
    pub heightfield: NativeHeightfieldHandle,
    pub max_requests: u32,
}

/// A tile to sample: `tile_cells + 3` heights a side from (`origin_x` −
/// `cell_size`, `origin_z` − `cell_size`), and `tile_cells + 1` colours,
/// weights or material indices a side from the origin, x fastest. Commit
/// it with its `revision`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeHeightfieldTileRequest {
    pub tile_cells: u32,
    pub ring: u32,
    pub x: i64,
    pub z: i64,
    pub revision: u32,
    pub origin_x: f64,
    pub origin_z: f64,
    pub cell_size: f64,
}

/// The waiting tiles, coarsest ring first and nearest the focus first
/// within a ring; borrowed until the next Heightfield call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHeightfieldRequestsResult {
    pub requests: *const NativeHeightfieldTileRequest,
    pub requests_len: usize,
}

/// One tile's samples (`NativeHeightfieldTileRequest`). Heights are in
/// heightfield units (NaN: no surface there). Colours are linear RGBA, drawn
/// as given; layer weights are a terrain layer material's first four
/// layers; a tile has one or the other or neither. Material indices default
/// to 0. A commit whose revision is no longer the tile's (it was
/// invalidated, or left every ring) is ignored.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHeightfieldTileCommit {
    pub heightfield: NativeHeightfieldHandle,
    pub ring: u32,
    pub x: i64,
    pub z: i64,
    pub revision: u32,
    pub heights: *const f32,
    pub heights_len: usize,
    /// Linear RGBA.
    pub colors: *const NativeColor,
    pub colors_len: usize,
    pub layer_weights: *const NativeVec4,
    pub layer_weights_len: usize,
    pub materials: *const u32,
    pub materials_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeHeightfieldCommitReceipt {
    /// The tile took the samples; false for a stale revision.
    pub accepted: bool,
}

/// Every tile of every ring overlapping the rectangle, in heightfield
/// units, is sampled again; each keeps drawing until its new samples land.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeHeightfieldInvalidateRequest {
    pub heightfield: NativeHeightfieldHandle,
    pub min_x: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_z: f64,
}

/// A ray in the heightfield's layer, met with the surface as drawn now.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeHeightfieldPickRequest {
    pub heightfield: NativeHeightfieldHandle,
    pub origin: NativeVec3,
    pub direction: NativeVec3,
    /// In layer units; 0 reaches as far as the heightfield does.
    pub max_distance: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeHeightfieldPickResult {
    pub hit: bool,
    /// Where, and the surface's normal there, in the layer.
    pub position: NativeVec3,
    pub normal: NativeVec3,
    /// Along the ray, in layer units.
    pub distance: f32,
    /// Where in the heightfield, and the drawn height there.
    pub x: f64,
    pub z: f64,
    pub height: f64,
    pub ring: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeHeightfieldReadout {
    /// Where the rings centre, in heightfield units, as the last call
    /// left them; `focused` false while a camera focus has no camera.
    pub focused: bool,
    pub focus_x: f64,
    pub focus_z: f64,
    /// Tiles holding samples, and those drawn.
    pub resident_tiles: u32,
    pub drawn_tiles: u32,
    /// Tiles the rings call for that wait for samples.
    pub pending_tiles: u32,
    /// Every tile the rings call for is drawn or covered by finer ones.
    pub settled: bool,
    pub vertices: u64,
    pub indices: u64,
    /// The samples and meshes held, in bytes.
    pub sample_bytes: u64,
    pub mesh_bytes: u64,
    /// Time the last call spent meshing, summed over tiles (they mesh in
    /// parallel), and the tiles it meshed.
    pub mesh_microseconds: u64,
    pub meshed_tiles: u32,
}

pub type NativeCreateHeightfield = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeHeightfieldCreateRequest,
    result: *mut NativeHeightfieldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeDestroyHeightfield = unsafe extern "C" fn(
    context: *mut c_void,
    heightfield: NativeHeightfieldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetHeightfieldFocus = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeHeightfieldFocusRequest,
    readout: *mut NativeHeightfieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadHeightfieldRequests = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeHeightfieldRequestsRequest,
    result: *mut NativeHeightfieldRequestsResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeCommitHeightfieldTile = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeHeightfieldTileCommit,
    receipt: *mut NativeHeightfieldCommitReceipt,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeInvalidateHeightfield = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeHeightfieldInvalidateRequest,
    readout: *mut NativeHeightfieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativePickHeightfield = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeHeightfieldPickRequest,
    result: *mut NativeHeightfieldPickResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadHeightfield = unsafe extern "C" fn(
    context: *mut c_void,
    heightfield: NativeHeightfieldHandle,
    readout: *mut NativeHeightfieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

/// Heightfield terrains: the product samples tiles, the Engine keeps their
/// residency, meshes and draws them in rings about a focus with seamless
/// geomorphing, and picks the drawn surface.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeHeightfieldApi {
    pub context: *mut c_void,
    pub create: NativeCreateHeightfield,
    pub destroy: NativeDestroyHeightfield,
    pub set_focus: NativeSetHeightfieldFocus,
    pub read_requests: NativeReadHeightfieldRequests,
    pub commit_tile: NativeCommitHeightfieldTile,
    pub invalidate: NativeInvalidateHeightfield,
    pub pick: NativePickHeightfield,
    pub read: NativeReadHeightfield,
}
