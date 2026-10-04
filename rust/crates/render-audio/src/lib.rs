//! Device realization of the Engine's committed audio projection.
//!
//! `AudioProjector` (render-presentation) admits and retains audio intent.
//! [`AudioRealizer`] turns the committed `PresentationOp::Audio` stream into
//! sound on one output device through kira. It owns no Engine clock: retained
//! cursors reach it only through `Restore` ops in a baseline, and the device
//! plays in real time between baselines. Realized positions never flow back
//! into Engine state; only natural completions and diagnostics are reported.
//!
//! Only this crate depends on kira and cpal.

mod opus;
mod rolloff;
mod soundtrack;
mod stream;

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::sync::Arc;

use kira::backend::Backend;
use kira::listener::ListenerHandle;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle, StreamingSoundSettings};
use kira::sound::{FromFileError, PlaybackState, Region};
use kira::track::{SpatialTrackBuilder, SpatialTrackHandle, TrackBuilder, TrackHandle};
use kira::{
    AudioManager, AudioManagerSettings, Capacities, Decibels, Easing, Panning, PlaybackRate,
    StartTime, Tween,
};
use render_model::AudioContainer;
use render_presentation::{
    AudioBus, AudioBusControl, AudioClipRef, AudioEmitter, AudioHandle, AudioProjectionDiagnostic,
    AudioProjectionDiagnosticCode, AudioProjectionOp, AudioSignalHandle, AudioSourceDescriptor,
    AudioSourcePatch, AudioVoiceControl, AudioVoiceDesiredState, PresentationOp,
};

pub use kira::backend::mock::{MockBackend, MockBackendSettings};
pub use kira::DefaultBackend;
pub use stream::{AudioBlockSink, StreamBackend, StreamBackendSettings};

/// kira preallocates its realtime arenas. Each spatial voice or one-shot
/// takes one sub-track; each bus plays every non-spatial sound directly.
const SUB_TRACK_CAPACITY: usize = 1024;
const BUS_SOUND_CAPACITY: usize = 1024;
/// Changes apply at once; any fade is the product's decision.
const IMMEDIATE: Tween = Tween {
    start_time: StartTime::Immediate,
    duration: std::time::Duration::ZERO,
    easing: Easing::Linear,
};
const BUSES: [AudioBus; 4] = [
    AudioBus::Sfx,
    AudioBus::Ambient,
    AudioBus::Ui,
    AudioBus::Music,
];

/// Encoded clip bytes by content hash. The Engine owns the bytes; the
/// realizer shares them while it decodes or streams the clip.
pub trait AudioClipSource {
    fn clip_bytes(&self, content_hash: &str) -> Option<Arc<[u8]>>;
}

impl<F> AudioClipSource for F
where
    F: Fn(&str) -> Option<Arc<[u8]>>,
{
    fn clip_bytes(&self, content_hash: &str) -> Option<Arc<[u8]>> {
        self(content_hash)
    }
}

/// World position of an entity for entity-attached emitters.
pub trait AudioEntityPositions {
    fn entity_position(&self, entity: u64) -> Option<[f32; 3]>;
}

/// No entity positions. Entity-attached voices then fail with a host
/// diagnostic.
pub struct NoEntityPositions;

impl AudioEntityPositions for NoEntityPositions {
    fn entity_position(&self, _entity: u64) -> Option<[f32; 3]> {
        None
    }
}

