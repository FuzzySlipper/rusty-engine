use std::collections::BTreeMap;

use render_model::{RenderAssetKind, ResolvedRenderAsset, JSON_SAFE_U64_MAX};
use serde::{Deserialize, Serialize};

use crate::billboard::{
    validate_content, validate_descriptor, validate_font, validate_layout_policy,
    MIN_POSITIVE_BILLBOARD_VALUE,
};
use crate::{
    AnimationControllerProjectionState, AnimationProjectionOp, AudioBusControl, AudioEmitter,
    AudioProjectionOp, AudioSourceDescriptor, AudioSourcePatch, BillboardAnchor, BillboardContent,
    BillboardDescriptor, BillboardFontRef, BillboardLayoutPolicy, BillboardLayoutSizing,
    BillboardMeter, BillboardPatch, BillboardProjectionOp, BillboardStyle, BillboardTextureRef,
    ParticleAnchor, ParticleCollisionVolume, ParticleEmitterDescriptor, ParticleEmitterPatch,
    ParticleProjectionOp, TelemetryOverlayProjectionOp,
};

pub const PRESENTATION_FRAME_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationOpMeta {
    pub sequence: u32,
}

impl PresentationOpMeta {
    pub const fn new(sequence: u32) -> Self {
        Self { sequence }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "domain", rename_all = "camelCase", deny_unknown_fields)]
pub enum PresentationOp {
    Audio {
        meta: PresentationOpMeta,
        op: AudioProjectionOp,
    },
    Billboard {
        meta: PresentationOpMeta,
        op: BillboardProjectionOp,
    },
    Particle {
        meta: PresentationOpMeta,
        op: ParticleProjectionOp,
    },
    TelemetryOverlay {
        meta: PresentationOpMeta,
        op: TelemetryOverlayProjectionOp,
    },
    Animation {
        meta: PresentationOpMeta,
        op: AnimationProjectionOp,
    },
}

