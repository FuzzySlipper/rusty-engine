//! Audio realized on this process's output device.
//!
//! `RUSTY_AUDIO_OUTPUT=device` opens the default output device when the
//! runtime loads and closes it when the runtime drops. Committed audio ops
//! then play here instead of in the browser: they are taken out of the
//! published presentation so no second realization plays them or reports
//! feedback for them. Natural completions and device diagnostics reach the
//! Engine through the same realization facts the browser reports.
//!
//! The listener follows the camera of the primary view in the committed view
//! composition, and entity-attached emitters follow the committed graphics
//! node published for their entity.
//!
//! When this process also draws video (the desktop window or the streamed
//! frames), a playing clip's own sound plays here too, from the clip's start,
//! as the browser's video element played it. The video ops stay in the
//! publications: the runtime's renderer draws the picture.

use csharp_engine_services::{AudioRealizationFact, EngineServiceSet};
use render_audio::{AudioEntityPositions, AudioRealizer, RealizedAudioFact};
use render_host_contracts::{RendererViewComposition, RendererViewTarget};
use render_presentation::{PresentationFrameDiff, PresentationOp, VideoProjectionOp};
use runtime_publication::RuntimePublication;

use crate::{native_audio_diagnostic_code, CsharpProductRuntimeError};

pub(crate) const AUDIO_OUTPUT_ENV: &str = "RUSTY_AUDIO_OUTPUT";
const AUDIO_OUTPUT_DEVICE: &str = "device";
/// The Engine's realization feedback admits this many facts per report.
const MAX_FACTS_PER_REPORT: usize = 128;

pub(crate) struct AudioOutput {
    realizer: AudioRealizer,
    next_fact_id: u64,
    /// This process draws video, so it plays the clips' sound as well.
    soundtracks: bool,
}

impl AudioOutput {
    /// The device path is opt-in until the desktop shell owns it. An unset
    /// variable keeps browser realization; an unknown value or a device
    /// that will not open is an error rather than silent fallback.
    pub(crate) fn from_environment(
        soundtracks: bool,
    ) -> Result<Option<Self>, CsharpProductRuntimeError> {
        let Some(value) = std::env::var_os(AUDIO_OUTPUT_ENV) else {
            return Ok(None);
        };
        if value != AUDIO_OUTPUT_DEVICE {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_AUDIO_OUTPUT",
                format!("{AUDIO_OUTPUT_ENV} must be `{AUDIO_OUTPUT_DEVICE}` when set"),
            ));
        }
        let realizer = AudioRealizer::open_default_device()
            .map_err(|message| CsharpProductRuntimeError::new("CSHARP_AUDIO_OUTPUT", message))?;
        Ok(Some(Self {
            realizer,
            next_fact_id: 1,
            soundtracks,
        }))
    }

    /// Plays a committed call's audio ops and removes them from its
    /// publications, then follows the call's camera and entity changes.
    pub(crate) fn realize(
        &mut self,
        services: &EngineServiceSet,
        outputs: &mut [RuntimePublication],
    ) {
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
        if !self.soundtracks {
            return;
        }
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

    pub(crate) fn set_suspended(&mut self, suspended: bool) {
        self.realizer.set_suspended(suspended);
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
        AudioBus, AudioBusControl, AudioProjectionOp, PresentationOpMeta, TelemetryOverlayHandle,
        TelemetryOverlayProjectionOp,
    };

    use super::*;

    fn composition(views: serde_json::Value) -> RendererViewComposition {
        let camera = |id: &str, yaw: f64, pitch: f64| {
            serde_json::json!({
                "id": id,
                "pose": { "position": [1.0, 2.0, 3.0], "pitchDegrees": pitch, "yawDegrees": yaw },
                "projection": { "kind": "perspective", "fovYDegrees": 60.0, "near": 0.1, "far": 100.0 }
            })
        };
        serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
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
        let other = PresentationOp::TelemetryOverlay {
            meta: PresentationOpMeta::new(2),
            op: TelemetryOverlayProjectionOp::Destroy {
                handle: TelemetryOverlayHandle::new(1),
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
