//! General constructive fields and retained Engine surface generation.
use crate::*;
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitFieldHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitNode {
    /// Opaque field-owned token. Its value is not the kernel's arena index.
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitBoxRequest {
    pub field: NativeImplicitFieldHandle,
    pub minimum: NativeVec3,
    pub maximum: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitSphereRequest {
    pub field: NativeImplicitFieldHandle,
    pub center: NativeVec3,
    pub radius: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitEllipsoidRequest {
    pub field: NativeImplicitFieldHandle,
    pub center: NativeVec3,
    pub radii: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitCapsuleRequest {
    pub field: NativeImplicitFieldHandle,
    pub start: NativeVec3,
    pub end: NativeVec3,
    pub radius: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitFrustumRequest {
    pub field: NativeImplicitFieldHandle,
    pub start: NativeVec3,
    pub end: NativeVec3,
    pub start_radius: f32,
    pub end_radius: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitPlaneRequest {
    pub field: NativeImplicitFieldHandle,
    pub normal: NativeVec3,
    pub offset: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitBinaryRequest {
    pub field: NativeImplicitFieldHandle,
    pub left: NativeImplicitNode,
    pub right: NativeImplicitNode,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitBlendRequest {
    pub field: NativeImplicitFieldHandle,
    pub left: NativeImplicitNode,
    pub right: NativeImplicitNode,
    pub radius: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitOffsetRequest {
    pub field: NativeImplicitFieldHandle,
    pub source: NativeImplicitNode,
    pub amount: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitTransformRequest {
    pub field: NativeImplicitFieldHandle,
    pub source: NativeImplicitNode,
    pub transform: NativeTransform,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitSampleRequest {
    pub field: NativeImplicitFieldHandle,
    pub source: NativeImplicitNode,
    pub position: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitSample {
    pub value: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitMaterialRegion {
    pub node: NativeImplicitNode,
    pub material: NativeMaterialHandle,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeImplicitMaterialBoundaryMode {
    Centroid = 0,
    Interpolated = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitGenerateRequest {
    pub field: NativeImplicitFieldHandle,
    pub source: NativeImplicitNode,
    pub minimum: NativeVec3,
    pub maximum: NativeVec3,
    pub cell_size: f32,
    pub crease_angle_degrees: f32,
    pub uv_scale: f32,
    pub default_material: NativeMaterialHandle,
    pub regions: *const NativeImplicitMaterialRegion,
    pub regions_len: usize,
    pub material_boundary_mode: NativeImplicitMaterialBoundaryMode,
    /// Optional material-boundary sample spacing. Zero preserves ordinary
    /// interpolation; a positive value requests bounded material refinement.
    pub material_sample_spacing: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitGenerationReadout {
    pub node_count: u32,
    pub vertices: u32,
    pub triangles: u32,
    pub material_groups: u32,
    pub octree_depth: u32,
    pub sample_spacing: f32,
    pub generation_seconds: f64,
    pub reoriented_triangles: u32,
    pub degenerate_triangles: u32,
}

pub type NativeCreateImplicitField =
    unsafe extern "C" fn(*mut c_void, *mut NativeImplicitFieldHandle) -> i32;

pub type NativeDestroyImplicitField =
    unsafe extern "C" fn(*mut c_void, NativeImplicitFieldHandle) -> i32;

pub type NativeAddImplicitBox =
    unsafe extern "C" fn(*mut c_void, NativeImplicitBoxRequest, *mut NativeImplicitNode) -> i32;

pub type NativeAddImplicitSphere =
    unsafe extern "C" fn(*mut c_void, NativeImplicitSphereRequest, *mut NativeImplicitNode) -> i32;

pub type NativeAddImplicitEllipsoid = unsafe extern "C" fn(
    *mut c_void,
    NativeImplicitEllipsoidRequest,
    *mut NativeImplicitNode,
) -> i32;

pub type NativeAddImplicitCapsule =
    unsafe extern "C" fn(*mut c_void, NativeImplicitCapsuleRequest, *mut NativeImplicitNode) -> i32;

pub type NativeAddImplicitFrustum =
    unsafe extern "C" fn(*mut c_void, NativeImplicitFrustumRequest, *mut NativeImplicitNode) -> i32;

pub type NativeAddImplicitPlane =
    unsafe extern "C" fn(*mut c_void, NativeImplicitPlaneRequest, *mut NativeImplicitNode) -> i32;

pub type NativeImplicitUnion =
    unsafe extern "C" fn(*mut c_void, NativeImplicitBinaryRequest, *mut NativeImplicitNode) -> i32;

pub type NativeImplicitIntersection =
    unsafe extern "C" fn(*mut c_void, NativeImplicitBinaryRequest, *mut NativeImplicitNode) -> i32;

pub type NativeImplicitDifference =
    unsafe extern "C" fn(*mut c_void, NativeImplicitBinaryRequest, *mut NativeImplicitNode) -> i32;

pub type NativeImplicitSmoothUnion =
    unsafe extern "C" fn(*mut c_void, NativeImplicitBlendRequest, *mut NativeImplicitNode) -> i32;

pub type NativeImplicitOffset =
    unsafe extern "C" fn(*mut c_void, NativeImplicitOffsetRequest, *mut NativeImplicitNode) -> i32;

pub type NativeImplicitTransform = unsafe extern "C" fn(
    *mut c_void,
    NativeImplicitTransformRequest,
    *mut NativeImplicitNode,
) -> i32;

pub type NativeSampleImplicitField = unsafe extern "C" fn(
    *mut c_void,
    NativeImplicitSampleRequest,
    *mut NativeImplicitSample,
) -> i32;

pub type NativeGenerateImplicitSurface = unsafe extern "C" fn(
    *mut c_void,
    *const NativeImplicitGenerateRequest,
    *mut NativeMeshResourceHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeDestroyImplicitOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;

pub type NativeReadImplicitGeneration = unsafe extern "C" fn(
    *mut c_void,
    NativeImplicitFieldHandle,
    *mut NativeImplicitGenerationReadout,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeImplicitSurfacesApi {
    pub context: *mut c_void,
    pub create_field: NativeCreateImplicitField,
    pub destroy_field: NativeDestroyImplicitField,
    pub add_box: NativeAddImplicitBox,
    pub add_sphere: NativeAddImplicitSphere,
    pub add_ellipsoid: NativeAddImplicitEllipsoid,
    pub add_capsule: NativeAddImplicitCapsule,
    pub add_plane: NativeAddImplicitPlane,
    pub union: NativeImplicitUnion,
    pub intersection: NativeImplicitIntersection,
    pub difference: NativeImplicitDifference,
    pub smooth_union: NativeImplicitSmoothUnion,
    pub offset: NativeImplicitOffset,
    pub transform: NativeImplicitTransform,
    pub sample: NativeSampleImplicitField,
    pub generate: NativeGenerateImplicitSurface,
    pub read_generation: NativeReadImplicitGeneration,
    pub destroy_operation_diagnostic_lease: NativeDestroyImplicitOperationDiagnosticLease,
    pub add_frustum: NativeAddImplicitFrustum,
}
