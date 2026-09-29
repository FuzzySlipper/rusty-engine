//! The world rendered in this process and streamed to the browser shell.
//!
//! `RUSTY_RENDER_OUTPUT=stream` starts a `render-wgpu` renderer when the
//! runtime loads. Each committed call's renderer publications are applied to
//! it as they are committed, and the host serves the frames it draws at
//! `/__rusty/product/runtime/frames`. The browser shell then shows those
//! frames under the product UI instead of realizing the world with Three.
//! The publications still reach the browser; its stream surface ignores the
//! world and realizes only audio and video.
//!
//! Animation facts reach the Engine from this renderer, through the same
//! realization feedback the browser reports.
//! `RUSTY_RENDER_STREAM_FORMAT=rgba` sends raw frames instead of JPEG, to
//! measure what the encoder saves.
//!
//! Playtest inspection reaches this renderer through Engine debug commands:
//! `engine.renderer.camera` (the observer camera), `engine.renderer.drawing`
//! (continuous or on-demand) and `engine.renderer.frame` (draw one frame now
//! and wait for it). `engine.renderer.presentation` describes the last drawn
//! frame. Each answer names the frame's sequence on the frame route, so a
//! page can wait until it shows that frame.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use csharp_engine_services::{AnimationRealizationFact, EngineServiceSet};
use product_dev_host::ProductDevFrameStream;
use render_host_contracts::RendererViewTarget;
use render_stream::{
    AnimationFact, DrawnFrame, FrameStreamer, RendererCameraPose, RendererOptions,
    RendererViewComposition, ResourceSource, SceneState, StreamFormat, StreamStats,
};
use serde_json::{json, Value};
use runtime_publication::RuntimePublication;

use crate::CsharpProductRuntimeError;

pub(crate) const RENDER_OUTPUT_ENV: &str = "RUSTY_RENDER_OUTPUT";
const RENDER_OUTPUT_STREAM: &str = "stream";
const STREAM_FORMAT_ENV: &str = "RUSTY_RENDER_STREAM_FORMAT";
/// The Engine's realization feedback admits this many facts per report.
const MAX_FACTS_PER_REPORT: usize = 128;
/// How long an inspection command waits for the frame it asked for.
const INSPECTION_FRAME_WAIT: Duration = Duration::from_secs(2);

/// Whether `command` is one of the runtime renderer's inspection commands.
pub(crate) fn is_inspection_command(command: &str) -> bool {
    matches!(
        command.split_whitespace().next(),
        Some("engine.renderer.camera" | "engine.renderer.drawing" | "engine.renderer.frame")
    )
}

/// Whether this process renders the world and streams it. An unset variable
/// keeps browser realization; an unknown value is an error rather than a
/// silent fallback.
pub(crate) fn stream_selected() -> Result<bool, CsharpProductRuntimeError> {
    match std::env::var_os(RENDER_OUTPUT_ENV) {
        None => Ok(false),
        Some(value) if value == RENDER_OUTPUT_STREAM => Ok(true),
        Some(_) => Err(CsharpProductRuntimeError::new(
            "CSHARP_RENDER_OUTPUT",
            format!("{RENDER_OUTPUT_ENV} must be `{RENDER_OUTPUT_STREAM}` when set"),
        )),
    }
}

pub(crate) struct FrameOutput {
    streamer: FrameStreamer,
    frames: Arc<ProductDevFrameStream>,
    next_fact_id: u64,
}

impl FrameOutput {
    pub(crate) fn from_environment(
        options: RendererOptions,
    ) -> Result<Option<Self>, CsharpProductRuntimeError> {
        if !stream_selected()? {
            return Ok(None);
        }
        let format = match std::env::var(STREAM_FORMAT_ENV).as_deref() {
            Err(_) | Ok("jpeg") => StreamFormat::Jpeg,
            Ok("rgba") => StreamFormat::Rgba8,
            Ok(_) => {
                return Err(CsharpProductRuntimeError::new(
                    "CSHARP_RENDER_OUTPUT",
                    format!("{STREAM_FORMAT_ENV} must be `jpeg` or `rgba` when set"),
                ))
            }
        };
        let frames = ProductDevFrameStream::new();
        let streamer = FrameStreamer::start(options, format, Arc::clone(&frames))
            .map_err(|message| CsharpProductRuntimeError::new("CSHARP_RENDER_OUTPUT", message))?;
        Ok(Some(Self {
            streamer,
            frames,
            next_fact_id: 1,
        }))
    }

    pub(crate) fn frames(&self) -> Arc<ProductDevFrameStream> {
        Arc::clone(&self.frames)
    }