/// What the device did, in the order it happened. These map one to one onto
/// the Engine's existing audio realization facts.
#[derive(Debug, Clone, PartialEq)]
pub enum RealizedAudioFact {
    OneShotCompleted {
        sequence: u32,
        signal_handle: AudioSignalHandle,
    },
    VoiceCompleted {
        sequence: u32,
        handle: AudioHandle,
    },
    Diagnostic {
        diagnostic: AudioProjectionDiagnostic,
        signal_handle: Option<AudioSignalHandle>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealizedVoiceState {
    Playing,
    Paused,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioRealizationReadout {
    pub voices: usize,
    pub playing_voices: usize,
    pub one_shots: usize,
    pub decoded_clips: usize,
}

/// Clip data shared by every playback of one clip. WAV decodes once, as the
/// browser buffer path does; compressed containers stay encoded and each
/// playback streams its own decoder, as the browser media-element path does.
#[derive(Clone)]
enum ClipData {
    Static(Box<StaticSoundData>),
    Encoded { bytes: Arc<[u8]>, opus: bool },
}

/// A fresh streaming decoder over encoded clip bytes. Opus uses its own
/// decoder; symphonia decodes Vorbis, MP3 and FLAC.
fn streaming_data(
    bytes: Arc<[u8]>,
    opus: bool,
) -> Result<StreamingSoundData<FromFileError>, FromFileError> {
    if opus {
        Ok(StreamingSoundData::from_decoder(opus::OggOpusDecoder::new(
            bytes,
        )?))
    } else {
        StreamingSoundData::from_cursor(Cursor::new(bytes))
    }
}

enum Sound {
    Static(StaticSoundHandle),
    Streaming(StreamingSoundHandle<FromFileError>),
}

macro_rules! each_sound {
    ($sound:expr, $handle:ident => $body:expr) => {
        match $sound {
            Sound::Static($handle) => $body,
            Sound::Streaming($handle) => $body,
        }
    };
}

/// One realized playback. A spatial emitter plays into its own spatial
/// sub-track, which persists until its sound finishes.
struct Playback {
    sound: Sound,
    spatial: Option<SpatialTrackHandle>,
    duration: Option<f64>,
    /// The descriptor's own rate, before the world rate scales it.
    pitch: f64,
    /// Where world time moved the sound while the device was suspended. kira
    /// applies the seek when its track resumes; until then this is the cursor.
    held_at: Option<f64>,
    /// World time carried the sound past its end while suspended.
    ended: bool,
}

impl Playback {
    fn finished(&self) -> bool {
        self.ended || each_sound!(&self.sound, handle => handle.state()) == PlaybackState::Stopped
    }

    fn position(&self) -> f64 {
        self.held_at
            .unwrap_or_else(|| each_sound!(&self.sound, handle => handle.position()))
    }

    fn cursor(&self, looping: bool) -> f64 {
        normalize_cursor(self.position(), self.duration, looping)
    }

    fn pause(&mut self) {
        each_sound!(&mut self.sound, handle => handle.pause(IMMEDIATE));
    }

    fn resume(&mut self) {
        each_sound!(&mut self.sound, handle => handle.resume(IMMEDIATE));
    }

    fn stop(&mut self) {
        each_sound!(&mut self.sound, handle => handle.stop(IMMEDIATE));
    }

    fn take_error(&mut self) -> Option<String> {
        match &mut self.sound {
            Sound::Static(_) => None,
            Sound::Streaming(handle) => handle.pop_error().map(|error| error.to_string()),
        }
    }

    /// Moves the suspended sound on by `world_seconds` of world time at its
    /// own rate. Past the end of a non-looping sound it ends there.
    fn advance(&mut self, world_seconds: f64, looping: bool) {
        if self.ended {
            return;
        }
        let target = self.position() + world_seconds * self.pitch;
        if let Some(duration) = self.duration.filter(|duration| *duration > 0.0) {
            if !looping && target >= duration {
                self.stop();
                self.ended = true;
                return;
            }
        }
        let target = normalize_cursor(target, self.duration, looping);
        each_sound!(&mut self.sound, handle => handle.seek_to(target));
        self.held_at = Some(target);
    }

    /// Plays at the descriptor's rate scaled by the world's.
    fn follow_world(&mut self, world_rate: f64) {
        let rate = PlaybackRate(self.pitch * world_rate);
        each_sound!(&mut self.sound, handle => handle.set_playback_rate(rate, IMMEDIATE));
    }

    /// Applies the descriptor fields a playing sound can change in place.
    fn apply(&mut self, descriptor: &AudioSourceDescriptor, world_rate: f64) {
        self.pitch = f64::from(descriptor.pitch);
        let volume = amplitude_to_decibels(descriptor.volume);
        let rate = PlaybackRate(self.pitch * world_rate);
        let pan = Panning(descriptor.pan);
        let region = loop_region(descriptor.looping);
        each_sound!(&mut self.sound, handle => {
            handle.set_volume(volume, IMMEDIATE);
            handle.set_playback_rate(rate, IMMEDIATE);
            handle.set_panning(pan, IMMEDIATE);
            handle.set_loop_region(region);
        });
        if let Some(track) = self.spatial.as_mut() {
            track.set_spatialization_strength(descriptor.spatial_blend, IMMEDIATE);
        }
    }
}

struct Voice {
    descriptor: AudioSourceDescriptor,
    sequence: u32,
    state: RealizedVoiceState,
    /// Cursor while no playback exists (paused from a baseline, or complete).
    cursor: f64,
    playback: Option<Playback>,
}

struct OneShot {
    sequence: u32,
    signal_handle: AudioSignalHandle,
    playback: Playback,
}

#[derive(Clone, Copy)]
struct BusState {
    volume: f32,
    muted: bool,
}

impl BusState {
    const DEFAULT: Self = Self {
        volume: 1.0,
        muted: false,
    };

    fn gain(self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }
}

type OpError = (AudioProjectionDiagnosticCode, String);

/// Plays the committed audio projection on one output device.
pub struct AudioRealizer<B: Backend = DefaultBackend> {
    manager: AudioManager<B>,
    listener: ListenerHandle,
    buses: [TrackHandle; BUSES.len()],
    bus_states: [BusState; BUSES.len()],
    clips: HashMap<String, ClipData>,
    voices: BTreeMap<AudioHandle, Voice>,
    one_shots: Vec<OneShot>,
    /// One-shots of a replaced owner: still playing out after a reset, no
    /// longer reported, but stoppable at shutdown.
    released_one_shots: Vec<Playback>,
    facts: Vec<RealizedAudioFact>,
    /// The playing video's own sound, outside the Engine buses.
    soundtrack: Option<StreamingSoundHandle<FromFileError>>,
    /// The soundtrack's own track, suspended with the buses, so a soundtrack
    /// that starts while the world holds holds at its start.
    soundtrack_track: TrackHandle,
    /// The soundtrack's position after a held advance, until it resumes.
    soundtrack_held_at: Option<f64>,
    /// How fast world time runs against realtime; every sound plays at this
    /// multiple of its own rate (see [`AudioRealizer::set_world_rate`]).
    world_rate: f64,
}

impl AudioRealizer<DefaultBackend> {
    /// Opens the system default output device. kira follows later
    /// default-device changes itself; dropping the realizer closes the stream.
    pub fn open_default_device() -> Result<Self, String> {
        Self::with_backend_settings(Default::default())
    }
}

impl AudioRealizer<StreamBackend> {
    /// Mixes in real time into `sink` at `sample_rate`, with no device.
    pub fn open_stream(sample_rate: u32, sink: AudioBlockSink) -> Result<Self, String> {
        Self::with_backend_settings(StreamBackendSettings { sample_rate, sink })
    }
}

impl<B: Backend> AudioRealizer<B> {
    pub fn with_backend_settings(backend_settings: B::Settings) -> Result<Self, String> {
        let settings = AudioManagerSettings {
            capacities: Capacities {
                sub_track_capacity: SUB_TRACK_CAPACITY,
                ..Capacities::default()
            },
            main_track_builder: Default::default(),
            internal_buffer_size: AudioManagerSettings::<DefaultBackend>::default()
                .internal_buffer_size,
            backend_settings,
        };
        let mut manager = AudioManager::<B>::new(settings)
            .map_err(|_| "audio output device could not be opened".to_owned())?;
        let listener = manager
            .add_listener(
                mint::Vector3::from([0.0, 0.0, 0.0]),
                mint::Quaternion::from([0.0, 0.0, 0.0, 1.0]),
            )
            .map_err(|error| error.to_string())?;
        let mut bus = || {
            manager
                .add_sub_track(
                    TrackBuilder::new()
                        .sound_capacity(BUS_SOUND_CAPACITY)
                        .sub_track_capacity(SUB_TRACK_CAPACITY),
                )
                .map_err(|error| error.to_string())
        };
        let buses = [bus()?, bus()?, bus()?, bus()?];
        let soundtrack_track = manager
            .add_sub_track(TrackBuilder::new())
            .map_err(|error| error.to_string())?;
        Ok(Self {
            manager,
            listener,
            buses,
            bus_states: [BusState::DEFAULT; BUSES.len()],
            clips: HashMap::new(),
            voices: BTreeMap::new(),
            one_shots: Vec::new(),
            released_one_shots: Vec::new(),
            facts: Vec::new(),
            soundtrack: None,
            soundtrack_track,
            soundtrack_held_at: None,
            world_rate: 1.0,
        })
    }

    pub fn backend_mut(&mut self) -> &mut B {
        self.manager.backend_mut()
    }

    /// Applies committed audio ops in order; other domains are ignored. A
    /// failing op is reported as a diagnostic fact and the rest still apply.
    pub fn apply<'a>(
        &mut self,
        ops: impl IntoIterator<Item = &'a PresentationOp>,
        clips: &impl AudioClipSource,
        entities: &impl AudioEntityPositions,
    ) {
        for op in ops {
            let PresentationOp::Audio { meta, op } = op else {
                continue;
            };
            if let Err((code, message)) = self.apply_op(meta.sequence, op, clips, entities) {
                let (handle, signal_handle) = match op {
                    AudioProjectionOp::Emit { signal_handle, .. } => (None, Some(*signal_handle)),
                    AudioProjectionOp::Retire { .. } => (None, None),
                    AudioProjectionOp::Create { handle, .. }
                    | AudioProjectionOp::Restore { handle, .. }
                    | AudioProjectionOp::Update { handle, .. }
                    | AudioProjectionOp::Destroy { handle }
                    | AudioProjectionOp::VoiceControl { handle, .. } => (Some(*handle), None),
                    AudioProjectionOp::BusControl { .. } => (None, None),
                };
                self.facts.push(RealizedAudioFact::Diagnostic {
                    diagnostic: AudioProjectionDiagnostic {
                        code,
                        sequence: meta.sequence,
                        handle,
                        message,
                    },
                    signal_handle,
                });
            }
        }
    }

    /// Stops retained voices and restores default buses before a baseline.
    /// One-shots already playing finish on their own, as they do in the
    /// browser, but their completions belong to the replaced owner and are
    /// not reported.
    pub fn reset(&mut self) {
        for voice in self.voices.values_mut() {
            if let Some(playback) = voice.playback.as_mut() {
                playback.stop();
            }
        }
        self.voices.clear();
        self.released_one_shots
            .extend(self.one_shots.drain(..).map(|one_shot| one_shot.playback));
        self.released_one_shots
            .retain(|playback| !playback.finished());
        self.facts.clear();
        for bus in BUSES {
            self.bus_states[bus_index(bus)] = BusState::DEFAULT;
            self.apply_bus_gain(bus);
        }
    }

    /// Stops every sound, one-shots included, for a runtime that has shut
    /// down. Unlike [`Self::reset`], nothing keeps playing.
    pub fn stop_all(&mut self) {
        self.reset();
        self.stop_soundtrack();
        for playback in &mut self.released_one_shots {
            playback.stop();
        }
        self.released_one_shots.clear();
    }

    /// Stops one active one-shot and forgets its realization identity without
    /// reporting a natural completion. The Engine service releases the
    /// product's pending clip owner when it admits the matching operation;
    /// unknown and already-completed signals are intentionally idempotent.
    pub fn retire_one_shot(&mut self, signal_handle: AudioSignalHandle) {
        self.one_shots.retain_mut(|one_shot| {
            if one_shot.signal_handle == signal_handle {
                one_shot.playback.stop();
                false
            } else {
                true
            }
        });
    }

    /// Follows the runtime lifecycle: a paused runtime advances no Engine
    /// cursor, so the device holds every sound where it is.
    pub fn set_suspended(&mut self, suspended: bool) {
        for bus in &mut self.buses {
            if suspended {
                bus.pause(IMMEDIATE);
            } else {
                bus.resume(IMMEDIATE);
            }
        }
        if suspended {
            self.soundtrack_track.pause(IMMEDIATE);
        } else {
            self.soundtrack_track.resume(IMMEDIATE);
        }
        if !suspended {
            // Resumed tracks apply the held seeks; kira's positions are
            // current again from here.
            self.soundtrack_held_at = None;
            let voices = self
                .voices
                .values_mut()
                .filter_map(|voice| voice.playback.as_mut());
            let one_shots = self
                .one_shots
                .iter_mut()
                .map(|one_shot| &mut one_shot.playback);
            for playback in voices.chain(one_shots).chain(&mut self.released_one_shots) {
                playback.held_at = None;
            }
        }
    }

    /// Plays every sound, and the video soundtrack, at `rate` times its own
    /// rate, so device cursors keep pace with the Engine's when gameplay time
    /// runs slower than realtime. This resamples (pitch falls with the rate);
    /// it is not a time stretch. A held world (rate 0) is suspended instead.
    pub fn set_world_rate(&mut self, rate: f64) {
        if !rate.is_finite() || rate <= 0.0 || rate == self.world_rate {
            return;
        }
        self.world_rate = rate;
        for voice in self.voices.values_mut() {
            if let Some(playback) = voice.playback.as_mut() {
                playback.follow_world(rate);
            }
        }
        for one_shot in &mut self.one_shots {
            one_shot.playback.follow_world(rate);
        }
        for playback in &mut self.released_one_shots {
            playback.follow_world(rate);
        }
        if let Some(soundtrack) = &mut self.soundtrack {
            soundtrack.set_playback_rate(PlaybackRate(rate), IMMEDIATE);
        }
    }

    /// Moves every playing sound and the video soundtrack on by `world_seconds`
    /// of world time that passed while the device was suspended (a playtest
    /// inspection advance runs world steps with the world held), so the device
    /// stands where the Engine's cursors do. A one-shot moved past its end
    /// completes as it would have.
    pub fn advance_held(&mut self, world_seconds: f64) {
        if !world_seconds.is_finite() || world_seconds <= 0.0 {
            return;
        }
        for voice in self.voices.values_mut() {
            if voice.state != RealizedVoiceState::Playing {
                continue;
            }
            if let Some(playback) = voice.playback.as_mut() {
                playback.advance(world_seconds, voice.descriptor.looping);
            }
        }
        // One-shots never loop.
        for one_shot in &mut self.one_shots {
            one_shot.playback.advance(world_seconds, false);
        }
        for playback in &mut self.released_one_shots {
            playback.advance(world_seconds, false);
        }
        if let Some(soundtrack) = &mut self.soundtrack {
            let target = self
                .soundtrack_held_at
                .unwrap_or_else(|| soundtrack.position())
                + world_seconds;
            soundtrack.seek_to(target);
            self.soundtrack_held_at = Some(target);
        }
    }

    /// Play a WebM video clip's own sound from its start, replacing any
    /// other. A clip without an Opus track plays silently. The browser's
    /// video element played it outside the Engine buses; so does this.
    pub fn play_soundtrack(&mut self, clip: &[u8]) -> Result<(), String> {
        self.stop_soundtrack();
        let clip = render_video::VideoClip::open(clip).map_err(|error| error.to_string())?;
        if clip.audio.is_none() {
            return Ok(());
        }
        let decoder =
            soundtrack::SoundtrackDecoder::new(clip).map_err(|error| error.to_string())?;
        let handle = self
            .soundtrack_track
            .play(
                StreamingSoundData::from_decoder(decoder)
                    .playback_rate(PlaybackRate(self.world_rate)),
            )
            .map_err(|error| error.to_string())?;
        self.soundtrack = Some(handle);
        self.soundtrack_held_at = None;
        Ok(())
    }

    pub fn stop_soundtrack(&mut self) {
        self.soundtrack_held_at = None;
        if let Some(mut soundtrack) = self.soundtrack.take() {
            soundtrack.stop(IMMEDIATE);
        }
    }

    /// Seconds into the playing soundtrack, if one is playing.
    pub fn soundtrack_position(&self) -> Option<f64> {
        self.soundtrack
            .as_ref()
            .filter(|soundtrack| soundtrack.state() != PlaybackState::Stopped)
            .map(|soundtrack| {
                self.soundtrack_held_at
                    .unwrap_or_else(|| soundtrack.position())
            })
    }

    /// Drops decoded data for clips the Engine no longer owns, unless a
    /// realized voice still plays them.
    pub fn retain_clips(&mut self, admitted: impl Fn(&str) -> bool) {
        let voices = &self.voices;
        self.clips.retain(|hash, _| {
            admitted(hash)
                || voices
                    .values()
                    .any(|voice| voice.descriptor.clip.content_hash == *hash)
        });
    }

    /// Places the listener at a camera pose. `forward` and `up` need not be
    /// unit length; a degenerate basis keeps the previous orientation.
    /// Without a call the listener stays at the origin facing -Z.
    pub fn set_listener_pose(&mut self, position: [f32; 3], forward: [f32; 3], up: [f32; 3]) {
        self.listener
            .set_position(mint::Vector3::from(position), IMMEDIATE);
        if let Some(orientation) = listener_orientation(forward, up) {
            self.listener
                .set_orientation(mint::Quaternion::from(orientation), IMMEDIATE);
        }
    }

    /// Notices sounds that ended on the device and follows entity-attached
    /// emitters. Call between Engine calls; then [`Self::take_facts`].
    pub fn refresh(&mut self, entities: &impl AudioEntityPositions) {
        let facts = &mut self.facts;
        self.released_one_shots
            .retain(|playback| !playback.finished());
        self.one_shots.retain_mut(|one_shot| {
            if let Some(message) = one_shot.playback.take_error() {
                facts.push(decode_failure(
                    one_shot.sequence,
                    None,
                    Some(one_shot.signal_handle),
                    message,
                ));
                return false;
            }
            if !one_shot.playback.finished() {
                return true;
            }
            // A looping one-shot never ends naturally; it can only be stopped.
            facts.push(RealizedAudioFact::OneShotCompleted {
                sequence: one_shot.sequence,
                signal_handle: one_shot.signal_handle,
            });
            false
        });
        for (handle, voice) in &mut self.voices {
            let Some(playback) = voice.playback.as_mut() else {
                continue;
            };
            if let Some(message) = playback.take_error() {
                facts.push(decode_failure(voice.sequence, Some(*handle), None, message));
            }
            if playback.finished() {
                voice.cursor = playback.duration.unwrap_or(voice.cursor);
                voice.state = RealizedVoiceState::Completed;
                voice.playback = None;
                if !voice.descriptor.looping {
                    facts.push(RealizedAudioFact::VoiceCompleted {
                        sequence: voice.sequence,
                        handle: *handle,
                    });
                }
                continue;
            }
            if let (AudioEmitter::EntityAttached { entity, offset }, Some(track)) =
                (&voice.descriptor.emitter, playback.spatial.as_mut())
            {
                if let Some(base) = entities.entity_position(*entity) {
                    track.set_position(mint::Vector3::from(add(base, *offset)), IMMEDIATE);
                }
            }
        }
    }

    pub fn take_facts(&mut self) -> Vec<RealizedAudioFact> {
        std::mem::take(&mut self.facts)
    }

    pub fn voice_state(&self, handle: AudioHandle) -> Option<RealizedVoiceState> {
        self.voices.get(&handle).map(|voice| voice.state)
    }

    /// The device's playback position, for inspection and tests only. The
    /// Engine cursor is the projector's.
    pub fn voice_cursor(&self, handle: AudioHandle) -> Option<f64> {
        self.voices.get(&handle).map(|voice| match &voice.playback {
            Some(playback) => playback.cursor(voice.descriptor.looping),
            None => voice.cursor,
        })
    }

    pub fn readout(&self) -> AudioRealizationReadout {
        AudioRealizationReadout {
            voices: self.voices.len(),
            playing_voices: self
                .voices
                .values()
                .filter(|voice| voice.state == RealizedVoiceState::Playing)
                .count(),
            one_shots: self.one_shots.len(),
            decoded_clips: self.clips.len(),
        }
    }

    fn apply_op(
        &mut self,
        sequence: u32,
        op: &AudioProjectionOp,
        clips: &impl AudioClipSource,
        entities: &impl AudioEntityPositions,
    ) -> Result<(), OpError> {
        match op {
            AudioProjectionOp::Emit {
                signal_handle,
                descriptor,
                ..
            } => {
                let playback = self.play(descriptor, 0.0, clips, entities)?;
                self.one_shots.push(OneShot {
                    sequence,
                    signal_handle: *signal_handle,
                    playback,
                });
            }
            AudioProjectionOp::Retire { signal_handle } => self.retire_one_shot(*signal_handle),
            AudioProjectionOp::Create { handle, descriptor } => self.insert_voice(
                *handle,
                sequence,
                descriptor,
                AudioVoiceDesiredState::Playing,
                0.0,
                clips,
                entities,
            )?,
            AudioProjectionOp::Restore {
                handle,
                descriptor,
                desired_state,
                cursor_seconds,
            } => self.insert_voice(
                *handle,
                sequence,
                descriptor,
                *desired_state,
                *cursor_seconds,
                clips,
                entities,
            )?,
            AudioProjectionOp::Update { handle, patch } => {
                self.update_voice(*handle, sequence, patch, clips, entities)?;
            }
            AudioProjectionOp::Destroy { handle } => {
                let mut voice = self.voices.remove(handle).ok_or_else(unknown_handle)?;
                if let Some(playback) = voice.playback.as_mut() {
                    playback.stop();
                }
            }
            AudioProjectionOp::VoiceControl { handle, control } => {
                self.control_voice(*handle, sequence, *control, clips, entities)?;
            }
            AudioProjectionOp::BusControl { bus, control } => {
                let state = &mut self.bus_states[bus_index(*bus)];
                match *control {
                    AudioBusControl::SetVolume { volume } => state.volume = volume,
                    AudioBusControl::SetMuted { muted } => state.muted = muted,
                }
                self.apply_bus_gain(*bus);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_voice(
        &mut self,
        handle: AudioHandle,
        sequence: u32,
        descriptor: &AudioSourceDescriptor,
        desired: AudioVoiceDesiredState,
        cursor: f64,
        clips: &impl AudioClipSource,
        entities: &impl AudioEntityPositions,
    ) -> Result<(), OpError> {
        if self.voices.contains_key(&handle) {
            return Err((
                AudioProjectionDiagnosticCode::DuplicateHandle,
                "audio handle is already realized".to_owned(),
            ));
        }
        let duration = self.clip_duration(&descriptor.clip, clips)?;
        let cursor = normalize_cursor(cursor, duration, descriptor.looping);
        let finished = !descriptor.looping && duration.is_some_and(|end| cursor >= end);
        let (state, playback) = match desired {
            // A baseline cursor at the end of a finite clip is already
            // complete; starting it would fabricate a second completion.
            AudioVoiceDesiredState::Playing if finished => (RealizedVoiceState::Completed, None),
            AudioVoiceDesiredState::Playing => (
                RealizedVoiceState::Playing,
                Some(self.play(descriptor, cursor, clips, entities)?),
            ),
            AudioVoiceDesiredState::Paused => (RealizedVoiceState::Paused, None),
        };
        self.voices.insert(
            handle,
            Voice {
                descriptor: descriptor.clone(),
                sequence,
                state,
                cursor,
                playback,
            },
        );
        Ok(())
    }

    fn update_voice(
        &mut self,
        handle: AudioHandle,
        sequence: u32,
        patch: &AudioSourcePatch,
        clips: &impl AudioClipSource,
        entities: &impl AudioEntityPositions,
    ) -> Result<(), OpError> {
        let world_rate = self.world_rate;
        let voice = self.voices.get_mut(&handle).ok_or_else(unknown_handle)?;
        let next = patched(voice.descriptor.clone(), patch);
        let previous = std::mem::replace(&mut voice.descriptor, next);
        voice.sequence = sequence;
        let Some(playback) = voice.playback.as_mut() else {
            return Ok(());
        };
        // A spatial track's range, rolloff and emitter kind are fixed when it
        // is built; changing them replays the voice from its cursor on a new one.
        let rebuild = is_spatial(&previous) != is_spatial(&voice.descriptor)
            || previous.max_distance != voice.descriptor.max_distance
            || previous.rolloff != voice.descriptor.rolloff
            || (is_spatial(&previous) && patch.emitter.is_some());
        if !rebuild {
            playback.apply(&voice.descriptor, world_rate);
            return Ok(());
        }
        let cursor = playback.cursor(previous.looping);
        playback.stop();
        voice.playback = None;
        if voice.state != RealizedVoiceState::Playing {
            // A paused voice keeps its cursor; Resume builds the new track.
            voice.cursor = cursor;
            return Ok(());
        }
        let descriptor = voice.descriptor.clone();
        let playback = self.play(&descriptor, cursor, clips, entities)?;
        if let Some(voice) = self.voices.get_mut(&handle) {
            voice.playback = Some(playback);
        }
        Ok(())
    }

    fn control_voice(
        &mut self,
        handle: AudioHandle,
        sequence: u32,
        control: AudioVoiceControl,
        clips: &impl AudioClipSource,
        entities: &impl AudioEntityPositions,
    ) -> Result<(), OpError> {
        let voice = self.voices.get_mut(&handle).ok_or_else(unknown_handle)?;
        voice.sequence = sequence;
        let restart_at = match control {
            AudioVoiceControl::Pause => {
                if voice.state == RealizedVoiceState::Playing {
                    if let Some(playback) = voice.playback.as_mut() {
                        playback.pause();
                    }
                    voice.state = RealizedVoiceState::Paused;
                }
                return Ok(());
            }
            AudioVoiceControl::Resume => match (voice.state, voice.playback.as_mut()) {
                (RealizedVoiceState::Paused, Some(playback)) => {
                    playback.resume();
                    voice.state = RealizedVoiceState::Playing;
                    return Ok(());
                }
                (RealizedVoiceState::Paused, None) => voice.cursor,
                // Playing needs nothing; a completed voice has nothing left.
                _ => return Ok(()),
            },
            AudioVoiceControl::Retrigger => {
                if let Some(playback) = voice.playback.as_mut() {
                    playback.stop();
                }
                voice.playback = None;
                voice.cursor = 0.0;
                0.0
            }
        };
        let descriptor = voice.descriptor.clone();
        let playback = self.play(&descriptor, restart_at, clips, entities)?;
        if let Some(voice) = self.voices.get_mut(&handle) {
            voice.playback = Some(playback);
            voice.state = RealizedVoiceState::Playing;
        }
        Ok(())
    }

    fn apply_bus_gain(&mut self, bus: AudioBus) {
        let gain = self.bus_states[bus_index(bus)].gain();
        self.buses[bus_index(bus)].set_volume(amplitude_to_decibels(gain), IMMEDIATE);
    }

    fn clip(
        &mut self,
        clip: &AudioClipRef,
        clips: &impl AudioClipSource,
    ) -> Result<ClipData, OpError> {
        if let Some(data) = self.clips.get(&clip.content_hash) {
            return Ok(data.clone());
        }
        let bytes = clips.clip_bytes(&clip.content_hash).ok_or((
            AudioProjectionDiagnosticCode::AssetMissing,
            "audio clip bytes are not admitted".to_owned(),
        ))?;
        let data = if bytes.starts_with(b"RIFF") {
            ClipData::Static(Box::new(
                StaticSoundData::from_cursor(Cursor::new(bytes))
                    .map_err(|error| decode_failed(&error))?,
            ))
        } else {
            let opus = AudioContainer::identify(&bytes) == Some(AudioContainer::Opus);
            // Probe once so a codec failure reports on the op that used it.
            streaming_data(Arc::clone(&bytes), opus).map_err(|error| decode_failed(&error))?;
            ClipData::Encoded { bytes, opus }
        };
        self.clips.insert(clip.content_hash.clone(), data.clone());
        Ok(data)
    }

    fn clip_duration(
        &mut self,
        clip: &AudioClipRef,
        clips: &impl AudioClipSource,
    ) -> Result<Option<f64>, OpError> {
        Ok(match self.clip(clip, clips)? {
            ClipData::Static(data) => Some(data.duration().as_secs_f64()),
            // Streaming durations come from container metadata that the
            // Engine does not admit; use the Engine's when it has one.
            ClipData::Encoded { .. } => clip.duration_seconds,
        })
    }

    fn play(
        &mut self,
        descriptor: &AudioSourceDescriptor,
        cursor: f64,
        clips: &impl AudioClipSource,
        entities: &impl AudioEntityPositions,
    ) -> Result<Playback, OpError> {
        let data = self.clip(&descriptor.clip, clips)?;
        let bus = bus_index(descriptor.bus);
        let mut spatial = if is_spatial(descriptor) {
            let position = emitter_position(&descriptor.emitter, entities).ok_or((
                AudioProjectionDiagnosticCode::HostFailure,
                "entity-attached audio source has no projected position".to_owned(),
            ))?;
            let builder = SpatialTrackBuilder::new()
                .attenuation_function(None)
                .with_effect(rolloff::DistanceRolloff::new(
                    descriptor.max_distance,
                    descriptor.rolloff,
                ))
                .spatialization_strength(descriptor.spatial_blend)
                .persist_until_sounds_finish(true);
            let listener = self.listener.id();
            Some(
                self.buses[bus]
                    .add_spatial_sub_track(listener, mint::Vector3::from(position), builder)
                    .map_err(|error| host_failure(&error))?,
            )
        } else {
            None
        };
        let volume = amplitude_to_decibels(descriptor.volume);
        let pitch = f64::from(descriptor.pitch);
        let rate = PlaybackRate(pitch * self.world_rate);
        let pan = Panning(descriptor.pan);
        let region = loop_region(descriptor.looping);
        let (sound, duration) = match data {
            ClipData::Static(data) => {
                let duration = data.duration().as_secs_f64();
                let data = data.with_settings(
                    StaticSoundSettings::new()
                        .start_position(cursor)
                        .loop_region(region)
                        .volume(volume)
                        .playback_rate(rate)
                        .panning(pan),
                );
                let handle = match spatial.as_mut() {
                    Some(track) => track.play(data),
                    None => self.buses[bus].play(data),
                }
                .map_err(|error| host_failure(&error))?;
                (Sound::Static(handle), Some(duration))
            }
            ClipData::Encoded { bytes, opus } => {
                let data = streaming_data(bytes, opus)
                    .map_err(|error| decode_failed(&error))?
                    .with_settings(
                        StreamingSoundSettings::new()
                            .start_position(cursor)
                            .loop_region(region)
                            .volume(volume)
                            .playback_rate(rate)
                            .panning(pan),
                    );
                let duration = Some(data.duration().as_secs_f64())
                    .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
                    .or(descriptor.clip.duration_seconds);
                let handle = match spatial.as_mut() {
                    Some(track) => track.play(data),
                    None => self.buses[bus].play(data),
                }
                .map_err(|error| host_failure(&error))?;
                (Sound::Streaming(handle), duration)
            }
        };
        Ok(Playback {
            sound,
            spatial,
            duration,
            pitch,
            held_at: None,
            ended: false,
        })
    }
}

fn bus_index(bus: AudioBus) -> usize {
    match bus {
        AudioBus::Sfx => 0,
        AudioBus::Ambient => 1,
        AudioBus::Ui => 2,
        AudioBus::Music => 3,
    }
}

fn is_spatial(descriptor: &AudioSourceDescriptor) -> bool {
    !matches!(descriptor.emitter, AudioEmitter::Global2d)
}

fn emitter_position(
    emitter: &AudioEmitter,
    entities: &impl AudioEntityPositions,
) -> Option<[f32; 3]> {
    match emitter {
        AudioEmitter::Global2d => Some([0.0; 3]),
        AudioEmitter::World3d { position } => Some(*position),
        AudioEmitter::EntityAttached { entity, offset } => entities
            .entity_position(*entity)
            .map(|base| add(base, *offset)),
    }
}

fn add(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

/// The `[x, y, z, w]` rotation taking kira's listener frame (+X right, +Y
/// up, -Z forward) to the given world forward and up.
fn listener_orientation(forward: [f32; 3], up: [f32; 3]) -> Option<[f32; 4]> {
    let normalize = |v: [f32; 3]| {
        let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        (length.is_finite() && length > f32::EPSILON).then(|| v.map(|c| c / length))
    };
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let forward = normalize(forward)?;
    let right = normalize(cross(forward, up))?;
    let up = cross(right, forward);
    let back = forward.map(|c| -c);
    // Columns of the rotation matrix are the local axes in world space.
    let (m00, m01, m02) = (right[0], up[0], back[0]);
    let (m10, m11, m12) = (right[1], up[1], back[1]);
    let (m20, m21, m22) = (right[2], up[2], back[2]);
    let trace = m00 + m11 + m22;
    let quaternion = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s]
    };
    Some(quaternion)
}

fn loop_region(looping: bool) -> Option<Region> {
    looping.then(|| Region::from(0.0..))
}

/// Linear gain in 0..=1 to kira decibels; kira treats -60 dB as silence.
fn amplitude_to_decibels(amplitude: f32) -> Decibels {
    if amplitude <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels((20.0 * amplitude.log10()).max(Decibels::SILENCE.0))
    }
}

fn normalize_cursor(cursor: f64, duration: Option<f64>, looping: bool) -> f64 {
    match duration {
        Some(duration) if looping => cursor.rem_euclid(duration),
        Some(duration) => cursor.min(duration),
        None => cursor,
    }
}

fn patched(
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
    if let Some(value) = patch.max_distance {
        descriptor.max_distance = value;
    }
    if let Some(value) = patch.rolloff {
        descriptor.rolloff = value;
    }
    if let Some(value) = patch.pan {
        descriptor.pan = value;
    }
    if let Some(value) = &patch.emitter {
        descriptor.emitter = value.clone();
    }
    descriptor
}

fn unknown_handle() -> OpError {
    (
        AudioProjectionDiagnosticCode::UnknownHandle,
        "audio handle is not realized".to_owned(),
    )
}

fn decode_failed(error: &impl std::fmt::Display) -> OpError {
    (
        AudioProjectionDiagnosticCode::DecodeFailed,
        format!("audio clip decoding failed: {error}"),
    )
}

fn host_failure(error: &impl std::fmt::Display) -> OpError {
    (
        AudioProjectionDiagnosticCode::HostFailure,
        format!("audio device operation failed: {error}"),
    )
}

fn decode_failure(
    sequence: u32,
    handle: Option<AudioHandle>,
    signal_handle: Option<AudioSignalHandle>,
    message: String,
) -> RealizedAudioFact {
    RealizedAudioFact::Diagnostic {
        diagnostic: AudioProjectionDiagnostic {
            code: AudioProjectionDiagnosticCode::DecodeFailed,
            sequence,
            handle,
            message,
        },
        signal_handle,
    }
}

#[cfg(test)]
mod tests;
