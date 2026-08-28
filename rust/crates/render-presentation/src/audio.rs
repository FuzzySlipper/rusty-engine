use std::collections::{BTreeMap, BTreeSet};

use render_model::{RenderAssetError, RenderAssetKind};
use serde::{Deserialize, Serialize};

use crate::{
    verify_asset, PresentationAssetError, PresentationAssetLookup, PresentationOp,
    PresentationOpMeta,
};

/// Maximum number of projector diagnostics retained for indexed inspection.
/// Diagnostics are retained in oldest-to-newest order; entries beyond this
/// bound evict the oldest entry.
pub const MAX_AUDIO_DIAGNOSTICS: usize = 128;

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

/// Engine-issued correlation for one realized one-shot audio signal.
///
/// This is intentionally separate from the string idempotency key: renderer
/// feedback names a concrete realization, while replay/admission can continue
/// to deduplicate by `signal_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AudioSignalHandle(u64);

impl AudioSignalHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

/// The desired presentation state for a retained voice.
///
/// This is deliberately not a host cursor or completion signal. The audio
/// owner knows whether the product currently wants the retained voice playing
/// or paused; host realization owns any eventual playback position feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioVoiceDesiredState {
    Playing,
    Paused,
}

/// A lifecycle control for a retained audio voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioVoiceControl {
    Pause,
    Resume,
    Retrigger,
}

/// A fixed-bus control. Audio buses are a closed Engine enum; this is not a
/// product-defined group registry.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AudioBusControl {
    SetVolume { volume: f32 },
    SetMuted { muted: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioVoiceReadout {
    pub handle: AudioHandle,
    pub descriptor: AudioSourceDescriptor,
    pub desired_state: AudioVoiceDesiredState,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioBusReadout {
    pub bus: AudioBus,
    pub volume: f32,
    pub muted: bool,
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
        signal_handle: AudioSignalHandle,
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
    VoiceControl {
        handle: AudioHandle,
        control: AudioVoiceControl,
    },
    BusControl {
        bus: AudioBus,
        control: AudioBusControl,
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
    InvalidControl,
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
    pub paused_sources: u32,
    pub referenced_clips: u32,
    pub emitted_signals: u64,
    pub retained_diagnostic_count: u32,
    pub evicted_diagnostic_count: u64,
    pub diagnostics: Vec<AudioProjectionDiagnostic>,
}

#[derive(Debug, Clone, Default)]
pub struct AudioProjector {
    active: BTreeMap<AudioHandle, RetainedAudioVoice>,
    buses: BTreeMap<AudioBus, AudioBusState>,
    seen_signals: BTreeSet<String>,
    referenced_clips: BTreeSet<String>,
    emitted_signals: u64,
    diagnostics: Vec<AudioProjectionDiagnostic>,
    evicted_diagnostic_count: u64,
}

#[derive(Debug, Clone)]
struct RetainedAudioVoice {
    descriptor: AudioSourceDescriptor,
    desired_state: AudioVoiceDesiredState,
}

#[derive(Debug, Clone, Copy)]
struct AudioBusState {
    volume: f32,
    muted: bool,
}

impl Default for AudioBusState {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
        }
    }
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
                self.retain_diagnostic(diagnostic.clone());
                return Err(diagnostic);
            }
            projected.push(PresentationOp::Audio { meta, op });
        }
        *self = staged;
        Ok(projected)
    }

    pub fn descriptor(&self, handle: AudioHandle) -> Option<&AudioSourceDescriptor> {
        self.active.get(&handle).map(|voice| &voice.descriptor)
    }

    pub fn voice(&self, handle: AudioHandle) -> Option<AudioVoiceReadout> {
        self.active.get(&handle).map(|voice| AudioVoiceReadout {
            handle,
            descriptor: voice.descriptor.clone(),
            desired_state: voice.desired_state,
        })
    }

    pub fn bus(&self, bus: AudioBus) -> AudioBusReadout {
        let state = self.buses.get(&bus).copied().unwrap_or_default();
        AudioBusReadout {
            bus,
            volume: state.volume,
            muted: state.muted,
        }
    }

    pub fn readout(&self) -> AudioProjectionReadout {
        AudioProjectionReadout {
            active_sources: self.active.len() as u32,
            paused_sources: self
                .active
                .values()
                .filter(|voice| voice.desired_state == AudioVoiceDesiredState::Paused)
                .count() as u32,
            referenced_clips: self.referenced_clips.len() as u32,
            emitted_signals: self.emitted_signals,
            retained_diagnostic_count: self.diagnostics.len() as u32,
            evicted_diagnostic_count: self.evicted_diagnostic_count,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn retain_diagnostic(&mut self, diagnostic: AudioProjectionDiagnostic) {
        if self.diagnostics.len() == MAX_AUDIO_DIAGNOSTICS {
            self.diagnostics.remove(0);
            self.evicted_diagnostic_count = self.evicted_diagnostic_count.saturating_add(1);
        }
        self.diagnostics.push(diagnostic);
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
                ..
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
                self.active.insert(
                    *handle,
                    RetainedAudioVoice {
                        descriptor: descriptor.clone(),
                        // Creation is an explicit play from offset zero. The
                        // offset is a wire/host realization rule, not a host
                        // cursor this owner can truthfully report.
                        desired_state: AudioVoiceDesiredState::Playing,
                    },
                );
            }
            AudioProjectionOp::Update { handle, patch } => {
                let current = self
                    .active
                    .get(handle)
                    .cloned()
                    .ok_or(AudioProjectionDiagnosticCode::UnknownHandle)?;
                let updated = apply_patch(current.descriptor, patch);
                validate_descriptor(assets, &updated)?;
                self.referenced_clips.insert(updated.clip.asset.clone());
                self.active.insert(
                    *handle,
                    RetainedAudioVoice {
                        descriptor: updated,
                        desired_state: current.desired_state,
                    },
                );
            }
            AudioProjectionOp::Destroy { handle } => {
                if self.active.remove(handle).is_none() {
                    return Err(AudioProjectionDiagnosticCode::UnknownHandle);
                }
            }
            AudioProjectionOp::VoiceControl { handle, control } => {
                let voice = self
                    .active
                    .get_mut(handle)
                    .ok_or(AudioProjectionDiagnosticCode::UnknownHandle)?;
                voice.desired_state = match control {
                    AudioVoiceControl::Pause => AudioVoiceDesiredState::Paused,
                    AudioVoiceControl::Resume | AudioVoiceControl::Retrigger => {
                        AudioVoiceDesiredState::Playing
                    }
                };
            }
            AudioProjectionOp::BusControl { bus, control } => {
                let state = self.buses.entry(*bus).or_default();
                match control {
                    AudioBusControl::SetVolume { volume } => {
                        if !in_range(*volume, 0.0, 1.0) {
                            return Err(AudioProjectionDiagnosticCode::InvalidControl);
                        }
                        state.volume = *volume;
                    }
                    AudioBusControl::SetMuted { muted } => state.muted = *muted,
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
        | AudioProjectionOp::Destroy { handle }
        | AudioProjectionOp::VoiceControl { handle, .. } => Some(*handle),
        AudioProjectionOp::BusControl { .. } => None,
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
        AudioProjectionDiagnosticCode::InvalidControl => "audio control is invalid",
        AudioProjectionDiagnosticCode::UnavailableHost => "audio host is unavailable",
        AudioProjectionDiagnosticCode::AudioContextBlocked => "audio context start was blocked",
        AudioProjectionDiagnosticCode::DecodeFailed => "audio clip decoding failed",
        AudioProjectionDiagnosticCode::HostFailure => "audio host operation failed",
    }
}
