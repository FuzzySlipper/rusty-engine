use std::collections::{BTreeMap, BTreeSet};

use render_model::{RenderAssetError, RenderAssetKind};
use serde::{Deserialize, Serialize};

use crate::{
    verify_asset, PresentationAssetError, PresentationAssetLookup, PresentationOp,
    PresentationOpMeta,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AudioHandle(u64);

impl AudioHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioBus {
    Sfx,
    Ambient,
    Ui,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AudioEmitter {
    Global2d,
    World3d { position: [f32; 3] },
    EntityAttached { entity: u64, offset: [f32; 3] },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioClipRef {
    pub asset: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioSourceDescriptor {
    pub clip: AudioClipRef,
    pub bus: AudioBus,
    pub volume: f32,
    pub pitch: f32,
    pub looping: bool,
    pub spatial_blend: f32,
    pub attenuation: f32,
    pub pan: f32,
    pub emitter: AudioEmitter,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioSourcePatch {
    pub volume: Option<f32>,
    pub pitch: Option<f32>,
    pub looping: Option<bool>,
    pub spatial_blend: Option<f32>,
    pub attenuation: Option<f32>,
    pub pan: Option<f32>,
    pub emitter: Option<AudioEmitter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AudioProjectionOp {
    Emit {
        signal_id: String,
        descriptor: AudioSourceDescriptor,
    },
    Create {
        handle: AudioHandle,
        descriptor: AudioSourceDescriptor,
    },
    Update {
        handle: AudioHandle,
        patch: AudioSourcePatch,
    },
    Destroy {
        handle: AudioHandle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioProjectionDiagnosticCode {
    InvalidDescriptor,
    AssetMissing,
    AssetKindMismatch,
    ContentHashMismatch,
    DuplicateSignal,
    DuplicateHandle,
    UnknownHandle,
    UnavailableHost,
    AudioContextBlocked,
    DecodeFailed,
    HostFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioProjectionDiagnostic {
    pub code: AudioProjectionDiagnosticCode,
    pub sequence: u32,
    pub handle: Option<AudioHandle>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioProjectionReadout {
    pub active_sources: u32,
    pub referenced_clips: u32,
    pub emitted_signals: u64,
    pub diagnostics: Vec<AudioProjectionDiagnostic>,
}

#[derive(Debug, Clone, Default)]
pub struct AudioProjector {
    active: BTreeMap<AudioHandle, AudioSourceDescriptor>,
    seen_signals: BTreeSet<String>,
    referenced_clips: BTreeSet<String>,
    emitted_signals: u64,
    diagnostics: Vec<AudioProjectionDiagnostic>,
}

impl AudioProjector {
    pub fn project(
        &mut self,
        assets: &impl PresentationAssetLookup,
        meta: PresentationOpMeta,
        op: AudioProjectionOp,
    ) -> Result<PresentationOp, AudioProjectionDiagnostic> {
        let mut projected = self.project_batch(assets, vec![(meta, op)])?;
        Ok(projected.pop().expect("one input produces one operation"))
    }

    /// Applies a domain batch atomically. A rejected later operation leaves all
    /// retained sources, signal ids, and counters at their pre-batch values.
    pub fn project_batch(
        &mut self,
        assets: &impl PresentationAssetLookup,
        ops: Vec<(PresentationOpMeta, AudioProjectionOp)>,
    ) -> Result<Vec<PresentationOp>, AudioProjectionDiagnostic> {
        let mut staged = self.clone();
        let mut projected = Vec::with_capacity(ops.len());
        for (meta, op) in ops {
            if let Err(code) = staged.validate_and_apply(assets, &op) {
                let diagnostic = AudioProjectionDiagnostic {
                    code,
                    sequence: meta.sequence,
                    handle: operation_handle(&op),
                    message: diagnostic_message(code).to_string(),
                };
                self.diagnostics.push(diagnostic.clone());
                return Err(diagnostic);
            }
            projected.push(PresentationOp::Audio { meta, op });
        }
        *self = staged;
        Ok(projected)
    }

    pub fn descriptor(&self, handle: AudioHandle) -> Option<&AudioSourceDescriptor> {
        self.active.get(&handle)
    }

    pub fn readout(&self) -> AudioProjectionReadout {
        AudioProjectionReadout {
            active_sources: self.active.len() as u32,
            referenced_clips: self.referenced_clips.len() as u32,
            emitted_signals: self.emitted_signals,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn validate_and_apply(
        &mut self,
        assets: &impl PresentationAssetLookup,
        op: &AudioProjectionOp,
    ) -> Result<(), AudioProjectionDiagnosticCode> {
        match op {
            AudioProjectionOp::Emit {
                signal_id,
                descriptor,
            } => {
                if signal_id.is_empty() {
                    return Err(AudioProjectionDiagnosticCode::InvalidDescriptor);
                }
                validate_descriptor(assets, descriptor)?;
                if !self.seen_signals.insert(signal_id.clone()) {
                    return Err(AudioProjectionDiagnosticCode::DuplicateSignal);
                }
                self.referenced_clips.insert(descriptor.clip.asset.clone());
                self.emitted_signals = self.emitted_signals.saturating_add(1);
            }
            AudioProjectionOp::Create { handle, descriptor } => {
                if self.active.contains_key(handle) {
                    return Err(AudioProjectionDiagnosticCode::DuplicateHandle);
                }
                validate_descriptor(assets, descriptor)?;
                self.referenced_clips.insert(descriptor.clip.asset.clone());
                self.active.insert(*handle, descriptor.clone());
            }
            AudioProjectionOp::Update { handle, patch } => {
                let current = self
                    .active
                    .get(handle)
                    .cloned()
                    .ok_or(AudioProjectionDiagnosticCode::UnknownHandle)?;
                let updated = apply_patch(current, patch);
                validate_descriptor(assets, &updated)?;
                self.referenced_clips.insert(updated.clip.asset.clone());
                self.active.insert(*handle, updated);
            }
            AudioProjectionOp::Destroy { handle } => {
                if self.active.remove(handle).is_none() {
                    return Err(AudioProjectionDiagnosticCode::UnknownHandle);
                }
            }
        }
        Ok(())
    }
}

fn validate_descriptor(
    assets: &impl PresentationAssetLookup,
    descriptor: &AudioSourceDescriptor,
) -> Result<(), AudioProjectionDiagnosticCode> {
    if !in_range(descriptor.volume, 0.0, 1.0)
        || !in_range(descriptor.pitch, 0.25, 4.0)
        || !in_range(descriptor.spatial_blend, 0.0, 1.0)
        || !in_range(descriptor.pan, -1.0, 1.0)
        || !descriptor.attenuation.is_finite()
        || descriptor.attenuation <= 0.0
        || !emitter_is_finite(&descriptor.emitter)
        || descriptor.clip.content_hash.is_empty()
    {
        return Err(AudioProjectionDiagnosticCode::InvalidDescriptor);
    }
    verify_asset(
        assets,
        &descriptor.clip.asset,
        RenderAssetKind::Audio,
        Some(&descriptor.clip.content_hash),
    )
    .map_err(asset_diagnostic)
}

fn apply_patch(
    mut descriptor: AudioSourceDescriptor,
    patch: &AudioSourcePatch,
) -> AudioSourceDescriptor {
    if let Some(value) = patch.volume {
        descriptor.volume = value;
    }
    if let Some(value) = patch.pitch {
        descriptor.pitch = value;
    }
    if let Some(value) = patch.looping {
        descriptor.looping = value;
    }
    if let Some(value) = patch.spatial_blend {
        descriptor.spatial_blend = value;
    }
    if let Some(value) = patch.attenuation {
        descriptor.attenuation = value;
    }
    if let Some(value) = patch.pan {
        descriptor.pan = value;
    }
    if let Some(value) = &patch.emitter {
        descriptor.emitter = value.clone();
    }
    descriptor
}

fn asset_diagnostic(error: PresentationAssetError) -> AudioProjectionDiagnosticCode {
    match error {
        PresentationAssetError::Missing(_) => AudioProjectionDiagnosticCode::AssetMissing,
        PresentationAssetError::Invalid(RenderAssetError::ContentHashMismatch { .. }) => {
            AudioProjectionDiagnosticCode::ContentHashMismatch
        }
        PresentationAssetError::Invalid(_) => AudioProjectionDiagnosticCode::AssetKindMismatch,
    }
}

fn in_range(value: f32, minimum: f32, maximum: f32) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

fn emitter_is_finite(emitter: &AudioEmitter) -> bool {
    match emitter {
        AudioEmitter::Global2d => true,
        AudioEmitter::World3d { position }
        | AudioEmitter::EntityAttached {
            offset: position, ..
        } => position.iter().all(|value| value.is_finite()),
    }
}

fn operation_handle(op: &AudioProjectionOp) -> Option<AudioHandle> {
    match op {
        AudioProjectionOp::Emit { .. } => None,
        AudioProjectionOp::Create { handle, .. }
        | AudioProjectionOp::Update { handle, .. }
        | AudioProjectionOp::Destroy { handle } => Some(*handle),
    }
}

const fn diagnostic_message(code: AudioProjectionDiagnosticCode) -> &'static str {
    match code {
        AudioProjectionDiagnosticCode::InvalidDescriptor => "audio descriptor is invalid",
        AudioProjectionDiagnosticCode::AssetMissing => "audio clip is unavailable",
        AudioProjectionDiagnosticCode::AssetKindMismatch => {
            "audio clip reference has the wrong resource kind"
        }
        AudioProjectionDiagnosticCode::ContentHashMismatch => {
            "audio clip content hash does not match"
        }
        AudioProjectionDiagnosticCode::DuplicateSignal => {
            "audio one-shot signal id was already projected"
        }
        AudioProjectionDiagnosticCode::DuplicateHandle => "audio handle is already active",
        AudioProjectionDiagnosticCode::UnknownHandle => "audio handle is not active",
        AudioProjectionDiagnosticCode::UnavailableHost => "audio host is unavailable",
        AudioProjectionDiagnosticCode::AudioContextBlocked => "audio context start was blocked",
        AudioProjectionDiagnosticCode::DecodeFailed => "audio clip decoding failed",
        AudioProjectionDiagnosticCode::HostFailure => "audio host operation failed",
    }
}
