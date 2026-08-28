use crate::*;
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAppearanceHandle {
    pub value: u64,
}

/// An Engine-owned retained light definition. The generated C# owner must be
/// disposed; a successful replacement intentionally leaves the prior owner a
/// safe tombstone.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeLightHandle {
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

/// An Engine-owned retained material definition. Its generated C# owner must
/// be disposed; disposing an already-replaced definition is intentionally safe.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeMaterialHandle {
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

/// Closed tagged representation of an Engine `LightDescriptor`. Fields not
/// used by a kind are ignored; Rust validates the selected descriptor before
/// it becomes retained renderer state.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeLightKind {
    Ambient = 0,
    Directional = 1,
    Point = 2,
    Spot = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeLightShadowIntent {
    Disabled = 0,
    Requested = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLightDescriptor {
    pub kind: NativeLightKind,
    pub color: NativeVec3,
    pub intensity: f32,
    pub enabled: bool,
    pub position: NativeVec3,
    pub direction: NativeVec3,
    pub has_range: bool,
    pub range: f32,
    pub decay: f32,
    pub outer_angle_radians: f32,
    pub penumbra: f32,
    pub shadow_intent: NativeLightShadowIntent,
}

/// One requested runtime light fact. `logical_id` and `parent_object_id`
/// identify product facts only; renderer handles never cross this boundary.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLightRequest {
    pub logical_id: u64,
    pub has_parent_object: bool,
    pub parent_object_id: u64,
    pub descriptor: NativeLightDescriptor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLightUpdateRequest {
    pub light: NativeLightHandle,
    pub replacement: NativeLightRequest,
}

/// Requested facts retained by the Engine for one live light owner. This is
/// deliberately not renderer or shadow realization feedback.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLightReadout {
    pub logical_id: u64,
    pub has_parent_object: bool,
    pub parent_object_id: u64,
    pub descriptor: NativeLightDescriptor,
}

/// Product-selected presentation layer. `Viewmodel` is renderer-relative;
/// Engine still owns the retained node and render pass realization.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRenderLayer {
    Scene = 0,
    Debug = 1,
    Ui = 2,
    Viewmodel = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeRenderResourceKind {
    #[default]
    Texture = 1,
    StaticMesh = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePrimitiveGeometry {
    Cube = 1,
    Sphere = 2,
    Quad = 3,
    Point = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBillboardMode {
    None = 0,
    Spherical = 1,
    Cylindrical = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSpriteSizeMode {
    World = 0,
    Pixel = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSpriteDepthPolicy {
    Default = 0,
    DepthTestOff = 1,
    DepthWriteOff = 2,
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
    pub kind: NativeRenderResourceKind,
    pub byte_length: u32,
}

/// Renderer-neutral PBR-like material values. Texture handle zero means an
/// untextured material. The Engine resolves resource identity and validates
/// the resulting retained material descriptor.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMaterialRequest {
    pub color: NativeColor,
    pub texture: NativeRenderResourceHandle,
    pub roughness: f32,
    pub texture_tint: NativeColor,
    pub emission_color: NativeVec3,
    pub emission_intensity: f32,
    pub double_sided: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMaterialUpdateRequest {
    pub material: NativeMaterialHandle,
    pub replacement: NativeMaterialRequest,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePrimitiveAppearanceRequest {
    pub geometry: NativePrimitiveGeometry,
    pub wireframe: bool,
    pub color: NativeColor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePrimitiveAppearanceReplaceRequest {
    /// The prior handle becomes a tombstone if replacement succeeds.
    pub appearance: NativeAppearanceHandle,
    pub replacement: NativePrimitiveAppearanceRequest,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMeshGroup {
    pub material_slot: u32,
    pub start: u32,
    pub count: u32,
}

/// One Engine-owned material selected for a static mesh material slot.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMeshMaterialBinding {
    pub material_slot: u32,
    pub material: NativeMaterialHandle,
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
pub struct NativeStaticMeshMaterialUpdateRequest {
    pub appearance: NativeAppearanceHandle,
    pub bindings: *const NativeMeshMaterialBinding,
    pub bindings_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpriteAppearanceRequest {
    pub texture: NativeRenderResourceHandle,
    pub uv_min: NativeVec2,
    pub uv_max: NativeVec2,
    pub pivot: NativeVec2,
    pub size: NativeVec2,
    pub billboard: NativeBillboardMode,
    pub size_mode: NativeSpriteSizeMode,
    pub render_order: i32,
    pub depth: NativeSpriteDepthPolicy,
    pub tint: NativeColor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpriteAppearanceReplaceRequest {
    /// The prior handle becomes a tombstone if replacement succeeds.
    pub appearance: NativeAppearanceHandle,
    pub replacement: NativeSpriteAppearanceRequest,
}

/// One complete renderer-neutral product appearance fact.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAppearanceFact {
    pub object_id: u64,
    pub transform: NativeTransform,
    pub appearance: NativeAppearanceHandle,
    pub visible: bool,
    pub layer: NativeRenderLayer,
}

/// Bounded Engine-generated presentation facts. These are not renderer frame
/// data and do not reveal backend resources or canvas state.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePresentationReadout {
    pub retained_object_count: u32,
    pub appearance_count: u32,
    pub material_count: u32,
    pub resource_count: u32,
}
