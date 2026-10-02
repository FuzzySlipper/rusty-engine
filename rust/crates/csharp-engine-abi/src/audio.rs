//! Typed audio presentation values crossing the trusted NativeAOT boundary.
//!
//! Audio clips are immutable Engine-admitted product resources. Callers may
//! release an acquired clip after dependent voices and one-shots are terminal.

use crate::{NativeContentReferenceHandle, NativeUtf8Slice, NativeVec3};

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

/// Engine-issued correlation for one realized one-shot signal.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAudioSignalHandle {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioBus {
    Sfx = 1,
    Ambient = 2,
    Ui = 3,
    Music = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioEmitterKind {
    Global2d = 1,
    World3d = 2,
    EntityAttached = 3,
}

/// How a spatial source's level falls between the reference distance
/// (`min(1, max_distance / 2)`, where it plays at full volume) and
/// `max_distance`, at and beyond which it is silent.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioRolloff {
    /// Amplitude falls linearly to zero.
    Linear = 1,
    /// Level falls linearly in decibels, to -60 dB (silence).
    LinearDecibels = 2,
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

/// The concrete audio output realization fact kind, distinct from the audio
/// projector's admission/readout state.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioRealizationFactKind {
    None = 0,
    NaturalCompletionOneShot = 1,
    NaturalCompletionRetainedVoice = 2,
    Diagnostic = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioClipRequest {
    /// A normalized product-content WAV path. The selected bytes are copied
    /// into Engine-owned preload storage before the direct call returns.
    pub path: NativeUtf8Slice,
}

/// Opens one immutable WAV clip from an already-retained ProductContent
/// reference. The Engine copies and validates the selected bytes before the
/// direct call returns, so the reference and its originating bundle can be
/// released independently afterwards.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioClipFromContentRequest {
    pub content: NativeContentReferenceHandle,
}

/// The outcome of attempting to retain an explicitly optional audio preload.
/// Required clips continue to use `OpenClip`, whose admission errors remain
/// strict. A skipped optional preload has no owning clip handle.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioOptionalPreloadOutcome {
    Admitted = 1,
    SkippedMissing = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioOptionalPreloadReceipt {
    pub outcome: NativeAudioOptionalPreloadOutcome,
    pub clip: NativeAudioClipHandle,
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
    /// The audible range of a spatial source, in world units: silent at and
    /// beyond it. Positive and finite.
    pub max_distance: f32,
    pub rolloff: NativeAudioRolloff,
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
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioResult {
    /// Retained diagnostics; borrowed like [`NativeAudioRealizationResult::facts`].
    pub diagnostics: *const NativeAudioDiagnostic,
    pub diagnostics_len: usize,
    /// Engine projector state, not browser-realized playback state.
    pub active_voices: u32,
    /// Retained voices whose Engine-owned desired state is paused. This is not
    /// a host cursor or completion signal.
    pub paused_voices: u32,
    pub admitted_clips: u32,
    pub emitted_signals: u64,
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

/// Fixed Engine-bus state. This is not realization feedback.
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
pub struct NativeAudioDiagnostic {
    pub code: NativeAudioDiagnosticCode,
    pub sequence: u32,
    /// Retained voice identity when the diagnostic is voice-scoped; zero for
    /// one-shots and resource-level diagnostics. This is observational and
    /// never an owning handle.
    pub voice_value: u64,
}

/// Borrowed realization feedback: every retained fact, committed between
/// product calls (not the Audio projector readout). `facts` points into Audio
/// bridge storage and stays valid until the next call on the same context;
/// the generated managed binding copies it before returning.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioRealizationResult {
    pub facts: *const NativeAudioRealizationFact,
    pub facts_len: usize,
    pub evicted_fact_count: u64,
}

/// One realization fact. `signal_handle` is set only for a one-shot
/// completion, `voice_value` only for retained voice/voice-scoped diagnostic
/// facts, and `code` only for diagnostics.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeAudioRealizationFact {
    pub kind: NativeAudioRealizationFactKind,
    pub fact_id: u64,
    pub sequence: u32,
    pub signal_handle: u64,
    pub voice_value: u64,
    pub code: NativeAudioDiagnosticCode,
}

impl Default for NativeAudioRealizationFact {
    fn default() -> Self {
        Self {
            kind: NativeAudioRealizationFactKind::None,
            fact_id: 0,
            sequence: 0,
            signal_handle: 0,
            voice_value: 0,
            code: NativeAudioDiagnosticCode::None,
        }
    }
}

impl Default for NativeAudioDiagnostic {
    fn default() -> Self {
        Self {
            code: NativeAudioDiagnosticCode::None,
            sequence: 0,
            voice_value: 0,
        }
    }
}
