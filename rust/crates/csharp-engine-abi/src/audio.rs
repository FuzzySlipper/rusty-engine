//! Typed audio presentation values crossing the trusted NativeAOT boundary.
//!
//! Audio clips are immutable Engine-admitted product resources and remain
//! available for the whole product runtime. Retained voices are the only
//! disposable audio owners.

use crate::{NativeUtf8Slice, NativeVec3};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAudioClipHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAudioVoiceHandle {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioBus {
    Sfx = 1,
    Ambient = 2,
    Ui = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioEmitterKind {
    Global2d = 1,
    World3d = 2,
    EntityAttached = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioVoiceControl {
    Pause = 1,
    Resume = 2,
    Retrigger = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioVoiceDesiredState {
    Playing = 1,
    Paused = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioDiagnosticCode {
    None = 0,
    InvalidDescriptor = 1,
    AssetMissing = 2,
    AssetKindMismatch = 3,
    ContentHashMismatch = 4,
    DuplicateSignal = 5,
    DuplicateHandle = 6,
    UnknownHandle = 7,
    UnavailableHost = 8,
    AudioContextBlocked = 9,
    DecodeFailed = 10,
    HostFailure = 11,
    InvalidControl = 12,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioClipRequest {
    /// A normalized product-content WAV path. The selected bytes are copied
    /// into Engine-owned preload storage before the direct call returns.
    pub path: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioSourceDescriptor {
    pub clip: NativeAudioClipHandle,
    pub bus: NativeAudioBus,
    pub volume: f32,
    pub pitch: f32,
    pub looping: bool,
    pub spatial_blend: f32,
    pub attenuation: f32,
    pub pan: f32,
    pub emitter_kind: NativeAudioEmitterKind,
    pub position: NativeVec3,
    pub entity: u64,
    pub offset: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioEmitRequest {
    /// Product-owned idempotency identity. Repeating it is rejected by the
    /// Engine projector rather than silently replaying the one-shot.
    pub signal_id: NativeUtf8Slice,
    pub descriptor: NativeAudioSourceDescriptor,
}

/// Replaces every mutable source parameter but retains the selected immutable
/// clip. Use `ReplaceVoice` when the clip itself must change.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioVoiceUpdateRequest {
    pub voice: NativeAudioVoiceHandle,
    pub descriptor: NativeAudioSourceDescriptor,
}

/// Atomically stops the old retained voice and starts a replacement with the
/// same logical product operation. The old owner becomes a harmless tombstone.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioVoiceReplaceRequest {
    pub voice: NativeAudioVoiceHandle,
    pub descriptor: NativeAudioSourceDescriptor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioVoiceControlRequest {
    pub voice: NativeAudioVoiceHandle,
    pub control: NativeAudioVoiceControl,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioBusVolumeRequest {
    pub bus: NativeAudioBus,
    pub volume: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioBusMutedRequest {
    pub bus: NativeAudioBus,
    pub muted: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAudioReadout {
    /// Engine projector state, not browser-realized playback state.
    pub active_voices: u32,
    /// Retained voices whose Engine-owned desired state is paused. This is not
    /// a host cursor or completion signal.
    pub paused_voices: u32,
    pub admitted_clips: u32,
    pub emitted_signals: u64,
    /// Number of diagnostics currently retained for indexed readout.
    pub retained_diagnostic_count: u32,
    /// Cumulative number of diagnostics evicted from the retained readout.
    pub evicted_diagnostic_count: u64,
}

/// Point readout for a retained voice. It deliberately omits descriptor and
/// browser-realization state; product code already owns the descriptor it
/// published, while the Engine owner only exposes desired playback state here.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeAudioVoiceReadout {
    pub present: bool,
    pub desired_state: NativeAudioVoiceDesiredState,
}

impl Default for NativeAudioVoiceReadout {
    fn default() -> Self {
        Self {
            present: false,
            desired_state: NativeAudioVoiceDesiredState::Playing,
        }
    }
}

/// Fixed Engine-bus state. This is not browser realization feedback.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeAudioBusReadout {
    pub volume: f32,
    pub muted: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioVoiceReadRequest {
    pub voice: NativeAudioVoiceHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioBusReadRequest {
    pub bus: NativeAudioBus,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioDiagnosticAtRequest {
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioDiagnosticAtReceipt {
    pub present: bool,
    pub code: NativeAudioDiagnosticCode,
    pub sequence: u32,
    /// Retained voice identity when the diagnostic is voice-scoped; zero for
    /// one-shots and resource-level diagnostics. This is observational and
    /// never an owning handle.
    pub voice_value: u64,
}

impl Default for NativeAudioDiagnosticAtReceipt {
    fn default() -> Self {
        Self {
            present: false,
            code: NativeAudioDiagnosticCode::None,
            sequence: 0,
            voice_value: 0,
        }
    }
}