impl PresentationOp {
    pub const fn meta(&self) -> PresentationOpMeta {
        match self {
            Self::Audio { meta, .. }
            | Self::Billboard { meta, .. }
            | Self::Particle { meta, .. }
            | Self::TelemetryOverlay { meta, .. }
            | Self::Animation { meta, .. } => *meta,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationFrameDiff {
    pub schema_version: u32,
    pub ops: Vec<PresentationOp>,
}

impl Default for PresentationFrameDiff {
    fn default() -> Self {
        Self::new()
    }
}

impl PresentationFrameDiff {
    pub const fn new() -> Self {
        Self {
            schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
            ops: Vec::new(),
        }
    }

    pub fn try_from_ops(ops: Vec<PresentationOp>) -> Result<Self, PresentationFrameError> {
        let frame = Self {
            schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
            ops,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), PresentationFrameError> {
        if self.schema_version != PRESENTATION_FRAME_SCHEMA_VERSION {
            return Err(PresentationFrameError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        for (index, op) in self.ops.iter().enumerate() {
            let expected = u32::try_from(index).map_err(|_| PresentationFrameError::TooManyOps)?;
            let actual = op.meta().sequence;
            if actual != expected {
                return Err(PresentationFrameError::NonContiguousSequence { expected, actual });
            }
            validate_json_safe_integers(op, actual)?;
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn encode_json(&self) -> Result<String, PresentationJsonError> {
        self.validate()
            .map_err(PresentationJsonError::InvalidFrame)?;
        serde_json::to_string_pretty(self).map_err(PresentationJsonError::Encode)
    }

    pub fn decode_json(input: &str) -> Result<Self, PresentationJsonError> {
        let frame: Self = serde_json::from_str(input).map_err(PresentationJsonError::Decode)?;
        frame
            .validate()
            .map_err(PresentationJsonError::InvalidFrame)?;
        Ok(frame)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationFrameError {
    UnsupportedSchemaVersion(u32),
    TooManyOps,
    NonContiguousSequence {
        expected: u32,
        actual: u32,
    },
    UnsafeJsonInteger {
        sequence: u32,
        field: &'static str,
        value: u64,
    },
    NonFiniteNumber {
        sequence: u32,
        field: &'static str,
    },
    InvalidDescriptor {
        sequence: u32,
        field: &'static str,
    },
}

fn validate_json_safe_integers(
    op: &PresentationOp,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match op {
        PresentationOp::Audio { op, .. } => match op {
            AudioProjectionOp::Emit { descriptor, .. } => validate_audio(descriptor, sequence),
            AudioProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "audio.handle")?;
                validate_audio(descriptor, sequence)
            }
            AudioProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "audio.handle")?;
                validate_audio_patch(patch, sequence)
            }
            AudioProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "audio.handle")
            }
            AudioProjectionOp::VoiceControl { handle, .. } => {
                json_safe(handle.raw(), sequence, "audio.handle")
            }
            AudioProjectionOp::BusControl { control, .. } => {
                validate_audio_bus_control(control, sequence)
            }
        },
        PresentationOp::Billboard { op, .. } => match op {
            BillboardProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "billboard.handle")?;
                validate_billboard(descriptor, sequence)
            }
            BillboardProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "billboard.handle")?;
                validate_billboard_patch(patch, sequence)
            }
            BillboardProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "billboard.handle")
            }
        },
        PresentationOp::Particle { op, .. } => match op {
            ParticleProjectionOp::Emit { descriptor, .. } => {
                validate_particle(descriptor, sequence)
            }
            ParticleProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "particle.handle")?;
                validate_particle(descriptor, sequence)
            }
            ParticleProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "particle.handle")?;
                validate_particle_patch(patch, sequence)
            }
            ParticleProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "particle.handle")
            }
        },
        PresentationOp::TelemetryOverlay { op, .. } => {
            let handle = match op {
                TelemetryOverlayProjectionOp::Create { handle, .. }
                | TelemetryOverlayProjectionOp::Update { handle, .. }
                | TelemetryOverlayProjectionOp::Destroy { handle } => handle,
            };
            json_safe(handle.raw(), sequence, "telemetryOverlay.handle")
        }
        PresentationOp::Animation { op, .. } => match op {
            AnimationProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "animation.handle")?;
                json_safe(descriptor.target.raw(), sequence, "animation.target")?;
                validate_animation_controller(&descriptor.controller, sequence)
            }
            AnimationProjectionOp::Update { handle, controller } => {
                json_safe(handle.raw(), sequence, "animation.handle")?;
                validate_animation_controller(controller, sequence)
            }
            AnimationProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "animation.handle")
            }
        },
    }
}

fn validate_audio(
    descriptor: &AudioSourceDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    validate_audio_emitter(&descriptor.emitter, sequence)
}

fn validate_audio_patch(
    patch: &AudioSourcePatch,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    patch
        .emitter
        .as_ref()
        .map_or(Ok(()), |emitter| validate_audio_emitter(emitter, sequence))
}

fn validate_audio_bus_control(
    control: &AudioBusControl,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match control {
        AudioBusControl::SetVolume { volume } => {
            finite_f32(*volume, sequence, "audio.control.volume")?;
            if !(0.0..=1.0).contains(volume) {
                return Err(PresentationFrameError::InvalidDescriptor {
                    sequence,
                    field: "audio.control.volume",
                });
            }
            Ok(())
        }
        AudioBusControl::SetMuted { .. } => Ok(()),
    }
}

fn validate_audio_emitter(
    emitter: &AudioEmitter,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match emitter {
        AudioEmitter::EntityAttached { entity, .. } => {
            json_safe(*entity, sequence, "audio.emitter.entity")
        }
        AudioEmitter::Global2d | AudioEmitter::World3d { .. } => Ok(()),
    }
}

