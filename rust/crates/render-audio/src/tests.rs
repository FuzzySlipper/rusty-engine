use super::*;
use kira::backend::Backend;
use render_presentation::PresentationOpMeta;

const SAMPLE_RATE: u32 = 48_000;
/// kira's mock backend renders its internal buffer (128 frames) per call.
const SECONDS_PER_PROCESS: f64 = 128.0 / SAMPLE_RATE as f64;
const WAV: &[u8] = include_bytes!("../../../../fixtures/audio-containers/tone.wav");
const OGG: &[u8] = include_bytes!("../../../../fixtures/audio-containers/tone.ogg");
const OPUS: &[u8] = include_bytes!("../../../../fixtures/audio-containers/tone.opus");
const MP3: &[u8] = include_bytes!("../../../../fixtures/audio-containers/tone.mp3");
const FLAC: &[u8] = include_bytes!("../../../../fixtures/audio-containers/tone.flac");

struct Clips(Vec<(&'static str, &'static [u8])>);

impl Clips {
    fn fixtures() -> Self {
        Self(vec![
            ("sha256:wav", WAV),
            ("sha256:ogg", OGG),
            ("sha256:opus", OPUS),
            ("sha256:mp3", MP3),
            ("sha256:flac", FLAC),
        ])
    }
}

impl AudioClipSource for Clips {
    fn clip_bytes(&self, content_hash: &str) -> Option<Arc<[u8]>> {
        self.0
            .iter()
            .find(|(hash, _)| *hash == content_hash)
            .map(|(_, bytes)| Arc::from(*bytes))
    }
}

fn realizer() -> AudioRealizer<MockBackend> {
    AudioRealizer::with_backend_settings(MockBackendSettings {
        sample_rate: SAMPLE_RATE,
    })
    .expect("mock backend opens")
}

fn run(realizer: &mut AudioRealizer<MockBackend>, seconds: f64) {
    let steps = (seconds / SECONDS_PER_PROCESS).ceil() as usize;
    for _ in 0..steps {
        let backend = realizer.backend_mut();
        backend.on_start_processing();
        backend.process();
    }
}

fn descriptor(hash: &str, looping: bool) -> AudioSourceDescriptor {
    AudioSourceDescriptor {
        clip: AudioClipRef {
            asset: format!("audio/{hash}"),
            content_hash: hash.to_owned(),
            duration_seconds: None,
        },
        bus: AudioBus::Sfx,
        volume: 0.8,
        pitch: 1.0,
        looping,
        spatial_blend: 0.0,
        max_distance: 1.0,
        rolloff: render_presentation::AudioRolloff::Linear,
        pan: 0.0,
        emitter: AudioEmitter::Global2d,
    }
}

fn op(sequence: u32, op: AudioProjectionOp) -> PresentationOp {
    PresentationOp::Audio {
        meta: PresentationOpMeta::new(sequence),
        op,
    }
}

fn emit(sequence: u32, signal: u64, descriptor: AudioSourceDescriptor) -> PresentationOp {
    op(
        sequence,
        AudioProjectionOp::Emit {
            signal_handle: AudioSignalHandle::new(signal),
            signal_id: "fire".to_owned(),
            descriptor,
        },
    )
}

fn restore(
    handle: u64,
    descriptor: AudioSourceDescriptor,
    desired_state: AudioVoiceDesiredState,
    cursor_seconds: f64,
) -> PresentationOp {
    op(
        1,
        AudioProjectionOp::Restore {
            handle: AudioHandle::new(handle),
            descriptor,
            desired_state,
            cursor_seconds,
        },
    )
}

fn control(handle: u64, control: AudioVoiceControl) -> PresentationOp {
    op(
        2,
        AudioProjectionOp::VoiceControl {
            handle: AudioHandle::new(handle),
            control,
        },
    )
}

fn apply(realizer: &mut AudioRealizer<MockBackend>, ops: &[PresentationOp]) {
    realizer.apply(ops, &Clips::fixtures(), &NoEntityPositions);
}

