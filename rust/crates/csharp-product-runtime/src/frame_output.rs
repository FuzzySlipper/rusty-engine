//! The world rendered in this process: streamed to the browser shell, or
//! presented to the desktop shell's window.
//!
//! `RUSTY_RENDER_OUTPUT` selects it; `stream` is the default. With `stream`, a
//! `render-wgpu` renderer starts on a headless device when the runtime loads,
//! and the host serves the frames it draws at
//! `/__rusty/product/runtime/frames`. With `window`, the renderer is built on
//! the desktop shell's device
//! ([`crate::CsharpProductRuntimeConfig::with_window_gpu`]) and the shell
//! draws it to its window. Either way each committed call's renderer
//! publications are applied as they are committed. The browser shell never
//! receives them: it shows the frames, or lets the window show through, under
//! the product UI. Video plays in these frames, above the UI.
//!
//! Animation, video and ghost plate facts reach the Engine from this
//! renderer, drawn or not: an unwatched stream advances clips and video on
//! Engine time without drawing (#8871). `RUSTY_RENDER_STREAM_FORMAT=rgba` sends raw frames instead of
//! JPEG, to measure what the encoder saves.
//!
//! Playtest inspection reaches the streamed renderer through Engine debug
//! commands (the window takes only the observer camera):
//! `engine.renderer.camera` (the observer camera), `engine.renderer.drawing`
//! (continuous or on-demand) and `engine.renderer.frame` (draw one frame now
//! and wait for it). `engine.renderer.presentation` describes the last drawn
//! frame. Each answer names the frame's sequence on the frame route, so a
//! page can wait until it shows that frame.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use csharp_engine_abi::{
    NativeGhostPlateFallbackReason, NativeGhostPlateLimitationMask, NativeVideoFailureCode,
};
use csharp_engine_services::{
    AnimationRealizationFact, EngineServiceSet, GhostPlateRealizationFact, VideoRealizationFact,
};
use product_dev_host::{
    ProductDevDrawingMode, ProductDevDrawnFrame, ProductDevFrameStream,
    ProductDevRendererInspection, ProductDevRendererStatistics, ProductDevStreamMedians,
    ProductDevStreamStatistics,
};
use render_host_contracts::RendererViewTarget;
use render_stream::{
    AnimationFact, DrawnFrame, FrameStreamer, Gpu, RendererCameraPose, RendererOptions,
    RendererViewComposition, ResourceSource, SceneDriver, SceneState, StreamFormat, StreamStats,
    VideoFact, VideoFailure,
};
use runtime_publication::RuntimePublication;
use serde_json::{json, Value};

use crate::{CsharpProductRuntimeError, RenderOutput};

pub(crate) const RENDER_OUTPUT_ENV: &str = "RUSTY_RENDER_OUTPUT";
const STREAM_FORMAT_ENV: &str = "RUSTY_RENDER_STREAM_FORMAT";
/// The Engine's realization feedback admits this many facts per report.
const MAX_FACTS_PER_REPORT: usize = 128;
/// How long an inspection command waits for the frame it asked for.
const INSPECTION_FRAME_WAIT: Duration = Duration::from_secs(2);
/// How often the renderer statistics C# reads are refreshed.
const STATISTICS_INTERVAL: Duration = Duration::from_secs(1);

/// Whether `command` is one of the runtime renderer's inspection commands.
pub(crate) fn is_inspection_command(command: &str) -> bool {
    matches!(
        command.split_whitespace().next(),
        Some("engine.renderer.camera" | "engine.renderer.drawing" | "engine.renderer.frame")
    )
}

/// `RUSTY_RENDER_OUTPUT`: `stream` when unset. An unknown value is an error
/// rather than a silent fallback.
pub(crate) fn render_output_mode() -> Result<RenderOutput, CsharpProductRuntimeError> {
    let Some(value) = std::env::var_os(RENDER_OUTPUT_ENV) else {
        return Ok(RenderOutput::Stream);
    };
    [RenderOutput::Stream, RenderOutput::Window]
        .into_iter()
        .find(|output| value == output.as_str())
        .ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_RENDER_OUTPUT",
                format!("{RENDER_OUTPUT_ENV} must be `stream` or `window` when set"),
            )
        })
}

