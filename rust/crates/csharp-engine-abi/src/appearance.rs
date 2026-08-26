use crate::*;
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAppearanceHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRenderResourceHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRngHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// One admitted immutable renderer resource selected by its product content path.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRenderResourceRequest {
    pub path: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeRenderResourceInfo {
    pub handle: NativeRenderResourceHandle,
    /// 1 texture, 2 static mesh.
    pub kind: u32,
    pub byte_length: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePrimitiveAppearanceRequest {
    /// 1 cube, 2 sphere, 3 quad, 4 point.
    pub geometry: u32,
    pub wireframe: u32,
    pub color: NativeColor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMeshGroup {
    pub material_slot: u32,
    pub start: u32,
    pub count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStaticMeshAppearanceRequest {
    pub resource: NativeRenderResourceHandle,
    /// 1 packed streams LE v1, 2 v2, 3 v3.
    pub encoding: u32,
    pub vertex_count: u32,
    pub index_count: u32,
    pub positions_byte_offset: u32,
    pub normals_byte_offset: u32,
    pub uvs_byte_offset: u32,
    pub colors_byte_offset: u32,
    pub indices_byte_offset: u32,
    pub bounds_min: NativeVec3,
    pub bounds_max: NativeVec3,
    pub color: NativeColor,
    pub groups: *const NativeMeshGroup,
    pub groups_len: usize,
}

/// Creates a retained visual-only static mesh from an inline `StaticMeshAsset`
/// JSON document already collected from product content.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStaticMeshContentAppearanceRequest {
    pub path: NativeUtf8Slice,
    pub color: NativeColor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpriteAppearanceRequest {
    pub texture: NativeRenderResourceHandle,
    pub uv_min: NativeVec2,
    pub uv_max: NativeVec2,
    pub pivot: NativeVec2,
    pub size: NativeVec2,
    /// 0 none, 1 spherical, 2 cylindrical.
    pub billboard: u32,
    pub render_order: i32,
    pub tint: NativeColor,
}

/// One complete renderer-neutral product appearance fact.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAppearanceFact {
    pub object_id: u64,
    pub transform: NativeTransform,
    pub appearance: NativeAppearanceHandle,
    pub visible: u32,
    pub reserved: u32,
}
