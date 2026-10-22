//! Camera composition: the committed `RendererViewComposition` the C#
//! `CameraView` service publishes (`RuntimePublication::ViewComposition`),
//! realized as view passes.
//!
//! Each frame, in order:
//! 1. offscreen views, by `(order, id)`, into their targets. A target is drawn
//!    only when it is stale: never drawn, a new composition, a delta applied
//!    since, an animated pose that changed on Engine time, or a view camera
//!    that moved;
//! 2. primary steps (views and presentations), by `(order, id)`. Each primary
//!    view clears its viewport and draws the world once, then the viewmodel
//!    layer after a depth break. A presentation draws an offscreen target
//!    into its destination viewport.
//!
//! There is no fallback world pass: primary area no view covers keeps the
//! environment clear. Camera poses come from the composition; motion
//! interpolates between its samples on the presentation time the host passes.
//! An observer pose, while the host sets one, replaces every primary view's
//! camera pose (each keeps its own projection); offscreen views keep theirs.
//! The backend neither validates nor sequences compositions: the runtime
//! validated this one, a target reallocates when its descriptor or revision
//! changes, and a view naming a missing camera or target draws nothing.

use std::collections::HashMap;

use render_host_contracts::{
    RendererCameraBasis, RendererCameraPose, RendererCompositionCamera, RendererCompositionTarget,
    RendererTargetSampling, RendererViewComposition, RendererViewTarget, RendererViewport,
    RendererViewportAnchors,
};

use crate::camera::{self, CameraMatrices, CameraMotion, CameraPose, CameraSampleReadout};
use crate::frame::{PassStart, PixelRect, ViewLayer, ViewPass, ViewStats};
use crate::labels::LabelPass;
use crate::target::{self, RenderPicture, TargetView, OFFSCREEN_FORMAT};
use crate::upscale::{FrameResolution, UpscaleMethod};
use crate::{FrameStats, OffscreenTarget, Renderer, SurfaceFrame, WindowSurface};

struct CompositionTarget {
    descriptor: RendererCompositionTarget,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    present: wgpu::BindGroup,
    /// Set by a new composition; cleared when the target is drawn.
    stale: bool,
    /// Scene generation and view cameras the target was last drawn with.
    drawn: Option<(u64, Vec<CameraMatrices>)>,
    last_refreshed_frame: Option<u64>,
}

#[derive(Default)]
pub(crate) struct ViewComposition {
    /// The installed composition with anchored views at their anchors'
    /// rects: what frames draw.
    composition: Option<RendererViewComposition>,
    /// The composition as the product committed it.
    committed: Option<RendererViewComposition>,
    /// The rects the product UI reports for anchored views.
    anchors: RendererViewportAnchors,
    /// Aligned with the composition's targets.
    targets: Vec<CompositionTarget>,
    /// Aligned with the composition's cameras.
    motions: Vec<CameraMotion>,
    /// The frame's passes, resolved when the composition is installed: a
    /// frame indexes cameras and targets and looks no name up.
    plan: Plan,
    /// Counts installed compositions on this renderer.
    revision: u64,
    /// Counts composition frames rendered.
    frame: u64,
    /// Inspection camera that replaces the primary views' poses.
    observer: Option<CameraPose>,
    /// A camera cut since the last primary frame (`motion.rs`).
    pub(crate) cut: bool,
    /// What the last composition frame drew from, for
    /// [`Renderer::drawn_cameras`]: each camera's sampled pose, the
    /// observer then, and each camera's primary/offscreen use.
    drawn_poses: Vec<CameraPose>,
    drawn_observer: Option<CameraPose>,
    drawn_use: Vec<CameraUse>,
}

/// A composition's passes with every name resolved to an index.
#[derive(Default)]
struct Plan {
    /// Offscreen views grouped by target, targets in the order their first
    /// view draws: (target, [(view, camera)]), each in (order, id) order.
    offscreen: Vec<(usize, Vec<(usize, usize)>)>,
    /// Primary views and presentations in (order, id) order.
    steps: Vec<PrimaryStep>,
    /// Per camera: which kinds of view draw from it.
    cameras: Vec<CameraUse>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CameraUse {
    primary: bool,
    offscreen: bool,
}

/// How one composition camera drew in the last composition frame, aligned
/// with that composition's cameras ([`Renderer::drawn_cameras`]).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnCamera {
    /// The pose its primary views drew from: the observer while one replaced
    /// it, otherwise its pose at the frame's presentation time (motion
    /// sampled). A camera only offscreen views use reports that sampled pose.
    pub pose: RendererCameraPose,
    pub basis: RendererCameraBasis,
    /// Its primary views drew from the observer.
    pub observer: bool,
    /// While the observer replaced it in primary views: the sampled pose its
    /// offscreen views still drew from.
    pub offscreen: Option<(RendererCameraPose, RendererCameraBasis)>,
}

