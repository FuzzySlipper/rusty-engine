//! The fixed pass pipeline, in order: propagate dirty transforms, prepare GPU
//! rows (lights and their shadow views, parts) and rebuild the shadow caster
//! list when a part changed, then per view pass: take the layer's draw list
//! (culled and batched, reused while neither the camera nor any part
//! changed), render stale shadow maps before the first world pass, and encode
//! sky and world (or viewmodel) draws. `composition.rs` orders the view
//! passes. A world view pass with ambient occlusion first draws its opaque
//! batches depth-only and computes the occlusion (`ambient_occlusion.rs`),
//! in the same encoder before its render pass.

use std::ops::{Add, AddAssign};

use glam::{Mat4, Vec3};
use render_host_contracts::RendererCompositionCamera;
use render_model::{
    FogDescriptor, RenderHandle, RenderLayer, ToneMappingDescriptor, ToneMappingOperator,
};

use crate::apply::light_row;
use crate::batch::{self, DrawList, Frustum};
use crate::camera::CameraMatrices;
use crate::effects::EffectsPass;
use crate::shaders::Features;
use crate::shadows::{self, ShadowMaps};
use crate::tables::{Builtin, Environment, MaterialRef, NodeKind, PART_ROW_FLOATS};
use crate::target::{ColorTarget, TargetView};
use crate::{
    srgb_to_linear, OffscreenTarget, PresentSkip, Renderer, WindowSurface, DEFAULT_CLEAR_SRGB,
    NEUTRAL_GROUND_SRGB, NEUTRAL_HEMISPHERE_INTENSITY, NEUTRAL_KEY_INTENSITY, NEUTRAL_KEY_POSITION,
    NEUTRAL_VIEWMODEL_KEY_POSITION,
};

const LIGHT_ROW_FLOATS: usize = 16;
/// Frame uniform (`rusty::types` `Frame`): two matrices, camera position,
/// light count and first light; then exposure and fog distances, fog colour,
/// and the tone mapping and fog modes; then the presentation time.
const FRAME_UNIFORM_BYTES: u64 = (16 + 16 + 4 + 4 + 4 + 4 + 4 + 4) * 4;

/// Per-frame counts for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Draw calls: instanced batches and blended parts, across view passes.
    pub draws: u32,
    /// Parts those draws cover, after culling.
    pub instances: u32,
    pub parts_uploaded: u32,
    /// Part ids written to the instance buffer (0 when every pass's drawn
    /// set was unchanged).
    pub instances_uploaded: u32,
    pub lights: u32,
    /// Caster draw calls across shadow layers; 0 when the maps were current.
    pub shadow_draws: u32,
    /// Offscreen composition views drawn this frame; a target that is not
    /// stale is presented as it is.
    pub offscreen_views: u32,
    /// Sprite nodes sprite preparation examined, across view passes. It
    /// follows the sprite count, not the scene's node count.
    pub sprite_candidates: u32,
    /// A playing video clip covered the primary target.
    pub video: bool,
    /// Pipelines bound for part draws across view passes and shadow layers:
    /// one per run of draws sharing a pass and material features.
    pub pipeline_binds: u32,
    /// World and caster pipelines compiled while rendering; 0 when every
    /// material's pipelines were made at its definition.
    pub pipelines_created: u32,
}

/// What one view pass drew.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ViewStats {
    pub draws: u32,
    pub instances: u32,
    pub instances_uploaded: u32,
    pub shadow_draws: u32,
    pub sprite_candidates: u32,
    pub pipeline_binds: u32,
    pub pipelines_created: u32,
}

impl Add for ViewStats {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            draws: self.draws + other.draws,
            instances: self.instances + other.instances,
            instances_uploaded: self.instances_uploaded + other.instances_uploaded,
            shadow_draws: self.shadow_draws + other.shadow_draws,
            sprite_candidates: self.sprite_candidates + other.sprite_candidates,
            pipeline_binds: self.pipeline_binds + other.pipeline_binds,
            pipelines_created: self.pipelines_created + other.pipelines_created,
        }
    }
}

impl AddAssign<ViewStats> for FrameStats {
    fn add_assign(&mut self, view: ViewStats) {
        self.draws += view.draws;
        self.instances += view.instances;
        self.instances_uploaded += view.instances_uploaded;
        self.shadow_draws += view.shadow_draws;
        self.sprite_candidates += view.sprite_candidates;
        self.pipeline_binds += view.pipeline_binds;
        self.pipelines_created += view.pipelines_created;
    }
}

/// What a run of batches encoded.
#[derive(Clone, Copy, Default)]
struct Encoded {
    draws: u32,
    pipeline_binds: u32,
}

