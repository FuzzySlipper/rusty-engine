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
        attenuation: 1.0,
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
fn spatial_emitters_play_and_unresolved_entities_are_diagnosed() {
    let mut realizer = realizer();
    let mut world = descriptor("sha256:wav", false);
    world.emitter = AudioEmitter::World3d {
        position: [3.0, 0.0, -2.0],
    };
    world.spatial_blend = 1.0;
    world.attenuation = 20.0;
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
            for frame in self.frames.chunks_exact(2) {
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
                    attenuation: Some(2.0),
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
