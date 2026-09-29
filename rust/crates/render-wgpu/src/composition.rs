//! Camera composition: the committed `RendererViewComposition` the C#
//! `CameraView` service publishes (`RuntimePublication::ViewComposition`),
//! realized as view passes.
//!
//! Each frame, in order:
//! 1. offscreen views, by `(order, id)`, into their targets. A target is drawn
//!    only when it is stale: never drawn, a new composition, a delta applied
//!    since, or a view camera that moved;
//! 2. primary steps (views and presentations), by `(order, id)`. Each primary
//!    view clears its viewport and draws the world once, then the viewmodel
//!    layer after a depth break. A presentation draws an offscreen target
//!    into its destination viewport.
//!
//! There is no fallback world pass: primary area no view covers keeps the
//! environment clear. Camera poses come from the composition; motion
//! interpolates between its samples on the presentation time the host passes.
//! The backend neither validates nor sequences compositions: the runtime
//! validated this one, a target reallocates when its descriptor or revision
//! changes, and a view naming a missing camera or target draws nothing.

use std::collections::HashMap;

use render_host_contracts::{
    RendererCompositionCamera, RendererCompositionTarget, RendererTargetSampling,
    RendererViewComposition, RendererViewTarget, RendererViewport,
};

use crate::camera::{self, CameraMatrices, CameraMotion, CameraPose, CameraSampleReadout};
use crate::frame::{PassStart, PixelRect, ViewLayer, ViewPass, ViewStats};
use crate::labels::LabelPass;
use crate::target::{self, TargetView, OFFSCREEN_FORMAT};
use crate::{FrameStats, OffscreenTarget, PresentSkip, Renderer, WindowSurface};

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
    composition: Option<RendererViewComposition>,
    targets: HashMap<String, CompositionTarget>,
    motions: HashMap<String, CameraMotion>,
    /// Counts installed compositions on this renderer.
    revision: u64,
    /// Counts composition frames rendered.
    frame: u64,
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
    View(usize),
    Presentation(usize),
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
        let mut targets = HashMap::with_capacity(composition.targets.len());
        for descriptor in &composition.targets {
            let target = match state.targets.remove(&descriptor.id) {
                Some(mut current) if current.descriptor == *descriptor => {
                    current.stale = true;
                    current
                }
                _ => composition_target(&self.gpu, &self.compose, descriptor),
            };
            targets.insert(descriptor.id.clone(), target);
        }
        state.targets = targets;
        let mut motions = HashMap::with_capacity(composition.cameras.len());
        for camera in &composition.cameras {
            let mut motion = state.motions.remove(&camera.id).unwrap_or_default();
            motion.receive(camera, time_seconds);
            motions.insert(camera.id.clone(), motion);
        }
        state.motions = motions;
        state.composition = Some(composition.clone());
        state.revision += 1;
    }

    /// Render the installed composition into the offscreen target at
    /// presentation time `time_seconds`.
    pub fn render_view_composition(
        &mut self,
        target: &OffscreenTarget,
        time_seconds: f64,
    ) -> FrameStats {
        let uploaded = self.prepare();
        let mut stats = self.render_composition(target.view(), time_seconds);
        stats.parts_uploaded = uploaded;
        stats
    }

    /// Render the installed composition into the window and present it.
    pub fn render_view_composition_to_surface(
        &mut self,
        surface: &mut WindowSurface,
        time_seconds: f64,
    ) -> Result<FrameStats, PresentSkip> {
        let uploaded = self.prepare();
        let gpu = self.gpu.clone();
        let mut stats = FrameStats::default();
        surface.present_with(&gpu, |view| {
            stats = self.render_composition(view, time_seconds);
        })?;
        stats.parts_uploaded = uploaded;
        Ok(stats)
    }

    pub fn view_composition_readout(&self) -> ViewCompositionReadout {
        let state = &self.composition;
        let composition = state.composition.as_ref();
        let targets = composition
            .map(|composition| &composition.targets[..])
            .unwrap_or_default()
            .iter()
            .filter_map(|descriptor| state.targets.get(&descriptor.id))
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
        let mut cameras: Vec<CameraSampleReadout> = state
            .motions
            .iter()
            .map(|(id, motion)| motion.readout(id))
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
    ) -> ViewStats {
        let clear = self.environment_clear();
        let world_camera = camera::camera_matrices(pose, &camera.projection, area.aspect());
        let labels = self.place_labels(&world_camera, area);
        let world = self.encode_view(ViewPass {
            target: *target,
            viewport: area,
            camera: world_camera,
            layer: ViewLayer::World,
            start,
            clear,
            sky: true,
        });
        // Depth-layer labels test the world's depth before the viewmodel
        // pass clears it.
        let depth_labels = self.draw_labels(target, area, &labels, LabelPass::Depth);
        // Every view pass clears its own viewport's depth first, so the
        // viewmodel's depth break may clear the whole target.
        let viewmodel = self.encode_view(ViewPass {
            target: *target,
            viewport: area,
            camera: camera::viewmodel_matrices(&camera.projection, area.aspect()),
            layer: ViewLayer::Viewmodel,
            start: PassStart::Target,
            clear,
            sky: false,
        });
        let top_labels = self.draw_labels(target, area, &labels, LabelPass::OnTop);
        let mut stats = world + viewmodel;
        stats.draws += depth_labels + top_labels;
        stats
    }

    fn render_composition(&mut self, primary: TargetView<'_>, time_seconds: f64) -> FrameStats {
        self.composition.frame += 1;
        let frame = self.composition.frame;
        let mut stats = FrameStats {
            lights: self.lights.world.count,
            ..FrameStats::default()
        };
        let Some(composition) = self.composition.composition.clone() else {
            self.clear_target(&primary);
            return stats;
        };
        let mut poses = HashMap::with_capacity(composition.cameras.len());
        for camera in &composition.cameras {
            let pose = self
                .composition
                .motions
                .get_mut(&camera.id)
                .and_then(|motion| motion.pose(time_seconds))
                .unwrap_or_else(|| camera::descriptor_pose(camera));
            poses.insert(camera.id.as_str(), (pose, camera));
        }

        // Offscreen views, grouped by target in (order, id) order.
        let offscreen = ordered(
            composition
                .views
                .iter()
                .filter_map(|view| match &view.target {
                    RendererViewTarget::Offscreen { target_id, .. } => {
                        Some((view.id.as_str(), view.order, (view, target_id.as_str())))
                    }
                    RendererViewTarget::Primary => None,
                }),
        );
        let mut target_ids: Vec<&str> = Vec::new();
        for (_, target_id) in &offscreen {
            if !target_ids.contains(target_id) {
                target_ids.push(target_id);
            }
        }
        let clear = self.environment_clear();
        for target_id in target_ids {
            let Some(target) = self.composition.targets.get(target_id) else {
                continue;
            };
            let (width, height) = (target.descriptor.width, target.descriptor.height);
            let (color, depth) = (target.color.clone(), target.depth.clone());
            let mut passes = Vec::new();
            for (view, _) in offscreen.iter().filter(|(_, id)| *id == target_id) {
                let Some((pose, camera)) = poses.get(view.camera_id.as_str()) else {
                    continue;
                };
                let area = pixel_viewport(&view.viewport, width, height);
                let matrices = camera::camera_matrices(*pose, &camera.projection, area.aspect());
                passes.push((area, matrices));
            }
            let cameras: Vec<CameraMatrices> =
                passes.iter().map(|(_, matrices)| *matrices).collect();
            let fresh = !target.stale
                && target.drawn.as_ref().is_some_and(|(generation, drawn)| {
                    *generation == self.scene_generation && *drawn == cameras
                });
            if fresh {
                continue;
            }
            // Offscreen composition targets stay single-sample, as Three's
            // render targets were.
            let view = TargetView {
                color: &color,
                resolve: None,
                depth: &depth,
                format: OFFSCREEN_FORMAT,
                samples: 1,
                width,
                height,
            };
            for (index, (area, matrices)) in passes.iter().enumerate() {
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
                });
                stats.offscreen_views += 1;
            }
            if let Some(target) = self.composition.targets.get_mut(target_id) {
                target.stale = false;
                target.drawn = Some((self.scene_generation, cameras));
                target.last_refreshed_frame = Some(frame);
            }
        }

        // Primary views and presentations, in (order, id) order.
        let steps = ordered(
            composition
                .views
                .iter()
                .enumerate()
                .filter(|(_, view)| view.target == RendererViewTarget::Primary)
                .map(|(index, view)| (view.id.as_str(), view.order, PrimaryStep::View(index)))
                .chain(composition.presentations.iter().enumerate().map(
                    |(index, presentation)| {
                        (
                            presentation.id.as_str(),
                            presentation.order,
                            PrimaryStep::Presentation(index),
                        )
                    },
                )),
        );
        let whole = PixelRect::whole(primary.width, primary.height);
        let first_covers = matches!(steps.first(), Some(PrimaryStep::View(index))
            if pixel_viewport(&composition.views[*index].viewport, primary.width, primary.height) == whole);
        if !first_covers {
            self.clear_target(&primary);
        }
        for (position, step) in steps.iter().enumerate() {
            match step {
                PrimaryStep::View(index) => {
                    let view = &composition.views[*index];
                    let Some((pose, camera)) = poses.get(view.camera_id.as_str()) else {
                        continue;
                    };
                    let area = pixel_viewport(&view.viewport, primary.width, primary.height);
                    let start = if position == 0 && first_covers {
                        PassStart::Target
                    } else {
                        PassStart::Viewport
                    };
                    stats += self.draw_primary_view(&primary, area, *pose, camera, start);
                }
                PrimaryStep::Presentation(index) => {
                    let presentation = &composition.presentations[*index];
                    let area = pixel_viewport(
                        &presentation.destination.viewport,
                        primary.width,
                        primary.height,
                    );
                    self.present_target(&primary, area, &presentation.source_target_id);
                }
            }
        }
        stats
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
                resolve_target: target.resolve,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                    store: wgpu::StoreOp::Store,
                },
            })],
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

    fn present_target(&mut self, primary: &TargetView<'_>, area: PixelRect, source: &str) {
        self.compose.prepare_blit(&self.gpu.device, primary.key());
        let Some(target) = self.composition.targets.get(source) else {
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
                    resolve_target: primary.resolve,
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
            self.compose.blit(&mut pass, primary.key(), &target.present);
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