impl AddAssign for Encoded {
    fn add_assign(&mut self, other: Self) {
        self.draws += other.draws;
        self.pipeline_binds += other.pipeline_binds;
    }
}

/// The draw list a view layer last used, kept while nothing changed. Each
/// layer owns a region of the instance buffer after the caster list.
pub(crate) struct ViewCache {
    view_proj: Mat4,
    pub list: DrawList,
    /// A part moved or regrouped: re-cull, but keep the list to compare.
    stale: bool,
}

/// Which retained layers a view pass draws.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewLayer {
    /// Every layer but the viewmodel.
    World,
    /// Camera-relative content, after a depth break, with its own lights.
    Viewmodel,
}

/// Rows `first..first + count` of the lights buffer.
#[derive(Clone, Copy, Default)]
pub(crate) struct LightRange {
    pub first: u32,
    pub count: u32,
}

/// World lights, then viewmodel lights, in one buffer.
#[derive(Clone, Copy, Default)]
pub(crate) struct LightRanges {
    pub world: LightRange,
    pub viewmodel: LightRange,
}

/// A pixel rectangle with its origin at the top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub fn whole(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width: width.max(1),
            height: height.max(1),
        }
    }

    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

/// How a view pass begins on its target.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassStart {
    /// Clear the whole target: colour and depth for a world pass, depth only
    /// for a viewmodel pass.
    Target,
    /// Clear only the viewport, keeping what earlier passes drew elsewhere.
    Viewport,
}

pub(crate) struct ViewPass<'a> {
    pub target: TargetView<'a>,
    pub viewport: PixelRect,
    pub camera: CameraMatrices,
    pub layer: ViewLayer,
    pub start: PassStart,
    /// Linear background colour a world pass clears to.
    pub clear: [f32; 4],
    /// Draw the retained sky behind a world pass.
    pub sky: bool,
}

impl Renderer {
    /// Render the retained scene from `camera` into the whole offscreen
    /// target: the world, then the viewmodel layer. A composition renders
    /// through [`Renderer::render_view_composition`] instead.
    pub fn render_offscreen(
        &mut self,
        camera: &RendererCompositionCamera,
        target: &OffscreenTarget,
    ) -> FrameStats {
        let uploaded = self.prepare();
        let mut stats = self.render_camera(camera, target.view());
        stats.parts_uploaded = uploaded;
        stats
    }

    /// Render the retained scene from `camera` into the window and present it.
    pub fn render_to_surface(
        &mut self,
        camera: &RendererCompositionCamera,
        surface: &mut WindowSurface,
    ) -> Result<FrameStats, PresentSkip> {
        let uploaded = self.prepare();
        let gpu = self.gpu.clone();
        let mut stats = FrameStats::default();
        surface.present_with(&gpu, |view| stats = self.render_camera(camera, view))?;
        stats.parts_uploaded = uploaded;
        Ok(stats)
    }

    /// Advance what Engine time changes, without drawing: animation poses,
    /// with their completions and bounds inspections, and the end of a
    /// playing clip. A stream renderer that nobody watches calls this, so the
    /// Engine still learns of completions; the next draw catches the picture
    /// up.
    pub fn advance_undrawn(&mut self) {
        self.advance_animations();
        self.end_finished_video();
        self.propagate_transforms();
        self.report_pending_bounds();
        // Posing writes skinned vertices. Submit so those staged writes do
        // not pile up while nothing draws.
        self.gpu.queue.submit(std::iter::empty());
    }

    /// Bring GPU rows up to date with the tables. Returns rows uploaded.
    pub(crate) fn prepare(&mut self) -> u32 {
        self.advance_animations();
        self.advance_video();
        self.propagate_transforms();
        self.report_pending_bounds();
        if self.tables.lights_dirty {
            self.upload_lights();
            self.tables.lights_dirty = false;
        }
        if self.tables.environment_dirty {
            self.rebuild_sky();
            self.tables.environment_dirty = false;
        }
        let uploaded = self.upload_parts();
        let regrouped = std::mem::take(&mut self.tables.parts.regrouped);
        let moved = std::mem::take(&mut self.tables.parts.moved);
        if regrouped {
            // Casters lead the instance buffer; they upload again only if
            // their ids changed.
            let casters = batch::caster_list(&self.tables.parts, 0);
            if casters != self.casters {
                self.casters = casters;
                self.casters_uploaded = false;
            }
        }
        if regrouped || moved {
            // Each layer re-culls on its next pass and uploads only a
            // different list.
            for view in self.views.iter_mut().flatten() {
                view.stale = true;
            }
            self.shadows.stale = true;
        }
        uploaded
    }