#[test]
fn one_shots_complete_for_every_decoded_container() {
    for (signal, hash) in [
        (1, "sha256:wav"),
        (2, "sha256:ogg"),
        (3, "sha256:mp3"),
        (4, "sha256:flac"),
        (5, "sha256:opus"),
    ] {
        let mut realizer = realizer();
        apply(&mut realizer, &[emit(7, signal, descriptor(hash, false))]);
        assert_eq!(realizer.take_facts(), [], "{hash} starts");
        run(&mut realizer, 0.5);
        realizer.refresh(&NoEntityPositions);
        assert_eq!(realizer.take_facts(), [], "{hash} is still playing");
        // Compressed clips stream from a decoder thread; the mock backend
        // renders faster than real time, so give that thread time to keep up.
        let mut facts = Vec::new();
        for _ in 0..200 {
            run(&mut realizer, 0.05);
            realizer.refresh(&NoEntityPositions);
            facts = realizer.take_facts();
            if !facts.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            facts,
            [RealizedAudioFact::OneShotCompleted {
                sequence: 7,
                signal_handle: AudioSignalHandle::new(signal),
            }],
            "{hash} completes once"
        );
        assert_eq!(realizer.readout().one_shots, 0);
    }
}

#[test]
fn a_looping_baseline_voice_resumes_from_the_engine_cursor() {
    for hash in ["sha256:wav", "sha256:ogg", "sha256:opus"] {
        let mut realizer = realizer();
        // 1.25 s into a 1 s loop is 0.25 s into the clip.
        let mut looped = descriptor(hash, true);
        looped.clip.duration_seconds = Some(1.0);
        apply(
            &mut realizer,
            &[restore(4, looped, AudioVoiceDesiredState::Playing, 1.25)],
        );
        run(&mut realizer, SECONDS_PER_PROCESS);
        let cursor = realizer.voice_cursor(AudioHandle::new(4)).expect("voice");
        assert!((cursor - 0.25).abs() < 0.02, "{hash} cursor {cursor}");
        run(&mut realizer, 2.0);
        realizer.refresh(&NoEntityPositions);
        assert_eq!(
            realizer.voice_state(AudioHandle::new(4)),
            Some(RealizedVoiceState::Playing),
            "{hash} loops"
        );
        assert_eq!(realizer.take_facts(), []);
    }
}

#[test]
fn a_finished_baseline_voice_is_not_restarted_or_completed_again() {
    let mut realizer = realizer();
    apply(
        &mut realizer,
        &[restore(
            5,
            descriptor("sha256:wav", false),
            AudioVoiceDesiredState::Playing,
            1.0,
        )],
    );
    assert_eq!(
        realizer.voice_state(AudioHandle::new(5)),
        Some(RealizedVoiceState::Completed)
    );
    run(&mut realizer, 0.1);
    realizer.refresh(&NoEntityPositions);
    assert_eq!(realizer.take_facts(), []);
}

#[test]
fn a_retained_voice_reports_its_natural_completion() {
    let mut realizer = realizer();
    apply(
        &mut realizer,
        &[op(
            6,
            AudioProjectionOp::Create {
                handle: AudioHandle::new(8),
                descriptor: descriptor("sha256:wav", false),
            },
        )],
    );
    run(&mut realizer, 1.1);
    realizer.refresh(&NoEntityPositions);
    assert_eq!(
        realizer.take_facts(),
        [RealizedAudioFact::VoiceCompleted {
            sequence: 6,
            handle: AudioHandle::new(8),
        }]
    );
    assert_eq!(
        realizer.voice_state(AudioHandle::new(8)),
        Some(RealizedVoiceState::Completed)
    );
}

