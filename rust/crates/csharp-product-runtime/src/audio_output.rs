//! Audio realized for the product: on this process's output device, or mixed
//! for the pages that watch its frames.
//!
//! The runtime opens its audio output when it loads and closes it when it
//! drops. Committed audio ops play here: they are taken out of the call's
//! publications. Natural completions and realization diagnostics reach the
//! Engine as realization facts.
//!
//! With the manifest's `audio.output` at `stream` (the default for streamed
//! frames), the same realizer mixes in real time with no device and the host
//! streams each mixed block to every watching page; voices complete on time
//! whether or not a page listens. `device-optional` (the default for window
//! output) plays on the default output device, and a machine without one (a
//! CI runner, a headless server) runs silent after one warning: its audio ops
//! are dropped and report no completions. `device-required` fails the load
//! without one.
//!
//! The listener follows the camera of the primary view in the committed view
//! composition, and entity-attached emitters follow the committed graphics
//! node published for their entity. A playing video clip's own sound plays
//! here too, from the clip's start, beside the picture this process draws.

use csharp_engine_services::{AudioRealizationFact, EngineServiceSet};
use product_host::RuntimePublication;
use std::sync::Arc;

use product_host::{ProductHostAudioStream, PRODUCT_HOST_AUDIO_SAMPLE_RATE};
use render_audio::{
    AudioClipSource, AudioEntityPositions, AudioRealizer, RealizedAudioFact, StreamBackend,
};
use render_host_contracts::{RendererViewComposition, RendererViewTarget};
use render_presentation::{PresentationFrameDiff, PresentationOp, VideoProjectionOp};

use crate::{native_audio_diagnostic_code, AudioOutputSelection, CsharpProductRuntimeError};

/// The Engine's realization feedback admits this many facts per report.
const MAX_FACTS_PER_REPORT: usize = 128;

pub(crate) struct AudioOutput {
    realizer: Realizer,
    next_fact_id: u64,
    /// Whether world time is held and how fast it runs, as last followed.
    world: Option<(bool, f64)>,
    /// The Engine world time the device last caught up with.
    world_seconds: f64,
}

/// The one realizer, on a device or mixing for the stream.
enum Realizer {
    Device(AudioRealizer),
    Stream {
        realizer: AudioRealizer<StreamBackend>,
        stream: Arc<ProductHostAudioStream>,
    },
}

/// Forwards each call to whichever realizer plays.
macro_rules! forward {
    ($(fn $name:ident(&mut self $(, $arg:ident: $type:ty)*) $(-> $output:ty)?;)*) => {
        $(fn $name(&mut self $(, $arg: $type)*) $(-> $output)? {
            match self {
                Realizer::Device(realizer) => realizer.$name($($arg),*),
                Realizer::Stream { realizer, .. } => realizer.$name($($arg),*),
            }
        })*
    };
}

impl Realizer {
    forward! {
        fn refresh(&mut self, entities: &impl AudioEntityPositions);
        fn reset(&mut self);
        fn stop_all(&mut self);
        fn stop_soundtrack(&mut self);
        fn play_soundtrack(&mut self, clip: &[u8]) -> Result<(), String>;
        fn set_listener_pose(&mut self, position: [f32; 3], forward: [f32; 3], up: [f32; 3]);
        fn set_suspended(&mut self, suspended: bool);
        fn set_world_rate(&mut self, rate: f64);
        fn advance_held(&mut self, world_seconds: f64);
        fn take_facts(&mut self) -> Vec<RealizedAudioFact>;
        fn retain_clips(&mut self, admitted: impl Fn(&str) -> bool);
    }

    fn apply(
        &mut self,
        ops: &[PresentationOp],
        clips: &impl AudioClipSource,
        entities: &impl AudioEntityPositions,
    ) {
        match self {
            Realizer::Device(realizer) => realizer.apply(ops, clips, entities),
            Realizer::Stream { realizer, .. } => realizer.apply(ops, clips, entities),
        }
    }
}

