//! Inspection views: Engine-owned cameras drawn beside the product's views.
//!
//! An inspection view never changes the product's composition. Anchored to a
//! named rect the page reports, it draws over the primary target after the
//! primary views, each frame the page reports the rect. Offscreen, it draws
//! only for a capture. Either way it draws the world from its own camera,
//! then the viewmodel layer as world geometry placed at the primary camera's
//! pose: the camera-space assembly stays where the player's camera holds it,
//! depth-tested against the world, however the inspection camera moves.

use glam::{Mat4, Vec4Swizzles};
use render_host_contracts::{RendererCameraPose, RendererCameraProjection};

use crate::camera::{self, CameraMatrices};
use crate::composition::pixel_viewport;
use crate::frame::{PassStart, PixelRect, ViewLayer, ViewPass, ViewStats};
use crate::labels::LabelPass;
use crate::overlays::OverlayCamera;
use crate::target::TargetView;
use crate::{FrameStats, OffscreenTarget, Renderer};

/// One inspection view.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectionView {
    pub id: String,
    pub pose: RendererCameraPose,
    pub projection: RendererCameraProjection,
    /// The page rect it draws at, by name; `None` draws only for captures.
    pub anchor: Option<String>,
    /// The point it orbits and frames about; the renderer does not read it.
    pub focus: [f64; 3],
}

impl Renderer {
    /// Replaces the inspection views; each draws from the next frame.
    pub fn set_inspection_views(&mut self, views: Vec<InspectionView>) {
        self.inspection_views = views;
    }

    /// The inspection views the last composition frame drew, with the
    /// cameras it drew them from.
    pub fn drawn_inspection_views(&self) -> Vec<InspectionView> {
        self.drawn_inspection.clone()
    }

    /// The anchored inspection views the page shows, at their rects.
    pub(crate) fn draw_anchored_inspection_views(
        &mut self,
        primary: &TargetView<'_>,
        primary_camera: Option<OverlayCamera>,
    ) -> ViewStats {
        let mut stats = ViewStats::default();
        self.drawn_inspection.clear();
        let views: Vec<(InspectionView, PixelRect)> = self
            .inspection_views
            .iter()
            .filter_map(|view| {
                let rect = self.composition_anchor(view.anchor.as_deref()?)?;
                Some((
                    view.clone(),
                    pixel_viewport(&rect, primary.width, primary.height),
                ))
            })
            .collect();
        // They draw into the output frame's picture: under a temporal stage
        // their labels mark its reactive mask as the primary views' do.
        let label_motion = self.temporal.single_sample_target().cloned();
        for (view, area) in views {
            if area.width == 0 || area.height == 0 {
                continue;
            }
            stats = stats
                + self.draw_inspection_view(
                    primary,
                    area,
                    &view,
                    primary_camera,
                    PassStart::Viewport,
                    label_motion.as_ref(),
                );
            self.drawn_inspection.push(view);
        }
        stats
    }

    /// Draws one inspection view over the whole of `target`, for a capture.
    /// `None` when there is no such view.
    pub fn render_inspection_view(
        &mut self,
        target: &OffscreenTarget,
        id: &str,
        time_seconds: f64,
    ) -> Option<FrameStats> {
        let view = self
            .inspection_views
            .iter()
            .find(|view| view.id == id)?
            .clone();
        let uploaded = self.prepare();
        let primary_camera = self.primary_overlay_camera(time_seconds, target.size());
        self.resolve_overlays(primary_camera);
        let target_view = target.view();
        let area = PixelRect::whole(target_view.width, target_view.height);
        let mut stats = FrameStats::default();
        // A capture's target feeds no temporal stage: its labels mark no
        // motion target, whatever frame the output last drew. It is a
        // reading: the view keeps none of the output's histories
        // (`Renderer::reading`).
        self.reading = true;
        stats += self.draw_inspection_view(
            &target_view,
            area,
            &view,
            primary_camera,
            PassStart::Target,
            None,
        );
        self.reading = false;
        stats.parts_uploaded = uploaded;
        Some(stats)
    }

    fn draw_inspection_view(
        &mut self,
        target: &TargetView<'_>,
        area: PixelRect,
        view: &InspectionView,
        primary_camera: Option<OverlayCamera>,
        start: PassStart,
        label_motion: Option<&wgpu::TextureView>,
    ) -> ViewStats {
        let clear = self.environment_clear();
        let world_camera = camera::camera_matrices(
            camera::pose_from_degrees(&view.pose),
            &view.projection,
            area.aspect(),
        );
        // The viewmodel's parts are in the primary camera's space.
        let primary_world = primary_camera.map(|camera| {
            Mat4::from_rotation_translation(camera.pose.orientation, camera.pose.position)
        });
        self.aim_overlay_labels(&view.id, primary_world);
        let labels = self.place_labels(&world_camera, area);
        let world = self.encode_view(ViewPass {
            target: *target,
            viewport: area,
            camera: world_camera,
            layer: ViewLayer::World,
            start,
            clear,
            sky: true,
            backdrop: None,
            camera_id: None,
            overlays: Some(view.id.as_str()),
            jitter: None,
            motion: None,
        });
        let depth_labels = self.draw_labels(target, area, &labels, LabelPass::Depth, label_motion);
        let viewmodel = match primary_world {
            Some(primary_world) => {
                let view_matrix = world_camera.view * primary_world;
                self.encode_view(ViewPass {
                    target: *target,
                    viewport: area,
                    camera: CameraMatrices {
                        view_proj: world_camera.projection * view_matrix,
                        view: view_matrix,
                        projection: world_camera.projection,
                        eye: (primary_world.inverse() * world_camera.eye.extend(1.0)).xyz(),
                    },
                    layer: ViewLayer::Viewmodel,
                    start: PassStart::KeepDepth,
                    clear,
                    sky: false,
                    backdrop: None,
                    camera_id: None,
                    overlays: Some(view.id.as_str()),
                    jitter: None,
                    motion: None,
                })
            }
            None => ViewStats::default(),
        };
        let top_labels = self.draw_labels(target, area, &labels, LabelPass::OnTop, label_motion);
        let mut stats = world + viewmodel;
        stats.draws += depth_labels + top_labels;
        stats
    }
}
