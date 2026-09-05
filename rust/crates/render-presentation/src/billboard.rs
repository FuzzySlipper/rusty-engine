use std::collections::{BTreeMap, BTreeSet};

use render_model::{RenderAssetError, RenderAssetKind};
use serde::{Deserialize, Serialize};

use crate::{
    verify_asset, PresentationAssetError, PresentationAssetLookup, PresentationOp,
    PresentationOpMeta,
};

const MAX_TEXT_BYTES: usize = 256;
const MAX_KEY_BYTES: usize = 128;
const MAX_ARGUMENTS: usize = 8;
const MAX_METERS: usize = 4;
const MAX_STATUS_CUES: usize = 8;
const MAX_METER_ABS_VALUE: f32 = 1_000_000_000_000.0;
const MAX_WIDTH_PIXELS: f32 = 2_048.0;
const MAX_SPACING_PIXELS: f32 = 128.0;
const MAX_RADIUS_PIXELS: f32 = 128.0;
const MAX_SAFE_AREA_PIXELS: f32 = 4_096.0;
const MAX_DISTANCE_SCALE: f32 = 16.0;
const MAX_BILLBOARD_DIAGNOSTICS: usize = 128;
pub(crate) const MIN_POSITIVE_BILLBOARD_VALUE: f32 = f32::EPSILON;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BillboardHandle(u64);