#[cfg(test)]
impl AudioOutput {
    fn voice_cursor(&self, handle: render_presentation::AudioHandle) -> Option<f64> {
        match &self.realizer {
            Realizer::Device(realizer) => realizer.voice_cursor(handle),
            Realizer::Stream { realizer, .. } => realizer.voice_cursor(handle),
        }
    }

    fn take_facts(&mut self) -> Vec<RealizedAudioFact> {
        self.realizer.take_facts()
    }
}

impl AudioOutput {
    /// Opens the selected output. `None` when no device opens and the
    /// device was not `required` (the manifest's `audio.output`).
    pub(crate) fn open(
        selection: AudioOutputSelection,
    ) -> Result<Option<Self>, CsharpProductRuntimeError> {
        let realizer = match selection {
            AudioOutputSelection::Stream => {
                let stream = ProductHostAudioStream::new();
                let sink = Arc::clone(&stream);
                let realizer = AudioRealizer::open_stream(
                    PRODUCT_HOST_AUDIO_SAMPLE_RATE,
                    Arc::new(move |block: &[f32]| sink.publish(block)),
                )
                .map_err(|message| {
                    CsharpProductRuntimeError::new("CSHARP_AUDIO_OUTPUT", message)
                })?;
                Realizer::Stream { realizer, stream }
            }
            AudioOutputSelection::Device { required } => {
                match AudioRealizer::open_default_device() {
                    Ok(realizer) => Realizer::Device(realizer),
                    Err(message) if required => {
                        return Err(CsharpProductRuntimeError::new(
                            "CSHARP_AUDIO_OUTPUT",
                            message,
                        ))
                    }
                    Err(message) => {
                        eprintln!("rusty: {message}; the product runs silent");
                        return Ok(None);
                    }
                }
            }
        };
        Ok(Some(Self {
            realizer,
            next_fact_id: 1,
            world: None,
            world_seconds: 0.0,
        }))
    }

    /// The mixed audio for the watching pages, when it streams.
    pub(crate) fn stream(&self) -> Option<Arc<ProductHostAudioStream>> {
        match &self.realizer {
            Realizer::Device(_) => None,
            Realizer::Stream { stream, .. } => Some(Arc::clone(stream)),
        }
    }

    /// Plays a committed call's audio ops and removes them from its
    /// publications, then follows the call's camera and entity changes.
    pub(crate) fn realize(
        &mut self,
        services: &EngineServiceSet,
        outputs: &mut [RuntimePublication],
    ) {
        // World time this call admitted while the device held (a playtest
        // inspection advance) moves the held sounds on, before the call's own
        // new sounds start at its end.
        let world_seconds = services.presentation_elapsed_seconds();
        let advanced = world_seconds - std::mem::replace(&mut self.world_seconds, world_seconds);
        if advanced > 0.0 && self.world.is_some_and(|(held, _)| held) {
            self.realizer.advance_held(advanced);
        }
        for output in outputs.iter() {
            if let RuntimePublication::ViewComposition(composition) = output {
                self.follow_camera(composition);
            }
        }
        for output in outputs.iter() {
            if let RuntimePublication::Presentation(frame) = output {
                self.follow_video(services, frame);
            }
        }
        let entities = EngineEntities(services);
        let ops = take_audio_ops(outputs);
        if !ops.is_empty() {
            self.realizer.apply(
                &ops,
                &|hash: &str| services.audio_clip_bytes(hash),
                &entities,
            );
        }
        self.realizer.refresh(&entities);
    }

