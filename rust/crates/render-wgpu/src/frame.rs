//! The fixed pass pipeline, in order: propagate dirty transforms, prepare GPU
//! rows (lights and their shadow views, parts) and cull each shadow layer's
//! casters when a part or layer changed, then per view pass: take the layer's
//! draw list (culled and batched, reused while neither the camera nor any
//! part changed), render stale shadow layers before the first world pass, and
//! encode sky and world (or viewmodel) draws. `composition.rs` orders the
//! view passes.
//! A world view pass with ambient occlusion first draws its opaque batches
//! depth-only and computes the occlusion (`ambient_occlusion.rs`), in the
//! same encoder before its render pass.

use std::collections::HashSet;
use std::ops::{Add, AddAssign};

use glam::{Mat4, Vec3};
use render_host_contracts::RendererCompositionCamera;
use render_model::{
    ColorGradingDescriptor, FogDescriptor, LightDescriptor, RenderHandle, RenderLayer,
    ToneMappingDescriptor, ToneMappingOperator,
};

use crate::apply::light_row;
use crate::batch::{self, DrawList, Frustum};
use crate::camera::CameraMatrices;
use crate::culling::CandidateList;
use crate::distance_fields::{world_box, FieldEntry};
use crate::effects::EffectsPass;
use crate::finish::{FinishPost, HDR_FORMAT};
use crate::light_clusters::{ClusterUniform, CLUSTER_CAPACITY};
use crate::pipelines::{Layouts, Pipelines};
use crate::shaders::Features;
use crate::shadows::{self, ShadowMaps};
use crate::tables::{Builtin, Environment, MaterialRef, NodeKind, PartId, PART_ROW_FLOATS};
use crate::target::{ColorTarget, TargetView};
use crate::{
    srgb_to_linear, AmbientOcclusionPath, OffscreenTarget, PresentSkip, Renderer, WindowSurface,
    DEFAULT_CLEAR_SRGB, NEUTRAL_GROUND_SRGB, NEUTRAL_HEMISPHERE_INTENSITY, NEUTRAL_KEY_INTENSITY,
    NEUTRAL_KEY_POSITION, NEUTRAL_VIEWMODEL_KEY_POSITION,
};

const LIGHT_ROW_FLOATS: usize = 16;
/// Frame uniform (`rusty::types` `Frame`): two matrices, camera position,
/// light count and first light; then exposure and fog distances, fog colour,
/// and the tone mapping and fog modes; then the presentation time and the
/// colour grading; then the light cluster grid and depth range.
const FRAME_UNIFORM_BYTES: u64 = (16 + 16 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4) * 4;
/// Instance regions before the shadow casters: a list and a visible region
/// for each of the world and viewmodel layers (`update_view_list`).
const VIEW_REGIONS: u32 = 4;

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
    /// Shadow layers rendered, and the casters drawn into them.
    pub shadow_layers: u32,
    pub shadow_casters: u32,
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
    pub shadow_layers: u32,
    pub shadow_casters: u32,
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
            shadow_layers: self.shadow_layers + other.shadow_layers,
            shadow_casters: self.shadow_casters + other.shadow_casters,
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
        self.shadow_layers += view.shadow_layers;
        self.shadow_casters += view.shadow_casters;
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

/// What the shadow layers rendered this frame.
#[derive(Clone, Copy, Default)]
struct ShadowsEncoded {
    encoded: Encoded,
    layers: u32,
    casters: u32,
    /// Caster pipelines compiled for them.
    pipelines_created: u32,
}