#[test]
fn a_paused_baseline_voice_resumes_at_its_cursor_and_controls_keep_position() {
    let mut realizer = realizer();
    apply(
        &mut realizer,
        &[restore(
            2,
            descriptor("sha256:wav", true),
            AudioVoiceDesiredState::Paused,
            0.4,
        )],
    );
    assert_eq!(
        realizer.voice_state(AudioHandle::new(2)),
        Some(RealizedVoiceState::Paused)
    );
    apply(&mut realizer, &[control(2, AudioVoiceControl::Resume)]);
    run(&mut realizer, 0.2);
    let resumed = realizer.voice_cursor(AudioHandle::new(2)).expect("voice");
    assert!((resumed - 0.6).abs() < 0.02, "resumed at {resumed}");

    apply(&mut realizer, &[control(2, AudioVoiceControl::Pause)]);
    run(&mut realizer, 0.3);
    let paused = realizer.voice_cursor(AudioHandle::new(2)).expect("voice");
    assert!((paused - resumed).abs() < 0.02, "pause holds {paused}");

    apply(&mut realizer, &[control(2, AudioVoiceControl::Retrigger)]);
    run(&mut realizer, 0.1);
    let retriggered = realizer.voice_cursor(AudioHandle::new(2)).expect("voice");
    assert!(retriggered < 0.12, "retrigger restarts at {retriggered}");
    assert_eq!(realizer.take_facts(), []);
}

#[test]
fn reset_stops_voices_and_forgets_one_shot_owners() {
    let mut realizer = realizer();
    apply(
        &mut realizer,
        &[
            emit(1, 1, descriptor("sha256:wav", false)),
            restore(
                3,
                descriptor("sha256:ogg", true),
                AudioVoiceDesiredState::Playing,
                0.0,
            ),
        ],
    );
    realizer.reset();
    run(&mut realizer, 1.2);
    realizer.refresh(&NoEntityPositions);
    assert_eq!(realizer.take_facts(), []);
    assert_eq!(realizer.readout().voices, 0);
    assert_eq!(realizer.readout().one_shots, 0);
}

#[test]
fn suspension_holds_playback_position() {
    let mut realizer = realizer();
    apply(
        &mut realizer,
        &[restore(
            1,
            descriptor("sha256:wav", true),
            AudioVoiceDesiredState::Playing,
            0.0,
        )],
    );
    run(&mut realizer, 0.2);
    realizer.set_suspended(true);
    run(&mut realizer, 0.05);
    let held = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    run(&mut realizer, 0.5);
    let still = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    assert!(
        (still - held).abs() < 1e-9,
        "suspended cursor moved {held} -> {still}"
    );
    realizer.set_suspended(false);
    run(&mut realizer, 0.2);
    let moved = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    assert!(moved > still + 0.1, "resumed cursor {moved}");
}

#[test]
fn playback_keeps_pace_with_a_slowed_world() {
    let mut realizer = realizer();
    let mut looped = descriptor("sha256:wav", true);
    looped.clip.duration_seconds = Some(1.0);
    apply(
        &mut realizer,
        &[restore(1, looped, AudioVoiceDesiredState::Playing, 0.0)],
    );
    // A tenth of realtime: half a second of device time is 0.05 s of world
    // time, as the Engine cursor advances.
    realizer.set_world_rate(0.1);
    run(&mut realizer, 0.02);
    let from = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    run(&mut realizer, 0.5);
    let slow = realizer.voice_cursor(AudioHandle::new(1)).expect("voice") - from;
    assert!((slow - 0.05).abs() < 0.01, "slowed cursor moved {slow}");
    // A voice started while slowed starts slowed too.
    apply(
        &mut realizer,
        &[emit(2, 7, descriptor("sha256:wav", false))],
    );
    realizer.set_world_rate(1.0);
    run(&mut realizer, 0.02);
    let from = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    run(&mut realizer, 0.3);
    let realtime = realizer.voice_cursor(AudioHandle::new(1)).expect("voice") - from;
    assert!(
        (realtime - 0.3).abs() < 0.02,
        "realtime cursor moved {realtime}"
    );
}