    /// Replaces the realization with the committed baseline after the
    /// realization owner was reset. Voices resume at their Engine cursors;
    /// earlier one-shots are not replayed.
    pub(crate) fn rebaseline(
        &mut self,
        services: &EngineServiceSet,
    ) -> Result<(), CsharpProductRuntimeError> {
        let baseline = services.audio_snapshot_frame()?;
        self.world_seconds = services.presentation_elapsed_seconds();
        self.follow_camera(&services.view_composition()?);
        self.realizer.reset();
        // A fresh realization plays an active clip from its start, as the
        // renderer shows it.
        self.realizer.stop_soundtrack();
        self.follow_video(services, &services.video_snapshot_frame());
        self.realizer.apply(
            &baseline.ops,
            &|hash: &str| services.audio_clip_bytes(hash),
            &EngineEntities(services),
        );
        Ok(())
    }

    /// Reports what the device finished or failed since the last call, and
    /// releases decoded clips the Engine no longer owns. Call between
    /// product calls.
    pub(crate) fn report(
        &mut self,
        services: &mut EngineServiceSet,
    ) -> Result<(), CsharpProductRuntimeError> {
        self.realizer.refresh(&EngineEntities(services));
        self.realizer
            .retain_clips(|hash| services.audio_clip_bytes(hash).is_some());
        let facts = self.realizer.take_facts();
        for chunk in facts.chunks(MAX_FACTS_PER_REPORT) {
            let facts: Vec<_> = chunk
                .iter()
                .map(|fact| self.engine_fact(fact.clone()))
                .collect();
            services.ingest_audio_realization_feedback(false, 0, facts)?;
        }
        Ok(())
    }

    /// Starts or ends the playing clip's sound with its video ops.
    fn follow_video(&mut self, services: &EngineServiceSet, frame: &PresentationFrameDiff) {
        for op in &frame.ops {
            let PresentationOp::Video { op, .. } = op else {
                continue;
            };
            match op {
                VideoProjectionOp::Play { clip, .. } => {
                    // A clip the renderer cannot read fails there, as a
                    // playback; its sound is simply absent here.
                    match services.borrowed_renderer_resource(&clip.asset) {
                        Some(resource) => {
                            let _ = self.realizer.play_soundtrack(resource.bytes());
                        }
                        None => self.realizer.stop_soundtrack(),
                    }
                }
                VideoProjectionOp::Stop { .. } | VideoProjectionOp::Skip { .. } => {
                    self.realizer.stop_soundtrack();
                }
            }
        }
    }

    /// Moves the listener to the primary view's camera. A composition with
    /// no primary view leaves the listener where it was.
    fn follow_camera(&mut self, composition: &RendererViewComposition) {
        if let Some((position, forward, up)) = primary_listener(composition) {
            self.realizer.set_listener_pose(position, forward, up);
        }
    }

    /// Plays world audio only while world time moves, at its rate: the
    /// Engine's audio cursors advance with admitted steps, so the device
    /// holds while the world does and slows with it.
    pub(crate) fn follow_world(&mut self, held: bool, rate: f64) {
        if self.world == Some((held, rate)) {
            return;
        }
        self.world = Some((held, rate));
        self.realizer.set_suspended(held);
        self.realizer.set_world_rate(rate);
    }

    pub(crate) fn silence(&mut self) {
        self.realizer.stop_all();
    }

    fn engine_fact(&mut self, fact: RealizedAudioFact) -> AudioRealizationFact {
        let fact_id = self.next_fact_id;
        self.next_fact_id += 1;
        match fact {
            RealizedAudioFact::OneShotCompleted {
                sequence,
                signal_handle,
            } => AudioRealizationFact::NaturalCompletionOneShot {
                fact_id,
                sequence,
                signal_handle: signal_handle.raw(),
            },
            RealizedAudioFact::VoiceCompleted { sequence, handle } => {
                AudioRealizationFact::NaturalCompletionRetainedVoice {
                    fact_id,
                    sequence,
                    voice_handle: handle.raw(),
                }
            }
            RealizedAudioFact::Diagnostic {
                diagnostic,
                signal_handle,
            } => AudioRealizationFact::Diagnostic {
                fact_id,
                code: native_audio_diagnostic_code(diagnostic.code),
                sequence: diagnostic.sequence,
                signal_handle: signal_handle.map(|handle| handle.raw()),
                voice_handle: diagnostic.handle.map(|handle| handle.raw()),
            },
        }
    }
}