/// The draw list a view layer last used, kept while nothing changed. Each
/// layer owns two regions of the instance buffer after the caster list: its
/// list (or, with GPU culling, its opaque candidates) and, with GPU culling,
/// the visible runs the cull writes.
pub(crate) struct ViewCache {
    view_proj: Mat4,
    /// The CPU list; with GPU culling, only the blended parts.
    pub list: DrawList,
    /// With GPU culling: the opaque candidates and their indirect arguments.
    pub candidates: Option<CandidateList>,
    /// A part moved or regrouped: re-cull, but keep the list to compare.
    stale: bool,
    /// A part regrouped: the candidates must be rebuilt.
    regrouped: bool,
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
        self.shadows_chosen = false;
        self.shadows_rendered = (0, 0);
        self.finish.begin_frame(&self.gpu);
        self.exposure_adapted = false;
        let mut layers_changed = false;
        if self.tables.lights_dirty {
            layers_changed = self.upload_lights();
            self.tables.lights_dirty = false;
        }
        if self.tables.environment_dirty {
            self.rebuild_sky();
            self.tables.environment_dirty = false;
        }
        let uploaded = self.upload_parts();
        let regrouped = std::mem::take(&mut self.tables.parts.regrouped);
        let moved = std::mem::take(&mut self.tables.parts.moved);
        if regrouped || !moved.is_empty() {
            // Each layer re-culls on its next pass and uploads only a
            // different list.
            for view in self.views.iter_mut().flatten() {
                view.stale = true;
                view.regrouped |= regrouped;
            }
        }
        if regrouped || !moved.is_empty() || layers_changed {
            self.cull_casters(&moved, false);
        }
        uploaded
    }

    /// Cull shadow layers' casters to their views and their lights' reach:
    /// every layer, or only the cascades after a fit. A layer whose casters
    /// differ, or hold a moved part, is stale; the others keep their maps.
    fn cull_casters(&mut self, moved: &HashSet<PartId>, cascades_only: bool) {
        if self.shadows.layers.is_empty() {
            return;
        }
        let parts = &self.tables.parts;
        let candidates = batch::caster_candidates(parts);
        // A point light's six faces share one reach.
        let mut reached: Option<((Vec3, f32), Vec<PartId>)> = None;
        // Casters follow the world and viewmodel lists in the instance
        // buffer: fixed layers' first, then cascades', so a fit moves no
        // fixed layer's list.
        let mut base = VIEW_REGIONS * parts.meta.len() as u32;
        let mut changed = false;
        for cascades in [false, true] {
            for layer in &mut self.shadows.layers {
                if layer.is_cascade() != cascades {
                    continue;
                }
                if cascades_only && !cascades {
                    base += layer.casters.instances();
                    continue;
                }
                let inside = match layer.reach() {
                    _ if !layer.has_view() => &Vec::new(),
                    None => &candidates,
                    Some(reach) => {
                        if reached.as_ref().is_none_or(|(at, _)| *at != reach) {
                            reached = Some((reach, batch::within_reach(parts, &candidates, reach)));
                        }
                        &reached.as_ref().expect("reach culled").1
                    }
                };
                let list = batch::caster_list(parts, inside, &Frustum::new(&layer.view_proj), base);
                base += list.instances();
                if list.ids != layer.casters.ids || list.ids.iter().any(|id| moved.contains(id)) {
                    layer.stale = true;
                }
                if list != layer.casters {
                    layer.casters = list;
                    changed = true;
                }
            }
        }
        if changed {
            self.casters_uploaded = false;
        }
        self.reserve_instances();
    }

    /// Fit each directional light's cascades to a world view's camera, and
    /// give its light row the split depths and the view axis receivers pick
    /// a cascade by. A cascade whose view changed is culled again and
    /// re-renders.
    fn fit_cascades(&mut self, camera: &CameraMatrices) {
        let (near, far) = shadows::view_depths(&camera.projection);
        let forward = -camera.view.row(2).truncate();
        let mut fitted = false;
        for index in 0..self.shadows.layers.len() {
            let shadows::LayerSource::Cascade {
                direction,
                distance,
                index: cascade,
                row,
                settings,
            } = self.shadows.layers[index].source
            else {
                continue;
            };
            let splits = shadows::cascade_splits(near, far.min(distance));
            let cascade = cascade as usize;
            let from = if cascade == 0 {
                near
            } else {
                splits[cascade - 1]
            };
            let view_proj =
                shadows::cascade_view(direction, camera, from, splits[cascade], settings.size);
            let layer = &mut self.shadows.layers[index];
            if layer.view_proj != view_proj {
                layer.view_proj = view_proj;
                layer.stale = true;
                self.shadows.write_view(&self.gpu.queue, index);
                fitted = true;
            }
            if cascade == 0 {
                // A directional row's position_range and extra.xyz
                // (`rusty::types::Light`).
                let at = u64::from(row) * LIGHT_ROW_FLOATS as u64 * 4;
                self.gpu.queue.write_buffer(
                    &self.lights_buffer,
                    at + 16,
                    bytemuck::cast_slice(&splits),
                );
                self.gpu.queue.write_buffer(
                    &self.lights_buffer,
                    at + 48,
                    bytemuck::cast_slice(&forward.to_array()),
                );
            }
        }
        if fitted {
            self.cull_casters(&HashSet::new(), true);
        }
    }

    /// Grow the instance buffer to hold the view regions (each sized for
    /// every part slot) and the shadow layers' casters. A new buffer starts
    /// empty, so everything uploads again.
    fn reserve_instances(&mut self) {
        let slots = self.tables.parts.meta.len() as u32;
        let needed = u64::from(VIEW_REGIONS * slots + self.shadows.caster_instances()).max(1) * 4;
        if needed > self.instances_buffer.size() {
            self.instances_buffer = storage_buffer(
                &self.gpu.device,
                "render-wgpu instances",
                needed.next_power_of_two(),
            );
            self.rebind_frame();
            self.culling.invalidate();
            self.views = Default::default();
            self.casters_uploaded = false;
        }
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
    /// the viewmodel layer, in camera-local coordinates). Returns whether a
    /// shadow layer's view changed.
    fn upload_lights(&mut self) -> bool {
        let mut rows: Vec<f32> = Vec::new();
        if self.options.default_world_lights {
            neutral_rig(&mut rows, NEUTRAL_KEY_POSITION);
        }
        // Only world lights cast: viewmodel lights are camera-local.
        let mut candidates = Vec::new();
        self.retained_light_rows(&mut rows, ViewLayer::World, Some(&mut candidates));
        let world_count = (rows.len() / LIGHT_ROW_FLOATS) as u32;
        // The lights every fragment sees (`light_clusters.wgsl` is_global):
        // ambient, hemisphere and directional rows, and point or spot rows
        // without a range. The clustered path names at most
        // `CLUSTER_CAPACITY` of them, so a pass with more loops.
        self.global_lights = rows
            .as_chunks::<LIGHT_ROW_FLOATS>()
            .0
            .iter()
            .filter(|row| row[3] < 3.0 || row[7] <= 0.0)
            .count() as u32;
        if self.options.default_viewmodel_lights {
            neutral_rig(&mut rows, NEUTRAL_VIEWMODEL_KEY_POSITION);
        }
        self.retained_light_rows(&mut rows, ViewLayer::Viewmodel, None);
        self.shadow_candidates = candidates;
        let layers_changed = self.choose_shadows(Some(&mut rows));
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
            self.light_clusters.invalidate();
            self.rebind_frame();
        }
        if !rows.is_empty() {
            self.gpu
                .queue
                .write_buffer(&self.lights_buffer, 0, bytemuck::cast_slice(&rows));
        }
        layers_changed
    }

    /// Choose which shadow candidates cast (`shadows::choose`, from the
    /// last world view's eye) and give the casting lights' layers to the
    /// atlas. Each candidate's light row gets its first layer + 1 in
    /// `extra.w`, or 0: in `rows` while they are being built, else written
    /// to the lights buffer, only when the choice changed. Returns whether
    /// a layer changed.
    fn choose_shadows(&mut self, rows: Option<&mut Vec<f32>>) -> bool {
        let chosen = shadows::choose(
            &self.shadow_candidates,
            self.options.shadow_budget,
            self.shadow_eye,
            &self.casting,
        );
        let casting: HashSet<RenderHandle> = self
            .shadow_candidates
            .iter()
            .zip(&chosen)
            .filter(|(_, chosen)| **chosen)
            .map(|(candidate, _)| candidate.light)
            .collect();
        if rows.is_none() && casting == self.casting {
            return false;
        }
        let mut sources = Vec::new();
        let mut firsts = Vec::with_capacity(chosen.len());
        for (candidate, chosen) in self.shadow_candidates.iter().zip(&chosen) {
            firsts.push(if *chosen {
                sources.extend(candidate.layers.iter().copied());
                (sources.len() - candidate.layers.len() + 1) as f32
            } else {
                0.0
            });
        }
        match rows {
            Some(rows) => {
                for (candidate, first) in self.shadow_candidates.iter().zip(&firsts) {
                    rows[candidate.row as usize * LIGHT_ROW_FLOATS + 15] = *first;
                }
            }
            None => {
                for (candidate, first) in self.shadow_candidates.iter().zip(&firsts) {
                    let at = (candidate.row as usize * LIGHT_ROW_FLOATS + 15) * 4;
                    self.gpu.queue.write_buffer(
                        &self.lights_buffer,
                        at as u64,
                        bytemuck::cast_slice(&[*first]),
                    );
                }
            }
        }
        self.casting = casting;
        let (replaced, changed) = self.shadows.set_layers(
            &self.gpu.device,
            &self.gpu.queue,
            &self.layouts.shadow_layer,
            &sources,
        );
        if replaced {
            self.rebind_frame();
        }
        changed
    }

    /// The resident distance fields of the shown scene-layer payload meshes,
    /// in world space, for a world view's cone trace.
    fn field_entries(&self) -> Vec<FieldEntry> {
        let mut entries = Vec::new();
        for (handle, mesh) in &self.tables.payload_meshes {
            let Some(slot) = mesh.distance_field.as_ref().and_then(|field| field.slot) else {
                continue;
            };
            let Some(node) = self.tables.nodes.get(handle) else {
                continue;
            };
            if !node.world_visible || node.world_layer != RenderLayer::Scene {
                continue;
            }
            let field = &mesh
                .distance_field
                .as_ref()
                .expect("checked above")
                .field_box;
            let (min, max) = world_box(&node.world, field);
            entries.push(FieldEntry { min, max, slot });
        }
        entries
    }

    /// Retained light rows in `layer`. With `candidates`, a light whose
    /// shadow is requested (and enabled by the host) becomes a candidate to
    /// cast; `choose_shadows` sets its row's `extra.w`.
    fn retained_light_rows(
        &self,
        rows: &mut Vec<f32>,
        layer: ViewLayer,
        mut candidates: Option<&mut Vec<shadows::ShadowCandidate>>,
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
                    if let Some(row) = light_row(light, &node.world) {
                        if let (true, Some(candidates)) =
                            (self.options.shadows, candidates.as_mut())
                        {
                            let index = (rows.len() / LIGHT_ROW_FLOATS) as u32;
                            let layers = shadows::light_layers(light, &node.world, index);
                            if !layers.is_empty() {
                                candidates.push(shadows::ShadowCandidate {
                                    light: *handle,
                                    row: index,
                                    layers,
                                    priority: light.shadow_settings().priority,
                                    position: match light {
                                        LightDescriptor::Point { position, .. }
                                        | LightDescriptor::Spot { position, .. } => Some(
                                            node.world
                                                .transform_point3(crate::convert::vec3(*position)),
                                        ),
                                        _ => None,
                                    },
                                });
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
        // The GPU cull reads world bounds beside the rows; a replaced bounds
        // buffer, or one that fell behind while the CPU culled, takes every
        // slot again.
        if !self.options.gpu_culling {
            self.culling.bounds_stale();
        } else {
            let slots = parts.meta.len() as u32;
            if self.culling.reserve_bounds(&self.gpu.device, slots) {
                for slot in 0..slots {
                    self.culling.write_bounds(
                        &self.gpu.queue,
                        slot,
                        &parts.state[slot as usize].world_bounds,
                    );
                }
            } else {
                for slot in &dirty {
                    self.culling.write_bounds(
                        &self.gpu.queue,
                        *slot,
                        &parts.state[*slot as usize].world_bounds,
                    );
                }
            }
        }
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
                clusters: &self.light_clusters.clusters,
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
        // Instance regions: per layer (world, viewmodel) a list region and a
        // visible region, each sized for every part slot, then each shadow
        // layer's casters. The CPU list uses the list region; GPU culling
        // puts the opaque candidates there, the blended parts after them, and
        // their visible runs in the visible region.
        let slots = self.tables.parts.meta.len() as u32;
        self.reserve_instances();
        let mut uploaded = 0;
        if !self.casters_uploaded {
            // In the order `cull_casters` placed them.
            let ids: Vec<u32> = [false, true]
                .iter()
                .flat_map(|&cascades| {
                    self.shadows
                        .layers
                        .iter()
                        .filter(move |layer| layer.is_cascade() == cascades)
                })
                .flat_map(|layer| layer.casters.ids.iter().copied())
                .collect();
            uploaded += self.upload_instances(VIEW_REGIONS * slots, &ids);
            self.casters_uploaded = true;
        }
        let gpu_culled = self.options.gpu_culling && self.culling.available();
        if self.views[slot].as_ref().is_some_and(|view| {
            view.view_proj == *view_proj && !view.stale && view.candidates.is_some() == gpu_culled
        }) {
            return uploaded;
        }
        let base = 2 * slot as u32 * slots;
        let visible_base = base + slots;
        let viewmodel = layer == ViewLayer::Viewmodel;
        let frustum = Frustum::new(view_proj);
        let previous = self.views[slot].take();
        let (list, candidates) = if gpu_culled {
            // The candidates survive a camera move and a part move; they
            // follow a regroup.
            let candidates = match previous {
                Some(ViewCache {
                    candidates: Some(candidates),
                    regrouped: false,
                    ..
                }) => candidates,
                _ => {
                    let list = batch::opaque_candidates(&self.tables.parts, viewmodel, base);
                    uploaded += self.upload_instances(base, &list.ids);
                    let candidates = CandidateList::new(list, visible_base, |batch| {
                        let part = self.tables.parts.meta[batch.part as usize].as_ref()?;
                        self.mesh(&part.mesh)?;
                        Some(if part.wireframe {
                            (part.first_index * 2, part.index_count * 2)
                        } else {
                            (part.first_index, part.index_count)
                        })
                    });
                    self.culling
                        .upload_candidates(&self.gpu, layer, &candidates);
                    candidates
                }
            };
            // Blended parts follow the candidates in the list region.
            let blended_base = base + candidates.list.instances();
            let list =
                batch::blended_list(&self.tables.parts, viewmodel, &frustum, eye, blended_base);
            uploaded += self.upload_instances(blended_base, &list.ids);
            (list, Some(candidates))
        } else {
            let list = batch::view_list(&self.tables.parts, viewmodel, &frustum, eye, base);
            // Lists carry their instance offsets, so a moved base compares
            // different and uploads.
            let unchanged = previous.as_ref().is_some_and(|view| view.list == list);
            if !unchanged {
                uploaded += self.upload_instances(base, &list.ids);
            }
            (list, None)
        };
        self.views[slot] = Some(ViewCache {
            view_proj: *view_proj,
            list,
            candidates,
            stale: false,
            regrouped: false,
        });
        uploaded
    }

    /// Draw a candidate list's batches from the GPU cull's arguments: one
    /// indirect draw per batch, or one multi-draw per run of batches sharing
    /// a pipeline, material and mesh. Returns what was encoded and the
    /// multi-draws issued.
    fn draw_batches_indirect<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'_>,
        layer: ViewLayer,
        candidates: &CandidateList,
        pipeline: impl Fn(batch::Pass, Features) -> &'a wgpu::RenderPipeline,
    ) -> (Encoded, u32) {
        let args = self.culling.args(layer);
        let batches = &candidates.list.batches;
        let mut current: Option<(batch::Pass, Features)> = None;
        let mut encoded = Encoded::default();
        let mut multi_draws = 0;
        let mut index = 0;
        while index < batches.len() {
            let draw = &batches[index];
            let Some(part) = self.tables.parts.meta[draw.part as usize].as_ref() else {
                index += 1;
                continue;
            };
            let Some(mesh) = self.mesh(&part.mesh) else {
                index += 1;
                continue;
            };
            let (material, features) = self.part_material(&part.material);
            let features = features | mesh.features();
            if current != Some((draw.pass, features)) {
                pass.set_pipeline(pipeline(draw.pass, features));
                current = Some((draw.pass, features));
                encoded.pipeline_binds += 1;
            }
            let indices = if part.wireframe {
                match mesh.edges.get() {
                    Some(edges) => edges,
                    None => {
                        index += 1;
                        continue;
                    }
                }
            } else {
                &mesh.indices
            };
            pass.set_bind_group(1, material, &[]);
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            if let Some(extra) = &mesh.extra {
                pass.set_vertex_buffer(1, extra.slice(..));
            }
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            // Following batches with the same pipeline, material and mesh
            // draw in the same call.
            let mut run = 1;
            if self.culling.multi_draw() {
                while index + run < batches.len() {
                    let next = &batches[index + run];
                    let same = self.tables.parts.meta[next.part as usize]
                        .as_ref()
                        .is_some_and(|next_part| {
                            next.pass == draw.pass
                                && next_part.wireframe == part.wireframe
                                && next_part.mesh == part.mesh
                                && self.mesh(&next_part.mesh).is_some_and(|next_mesh| {
                                    std::ptr::eq(next_mesh, mesh)
                                        && features
                                            == (self.part_material(&next_part.material).1
                                                | next_mesh.features())
                                })
                                && std::ptr::eq(self.part_material(&next_part.material).0, material)
                        });
                    if !same {
                        break;
                    }
                    run += 1;
                }
            }
            let offset = index as u64 * 20;
            if run > 1 {
                pass.multi_draw_indexed_indirect(args, offset, run as u32);
                multi_draws += 1;
            } else {
                pass.draw_indexed_indirect(args, offset);
            }
            encoded.draws += run as u32;
            index += run;
        }
        (encoded, multi_draws)
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

    /// Render each stale shadow layer's casters, or every layer when
    /// presentation time moved and a product's caster stage (which may read
    /// it) draws.
    fn encode_shadows(&mut self, encoder: &mut wgpu::CommandEncoder) -> ShadowsEncoded {
        let retimed = self.shadows.timed && self.shadows.time != self.animation_time;
        if !retimed && self.shadows.layers.iter().all(|layer| !layer.stale) {
            return Default::default();
        }
        self.shadows.time = self.animation_time;
        let batches: Vec<batch::Batch> = self
            .shadows
            .layers
            .iter()
            .flat_map(|layer| layer.casters.batches.iter().copied())
            .collect();
        let variants = self.batch_variants(&batches);
        self.shadows.timed = variants
            .iter()
            .any(|(features, _)| features.caster().product() != 0);
        let mut drawn = ShadowsEncoded::default();
        for (features, pass) in variants {
            drawn.pipelines_created += u32::from(self.layouts.prepare_caster(
                &self.gpu.device,
                features,
                pass,
            ));
        }
        for (index, layer) in self.shadows.layers.iter().enumerate() {
            if !(retimed || layer.stale) {
                continue;
            }
            drawn.layers += 1;
            drawn.casters += layer.casters.instances();
            let index = index as u32;
            let tile = layer.tile;
            // A tile shares its page with others' maps: it clears only its
            // own square.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.shadows.page_view(tile.page),
                    depth_ops: Some(wgpu::Operations {
                        load: if tile.is_page() {
                            wgpu::LoadOp::Clear(1.0)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let size = tile.size as f32;
            pass.set_viewport(tile.x as f32, tile.y as f32, size, size, 0.0, 1.0);
            pass.set_scissor_rect(tile.x, tile.y, tile.size, tile.size);
            if !tile.is_page() {
                pass.set_pipeline(&self.layouts.shadow_clear);
                pass.draw(0..3, 0..1);
            }
            pass.set_bind_group(0, &self.caster_bind_group, &[]);
            pass.set_bind_group(
                2,
                &self.shadows.layer_bind_group,
                &[ShadowMaps::layer_offset(index)],
            );
            drawn.encoded +=
                self.draw_batches(&mut pass, &layer.casters.batches, |pass, features| {
                    self.layouts.shadow.get(pass, features)
                });
        }
        for layer in &mut self.shadows.layers {
            layer.stale = false;
        }
        self.shadows_rendered.0 += drawn.layers;
        self.shadows_rendered.1 += drawn.casters;
        drawn
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
        if world_layer {
            // A budget chooses casting lights near the first world view of
            // each frame.
            if self.options.shadow_budget.is_some() && !self.shadows_chosen {
                self.shadows_chosen = true;
                self.shadow_eye = eye;
                if self.choose_shadows(None) {
                    self.cull_casters(&HashSet::new(), false);
                }
            }
            self.fit_cascades(&view.camera);
        }
        let instances_uploaded = self.update_view_list(&view_proj, eye, view.layer);
        // The world, sprites and particles draw into the view's HDR target;
        // the background and the finished world into the view's own.
        let target = view.target.key();
        let hdr_format = ColorTarget {
            format: HDR_FORMAT,
            samples: target.samples,
        };
        let effects = self.prepare_effects(&view, hdr_format);
        let slot = view.layer as usize;
        // With GPU culling the opaque parts are candidates, not in the list.
        if !world_layer
            && effects.draws() == 0
            && self.views[slot].as_ref().is_none_or(|cache| {
                cache.list.batches.is_empty()
                    && cache
                        .candidates
                        .as_ref()
                        .is_none_or(|candidates| candidates.list.batches.is_empty())
            })
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
        bytes.extend_from_slice(&finish_uniform(
            self.tables.tone_mapping,
            self.tables.fog,
            self.tables.color_grading.is_some(),
        ));
        // The Engine presentation time (`set_animation_time`): it holds while
        // the simulation does, so a held frame draws the same.
        for value in [self.animation_time as f32, 0.0, 0.0, 0.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in grading_uniform(self.tables.color_grading.unwrap_or_default()) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        // The cluster fields follow once the view's clusters are encoded.
        let cluster_offset = bytes.len() as u64;
        self.gpu.queue.write_buffer(&self.frame_buffer, 0, &bytes);

        // Format and sample count: every pipeline drawing here must match.
        let pipeline_set = |pipelines: &mut Vec<Pipelines>, layouts: &Layouts, key| match pipelines
            .iter()
            .position(|set| set.target == key)
        {
            Some(index) => index,
            None => {
                pipelines.push(layouts.pipelines(&self.gpu.device, key));
                pipelines.len() - 1
            }
        };
        let format_index = pipeline_set(&mut self.pipelines, &self.layouts, hdr_format);
        let sky_index = pipeline_set(&mut self.pipelines, &self.layouts, target);
        let cache = self.views[slot].as_ref().expect("view list is current");
        let mut variants = self.batch_variants(&cache.list.batches);
        if let Some(candidates) = &cache.candidates {
            variants.extend(self.batch_variants(&candidates.list.batches));
        }
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
                &self.distance_fields,
            )
        } else {
            None
        };
        let culled = self.views[slot]
            .as_ref()
            .expect("view list is current")
            .candidates
            .is_some();
        let prepass_batches: Vec<batch::Batch> = if occlusion.is_some() {
            let cache = self.views[slot].as_ref().expect("view list is current");
            cache
                .candidates
                .as_ref()
                .map_or(&cache.list, |candidates| &candidates.list)
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
        if !whole && world_layer {
            self.compose.prepare_clear(&self.gpu, target, view.clear);
        }
        if world_layer {
            // Ghost plates snap per view; a view is known by its viewport.
            let area = view.viewport;
            let key = (u64::from(area.x) << 48)
                ^ (u64::from(area.y) << 32)
                ^ (u64::from(area.width) << 16)
                ^ u64::from(area.height);
            self.select_ghost_sectors(eye, key, hdr_format);
        }
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu view"),
            });
        let shadows = if world_layer {
            self.encode_shadows(&mut encoder)
        } else {
            Default::default()
        };
        // The GPU cull writes the opaque visible runs and indirect arguments
        // every pass, before anything draws them.
        if culled {
            let candidates = self.views[slot]
                .as_ref()
                .and_then(|cache| cache.candidates.as_ref())
                .expect("culled view has candidates");
            self.culling.encode(
                &self.gpu,
                &mut encoder,
                &self.instances_buffer,
                view.layer,
                candidates,
                &view_proj,
            );
        } else {
            self.culling.skipped();
        }
        if let Some(occlusion) = &occlusion {
            self.shadows.write_camera(&self.gpu.queue, &view_proj);
            let camera_offset = ShadowMaps::layer_offset(self.shadows.camera_slot());
            self.ambient_occlusion
                .encode_prepass(&mut encoder, occlusion, |pass| {
                    pass.set_bind_group(0, &self.caster_bind_group, &[]);
                    pass.set_bind_group(2, &self.shadows.layer_bind_group, &[camera_offset]);
                    match self.views[slot]
                        .as_ref()
                        .and_then(|cache| cache.candidates.as_ref())
                    {
                        Some(candidates) => {
                            self.draw_batches_indirect(
                                pass,
                                view.layer,
                                candidates,
                                |pass, features| self.layouts.prepass.get(pass, features),
                            );
                        }
                        None => {
                            self.draw_batches(pass, &prepass_batches, |pass, features| {
                                self.layouts.prepass.get(pass, features)
                            });
                        }
                    }
                });
            if occlusion.path() == AmbientOcclusionPath::DistanceField {
                let entries = self.field_entries();
                self.distance_fields.begin_view(
                    &self.gpu,
                    &view.camera,
                    occlusion.region(),
                    &entries,
                );
            }
            self.ambient_occlusion.encode(
                &self.gpu,
                &mut encoder,
                occlusion,
                &self.distance_fields,
            );
        }
        let area = view.viewport;
        let in_viewport = |pass: &mut wgpu::RenderPass<'_>| {
            pass.set_viewport(
                area.x as f32,
                area.y as f32,
                area.width as f32,
                area.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(area.x, area.y, area.width, area.height);
        };
        if world_layer {
            // The background: the clear colour, then the sky. It is never
            // finished.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view.target.color,
                    depth_slice: None,
                    // The view's finish pass, which composites the world
                    // over this, resolves a multisampled target.
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if whole {
                            let [r, g, b, a] = view.clear.map(f64::from);
                            wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a })
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: view.target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: if whole {
                            wgpu::LoadOp::Clear(1.0)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            in_viewport(&mut pass);
            if !whole {
                self.compose.clear_viewport(&mut pass, target, true);
            }
            if let (true, Some(sky)) = (view.sky, &self.sky_bind_group) {
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_pipeline(&self.pipelines[sky_index].sky);
                pass.set_bind_group(1, sky, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        // Bloom spreads the world's light; auto exposure adapts at the first
        // world view of a frame. Both read the world resolved.
        let bloom = self
            .tables
            .bloom
            .filter(|bloom| world_layer && bloom.intensity > 0.0);
        let auto_exposure = self.tables.auto_exposure;
        if auto_exposure.is_none() {
            self.finish.post.reset_exposure();
        }
        let adapting = auto_exposure.filter(|_| world_layer && !self.exposure_adapted);
        let size = (view.target.width, view.target.height, target.samples);
        let hdr = self.finish.target(
            &self.gpu,
            size.0,
            size.1,
            size.2,
            bloom.is_some() || adapting.is_some(),
        );
        // A world pass's lights are binned into clusters when the host asks
        // and the device can; the viewmodel's few lights loop.
        let clusters = if world_layer && self.options.clustered_lighting {
            if self.global_lights > CLUSTER_CAPACITY {
                self.light_clusters.looped(self.global_lights);
                ClusterUniform::LOOP
            } else {
                self.light_clusters.encode(
                    &self.gpu,
                    &mut encoder,
                    &self.lights_buffer,
                    &view.camera,
                    lights,
                )
            }
        } else {
            if world_layer {
                self.light_clusters.skipped();
            }
            ClusterUniform::LOOP
        };
        let mut cluster_bytes = Vec::with_capacity(32);
        for value in clusters.grid {
            cluster_bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in clusters.depth {
            cluster_bytes.extend_from_slice(&value.to_le_bytes());
        }
        self.gpu
            .queue
            .write_buffer(&self.frame_buffer, cluster_offset, &cluster_bytes);
        let pipelines = &self.pipelines[format_index];
        let cache = self.views[slot].as_ref().expect("view list is current");
        let list = &cache.list;
        let candidates = cache.candidates.as_ref();
        let mut multi_draws = 0;
        let parts;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if world_layer {
                    "render-wgpu world"
                } else {
                    "render-wgpu viewmodel"
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &hdr.color,
                    depth_slice: None,
                    resolve_target: hdr.resolve.as_ref(),
                    ops: wgpu::Operations {
                        load: if whole {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: view.target.depth,
                    depth_ops: Some(wgpu::Operations {
                        // A world pass's depth was cleared with its
                        // background; a viewmodel pass breaks depth here.
                        load: if whole && !world_layer {
                            wgpu::LoadOp::Clear(1.0)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.finish.world_writes(world_layer),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            in_viewport(&mut pass);
            if !whole {
                // A viewport of a shared HDR target clears with a triangle,
                // depth included (again, for a world pass).
                self.finish
                    .clear_viewport(&self.gpu.device, &mut pass, target.samples);
            }
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            let occlusion_bind_group = self.ambient_occlusion.apply_bind_group(occlusion.as_ref());
            pass.set_bind_group(2, occlusion_bind_group, &[]);
            // Solid sprites draw between the world's opaque and blended parts.
            let batches = &list.batches;
            let blend_start = batches
                .iter()
                .position(|batch| batch.pass >= batch::Pass::Blend)
                .unwrap_or(batches.len());
            let mut encoded = match candidates {
                Some(candidates) => {
                    let (encoded, runs) = self.draw_batches_indirect(
                        &mut pass,
                        view.layer,
                        candidates,
                        |pass, features| pipelines.get(pass, features),
                    );
                    multi_draws = runs;
                    encoded
                }
                None => self.draw_batches(&mut pass, &batches[..blend_start], |pass, features| {
                    pipelines.get(pass, features)
                }),
            };
            if world_layer {
                encoded.draws += self.draw_ghost_plates(&mut pass, hdr_format);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
            }
            // Blended parts are not in the pre-pass's depth, so the occlusion
            // under them belongs to what they cover: they draw without it.
            if occlusion.is_some() {
                pass.set_bind_group(2, self.ambient_occlusion.apply_bind_group(None), &[]);
            }
            self.effects
                .draw_solid_sprites(&mut pass, hdr_format, &effects);
            encoded += self.draw_blended(
                &mut pass,
                hdr_format,
                &batches[blend_start..],
                &effects,
                eye,
                |pass, features| pipelines.get(pass, features),
            );
            self.effects.draw_particles(
                &mut pass,
                hdr_format,
                &effects,
                self.builtins.get(&Builtin::Cube),
            );
            parts = encoded;
        }
        if world_layer {
            self.finish.resolve_world(&mut encoder);
        }
        let draws = parts.draws + effects.draws();
        if bloom.is_some() || adapting.is_some() {
            self.exposure_adapted |= adapting.is_some();
            self.finish.encode_post(
                &self.gpu,
                &mut encoder,
                size,
                bloom,
                adapting,
                self.animation_time,
                world_layer,
            );
        }
        self.finish.encode(
            &self.gpu,
            &mut encoder,
            &view.target,
            [area.x, area.y, area.width, area.height],
            &self.frame_buffer,
            FinishPost {
                bloom: bloom.map_or(0.0, |bloom| bloom.intensity),
                auto_exposure: auto_exposure.is_some(),
            },
            world_layer,
        );
        self.gpu.queue.submit([encoder.finish()]);
        if occlusion.is_some() {
            self.ambient_occlusion.submitted();
        }
        self.finish.submitted();
        if clusters.grid[3] == 1 {
            self.light_clusters.submitted();
        }
        if culled {
            self.culling.drew(multi_draws);
            self.culling.submitted();
        }
        let instances =
            list.instances() + candidates.map_or(0, |candidates| candidates.list.instances());
        ViewStats {
            draws,
            instances,
            instances_uploaded,
            shadow_draws: shadows.encoded.draws,
            shadow_layers: shadows.layers,
            shadow_casters: shadows.casters,
            sprite_candidates: effects.sprite_candidates,
            pipeline_binds: parts.pipeline_binds + shadows.encoded.pipeline_binds,
            pipelines_created: pipelines_created + shadows.pipelines_created,
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
    pub clusters: &'a wgpu::Buffer,
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
                resource: bindings.shadows.views_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: bindings.clusters.as_entire_binding(),
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
                resource: shadows.views_buffer.as_entire_binding(),
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
/// then the tone mapping, fog and grading modes (`rusty::finish`).
fn finish_uniform(
    tone_mapping: ToneMappingDescriptor,
    fog: Option<FogDescriptor>,
    graded: bool,
) -> Vec<u8> {
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
    for value in [operator, mode, u32::from(graded), 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// The frame uniform's grading rows (`rusty::finish::graded`): the white
/// point's LMS scales, then the contrast exponent about middle grey and the
/// saturation factor.
fn grading_uniform(grading: ColorGradingDescriptor) -> [f32; 8] {
    let [l, m, s] = white_balance(grading.temperature, grading.tint);
    [
        l,
        m,
        s,
        0.0,
        1.0 + grading.contrast,
        1.0 + grading.saturation,
        0.0,
        0.0,
    ]
}

/// The LMS scales that move the D65 white point by `temperature` and
/// `tint` (each -1 to 1), as Unity's colour balance does: a warmer
/// temperature takes a bluer reference white, a positive tint a greener
/// one.
fn white_balance(temperature: f32, tint: f32) -> [f32; 3] {
    // CIE xy to LMS (CAT02), for a white of luminance 1.
    let lms = |x: f32, y: f32| {
        let (big_x, big_z) = (x / y, (1.0 - x - y) / y);
        [
            0.7328 * big_x + 0.4296 - 0.1624 * big_z,
            -0.7036 * big_x + 1.6975 + 0.0061 * big_z,
            0.0030 * big_x + 0.0136 + 0.9834 * big_z,
        ]
    };
    let (t1, t2) = (temperature * 100.0 / 65.0, tint * 100.0 / 65.0);
    // D65's x, moved along the daylight locus, and its y off it by the tint.
    let x = 0.31271 - t1 * if t1 < 0.0 { 0.1 } else { 0.05 };
    let y = 2.87 * x - 3.0 * x * x - 0.275_095_07 + t2 * 0.05;
    let reference = lms(x, y);
    let d65 = [0.949_237, 1.035_42, 1.087_28];
    [0, 1, 2].map(|i| d65[i] / reference[i])
}

#[cfg(test)]
mod tests {
    use super::white_balance;

    #[test]
    fn a_neutral_white_balance_leaves_white_alone_and_warmth_raises_long_over_short() {
        for scale in white_balance(0.0, 0.0) {
            assert!((scale - 1.0).abs() < 2e-3, "{scale}");
        }
        let [long, _, short] = white_balance(0.5, 0.0);
        assert!(long > 1.0 && short < 1.0, "{long} {short}");
        let [long, _, short] = white_balance(-0.5, 0.0);
        assert!(long < 1.0 && short > 1.0, "{long} {short}");
    }
}