#[test]
fn world_time_advanced_while_suspended_moves_every_sound_on() {
    let mut realizer = realizer();
    let mut looped = descriptor("sha256:wav", true);
    looped.clip.duration_seconds = Some(1.0);
    apply(
        &mut realizer,
        &[
            restore(1, looped, AudioVoiceDesiredState::Playing, 0.0),
            emit(2, 7, descriptor("sha256:wav", false)),
        ],
    );
    run(&mut realizer, 0.1);
    // Held (an inspection hold): the device stops where it is...
    realizer.set_suspended(true);
    run(&mut realizer, 0.05);
    let held = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    // ...and the inspection advance runs 0.4 s of world time.
    realizer.advance_held(0.4);
    run(&mut realizer, 0.05);
    let advanced = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    assert!(
        (advanced - held - 0.4).abs() < 0.02,
        "voice moved {held} -> {advanced}"
    );
    // A further 1 s carries the one-shot (about 1 s long) past its end: it
    // completes at that world moment, while the device is still held.
    realizer.advance_held(1.0);
    run(&mut realizer, 0.05);
    realizer.refresh(&NoEntityPositions);
    assert_eq!(
        realizer.take_facts(),
        [RealizedAudioFact::OneShotCompleted {
            sequence: 2,
            signal_handle: AudioSignalHandle::new(7),
        }]
    );
    let looped_on = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    assert!(
        (looped_on - (advanced + 1.0) % 1.0).abs() < 0.02,
        "looping voice wraps: {advanced} -> {looped_on}"
    );
    // Resuming plays on from that world moment, not from where it held.
    realizer.set_suspended(false);
    run(&mut realizer, 0.1);
    let resumed = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    assert!(
        (resumed - (looped_on + 0.1)).abs() < 0.03,
        "resumed at {resumed}, expected about {}",
        looped_on + 0.1
    );
}

#[test]
fn spatial_emitters_play_and_unresolved_entities_are_diagnosed() {
    let mut realizer = realizer();
    let mut world = descriptor("sha256:wav", false);
    world.emitter = AudioEmitter::World3d {
        position: [3.0, 0.0, -2.0],
    };
    world.spatial_blend = 1.0;
    world.max_distance = 20.0;
    let mut attached = world.clone();
    attached.emitter = AudioEmitter::EntityAttached {
        entity: 12,
        offset: [0.0; 3],
    };
    apply(&mut realizer, &[emit(1, 1, world), emit(2, 2, attached)]);
    let facts = realizer.take_facts();
    assert!(matches!(
        facts.as_slice(),
        [RealizedAudioFact::Diagnostic { diagnostic, signal_handle: Some(signal) }]
            if diagnostic.code == AudioProjectionDiagnosticCode::HostFailure
                && *signal == AudioSignalHandle::new(2)
    ));
    run(&mut realizer, 1.1);
    realizer.refresh(&NoEntityPositions);
    assert_eq!(
        realizer.take_facts(),
        [RealizedAudioFact::OneShotCompleted {
            sequence: 1,
            signal_handle: AudioSignalHandle::new(1),
        }]
    );
}