pub(crate) struct FrameOutput {
    driver: Arc<SceneDriver>,
    /// The stream's render thread and frame route, when frames are streamed.
    stream: Option<(FrameStreamer, Arc<ProductDevFrameStream>)>,
    next_fact_id: u64,
    next_video_fact_id: u64,
    /// When the renderer statistics C# reads (`Diagnostics.ReadRenderer`)
    /// were last refreshed.
    statistics_reported: Option<Instant>,
}

impl FrameOutput {
    pub(crate) fn start(
        output: RenderOutput,
        options: RendererOptions,
        window_gpu: Option<&Gpu>,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let error =
            |message: String| CsharpProductRuntimeError::new("CSHARP_RENDER_OUTPUT", message);
        let (driver, stream) = match output {
            RenderOutput::Window => {
                let gpu = window_gpu.ok_or_else(|| {
                    error(format!(
                        "{RENDER_OUTPUT_ENV}=window needs the desktop shell (a runtime built with the `desktop` feature)"
                    ))
                })?;
                (SceneDriver::new(gpu.clone(), options), None)
            }
            RenderOutput::Stream => {
                let format = match std::env::var(STREAM_FORMAT_ENV).as_deref() {
                    Err(_) | Ok("jpeg") => StreamFormat::Jpeg,
                    Ok("rgba") => StreamFormat::Rgba8,
                    Ok(_) => {
                        return Err(error(format!(
                            "{STREAM_FORMAT_ENV} must be `jpeg` or `rgba` when set"
                        )))
                    }
                };
                let gpu = Gpu::headless().map_err(|gpu| error(gpu.to_string()))?;
                let driver = SceneDriver::new(gpu, options);
                let frames = ProductDevFrameStream::new();
                let streamer =
                    FrameStreamer::start(Arc::clone(&driver), format, Arc::clone(&frames))
                        .map_err(error)?;
                (driver, Some((streamer, frames)))
            }
        };
        Ok(Self {
            driver,
            stream,
            next_fact_id: 1,
            next_video_fact_id: 1,
            statistics_reported: None,
        })
    }

    /// The renderer the desktop shell draws.
    pub(crate) fn driver(&self) -> Arc<SceneDriver> {
        Arc::clone(&self.driver)
    }

    pub(crate) fn frames(&self) -> Option<Arc<ProductDevFrameStream>> {
        self.stream.as_ref().map(|(_, frames)| Arc::clone(frames))
    }