struct EngineEntities<'a>(&'a EngineServiceSet);

impl AudioEntityPositions for EngineEntities<'_> {
    fn entity_position(&self, entity: u64) -> Option<[f32; 3]> {
        self.0.entity_world_position(entity)
    }
}

type ListenerPose = ([f32; 3], [f32; 3], [f32; 3]);

/// Position, forward and up of the camera shown by the lowest-ordered primary
/// view. Without an explicit basis, yaw 0 faces -Z, positive yaw turns right
/// and positive pitch looks up, as the renderer draws the camera.
fn primary_listener(composition: &RendererViewComposition) -> Option<ListenerPose> {
    let view = composition
        .views
        .iter()
        .filter(|view| matches!(view.target, RendererViewTarget::Primary))
        .min_by(|left, right| left.order.cmp(&right.order).then(left.id.cmp(&right.id)))?;
    let camera = composition
        .cameras
        .iter()
        .find(|camera| camera.id == view.camera_id)?;
    let position = camera.pose.position.map(|value| value as f32);
    if let Some(basis) = camera.basis {
        return Some((
            position,
            basis.forward.map(|value| value as f32),
            basis.up.map(|value| value as f32),
        ));
    }
    let (yaw, pitch) = (
        camera.pose.yaw_degrees.to_radians(),
        camera.pose.pitch_degrees.to_radians(),
    );
    let forward = [
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ];
    let up = [
        -yaw.sin() * pitch.sin(),
        pitch.cos(),
        yaw.cos() * pitch.sin(),
    ];
    Some((
        position,
        forward.map(|value| value as f32),
        up.map(|value| value as f32),
    ))
}

/// Removes audio ops from presentation publications, in order. A frame's
/// publication stamp counts its own ops, so the count follows the removal;
/// its revisions are unchanged.
pub(crate) fn take_audio_ops(outputs: &mut [RuntimePublication]) -> Vec<PresentationOp> {
    let mut taken = Vec::new();
    for output in outputs {
        let RuntimePublication::Presentation(frame) = output else {
            continue;
        };
        if !frame
            .ops
            .iter()
            .any(|op| matches!(op, PresentationOp::Audio { .. }))
        {
            continue;
        }
        let (audio, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut frame.ops)
            .into_iter()
            .partition(|op| matches!(op, PresentationOp::Audio { .. }));
        taken.extend(audio);
        frame.ops = rest;
        recount(frame);
    }
    taken
}

fn recount(frame: &mut PresentationFrameDiff) {
    if let Some(publication) = frame.publication.as_mut() {
        publication.operation_count = u32::try_from(frame.ops.len())
            .expect("a frame that fit its count still fits after removal");
    }
}

#[cfg(test)]
mod tests {
    use render_presentation::{
        AudioBus, AudioBusControl, AudioProjectionOp, PresentationOpMeta, VideoPlaybackHandle,
    };

    use super::*;
    use csharp_engine_abi::*;
    use render_audio::RealizedAudioFact;
    use render_presentation::AudioHandle;

    const TONE: &[u8] = include_bytes!("../../../../fixtures/audio-containers/tone.wav");
    const OK: i32 = 1;

