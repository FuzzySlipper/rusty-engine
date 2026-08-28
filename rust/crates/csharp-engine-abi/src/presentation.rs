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

/// A full replacement descriptor for a retained particle emitter. Curves use
/// borrowed typed slices copied by Engine before the call returns. Presentation-
/// only collision remains a separate future typed surface rather than a JSON
/// escape hatch.
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
