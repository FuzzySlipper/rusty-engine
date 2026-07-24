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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
                self.diagnostics.push(diagnostic.clone());
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
        if let BillboardContent::Icon { texture, .. } = &descriptor.content {
            self.referenced_icons.insert(texture.asset.clone());
        }
    }
}

fn validate_descriptor(
    assets: &impl PresentationAssetLookup,
    descriptor: &BillboardDescriptor,
) -> Result<(), BillboardProjectionDiagnosticCode> {
    if !anchor_is_finite(&descriptor.anchor)
        || !in_range(descriptor.height_pixels, 8.0, 256.0)
        || !in_range(descriptor.max_distance, f32::EPSILON, 10_000.0)
        || !color_is_valid(descriptor.color)
        || !color_is_valid(descriptor.background)
    {
        return Err(BillboardProjectionDiagnosticCode::InvalidDescriptor);
    }
    validate_content(assets, &descriptor.content)?;
    validate_font(assets, &descriptor.font)
}

fn validate_content(
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
            verify_asset(
                assets,
                &texture.asset,
                RenderAssetKind::Texture,
                Some(&texture.content_hash),
            )
            .map_err(asset_diagnostic)
        }
    }
}

fn validate_font(
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
        descriptor.content = value.clone();
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