impl BillboardHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum BillboardAnchor {
    World { position: [f32; 3] },
    EntityAttached { entity: u64, offset: [f32; 3] },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardTemplateArgument {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardTextureRef {
    pub asset: String,
    pub content_hash: String,
}

/// A host-neutral localized string. Hosts choose the active locale and retain
/// the fallback for deterministic display and accessibility behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardLocalizedText {
    pub localization_key: String,
    pub fallback_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BillboardMeterFillDirection {
    LeftToRight,
    RightToLeft,
    BottomToTop,
    TopToBottom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardMeter {
    /// Stable authored identity used by hosts when updating a meter in place.
    pub id: String,
    pub accessible_label: BillboardLocalizedText,
    pub current: f32,
    pub min: f32,
    pub max: f32,
    pub preview: Option<f32>,
    pub fill_direction: BillboardMeterFillDirection,
    pub segments: u8,
    pub fill: [f32; 4],
    pub preview_fill: [f32; 4],
    pub back: [f32; 4],
    pub border: [f32; 4],
}

impl BillboardMeter {
    /// Derive a normalized current fraction without storing a second
    /// presentation authority in the descriptor.
    pub fn current_fraction(&self) -> Option<f32> {
        normalized_fraction(self.current, self.min, self.max)
    }

    /// Derive a normalized preview fraction without storing a second
    /// presentation authority in the descriptor.
    pub fn preview_fraction(&self) -> Option<f32> {
        self.preview
            .and_then(|value| normalized_fraction(value, self.min, self.max))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardStatusCue {
    /// Stable authored identity used by hosts when updating a cue in place.
    pub id: String,
    pub label: BillboardLocalizedText,
    pub icon: Option<BillboardTextureRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardStyle {
    pub opacity: f32,
    pub backing: [f32; 4],
    pub border: [f32; 4],
    pub radius_pixels: f32,
}

impl Default for BillboardStyle {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            backing: [0.0; 4],
            border: [0.0; 4],
            radius_pixels: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BillboardAlignment {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BillboardLayoutSizing {
    ConstantPixels,
    DistanceScaled {
        reference_distance: f32,
        min_scale: f32,
        max_scale: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardSafeArea {
    pub top_pixels: f32,
    pub right_pixels: f32,
    pub bottom_pixels: f32,
    pub left_pixels: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BillboardEdgeBehavior {
    Clamp,
    Cull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BillboardOverlapBehavior {
    Stack,
    Suppress,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardLayoutPolicy {
    pub priority: i32,
    pub sizing: BillboardLayoutSizing,
    pub safe_area: BillboardSafeArea,
    pub edge_behavior: BillboardEdgeBehavior,
    pub overlap_behavior: BillboardOverlapBehavior,
}

/// A bounded, renderer-neutral world indicator. It deliberately has no input
/// or pointer-routing fields: ordinary indicators are noninteractive and
/// pointer-transparent in every host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardIndicator {
    pub label: Option<BillboardLocalizedText>,
    pub icon: Option<BillboardTextureRef>,
    pub accessible_label: BillboardLocalizedText,
    pub meters: Vec<BillboardMeter>,
    pub status_cues: Vec<BillboardStatusCue>,
    pub width_pixels: f32,
    pub spacing_pixels: f32,
    pub alignment: BillboardAlignment,
    pub style: BillboardStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BillboardContent {
    Text {
        localization_key: String,
        fallback_text: String,
        arguments: Vec<BillboardTemplateArgument>,
    },
    Value {
        label_key: String,
        fallback_label: String,
        value: String,
        unit_key: Option<String>,
        fallback_unit: Option<String>,
    },
    Icon {
        texture: BillboardTextureRef,
        alt_key: String,
        fallback_alt: String,
    },
    Structured {
        indicator: BillboardIndicator,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BillboardFontRef {
    System {
        family: String,
    },
    Asset {
        asset: String,
        content_hash: String,
        family: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BillboardLayer {
    AlwaysOnTop,
    DepthTested,
    Occluded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardDescriptor {
    pub anchor: BillboardAnchor,
    pub content: BillboardContent,
    pub font: BillboardFontRef,
    pub height_pixels: f32,
    pub color: [f32; 4],
    pub background: [f32; 4],
    pub max_distance: f32,
    pub layer: BillboardLayer,
    pub visible: bool,
    /// Legacy billboard content omits layout. Structured content must carry
    /// one so hosts have an explicit bounded screen-layout policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<BillboardLayoutPolicy>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardPatch {
    pub anchor: Option<BillboardAnchor>,
    pub content: Option<BillboardContent>,
    pub font: Option<BillboardFontRef>,
    pub height_pixels: Option<f32>,
    pub color: Option<[f32; 4]>,
    pub background: Option<[f32; 4]>,
    pub max_distance: Option<f32>,
    pub layer: Option<BillboardLayer>,
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<BillboardLayoutPolicy>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BillboardProjectionOp {
    Create {
        handle: BillboardHandle,
        descriptor: BillboardDescriptor,
    },
    Update {
        handle: BillboardHandle,
        patch: BillboardPatch,
    },
    Destroy {
        handle: BillboardHandle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BillboardProjectionDiagnosticCode {
    InvalidDescriptor,
    AssetMissing,
    AssetKindMismatch,
    ContentHashMismatch,
    DuplicateHandle,
    UnknownHandle,
    AnchorMissing,
    UnavailableHost,
    FontLoadFailed,
    IconLoadFailed,
    HostFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardProjectionDiagnostic {
    pub code: BillboardProjectionDiagnosticCode,
    pub sequence: u32,
    pub handle: Option<BillboardHandle>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillboardProjectionReadout {
    pub active_billboards: u32,
    pub referenced_fonts: u32,
    pub referenced_icons: u32,
    pub diagnostics: Vec<BillboardProjectionDiagnostic>,
}

#[derive(Debug, Clone, Default)]
pub struct BillboardProjector {
    active: BTreeMap<BillboardHandle, BillboardDescriptor>,
    referenced_fonts: BTreeSet<String>,
    referenced_icons: BTreeSet<String>,
    diagnostics: Vec<BillboardProjectionDiagnostic>,
}

impl BillboardProjector {
    pub fn project(
        &mut self,
        assets: &impl PresentationAssetLookup,
        meta: PresentationOpMeta,
        op: BillboardProjectionOp,
    ) -> Result<PresentationOp, BillboardProjectionDiagnostic> {
        let mut projected = self.project_batch(assets, vec![(meta, op)])?;
        Ok(projected.pop().expect("one input produces one operation"))
    }

    pub fn project_batch(
        &mut self,
        assets: &impl PresentationAssetLookup,
        ops: Vec<(PresentationOpMeta, BillboardProjectionOp)>,
    ) -> Result<Vec<PresentationOp>, BillboardProjectionDiagnostic> {
        let mut staged = self.clone();
        let mut projected = Vec::with_capacity(ops.len());
        for (meta, op) in ops {
            if let Err(code) = staged.validate_and_apply(assets, &op) {
                let diagnostic = BillboardProjectionDiagnostic {
                    code,
                    sequence: meta.sequence,
                    handle: operation_handle(&op),
                    message: diagnostic_message(code).to_string(),
                };
                self.retain_diagnostic(diagnostic.clone());
                return Err(diagnostic);
            }
            projected.push(PresentationOp::Billboard { meta, op });
        }
        *self = staged;
        Ok(projected)
    }

    pub fn descriptor(&self, handle: BillboardHandle) -> Option<&BillboardDescriptor> {
        self.active.get(&handle)
    }

    /// Iterates retained billboards in stable handle order for a baseline.
    pub fn active_billboards(
        &self,
    ) -> impl Iterator<Item = (BillboardHandle, &BillboardDescriptor)> + '_ {
        self.active
            .iter()
            .map(|(&handle, descriptor)| (handle, descriptor))
    }

    pub fn readout(&self) -> BillboardProjectionReadout {
        BillboardProjectionReadout {
            active_billboards: self.active.len() as u32,
            referenced_fonts: self.referenced_fonts.len() as u32,
            referenced_icons: self.referenced_icons.len() as u32,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn retain_diagnostic(&mut self, diagnostic: BillboardProjectionDiagnostic) {
        if let Some(index) = self.diagnostics.iter().position(|existing| {
            existing.code == diagnostic.code
                && existing.handle == diagnostic.handle
                && existing.message == diagnostic.message
        }) {
            self.diagnostics[index] = diagnostic;
            return;
        }
        if self.diagnostics.len() == MAX_BILLBOARD_DIAGNOSTICS {
            self.diagnostics.remove(0);
        }
        self.diagnostics.push(diagnostic);
    }

    fn validate_and_apply(
        &mut self,
        assets: &impl PresentationAssetLookup,
        op: &BillboardProjectionOp,
    ) -> Result<(), BillboardProjectionDiagnosticCode> {
        match op {
            BillboardProjectionOp::Create { handle, descriptor } => {
                if self.active.contains_key(handle) {
                    return Err(BillboardProjectionDiagnosticCode::DuplicateHandle);
                }
                validate_descriptor(assets, descriptor)?;
                self.record_assets(descriptor);
                self.active.insert(*handle, descriptor.clone());
            }
            BillboardProjectionOp::Update { handle, patch } => {
                let current = self
                    .active
                    .get(handle)
                    .cloned()
                    .ok_or(BillboardProjectionDiagnosticCode::UnknownHandle)?;
                let updated = apply_patch(current, patch);
                validate_descriptor(assets, &updated)?;
                self.record_assets(&updated);
                self.active.insert(*handle, updated);
            }
            BillboardProjectionOp::Destroy { handle } => {
                if self.active.remove(handle).is_none() {
                    return Err(BillboardProjectionDiagnosticCode::UnknownHandle);
                }
            }
        }
        Ok(())
    }

    fn record_assets(&mut self, descriptor: &BillboardDescriptor) {
        if let BillboardFontRef::Asset { asset, .. } = &descriptor.font {
            self.referenced_fonts.insert(asset.clone());
        }
        match &descriptor.content {
            BillboardContent::Icon { texture, .. } => {
                self.referenced_icons.insert(texture.asset.clone());
            }
            BillboardContent::Structured { indicator } => {
                if let Some(icon) = &indicator.icon {
                    self.referenced_icons.insert(icon.asset.clone());
                }
                for cue in &indicator.status_cues {
                    if let Some(icon) = &cue.icon {
                        self.referenced_icons.insert(icon.asset.clone());
                    }
                }
            }
            BillboardContent::Text { .. } | BillboardContent::Value { .. } => {}
        }
    }
}

pub(crate) fn validate_descriptor(
    assets: &impl PresentationAssetLookup,
    descriptor: &BillboardDescriptor,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    if !anchor_is_finite(&descriptor.anchor)
        || !in_range(descriptor.height_pixels, 8.0, 256.0)
        || !in_range(
            descriptor.max_distance,
            MIN_POSITIVE_BILLBOARD_VALUE,
            10_000.0,
        )
        || !color_is_valid(descriptor.color)
        || !color_is_valid(descriptor.background)
    {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    match &descriptor.content {
        BillboardContent::Structured { .. } => {
            let layout = descriptor
                .layout
                .as_ref()
                .ok_or(BillboardProjectionDiagnosticCode::InvalidDescriptor)?;
            validate_layout_policy(layout)?;
        }
        BillboardContent::Text { .. }
        | BillboardContent::Value { .. }
        | BillboardContent::Icon { .. } => {
            if descriptor.layout.is_some() {
                return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
            }
        }
    }
    validate_content(assets, &descriptor.content)?;
    validate_font(assets, &descriptor.font)
}

pub(crate) fn validate_content(
    assets: &impl PresentationAssetLookup,
    content: &BillboardContent,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    match content {
        BillboardContent::Text {
            localization_key,
            fallback_text,
            arguments,
        } => {
            validate_key(localization_key)?;
            validate_text(fallback_text)?;
            validate_arguments(arguments)
        }
        BillboardContent::Value {
            label_key,
            fallback_label,
            value,
            unit_key,
            fallback_unit,
        } => {
            validate_key(label_key)?;
            validate_text(fallback_label)?;
            validate_text(value)?;
            optional(unit_key.as_deref(), validate_key)?;
            optional(fallback_unit.as_deref(), validate_text)
        }
        BillboardContent::Icon {
            texture,
            alt_key,
            fallback_alt,
        } => {
            validate_key(alt_key)?;
            validate_text(fallback_alt)?;
            validate_texture_ref(assets, texture)
        }
        BillboardContent::Structured { indicator } => validate_indicator(assets, indicator),
    }
}

fn validate_indicator(
    assets: &impl PresentationAssetLookup,
    indicator: &BillboardIndicator,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    optional_localized_text(indicator.label.as_ref())?;
    validate_localized_text(&indicator.accessible_label)?;
    if indicator.meters.len() > MAX_METERS || indicator.status_cues.len() > MAX_STATUS_CUES {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    if !in_range(indicator.width_pixels, 1.0, MAX_WIDTH_PIXELS)
        || !in_range(indicator.spacing_pixels, 0.0, MAX_SPACING_PIXELS)
        || !validate_style(&indicator.style)
    {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    if let Some(icon) = &indicator.icon {
        validate_texture_ref(assets, icon)?;
    }

    let mut meter_ids = BTreeSet::new();
    for meter in &indicator.meters {
        validate_meter(meter)?;
        if !meter_ids.insert(meter.id.as_str()) {
            return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
        }
    }

    let mut cue_ids = BTreeSet::new();
    for cue in &indicator.status_cues {
        validate_key(&cue.id)?;
        validate_localized_text(&cue.label)?;
        if !cue_ids.insert(cue.id.as_str()) {
            return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
        }
        if let Some(icon) = &cue.icon {
            validate_texture_ref(assets, icon)?;
        }
    }
    Ok(())
}

fn validate_meter(meter: &BillboardMeter) -> Result<(), BillboardProjectionDiagnosticCode> {
    validate_key(&meter.id)?;
    validate_localized_text(&meter.accessible_label)?;
    let range = meter.max - meter.min;
    if !meter.current.is_finite()
        || !meter.min.is_finite()
        || !meter.max.is_finite()
        || meter.min >= meter.max
        || meter.current < meter.min
        || meter.current > meter.max
        || !range.is_finite()
        || meter.current.abs() > MAX_METER_ABS_VALUE
        || meter.min.abs() > MAX_METER_ABS_VALUE
        || meter.max.abs() > MAX_METER_ABS_VALUE
    {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    if let Some(preview) = meter.preview {
        if !preview.is_finite()
            || preview.abs() > MAX_METER_ABS_VALUE
            || preview < meter.min
            || preview > meter.max
        {
            return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
        }
    }
    if !(1..=32).contains(&meter.segments)
        || !color_is_valid(meter.fill)
        || !color_is_valid(meter.preview_fill)
        || !color_is_valid(meter.back)
        || !color_is_valid(meter.border)
    {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    Ok(())
}

fn validate_localized_text(
    text: &BillboardLocalizedText,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    validate_key(&text.localization_key)?;
    validate_text(&text.fallback_text)
}

fn optional_localized_text(
    text: Option<&BillboardLocalizedText>,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    text.map_or(Ok(()), validate_localized_text)
}

fn validate_texture_ref(
    assets: &impl PresentationAssetLookup,
    texture: &BillboardTextureRef,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    validate_key(&texture.asset)?;
    validate_text(&texture.content_hash)?;
    verify_asset(
        assets,
        &texture.asset,
        RenderAssetKind::Texture,
        Some(&texture.content_hash),
    )
    .map_err(asset_diagnostic)
}

fn validate_style(style: &BillboardStyle) -> bool {
    in_range(style.opacity, 0.0, 1.0)
        && color_is_valid(style.backing)
        && color_is_valid(style.border)
        && in_range(style.radius_pixels, 0.0, MAX_RADIUS_PIXELS)
}

pub(crate) fn validate_layout_policy(
    policy: &BillboardLayoutPolicy,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    if !validate_safe_area(policy.safe_area) {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    match &policy.sizing {
        BillboardLayoutSizing::ConstantPixels => {}
        BillboardLayoutSizing::DistanceScaled {
            reference_distance,
            min_scale,
            max_scale,
        } => {
            if !in_range(*reference_distance, MIN_POSITIVE_BILLBOARD_VALUE, 10_000.0)
                || !in_range(*min_scale, MIN_POSITIVE_BILLBOARD_VALUE, MAX_DISTANCE_SCALE)
                || !in_range(*max_scale, MIN_POSITIVE_BILLBOARD_VALUE, MAX_DISTANCE_SCALE)
                || min_scale > max_scale
            {
                return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
            }
        }
    }
    Ok(())
}

fn validate_safe_area(safe_area: BillboardSafeArea) -> bool {
    [
        safe_area.top_pixels,
        safe_area.right_pixels,
        safe_area.bottom_pixels,
        safe_area.left_pixels,
    ]
    .into_iter()
    .all(|value| in_range(value, 0.0, MAX_SAFE_AREA_PIXELS))
}

pub(crate) fn validate_font(
    assets: &impl PresentationAssetLookup,
    font: &BillboardFontRef,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    match font {
        BillboardFontRef::System { family } => validate_text(family),
        BillboardFontRef::Asset {
            asset,
            content_hash,
            family,
        } => {
            validate_text(family)?;
            verify_asset(assets, asset, RenderAssetKind::Font, Some(content_hash))
                .map_err(asset_diagnostic)
        }
    }
}

fn apply_patch(mut descriptor: BillboardDescriptor, patch: &BillboardPatch) -> BillboardDescriptor {
    if let Some(value) = &patch.anchor {
        descriptor.anchor = value.clone();
    }
    if let Some(value) = &patch.content {
        let structured = matches!(value, BillboardContent::Structured { .. });
        descriptor.content = value.clone();
        if !structured && patch.layout.is_none() {
            descriptor.layout = None;
        }
    }
    if let Some(value) = &patch.font {
        descriptor.font = value.clone();
    }
    if let Some(value) = patch.height_pixels {
        descriptor.height_pixels = value;
    }
    if let Some(value) = patch.color {
        descriptor.color = value;
    }
    if let Some(value) = patch.background {
        descriptor.background = value;
    }
    if let Some(value) = patch.max_distance {
        descriptor.max_distance = value;
    }
    if let Some(value) = patch.layer {
        descriptor.layer = value;
    }
    if let Some(value) = patch.visible {
        descriptor.visible = value;
    }
    if let Some(value) = &patch.layout {
        descriptor.layout = Some(value.clone());
    }
    descriptor
}

fn anchor_is_finite(anchor: &BillboardAnchor) -> bool {
    match anchor {
        BillboardAnchor::World { position }
        | BillboardAnchor::EntityAttached {
            offset: position, ..
        } => position.iter().all(|value| value.is_finite()),
    }
}

fn color_is_valid(color: [f32; 4]) -> bool {
    color.into_iter().all(|value| in_range(value, 0.0, 1.0))
}

fn in_range(value: f32, minimum: f32, maximum: f32) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

fn normalized_fraction(value: f32, minimum: f32, maximum: f32) -> Option<f32> {
    let range = maximum - minimum;
    if value.is_finite()
        && minimum.is_finite()
        && maximum.is_finite()
        && range.is_finite()
        && range > 0.0
        && value >= minimum
        && value <= maximum
    {
        Some((value - minimum) / range)
    } else {
        None
    }
}

fn validate_key(value: &str) -> Result<(), BillboardProjectionDiagnosticCode> {
    if value.is_empty() || value.len() > MAX_KEY_BYTES {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), BillboardProjectionDiagnosticCode> {
    if value.is_empty() || value.len() > MAX_TEXT_BYTES {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    Ok(())
}

fn optional(
    value: Option<&str>,
    validate: impl FnOnce(&str) -> Result<(), BillboardProjectionDiagnosticCode>,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    value.map_or(Ok(()), validate)
}

fn validate_arguments(
    arguments: &[BillboardTemplateArgument],
) -> Result<(), BillboardProjectionDiagnosticCode> {
    if arguments.len() > MAX_ARGUMENTS {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    let mut names = BTreeSet::new();
    for argument in arguments {
        validate_key(&argument.name)?;
        validate_text(&argument.value)?;
        if !names.insert(argument.name.as_str()) {
            return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
        }
    }
    Ok(())
}

fn asset_diagnostic(error: PresentationAssetError) -> BillboardProjectionDiagnosticCode {
    match error {
        PresentationAssetError::Missing(_) => BillboardProjectionDiagnosticCode::AssetMissing,
        PresentationAssetError::Invalid(RenderAssetError::ContentHashMismatch { .. }) => {
            BillboardProjectionDiagnosticCode::ContentHashMismatch
        }
        PresentationAssetError::Invalid(_) => BillboardProjectionDiagnosticCode::AssetKindMismatch,
    }
}

fn operation_handle(op: &BillboardProjectionOp) -> Option<BillboardHandle> {
    Some(match op {
        BillboardProjectionOp::Create { handle, .. }
        | BillboardProjectionOp::Update { handle, .. }
        | BillboardProjectionOp::Destroy { handle } => *handle,
    })
}

const fn diagnostic_message(code: BillboardProjectionDiagnosticCode) -> &'static str {
    match code {
        BillboardProjectionDiagnosticCode::InvalidDescriptor => "billboard descriptor is invalid",
        BillboardProjectionDiagnosticCode::AssetMissing => "billboard resource is unavailable",
        BillboardProjectionDiagnosticCode::AssetKindMismatch => {
            "billboard resource has the wrong kind"
        }
        BillboardProjectionDiagnosticCode::ContentHashMismatch => {
            "billboard resource content hash does not match"
        }
        BillboardProjectionDiagnosticCode::DuplicateHandle => "billboard handle is already active",
        BillboardProjectionDiagnosticCode::UnknownHandle => "billboard handle is not active",
        BillboardProjectionDiagnosticCode::AnchorMissing => {
            "billboard entity anchor is unavailable"
        }
        BillboardProjectionDiagnosticCode::UnavailableHost => "billboard host is unavailable",
        BillboardProjectionDiagnosticCode::FontLoadFailed => "billboard font failed to load",
        BillboardProjectionDiagnosticCode::IconLoadFailed => "billboard icon failed to load",
        BillboardProjectionDiagnosticCode::HostFailure => "billboard host operation failed",
    }
}
