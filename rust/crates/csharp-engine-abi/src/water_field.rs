use std::ffi::c_void;

use crate::*;

/// A water field: a region of column water over a product-supplied bed,
/// advanced by the Engine on its own worker through admitted simulation time.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NativeWaterFieldHandle {
    pub value: u64,
}

/// One edge of a field's grid.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWaterFieldSide {
    /// The last column (x = width − 1).
    XMax = 0,
    /// The first column (x = 0).
    XMin = 1,
    /// The last row (z = length − 1).
    ZMax = 2,
    /// The first row (z = 0).
    ZMin = 3,
}

/// What lies beyond a run of edge cells.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWaterFieldBoundaryKind {
    /// Nothing leaves or enters. Every edge starts as a wall.
    Wall = 0,
    /// A reservoir held at `value`, a surface height in the field's units
    /// (a sea, a lake, a river's stage): water flows in or out.
    Stage = 1,
    /// `value` cubic metres a second enter, spread over the run of cells.
    Inflow = 2,
    /// The ground beyond continues the slope of the last two cells, so
    /// water leaves at the rate that slope drives, as if the field were not
    /// cut there.
    Open = 3,
}

/// How a field advances. Zero in any field selects its default.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWaterFieldPolicy {
    /// The Courant number of each step, above 0 and at most 1 (default
    /// 0.5): lower steps more finely for the same admitted time.
    pub courant: f32,
    /// The longest step, in seconds (default: as long as the Courant number
    /// allows).
    pub max_step_seconds: f32,
    /// Threads one step may use (default 1).
    pub threads: u32,
    /// Work the field may owe its worker, in worker seconds: the admitted
    /// time not yet advanced times what a simulated second last cost
    /// (default 1). Admitted time past it is dropped and reported, so a field
    /// that costs more than it advances falls behind by a bounded amount; a
    /// cheap field fast-forwarded far ahead of the wall clock drops nothing.
    pub max_backlog_seconds: f32,
    /// Manning roughness, s/m^(1/3) (default 0.03, an earth channel).
    pub roughness: f32,
}

/// A new field of `width` by `length` cells, `cell_size` units a side. Its
/// cell (x, z) spans x·cell_size to (x + 1)·cell_size from its corner, and
/// `bed` gives each cell's ground height, x fastest, `width * length` of
/// them. `depth` is the water already standing in each cell (empty: dry).
/// Its corner stands at `origin` in the scene, or with a `session` at the
/// world point `anchor` resolved into that session's frame, again after
/// every world-origin rebase. Bed heights, stages and bed edits are in the
/// field's own frame, from its corner; the solver reads one unit as one
/// metre.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldCreateRequest {
    pub width: u32,
    pub length: u32,
    pub cell_size: f32,
    pub bed: *const f32,
    pub bed_len: usize,
    pub depth: *const f32,
    pub depth_len: usize,
    pub session: NativeOptionalSpatialSession,
    pub anchor: NativeWorldOriginGlobalPosition,
    pub origin: NativeVec3,
    pub policy: NativeWaterFieldPolicy,
}

/// One cell and a value for it (a bed height, or a source rate).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWaterFieldCellValue {
    pub x: u32,
    pub z: u32,
    pub value: f32,
}

/// New bed heights for some cells (a dam, a breach, a terrain edit). The
/// water standing on them is kept, so a raised cell lifts its water and it
/// runs off. The field's incarnation increases.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldBedRequest {
    pub field: NativeWaterFieldHandle,
    pub cells: *const NativeWaterFieldCellValue,
    pub cells_len: usize,
}

/// Replaces every source: each cell takes `uniform_rate` (rain over the
/// whole field), then the listed cells their own rate. Rates are metres of
/// depth a second: positive adds water, negative drains it (never more than
/// is there). Sources hold until replaced.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldSourcesRequest {
    pub field: NativeWaterFieldHandle,
    pub uniform_rate: f32,
    pub cells: *const NativeWaterFieldCellValue,
    pub cells_len: usize,
}

/// Sets what lies beyond `count` edge cells of `side` from `first` (0: to
/// the end of the side), counted along x for the z sides and along z for
/// the x sides.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldBoundaryRequest {
    pub field: NativeWaterFieldHandle,
    pub side: NativeWaterFieldSide,
    pub first: u32,
    pub count: u32,
    pub kind: NativeWaterFieldBoundaryKind,
    pub value: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWaterFieldPolicyRequest {
    pub field: NativeWaterFieldHandle,
    pub policy: NativeWaterFieldPolicy,
}

/// Draws the field's wet cells as one surface with `material` (a material
/// with water settings, blended), redrawn from each new snapshot. Cells
/// shallower than `drawn_depth` (0: 0.01) are left out. A zero material
/// stops drawing.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldPresentRequest {
    pub field: NativeWaterFieldHandle,
    pub material: NativeMaterialHandle,
    pub layer: NativeRenderLayer,
    pub drawn_depth: f32,
}

