//! Typed audio presentation values crossing the trusted NativeAOT boundary.
//!
//! Audio clips are immutable Engine-admitted product resources. Callers may
//! release an acquired clip after dependent voices and one-shots are terminal.
//! A product that ends a one-shot early can call the safe `RetireOneShot`
//! operation with the signal returned by `Emit`; the Engine then ends that
//! playback and releases its pending clip owner.

use crate::{
    NativeContentReferenceHandle, NativeSpatialSessionHandle, NativeUtf8Slice, NativeVec3,
};

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
    Sfx = 0,
    Ambient = 1,
    Ui = 2,
    Music = 3,
}

/// The time a bus's sounds follow. `World` sounds hold while gameplay time is
/// held, resume where they held, and play at the gameplay rate; `Host` sounds
/// play on through holds and rate changes. `Sfx` follows `World` and the
/// other buses `Host` until a product sets a bus's clock.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioClock {
    World = 0,
    Host = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioEmitterKind {
    Global2d = 0,
    World3d = 1,
    EntityAttached = 2,
}

/// How a spatial source's level falls between the reference distance
/// (`min(1, max_distance / 2)`, where it plays at full volume) and
/// `max_distance`, at and beyond which it is silent.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioRolloff {
    /// Amplitude falls linearly to zero.
    Linear = 0,
    /// Level falls linearly in decibels, to -60 dB (silence).
    LinearDecibels = 1,
}

/// Whether the world's geometry acts on a spatial source once an acoustic
/// space is set: solid muffles it, open paths carry it round, and the
/// listener's space adds reverb.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeAudioAcoustics {
    #[default]
    World = 0,
    /// Plays as if nothing stood in the way, with no reverb.
    Dry = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAudioVoiceControl {
    Pause = 0,
    Resume = 1,
    Retrigger = 2,
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
    DuplicateSignal = 5,
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
    pub acoustics: NativeAudioAcoustics,
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

/// Sets the clock a bus follows; sounds already playing on it follow the new
/// clock from then on.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioBusClockRequest {
    pub bus: NativeAudioBus,
    pub clock: NativeAudioClock,
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
    pub clock: NativeAudioClock,
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

/// Names the spatial session whose collision the listener and every
/// spatial source are in, so its geometry acts on them. `reverb_level`
/// scales the reverb the listener's space adds, 0 to 1.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioAcousticSpaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub reverb_level: f32,
}

/// Explicit empty requests keep the generated direct API uniformly typed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAudioClearAcousticSpaceRequest {
    pub reserved: u32,
}

/// How a sound made at `source`, audible to `range` with `rolloff`, arrives
/// at each of `points` through `session`'s collision, as playback would play
/// it. At most 64 points.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioArrivalRequest {
    pub session: NativeSpatialSessionHandle,
    pub source: NativeVec3,
    pub range: f32,
    pub rolloff: NativeAudioRolloff,
    pub points: *const NativeVec3,
    pub points_len: usize,
}

/// The way a sound reaches a point.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeAudioArrivalPath {
    /// Beyond its range.
    #[default]
    Silent = 0,
    /// Straight, through nothing.
    Open = 1,
    /// Straight, through solid: quieter and low-passed.
    Occluded = 2,
    /// By an open path round the solid.
    Around = 3,
}

/// How a sound arrives at one point. `gain` is the amplitude playback would
/// play it at, before the source's own volume and bus; `low_pass_hz` is
/// 20000 when unfiltered; `arrives_from` is a point about three metres back
/// along the path it takes, or the source when it comes straight.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAudioArrival {
    pub path: NativeAudioArrivalPath,
    pub gain: f32,
    pub low_pass_hz: f32,
    pub path_metres: f32,
    pub arrives_from: NativeVec3,
}

/// Borrowed query result: `arrivals` matches the request's points in order
/// and stays valid until the next call on the same context. The open-air
/// search settled `cells_searched` cells; `rays_cast` counts straight rays
/// and search segments; `truncated` says the search stopped at its cap.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioArrivalResult {
    pub arrivals: *const NativeAudioArrival,
    pub arrivals_len: usize,
    pub cells_searched: u32,
    pub rays_cast: u32,
    pub truncated: bool,
}
