//! Renderer-neutral billboard and particle facts for trusted NativeAOT products.
//!
//! Products name their own live facts. The Engine validates and projects them
//! through its retained presentation projectors; neither renderer handles nor
//! presentation frames cross the ABI.

use crate::{NativeColor, NativeRenderResourceHandle, NativeUtf8Slice, NativeVec3};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePresentationBillboardHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePresentationEmitterHandle {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationAnchorKind {
    World = 1,
    EntityAttached = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationAnchor {
    pub kind: NativePresentationAnchorKind,
    pub position: NativeVec3,
    pub entity: u64,
    pub offset: NativeVec3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBillboardContentKind {
    Text = 1,
    Value = 2,
    Icon = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationBillboardMeterFillDirection {
    LeftToRight = 1,
    RightToLeft = 2,
    BottomToTop = 3,
    TopToBottom = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationBillboardMeter {
    pub id: NativeUtf8Slice,
    pub accessible_label_key: NativeUtf8Slice,
    pub accessible_fallback_text: NativeUtf8Slice,
    pub current: f32,
    pub minimum: f32,
    pub maximum: f32,
    pub has_preview: bool,
    pub preview: f32,
    pub fill_direction: NativePresentationBillboardMeterFillDirection,
    pub segments: u8,
    pub fill: NativeColor,
    pub preview_fill: NativeColor,
    pub back: NativeColor,
    pub border: NativeColor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationBillboardStatusCue {
    pub id: NativeUtf8Slice,
    pub label_key: NativeUtf8Slice,
    pub label_fallback_text: NativeUtf8Slice,
    pub has_icon: bool,
    pub icon: NativeRenderResourceHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationBillboardStyle {
    pub opacity: f32,
    pub backing: NativeColor,
    pub border: NativeColor,
    pub radius_pixels: f32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationBillboardAlignment {
    Start = 1,
    Center = 2,
    End = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationBillboardLayoutSizing {
    ConstantPixels = 1,
    DistanceScaled = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationBillboardSafeArea {
    pub top_pixels: f32,
    pub right_pixels: f32,
    pub bottom_pixels: f32,
    pub left_pixels: f32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationBillboardEdgeBehavior {
    Clamp = 1,
    Cull = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationBillboardOverlapBehavior {
    Stack = 1,
    Suppress = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationBillboardLayout {
    pub priority: i32,
    pub sizing: NativePresentationBillboardLayoutSizing,
    pub reference_distance: f32,
    pub minimum_scale: f32,
    pub maximum_scale: f32,
    pub safe_area: NativePresentationBillboardSafeArea,
    pub edge_behavior: NativePresentationBillboardEdgeBehavior,
    pub overlap_behavior: NativePresentationBillboardOverlapBehavior,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationBillboardLayer {
    AlwaysOnTop = 1,
    DepthTested = 2,
    Occluded = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationFontKind {
    System = 1,
    Asset = 2,
}

/// A complete replacement descriptor for one ordinary world indicator. Asset
/// fonts name an already admitted Engine resource; raw asset paths and hashes
/// never cross the product ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationBillboardDescriptor {
    pub logical_id: u64,
    pub anchor: NativePresentationAnchor,
    pub content_kind: NativeBillboardContentKind,
    pub localization_key: NativeUtf8Slice,
    pub fallback_text: NativeUtf8Slice,
    pub value: NativeUtf8Slice,
    pub unit_key: NativeUtf8Slice,
    pub fallback_unit: NativeUtf8Slice,
    pub texture: NativeRenderResourceHandle,
    pub font_kind: NativePresentationFontKind,
    pub font_asset: NativeRenderResourceHandle,
    pub font_family: NativeUtf8Slice,
    pub height_pixels: f32,
    pub color: NativeColor,
    pub background: NativeColor,
    pub max_distance: f32,
    pub layer: NativePresentationBillboardLayer,
    pub visible: bool,
}

/// A complete replacement descriptor for one structured world indicator. It
/// has a named presentation operation so ordinary billboards stay compact.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationStructuredBillboardDescriptor {
    pub logical_id: u64,
    pub anchor: NativePresentationAnchor,
    pub has_label: bool,
    pub label_key: NativeUtf8Slice,
    pub label_fallback_text: NativeUtf8Slice,
    pub has_icon: bool,
    pub icon: NativeRenderResourceHandle,
    pub accessible_label_key: NativeUtf8Slice,
    pub accessible_fallback_text: NativeUtf8Slice,
    pub meters: *const NativePresentationBillboardMeter,
    pub meters_len: usize,
    pub status_cues: *const NativePresentationBillboardStatusCue,
    pub status_cues_len: usize,
    pub width_pixels: f32,
    pub spacing_pixels: f32,
    pub alignment: NativePresentationBillboardAlignment,
    pub style: NativePresentationBillboardStyle,
    pub layout: NativePresentationBillboardLayout,
    pub font_kind: NativePresentationFontKind,
    pub font_asset: NativeRenderResourceHandle,
    pub font_family: NativeUtf8Slice,
    pub height_pixels: f32,
    pub color: NativeColor,
    pub background: NativeColor,
    pub max_distance: f32,
    pub layer: NativePresentationBillboardLayer,
    pub visible: bool,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationParticleVisual {
    Billboard = 1,
    Cube = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationParticleScalarKey {
    pub age: f32,
    pub value: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationParticleColorKey {
    pub age: f32,
    pub color: NativeColor,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationParticleCollisionLimitBehavior {
    Sleep = 1,
    Kill = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationParticleCollisionVolumeKind {
    Plane = 1,
    Aabb = 2,
}

/// A closed spawn-relative presentation collision volume. Plane values use
/// `normal` and `offset`; AABB values use `minimum` and `maximum`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationParticleCollisionVolume {
    pub kind: NativePresentationParticleCollisionVolumeKind,
    pub normal: NativeVec3,
    pub offset: f32,
    pub minimum: NativeVec3,
    pub maximum: NativeVec3,
}

/// Scalar collision settings for presentation particles. Volumes remain a
/// top-level bounded borrowed slice on the enclosing particle descriptor so
/// generated C# can pin and copy them synchronously.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationParticleCollision {
    pub radius: f32,
    pub restitution: f32,
    pub friction: f32,
    pub maximum_impacts: u16,
    pub sleep_speed: f32,
    pub limit_behavior: NativePresentationParticleCollisionLimitBehavior,
}

/// A full replacement descriptor for a retained particle emitter. Curves use
/// borrowed typed slices copied by Engine before the call returns. Optional
/// presentation-only collision is spawn-relative and never uses Spatial or
/// Dynamics state.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationParticleDescriptor {
    pub logical_id: u64,
    /// Retained emitter calls require this stable nonzero product identity and
    /// return it as their owner. Direct one-shot Emit ignores it; its separate
    /// `signal_id` field supplies idempotency instead.
    pub signal_id: NativeUtf8Slice,
    pub anchor: NativePresentationAnchor,
    pub visual: NativePresentationParticleVisual,
    pub sprite: NativeRenderResourceHandle,
    pub sprite_frame_count: u16,
    pub rate_per_second: f32,
    pub burst_count: u32,
    pub lifetime_min_seconds: f32,
    pub lifetime_max_seconds: f32,
    pub velocity_min: NativeVec3,
    pub velocity_max: NativeVec3,
    pub acceleration: NativeVec3,
    pub size_curve: *const NativePresentationParticleScalarKey,
    pub size_curve_len: usize,
    pub color_curve: *const NativePresentationParticleColorKey,
    pub color_curve_len: usize,
    pub flipbook_frames_per_second: f32,
    pub seed: u64,
    pub max_particles: u32,
    pub visible: bool,
    pub has_collision: bool,
    pub collision: NativePresentationParticleCollision,
    pub collision_volumes: *const NativePresentationParticleCollisionVolume,
    pub collision_volumes_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePresentationFactsReadout {
    pub active_billboards: u32,
    pub active_emitters: u32,
    pub reserved_particles: u32,
    pub emitted_bursts: u64,
    pub billboard_diagnostic_count: u32,
    pub particle_diagnostic_count: u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationDiagnosticDomain {
    None = 0,
    Billboard = 1,
    Particle = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePresentationDiagnosticCode {
    None = 0,
    InvalidDescriptor = 1,
    AssetMissing = 2,
    AssetKindMismatch = 3,
    ContentHashMismatch = 4,
    DuplicateHandle = 5,
    UnknownHandle = 6,
    DuplicateSignal = 7,
    BudgetExceeded = 8,
    AnchorMissing = 9,
    UnavailableHost = 10,
    FontLoadFailed = 11,
    IconOrSpriteLoadFailed = 12,
    HostFailure = 13,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationDiagnosticAtRequest {
    pub domain: NativePresentationDiagnosticDomain,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationDiagnosticAtReceipt {
    pub present: bool,
    pub code: NativePresentationDiagnosticCode,
    pub sequence: u32,
    pub logical_id: u64,
}

/// Opaque retained ghost-plate owner. The generated C# facade owns the
/// matching destroy call; products never receive a renderer target handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeGhostPlatePresentationHandle {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeGhostPlateCaptureLightingMode {
    Scene = 1,
    Isolated = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeGhostPlateCaptureLighting {
    pub mode: NativeGhostPlateCaptureLightingMode,
    pub ambient_color: NativeVec3,
    pub ambient_intensity: f32,
    pub key_direction: NativeVec3,
    pub key_color: NativeVec3,
    pub key_intensity: f32,
    pub fill_direction: NativeVec3,
    pub fill_color: NativeVec3,
    pub fill_intensity: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeGhostPlateCaptureSettings {
    pub resolution: u16,
    pub azimuth_degrees: f32,
    pub elevation_degrees: f32,
    pub near: f32,
    pub far: f32,
    pub field_of_view_degrees: f32,
    pub lighting: NativeGhostPlateCaptureLighting,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeGhostPlateAnchorPolicy {
    BoundsCenter = 1,
    BoundsNormalized = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeGhostPlateMapping {
    PlateLocked = 1,
    ProjectiveSurface = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeGhostPlateShellMode {
    WholeMesh = 1,
    StrictSource = 2,
    RepairedSource = 3,
}

/// Product-owned placement facts for one retained ghost plate.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeGhostPlatePlacement {
    pub transform: crate::NativeTransform,
    pub width: f32,
    pub height: f32,
}

/// Direction selection is always a hard snap. Transition mode and duration
/// deliberately do not cross the product ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeGhostPlateConfig {
    pub depth_retention: f32,
    pub anchor_policy: NativeGhostPlateAnchorPolicy,
    pub anchor_value: f32,
    pub plate_mapping: NativeGhostPlateMapping,
    pub shell_mode: NativeGhostPlateShellMode,
    pub shell_depth_epsilon: f32,
    pub sector_count: u8,
    pub sector_hysteresis_degrees: f32,
}

/// Creates a retained ghost plate from the current complete Appearance
/// snapshot entry named by stable product object ID. The Engine resolves that
/// ID to its renderer target internally.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCreateGhostPlatePresentationRequest {
    pub source_object_id: u64,
    pub placement: NativeGhostPlatePlacement,
    pub capture: NativeGhostPlateCaptureSettings,
    pub config: NativeGhostPlateConfig,
}

/// A complete placement/configuration replacement. The host realizes it as an
/// atomic replacement, so capture-bank identity changes preserve the prior
/// live presentation when preparation fails.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUpdateGhostPlatePresentationRequest {
    pub presentation: NativeGhostPlatePresentationHandle,
    pub placement: NativeGhostPlatePlacement,
    pub config: NativeGhostPlateConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRecaptureGhostPlatePresentationRequest {
    pub presentation: NativeGhostPlatePresentationHandle,
    pub capture: NativeGhostPlateCaptureSettings,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeGhostPlateFallbackReason {
    #[default]
    None = 0,
    PreparedSourceUnsupported = 1,
    RealizationFailed = 2,
}

/// Compact closed summary of the retained ghost realization limits. The two
/// profiles are the only backend states currently emitted: a single capture
/// or a hard-snapped directional capture bank. Individual bit values remain
/// named so product code can inspect the closed mask without renderer strings.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeGhostPlateLimitationMask {
    #[default]
    None = 0,
    RetainedSourceOnly = 1,
    SingleCaptureView = 2,
    FrozenAppearancePose = 4,
    WholeHierarchyRelief = 8,
    Rgba8ShellDepth = 16,
    FragmentRatiosUnavailableWithoutReadback = 32,
    GpuTimeNotMeasured = 64,
    SingleCaptureViewProfile = 127,
    DirectionalCaptureBankProfile = 125,
}

/// Copied latest renderer observation plus Engine-owned source/retention
/// facts. Renderer observation is explicitly marked absent until the bound
/// host reports its first snapshot; no backend object crosses into C#.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeGhostPlatePresentationReadout {
    pub source_object_id: u64,
    pub source_present: bool,
    pub has_renderer_observation: bool,
    pub source_matches: bool,
    pub current_sector: u32,
    pub has_local_angular_offset: bool,
    pub local_angular_offset_degrees: f32,
    pub fallback_active: bool,
    pub fallback_reason: NativeGhostPlateFallbackReason,
    pub limitation_mask: NativeGhostPlateLimitationMask,
    pub has_preparation_cpu_milliseconds: bool,
    pub preparation_cpu_milliseconds: f64,
    pub has_capture_cpu_submission_milliseconds: bool,
    pub capture_cpu_submission_milliseconds: f64,
    pub retained_sector_count: u32,
    pub retained_mesh_count: u32,
    pub retained_material_count: u32,
    pub retained_borrowed_texture_count: u32,
    pub capture: NativeGhostPlateCaptureSettings,
    pub config: NativeGhostPlateConfig,
}

pub type NativeCreateGhostPlatePresentation = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *const NativeCreateGhostPlatePresentationRequest,
    *mut NativeGhostPlatePresentationHandle,
) -> i32;
pub type NativeUpdateGhostPlatePresentation = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *const NativeUpdateGhostPlatePresentationRequest,
) -> i32;
pub type NativeRecaptureGhostPlatePresentation = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *const NativeRecaptureGhostPlatePresentationRequest,
) -> i32;
pub type NativeReadGhostPlatePresentation = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    NativeGhostPlatePresentationHandle,
    *mut NativeGhostPlatePresentationReadout,
) -> i32;
pub type NativeDestroyGhostPlatePresentation =
    unsafe extern "C" fn(*mut std::ffi::c_void, NativeGhostPlatePresentationHandle) -> i32;

impl Default for NativePresentationDiagnosticAtReceipt {
    fn default() -> Self {
        Self {
            present: false,
            code: NativePresentationDiagnosticCode::None,
            sequence: 0,
            logical_id: 0,
        }
    }
}