fn validate_billboard(
    descriptor: &BillboardDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    validate_billboard_anchor(&descriptor.anchor, sequence)?;
    validate_billboard_numbers(descriptor, sequence)?;
    let assets = synthetic_billboard_assets(Some(&descriptor.font), Some(&descriptor.content));
    validate_descriptor(&assets, descriptor).map_err(|_| {
        PresentationFrameError::InvalidDescriptor {
            sequence,
            field: "billboard",
        }
    })
}

fn validate_billboard_patch(
    patch: &BillboardPatch,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    if let Some(anchor) = &patch.anchor {
        validate_billboard_anchor(anchor, sequence)?;
    }
    if let Some(height_pixels) = patch.height_pixels {
        finite_f32(height_pixels, sequence, "billboard.heightPixels")?;
        if !(8.0..=256.0).contains(&height_pixels) {
            return invalid_billboard(sequence, "billboard.heightPixels");
        }
    }
    if let Some(color) = patch.color {
        finite_color(color, sequence, "billboard.color")?;
        if !wire_color_is_valid(color) {
            return invalid_billboard(sequence, "billboard.color");
        }
    }
    if let Some(background) = patch.background {
        finite_color(background, sequence, "billboard.background")?;
        if !wire_color_is_valid(background) {
            return invalid_billboard(sequence, "billboard.background");
        }
    }
    if let Some(max_distance) = patch.max_distance {
        finite_f32(max_distance, sequence, "billboard.maxDistance")?;
        if !(MIN_POSITIVE_BILLBOARD_VALUE..=10_000.0).contains(&max_distance) {
            return invalid_billboard(sequence, "billboard.maxDistance");
        }
    }
    if let Some(content) = &patch.content {
        validate_billboard_content_numbers(content, sequence)?;
        let assets = synthetic_billboard_assets(None, Some(content));
        validate_content(&assets, content).map_err(|_| {
            PresentationFrameError::InvalidDescriptor {
                sequence,
                field: "billboard.content",
            }
        })?;
    }
    if let Some(font) = &patch.font {
        let assets = synthetic_billboard_assets(Some(font), None);
        validate_font(&assets, font).map_err(|_| PresentationFrameError::InvalidDescriptor {
            sequence,
            field: "billboard.font",
        })?;
    }
    if let Some(layout) = &patch.layout {
        validate_layout_numbers(layout, sequence)?;
        validate_layout_policy(layout).map_err(|_| PresentationFrameError::InvalidDescriptor {
            sequence,
            field: "billboard.layout",
        })?;
    }
    Ok(())
}

fn invalid_billboard(sequence: u32, field: &'static str) -> Result<(), PresentationFrameError> {
    Err(PresentationFrameError::InvalidDescriptor { sequence, field })
}

fn wire_color_is_valid(color: [f32; 4]) -> bool {
    color.into_iter().all(|value| (0.0..=1.0).contains(&value))
}

fn synthetic_billboard_assets(
    font: Option<&BillboardFontRef>,
    content: Option<&BillboardContent>,
) -> BTreeMap<String, ResolvedRenderAsset> {
    let mut assets = BTreeMap::new();
    if let Some(BillboardFontRef::Asset {
        asset,
        content_hash,
        ..
    }) = font
    {
        insert_synthetic_asset(&mut assets, asset, RenderAssetKind::Font, content_hash);
    }
    if let Some(content) = content {
        match content {
            BillboardContent::Icon { texture, .. } => {
                insert_synthetic_texture(&mut assets, texture);
            }
            BillboardContent::Structured { indicator } => {
                if let Some(texture) = &indicator.icon {
                    insert_synthetic_texture(&mut assets, texture);
                }
                for cue in &indicator.status_cues {
                    if let Some(texture) = &cue.icon {
                        insert_synthetic_texture(&mut assets, texture);
                    }
                }
            }
            BillboardContent::Text { .. } | BillboardContent::Value { .. } => {}
        }
    }
    assets
}