    /// Applies a committed call's renderer publications, with the simulation
    /// step and held state the call left.
    pub(crate) fn realize(
        &self,
        services: &EngineServiceSet,
        outputs: &[RuntimePublication],
        simulation: Simulation,
    ) {
        self.driver.apply(
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
        self.driver.rebaseline(
            baseline,
            &EngineResources(services),
            &|entity| services.entity_world_position(entity),
            scene_state(services, simulation),
        );
    }

    /// Follows a lifecycle or time-mode change that no product call
    /// published.
    pub(crate) fn follow_simulation(&self, simulation: Simulation) {
        self.driver.set_simulation(simulation.held, simulation.step);
    }

    /// Reports what the renderer observed since the last call, drawn or not.
    /// Call between product calls.
    pub(crate) fn report(&mut self, services: &mut EngineServiceSet) {
        let facts = self.driver.take_animation_facts();
        for chunk in facts.chunks(MAX_FACTS_PER_REPORT) {
            let facts: Vec<_> = chunk
                .iter()
                .map(|fact| self.engine_fact(fact.clone()))
                .collect();
            services.ingest_animation_realization_feedback(false, 0, facts);
        }
        let facts = self.driver.take_video_facts();
        for chunk in facts.chunks(MAX_FACTS_PER_REPORT) {
            let facts: Vec<_> = chunk.iter().map(|fact| self.video_fact(*fact)).collect();
            // The Engine rejects no fact this renderer produces; one it did
            // would be the Engine's defect, not the product's.
            let _ = services.ingest_video_realization_feedback(false, 0, facts);
        }
        // A complete latest snapshot; it replaces the Engine's observation.
        let plates = self.driver.ghost_plate_readouts();
        services
            .ingest_ghost_plate_realization_feedback(false, plates.iter().map(ghost_plate_fact));
        if self
            .statistics_reported
            .is_none_or(|reported| reported.elapsed() >= STATISTICS_INTERVAL)
        {
            self.statistics_reported = Some(Instant::now());
            // The statistics are plain values, so they always encode.
            let _ = services.ingest_renderer_diagnostics(
                &serde_json::to_value(self.statistics()).unwrap_or_default(),
            );
        }
    }

    fn video_fact(&mut self, fact: VideoFact) -> VideoRealizationFact {
        let fact_id = self.next_video_fact_id;
        self.next_video_fact_id += 1;
        match fact {
            VideoFact::Completed { handle } => VideoRealizationFact::Completed {
                fact_id,
                handle: handle.raw(),
            },
            VideoFact::Skipped { handle } => VideoRealizationFact::Skipped {
                fact_id,
                handle: handle.raw(),
            },
            VideoFact::Failed { handle, failure } => VideoRealizationFact::Failed {
                fact_id,
                handle: handle.raw(),
                failure: match failure {
                    VideoFailure::DecodeFailed => NativeVideoFailureCode::DecodeFailed,
                    VideoFailure::HostFailure => NativeVideoFailureCode::HostFailure,
                },
            },
        }
    }

    /// Runs an inspection command (see [`is_inspection_command`]). A change
    /// draws a frame and waits for it; the answer carries that frame.
    pub(crate) fn execute_inspection(
        &self,
        command: &str,
    ) -> Result<ProductDevRendererInspection, String> {
        let Some((streamer, _)) = &self.stream else {
            return self.execute_window_inspection(command);
        };
        let words: Vec<&str> = command.split_whitespace().collect();
        let drawn = match words.as_slice() {
            ["engine.renderer.camera"] | ["engine.renderer.drawing"] => None,
            ["engine.renderer.camera", "none"] => {
                self.driver.set_observer(None);
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
                self.driver.set_observer(Some(RendererCameraPose {
                    position: [number(x)?, number(y)?, number(z)?],
                    yaw_degrees: number(yaw)?,
                    pitch_degrees: number(pitch)?,
                }));
                self.draw_now()?
            }
            ["engine.renderer.drawing", mode @ ("continuous" | "on-demand")] => {
                streamer.set_on_demand(*mode == "on-demand");
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
        let inspection = streamer.inspection();
        let camera = inspection.observer.or_else(|| {
            inspection
                .composition
                .as_deref()
                .and_then(primary_camera_pose)
        });
        let frame = drawn.or(inspection.last_drawn);
        Ok(ProductDevRendererInspection {
            drawing: if inspection.on_demand {
                ProductDevDrawingMode::OnDemand
            } else {
                ProductDevDrawingMode::Continuous
            },
            output: RenderOutput::Stream,
            held: inspection.held,
            observer: inspection.observer.is_some(),
            camera,
            frame: frame.map(|frame| ProductDevDrawnFrame {
                sequence: frame.sequence,
                step: frame.step,
            }),
        })
    }

    /// The desktop window draws every frame itself; only the observer
    /// camera applies to it.
    fn execute_window_inspection(
        &self,
        command: &str,
    ) -> Result<ProductDevRendererInspection, String> {
        let words: Vec<&str> = command.split_whitespace().collect();
        let pose = |values: &[&str]| -> Result<RendererCameraPose, String> {
            let number = |value: &str| {
                value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or("camera values must be finite numbers")
            };
            Ok(RendererCameraPose {
                position: [number(values[0])?, number(values[1])?, number(values[2])?],
                yaw_degrees: number(values[3])?,
                pitch_degrees: number(values[4])?,
            })
        };
        match words.as_slice() {
            ["engine.renderer.camera"] => {}
            ["engine.renderer.camera", "none"] => self.driver.set_observer(None),
            ["engine.renderer.camera", values @ ..] if values.len() == 5 => {
                self.driver.set_observer(Some(pose(values)?));
            }
            _ => {
                return Err("the desktop window draws every frame; it takes only \
                     engine.renderer.camera [none | x y z yawDegrees pitchDegrees]"
                    .to_owned())
            }
        }
        let (held, observer, composition) = self.driver.view_state();
        Ok(ProductDevRendererInspection {
            drawing: ProductDevDrawingMode::Continuous,
            output: RenderOutput::Window,
            held,
            observer: observer.is_some(),
            camera: observer.or_else(|| composition.as_deref().and_then(primary_camera_pose)),
            frame: None,
        })
    }

    fn draw_now(&self) -> Result<Option<DrawnFrame>, String> {
        let Some((streamer, _)) = &self.stream else {
            return Ok(None);
        };
        streamer
            .draw_now(INSPECTION_FRAME_WAIT)
            .map(Some)
            .ok_or_else(|| "the renderer did not draw the requested frame in time".to_owned())
    }

    /// The `engine.renderer.presentation` observation of the last drawn
    /// frame, in the browser surface's shape. Rendering and the runtime share
    /// one process, so the observation always belongs to `runtime`.
    pub(crate) fn presentation(&self, runtime: Value) -> Value {
        let inspection = self
            .stream
            .as_ref()
            .map(|(streamer, _)| streamer.inspection());
        let Some((inspection, frame)) = inspection.and_then(|inspection| {
            inspection
                .last_drawn
                .clone()
                .map(|frame| (inspection, frame))
        }) else {
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
        let frontiers = |revision: u64| json!([{ "stream": render_presentation::PRESENTATION_WORLD_STREAM, "revision": revision }]);
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
                "cameras": drawn_cameras(&composition.cameras, &frame.cameras),
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
                    "video": frame.video,
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

    /// The renderer's adapter and what its recent frames cost, for
    /// `engine.renderer.*` and `Diagnostics.ReadRenderer`.
    pub(crate) fn statistics(&self) -> ProductDevRendererStatistics {
        let Some((streamer, route)) = &self.stream else {
            let (skipped_ops, last_skip) = self.driver.skipped_ops();
            let adapter = self.driver.gpu().adapter_summary();
            return ProductDevRendererStatistics {
                adapter: format!("{} ({})", adapter.name, adapter.backend),
                output: RenderOutput::Window,
                stream: None,
                skipped_ops: skipped_op_counts(skipped_ops),
                last_skip,
            };
        };
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
        } = streamer.stats();
        ProductDevRendererStatistics {
            adapter,
            output: RenderOutput::Stream,
            stream: Some(ProductDevStreamStatistics {
                viewer_size: route.wanted_size(),
                recent_frames: frames,
                frames_per_second,
                median_ms: ProductDevStreamMedians {
                    render: render_ms,
                    readback: readback_ms,
                    encode: encode_ms,
                },
                median_bytes_per_frame: bytes_per_frame,
                bytes_per_second,
            }),
            skipped_ops: skipped_op_counts(skipped_ops),
            last_skip,
        }
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
fn skipped_op_counts(
    skipped: std::collections::BTreeMap<&'static str, u64>,
) -> std::collections::BTreeMap<String, u64> {
    skipped
        .into_iter()
        .map(|(op, count)| (op.to_owned(), count))
        .collect()
}

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

/// One realized ghost plate as the Engine's realization fact. render-wgpu
/// ports the Three lane's capture bank, so the retained-profile limits apply
/// unchanged (single capture view with one sector). The plate is built from
/// its own descriptor's captured source, so the source always matches. A plate
/// whose capture failed is not realized and has no fact; no realized plate
/// draws a stand-in, so there is no fallback. The whole CPU build (the plate's
/// isolated renderer and every sector's capture) is its capture submission
/// time. Its meshes and materials are the frozen source's parts and
/// materials; it borrows no textures.
fn ghost_plate_fact(plate: &render_wgpu::GhostPlateReadout) -> GhostPlateRealizationFact {
    GhostPlateRealizationFact {
        handle: plate.handle.raw(),
        source_matches: true,
        current_sector: plate.current_sector,
        local_angular_offset_degrees: Some(plate.local_azimuth_degrees),
        fallback_active: false,
        fallback_reason: NativeGhostPlateFallbackReason::None,
        limitation_mask: if plate.sector_count == 1 {
            NativeGhostPlateLimitationMask::SingleCaptureViewProfile
        } else {
            NativeGhostPlateLimitationMask::DirectionalCaptureBankProfile
        },
        preparation_cpu_milliseconds: None,
        capture_cpu_submission_milliseconds: Some(plate.capture_milliseconds),
        retained_sector_count: plate.sector_count,
        retained_mesh_count: plate.parts,
        retained_material_count: plate.materials,
        retained_borrowed_texture_count: 0,
    }
}

/// The submitted cameras: each composition camera with the pose and basis it
/// drew from in this frame (motion sampled, or the observer's where the
/// observer replaced it in primary views). `observer` marks the latter, and
/// `offscreenPose` keeps the sampled pose its offscreen views still drew.
/// The authored descriptors stay in `sourceCameras`.
fn drawn_cameras(
    descriptors: &[render_host_contracts::RendererCompositionCamera],
    drawn: &[render_wgpu::DrawnCamera],
) -> Vec<Value> {
    descriptors
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            let mut camera = json!(descriptor);
            if let (Some(drawn), Value::Object(fields)) = (drawn.get(index), &mut camera) {
                fields.insert("pose".to_owned(), json!(drawn.pose));
                fields.insert("basis".to_owned(), json!(drawn.basis));
                fields.insert("observer".to_owned(), json!(drawn.observer));
                if let Some((pose, basis)) = &drawn.offscreen {
                    fields.insert("offscreenPose".to_owned(), json!(pose));
                    fields.insert("offscreenBasis".to_owned(), json!(basis));
                }
            }
            camera
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_host_contracts::{
        RendererCameraBasis, RendererCameraPose, RendererCameraProjection,
        RendererCompositionCamera,
    };

    #[test]
    fn a_drawn_ghost_plate_reports_its_sector_and_bank_profile() {
        let readout = |sector_count: u32| render_wgpu::GhostPlateReadout {
            handle: render_presentation::GhostPlateHandle::new(7),
            source: render_model::RenderHandle::new(41),
            sector_count,
            current_sector: sector_count - 1,
            local_azimuth_degrees: 271.5,
            capture_milliseconds: 3.25,
            parts: 2,
            materials: 1,
        };
        let fact = ghost_plate_fact(&readout(4));
        assert_eq!(fact.handle, 7);
        assert!(fact.source_matches);
        assert_eq!(fact.current_sector, 3, "the sector the last view drew");
        assert_eq!(fact.local_angular_offset_degrees, Some(271.5));
        assert!(!fact.fallback_active);
        assert_eq!(fact.fallback_reason, NativeGhostPlateFallbackReason::None);
        assert_eq!(
            fact.limitation_mask,
            NativeGhostPlateLimitationMask::DirectionalCaptureBankProfile
        );
        assert_eq!(fact.preparation_cpu_milliseconds, None);
        assert_eq!(fact.capture_cpu_submission_milliseconds, Some(3.25));
        assert_eq!(
            (
                fact.retained_sector_count,
                fact.retained_mesh_count,
                fact.retained_material_count,
                fact.retained_borrowed_texture_count
            ),
            (4, 2, 1, 0)
        );
        assert_eq!(
            ghost_plate_fact(&readout(1)).limitation_mask,
            NativeGhostPlateLimitationMask::SingleCaptureViewProfile
        );
    }

    #[test]
    fn submitted_cameras_carry_the_drawn_pose_and_keep_the_offscreen_one() {
        let pose = |x: f64, yaw: f64| RendererCameraPose {
            position: [x, 1.0, 2.0],
            pitch_degrees: 0.0,
            yaw_degrees: yaw,
        };
        let basis = RendererCameraBasis {
            forward: [0.0, 0.0, -1.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
        };
        let descriptor = RendererCompositionCamera {
            id: "player".to_owned(),
            pose: pose(0.0, 0.0),
            basis: None,
            projection: RendererCameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 100.0,
            },
            motion: None,
        };
        let drawn = render_wgpu::DrawnCamera {
            pose: pose(5.0, 30.0),
            basis,
            observer: true,
            offscreen: Some((pose(0.5, 0.0), basis)),
        };
        let cameras = drawn_cameras(std::slice::from_ref(&descriptor), &[drawn]);
        let camera = &cameras[0];
        assert_eq!(camera["id"], "player");
        assert_eq!(camera["pose"]["position"], json!([5.0, 1.0, 2.0]));
        assert_eq!(camera["pose"]["yawDegrees"], 30.0);
        assert_eq!(camera["observer"], true);
        assert_eq!(camera["offscreenPose"]["position"], json!([0.5, 1.0, 2.0]));
        assert_eq!(camera["projection"], json!(descriptor.projection));
        // A frame drawn before any camera report keeps the descriptor.
        let unreported = drawn_cameras(std::slice::from_ref(&descriptor), &[]);
        assert_eq!(unreported[0], json!(descriptor));
    }
}