/// Whether an offscreen target shows the current scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetStatus {
    NeverRendered,
    /// It will be drawn again on the next frame.
    Stale,
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetReadout {
    pub id: String,
    pub revision: u64,
    pub status: TargetStatus,
    pub last_refreshed_frame: Option<u64>,
}

/// What the renderer realized of the installed composition, for the
/// presentation observation.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewCompositionReadout {
    /// Renderer-local count of installed compositions, not a publication
    /// revision.
    pub revision: u64,
    /// Composition frames rendered so far.
    pub frame: u64,
    pub views: usize,
    pub presentations: usize,
    pub targets: Vec<TargetReadout>,
    pub cameras: Vec<CameraSampleReadout>,
}

/// `composition` with each anchored view's viewport replaced by its
/// anchor's rect, where `anchors` has one.
fn anchored(
    composition: &RendererViewComposition,
    anchors: &RendererViewportAnchors,
) -> RendererViewComposition {
    let mut effective = composition.clone();
    for view in &mut effective.views {
        if let Some(rect) = view
            .viewport_anchor
            .as_ref()
            .and_then(|name| anchors.get(name))
        {
            view.viewport = *rect;
        }
    }
    effective
}

/// A normalized, bottom-left based viewport in pixels, top-left based.
pub(crate) fn pixel_viewport(viewport: &RendererViewport, width: u32, height: u32) -> PixelRect {
    let (full_width, full_height) = (f64::from(width.max(1)), f64::from(height.max(1)));
    let x = (viewport.x * full_width)
        .round()
        .clamp(0.0, full_width - 1.0);
    let y = (viewport.y * full_height)
        .round()
        .clamp(0.0, full_height - 1.0);
    let area_width = (viewport.width * full_width)
        .round()
        .min(full_width - x)
        .max(1.0);
    let area_height = (viewport.height * full_height)
        .round()
        .min(full_height - y)
        .max(1.0);
    PixelRect {
        x: x as u32,
        y: (full_height - y - area_height) as u32,
        width: area_width as u32,
        height: area_height as u32,
    }
}

fn ordered<'a, T>(items: impl Iterator<Item = (&'a str, u64, T)>) -> Vec<T> {
    let mut items: Vec<_> = items.collect();
    items.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)));
    items.into_iter().map(|(_, _, item)| item).collect()
}

enum PrimaryStep {
    View { view: usize, camera: usize },
    Presentation { presentation: usize, target: usize },
}

impl Plan {
    fn new(composition: &RendererViewComposition) -> Self {
        let cameras: HashMap<&str, usize> = composition
            .cameras
            .iter()
            .enumerate()
            .map(|(index, camera)| (camera.id.as_str(), index))
            .collect();
        let targets: HashMap<&str, usize> = composition
            .targets
            .iter()
            .enumerate()
            .map(|(index, target)| (target.id.as_str(), index))
            .collect();
        let mut plan = Plan {
            cameras: vec![CameraUse::default(); composition.cameras.len()],
            ..Plan::default()
        };
        // A view naming a missing camera or target draws nothing.
        let offscreen = ordered(composition.views.iter().enumerate().filter_map(
            |(index, view)| {
                let RendererViewTarget::Offscreen { target_id, .. } = &view.target else {
                    return None;
                };
                let camera = *cameras.get(view.camera_id.as_str())?;
                let target = *targets.get(target_id.as_str())?;
                Some((view.id.as_str(), view.order, (index, camera, target)))
            },
        ));
        for (view, camera, target) in offscreen {
            plan.cameras[camera].offscreen = true;
            match plan
                .offscreen
                .iter_mut()
                .find(|(existing, _)| *existing == target)
            {
                Some((_, passes)) => passes.push((view, camera)),
                None => plan.offscreen.push((target, vec![(view, camera)])),
            }
        }
        let views = composition
            .views
            .iter()
            .enumerate()
            .filter_map(|(index, view)| {
                (view.target == RendererViewTarget::Primary)
                    .then(|| cameras.get(view.camera_id.as_str()))
                    .flatten()
                    .map(|camera| {
                        let step = PrimaryStep::View {
                            view: index,
                            camera: *camera,
                        };
                        (view.id.as_str(), view.order, step)
                    })
            });
        let presentations =
            composition
                .presentations
                .iter()
                .enumerate()
                .filter_map(|(index, presentation)| {
                    let target = *targets.get(presentation.source_target_id.as_str())?;
                    let step = PrimaryStep::Presentation {
                        presentation: index,
                        target,
                    };
                    Some((presentation.id.as_str(), presentation.order, step))
                });
        plan.steps = ordered(views.chain(presentations));
        for step in &plan.steps {
            if let PrimaryStep::View { camera, .. } = step {
                plan.cameras[*camera].primary = true;
            }
        }
        plan
    }
}