fn insert_synthetic_texture(
    assets: &mut BTreeMap<String, ResolvedRenderAsset>,
    texture: &BillboardTextureRef,
) {
    insert_synthetic_asset(
        assets,
        &texture.asset,
        RenderAssetKind::Texture,
        &texture.content_hash,
    );
}

fn insert_synthetic_asset(
    assets: &mut BTreeMap<String, ResolvedRenderAsset>,
    id: &str,
    kind: RenderAssetKind,
    content_hash: &str,
) {
    assets.insert(
        id.to_string(),
        ResolvedRenderAsset {
            id: id.to_string(),
            kind,
            content_hash: Some(content_hash.to_string()),
            version: 1,
        },
    );
}

fn validate_billboard_anchor(
    anchor: &BillboardAnchor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match anchor {
        BillboardAnchor::EntityAttached { entity, offset, .. } => {
            json_safe(*entity, sequence, "billboard.anchor.entity")?;
            finite_values(*offset, sequence, "billboard.anchor.offset")
        }
        BillboardAnchor::World { position } => {
            finite_values(*position, sequence, "billboard.anchor.position")
        }
    }
}

fn validate_billboard_numbers(
    descriptor: &BillboardDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    finite_f32(descriptor.height_pixels, sequence, "billboard.heightPixels")?;
    finite_color(descriptor.color, sequence, "billboard.color")?;
    finite_color(descriptor.background, sequence, "billboard.background")?;
    finite_f32(descriptor.max_distance, sequence, "billboard.maxDistance")?;
    validate_billboard_content_numbers(&descriptor.content, sequence)?;
    if let Some(layout) = &descriptor.layout {
        validate_layout_numbers(layout, sequence)?;
    }
    Ok(())
}

fn validate_billboard_content_numbers(
    content: &BillboardContent,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    if let BillboardContent::Structured { indicator } = content {
        finite_f32(
            indicator.width_pixels,
            sequence,
            "billboard.indicator.widthPixels",
        )?;
        finite_f32(
            indicator.spacing_pixels,
            sequence,
            "billboard.indicator.spacingPixels",
        )?;
        validate_style_numbers(&indicator.style, sequence)?;
        for meter in &indicator.meters {
            validate_meter_numbers(meter, sequence)?;
        }
    }
    Ok(())
}

fn validate_meter_numbers(
    meter: &BillboardMeter,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    finite_f32(meter.current, sequence, "billboard.meter.current")?;
    finite_f32(meter.min, sequence, "billboard.meter.min")?;
    finite_f32(meter.max, sequence, "billboard.meter.max")?;
    if let Some(preview) = meter.preview {
        finite_f32(preview, sequence, "billboard.meter.preview")?;
    }
    finite_color(meter.fill, sequence, "billboard.meter.fill")?;
    finite_color(meter.preview_fill, sequence, "billboard.meter.previewFill")?;
    finite_color(meter.back, sequence, "billboard.meter.back")?;
    finite_color(meter.border, sequence, "billboard.meter.border")
}

fn validate_style_numbers(
    style: &BillboardStyle,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    finite_f32(style.opacity, sequence, "billboard.indicator.style.opacity")?;
    finite_color(style.backing, sequence, "billboard.indicator.style.backing")?;
    finite_color(style.border, sequence, "billboard.indicator.style.border")?;
    finite_f32(
        style.radius_pixels,
        sequence,
        "billboard.indicator.style.radiusPixels",
    )
}

fn validate_layout_numbers(
    layout: &BillboardLayoutPolicy,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    finite_color(
        [
            layout.safe_area.top_pixels,
            layout.safe_area.right_pixels,
            layout.safe_area.bottom_pixels,
            layout.safe_area.left_pixels,
        ],
        sequence,
        "billboard.layout.safeArea",
    )?;
    if let BillboardLayoutSizing::DistanceScaled {
        reference_distance,
        min_scale,
        max_scale,
    } = &layout.sizing
    {
        finite_f32(
            *reference_distance,
            sequence,
            "billboard.layout.sizing.referenceDistance",
        )?;
        finite_f32(*min_scale, sequence, "billboard.layout.sizing.minScale")?;
        finite_f32(*max_scale, sequence, "billboard.layout.sizing.maxScale")?;
    }
    Ok(())
}