    /// Applies a committed call's renderer publications, with the simulation
    /// step and held state the call left.
    pub(crate) fn realize(
        &self,
        services: &EngineServiceSet,
        outputs: &[RuntimePublication],
        simulation: Simulation,
    ) {
        self.streamer.apply(
            outputs,
            &EngineResources(services),
            &|entity| services.entity_world_position(entity),
            scene_state(services, simulation),
        );
    }

    /// Rebuilds the renderer from the committed world, after a call's
    /// renderer work was lost or the world was replaced.
    pub(crate) fn rebaseline(
        &self,
        services: &EngineServiceSet,
        baseline: &[RuntimePublication],
        simulation: Simulation,
    ) {
        self.streamer.rebaseline(
            baseline,
            &EngineResources(services),
            &|entity| services.entity_world_position(entity),
            scene_state(services, simulation),
        );
    }

    /// Follows a lifecycle or time-mode change that no product call
    /// published.
    pub(crate) fn follow_simulation(&self, simulation: Simulation) {
        self.streamer.set_simulation(simulation.held, simulation.step);
    }

    /// Reports what the drawn frames observed since the last call. Call
    /// between product calls.
    pub(crate) fn report(&mut self, services: &mut EngineServiceSet) {
        let facts = self.streamer.take_animation_facts();
        for chunk in facts.chunks(MAX_FACTS_PER_REPORT) {
            let facts: Vec<_> = chunk
                .iter()
                .map(|fact| self.engine_fact(fact.clone()))
                .collect();
            services.ingest_animation_realization_feedback(false, 0, facts);
        }
    }

    /// Runs an inspection command (see [`is_inspection_command`]). A change
    /// draws a frame and waits for it; the answer carries that frame.
    pub(crate) fn execute_inspection(&self, command: &str) -> Result<Value, String> {
        let words: Vec<&str> = command.split_whitespace().collect();
        let drawn = match words.as_slice() {
            ["engine.renderer.camera"] | ["engine.renderer.drawing"] => None,
            ["engine.renderer.camera", "none"] => {
                self.streamer.set_observer(None);
                self.draw_now()?
            }
            ["engine.renderer.camera", x, y, z, yaw, pitch] => {
                let number = |value: &str| {
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .ok_or("camera values must be finite numbers")
                };
                self.streamer.set_observer(Some(RendererCameraPose {
                    position: [number(x)?, number(y)?, number(z)?],
                    yaw_degrees: number(yaw)?,
                    pitch_degrees: number(pitch)?,
                }));
                self.draw_now()?
            }
            ["engine.renderer.drawing", mode @ ("continuous" | "on-demand")] => {
                self.streamer.set_on_demand(*mode == "on-demand");
                None
            }
            ["engine.renderer.frame"] => self.draw_now()?,
            _ => {
                return Err(
                    "usage: engine.renderer.camera [none | x y z yawDegrees pitchDegrees], \
                     engine.renderer.drawing [continuous | on-demand], engine.renderer.frame"
                        .to_owned(),
                )
            }
        };
        let inspection = self.streamer.inspection();
        let camera = inspection.observer.or_else(|| {
            inspection
                .composition
                .as_deref()
                .and_then(primary_camera_pose)
        });
        let frame = drawn.or(inspection.last_drawn);
        Ok(json!({
            "drawing": if inspection.on_demand { "on-demand" } else { "continuous" },
            "held": inspection.held,
            "observer": inspection.observer.is_some(),
            "camera": camera,
            "frame": frame.map(|frame| json!({ "sequence": frame.sequence, "step": frame.step })),
        }))
    }

    fn draw_now(&self) -> Result<Option<DrawnFrame>, String> {
        self.streamer
            .draw_now(INSPECTION_FRAME_WAIT)
            .map(Some)
            .ok_or_else(|| "the renderer did not draw the requested frame in time".to_owned())
    }

