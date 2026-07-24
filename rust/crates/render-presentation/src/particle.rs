use std::collections::{BTreeMap, BTreeSet};

use render_model::{RenderAssetError, RenderAssetKind};
use serde::{Deserialize, Serialize};

use crate::{
    verify_asset, PresentationAssetError, PresentationAssetLookup, PresentationOp,
    PresentationOpMeta,
};

const MAX_CURVE_KEYS: usize = 8;
const JSON_SAFE_U64_MAX: u64 = (1_u64 << 53) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParticleEmitterHandle(u64);

impl ParticleEmitterHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ParticleAnchor {
    World { position: [f32; 3] },
    EntityAttached { entity: u64, offset: [f32; 3] },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleSpriteRef {
    pub asset: String,
    pub content_hash: String,
    pub frame_count: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleScalarKey {
    pub age: f32,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleColorKey {
    pub age: f32,
    pub color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleEmitterDescriptor {
    pub anchor: ParticleAnchor,
    pub sprite: ParticleSpriteRef,
    pub rate_per_second: f32,
    pub burst_count: u32,
    pub lifetime_seconds: [f32; 2],
    pub velocity_min: [f32; 3],
    pub velocity_max: [f32; 3],
    pub acceleration: [f32; 3],
    pub size_curve: Vec<ParticleScalarKey>,
    pub color_curve: Vec<ParticleColorKey>,
    pub flipbook_frames_per_second: f32,
    pub seed: u64,
    pub max_particles: u32,
    pub visible: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleEmitterPatch {
    pub anchor: Option<ParticleAnchor>,
    pub sprite: Option<ParticleSpriteRef>,
    pub rate_per_second: Option<f32>,
    pub burst_count: Option<u32>,
    pub lifetime_seconds: Option<[f32; 2]>,
    pub velocity_min: Option<[f32; 3]>,
    pub velocity_max: Option<[f32; 3]>,
    pub acceleration: Option<[f32; 3]>,
    pub size_curve: Option<Vec<ParticleScalarKey>>,
    pub color_curve: Option<Vec<ParticleColorKey>>,
    pub flipbook_frames_per_second: Option<f32>,
    pub max_particles: Option<u32>,
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParticleProjectionOp {
    Emit {
        signal_id: String,
        descriptor: ParticleEmitterDescriptor,
    },
    Create {
        handle: ParticleEmitterHandle,
        descriptor: ParticleEmitterDescriptor,
    },
    Update {
        handle: ParticleEmitterHandle,
        patch: ParticleEmitterPatch,
    },
    Destroy {
        handle: ParticleEmitterHandle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticleProjectionDiagnosticCode {
    InvalidDescriptor,
    AssetMissing,
    AssetKindMismatch,
    ContentHashMismatch,
    DuplicateSignal,
    DuplicateHandle,
    UnknownHandle,
    AnchorMissing,
    BudgetExceeded,
    UnavailableHost,
    SpriteLoadFailed,
    HostFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleProjectionDiagnostic {
    pub code: ParticleProjectionDiagnosticCode,
    pub sequence: u32,
    pub handle: Option<ParticleEmitterHandle>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleProjectionReadout {
    pub active_emitters: u32,
    pub reserved_particles: u32,
    pub referenced_sprites: u32,
    pub emitted_bursts: u64,
    pub diagnostics: Vec<ParticleProjectionDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleProjectionLimits {
    pub max_active_emitters: u32,
    pub max_particles_per_emitter: u32,
    pub max_reserved_particles: u32,
}

impl Default for ParticleProjectionLimits {
    fn default() -> Self {
        Self {
            max_active_emitters: 64,
            max_particles_per_emitter: 1_024,
            max_reserved_particles: 4_096,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleProjector {
    limits: ParticleProjectionLimits,
    active: BTreeMap<ParticleEmitterHandle, ParticleEmitterDescriptor>,
    seen_signals: BTreeSet<String>,
    referenced_sprites: BTreeSet<String>,
    emitted_bursts: u64,
    diagnostics: Vec<ParticleProjectionDiagnostic>,
}

impl Default for ParticleProjector {
    fn default() -> Self {
        Self::new(ParticleProjectionLimits::default())
    }
}

impl ParticleProjector {
    pub fn new(limits: ParticleProjectionLimits) -> Self {
        Self {
            limits,
            active: BTreeMap::new(),
            seen_signals: BTreeSet::new(),
            referenced_sprites: BTreeSet::new(),
            emitted_bursts: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn project(
        &mut self,
        assets: &impl PresentationAssetLookup,
        meta: PresentationOpMeta,
        op: ParticleProjectionOp,
    ) -> Result<PresentationOp, ParticleProjectionDiagnostic> {
        let mut projected = self.project_batch(assets, vec![(meta, op)])?;
        Ok(projected.pop().expect("one input produces one operation"))
    }

    pub fn project_batch(
        &mut self,
        assets: &impl PresentationAssetLookup,
        ops: Vec<(PresentationOpMeta, ParticleProjectionOp)>,
    ) -> Result<Vec<PresentationOp>, ParticleProjectionDiagnostic> {
        let mut staged = self.clone();
        let mut projected = Vec::with_capacity(ops.len());
        for (meta, op) in ops {
            if let Err(code) = staged.validate_and_apply(assets, &op) {
                let diagnostic = ParticleProjectionDiagnostic {
                    code,
                    sequence: meta.sequence,
                    handle: operation_handle(&op),
                    message: diagnostic_message(code).to_string(),
                };
                self.diagnostics.push(diagnostic.clone());
                return Err(diagnostic);
            }
            projected.push(PresentationOp::Particle { meta, op });
        }
        *self = staged;
        Ok(projected)
    }

    pub fn descriptor(&self, handle: ParticleEmitterHandle) -> Option<&ParticleEmitterDescriptor> {
        self.active.get(&handle)
    }

    pub fn readout(&self) -> ParticleProjectionReadout {
        ParticleProjectionReadout {
            active_emitters: self.active.len() as u32,
            reserved_particles: self.reserved_particles(),
            referenced_sprites: self.referenced_sprites.len() as u32,
            emitted_bursts: self.emitted_bursts,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn reset(&mut self) {
        let limits = self.limits;
        *self = Self::new(limits);
    }

    fn validate_and_apply(
        &mut self,
        assets: &impl PresentationAssetLookup,
        op: &ParticleProjectionOp,
    ) -> Result<(), ParticleProjectionDiagnosticCode> {
        match op {
            ParticleProjectionOp::Emit {
                signal_id,
                descriptor,
            } => {
                if signal_id.is_empty() || descriptor.burst_count == 0 {
                    return Err(ParticleProjectionDiagnosticCode::InvalidDescriptor);
                }
                self.validate_descriptor(assets, descriptor)?;
                if !self.seen_signals.insert(signal_id.clone()) {
                    return Err(ParticleProjectionDiagnosticCode::DuplicateSignal);
                }
                self.referenced_sprites
                    .insert(descriptor.sprite.asset.clone());
                self.emitted_bursts = self.emitted_bursts.saturating_add(1);
            }
            ParticleProjectionOp::Create { handle, descriptor } => {
                if self.active.contains_key(handle) {
                    return Err(ParticleProjectionDiagnosticCode::DuplicateHandle);
                }
                self.validate_descriptor(assets, descriptor)?;
                if descriptor.rate_per_second <= 0.0
                    || self.active.len() as u32 >= self.limits.max_active_emitters
                    || self
                        .reserved_particles()
                        .saturating_add(descriptor.max_particles)
                        > self.limits.max_reserved_particles
                {
                    return Err(ParticleProjectionDiagnosticCode::BudgetExceeded);
                }
                self.referenced_sprites
                    .insert(descriptor.sprite.asset.clone());
                self.active.insert(*handle, descriptor.clone());
            }
            ParticleProjectionOp::Update { handle, patch } => {
                let current = self
                    .active
                    .get(handle)
                    .cloned()
                    .ok_or(ParticleProjectionDiagnosticCode::UnknownHandle)?;
                let updated = apply_patch(current.clone(), patch);
                self.validate_descriptor(assets, &updated)?;
                if updated.rate_per_second <= 0.0
                    || self
                        .reserved_particles()
                        .saturating_sub(current.max_particles)
                        .saturating_add(updated.max_particles)
                        > self.limits.max_reserved_particles
                {
                    return Err(ParticleProjectionDiagnosticCode::BudgetExceeded);
                }
                self.referenced_sprites.insert(updated.sprite.asset.clone());
                self.active.insert(*handle, updated);
            }
            ParticleProjectionOp::Destroy { handle } => {
                if self.active.remove(handle).is_none() {
                    return Err(ParticleProjectionDiagnosticCode::UnknownHandle);
                }
            }
        }
        Ok(())
    }

    fn validate_descriptor(
        &self,
        assets: &impl PresentationAssetLookup,
        descriptor: &ParticleEmitterDescriptor,
    ) -> Result<(), ParticleProjectionDiagnosticCode> {
        if !anchor_is_finite(&descriptor.anchor)
            || !in_range(descriptor.rate_per_second, 0.0, 10_000.0)
            || descriptor.burst_count > self.limits.max_particles_per_emitter
            || descriptor.max_particles == 0
            || descriptor.max_particles > self.limits.max_particles_per_emitter
            || !ordered_positive_range(descriptor.lifetime_seconds, 0.01, 60.0)
            || !ordered_vec3(descriptor.velocity_min, descriptor.velocity_max)
            || !finite_vec3(descriptor.acceleration)
            || !in_range(descriptor.flipbook_frames_per_second, 0.0, 120.0)
            || descriptor.burst_count > descriptor.max_particles
            || descriptor.seed > JSON_SAFE_U64_MAX
            || !validate_scalar_curve(&descriptor.size_curve)
            || !validate_color_curve(&descriptor.color_curve)
            || descriptor.sprite.content_hash.is_empty()
        {
            return Err(ParticleProjectionDiagnosticCode::InvalidDescriptor);
        }
        if descriptor.sprite.frame_count == 0
            || (descriptor.sprite.frame_count > 1 && descriptor.flipbook_frames_per_second <= 0.0)
        {
            return Err(ParticleProjectionDiagnosticCode::InvalidDescriptor);
        }
        let kind = if descriptor.sprite.frame_count == 1 {
            RenderAssetKind::Sprite
        } else {
            RenderAssetKind::SpriteAtlas
        };
        verify_asset(
            assets,
            &descriptor.sprite.asset,
            kind,
            Some(&descriptor.sprite.content_hash),
        )
        .map_err(asset_diagnostic)
    }

    fn reserved_particles(&self) -> u32 {
        self.active.values().fold(0_u32, |total, descriptor| {
            total.saturating_add(descriptor.max_particles)
        })
    }
}

fn apply_patch(
    mut descriptor: ParticleEmitterDescriptor,
    patch: &ParticleEmitterPatch,
) -> ParticleEmitterDescriptor {
    if let Some(value) = &patch.anchor {
        descriptor.anchor = value.clone();
    }
    if let Some(value) = &patch.sprite {
        descriptor.sprite = value.clone();
    }
    if let Some(value) = patch.rate_per_second {
        descriptor.rate_per_second = value;
    }
    if let Some(value) = patch.burst_count {
        descriptor.burst_count = value;
    }
    if let Some(value) = patch.lifetime_seconds {
        descriptor.lifetime_seconds = value;
    }
    if let Some(value) = patch.velocity_min {
        descriptor.velocity_min = value;
    }
    if let Some(value) = patch.velocity_max {
        descriptor.velocity_max = value;
    }
    if let Some(value) = patch.acceleration {
        descriptor.acceleration = value;
    }
    if let Some(value) = &patch.size_curve {
        descriptor.size_curve.clone_from(value);
    }
    if let Some(value) = &patch.color_curve {
        descriptor.color_curve.clone_from(value);
    }
    if let Some(value) = patch.flipbook_frames_per_second {
        descriptor.flipbook_frames_per_second = value;
    }
    if let Some(value) = patch.max_particles {
        descriptor.max_particles = value;
    }
    if let Some(value) = patch.visible {
        descriptor.visible = value;
    }
    descriptor
}

fn validate_scalar_curve(keys: &[ParticleScalarKey]) -> bool {
    curve_ages(keys.iter().map(|key| key.age))
        && keys
            .iter()
            .all(|key| key.value.is_finite() && key.value >= 0.0)
}

fn validate_color_curve(keys: &[ParticleColorKey]) -> bool {
    curve_ages(keys.iter().map(|key| key.age))
        && keys
            .iter()
            .all(|key| key.color.into_iter().all(|value| in_range(value, 0.0, 1.0)))
}

fn curve_ages(ages: impl Iterator<Item = f32>) -> bool {
    let values = ages.collect::<Vec<_>>();
    values.len() >= 2
        && values.len() <= MAX_CURVE_KEYS
        && values.first() == Some(&0.0)
        && values.last() == Some(&1.0)
        && values
            .windows(2)
            .all(|pair| pair[0].is_finite() && pair[0] < pair[1])
}

fn anchor_is_finite(anchor: &ParticleAnchor) -> bool {
    match anchor {
        ParticleAnchor::World { position }
        | ParticleAnchor::EntityAttached {
            offset: position, ..
        } => finite_vec3(*position),
    }
}

fn finite_vec3(value: [f32; 3]) -> bool {
    value.into_iter().all(f32::is_finite)
}

fn ordered_vec3(minimum: [f32; 3], maximum: [f32; 3]) -> bool {
    finite_vec3(minimum)
        && finite_vec3(maximum)
        && minimum
            .into_iter()
            .zip(maximum)
            .all(|(low, high)| low <= high)
}

fn ordered_positive_range(value: [f32; 2], minimum: f32, maximum: f32) -> bool {
    in_range(value[0], minimum, maximum)
        && in_range(value[1], minimum, maximum)
        && value[0] <= value[1]
}

fn in_range(value: f32, minimum: f32, maximum: f32) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

fn asset_diagnostic(error: PresentationAssetError) -> ParticleProjectionDiagnosticCode {
    match error {
        PresentationAssetError::Missing(_) => ParticleProjectionDiagnosticCode::AssetMissing,
        PresentationAssetError::Invalid(RenderAssetError::ContentHashMismatch { .. }) => {
            ParticleProjectionDiagnosticCode::ContentHashMismatch
        }
        PresentationAssetError::Invalid(_) => ParticleProjectionDiagnosticCode::AssetKindMismatch,
    }
}

fn operation_handle(op: &ParticleProjectionOp) -> Option<ParticleEmitterHandle> {
    match op {
        ParticleProjectionOp::Emit { .. } => None,
        ParticleProjectionOp::Create { handle, .. }
        | ParticleProjectionOp::Update { handle, .. }
        | ParticleProjectionOp::Destroy { handle } => Some(*handle),
    }
}

const fn diagnostic_message(code: ParticleProjectionDiagnosticCode) -> &'static str {
    match code {
        ParticleProjectionDiagnosticCode::InvalidDescriptor => "particle descriptor is invalid",
        ParticleProjectionDiagnosticCode::AssetMissing => "particle sprite is unavailable",
        ParticleProjectionDiagnosticCode::AssetKindMismatch => {
            "particle sprite kind does not match its frame count"
        }
        ParticleProjectionDiagnosticCode::ContentHashMismatch => {
            "particle sprite content hash does not match"
        }
        ParticleProjectionDiagnosticCode::DuplicateSignal => {
            "particle burst signal was already projected"
        }
        ParticleProjectionDiagnosticCode::DuplicateHandle => {
            "particle emitter handle is already active"
        }
        ParticleProjectionDiagnosticCode::UnknownHandle => "particle emitter handle is not active",
        ParticleProjectionDiagnosticCode::AnchorMissing => "particle entity anchor is unavailable",
        ParticleProjectionDiagnosticCode::BudgetExceeded => {
            "particle projection budget is exhausted"
        }
        ParticleProjectionDiagnosticCode::UnavailableHost => "particle host is unavailable",
        ParticleProjectionDiagnosticCode::SpriteLoadFailed => "particle sprite failed to load",
        ParticleProjectionDiagnosticCode::HostFailure => "particle host operation failed",
    }
}