    /// Recompute world transform, visibility and layer for each dirty subtree.
    fn propagate_transforms(&mut self) {
        let dirty: Vec<RenderHandle> = self.tables.dirty_nodes.drain().collect();
        let dirty_set: std::collections::HashSet<RenderHandle> = dirty.iter().copied().collect();
        for root in dirty {
            // An ancestor that is also dirty recomputes this subtree anyway.
            let mut ancestor = self.tables.nodes.get(&root).and_then(|node| node.parent);
            let mut covered = false;
            while let Some(handle) = ancestor {
                if dirty_set.contains(&handle) {
                    covered = true;
                    break;
                }
                ancestor = self.tables.nodes.get(&handle).and_then(|node| node.parent);
            }
            if !covered {
                self.update_subtree(root);
            }
        }
    }

    fn update_subtree(&mut self, root: RenderHandle) {
        let mut stack = vec![root];
        while let Some(handle) = stack.pop() {
            // A joint-attached child hangs from its parent's posed joint.
            let parent_state = self.tables.nodes.get(&handle).and_then(|node| {
                let parent_handle = node.parent?;
                let parent = self.tables.nodes.get(&parent_handle)?;
                let joint = node
                    .parent_joint_node
                    .and_then(|joint| self.joint_pose_at(parent_handle, joint))
                    .unwrap_or(glam::Mat4::IDENTITY);
                Some((
                    parent.world * joint,
                    parent.world_visible,
                    parent.world_layer,
                ))
            });
            let Some(node) = self.tables.nodes.get_mut(&handle) else {
                continue;
            };
            let (world, visible, layer) = match parent_state {
                Some((world, visible, layer)) => {
                    (world * node.local, visible && node.visible, layer)
                }
                None => (node.local, node.visible, node.layer),
            };
            node.world = world;
            node.world_visible = visible;
            node.world_layer = layer;
            if matches!(node.kind, NodeKind::Light(_)) {
                self.tables.lights_dirty = true;
            }
            let parts = node.parts.clone();
            stack.extend(node.children.iter().copied());
            for part in parts {
                self.tables.parts.write(part, &world, visible, layer);
            }
        }
    }

    /// World lights (the world rig and retained lights outside the viewmodel
    /// layer), then viewmodel lights (the viewmodel rig and retained lights in
    /// the viewmodel layer, in camera-local coordinates).
    fn upload_lights(&mut self) {
        let mut rows: Vec<f32> = Vec::new();
        if self.options.default_world_lights {
            neutral_rig(&mut rows, NEUTRAL_KEY_POSITION);
        }
        // Only world lights cast: viewmodel lights are camera-local.
        let mut shadow_views: Vec<Mat4> = Vec::new();
        self.retained_light_rows(&mut rows, ViewLayer::World, Some(&mut shadow_views));
        let world_count = (rows.len() / LIGHT_ROW_FLOATS) as u32;
        if self.options.default_viewmodel_lights {
            neutral_rig(&mut rows, NEUTRAL_VIEWMODEL_KEY_POSITION);
        }
        self.retained_light_rows(&mut rows, ViewLayer::Viewmodel, None);
        if self.shadows.set_layers(
            &self.gpu.device,
            &self.gpu.queue,
            &self.layouts.shadow_layer,
            &shadow_views,
        ) {
            self.rebind_frame();
        }
        let total = (rows.len() / LIGHT_ROW_FLOATS) as u32;
        self.lights = LightRanges {
            world: LightRange {
                first: 0,
                count: world_count,
            },
            viewmodel: LightRange {
                first: world_count,
                count: total - world_count,
            },
        };
        let needed = rows.len().max(LIGHT_ROW_FLOATS) * 4;
        if needed as u64 > self.lights_buffer.size() {
            self.lights_buffer = storage_buffer(
                &self.gpu.device,
                "render-wgpu lights",
                (needed as u64).next_power_of_two(),
            );
            self.rebind_frame();
        }
        if !rows.is_empty() {
            self.gpu
                .queue
                .write_buffer(&self.lights_buffer, 0, bytemuck::cast_slice(&rows));
        }
    }