impl Renderer {
    /// Install the committed camera composition. `time_seconds` is the
    /// presentation time it arrived at, on the same clock the host passes to
    /// [`Renderer::render_view_composition`].
    pub fn set_view_composition(
        &mut self,
        composition: &RendererViewComposition,
        time_seconds: f64,
    ) {
        let state = &mut self.composition;
        // Names are resolved here, once per installed composition.
        let mut previous_targets: HashMap<String, CompositionTarget> = state
            .targets
            .drain(..)
            .map(|target| (target.descriptor.id.clone(), target))
            .collect();
        state.targets = composition
            .targets
            .iter()
            .map(|descriptor| match previous_targets.remove(&descriptor.id) {
                Some(mut current) if current.descriptor == *descriptor => {
                    current.stale = true;
                    current
                }
                _ => composition_target(&self.gpu, &self.compose, descriptor),
            })
            .collect();
        let mut previous_motions: HashMap<String, CameraMotion> = state
            .composition
            .iter()
            .flat_map(|previous| previous.cameras.iter().map(|camera| camera.id.clone()))
            .zip(state.motions.drain(..))
            .collect();
        state.motions = composition
            .cameras
            .iter()
            .map(|camera| {
                let mut motion = previous_motions.remove(&camera.id).unwrap_or_default();
                let sampled = motion.receive(camera, time_seconds);
                state.cut |= sampled && camera.motion.as_ref().is_some_and(|motion| motion.cut);
                motion
            })
            .collect();
        let effective = anchored(composition, &state.anchors);
        state.plan = Plan::new(&effective);
        state.composition = Some(effective);
        state.committed = Some(composition.clone());
        state.revision += 1;
    }

    /// Draw each anchored view at the rect its anchor names, or at its
    /// committed viewport while `anchors` has none for it. Host
    /// presentation facts, like the pixel ratio: the product's composition
    /// and camera motion are unchanged.
    pub fn set_viewport_anchors(&mut self, anchors: RendererViewportAnchors) {
        let state = &mut self.composition;
        if state.anchors == anchors {
            return;
        }
        state.anchors = anchors;
        if let Some(committed) = &state.committed {
            let effective = anchored(committed, &state.anchors);
            state.plan = Plan::new(&effective);
            state.composition = Some(effective);
        }
    }

    /// Draw every primary view from `pose` instead of its camera's pose, or
    /// from the composition's cameras again with `None`. Inspection only: the
    /// product's cameras and their motion are unchanged.
    pub fn set_observer(&mut self, pose: Option<RendererCameraPose>) {
        self.composition.observer = pose.map(|pose| camera::pose_from_degrees(&pose));
    }

    /// Render the installed composition into the offscreen target at
    /// presentation time `time_seconds`.
    pub fn render_view_composition(
        &mut self,
        target: &OffscreenTarget,
        time_seconds: f64,
    ) -> FrameStats {
        let uploaded = self.prepare();
        let mut stats = self.draw_primary(target.view(), |renderer, view| {
            renderer.render_composition(view, time_seconds)
        });
        stats.video = self.draw_video(&target.view());
        stats.parts_uploaded = uploaded;
        stats
    }