/// A point in the scene (or for a field drawn in the backdrop, the
/// backdrop), x and z.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWaterFieldPoint {
    pub x: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldSampleRequest {
    pub field: NativeWaterFieldHandle,
    pub points: *const NativeWaterFieldPoint,
    pub points_len: usize,
}

/// The water at one point in the latest snapshot, between the four nearest
/// cell centres: the surface of the wet ones among them, the ground of all
/// four. Heights are in the scene. Outside the field nothing is `inside` and
/// nothing is wet.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWaterFieldSample {
    pub inside: bool,
    pub wet: bool,
    /// Water surface and ground height, and the depth between them.
    pub surface: f32,
    pub bed: f32,
    pub depth: f32,
    /// Depth-averaged flow, units a second.
    pub velocity_x: f32,
    pub velocity_z: f32,
    /// The snapshot's simulated time, and how far admitted time has run
    /// past it, in seconds.
    pub time: f64,
    pub age: f64,
}

/// One sample per point, in order; borrowed until the next WaterField call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldSamplesResult {
    pub samples: *const NativeWaterFieldSample,
    pub samples_len: usize,
}

/// One cell of a snapshot; its surface in the field's own frame, like the
/// bed it was given.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWaterFieldCell {
    pub depth: f32,
    pub surface: f32,
    pub velocity_x: f32,
    pub velocity_z: f32,
}

/// Every cell of the latest snapshot, x fastest; borrowed until the next
/// WaterField call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldCellsResult {
    pub cells: *const NativeWaterFieldCell,
    pub cells_len: usize,
}

/// A field's latest snapshot and its worker.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeWaterFieldReadout {
    /// Where the field's corner stands in the scene now.
    pub origin: NativeVec3,
    pub width: u32,
    pub length: u32,
    pub cell_size: f32,
    /// Bumped by every bed edit; the snapshot's own, which lags the
    /// field's until the worker has advanced past the edit.
    pub incarnation: u64,
    pub snapshot_incarnation: u64,
    /// Snapshots published so far.
    pub snapshots: u64,
    /// The snapshot's simulated time, the admitted time handed to the field,
    /// and the gap between them (seconds).
    pub time: f64,
    pub admitted_time: f64,
    pub age: f64,
    /// Admitted seconds dropped because the field owed its worker more work
    /// than its policy's backlog allows.
    pub dropped_seconds: f64,
    /// Cubic metres in the field, and in total since creation: entered and
    /// left through its edges, added by sources and taken by sinks.
    pub volume: f64,
    pub boundary_inflow: f64,
    pub boundary_outflow: f64,
    pub source_volume: f64,
    pub sink_volume: f64,
    pub wet_cells: u32,
    /// The worker's last advance: its steps, the wall time it took
    /// (building the snapshot and drawn mesh included), and that time per
    /// simulated second (above 1, the field cannot keep up).
    pub steps: u32,
    pub step_microseconds: u64,
    pub load: f32,
    /// Cells the drawn surface shows (0 when not presented).
    pub drawn_cells: u32,
}

pub type NativeCreateWaterField = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeWaterFieldCreateRequest,
    result: *mut NativeWaterFieldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeDestroyWaterField = unsafe extern "C" fn(
    context: *mut c_void,
    field: NativeWaterFieldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeEditWaterFieldBed = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeWaterFieldBedRequest,
    readout: *mut NativeWaterFieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetWaterFieldSources = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeWaterFieldSourcesRequest,
    readout: *mut NativeWaterFieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetWaterFieldBoundary = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeWaterFieldBoundaryRequest,
    readout: *mut NativeWaterFieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetWaterFieldPolicy = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeWaterFieldPolicyRequest,
    readout: *mut NativeWaterFieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativePresentWaterField = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeWaterFieldPresentRequest,
    readout: *mut NativeWaterFieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSampleWaterField = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeWaterFieldSampleRequest,
    result: *mut NativeWaterFieldSamplesResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadWaterFieldCells = unsafe extern "C" fn(
    context: *mut c_void,
    field: NativeWaterFieldHandle,
    result: *mut NativeWaterFieldCellsResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadWaterField = unsafe extern "C" fn(
    context: *mut c_void,
    field: NativeWaterFieldHandle,
    readout: *mut NativeWaterFieldReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

/// Water fields: the product supplies a region's bed, edges and sources;
/// the Engine advances its column water on a worker through admitted time,
/// publishes complete snapshots, answers samples, and can draw the surface.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWaterFieldApi {
    pub context: *mut c_void,
    pub create: NativeCreateWaterField,
    pub destroy: NativeDestroyWaterField,
    pub edit_bed: NativeEditWaterFieldBed,
    pub set_sources: NativeSetWaterFieldSources,
    pub set_boundary: NativeSetWaterFieldBoundary,
    pub set_policy: NativeSetWaterFieldPolicy,
    pub present: NativePresentWaterField,
    pub sample: NativeSampleWaterField,
    pub read_cells: NativeReadWaterFieldCells,
    pub read: NativeReadWaterField,
}