    /// Retained light rows in `layer`. With `shadow_views`, a light whose
    /// shadow is requested (and enabled by the host) gets its shadow layers
    /// appended there and its row's `extra.w` set to the first layer + 1.
    fn retained_light_rows(
        &self,
        rows: &mut Vec<f32>,
        layer: ViewLayer,
        mut shadow_views: Option<&mut Vec<Mat4>>,
    ) {
        let mut handles: Vec<&RenderHandle> = self.tables.lights.iter().collect();
        handles.sort();
        for handle in handles {
            if let Some(node) = self.tables.nodes.get(handle) {
                let in_layer =
                    (node.world_layer == RenderLayer::Viewmodel) == (layer == ViewLayer::Viewmodel);
                if let (NodeKind::Light(light), true, true) =
                    (&node.kind, node.world_visible, in_layer)
                {
                    if let Some(mut row) = light_row(light, &node.world) {
                        if let (true, Some(views)) = (self.options.shadows, shadow_views.as_mut()) {
                            let layers = shadows::light_views(light, &node.world);
                            if !layers.is_empty() {
                                row[15] = (views.len() + 1) as f32;
                                views.extend(layers);
                            }
                        }
                        rows.extend_from_slice(&row);
                    }
                }
            }
        }
    }

    fn upload_parts(&mut self) -> u32 {
        let parts = &mut self.tables.parts;
        let row_bytes = (PART_ROW_FLOATS * 4) as u64;
        let needed = (parts.meta.len().max(1) as u64) * row_bytes;
        if parts.grown && needed > self.parts_buffer.size() {
            self.parts_buffer = storage_buffer(
                &self.gpu.device,
                "render-wgpu parts",
                needed.next_power_of_two(),
            );
            // A new buffer starts empty: upload every row once.
            parts.dirty.extend(0..parts.meta.len() as u32);
            parts.grown = false;
            self.rebind_frame();
        }
        let parts = &mut self.tables.parts;
        parts.grown = false;
        let mut dirty: Vec<u32> = parts.dirty.drain().collect();
        dirty.sort_unstable();
        let uploaded = dirty.len() as u32;
        // Coalesce consecutive rows into one write each.
        let mut index = 0;
        while index < dirty.len() {
            let start = dirty[index];
            let mut end = start;
            while index + 1 < dirty.len() && dirty[index + 1] == end + 1 {
                index += 1;
                end += 1;
            }
            let floats =
                &parts.gpu[start as usize * PART_ROW_FLOATS..(end as usize + 1) * PART_ROW_FLOATS];
            self.gpu.queue.write_buffer(
                &self.parts_buffer,
                u64::from(start) * row_bytes,
                bytemuck::cast_slice(floats),
            );
            index += 1;
        }
        uploaded
    }

    fn rebind_frame(&mut self) {
        self.frame_bind_group = frame_bind_group(
            &self.gpu.device,
            &self.layouts.frame,
            FrameBindings {
                frame: &self.frame_buffer,
                parts: &self.parts_buffer,
                lights: &self.lights_buffer,
                instances: &self.instances_buffer,
                shadows: &self.shadows,
            },
        );
        self.caster_bind_group = caster_bind_group(
            &self.gpu.device,
            &self.layouts.casters,
            &self.frame_buffer,
            &self.parts_buffer,
            &self.instances_buffer,
            &self.shadows,
        );
    }

    /// The layer's draw list for this camera, rebuilt when the camera or any
    /// part changed and uploaded only when the drawn set differs. Returns the
    /// part ids uploaded.
    fn update_view_list(&mut self, view_proj: &Mat4, eye: Vec3, layer: ViewLayer) -> u32 {
        let slot = layer as usize;
        if self.views[slot]
            .as_ref()
            .is_some_and(|view| view.view_proj == *view_proj && !view.stale)
        {
            return 0;
        }
        // Instance regions: casters, then the world list, then the viewmodel
        // list, each list sized for every part slot.
        let slots = self.tables.parts.meta.len() as u32;
        let base = self.casters.instances() + slot as u32 * slots;
        let list = batch::view_list(
            &self.tables.parts,
            layer == ViewLayer::Viewmodel,
            &Frustum::new(view_proj),
            eye,
            base,
        );
        let needed = u64::from(self.casters.instances() + 2 * slots).max(1) * 4;
        if needed > self.instances_buffer.size() {
            self.instances_buffer = storage_buffer(
                &self.gpu.device,
                "render-wgpu instances",
                needed.next_power_of_two(),
            );
            self.rebind_frame();
            // A new buffer starts empty: everything uploads again.
            self.views = Default::default();
            self.casters_uploaded = false;
        }
        let mut uploaded = 0;
        if !self.casters_uploaded {
            uploaded += self.upload_instances(0, &self.casters.ids);
            self.casters_uploaded = true;
        }
        // Lists carry their instance offsets, so a moved base compares
        // different and uploads.
        let unchanged = self.views[slot]
            .as_ref()
            .is_some_and(|view| view.list == list);
        if !unchanged {
            uploaded += self.upload_instances(base, &list.ids);
        }
        self.views[slot] = Some(ViewCache {
            view_proj: *view_proj,
            list,
            stale: false,
        });
        uploaded
    }