    /// Draw the primary output through `draw` at render resolution, then
    /// bring it to output resolution (`upscale.rs`). With no upscale stage
    /// selected and no image effect, `draw` draws into `output` itself.
    /// Otherwise it draws into the render picture, and the stage, or the
    /// image effect in its place, draws that over the whole of `output`. A
    /// video and the hosts' UI draw at output resolution after.
    pub(crate) fn draw_primary(
        &mut self,
        output: TargetView<'_>,
        draw: impl FnOnce(&mut Self, TargetView<'_>) -> FrameStats,
    ) -> FrameStats {
        // An image effect reads the whole picture, so the views draw into a
        // picture of their own first, as an upscale stage's do. It takes
        // the stage's place, so the temporal stage is refused with it
        // (`Renderer::temporal_refusal`).
        let effect = self.image_effect.active();
        let mut stage = UpscaleMethod::selected(
            &self.options,
            self.temporal_refusal().is_none(),
            self.dlss_refusal().is_none(),
        );
        if !stage.is_some_and(UpscaleMethod::temporal) {
            self.upscale.forget_history();
        }
        let scale = stage.map_or(1.0, |_| self.options.render_scale);
        let mut resolution = FrameResolution::scaled((output.width, output.height), scale);
        // DLSS draws from the input sizes its mode takes; refused as it
        // starts, TAAU draws.
        if stage == Some(UpscaleMethod::Dlss) {
            match self
                .upscale
                .dlss_render_size(&self.gpu, resolution, self.options.render_scale)
            {
                Some(render) => resolution.render = render,
                None => stage = Some(UpscaleMethod::Taau),
            }
        }
        // A temporal stage's jitter and motion, or the motion alone while
        // inspection shows it (`motion.rs`).
        let jittering = stage.is_some_and(UpscaleMethod::temporal);
        let motion = jittering || self.overlays.shown.motion;
        // A temporal stage's picture is single-sample whatever the output's
        // samples: it anti-aliases in MSAA's place.
        let samples = if jittering { 1 } else { output.samples };
        let cut = std::mem::take(&mut self.composition.cut);
        let kept = self.temporal.keeps_history();
        if self.temporal.begin_frame(
            &self.gpu,
            &self.tables.parts,
            motion,
            jittering,
            resolution,
            samples,
            &self.options,
            self.composition.observer.is_some(),
            cut,
            self.presentation_time,
        ) {
            self.rebind_frame();
        }
        if motion {
            self.particles.drawn();
            self.settle_skinned_history();
        } else if kept {
            self.forget_skinned_history();
        }
        if stage.is_none() && !effect {
            self.picture = None;
            let mut stats = draw(self, output);
            stats.cpu_batch_us = self.batch_time.as_micros() as u32;
            stats.resolution = resolution;
            stats.motion = self.temporal.end_frame();
            self.upscaled = crate::UpscaleReadout {
                resolution,
                method: None,
            };
            self.show_motion(&output);
            return stats;
        }
        let (width, height) = resolution.render;
        let picture = match self.picture.take() {
            Some(picture) if picture.matches(width, height, samples, jittering) => picture,
            _ => RenderPicture::new(&self.gpu, width, height, samples, jittering),
        };
        // The viewmodel passes draw against a depth of their own, cleared
        // here since a pass with nothing to draw leaves it untouched.
        self.viewmodel_depth = picture.viewmodel_depth().cloned();
        if let Some(depth) = &self.viewmodel_depth {
            self.clear_depth(depth);
        }
        let mut stats = draw(self, picture.view());
        self.viewmodel_depth = None;
        stats.cpu_batch_us = self.batch_time.as_micros() as u32;
        stats.resolution = resolution;
        stats.motion = self.temporal.end_frame();
        stats.upscale = stage.filter(|_| !effect);
        self.upscaled = crate::UpscaleReadout {
            resolution,
            method: stats.upscale,
        };
        if effect {
            let white = &self.white;
            let texture = |slot: usize| {
                self.tables
                    .image_effect
                    .as_ref()
                    .and_then(|effect| effect.textures[slot].as_ref())
                    .and_then(|id| self.tables.textures.get(id))
                    .unwrap_or(white)
            };
            let parameters = self
                .tables
                .image_effect
                .as_ref()
                .map_or([[0.0; 4]; 4], |effect| effect.parameters);
            self.image_effect.apply(
                &self.gpu,
                &picture,
                &output,
                &parameters,
                [texture(0), texture(1)],
                self.presentation_time,
            );
        } else if let Some(stage) = stage {
            self.upscale.run(
                &self.gpu,
                &mut self.layouts.shaders,
                &mut self.compose,
                stage,
                &picture,
                self.temporal.inputs(),
                self.options.render_scale,
                &output,
            );
        }
        self.picture = Some(picture);
        self.show_motion(&output);
        stats
    }

    /// Clear `depth` to the far plane.
    fn clear_depth(&self, depth: &wgpu::TextureView) {
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu viewmodel depth"),
            });
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-wgpu viewmodel depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// The motion target over the whole of `output`, while inspection shows
    /// it (`OverlayShown::motion`).
    fn show_motion(&mut self, output: &TargetView<'_>) {
        if !self.overlays.shown.motion {
            return;
        }
        if let Some(inputs) = self.temporal.inputs() {
            self.compose.show_motion(&self.gpu, output, inputs.motion);
        }
    }

