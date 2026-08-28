//! Typed, retained authored-content catalog access for trusted NativeAOT products.

use crate::{
    NativeColor, NativeContentReferenceHandle, NativeEngineDiagnosticLeaseHandle,
    NativeLightShadowIntent, NativeOperationErrorReceipt, NativeTransform, NativeUtf8Slice,
    NativeVec3,
};
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredCatalogHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredCatalogReadoutLeaseHandle {
    pub value: u64,
}
/// Retained, immutable prefab registry owned by the shared AuthoredContent
/// service. A registry is explicitly validated against one admitted catalog.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredPrefabRegistryHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredPrefabRegistryReadoutLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredResolvedPrefabLeaseHandle {
    pub value: u64,
}
/// Retained, immutable scene admission plan. It owns the prepared
/// `SceneAdmissionPlan` as well as the copied readout rows exposed to C#.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredScenePlanHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredScenePlanReadoutLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAssetKind {
    Material = 1,
    StaticMesh = 2,
    AnimatedMesh = 3,
    Sprite = 4,
    SpriteSheet = 5,
    Texture = 6,
    AudioClip = 7,
    Font = 8,
    VoxelVolume = 9,
    VoxelObject = 10,
    Script = 11,
    Scene = 12,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAssetVersionRequirementKind {
    Any = 1,
    Exact = 2,
    AtLeast = 3,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredPrefabPartSourceKind {
    Scene = 1,
    EntityDefinition = 2,
    VoxelObject = 3,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredPrefabOverrideKind {
    Transform = 1,
    EntityDefinition = 2,
    Asset = 3,
    Material = 4,
    Activation = 5,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredSceneNodeKind {
    EmptyGroup = 1,
    StaticMesh = 2,
    AnimatedMesh = 3,
    Sprite = 4,
    VoxelVolume = 5,
    Light = 6,
    Marker = 7,
    EntityInstance = 8,
    Bootstrap = 9,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredSceneEntityReferenceKind {
    EntityDefinition = 1,
    Prefab = 2,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredSceneLightKind {
    Ambient = 1,
    Directional = 2,
    Point = 3,
    Spot = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredStructuralClass {
    Decorative = 1,
    Solid = 2,
    Structural = 3,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredUvStrategy {
    Flat = 1,
    Planar = 2,
    Atlas = 3,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredTextureFilter {
    Nearest = 1,
    Linear = 2,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredTextureWrap {
    Clamp = 1,
    Repeat = 2,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredAtlasInset {
    HalfTexel = 1,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredVoxelAlphaModeKind {
    Opaque = 1,
    Mask = 2,
    Blend = 3,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredVoxelSurfaceMappingKind {
    Repeat = 1,
    Atlas = 2,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredFallbackContext {
    DebugOverlay = 1,
    CosmeticSurface = 2,
    CollisionCritical = 3,
    BackgroundDecoration = 4,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredFallbackOutcomeKind {
    UseFallback = 1,
    FailClosed = 2,
    Skip = 3,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAuthoredFallbackVisual {
    None = 0,
    MagentaSquare = 1,
    GreyMaterial = 2,
}

/// Typed stable identity. `id` carries its kind-prefixed canonical spelling.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredAssetReference {
    pub id: NativeUtf8Slice,
    pub version_kind: NativeAssetVersionRequirementKind,
    pub version: u32,
    pub has_hash: bool,
    /// Algorithm-agnostic lowercase even-length hex, never a Content SHA-256 word value.
    pub hash: NativeUtf8Slice,
}

/// Payload-free catalog entry. Material and the material/texture/voxel payload
/// families deliberately have no representation in this first authored-content tranche.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogEntryInput {
    pub id: NativeUtf8Slice,
    pub version: u32,
    pub has_hash: bool,
    pub hash: NativeUtf8Slice,
    pub has_source_path: bool,
    pub source_path: NativeUtf8Slice,
    pub has_label: bool,
    pub label: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogDependencyInput {
    pub entry_id: NativeUtf8Slice,
    pub reference_id: NativeUtf8Slice,
    pub reference_version_kind: NativeAssetVersionRequirementKind,
    pub reference_version: u32,
    pub reference_has_hash: bool,
    pub reference_hash: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogAdmitRequest {
    pub entries: *const NativeAuthoredCatalogEntryInput,
    pub entries_len: usize,
    pub dependencies: *const NativeAuthoredCatalogDependencyInput,
    pub dependencies_len: usize,
}

/// One material payload attached to an entry in a grouped typed catalog admit.
/// Optional texture and voxel-surface values use their explicit `has_*` flags;
/// their accompanying fields must still be initialized by callers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredMaterialInput {
    pub entry_id: NativeUtf8Slice,
    pub solid: bool,
    pub collidable: bool,
    pub occludes: bool,
    pub structural_class: NativeAuthoredStructuralClass,
    pub color: NativeColor,
    pub has_texture: bool,
    pub texture_id: NativeUtf8Slice,
    pub texture_version_kind: NativeAssetVersionRequirementKind,
    pub texture_version: u32,
    pub texture_has_hash: bool,
    pub texture_hash: NativeUtf8Slice,
    pub roughness: f32,
    pub texture_tint: NativeColor,
    pub emission_color: NativeColor,
    pub emissive: f32,
    pub uv_strategy: NativeAuthoredUvStrategy,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredTextureInput {
    pub entry_id: NativeUtf8Slice,
    pub width: u32,
    pub height: u32,
    pub filter: NativeAuthoredTextureFilter,
    pub wrap: NativeAuthoredTextureWrap,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredVoxelAtlasInput {
    pub entry_id: NativeUtf8Slice,
    pub schema_version: u32,
    pub texture_id: NativeUtf8Slice,
    pub texture_version_kind: NativeAssetVersionRequirementKind,
    pub texture_version: u32,
    pub texture_has_hash: bool,
    pub texture_hash: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredAtlasRegionInput {
    pub atlas_entry_id: NativeUtf8Slice,
    pub id: NativeUtf8Slice,
    pub content_min_x: u32,
    pub content_min_y: u32,
    pub content_extent_x: u32,
    pub content_extent_y: u32,
    pub padding_left: u16,
    pub padding_right: u16,
    pub padding_bottom: u16,
    pub padding_top: u16,
    pub inset: NativeAuthoredAtlasInset,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredVoxelSurfaceInput {
    pub material_entry_id: NativeUtf8Slice,
    pub schema_version: u32,
    pub mapping_kind: NativeAuthoredVoxelSurfaceMappingKind,
    pub texture_id: NativeUtf8Slice,
    pub texture_version_kind: NativeAssetVersionRequirementKind,
    pub texture_version: u32,
    pub texture_has_hash: bool,
    pub texture_hash: NativeUtf8Slice,
    pub atlas_id: NativeUtf8Slice,
    pub atlas_version_kind: NativeAssetVersionRequirementKind,
    pub atlas_version: u32,
    pub atlas_has_hash: bool,
    pub atlas_hash: NativeUtf8Slice,
    pub region: NativeUtf8Slice,
    pub tile_scale_x: f32,
    pub tile_scale_y: f32,
    pub tile_origin_x: f32,
    pub tile_origin_y: f32,
    pub alpha_mode: NativeAuthoredVoxelAlphaModeKind,
    pub alpha_cutoff: f32,
}
/// All catalog payload arrays remain borrowed only for the direct admit call.
/// Rust builds the catalog value and canonicalizes it before returning a handle.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogPayloadAdmitRequest {
    pub entries: *const NativeAuthoredCatalogEntryInput,
    pub entries_len: usize,
    pub dependencies: *const NativeAuthoredCatalogDependencyInput,
    pub dependencies_len: usize,
    pub materials: *const NativeAuthoredMaterialInput,
    pub materials_len: usize,
    pub textures: *const NativeAuthoredTextureInput,
    pub textures_len: usize,
    pub voxel_atlases: *const NativeAuthoredVoxelAtlasInput,
    pub voxel_atlases_len: usize,
    pub atlas_regions: *const NativeAuthoredAtlasRegionInput,
    pub atlas_regions_len: usize,
    pub voxel_surfaces: *const NativeAuthoredVoxelSurfaceInput,
    pub voxel_surfaces_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogFromContentRequest {
    pub content: NativeContentReferenceHandle,
}
/// A complete prefab definition header. Parts, roles, removed roles, and
/// overrides are grouped in the surrounding flat request by `prefab_id`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabDefinitionInput {
    pub id: u64,
    pub schema_version: u32,
    pub display_name: NativeUtf8Slice,
    pub has_variant: bool,
    pub variant_id: NativeUtf8Slice,
    pub variant_base: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabPartInput {
    pub prefab_id: u64,
    pub id: u64,
    pub namespace: NativeUtf8Slice,
    pub display_name: NativeUtf8Slice,
    pub has_parent: bool,
    pub parent_id: u64,
    pub transform: NativeTransform,
    pub source_kind: NativeAuthoredPrefabPartSourceKind,
    /// Asset id for Scene/VoxelObject or stable entity-definition id.
    pub source: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabRoleInput {
    pub prefab_id: u64,
    pub role: NativeUtf8Slice,
    pub part_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabRemovedRoleInput {
    pub prefab_id: u64,
    pub role: NativeUtf8Slice,
}
/// `value` is used by entity-definition, asset, and material overrides;
/// `transform` is used only by Transform and `active` only by Activation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabOverrideInput {
    pub prefab_id: u64,
    pub target_role: NativeUtf8Slice,
    pub kind: NativeAuthoredPrefabOverrideKind,
    pub transform: NativeTransform,
    pub value: NativeUtf8Slice,
    pub active: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabInstanceOverrideInput {
    pub target_role: NativeUtf8Slice,
    pub kind: NativeAuthoredPrefabOverrideKind,
    pub transform: NativeTransform,
    pub value: NativeUtf8Slice,
    pub active: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabEntityDefinitionInput {
    pub stable_id: NativeUtf8Slice,
}
/// Every row is direct and flat. Rust copies and validates values before
/// retaining a registry handle; no input span survives the call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabRegistryAdmitRequest {
    pub schema_version: u32,
    pub catalog: NativeAuthoredCatalogHandle,
    pub definitions: *const NativeAuthoredPrefabDefinitionInput,
    pub definitions_len: usize,
    pub parts: *const NativeAuthoredPrefabPartInput,
    pub parts_len: usize,
    pub roles: *const NativeAuthoredPrefabRoleInput,
    pub roles_len: usize,
    pub removed_roles: *const NativeAuthoredPrefabRemovedRoleInput,
    pub removed_roles_len: usize,
    pub overrides: *const NativeAuthoredPrefabOverrideInput,
    pub overrides_len: usize,
    pub entity_definition_ids: *const NativeAuthoredPrefabEntityDefinitionInput,
    pub entity_definition_ids_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabRegistryFromContentRequest {
    pub content: NativeContentReferenceHandle,
    pub catalog: NativeAuthoredCatalogHandle,
    pub entity_definition_ids: *const NativeAuthoredPrefabEntityDefinitionInput,
    pub entity_definition_ids_len: usize,
}
/// Instance overrides are intentionally separate from retained registry
/// definitions: resolution is a pure, non-mutating owner operation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabResolveRequest {
    pub registry: NativeAuthoredPrefabRegistryHandle,
    pub prefab_id: u64,
    pub instance_overrides: *const NativeAuthoredPrefabInstanceOverrideInput,
    pub instance_overrides_len: usize,
}
/// Explicit product-owned identity admitted as part of scene preparation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneEntityDefinitionInput {
    pub stable_id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneGeneratorPresetInput {
    pub provider_id: NativeUtf8Slice,
    pub preset_id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneCatalogIdInput {
    pub catalog_id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneDependencyInput {
    pub reference_id: NativeUtf8Slice,
    pub reference_version_kind: NativeAssetVersionRequirementKind,
    pub reference_version: u32,
    pub reference_has_hash: bool,
    pub reference_hash: NativeUtf8Slice,
}
/// One flat scene node. `asset` is used only by the four asset-bearing kinds;
/// `marker_id` only by Marker. Other fields must still be initialized.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneNodeInput {
    pub id: u64,
    pub has_parent: bool,
    pub parent_id: u64,
    pub child_order: u32,
    pub transform: NativeTransform,
    pub renderable_transform: NativeTransform,
    pub kind: NativeAuthoredSceneNodeKind,
    pub asset_id: NativeUtf8Slice,
    pub asset_version_kind: NativeAssetVersionRequirementKind,
    pub asset_version: u32,
    pub asset_has_hash: bool,
    pub asset_hash: NativeUtf8Slice,
    pub marker_id: NativeUtf8Slice,
    pub has_label: bool,
    pub label: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneNodeTagInput {
    pub node_id: u64,
    pub tag: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneEntityInstanceInput {
    pub node_id: u64,
    pub instance_id: NativeUtf8Slice,
    pub reference_kind: NativeAuthoredSceneEntityReferenceKind,
    /// Stable entity-definition id for EntityDefinition; initialized but unused for Prefab.
    pub entity_definition_id: NativeUtf8Slice,
    pub prefab_id: u64,
    pub has_variant: bool,
    pub variant_id: NativeUtf8Slice,
    pub instantiation_seed: u64,
    pub has_spawn_marker: bool,
    pub spawn_marker_id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneLightInput {
    pub node_id: u64,
    pub kind: NativeAuthoredSceneLightKind,
    pub color: NativeVec3,
    pub intensity: f32,
    pub enabled: bool,
    pub has_range: bool,
    pub range: f32,
    pub decay: f32,
    pub outer_angle_radians: f32,
    pub penumbra: f32,
    pub shadow_intent: NativeLightShadowIntent,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneBootstrapInput {
    pub node_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneGeneratorInput {
    pub bootstrap_node_id: u64,
    pub provider_id: NativeUtf8Slice,
    pub preset_id: NativeUtf8Slice,
    pub seed: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneCatalogBindingInput {
    pub bootstrap_node_id: u64,
    pub binding_id: NativeUtf8Slice,
    pub catalog_id: NativeUtf8Slice,
    pub source_path: NativeUtf8Slice,
}
/// All typed scene spans are borrowed only for direct plan preparation. The
/// service builds and validates `FlatSceneDocument` before retaining its plan.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredScenePrepareRequest {
    pub scene_id: u64,
    pub scene_revision: u64,
    pub schema_version: u32,
    pub has_name: bool,
    pub name: NativeUtf8Slice,
    pub authoring_format_version: u32,
    /// Retained catalog and prefab handles derive asset and prefab reachability.
    pub catalog: NativeAuthoredCatalogHandle,
    pub prefab_registry: NativeAuthoredPrefabRegistryHandle,
    pub entity_definition_ids: *const NativeAuthoredSceneEntityDefinitionInput,
    pub entity_definition_ids_len: usize,
    pub generator_presets: *const NativeAuthoredSceneGeneratorPresetInput,
    pub generator_presets_len: usize,
    pub catalog_ids: *const NativeAuthoredSceneCatalogIdInput,
    pub catalog_ids_len: usize,
    pub base_entity: u64,
    pub dependencies: *const NativeAuthoredSceneDependencyInput,
    pub dependencies_len: usize,
    pub nodes: *const NativeAuthoredSceneNodeInput,
    pub nodes_len: usize,
    pub tags: *const NativeAuthoredSceneNodeTagInput,
    pub tags_len: usize,
    pub instances: *const NativeAuthoredSceneEntityInstanceInput,
    pub instances_len: usize,
    pub lights: *const NativeAuthoredSceneLightInput,
    pub lights_len: usize,
    pub bootstraps: *const NativeAuthoredSceneBootstrapInput,
    pub bootstraps_len: usize,
    pub generators: *const NativeAuthoredSceneGeneratorInput,
    pub generators_len: usize,
    pub catalog_bindings: *const NativeAuthoredSceneCatalogBindingInput,
    pub catalog_bindings_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredScenePrepareFromContentRequest {
    pub content: NativeContentReferenceHandle,
    pub catalog: NativeAuthoredCatalogHandle,
    pub prefab_registry: NativeAuthoredPrefabRegistryHandle,
    pub entity_definition_ids: *const NativeAuthoredSceneEntityDefinitionInput,
    pub entity_definition_ids_len: usize,
    pub generator_presets: *const NativeAuthoredSceneGeneratorPresetInput,
    pub generator_presets_len: usize,
    pub catalog_ids: *const NativeAuthoredSceneCatalogIdInput,
    pub catalog_ids_len: usize,
    pub base_entity: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogResolveRequest {
    pub catalog: NativeAuthoredCatalogHandle,
    pub reference_id: NativeUtf8Slice,
    pub reference_version_kind: NativeAssetVersionRequirementKind,
    pub reference_version: u32,
    pub reference_has_hash: bool,
    pub reference_hash: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredMaterialResolveRequest {
    pub catalog: NativeAuthoredCatalogHandle,
    pub material_id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredFallbackResolveRequest {
    pub kind: NativeAssetKind,
    pub context: NativeAuthoredFallbackContext,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogEntryReadout {
    pub id: NativeUtf8Slice,
    pub kind: NativeAssetKind,
    pub version: u32,
    pub has_hash: bool,
    pub hash: NativeUtf8Slice,
    pub has_source_path: bool,
    pub source_path: NativeUtf8Slice,
    pub has_label: bool,
    pub label: NativeUtf8Slice,
    pub dependency_count: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogDependencyReadout {
    pub entry_id: NativeUtf8Slice,
    pub reference: NativeAuthoredAssetReference,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredMaterialReadout {
    pub entry_id: NativeUtf8Slice,
    pub solid: bool,
    pub collidable: bool,
    pub occludes: bool,
    pub structural_class: NativeAuthoredStructuralClass,
    pub color: NativeColor,
    pub has_texture: bool,
    pub texture: NativeAuthoredAssetReference,
    pub roughness: f32,
    pub texture_tint: NativeColor,
    pub emission_color: NativeColor,
    pub emissive: f32,
    pub uv_strategy: NativeAuthoredUvStrategy,
    pub has_voxel_surface: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredTextureReadout {
    pub entry_id: NativeUtf8Slice,
    pub width: u32,
    pub height: u32,
    pub filter: NativeAuthoredTextureFilter,
    pub wrap: NativeAuthoredTextureWrap,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredVoxelAtlasReadout {
    pub entry_id: NativeUtf8Slice,
    pub schema_version: u32,
    pub texture: NativeAuthoredAssetReference,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredAtlasRegionReadout {
    pub atlas_entry_id: NativeUtf8Slice,
    pub id: NativeUtf8Slice,
    pub content_min_x: u32,
    pub content_min_y: u32,
    pub content_extent_x: u32,
    pub content_extent_y: u32,
    pub padding_left: u16,
    pub padding_right: u16,
    pub padding_bottom: u16,
    pub padding_top: u16,
    pub inset: NativeAuthoredAtlasInset,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredVoxelSurfaceReadout {
    pub material_entry_id: NativeUtf8Slice,
    pub schema_version: u32,
    pub mapping_kind: NativeAuthoredVoxelSurfaceMappingKind,
    pub texture: NativeAuthoredAssetReference,
    pub atlas: NativeAuthoredAssetReference,
    pub region: NativeUtf8Slice,
    pub tile_scale_x: f32,
    pub tile_scale_y: f32,
    pub tile_origin_x: f32,
    pub tile_origin_y: f32,
    pub alpha_mode: NativeAuthoredVoxelAlphaModeKind,
    pub alpha_cutoff: f32,
    /// False for catalog inspection. True only for the named owner resolution
    /// operations, which fill actual entry versions and texture sampling facts.
    pub has_resolved_mapping: bool,
    pub resolved_texture_version: u32,
    pub resolved_atlas_version: u32,
    pub resolved_filter: NativeAuthoredTextureFilter,
    pub resolved_wrap: NativeAuthoredTextureWrap,
    /// The exact texture reference selected by the owner, including the atlas
    /// texture for atlas mappings. Empty unless `has_resolved_mapping` is true.
    pub resolved_texture: NativeAuthoredAssetReference,
    pub has_resolved_region: bool,
    pub resolved_region_id: NativeUtf8Slice,
    pub resolved_region_min_x: u32,
    pub resolved_region_min_y: u32,
    pub resolved_region_extent_x: u32,
    pub resolved_region_extent_y: u32,
    pub resolved_region_padding_left: u16,
    pub resolved_region_padding_right: u16,
    pub resolved_region_padding_bottom: u16,
    pub resolved_region_padding_top: u16,
    pub resolved_region_inset: NativeAuthoredAtlasInset,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogReadoutLease {
    pub handle: NativeAuthoredCatalogReadoutLeaseHandle,
    pub canonical_hash: NativeUtf8Slice,
    pub entry_count: u32,
    pub entries: *const NativeAuthoredCatalogEntryReadout,
    pub entries_len: usize,
    pub dependencies: *const NativeAuthoredCatalogDependencyReadout,
    pub dependencies_len: usize,
    pub materials: *const NativeAuthoredMaterialReadout,
    pub materials_len: usize,
    pub textures: *const NativeAuthoredTextureReadout,
    pub textures_len: usize,
    pub voxel_atlases: *const NativeAuthoredVoxelAtlasReadout,
    pub voxel_atlases_len: usize,
    pub atlas_regions: *const NativeAuthoredAtlasRegionReadout,
    pub atlas_regions_len: usize,
    pub voxel_surfaces: *const NativeAuthoredVoxelSurfaceReadout,
    pub voxel_surfaces_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredResolvedEntryLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredResolvedEntryLease {
    pub handle: NativeAuthoredResolvedEntryLeaseHandle,
    pub entry: *const NativeAuthoredCatalogEntryReadout,
    pub entry_len: usize,
    pub dependencies: *const NativeAuthoredCatalogDependencyReadout,
    pub dependencies_len: usize,
    pub materials: *const NativeAuthoredMaterialReadout,
    pub materials_len: usize,
    pub textures: *const NativeAuthoredTextureReadout,
    pub textures_len: usize,
    pub voxel_atlases: *const NativeAuthoredVoxelAtlasReadout,
    pub voxel_atlases_len: usize,
    pub atlas_regions: *const NativeAuthoredAtlasRegionReadout,
    pub atlas_regions_len: usize,
    pub voxel_surfaces: *const NativeAuthoredVoxelSurfaceReadout,
    pub voxel_surfaces_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredMaterialResolutionLeaseHandle {
    pub value: u64,
}
/// Owner-derived render and collision material facts. This contains no renderer
/// resources or realization handles; C# remains a consumer of the projection.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredMaterialResolutionLease {
    pub handle: NativeAuthoredMaterialResolutionLeaseHandle,
    pub materials: *const NativeAuthoredMaterialReadout,
    pub materials_len: usize,
    pub voxel_surfaces: *const NativeAuthoredVoxelSurfaceReadout,
    pub voxel_surfaces_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredVoxelSurfaceResolutionLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredVoxelSurfaceResolutionLease {
    pub handle: NativeAuthoredVoxelSurfaceResolutionLeaseHandle,
    pub surfaces: *const NativeAuthoredVoxelSurfaceReadout,
    pub surfaces_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredFallbackReadout {
    pub outcome: NativeAuthoredFallbackOutcomeKind,
    pub visual: NativeAuthoredFallbackVisual,
    pub reason: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredFallbackLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredFallbackLease {
    pub handle: NativeAuthoredFallbackLeaseHandle,
    pub outcomes: *const NativeAuthoredFallbackReadout,
    pub outcomes_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabDefinitionReadout {
    pub id: u64,
    pub schema_version: u32,
    pub display_name: NativeUtf8Slice,
    pub has_variant: bool,
    pub variant_id: NativeUtf8Slice,
    pub variant_base: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabPartReadout {
    pub prefab_id: u64,
    pub id: u64,
    pub namespace: NativeUtf8Slice,
    pub display_name: NativeUtf8Slice,
    pub has_parent: bool,
    pub parent_id: u64,
    pub transform: NativeTransform,
    pub source_kind: NativeAuthoredPrefabPartSourceKind,
    pub source: NativeUtf8Slice,
    pub has_material: bool,
    /// Empty unless `has_material` is true. Base registry parts have no
    /// material override, while resolved parts may carry one.
    pub material: NativeUtf8Slice,
    pub active: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabRoleReadout {
    pub prefab_id: u64,
    pub part_id: u64,
    pub role: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabRemovedRoleReadout {
    pub prefab_id: u64,
    pub role: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabOverrideReadout {
    pub prefab_id: u64,
    pub target_role: NativeUtf8Slice,
    pub kind: NativeAuthoredPrefabOverrideKind,
    pub transform: NativeTransform,
    pub value: NativeUtf8Slice,
    pub active: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredPrefabRegistryReadoutLease {
    pub handle: NativeAuthoredPrefabRegistryReadoutLeaseHandle,
    pub schema_version: u32,
    pub definitions: *const NativeAuthoredPrefabDefinitionReadout,
    pub definitions_len: usize,
    pub parts: *const NativeAuthoredPrefabPartReadout,
    pub parts_len: usize,
    pub roles: *const NativeAuthoredPrefabRoleReadout,
    pub roles_len: usize,
    pub removed_roles: *const NativeAuthoredPrefabRemovedRoleReadout,
    pub removed_roles_len: usize,
    pub overrides: *const NativeAuthoredPrefabOverrideReadout,
    pub overrides_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredResolvedPrefabLease {
    pub handle: NativeAuthoredResolvedPrefabLeaseHandle,
    pub requested_id: u64,
    pub base_id: u64,
    pub has_variant: bool,
    pub variant_id: NativeUtf8Slice,
    pub parts: *const NativeAuthoredPrefabPartReadout,
    pub parts_len: usize,
    pub roles: *const NativeAuthoredPrefabRoleReadout,
    pub roles_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneAllocationReadout {
    pub node_id: u64,
    pub entity_id: u64,
    pub has_parent_entity: bool,
    pub parent_entity_id: u64,
    pub local_transform: NativeTransform,
    pub world_transform: NativeTransform,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneResolvedInstanceReadout {
    pub node_id: u64,
    pub entity_id: u64,
    pub instance_id: NativeUtf8Slice,
    pub reference_kind: NativeAuthoredSceneEntityReferenceKind,
    pub entity_definition_id: NativeUtf8Slice,
    pub prefab_id: u64,
    pub has_variant: bool,
    pub variant_id: NativeUtf8Slice,
    pub instantiation_seed: u64,
    pub has_spawn_marker: bool,
    pub spawn_marker_id: NativeUtf8Slice,
    pub local_transform: NativeTransform,
    pub world_transform: NativeTransform,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredScenePlannedLightReadout {
    pub node_id: u64,
    pub entity_id: u64,
    pub kind: NativeAuthoredSceneLightKind,
    pub color: NativeVec3,
    pub intensity: f32,
    pub enabled: bool,
    pub has_range: bool,
    pub range: f32,
    pub decay: f32,
    pub outer_angle_radians: f32,
    pub penumbra: f32,
    pub shadow_intent: NativeLightShadowIntent,
    pub world_transform: NativeTransform,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredScenePlannedRenderableReadout {
    pub node_id: u64,
    pub entity_id: u64,
    pub asset_kind: NativeAssetKind,
    pub asset: NativeAuthoredAssetReference,
    pub world_transform: NativeTransform,
    pub renderable_local_transform: NativeTransform,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneBootstrapGeneratorReadout {
    pub provider_id: NativeUtf8Slice,
    pub preset_id: NativeUtf8Slice,
    pub seed: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredSceneBootstrapCatalogBindingReadout {
    pub binding_id: NativeUtf8Slice,
    pub catalog_id: NativeUtf8Slice,
    pub source_path: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredScenePlanReadoutLease {
    pub handle: NativeAuthoredScenePlanReadoutLeaseHandle,
    pub scene_id: u64,
    pub scene_revision: u64,
    pub allocations: *const NativeAuthoredSceneAllocationReadout,
    pub allocations_len: usize,
    pub resolved_instances: *const NativeAuthoredSceneResolvedInstanceReadout,
    pub resolved_instances_len: usize,
    pub lights: *const NativeAuthoredScenePlannedLightReadout,
    pub lights_len: usize,
    pub renderables: *const NativeAuthoredScenePlannedRenderableReadout,
    pub renderables_len: usize,
    pub generators: *const NativeAuthoredSceneBootstrapGeneratorReadout,
    pub generators_len: usize,
    pub catalog_bindings: *const NativeAuthoredSceneBootstrapCatalogBindingReadout,
    pub catalog_bindings_len: usize,
}

pub type NativeAdmitAuthoredCatalog = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredCatalogAdmitRequest,
    *mut NativeAuthoredCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeAdmitAuthoredCatalogFromContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredCatalogFromContentRequest,
    *mut NativeAuthoredCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeAdmitAuthoredCatalogPayload = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredCatalogPayloadAdmitRequest,
    *mut NativeAuthoredCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredCatalog =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredCatalogHandle) -> i32;
pub type NativeReadAuthoredCatalog = unsafe extern "C" fn(
    *mut c_void,
    NativeAuthoredCatalogHandle,
    *mut NativeAuthoredCatalogReadoutLease,
) -> i32;
pub type NativeDestroyAuthoredCatalogReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredCatalogReadoutLeaseHandle) -> i32;
pub type NativeResolveAuthoredCatalogReference = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredCatalogResolveRequest,
    *mut NativeAuthoredResolvedEntryLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredResolvedEntryLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredResolvedEntryLeaseHandle) -> i32;
pub type NativeResolveAuthoredMaterial = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredMaterialResolveRequest,
    *mut NativeAuthoredMaterialResolutionLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredMaterialResolutionLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredMaterialResolutionLeaseHandle) -> i32;
pub type NativeResolveAuthoredVoxelSurface = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredMaterialResolveRequest,
    *mut NativeAuthoredVoxelSurfaceResolutionLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredVoxelSurfaceResolutionLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredVoxelSurfaceResolutionLeaseHandle) -> i32;
pub type NativeResolveAuthoredFallback = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredFallbackResolveRequest,
    *mut NativeAuthoredFallbackLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredFallbackLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredFallbackLeaseHandle) -> i32;
pub type NativeAdmitAuthoredPrefabRegistry = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredPrefabRegistryAdmitRequest,
    *mut NativeAuthoredPrefabRegistryHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeAdmitAuthoredPrefabRegistryFromContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredPrefabRegistryFromContentRequest,
    *mut NativeAuthoredPrefabRegistryHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredPrefabRegistry =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredPrefabRegistryHandle) -> i32;
pub type NativeReadAuthoredPrefabRegistry = unsafe extern "C" fn(
    *mut c_void,
    NativeAuthoredPrefabRegistryHandle,
    *mut NativeAuthoredPrefabRegistryReadoutLease,
) -> i32;
pub type NativeDestroyAuthoredPrefabRegistryReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredPrefabRegistryReadoutLeaseHandle) -> i32;
pub type NativeResolveAuthoredPrefab = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredPrefabResolveRequest,
    *mut NativeAuthoredResolvedPrefabLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredResolvedPrefabLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredResolvedPrefabLeaseHandle) -> i32;
pub type NativePrepareAuthoredScene = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredScenePrepareRequest,
    *mut NativeAuthoredScenePlanHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativePrepareAuthoredSceneFromContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredScenePrepareFromContentRequest,
    *mut NativeAuthoredScenePlanHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredScenePlan =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredScenePlanHandle) -> i32;
pub type NativeReadAuthoredScenePlan = unsafe extern "C" fn(
    *mut c_void,
    NativeAuthoredScenePlanHandle,
    *mut NativeAuthoredScenePlanReadoutLease,
) -> i32;
pub type NativeDestroyAuthoredScenePlanReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredScenePlanReadoutLeaseHandle) -> i32;
pub type NativeDestroyAuthoredContentOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