    fn upload_instances(&self, first: u32, ids: &[u32]) -> u32 {
        if !ids.is_empty() {
            self.gpu.queue.write_buffer(
                &self.instances_buffer,
                u64::from(first) * 4,
                bytemuck::cast_slice(ids),
            );
        }
        ids.len() as u32
    }

    /// Render every shadow layer's casters when a light or part changed since
    /// the maps were drawn, or presentation time moved and a product's caster
    /// stage (which may read it) draws. Returns what the casters encoded and
    /// the caster pipelines compiled for them.
    fn encode_shadows(&mut self, encoder: &mut wgpu::CommandEncoder) -> (Encoded, u32) {
        let retimed = self.shadows.timed && self.shadows.time != self.animation_time;
        if !(self.shadows.stale || retimed) || self.shadows.layers == 0 {
            return Default::default();
        }
        self.shadows.stale = false;
        self.shadows.time = self.animation_time;
        let variants = self.batch_variants(&self.casters.batches);
        self.shadows.timed = variants
            .iter()
            .any(|(features, _)| features.caster().product() != 0);
        let mut created = 0;
        for (features, pass) in variants {
            created += u32::from(
                self.layouts
                    .prepare_caster(&self.gpu.device, features, pass),
            );
        }
        let mut encoded = Encoded::default();
        for layer in 0..self.shadows.layers {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.shadows.layer_view(layer),
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
            pass.set_bind_group(0, &self.caster_bind_group, &[]);
            pass.set_bind_group(
                2,
                &self.shadows.layer_bind_group,
                &[ShadowMaps::layer_offset(layer)],
            );
            encoded += self.draw_batches(&mut pass, &self.casters.batches, |pass, features| {
                self.layouts.shadow.get(pass, features)
            });
        }
        (encoded, created)
    }

    /// The bind group and features a part's material draws with; a released
    /// material draws as the lit fallback.
    fn part_material(&self, material: &MaterialRef) -> (&wgpu::BindGroup, Features) {
        let fallback = (&self.lit_fallback_material, Features::default());
        match material {
            MaterialRef::Retained(id) => self
                .tables
                .materials
                .get(*id)
                .map_or(fallback, |row| (&row.bind_group, row.features)),
            MaterialRef::Unlit => (&self.unlit_material, Features::UNLIT),
            MaterialRef::LitFallback => fallback,
        }
    }

    /// The (features, pass) pairs a draw list's batches draw with.
    fn batch_variants(&self, batches: &[batch::Batch]) -> Vec<(Features, batch::Pass)> {
        let mut variants: Vec<(Features, batch::Pass)> = batches
            .iter()
            .filter_map(|draw| {
                let part = self.tables.parts.meta[draw.part as usize].as_ref()?;
                let mesh = self.mesh(&part.mesh)?;
                Some((
                    self.part_material(&part.material).1 | mesh.features(),
                    draw.pass,
                ))
            })
            .collect();
        variants.sort_unstable();
        variants.dedup();
        variants
    }