    /// Render the installed composition into an acquired window frame.
    /// Present it with [`WindowSurface::present`].
    pub fn render_view_composition_to_frame(
        &mut self,
        surface: &mut WindowSurface,
        frame: &SurfaceFrame,
        time_seconds: f64,
    ) -> FrameStats {
        let uploaded = self.prepare();
        surface.request(self.samples(), self.vsync());
        self.display_vsync_only = Some(surface.vsync_only());
        self.surface_size = Some(surface.size());
        let (view, _) = surface.views(frame);
        let mut stats = self.draw_primary(view, |renderer, view| {
            renderer.render_composition(view, time_seconds)
        });
        stats.video = self.draw_video(&view);
        stats.parts_uploaded = uploaded;
        stats
    }

    pub fn view_composition_readout(&self) -> ViewCompositionReadout {
        let state = &self.composition;
        let composition = state.composition.as_ref();
        let targets = state
            .targets
            .iter()
            .map(|target| TargetReadout {
                id: target.descriptor.id.clone(),
                revision: target.descriptor.revision,
                status: match (&target.drawn, target.last_refreshed_frame) {
                    (_, None) => TargetStatus::NeverRendered,
                    (Some((generation, _)), Some(_))
                        if !target.stale && *generation == self.scene_generation =>
                    {
                        TargetStatus::Current
                    }
                    _ => TargetStatus::Stale,
                },
                last_refreshed_frame: target.last_refreshed_frame,
            })
            .collect();
        let mut cameras: Vec<CameraSampleReadout> = composition
            .map(|composition| &composition.cameras[..])
            .unwrap_or_default()
            .iter()
            .zip(&state.motions)
            .map(|(camera, motion)| motion.readout(&camera.id))
            .collect();
        cameras.sort_by(|a, b| a.camera_id.cmp(&b.camera_id));
        ViewCompositionReadout {
            revision: state.revision,
            frame: state.frame,
            views: composition.map_or(0, |composition| composition.views.len()),
            presentations: composition.map_or(0, |composition| composition.presentations.len()),
            targets,
            cameras,
        }
    }

    /// One camera over the whole target: the world, then the viewmodel.
    pub(crate) fn render_camera(
        &mut self,
        camera: &RendererCompositionCamera,
        target: TargetView<'_>,
    ) -> FrameStats {
        let area = PixelRect::whole(target.width, target.height);
        let drawn = self.draw_primary_view(
            &target,
            area,
            camera::descriptor_pose(camera),
            camera,
            PassStart::Target,
            false,
        );
        let mut stats = FrameStats {
            lights: self.lights.world.count,
            ..FrameStats::default()
        };
        stats += drawn;
        stats
    }