    fn utf8(text: &'static str) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: text.as_ptr(),
            len: text.len(),
        }
    }

    fn source(clip: NativeAudioClipHandle, looping: bool) -> NativeAudioSourceDescriptor {
        NativeAudioSourceDescriptor {
            clip,
            bus: NativeAudioBus::Sfx,
            volume: 1.0,
            pitch: 1.0,
            looping,
            spatial_blend: 0.0,
            max_distance: 1.0,
            rolloff: NativeAudioRolloff::Linear,
            pan: 0.0,
            emitter_kind: NativeAudioEmitterKind::Global2d,
            position: NativeVec3::default(),
            entity: 0,
            offset: NativeVec3::default(),
        }
    }

    fn one_step(services: &mut EngineServiceSet, step: u64) {
        services.begin_update_call(
            runtime_ui::RuntimeUiRuntimeBinding::new(
                runtime_lifecycle::RuntimeInstanceId::new(1),
                runtime_lifecycle::RuntimeGeneration::ZERO,
                runtime_lifecycle::RuntimeControlRevision::ZERO,
            ),
            NativeProductUpdateFacts {
                mode: NativeProductUpdateMode::Realtime,
                lifecycle_state: NativeProductLifecycleState::Running,
                generation: 1,
                control_revision: 1,
                observed_host_time_nanoseconds: 0,
                simulation_step: step,
                fixed_step_hz: 60,
                admitted_step_count: 1,
                dropped_step_count: 0,
                fixed_delta_seconds: 1.0 / 60.0,
                gameplay_time_selected: false,
                gameplay_rate: 1.0,
                gameplay_advance_remaining_steps: 0,
                host_elapsed_seconds: 0.0,
            },
        );
        services.finish_call().expect("update call");
    }

    #[test]
    fn an_inspection_advance_moves_held_device_audio_to_the_world_moment() {
        let mut services = EngineServiceSet::new(
            csharp_engine_services::parse_runtime_appearance_catalog(None).unwrap(),
            [("tone.wav".to_owned(), std::sync::Arc::<[u8]>::from(TONE))].into(),
            None,
            product_host::ProductHostLog::new(Default::default())
                .unwrap()
                .handle(),
        )
        .unwrap();
        let mut output = AudioOutput::open(AudioOutputSelection::Stream)
            .unwrap()
            .expect("stream output");
        // The product starts a looping voice and a one-second one-shot.
        services.begin_call(runtime_ui::RuntimeUiRuntimeBinding::new(
            runtime_lifecycle::RuntimeInstanceId::new(1),
            runtime_lifecycle::RuntimeGeneration::ZERO,
            runtime_lifecycle::RuntimeControlRevision::ZERO,
        ));
        let api = services.api().audio;
        let mut clip = NativeAudioClipHandle::default();
        let mut voice = NativeAudioVoiceHandle::default();
        let mut signal = NativeAudioSignalHandle::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        unsafe {
            let request = NativeAudioClipRequest {
                path: utf8("tone.wav"),
            };
            assert_eq!(
                (api.open_clip)(api.context, &request, &mut clip, &mut error),
                OK
            );
            let looped = source(clip, true);
            assert_eq!(
                (api.create_voice)(api.context, &looped, &mut voice, &mut error),
                OK
            );
            let emit = NativeAudioEmitRequest {
                signal_id: utf8("shot"),
                descriptor: source(clip, false),
            };
            assert_eq!((api.emit)(api.context, &emit, &mut signal, &mut error), OK);
        }
        let mut call = services.finish_call().expect("product call");
        let mut outputs = crate::service_outputs(call.take_output()).expect("outputs");
        output.realize(&services, &mut outputs);
        // Playtest inspection holds the world: the device holds too.
        output.follow_world(true, 1.0);
        let held = output
            .voice_cursor(AudioHandle::new(voice.value))
            .expect("voice");
        // A 0.25 s inspection advance: fifteen admitted steps.
        for step in 0..15 {
            one_step(&mut services, step);
            output.realize(&services, &mut []);
        }
        let moved = output
            .voice_cursor(AudioHandle::new(voice.value))
            .expect("voice");
        assert!(
            (moved - held - 0.25).abs() < 0.02,
            "the voice stands at the world moment: {held} -> {moved}"
        );
        // A further second carries the one-shot past its end: it completes
        // at that world moment, while the world is still held.
        for step in 15..75 {
            one_step(&mut services, step);
            output.realize(&services, &mut []);
        }
        assert!(output.take_facts().iter().any(|fact| matches!(
            fact,
            RealizedAudioFact::OneShotCompleted { signal_handle, .. }
                if signal_handle.raw() == signal.value
        )));
    }

    fn composition(views: serde_json::Value) -> RendererViewComposition {
        let camera = |id: &str, yaw: f64, pitch: f64| {
            serde_json::json!({
                "id": id,
                "pose": { "position": [1.0, 2.0, 3.0], "pitchDegrees": pitch, "yawDegrees": yaw },
                "projection": { "kind": "perspective", "fovYDegrees": 60.0, "near": 0.1, "far": 100.0 }
            })
        };
        serde_json::from_value(serde_json::json!({
            "cameras": [camera("side", 90.0, 0.0), camera("down", 0.0, -90.0)],
            "targets": [],
            "views": views,
            "presentations": []
        }))
        .expect("composition")
    }

    fn view(id: &str, camera: &str, order: u64, target: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "cameraId": camera,
            "target": target,
            "viewport": { "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0 },
            "order": order
        })
    }

    fn close(actual: [f32; 3], expected: [f32; 3]) -> bool {
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| (actual - expected).abs() < 1e-5)
    }

    #[test]
    fn the_listener_is_the_lowest_ordered_primary_view_camera() {
        let primary = serde_json::json!({ "kind": "primary" });
        let offscreen =
            serde_json::json!({ "kind": "offscreen", "targetId": "minimap", "targetRevision": 1 });
        let (position, forward, up) = primary_listener(&composition(serde_json::json!([
            view("map", "down", 0, offscreen),
            view("late", "down", 5, primary.clone()),
            view("main", "side", 1, primary),
        ])))
        .expect("primary view");
        assert_eq!(position, [1.0, 2.0, 3.0]);
        // Yaw 90 turns right from -Z to +X.
        assert!(close(forward, [1.0, 0.0, 0.0]), "{forward:?}");
        assert!(close(up, [0.0, 1.0, 0.0]), "{up:?}");

        let (_, forward, up) = primary_listener(&composition(serde_json::json!([view(
            "main",
            "down",
            0,
            serde_json::json!({ "kind": "primary" })
        )])))
        .expect("primary view");
        // Pitch -90 looks straight down with up along -Z.
        assert!(close(forward, [0.0, -1.0, 0.0]), "{forward:?}");
        assert!(close(up, [0.0, 0.0, -1.0]), "{up:?}");

        assert!(primary_listener(&composition(serde_json::json!([]))).is_none());
    }

    #[test]
    fn audio_ops_leave_the_publication_and_its_count_follows() {
        let audio = PresentationOp::Audio {
            meta: PresentationOpMeta::new(1),
            op: AudioProjectionOp::BusControl {
                bus: AudioBus::Sfx,
                control: AudioBusControl::SetMuted { muted: true },
            },
        };
        let other = PresentationOp::Video {
            meta: PresentationOpMeta::new(2),
            op: VideoProjectionOp::Stop {
                handle: VideoPlaybackHandle::new(1),
            },
        };
        let mut frame = PresentationFrameDiff::new();
        frame.ops = vec![audio.clone(), other.clone()];
        frame.publication = Some(render_model::RenderFramePublication {
            stream: "presentation-world".to_owned(),
            base_revision: 4,
            revision: 5,
            operation_count: 2,
        });
        let mut outputs = vec![RuntimePublication::Presentation(frame)];

        assert_eq!(take_audio_ops(&mut outputs), [audio]);
        let RuntimePublication::Presentation(frame) = &outputs[0] else {
            unreachable!();
        };
        assert_eq!(frame.ops, [other]);
        let publication = frame.publication.as_ref().expect("stamp kept");
        assert_eq!((publication.revision, publication.operation_count), (5, 1));
    }
}