    /// Encode a draw list's batches with the pipeline each pass and material
    /// feature set selects.
    fn draw_batches<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'_>,
        batches: &[batch::Batch],
        pipeline: impl Fn(batch::Pass, Features) -> &'a wgpu::RenderPipeline,
    ) -> Encoded {
        let mut current: Option<(batch::Pass, Features)> = None;
        let mut encoded = Encoded::default();
        for draw in batches {
            let Some(part) = self.tables.parts.meta[draw.part as usize].as_ref() else {
                continue;
            };
            let Some(mesh) = self.mesh(&part.mesh) else {
                continue;
            };
            let (material, features) = self.part_material(&part.material);
            let features = features | mesh.features();
            if current != Some((draw.pass, features)) {
                pass.set_pipeline(pipeline(draw.pass, features));
                current = Some((draw.pass, features));
                encoded.pipeline_binds += 1;
            }
            // A wireframe part draws its triangles' edges: two edge indices
            // per triangle index.
            let (indices, range) = if part.wireframe {
                let Some(edges) = mesh.edges.get() else {
                    continue;
                };
                let (first, count) = (part.first_index * 2, part.index_count * 2);
                (edges, first..first + count)
            } else {
                let (first, count) = (part.first_index, part.index_count);
                (&mesh.indices, first..first + count)
            };
            pass.set_bind_group(1, material, &[]);
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            if let Some(extra) = &mesh.extra {
                pass.set_vertex_buffer(1, extra.slice(..));
            }
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(
                range,
                0,
                draw.first_instance..draw.first_instance + draw.instances,
            );
            encoded.draws += 1;
        }
        encoded
    }

    /// Blended parts and blended sprites in one order: render order (parts
    /// are 0), then back to front.
    /// A blended surface writes no depth, so drawing either family as a
    /// block would let whatever draws second cover the other.
    fn draw_blended<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        parts: &[batch::Batch],
        effects: &EffectsPass,
        eye: Vec3,
        pipeline: impl Fn(batch::Pass, Features) -> &'a wgpu::RenderPipeline + Copy,
    ) -> Encoded {
        let part_depth = |batch: &batch::Batch| {
            let bounds = &self.tables.parts.state[batch.part as usize].world_bounds;
            ((bounds.min + bounds.max) * 0.5).distance_squared(eye)
        };
        let (mut part, mut sprite, mut encoded) = (0, 0, Encoded::default());
        loop {
            let part_first = match (parts.get(part), effects.blended_key(sprite)) {
                (None, None) => break,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some(batch), Some((order, depth))) => {
                    order > 0 || (order == 0 && part_depth(batch) >= depth)
                }
            };
            if part_first {
                encoded += self.draw_batches(pass, &parts[part..part + 1], pipeline);
                part += 1;
            } else {
                self.effects
                    .draw_blended_sprite(pass, format, effects, sprite);
                sprite += 1;
            }
        }
        encoded
    }

    fn rebuild_sky(&mut self) {
        self.sky_bind_group = None;
        let Environment::Sky(sky) = &self.tables.environment else {
            return;
        };
        let Some(first) = self.tables.textures.get(&sky.texture) else {
            return;
        };
        let (second, amount) = match &sky.blend {
            Some(blend) => match self.tables.textures.get(&blend.texture) {
                Some(second) => (second, blend.amount),
                None => (first, 0.0),
            },
            None => (first, 0.0),
        };
        let mut uniform = [0u8; 16];
        uniform[0..4].copy_from_slice(&amount.to_le_bytes());
        use wgpu::util::DeviceExt;
        let buffer = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render-wgpu sky"),
                contents: &uniform,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        self.sky_bind_group = Some(
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("render-wgpu sky"),
                    layout: &self.layouts.sky,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&first.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&second.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&first.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::Sampler(&second.sampler),
                        },
                    ],
                }),
        );
    }

    /// The linear colour a world pass clears to: the retained background
    /// colour, or the Engine default behind a sky or with nothing selected.
    pub(crate) fn environment_clear(&self) -> [f32; 4] {
        match &self.tables.environment {
            Environment::Color(color) => *color,
            _ => {
                let rgb = DEFAULT_CLEAR_SRGB.map(srgb_to_linear);
                [rgb[0], rgb[1], rgb[2], 1.0]
            }
        }
    }

    /// Encode and submit one view pass. Each pass writes the frame uniform
    /// and submits on its own, so passes with different cameras never share
    /// one uniform write.
    pub(crate) fn encode_view(&mut self, view: ViewPass<'_>) -> ViewStats {
        let world_layer = view.layer == ViewLayer::World;
        let lights = if world_layer {
            self.lights.world
        } else {
            self.lights.viewmodel
        };
        let view_proj = view.camera.view_proj;
        let eye = view.camera.eye;
        let instances_uploaded = self.update_view_list(&view_proj, eye, view.layer);
        let effects = self.prepare_effects(&view);
        let slot = view.layer as usize;
        if !world_layer
            && effects.draws() == 0
            && self.views[slot]
                .as_ref()
                .is_none_or(|cache| cache.list.batches.is_empty())
        {
            return ViewStats {
                instances_uploaded,
                sprite_candidates: effects.sprite_candidates,
                ..ViewStats::default()
            };
        }
        let mut uniform = Vec::with_capacity((FRAME_UNIFORM_BYTES / 4) as usize);
        uniform.extend_from_slice(&view_proj.to_cols_array());
        uniform.extend_from_slice(&view_proj.inverse().to_cols_array());
        uniform.extend_from_slice(&[eye.x, eye.y, eye.z, 1.0]);
        let mut bytes: Vec<u8> = bytemuck::cast_slice(&uniform).to_vec();
        for count in [lights.count, lights.first, 0, 0] {
            bytes.extend_from_slice(&count.to_le_bytes());
        }
        bytes.extend_from_slice(&finish_uniform(self.tables.tone_mapping, self.tables.fog));
        // The Engine presentation time (`set_animation_time`): it holds while
        // the simulation does, so a held frame draws the same.
        for value in [self.animation_time as f32, 0.0, 0.0, 0.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        self.gpu.queue.write_buffer(&self.frame_buffer, 0, &bytes);

        // Format and sample count: every pipeline drawing here must match.
        let format = view.target.key();
        let format_index = match self.pipelines.iter().position(|set| set.target == format) {
            Some(index) => index,
            None => {
                self.pipelines
                    .push(self.layouts.pipelines(&self.gpu.device, format));
                self.pipelines.len() - 1
            }
        };
        let variants = self.batch_variants(
            &self.views[slot]
                .as_ref()
                .expect("view list is current")
                .list
                .batches,
        );
        let mut pipelines_created = 0;
        for (features, pass) in variants {
            pipelines_created += u32::from(self.layouts.prepare(
                &self.gpu.device,
                &mut self.pipelines[format_index],
                features,
                pass,
            ));
        }
        // A world view's occlusion: its depth pre-pass draws the opaque
        // batches with the pre-pass pipelines, which compile like the
        // caster pipelines.
        let occlusion = if world_layer {
            self.ambient_occlusion.begin_view(
                &self.gpu,
                self.options.ambient_occlusion,
                (view.target.width, view.target.height),
                view.viewport,
                &view.camera,
            )
        } else {
            None
        };
        let prepass_batches: Vec<batch::Batch> = if occlusion.is_some() {
            self.views[slot]
                .as_ref()
                .expect("view list is current")
                .list
                .batches
                .iter()
                .copied()
                .filter(|draw| draw.pass < batch::Pass::Lines)
                .collect()
        } else {
            Vec::new()
        };
        for (features, pass) in self.batch_variants(&prepass_batches) {
            pipelines_created += u32::from(self.layouts.prepare_prepass(
                &self.gpu.device,
                features,
                pass,
            ));
        }
        let whole = view.start == PassStart::Target;
        if !whole {
            self.compose.prepare_clear(&self.gpu, format, view.clear);
        }
        let color_load = if whole && world_layer {
            let [r, g, b, a] = view.clear.map(f64::from);
            wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a })
        } else {
            wgpu::LoadOp::Load
        };
        let depth_load = if whole {
            wgpu::LoadOp::Clear(1.0)
        } else {
            wgpu::LoadOp::Load
        };
        if world_layer {
            // Ghost plates snap per view; a view is known by its viewport.
            let area = view.viewport;
            let key = (u64::from(area.x) << 48)
                ^ (u64::from(area.y) << 32)
                ^ (u64::from(area.width) << 16)
                ^ u64::from(area.height);
            self.select_ghost_sectors(eye, key, format);
        }
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu view"),
            });
        let (shadows, casters_created) = if world_layer {
            self.encode_shadows(&mut encoder)
        } else {
            Default::default()
        };
        if let Some(occlusion) = &occlusion {
            self.shadows.write_camera(&self.gpu.queue, &view_proj);
            let camera_offset = ShadowMaps::layer_offset(self.shadows.camera_slot());
            self.ambient_occlusion
                .encode_prepass(&mut encoder, occlusion, |pass| {
                    pass.set_bind_group(0, &self.caster_bind_group, &[]);
                    pass.set_bind_group(2, &self.shadows.layer_bind_group, &[camera_offset]);
                    self.draw_batches(pass, &prepass_batches, |pass, features| {
                        self.layouts.prepass.get(pass, features)
                    });
                });
            self.ambient_occlusion.encode(&mut encoder, occlusion);
        }
        let pipelines = &self.pipelines[format_index];
        let list = &self.views[slot]
            .as_ref()
            .expect("view list is current")
            .list;
        let parts;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if world_layer {
                    "render-wgpu world"
                } else {
                    "render-wgpu viewmodel"
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view.target.color,
                    depth_slice: None,
                    // A multisampled primary resolves at the end of every
                    // pass; later passes load the multisampled colour.
                    resolve_target: view.target.resolve,
                    ops: wgpu::Operations {
                        load: color_load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: view.target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: depth_load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let area = view.viewport;
            pass.set_viewport(
                area.x as f32,
                area.y as f32,
                area.width as f32,
                area.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(area.x, area.y, area.width, area.height);
            if !whole {
                self.compose.clear_viewport(&mut pass, format, world_layer);
            }
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            let occlusion_bind_group = self.ambient_occlusion.apply_bind_group(occlusion.as_ref());
            pass.set_bind_group(2, occlusion_bind_group, &[]);
            if let (true, true, Some(sky)) = (world_layer, view.sky, &self.sky_bind_group) {
                pass.set_pipeline(&pipelines.sky);
                pass.set_bind_group(1, sky, &[]);
                pass.draw(0..3, 0..1);
            }
            // Solid sprites draw between the world's opaque and blended parts.
            let batches = &list.batches;
            let blend_start = batches
                .iter()
                .position(|batch| batch.pass >= batch::Pass::Blend)
                .unwrap_or(batches.len());
            let mut encoded =
                self.draw_batches(&mut pass, &batches[..blend_start], |pass, features| {
                    pipelines.get(pass, features)
                });
            if world_layer {
                encoded.draws += self.draw_ghost_plates(&mut pass, format);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_bind_group(2, occlusion_bind_group, &[]);
            }
            self.effects.draw_solid_sprites(&mut pass, format, &effects);
            encoded += self.draw_blended(
                &mut pass,
                format,
                &batches[blend_start..],
                &effects,
                eye,
                |pass, features| pipelines.get(pass, features),
            );
            self.effects.draw_particles(
                &mut pass,
                format,
                &effects,
                self.builtins.get(&Builtin::Cube),
            );
            parts = encoded;
        }
        self.gpu.queue.submit([encoder.finish()]);
        if occlusion.is_some() {
            self.ambient_occlusion.submitted();
        }
        ViewStats {
            draws: parts.draws + effects.draws(),
            instances: list.instances(),
            instances_uploaded,
            shadow_draws: shadows.draws,
            sprite_candidates: effects.sprite_candidates,
            pipeline_binds: parts.pipeline_binds + shadows.pipeline_binds,
            pipelines_created: pipelines_created + casters_created,
        }
    }
}