    /// The `engine.renderer.presentation` observation of the last drawn
    /// frame, in the browser surface's shape. Rendering and the runtime share
    /// one process, so the observation always belongs to `runtime`.
    pub(crate) fn presentation(&self, runtime: Value) -> Value {
        let inspection = self.streamer.inspection();
        let Some(frame) = inspection.last_drawn else {
            return json!({
                "schemaVersion": 1,
                "runtime": runtime,
                "observationRuntime": runtime,
                "available": false,
                "presentation": null,
                "captureCorrelation": "unavailable",
                "worldReadiness": "unavailable",
            });
        };
        let frontiers = |revision: u64| {
            json!([{ "stream": render_presentation::PRESENTATION_WORLD_STREAM, "revision": revision }])
        };
        let viewport = json!({
            "cssWidth": frame.width,
            "cssHeight": frame.height,
            "backingWidth": frame.width,
            "backingHeight": frame.height,
        });
        let views = frame.composition.as_deref().map(|composition| {
            json!({
                "schemaVersion": 1,
                "revision": frame.composition_revision,
                "cameras": composition.cameras,
                "sourceCameras": composition.cameras,
                "targets": composition.targets,
                "views": composition.views,
                "presentations": composition.presentations,
            })
        });
        json!({
            "schemaVersion": 1,
            "runtime": runtime,
            "observationRuntime": runtime,
            "available": true,
            "observationAgeMs": frame.drawn_at.elapsed().as_millis() as u64,
            "presentation": {
                "surfaceId": format!("runtime-stream-{}", frame.renderer_id),
                "state": if inspection.pending { "pending" } else { "submitted" },
                "pendingRealizations": 0,
                "realizedPublicationFrontiers": frontiers(frame.world_revision),
                "realizedViewRevision": frame.composition_revision,
                "submitted": {
                    "renderSequence": frame.sequence,
                    "frameSequence": frame.sequence,
                    "simulationStep": frame.step,
                    "held": frame.held,
                    "publicationFrontiers": frontiers(frame.world_revision),
                    "viewRevision": frame.composition_revision,
                    "viewport": viewport,
                    "fallbackCamera": null,
                    "observer": frame.observer,
                    "views": views,
                },
                "gpuCompletion": "unavailable",
                "captureCorrelation": "frame-sequence",
            },
            "captureCorrelation": "frame-sequence",
            "worldReadiness": "unavailable",
        })
    }

    /// What the recent streamed frames cost, for `engine.renderer.*`.
    pub(crate) fn stats_json(&self) -> serde_json::Value {
        let StreamStats {
            adapter,
            frames,
            frames_per_second,
            render_ms,
            readback_ms,
            encode_ms,
            bytes_per_frame,
            bytes_per_second,
            skipped_ops,
            last_skip,
        } = self.streamer.stats();
        serde_json::json!({
            "adapter": adapter,
            "viewerSize": self.frames.wanted_size(),
            "recentFrames": frames,
            "framesPerSecond": frames_per_second,
            "medianMs": { "render": render_ms, "readback": readback_ms, "encode": encode_ms },
            "medianBytesPerFrame": bytes_per_frame,
            "bytesPerSecond": bytes_per_second,
            "skippedOps": skipped_ops,
            "lastSkip": last_skip,
        })
    }

    fn engine_fact(&mut self, fact: AnimationFact) -> AnimationRealizationFact {
        let fact_id = self.next_fact_id;
        self.next_fact_id += 1;
        match fact {
            AnimationFact::NaturalCompletion {
                object_id,
                generation,
                clip,
            } => AnimationRealizationFact::NaturalCompletion {
                fact_id,
                object_id,
                generation,
                clip,
            },
            AnimationFact::MeshInspection {
                object_id,
                generation,
                request,
                bounds,
            } => AnimationRealizationFact::MeshInspection {
                fact_id,
                object_id,
                generation,
                request,
                bounds_min: bounds.map_or([0.0; 3], |(min, _)| min),
                bounds_max: bounds.map_or([0.0; 3], |(_, max)| max),
                has_bounds: bounds.is_some(),
                voxel_normal_meshes: 0,
            },
        }
    }
}

struct EngineResources<'a>(&'a EngineServiceSet);

impl ResourceSource for EngineResources<'_> {
    fn bytes(&self, identity: &str) -> Option<Cow<'_, [u8]>> {
        self.0
            .borrowed_renderer_resource(identity)
            .map(|resource| Cow::Borrowed(resource.bytes()))
    }
}

/// The simulation step a frame shows, and whether time is held.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Simulation {
    pub held: bool,
    pub step: u64,
}

fn scene_state(services: &EngineServiceSet, simulation: Simulation) -> SceneState {
    SceneState {
        elapsed_seconds: services.presentation_elapsed_seconds(),
        world_revision: services
            .renderer_publication_frontiers()
            .first()
            .map_or(0, |(_, revision)| *revision),
        step: simulation.step,
        held: simulation.held,
    }
}

/// The camera pose of the lowest-ordered primary view.
fn primary_camera_pose(composition: &RendererViewComposition) -> Option<RendererCameraPose> {
    let view = composition
        .views
        .iter()
        .filter(|view| matches!(view.target, RendererViewTarget::Primary))
        .min_by(|left, right| left.order.cmp(&right.order).then(left.id.cmp(&right.id)))?;
    composition
        .cameras
        .iter()
        .find(|camera| camera.id == view.camera_id)
        .map(|camera| camera.pose)
}