    fn draw_primary_view(
        &mut self,
        target: &TargetView<'_>,
        area: PixelRect,
        pose: CameraPose,
        camera: &RendererCompositionCamera,
        start: PassStart,
        overlays: bool,
    ) -> ViewStats {
        let clear = self.environment_clear();
        let world_camera = camera::camera_matrices(pose, &camera.projection, area.aspect());
        let viewmodel_camera = camera::viewmodel_matrices(
            &camera.projection,
            camera.viewmodel_fov_y_degrees,
            area.aspect(),
        );
        // A viewmodel label goes where this view's world projection puts the
        // pixel its viewmodel projection draws the point at.
        let viewmodel_to_world = world_camera.view_proj.inverse() * viewmodel_camera.view_proj;
        self.aim_overlay_labels(crate::overlays::PLAYER_VIEW, Some(viewmodel_to_world));
        let labels = self.place_labels(&world_camera, area);
        // A temporal stage's jitter and motion (`motion.rs`): labels keep
        // the unjittered camera.
        let jitter = self.temporal.frame().and_then(|frame| frame.jitter);
        let world_motion =
            self.temporal
                .view_pass(&camera.id, ViewLayer::World, &world_camera, area);
        let viewmodel_motion =
            self.temporal
                .view_pass(&camera.id, ViewLayer::Viewmodel, &viewmodel_camera, area);
        let world = self.encode_view(ViewPass {
            target: *target,
            viewport: area,
            camera: world_camera,
            layer: ViewLayer::World,
            start,
            clear,
            sky: true,
            backdrop: None,
            camera_id: Some(camera.id.as_str()),
            overlays: overlays.then_some(crate::overlays::PLAYER_VIEW),
            jitter,
            motion: world_motion,
        });
        self.capture_sky(&world_camera, Some(camera.id.as_str()), clear);
        // Depth-layer labels test the world's depth before the viewmodel
        // pass clears it.
        let depth_labels = self.draw_labels(target, area, &labels, LabelPass::Depth);
        // Every view pass clears its own viewport's depth first, so the
        // viewmodel's depth break may clear the whole target.
        // Under a temporal stage the viewmodel keeps its own depth, so the
        // world's stays for the stage (`RenderPicture::viewmodel_depth`).
        let viewmodel_depth = self.viewmodel_depth.clone();
        let viewmodel_target = match &viewmodel_depth {
            Some(depth) => TargetView { depth, ..*target },
            None => *target,
        };
        let viewmodel = self.encode_view(ViewPass {
            target: viewmodel_target,
            viewport: area,
            camera: viewmodel_camera,
            layer: ViewLayer::Viewmodel,
            start: PassStart::Target,
            clear,
            sky: false,
            backdrop: None,
            camera_id: None,
            overlays: overlays.then_some(crate::overlays::PLAYER_VIEW),
            jitter,
            motion: viewmodel_motion,
        });
        let top_labels = self.draw_labels(target, area, &labels, LabelPass::OnTop);
        let mut stats = world + viewmodel;
        stats.draws += depth_labels + top_labels;
        stats
    }