pub(crate) fn storage_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(crate) fn frame_uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu frame"),
        size: FRAME_UNIFORM_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(crate) struct FrameBindings<'a> {
    pub frame: &'a wgpu::Buffer,
    pub parts: &'a wgpu::Buffer,
    pub lights: &'a wgpu::Buffer,
    pub instances: &'a wgpu::Buffer,
    pub shadows: &'a ShadowMaps,
}

pub(crate) fn frame_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    bindings: FrameBindings<'_>,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("render-wgpu frame"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: bindings.frame.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: bindings.parts.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: bindings.lights.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: bindings.instances.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&bindings.shadows.array_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&bindings.shadows.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: bindings.shadows.matrices_buffer.as_entire_binding(),
            },
        ],
    })
}

pub(crate) fn caster_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    frame: &wgpu::Buffer,
    parts: &wgpu::Buffer,
    instances: &wgpu::Buffer,
    shadows: &ShadowMaps,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("render-wgpu casters"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: parts.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: instances.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: shadows.matrices_buffer.as_entire_binding(),
            },
        ],
    })
}

/// The neutral rig: a hemisphere light and a key light shining from
/// `key_position` toward the origin.
fn neutral_rig(rows: &mut Vec<f32>, key_position: [f32; 3]) {
    let sky = [NEUTRAL_HEMISPHERE_INTENSITY; 3];
    let ground = NEUTRAL_GROUND_SRGB.map(|c| srgb_to_linear(c) * NEUTRAL_HEMISPHERE_INTENSITY);
    rows.extend_from_slice(&[
        sky[0], sky[1], sky[2], 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, ground[0], ground[1],
        ground[2], 0.0,
    ]);
    let travel = -crate::convert::vec3(key_position).normalize();
    rows.extend_from_slice(&[
        NEUTRAL_KEY_INTENSITY,
        NEUTRAL_KEY_INTENSITY,
        NEUTRAL_KEY_INTENSITY,
        2.0,
        0.0,
        0.0,
        0.0,
        0.0,
        travel.x,
        travel.y,
        travel.z,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ]);
}

/// The frame uniform's finish rows: exposure and fog distances, fog colour,
/// then the tone mapping and fog modes (`rusty::finish`).
fn finish_uniform(tone_mapping: ToneMappingDescriptor, fog: Option<FogDescriptor>) -> Vec<u8> {
    let operator: u32 = match tone_mapping.operator {
        ToneMappingOperator::None => 0,
        ToneMappingOperator::Neutral => 1,
        ToneMappingOperator::AcesFilmic => 2,
    };
    let (mode, start, end, density): (u32, f32, f32, f32) = match fog {
        None => (0, 0.0, 0.0, 0.0),
        Some(FogDescriptor::Linear { start, end, .. }) => (1, start, end, 0.0),
        Some(FogDescriptor::Exponential { density, .. }) => (2, 0.0, 0.0, density),
        Some(FogDescriptor::ExponentialSquared { density, .. }) => (3, 0.0, 0.0, density),
    };
    let [r, g, b] = fog.map_or([0.0; 3], |fog| fog.color());
    let mut bytes = Vec::with_capacity(48);
    for value in [tone_mapping.exposure, start, end, density, r, g, b, 0.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [operator, mode, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}