#[test]
fn missing_bytes_and_unknown_handles_are_diagnosed_per_op() {
    let mut realizer = realizer();
    apply(
        &mut realizer,
        &[
            emit(1, 1, descriptor("sha256:absent", false)),
            control(99, AudioVoiceControl::Pause),
            emit(3, 3, descriptor("sha256:wav", false)),
        ],
    );
    let codes: Vec<_> = realizer
        .take_facts()
        .into_iter()
        .map(|fact| match fact {
            RealizedAudioFact::Diagnostic { diagnostic, .. } => diagnostic.code,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        codes,
        [
            AudioProjectionDiagnosticCode::AssetMissing,
            AudioProjectionDiagnosticCode::UnknownHandle
        ]
    );
    assert_eq!(realizer.readout().one_shots, 1, "later ops still apply");
}

#[test]
fn released_clips_are_dropped_unless_a_voice_still_plays_them() {
    let mut realizer = realizer();
    apply(
        &mut realizer,
        &[
            emit(1, 1, descriptor("sha256:wav", false)),
            restore(
                2,
                descriptor("sha256:ogg", true),
                AudioVoiceDesiredState::Playing,
                0.0,
            ),
        ],
    );
    assert_eq!(realizer.readout().decoded_clips, 2);
    realizer.retain_clips(|_| false);
    assert_eq!(realizer.readout().decoded_clips, 1);
}

/// Renders through kira's real mixer so tests can measure the samples a
/// device would receive, per stereo channel.
struct CaptureBackend {
    renderer: Option<kira::backend::Renderer>,
    frames: Vec<f32>,
}

impl Backend for CaptureBackend {
    type Settings = ();
    type Error = ();

    fn setup(_: (), internal_buffer_size: usize) -> Result<(Self, u32), ()> {
        Ok((
            Self {
                renderer: None,
                frames: vec![0.0; internal_buffer_size * 2],
            },
            SAMPLE_RATE,
        ))
    }

    fn start(&mut self, renderer: kira::backend::Renderer) -> Result<(), ()> {
        self.renderer = Some(renderer);
        Ok(())
    }
}

impl CaptureBackend {
    /// Peak absolute sample of the left and right channels over `seconds`.
    fn render_peaks(&mut self, seconds: f64) -> [f32; 2] {
        let mut peaks = [0.0_f32; 2];
        for _ in 0..(seconds / SECONDS_PER_PROCESS).ceil() as usize {
            let renderer = self.renderer.as_mut().expect("started");
            renderer.on_start_processing();
            renderer.process(&mut self.frames, 2);
            for frame in self.frames.as_chunks::<2>().0 {
                peaks[0] = peaks[0].max(frame[0].abs());
                peaks[1] = peaks[1].max(frame[1].abs());
            }
        }
        peaks
    }

    fn render_peak(&mut self, seconds: f64) -> f32 {
        let [left, right] = self.render_peaks(seconds);
        left.max(right)
    }
}

fn capture_realizer() -> AudioRealizer<CaptureBackend> {
    AudioRealizer::with_backend_settings(()).expect("capture backend opens")
}

#[test]
fn rebuilding_a_paused_voice_keeps_it_paused_at_its_cursor() {
    let mut realizer = realizer();
    apply(
        &mut realizer,
        &[restore(
            1,
            descriptor("sha256:wav", true),
            AudioVoiceDesiredState::Playing,
            0.0,
        )],
    );
    run(&mut realizer, 0.1);
    apply(&mut realizer, &[control(1, AudioVoiceControl::Pause)]);
    run(&mut realizer, 0.05);
    let paused = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    apply(
        &mut realizer,
        &[op(
            3,
            AudioProjectionOp::Update {
                handle: AudioHandle::new(1),
                patch: AudioSourcePatch {
                    max_distance: Some(2.0),
                    ..Default::default()
                },
            },
        )],
    );
    run(&mut realizer, 0.2);
    let held = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    assert!(
        (held - paused).abs() < 0.01,
        "paused cursor advanced {paused} -> {held}"
    );
    assert_eq!(
        realizer.voice_state(AudioHandle::new(1)),
        Some(RealizedVoiceState::Paused)
    );
    apply(&mut realizer, &[control(1, AudioVoiceControl::Resume)]);
    run(&mut realizer, 0.1);
    let resumed = realizer.voice_cursor(AudioHandle::new(1)).expect("voice");
    assert!((resumed - held - 0.1).abs() < 0.02, "resumed at {resumed}");
}

#[test]
fn stop_all_silences_one_shots_and_voices() {
    let mut realizer = capture_realizer();
    realizer.apply(
        &[
            emit(1, 1, descriptor("sha256:wav", false)),
            restore(
                2,
                descriptor("sha256:wav", true),
                AudioVoiceDesiredState::Playing,
                0.0,
            ),
        ],
        &Clips::fixtures(),
        &NoEntityPositions,
    );
    let before = realizer.backend_mut().render_peak(0.1);
    assert!(before > 0.01, "fixture plays: {before}");
    realizer.stop_all();
    realizer.backend_mut().render_peak(0.05);
    let after = realizer.backend_mut().render_peak(0.1);
    assert!(after < 1e-5, "stopped runtime still emits {after}");
    realizer.refresh(&NoEntityPositions);
    assert_eq!(
        realizer.take_facts(),
        [],
        "a stop is not a natural completion"
    );
}

#[test]
fn retiring_one_shot_silences_it_without_completion() {
    let mut realizer = capture_realizer();
    realizer.apply(
        &[emit(1, 7, descriptor("sha256:wav", false))],
        &Clips::fixtures(),
        &NoEntityPositions,
    );
    assert!(realizer.backend_mut().render_peak(0.1) > 0.01);

    realizer.apply(
        &[op(
            2,
            AudioProjectionOp::Retire {
                signal_handle: AudioSignalHandle::new(7),
            },
        )],
        &Clips::fixtures(),
        &NoEntityPositions,
    );
    realizer.backend_mut().render_peak(0.05);
    assert!(
        realizer.backend_mut().render_peak(0.1) < 1e-5,
        "retired one-shot kept rendering"
    );
    realizer.refresh(&NoEntityPositions);
    assert_eq!(realizer.take_facts(), [], "retirement is not completion");
    assert_eq!(realizer.readout().one_shots, 0);
}

fn spatial(hash: &str, emitter: AudioEmitter, looping: bool) -> AudioSourceDescriptor {
    AudioSourceDescriptor {
        spatial_blend: 1.0,
        max_distance: 20.0,
        rolloff: render_presentation::AudioRolloff::Linear,
        emitter,
        ..descriptor(hash, looping)
    }
}

/// Settles a pose change (kira interpolates listener and track positions over
/// one buffer), then measures.
fn settled_peaks(realizer: &mut AudioRealizer<CaptureBackend>) -> [f32; 2] {
    realizer.backend_mut().render_peaks(0.02);
    realizer.backend_mut().render_peaks(0.1)
}

#[test]
fn the_listener_pose_pans_and_attenuates_world_emitters() {
    let mut realizer = capture_realizer();
    realizer.apply(
        &[restore(
            1,
            spatial(
                "sha256:wav",
                AudioEmitter::World3d {
                    position: [0.0, 0.0, 3.0],
                },
                true,
            ),
            AudioVoiceDesiredState::Playing,
            0.0,
        )],
        &Clips::fixtures(),
        &NoEntityPositions,
    );
    // Facing +X, world +Z is on the listener's right.
    realizer.set_listener_pose([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let [left, right] = settled_peaks(&mut realizer);
    assert!(right > 2.0 * left, "facing +X: left {left}, right {right}");
    // Facing -X, the same emitter is on the left.
    realizer.set_listener_pose([0.0; 3], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let [left, right] = settled_peaks(&mut realizer);
    assert!(left > 2.0 * right, "facing -X: left {left}, right {right}");
    // Moving away along -Z to 15 m from the emitter attenuates it.
    let near = left.max(right);
    realizer.set_listener_pose([0.0, 0.0, -12.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let far = {
        let [left, right] = settled_peaks(&mut realizer);
        left.max(right)
    };
    assert!(far < 0.5 * near, "near {near}, far {far}");
}

struct MovingEntity(std::cell::Cell<[f32; 3]>);

impl AudioEntityPositions for MovingEntity {
    fn entity_position(&self, entity: u64) -> Option<[f32; 3]> {
        (entity == 9).then(|| self.0.get())
    }
}

#[test]
fn an_entity_attached_voice_follows_its_entity() {
    let mut realizer = capture_realizer();
    let entity = MovingEntity(std::cell::Cell::new([0.0, 0.0, 3.0]));
    realizer.set_listener_pose([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    realizer.apply(
        &[restore(
            1,
            spatial(
                "sha256:wav",
                AudioEmitter::EntityAttached {
                    entity: 9,
                    offset: [0.0, 1.0, 0.0],
                },
                true,
            ),
            AudioVoiceDesiredState::Playing,
            0.0,
        )],
        &Clips::fixtures(),
        &entity,
    );
    assert_eq!(realizer.take_facts(), []);
    let [left, right] = settled_peaks(&mut realizer);
    assert!(right > 2.0 * left, "entity on the right: {left} {right}");
    entity.0.set([0.0, 0.0, -3.0]);
    realizer.refresh(&entity);
    let [left, right] = settled_peaks(&mut realizer);
    assert!(left > 2.0 * right, "entity moved left: {left} {right}");
}

#[test]
fn stop_all_also_stops_one_shots_released_by_an_earlier_reset() {
    let mut realizer = capture_realizer();
    realizer.apply(
        &[emit(1, 1, descriptor("sha256:wav", false))],
        &Clips::fixtures(),
        &NoEntityPositions,
    );
    assert!(realizer.backend_mut().render_peak(0.1) > 0.01);
    // A baseline reset lets the one-shot play out without reporting it.
    realizer.reset();
    assert!(
        realizer.backend_mut().render_peak(0.05) > 0.01,
        "reset keeps it playing"
    );
    realizer.stop_all();
    realizer.backend_mut().render_peak(0.05);
    let after = realizer.backend_mut().render_peak(0.1);
    assert!(after < 1e-5, "shutdown after a reset still emits {after}");
    realizer.refresh(&NoEntityPositions);
    assert_eq!(realizer.take_facts(), []);
}

#[test]
fn a_video_soundtrack_plays_pauses_with_the_runtime_and_stops() {
    let clip = include_bytes!("../../render-video/tests/fixtures/testsrc.webm");
    let mut realizer = realizer();
    assert_eq!(realizer.soundtrack_position(), None);
    realizer.play_soundtrack(clip).expect("plays");
    // The soundtrack streams from a decoder thread; the mock backend renders
    // faster than real time, so give that thread time to keep up.
    let position = |realizer: &mut AudioRealizer<MockBackend>, seconds: f64| {
        for _ in 0..(seconds / 0.05) as usize {
            run(realizer, 0.05);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        realizer.soundtrack_position().expect("not stopped")
    };
    let playing = position(&mut realizer, 0.5);
    assert!(playing > 0.0, "{playing}");
    realizer.set_suspended(true);
    let held = position(&mut realizer, 0.5);
    assert!((held - playing).abs() < 0.06, "{held} vs {playing}");
    realizer.set_suspended(false);
    realizer.stop_soundtrack();
    assert_eq!(realizer.soundtrack_position(), None);
}

/// The device plays a spatial voice at the descriptor's rolloff of its
/// distance, silent at and beyond its maximum distance, and follows a range
/// change through an ordinary voice update.
#[test]
fn spatial_voices_fall_off_to_silence_at_their_maximum_distance() {
    let tone = |realizer: &mut AudioRealizer<CaptureBackend>,
                distance: f32,
                max_distance: f32,
                rolloff: render_presentation::AudioRolloff| {
        let mut source = descriptor("sha256:wav", true);
        source.volume = 1.0;
        source.max_distance = max_distance;
        source.rolloff = rolloff;
        if distance > 0.0 {
            source.emitter = AudioEmitter::World3d {
                position: [0.0, 0.0, -distance],
            };
        }
        realizer.apply(
            &[restore(1, source, AudioVoiceDesiredState::Playing, 0.0)],
            &Clips::fixtures(),
            &NoEntityPositions,
        );
        realizer.backend_mut().render_peak(0.05);
        realizer.backend_mut().render_peak(0.2)
    };
    let fresh = capture_realizer;
    use render_presentation::AudioRolloff::{Linear, LinearDecibels};

    let full = tone(&mut fresh(), 0.0, 16.0, Linear);
    assert!(full > 0.1, "the tone did not play: {full}");
    let level = |distance, max_distance, rolloff| {
        tone(&mut fresh(), distance, max_distance, rolloff) / full
    };
    let close = |actual: f32, expected: f32| (actual - expected).abs() < 0.01;
    // Full volume within the reference distance, min(1, max / 2).
    assert!(close(level(0.5, 16.0, Linear), 1.0));
    // Linear: amplitude halves halfway from the reference to the maximum.
    assert!(close(level(8.5, 16.0, Linear), 0.5));
    // LinearDecibels: -30 dB there.
    assert!(close(level(8.5, 16.0, LinearDecibels), 10.0_f32.powf(-1.5)));
    for rolloff in [Linear, LinearDecibels] {
        assert_eq!(
            level(16.0, 16.0, rolloff),
            0.0,
            "{rolloff:?} at the maximum"
        );
        assert_eq!(
            level(20.0, 16.0, rolloff),
            0.0,
            "{rolloff:?} beyond the maximum"
        );
    }

    // A voice 20 m away: silent at 16 m, audible once its range is 24 m, and
    // silent again when the range goes back.
    let mut realizer = fresh();
    assert_eq!(tone(&mut realizer, 20.0, 16.0, Linear), 0.0);
    let range = |realizer: &mut AudioRealizer<CaptureBackend>, max_distance| {
        realizer.apply(
            &[op(
                3,
                AudioProjectionOp::Update {
                    handle: AudioHandle::new(1),
                    patch: AudioSourcePatch {
                        max_distance: Some(max_distance),
                        ..Default::default()
                    },
                },
            )],
            &Clips::fixtures(),
            &NoEntityPositions,
        );
        realizer.backend_mut().render_peak(0.05);
        realizer.backend_mut().render_peak(0.2) / full
    };
    assert!(close(range(&mut realizer, 24.0), 1.0 - 19.0 / 23.0));
    assert_eq!(range(&mut realizer, 16.0), 0.0);
}

#[test]
fn the_music_bus_silences_music_and_leaves_ambience_playing() {
    let on = |bus| AudioSourceDescriptor {
        bus,
        ..descriptor("sha256:wav", true)
    };
    let silence_music = op(
        3,
        AudioProjectionOp::BusControl {
            bus: AudioBus::Music,
            control: AudioBusControl::SetVolume { volume: 0.0 },
        },
    );
    let mut realizer = capture_realizer();
    let peak = |realizer: &mut AudioRealizer<CaptureBackend>| {
        let [left, right] = settled_peaks(realizer);
        left.max(right)
    };
    realizer.apply(
        &[restore(
            1,
            on(AudioBus::Music),
            AudioVoiceDesiredState::Playing,
            0.0,
        )],
        &Clips::fixtures(),
        &NoEntityPositions,
    );
    let music = peak(&mut realizer);
    assert!(music > 0.05, "music plays: {music}");
    realizer.apply(&[silence_music], &Clips::fixtures(), &NoEntityPositions);
    let silenced = peak(&mut realizer);
    assert!(silenced < music * 0.05, "music bus at 0: {silenced}");
    realizer.apply(
        &[restore(
            2,
            on(AudioBus::Ambient),
            AudioVoiceDesiredState::Playing,
            0.0,
        )],
        &Clips::fixtures(),
        &NoEntityPositions,
    );
    let ambience = peak(&mut realizer);
    assert!(ambience > music * 0.5, "ambience still plays: {ambience}");
}

#[test]
fn a_streamed_mix_carries_a_one_shot_in_real_time_and_reports_it_complete() {
    let peak = Arc::new(std::sync::Mutex::new((0_usize, 0.0_f32)));
    let sink = Arc::clone(&peak);
    let opened = std::time::Instant::now();
    let mut realizer = AudioRealizer::open_stream(
        SAMPLE_RATE,
        Arc::new(move |block: &[f32]| {
            let mut state = sink.lock().unwrap();
            state.0 += block.len() / 2;
            state.1 = block
                .iter()
                .fold(state.1, |peak, sample| peak.max(sample.abs()));
        }),
    )
    .expect("a stream needs no device");
    realizer.apply(
        &[emit(1, 7, descriptor("sha256:wav", false))],
        &Clips::fixtures(),
        &NoEntityPositions,
    );
    let started = std::time::Instant::now();
    let completed = loop {
        std::thread::sleep(std::time::Duration::from_millis(20));
        realizer.refresh(&NoEntityPositions);
        if let Some(fact) = realizer.take_facts().into_iter().next() {
            break fact;
        }
        assert!(
            started.elapsed().as_secs() < 5,
            "the one-shot never completed"
        );
    };
    assert_eq!(
        completed,
        RealizedAudioFact::OneShotCompleted {
            sequence: 1,
            signal_handle: AudioSignalHandle::new(7),
        }
    );
    let (frames, peak) = *peak.lock().unwrap();
    // The mix renders whole 10 ms blocks only as they fall due.
    let elapsed = opened.elapsed().as_secs_f64() * f64::from(SAMPLE_RATE);
    assert_eq!(frames % 480, 0);
    assert!((frames as f64) <= elapsed, "{frames} frames in {elapsed}");
    assert!(peak > 0.01, "the tone reached the stream: peak {peak}");
}