fn finite_color(
    values: [f32; 4],
    sequence: u32,
    field: &'static str,
) -> Result<(), PresentationFrameError> {
    finite_values(values, sequence, field)
}

fn finite_values<const N: usize>(
    values: [f32; N],
    sequence: u32,
    field: &'static str,
) -> Result<(), PresentationFrameError> {
    for value in values {
        finite_f32(value, sequence, field)?;
    }
    Ok(())
}

fn finite_f32(
    value: f32,
    sequence: u32,
    field: &'static str,
) -> Result<(), PresentationFrameError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(PresentationFrameError::NonFiniteNumber { sequence, field })
    }
}

fn validate_particle(
    descriptor: &ParticleEmitterDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    json_safe(descriptor.seed, sequence, "particle.seed")?;
    validate_particle_anchor(&descriptor.anchor, sequence)?;
    if let Some(collision) = &descriptor.collision {
        validate_particle_collision(collision, sequence)?;
    }
    Ok(())
}

fn validate_particle_patch(
    patch: &ParticleEmitterPatch,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    patch
        .anchor
        .as_ref()
        .map_or(Ok(()), |anchor| validate_particle_anchor(anchor, sequence))?;
    if let Some(Some(collision)) = &patch.collision {
        validate_particle_collision(collision, sequence)?;
    }
    Ok(())
}

fn validate_particle_collision(
    collision: &crate::ParticleCollisionDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    finite_f32(collision.radius, sequence, "particle.collision.radius")?;
    finite_f32(
        collision.restitution,
        sequence,
        "particle.collision.restitution",
    )?;
    finite_f32(collision.friction, sequence, "particle.collision.friction")?;
    finite_f32(
        collision.sleep_speed,
        sequence,
        "particle.collision.sleepSpeed",
    )?;
    for volume in &collision.volumes {
        match volume {
            ParticleCollisionVolume::Plane { normal, offset } => {
                finite_values(*normal, sequence, "particle.collision.plane.normal")?;
                finite_f32(*offset, sequence, "particle.collision.plane.offset")?;
            }
            ParticleCollisionVolume::Aabb { minimum, maximum } => {
                finite_values(*minimum, sequence, "particle.collision.aabb.minimum")?;
                finite_values(*maximum, sequence, "particle.collision.aabb.maximum")?;
            }
        }
    }
    Ok(())
}

fn validate_particle_anchor(
    anchor: &ParticleAnchor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match anchor {
        ParticleAnchor::EntityAttached { entity, .. } => {
            json_safe(*entity, sequence, "particle.anchor.entity")
        }
        ParticleAnchor::World { .. } => Ok(()),
    }
}

fn validate_animation_controller(
    controller: &AnimationControllerProjectionState,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    json_safe(controller.entity, sequence, "animation.controller.entity")?;
    json_safe(
        controller.revision,
        sequence,
        "animation.controller.revision",
    )?;
    json_safe(
        controller.controller_tick,
        sequence,
        "animation.controller.controllerTick",
    )?;
    if let Some(fact) = &controller.transition_fact {
        json_safe(
            fact.controller_tick,
            sequence,
            "animation.controller.transitionFact.controllerTick",
        )?;
    }
    Ok(())
}

fn json_safe(value: u64, sequence: u32, field: &'static str) -> Result<(), PresentationFrameError> {
    if value <= JSON_SAFE_U64_MAX {
        Ok(())
    } else {
        Err(PresentationFrameError::UnsafeJsonInteger {
            sequence,
            field,
            value,
        })
    }
}

#[derive(Debug)]
pub enum PresentationJsonError {
    Encode(serde_json::Error),
    Decode(serde_json::Error),
    InvalidFrame(PresentationFrameError),
}

impl core::fmt::Display for PresentationJsonError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PresentationJsonError {}