    pub(crate) fn render_composition(
        &mut self,
        primary: TargetView<'_>,
        time_seconds: f64,
    ) -> FrameStats {
        self.composition.frame += 1;
        let frame = self.composition.frame;
        let mut stats = FrameStats {
            lights: self.lights.world.count,
            ..FrameStats::default()
        };
        // Taken for the frame and restored after it: the view passes below
        // borrow the renderer mutably, and the frame copies nothing.
        let Some(composition) = self.composition.composition.take() else {
            self.clear_target(&primary);
            return stats;
        };
        let plan = std::mem::take(&mut self.composition.plan);
        let mut poses = std::mem::take(&mut self.composition.drawn_poses);
        poses.clear();
        for (camera, motion) in composition
            .cameras
            .iter()
            .zip(&mut self.composition.motions)
        {
            poses.push(
                motion
                    .pose(time_seconds)
                    .unwrap_or_else(|| camera::descriptor_pose(camera)),
            );
        }

        // Overlays resolve once a frame, for the first primary view's camera.
        let primary_camera =
            first_primary_camera(&composition, &plan, &poses, (primary.width, primary.height));
        self.resolve_overlays(primary_camera);

        let clear = self.environment_clear();
        for (target_index, views) in &plan.offscreen {
            let target = &self.composition.targets[*target_index];
            let (width, height) = (target.descriptor.width, target.descriptor.height);
            let (color, depth) = (target.color.clone(), target.depth.clone());
            let passes: Vec<(PixelRect, CameraMatrices, usize)> = views
                .iter()
                .map(|(view, camera)| {
                    let area = pixel_viewport(&composition.views[*view].viewport, width, height);
                    let projection = &composition.cameras[*camera].projection;
                    let matrices =
                        camera::camera_matrices(poses[*camera], projection, area.aspect());
                    (area, matrices, *camera)
                })
                .collect();
            let cameras: Vec<CameraMatrices> =
                passes.iter().map(|(_, matrices, _)| *matrices).collect();
            let fresh = !target.stale
                && target.drawn.as_ref().is_some_and(|(generation, drawn)| {
                    *generation == self.scene_generation && *drawn == cameras
                });
            if fresh {
                continue;
            }
            // Offscreen composition targets are single-sample.
            let view = TargetView {
                color: &color,
                depth: &depth,
                format: OFFSCREEN_FORMAT,
                samples: 1,
                width,
                height,
            };
            for (index, (area, matrices, camera)) in passes.iter().enumerate() {
                stats += self.encode_view(ViewPass {
                    target: view,
                    viewport: *area,
                    camera: *matrices,
                    layer: ViewLayer::World,
                    start: if index == 0 {
                        PassStart::Target
                    } else {
                        PassStart::Viewport
                    },
                    clear,
                    sky: true,
                    backdrop: None,
                    camera_id: Some(composition.cameras[*camera].id.as_str()),
                    overlays: None,
                    jitter: None,
                    motion: None,
                });
                stats.offscreen_views += 1;
            }
            let target = &mut self.composition.targets[*target_index];
            target.stale = false;
            target.drawn = Some((self.scene_generation, cameras));
            target.last_refreshed_frame = Some(frame);
        }

        // Primary views and presentations, in (order, id) order.
        let whole = PixelRect::whole(primary.width, primary.height);
        let first_covers = matches!(plan.steps.first(), Some(PrimaryStep::View { view, .. })
            if pixel_viewport(&composition.views[*view].viewport, primary.width, primary.height) == whole);
        if !first_covers {
            self.clear_target(&primary);
        }
        let observer = self.composition.observer;
        for (position, step) in plan.steps.iter().enumerate() {
            match step {
                PrimaryStep::View { view, camera } => {
                    let pose = observer.unwrap_or(poses[*camera]);
                    let area = pixel_viewport(
                        &composition.views[*view].viewport,
                        primary.width,
                        primary.height,
                    );
                    let start = if position == 0 && first_covers {
                        PassStart::Target
                    } else {
                        PassStart::Viewport
                    };
                    stats += self.draw_primary_view(
                        &primary,
                        area,
                        pose,
                        &composition.cameras[*camera],
                        start,
                        true,
                    );
                }
                PrimaryStep::Presentation {
                    presentation,
                    target,
                } => {
                    let area = pixel_viewport(
                        &composition.presentations[*presentation]
                            .destination
                            .viewport,
                        primary.width,
                        primary.height,
                    );
                    self.present_target(&primary, area, *target);
                }
            }
        }
        stats += self.draw_anchored_inspection_views(&primary, primary_camera);
        let state = &mut self.composition;
        state.drawn_use.clone_from(&plan.cameras);
        state.drawn_observer = observer;
        state.drawn_poses = poses;
        state.plan = plan;
        state.composition = Some(composition);
        stats
    }

    /// The primary camera at `time_seconds`, for a target of `size`: the
    /// camera of the composition's first primary view.
    pub(crate) fn primary_overlay_camera(
        &mut self,
        time_seconds: f64,
        size: (u32, u32),
    ) -> Option<crate::overlays::OverlayCamera> {
        let state = &mut self.composition;
        let composition = state.composition.as_ref()?;
        let poses: Vec<CameraPose> = composition
            .cameras
            .iter()
            .zip(&mut state.motions)
            .map(|(camera, motion)| {
                motion
                    .pose(time_seconds)
                    .unwrap_or_else(|| camera::descriptor_pose(camera))
            })
            .collect();
        first_primary_camera(composition, &state.plan, &poses, size)
    }

    /// The rect the page reports for a view anchor.
    pub(crate) fn composition_anchor(&self, name: &str) -> Option<RendererViewport> {
        self.composition.anchors.get(name).copied()
    }

    /// How each camera of the installed composition drew in the last
    /// composition frame, aligned with its cameras: the observer where it
    /// replaced a camera in primary views, otherwise the sampled pose.
    /// The size of the window surface last presented: a window renderer's
    /// output size. `None` for a renderer that only draws offscreen.
    pub fn surface_size(&self) -> Option<(u32, u32)> {
        self.surface_size
    }

    pub fn drawn_cameras(&self) -> Vec<DrawnCamera> {
        let state = &self.composition;
        state
            .drawn_poses
            .iter()
            .zip(&state.drawn_use)
            .map(|(sampled, used)| match state.drawn_observer {
                Some(observer) if used.primary => {
                    let (pose, basis) = camera::pose_readout(observer);
                    DrawnCamera {
                        pose,
                        basis,
                        observer: true,
                        offscreen: used.offscreen.then(|| camera::pose_readout(*sampled)),
                    }
                }
                _ => {
                    let (pose, basis) = camera::pose_readout(*sampled);
                    DrawnCamera {
                        pose,
                        basis,
                        observer: false,
                        offscreen: None,
                    }
                }
            })
            .collect()
    }

    /// Clear the whole target to the environment colour and depth.
    fn clear_target(&mut self, target: &TargetView<'_>) {
        let [r, g, b, a] = self.environment_clear().map(f64::from);
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu clear"),
            });
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-wgpu clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        // The depth may be multisampled: a pass of its own.
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-wgpu clear depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.gpu.queue.submit([encoder.finish()]);
    }

    fn present_target(&mut self, primary: &TargetView<'_>, area: PixelRect, target: usize) {
        self.compose.prepare_blit(&self.gpu.device, primary.format);
        let Some(target) = self.composition.targets.get(target) else {
            return;
        };
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu present target"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu present target"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: primary.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                area.x as f32,
                area.y as f32,
                area.width as f32,
                area.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(area.x, area.y, area.width, area.height);
            self.compose
                .blit(&mut pass, primary.format, &target.present);
        }
        self.gpu.queue.submit([encoder.finish()]);
    }
}

fn composition_target(
    gpu: &crate::Gpu,
    compose: &crate::compose::Compose,
    descriptor: &RendererCompositionTarget,
) -> CompositionTarget {
    // `RendererTargetDepth::None` still gets a depth buffer: every view pass
    // depth-tests, and drawing in submission order is not a product need.
    let color = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render-wgpu composition target"),
        size: target::extent(descriptor.width, descriptor.height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let filter = match descriptor.sampling {
        RendererTargetSampling::Linear => wgpu::FilterMode::Linear,
        RendererTargetSampling::Nearest => wgpu::FilterMode::Nearest,
    };
    let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("render-wgpu composition target"),
        mag_filter: filter,
        min_filter: filter,
        ..Default::default()
    });
    let color = color.create_view(&Default::default());
    CompositionTarget {
        present: compose.blit_bind_group(&gpu.device, &color, &sampler),
        depth: target::depth_texture(gpu, descriptor.width, descriptor.height),
        color,
        descriptor: descriptor.clone(),
        stale: true,
        drawn: None,
        last_refreshed_frame: None,
    }
}

/// The camera of a composition's first primary view, posed by `poses`.
fn first_primary_camera(
    composition: &RendererViewComposition,
    plan: &Plan,
    poses: &[CameraPose],
    (width, height): (u32, u32),
) -> Option<crate::overlays::OverlayCamera> {
    plan.steps.iter().find_map(|step| match step {
        PrimaryStep::View { view, camera } => Some(crate::overlays::OverlayCamera {
            pose: poses[*camera],
            projection: composition.cameras[*camera].projection,
            aspect: pixel_viewport(&composition.views[*view].viewport, width, height).aspect(),
        }),
        PrimaryStep::Presentation { .. } => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewports_are_bottom_left_based_and_clamped() {
        let viewport = |x, y, width, height| RendererViewport {
            x,
            y,
            width,
            height,
        };
        assert_eq!(
            pixel_viewport(&viewport(0.0, 0.0, 1.0, 1.0), 320, 180),
            PixelRect::whole(320, 180)
        );
        // The lower-right quarter.
        assert_eq!(
            pixel_viewport(&viewport(0.5, 0.0, 0.5, 0.5), 320, 180),
            PixelRect {
                x: 160,
                y: 90,
                width: 160,
                height: 90
            }
        );
        // Past the edge: clamped to at least one pixel inside.
        assert_eq!(
            pixel_viewport(&viewport(1.0, 1.0, 0.5, 0.5), 320, 180),
            PixelRect {
                x: 319,
                y: 0,
                width: 1,
                height: 1
            }
        );
    }
}
