//! Animated meshes: GLB assets, instances, playback on the Engine timeline,
//! skinning, joint attachments, and the facts the runtime reads back.
//!
//! **Time.** Poses advance on the Engine presentation timeline the host sets
//! with [`Renderer::set_animation_time`] (`PresentationWorld`'s elapsed
//! seconds), never on a display clock. Direct playback is the retained
//! [`AnimatedMeshPlaybackTimeline`], so a fresh renderer given the baseline's
//! snapshot command lands on the same pose. Fades are a renderer-local ramp
//! over the same timeline.
//!
//! **Cost.** A pose is evaluated only when it can change: while a clip plays
//! or fades, or after a command. Rigid (unskinned) GLB nodes upload their
//! geometry once per asset and move by their part's local matrix; skinned
//! primitives are skinned on the CPU into the instance's own vertex buffer.
//! A held, sampled or stopped pose costs nothing per frame.
//!
//! **Controllers** (`PresentationOp::Animation`) publish the Engine's
//! controller state with per-clip phases each update; a controlled instance
//! samples `frozen_pose` of that state, each clip advanced by its speed over
//! the Engine time since the update. A controller replaces direct playback on
//! its target until destroyed.
//!
//! **Blending** follows Three's mixer: each property takes its first
//! contributing clip, later clips mix in by `w / (Σw + w)` (slerp for
//! rotations), and a total weight below 1 mixes toward the rest pose.

use std::collections::{BTreeMap, HashMap};

use glam::{Mat4, Quat, Vec3};
use render_model::{
    AnimatedMeshAsset, AnimatedMeshClipPose, AnimatedMeshInspection,
    AnimatedMeshInstanceDescriptor, AnimatedMeshPlaybackCommand, AnimatedMeshPlaybackTimeline,
    AnimationLoopMode, MaterialAlphaModeDescriptor, MaterialUvStrategy, RenderHandle,
    RenderMaterialDescriptor,
};

use render_presentation::{AnimationControllerProjectionState, AnimationProjectionOp};

use crate::apply::MaterialParams;
use crate::glb::{self, GlbAlpha, GlbClip, GlbModel, Path, Trs};
use crate::pipelines::VERTEX_FLOATS;
use crate::resources::ResourceSource;
use crate::tables::{Aabb, GpuMesh, MaterialRef, NodeKind, Topology};
use crate::Renderer;

/// What the runtime reads back from animated meshes (the Three lane's
/// animation feedback facts), keyed by the instance's source entity and its
/// per-entity realization generation.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationFact {
    /// A `loop: once` clip played to its end.
    NaturalCompletion {
        object_id: u64,
        generation: u64,
        clip: String,
    },
    /// Posed world bounds for a changed `bounds_request`; `None` when the
    /// instance draws nothing.
    MeshInspection {
        object_id: u64,
        generation: u64,
        request: u32,
        bounds: Option<([f32; 3], [f32; 3])>,
    },
}

/// An animation controller projected onto an animated instance.
pub(crate) struct ControllerRow {
    target: RenderHandle,
    state: AnimationControllerProjectionState,
    /// The Engine time this state was received at.
    received_at: f64,
}

impl ControllerRow {
    /// The weighted clip times at `now`: the published phases advanced by
    /// each clip's speed since the update.
    fn pose(&self, now: f64) -> Vec<AnimatedMeshClipPose> {
        let elapsed = (now - self.received_at).max(0.0);
        let mut state = self.state.clone();
        let speed = |clip: &str| {
            std::iter::once(&self.state.motion)
                .chain(
                    self.state
                        .transition
                        .iter()
                        .map(|transition| &transition.target_motion),
                )
                .find(|motion| motion.clip_a == clip || motion.clip_b.as_deref() == Some(clip))
                .map_or(1.0, |motion| f64::from(motion.speed_milli) / 1000.0)
        };
        for phase in &mut state.clip_phases {
            phase.time_seconds += elapsed * speed(&phase.clip);
        }
        match state.frozen_pose(self.state.phase_seconds + elapsed) {
            AnimatedMeshPlaybackCommand::SamplePose { clips } => clips,
            _ => Vec::new(),
        }
    }
}

/// One admitted animated-mesh asset.
pub(crate) struct AnimatedAssetRow {
    model: GlbModel,
    clips: HashMap<String, GlbClip>,
    /// Uploaded unskinned primitives, by (mesh, primitive).
    rigid: HashMap<(u32, u32), GpuMesh>,
    /// Retained material id per GLB material.
    materials: Vec<String>,
    textures: Vec<String>,
    /// Engine slot to GLB material index.
    slots: BTreeMap<u16, usize>,
    /// Joint (bone) names that resolve to exactly one node.
    joints: HashMap<String, usize>,
}

/// One direct-playback state, kept as it fades out.
#[derive(Clone)]
struct Playback {
    timeline: AnimatedMeshPlaybackTimeline,
    /// A frozen `SamplePose`: clips with their times and weights.
    pose: Option<Vec<AnimatedMeshClipPose>>,
}

struct Fade {
    prior: Playback,
    start: f64,
    duration: f64,
    /// A cross-fade also ramps the new clip in; a stop only fades out.
    cross: bool,
}

pub(crate) struct AnimatedInstance {
    asset: String,
    object_id: Option<u64>,
    generation: u64,
    playback: Playback,
    fade: Option<Fade>,
    /// The `once` clip whose end is reported, until any new command.
    completion: Option<String>,
    inspection: AnimatedMeshInspection,
    bounds_pending: bool,
    /// Node transforms in instance space, per GLB node.
    pose: Vec<Mat4>,
    pose_dirty: bool,
    /// The Engine time the pose was evaluated at.
    posed_at: f64,
    /// Per-instance skinned vertex buffers, by (node, primitive).
    skinned: HashMap<(u32, u32), GpuMesh>,
    /// The GLB node each of the node row's parts draws, in part order.
    part_nodes: Vec<usize>,
}

/// Resource identity of an admitted GLB (`sha256:<hex>` names `<kind>/<hex>`).
fn resource_identity(kind: &str, content_hash: Option<&str>) -> Option<String> {
    let hash = content_hash?;
    Some(format!(
        "{kind}/{}",
        hash.strip_prefix("sha256:").unwrap_or(hash)
    ))
}

impl Renderer {
    /// The Engine presentation timeline in seconds (`PresentationWorld`'s
    /// elapsed time). Animated poses advance on it, not on display time.
    pub fn set_animation_time(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds >= 0.0 {
            self.animation_time = seconds;
        }
    }

    /// Apply one animation controller op (`PresentationOp::Animation`).
    pub(crate) fn apply_animation_op(&mut self, op: &AnimationProjectionOp) -> Result<(), String> {
        let now = self.animation_time;
        let target = match op {
            AnimationProjectionOp::Create { handle, descriptor } => {
                self.tables.controllers.insert(
                    handle.raw(),
                    ControllerRow {
                        target: descriptor.target,
                        state: descriptor.controller.clone(),
                        received_at: now,
                    },
                );
                descriptor.target
            }
            AnimationProjectionOp::Update { handle, controller } => {
                let row = self
                    .tables
                    .controllers
                    .get_mut(&handle.raw())
                    .ok_or_else(|| format!("unknown animation controller {}", handle.raw()))?;
                row.state = controller.clone();
                row.received_at = now;
                row.target
            }
            AnimationProjectionOp::Destroy { handle } => {
                let row = self
                    .tables
                    .controllers
                    .remove(&handle.raw())
                    .ok_or_else(|| format!("unknown animation controller {}", handle.raw()))?;
                row.target
            }
        };
        if let Some(instance) = self.tables.animated.get_mut(&target) {
            instance.pose_dirty = true;
        }
        Ok(())
    }

    /// Facts gathered since the last call, oldest first.
    pub fn take_animation_facts(&mut self) -> Vec<AnimationFact> {
        std::mem::take(&mut self.animation_facts)
    }

    pub(crate) fn define_animated_mesh(
        &mut self,
        asset: &AnimatedMeshAsset,
        resources: &dyn ResourceSource,
    ) -> Result<(), String> {
        let identity = resource_identity("animated-mesh-resource", asset.content_hash.as_deref())
            .ok_or_else(|| format!("{} has no content hash", asset.asset))?;
        let bytes = resources
            .bytes(&identity)
            .ok_or_else(|| format!("resource {identity} is not available"))?;
        let model = glb::decode(&bytes)?;
        let mut clips = HashMap::new();
        let names = |model: &GlbModel| -> HashMap<String, usize> {
            model
                .clips
                .iter()
                .enumerate()
                .filter_map(|(index, clip)| clip.name.clone().map(|name| (name, index)))
                .collect()
        };
        let mut own = decode_clips(&model);
        let own_names = names(&model);
        for descriptor in &asset.clips {
            let name = descriptor.name.as_deref().unwrap_or(&descriptor.id);
            if let Some(clip) = own_names.get(name).and_then(|index| own[*index].take()) {
                clips.insert(descriptor.id.clone(), clip);
            }
        }
        // Clip packs share the rig; their channels bind by node name.
        let by_name: HashMap<&str, usize> = model
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| node.name.as_deref().map(|name| (name, index)))
            .collect();
        for pack in &asset.clip_packs {
            let identity =
                resource_identity("clip-pack-resource", Some(pack.content_hash.as_str()))
                    .ok_or_else(|| format!("clip pack {} has no content hash", pack.asset))?;
            let bytes = resources
                .bytes(&identity)
                .ok_or_else(|| format!("resource {identity} is not available"))?;
            let pack_model = glb::decode(&bytes)?;
            let pack_names = names(&pack_model);
            let mut pack_clips = decode_clips(&pack_model);
            for descriptor in &pack.clips {
                let name = descriptor.name.as_deref().unwrap_or(&descriptor.id);
                let Some(mut clip) = pack_names
                    .get(name)
                    .and_then(|index| pack_clips[*index].take())
                else {
                    continue;
                };
                clip.channels.retain_mut(|channel| {
                    let target = pack_model.nodes[channel.node]
                        .name
                        .as_deref()
                        .and_then(|name| by_name.get(name));
                    match target {
                        Some(node) => {
                            channel.node = *node;
                            true
                        }
                        None => false,
                    }
                });
                clips.insert(descriptor.id.clone(), clip);
            }
        }

        // Decoding is done and nothing below fails: retire the previous
        // definition now, before its asset-derived texture and material ids
        // are reused, so its cleanup cannot remove the replacements.
        self.release_animated_mesh_resources(&asset.asset);
        let label = asset.asset.clone();
        let textures: Vec<String> = (0..model.textures.len())
            .map(|index| format!("{label}#texture/{index}"))
            .collect();
        for (id, texture) in textures.iter().zip(&model.textures) {
            let uploaded = match &texture.image {
                Some(image) => crate::apply::upload_rgba_texture(
                    &self.gpu,
                    id,
                    image.width,
                    image.height,
                    &image.rgba,
                    true,
                    texture.nearest,
                    texture.repeat,
                ),
                None => crate::apply::upload_rgba_texture(
                    &self.gpu, id, 1, 1, &[255; 4], true, true, false,
                ),
            };
            self.tables.textures.insert(id.clone(), uploaded);
        }
        let materials: Vec<String> = (0..model.materials.len())
            .map(|index| format!("{label}#material/{index}"))
            .collect();
        for (id, material) in materials.iter().zip(&model.materials) {
            let texture = material
                .base_color_texture
                .map(|index| textures[index].clone());
            let descriptor = RenderMaterialDescriptor {
                schema_version: 1,
                id: id.clone(),
                color: material.base_color,
                texture,
                roughness: material.roughness,
                texture_tint: [1.0; 4],
                emission_color: material.emissive,
                emission_intensity: 1.0,
                uv_strategy: MaterialUvStrategy::Flat,
                alpha_mode: match material.alpha {
                    GlbAlpha::Opaque => MaterialAlphaModeDescriptor::Opaque,
                    GlbAlpha::Mask(cutoff) => MaterialAlphaModeDescriptor::Mask { cutoff },
                    GlbAlpha::Blend => MaterialAlphaModeDescriptor::Blend,
                },
                double_sided: material.double_sided,
                voxel_surface: None,
            };
            self.define_material_with(descriptor, material.metallic, material.unlit);
        }

        let mut rigid = HashMap::new();
        for (mesh_index, primitives) in model.meshes.iter().enumerate() {
            for (primitive_index, primitive) in primitives.iter().enumerate() {
                if primitive.joints.is_some() {
                    continue;
                }
                let vertices = interleave(primitive, &primitive.positions, &primitive.normals);
                let mesh = self.upload_vertices(
                    &format!("{label} mesh {mesh_index}/{primitive_index}"),
                    &vertices,
                    &primitive.indices,
                    Topology::Triangles,
                    vec![(0, 0, primitive.indices.len() as u32)],
                    BTreeMap::new(),
                );
                rigid.insert((mesh_index as u32, primitive_index as u32), mesh);
            }
        }
        let mut joint_counts: HashMap<String, Vec<usize>> = HashMap::new();
        for skin in &model.skins {
            for joint in &skin.joints {
                if let Some(name) = &model.nodes[*joint].name {
                    let entry = joint_counts.entry(name.clone()).or_default();
                    if !entry.contains(joint) {
                        entry.push(*joint);
                    }
                }
            }
        }
        let joints = joint_counts
            .into_iter()
            .filter_map(|(name, nodes)| (nodes.len() == 1).then(|| (name, nodes[0])))
            .collect();
        let slots = asset
            .embedded_material_slots
            .iter()
            .map(|slot| (slot.slot, usize::from(slot.source_material_slot)))
            .collect();
        self.tables.animated_assets.insert(
            asset.asset.clone(),
            AnimatedAssetRow {
                model,
                clips,
                rigid,
                materials,
                textures,
                slots,
                joints,
            },
        );
        // Live instances (a redefinition keeps them) re-pose on the new asset.
        let instances: Vec<RenderHandle> = self
            .tables
            .animated
            .iter()
            .filter(|(_, instance)| instance.asset == asset.asset)
            .map(|(handle, _)| *handle)
            .collect();
        for handle in instances {
            self.reset_animated_instance(handle);
        }
        Ok(())
    }

    pub(crate) fn release_animated_mesh(&mut self, asset: &str) {
        self.release_animated_mesh_resources(asset);
        let instances: Vec<RenderHandle> = self
            .tables
            .animated
            .iter()
            .filter(|(_, instance)| instance.asset == asset)
            .map(|(handle, _)| *handle)
            .collect();
        for handle in instances {
            self.reset_animated_instance(handle);
        }
    }

    fn release_animated_mesh_resources(&mut self, asset: &str) {
        if let Some(row) = self.tables.animated_assets.remove(asset) {
            for id in row.materials {
                self.tables.materials.remove(&id);
            }
            for id in row.textures {
                self.tables.textures.remove(&id);
            }
        }
    }

    /// Start (or restart) an instance's realization after its node row
    /// exists: a new generation, the descriptor's playback, then its parts.
    pub(crate) fn create_animated_instance(&mut self, handle: RenderHandle) {
        let Some(NodeKind::AnimatedMesh(descriptor)) =
            self.tables.nodes.get(&handle).map(|node| &node.kind)
        else {
            return;
        };
        let descriptor: AnimatedMeshInstanceDescriptor = (**descriptor).clone();
        let object_id = descriptor.metadata.source_entity;
        let generation = match object_id {
            Some(object) => {
                let next = self.animation_generations.entry(object).or_insert(0);
                *next += 1;
                *next
            }
            None => 0,
        };
        self.tables.animated.insert(
            handle,
            AnimatedInstance {
                asset: descriptor.asset.clone(),
                object_id,
                generation,
                playback: Playback {
                    timeline: AnimatedMeshPlaybackTimeline::Unset,
                    pose: None,
                },
                fade: None,
                completion: None,
                inspection: AnimatedMeshInspection::default(),
                bounds_pending: false,
                pose: Vec::new(),
                pose_dirty: true,
                posed_at: f64::NAN,
                skinned: HashMap::new(),
                part_nodes: Vec::new(),
            },
        );
        if let Some(playback) = &descriptor.playback {
            let _ = self.set_animated_playback(handle, playback);
        }
        let _ = self.set_animated_inspection(handle, &descriptor.inspection);
        self.reset_animated_instance(handle);
    }

    /// Re-derive an instance's pose buffers and parts from its asset.
    fn reset_animated_instance(&mut self, handle: RenderHandle) {
        let Some(instance) = self.tables.animated.get_mut(&handle) else {
            return;
        };
        instance.skinned.clear();
        instance.pose.clear();
        instance.pose_dirty = true;
        self.pose_animated_instance(handle);
        self.rebuild_parts(handle);
    }

    pub(crate) fn set_animated_playback(
        &mut self,
        handle: RenderHandle,
        command: &AnimatedMeshPlaybackCommand,
    ) -> Result<(), String> {
        let now = self.animation_time;
        let durations = self.clip_durations(handle);
        let instance = self
            .tables
            .animated
            .get_mut(&handle)
            .ok_or_else(|| format!("unknown animated mesh {}", handle.raw()))?;
        let prior = instance.playback.clone();
        let prior_clip = current_clip(&prior);
        instance.completion = None;
        instance
            .playback
            .timeline
            .apply_command(command, now)
            .map_err(|error| format!("{error:?}"))?;
        instance.playback.pose = match command {
            AnimatedMeshPlaybackCommand::SamplePose { clips } => Some(clips.clone()),
            _ => None,
        };
        instance.fade = None;
        match command {
            AnimatedMeshPlaybackCommand::Play {
                clip,
                r#loop,
                fade_seconds,
                start_offset_seconds,
                start_paused,
                ..
            } => {
                let fade = fade_seconds.unwrap_or(0.0);
                if fade > 0.0 && prior_clip.as_deref().is_some_and(|prior| prior != clip) {
                    instance.fade = Some(Fade {
                        prior,
                        start: now,
                        duration: f64::from(fade),
                        cross: true,
                    });
                }
                let duration = durations.get(clip).copied().unwrap_or(0.0);
                let started_at_end =
                    start_offset_seconds.is_some_and(|offset| offset >= f64::from(duration));
                if *r#loop == AnimationLoopMode::Once && !start_paused && !started_at_end {
                    instance.completion = Some(clip.clone());
                }
            }
            AnimatedMeshPlaybackCommand::Stop { fade_seconds } => {
                let fade = fade_seconds.unwrap_or(0.0);
                if fade > 0.0 {
                    instance.fade = Some(Fade {
                        prior,
                        start: now,
                        duration: f64::from(fade),
                        cross: false,
                    });
                }
            }
            AnimatedMeshPlaybackCommand::Resume => {
                if let AnimatedMeshPlaybackTimeline::Playing {
                    clip,
                    r#loop: AnimationLoopMode::Once,
                    ..
                } = &instance.playback.timeline
                {
                    instance.completion = Some(clip.clone());
                }
            }
            _ => {}
        }
        instance.pose_dirty = true;
        Ok(())
    }

    pub(crate) fn set_animated_inspection(
        &mut self,
        handle: RenderHandle,
        inspection: &AnimatedMeshInspection,
    ) -> Result<(), String> {
        let instance = self
            .tables
            .animated
            .get_mut(&handle)
            .ok_or_else(|| format!("unknown animated mesh {}", handle.raw()))?;
        if inspection.bounds_request != 0
            && inspection.bounds_request != instance.inspection.bounds_request
        {
            instance.bounds_pending = true;
            instance.pose_dirty = true;
        }
        let matte_changed = inspection.matte != instance.inspection.matte;
        instance.inspection = inspection.clone();
        if matte_changed {
            self.rebuild_parts(handle);
        }
        let mut unrealized = Vec::new();
        if inspection.wireframe {
            unrealized.push("wireframe (#8819)");
        }
        if inspection.whole_voxel_normals {
            unrealized.push("whole-voxel normals");
        }
        if unrealized.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "inspection {} is not realized",
                unrealized.join(", ")
            ))
        }
    }

    fn clip_durations(&self, handle: RenderHandle) -> HashMap<String, f32> {
        self.tables
            .animated
            .get(&handle)
            .and_then(|instance| self.tables.animated_assets.get(&instance.asset))
            .map(|asset| {
                asset
                    .clips
                    .iter()
                    .map(|(id, clip)| (id.clone(), clip.duration))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Advance every instance whose pose can change to the current Engine
    /// time. Called before transforms propagate.
    pub(crate) fn advance_animations(&mut self) {
        let handles: Vec<RenderHandle> = self
            .tables
            .animated
            .iter()
            .filter(|(handle, instance)| {
                let controlled = self
                    .tables
                    .controllers
                    .values()
                    .any(|controller| controller.target == **handle);
                instance.pose_dirty
                    || ((controlled || instance.varies())
                        && instance.posed_at != self.animation_time)
            })
            .map(|(handle, _)| *handle)
            .collect();
        for handle in handles {
            self.pose_animated_instance(handle);
        }
    }

    /// Evaluate one instance's pose at the current time and push it to its
    /// parts, skinned buffers, joint-attached children and facts.
    fn pose_animated_instance(&mut self, handle: RenderHandle) {
        let now = self.animation_time;
        let Some(instance) = self.tables.animated.get_mut(&handle) else {
            return;
        };
        let Some(asset) = self.tables.animated_assets.get(&instance.asset) else {
            instance.pose_dirty = false;
            return;
        };
        if instance
            .fade
            .as_ref()
            .is_some_and(|fade| now >= fade.start + fade.duration)
        {
            instance.fade = None;
        }
        let mut actions = Vec::new();
        let fade_progress = instance
            .fade
            .as_ref()
            .map(|fade| ((now - fade.start) / fade.duration).clamp(0.0, 1.0) as f32);
        if let (Some(fade), Some(progress)) = (&instance.fade, fade_progress) {
            for (clip, time, weight) in actions_of(asset, &fade.prior, now).0 {
                actions.push((clip, time, weight * (1.0 - progress)));
            }
        }
        let controller = self
            .tables
            .controllers
            .values()
            .find(|controller| controller.target == handle);
        let (current, finished) = match controller {
            // A controller replaces direct playback on its target.
            Some(controller) => (
                actions_of(
                    asset,
                    &Playback {
                        timeline: AnimatedMeshPlaybackTimeline::Unset,
                        pose: Some(controller.pose(now)),
                    },
                    now,
                )
                .0,
                false,
            ),
            None => actions_of(asset, &instance.playback, now),
        };
        let fade_in = match (&instance.fade, fade_progress) {
            (Some(fade), Some(progress)) if fade.cross => progress,
            _ => 1.0,
        };
        actions.extend(
            current
                .into_iter()
                .map(|(clip, time, weight)| (clip, time, weight * fade_in)),
        );
        instance.pose = evaluate_pose(&asset.model, &asset.clips, &actions);
        instance.pose_dirty = false;
        instance.posed_at = now;
        let completed = if finished {
            instance.completion.take()
        } else {
            None
        };
        if let Some(clip) = completed {
            if let Some(object_id) = instance.object_id {
                self.animation_facts.push(AnimationFact::NaturalCompletion {
                    object_id,
                    generation: instance.generation,
                    clip,
                });
            }
        }
        self.skin_animated_instance(handle);
        self.write_animated_parts(handle);
        // Children attached to joints follow the new pose.
        if let Some(node) = self.tables.nodes.get(&handle) {
            for child in &node.children {
                if self
                    .tables
                    .nodes
                    .get(child)
                    .is_some_and(|child| child.parent_joint.is_some())
                {
                    self.tables.dirty_nodes.insert(*child);
                }
            }
        }
    }

    /// Answer pending bounds requests from the posed, propagated world
    /// state. Runs after `propagate_transforms`, so a transform and a
    /// request in the same delta report the new placement (#8788 review).
    pub(crate) fn report_pending_bounds(&mut self) {
        let pending: Vec<RenderHandle> = self
            .tables
            .animated
            .iter()
            .filter(|(_, instance)| instance.bounds_pending)
            .map(|(handle, _)| *handle)
            .collect();
        for handle in pending {
            self.report_animated_bounds(handle);
        }
    }

    /// CPU-skin every skinned primitive into the instance's vertex buffers.
    fn skin_animated_instance(&mut self, handle: RenderHandle) {
        let Some(instance) = self.tables.animated.get(&handle) else {
            return;
        };
        let Some(asset) = self.tables.animated_assets.get(&instance.asset) else {
            return;
        };
        let mut updates = Vec::new();
        for &node_index in &asset.model.order {
            let node = &asset.model.nodes[node_index];
            let (Some(mesh), Some(skin)) = (node.mesh, node.skin) else {
                continue;
            };
            let skin = &asset.model.skins[skin];
            let palette: Vec<Mat4> = skin
                .joints
                .iter()
                .zip(&skin.inverse_binds)
                .map(|(joint, inverse)| instance.pose[*joint] * *inverse)
                .collect();
            for (primitive_index, primitive) in asset.model.meshes[mesh].iter().enumerate() {
                let (Some(joints), Some(weights)) = (&primitive.joints, &primitive.weights) else {
                    continue;
                };
                let mut positions = Vec::with_capacity(primitive.positions.len());
                let mut normals = Vec::with_capacity(primitive.positions.len());
                for (vertex, position) in primitive.positions.iter().enumerate() {
                    let mut matrix = Mat4::ZERO;
                    for influence in 0..4 {
                        let weight = weights[vertex][influence];
                        if weight > 0.0 {
                            let joint = usize::from(joints[vertex][influence]);
                            if let Some(joint_matrix) = palette.get(joint) {
                                matrix += *joint_matrix * weight;
                            }
                        }
                    }
                    positions.push(matrix.transform_point3(Vec3::from(*position)).to_array());
                    let normal = matrix
                        .transform_vector3(Vec3::from(primitive.normals[vertex]))
                        .normalize_or(Vec3::Y);
                    normals.push(normal.to_array());
                }
                let vertices = interleave(primitive, &positions, &normals);
                updates.push(((node_index as u32, primitive_index as u32), vertices));
            }
        }
        for (key, vertices) in updates {
            let exists = self
                .tables
                .animated
                .get(&handle)
                .is_some_and(|instance| instance.skinned.contains_key(&key));
            if exists {
                let instance = &self.tables.animated[&handle];
                let mesh = &instance.skinned[&key];
                self.gpu
                    .queue
                    .write_buffer(&mesh.vertices, 0, bytemuck::cast_slice(&vertices));
                let bounds = vertex_bounds(&vertices);
                if let Some(mesh) = self
                    .tables
                    .animated
                    .get_mut(&handle)
                    .and_then(|instance| instance.skinned.get_mut(&key))
                {
                    mesh.bounds = bounds;
                    mesh.cpu = std::sync::Arc::new(crate::tables::CpuGeometry {
                        positions: vertex_positions(&vertices),
                        indices: mesh.cpu.indices.clone(),
                    });
                }
            } else {
                let asset = &self.tables.animated_assets[&self.tables.animated[&handle].asset];
                let node = &asset.model.nodes[key.0 as usize];
                let primitive = &asset.model.meshes[node.mesh.expect("skinned node has a mesh")]
                    [key.1 as usize];
                let indices = primitive.indices.clone();
                let mesh = self.upload_dynamic_vertices(
                    &format!("animated {} skin {}/{}", handle.raw(), key.0, key.1),
                    &vertices,
                    &indices,
                );
                if let Some(instance) = self.tables.animated.get_mut(&handle) {
                    instance.skinned.insert(key, mesh);
                }
            }
        }
    }

    /// Re-write the instance's part rows with their node poses (rigid parts)
    /// and skinned bounds.
    pub(crate) fn write_animated_parts(&mut self, handle: RenderHandle) {
        let (Some(instance), Some(node)) = (
            self.tables.animated.get(&handle),
            self.tables.nodes.get(&handle),
        ) else {
            return;
        };
        if node.parts.len() != instance.part_nodes.len() {
            return;
        }
        let asset = self.tables.animated_assets.get(&instance.asset);
        for (part, glb_node) in node.parts.iter().zip(&instance.part_nodes) {
            let skinned = asset.is_some_and(|asset| asset.model.nodes[*glb_node].skin.is_some());
            let local = if skinned {
                Mat4::IDENTITY
            } else {
                instance
                    .pose
                    .get(*glb_node)
                    .copied()
                    .unwrap_or(Mat4::IDENTITY)
            };
            let bounds = self
                .tables
                .parts
                .meta
                .get(*part as usize)
                .and_then(Option::as_ref)
                .and_then(|meta| self.mesh(&meta.mesh))
                .map(|mesh| mesh.bounds);
            let state = &mut self.tables.parts.state[*part as usize];
            state.local = Some(local);
            if let Some(bounds) = bounds {
                state.local_bounds = bounds;
            }
            self.tables
                .parts
                .write(*part, &node.world, node.world_visible, node.world_layer);
        }
    }

    fn report_animated_bounds(&mut self, handle: RenderHandle) {
        let Some(instance) = self.tables.animated.get_mut(&handle) else {
            return;
        };
        if !instance.bounds_pending {
            return;
        }
        instance.bounds_pending = false;
        let (Some(object_id), generation, request) = (
            instance.object_id,
            instance.generation,
            instance.inspection.bounds_request,
        ) else {
            return;
        };
        let bounds = self.animated_world_bounds(handle);
        self.animation_facts.push(AnimationFact::MeshInspection {
            object_id,
            generation,
            request,
            bounds: bounds.map(|bounds| (bounds.min.to_array(), bounds.max.to_array())),
        });
    }

    /// Exact posed world bounds: every vertex at its current pose.
    pub(crate) fn animated_world_bounds(&self, handle: RenderHandle) -> Option<Aabb> {
        let instance = self.tables.animated.get(&handle)?;
        let asset = self.tables.animated_assets.get(&instance.asset)?;
        let world = self.tables.nodes.get(&handle)?.world;
        let mut bounds = Aabb::EMPTY;
        for &node_index in &asset.model.order {
            let node = &asset.model.nodes[node_index];
            let Some(mesh) = node.mesh else { continue };
            for (primitive_index, primitive) in asset.model.meshes[mesh].iter().enumerate() {
                if let Some(skinned) = instance
                    .skinned
                    .get(&(node_index as u32, primitive_index as u32))
                {
                    let skinned = skinned.bounds.transformed(&world);
                    if !skinned.is_empty() {
                        bounds.include(skinned.min);
                        bounds.include(skinned.max);
                    }
                    continue;
                }
                let matrix = world * instance.pose.get(node_index).copied().unwrap_or_default();
                for position in &primitive.positions {
                    bounds.include(matrix.transform_point3(Vec3::from(*position)));
                }
            }
        }
        let finite = bounds.min.is_finite() && bounds.max.is_finite();
        (!bounds.is_empty() && finite).then_some(bounds)
    }

    /// A joint's current transform in its instance's space.
    pub(crate) fn joint_pose(&self, handle: RenderHandle, joint: &str) -> Option<Mat4> {
        let instance = self.tables.animated.get(&handle)?;
        let asset = self.tables.animated_assets.get(&instance.asset)?;
        let node = *asset.joints.get(joint)?;
        instance.pose.get(node).copied()
    }

    /// The parts of an animated instance's node, in draw order: each GLB node
    /// with a mesh, each primitive, with its material binding.
    pub(crate) fn animated_parts(
        &mut self,
        handle: RenderHandle,
    ) -> Vec<(crate::tables::MeshRef, u32, MaterialRef)> {
        let Some(instance) = self.tables.animated.get(&handle) else {
            return Vec::new();
        };
        let Some(asset) = self.tables.animated_assets.get(&instance.asset) else {
            return Vec::new();
        };
        let overrides: BTreeMap<u16, String> = match self.tables.nodes.get(&handle).map(|n| &n.kind)
        {
            Some(NodeKind::AnimatedMesh(descriptor)) => descriptor
                .material_overrides
                .iter()
                .map(|slot| (slot.slot, slot.material.clone()))
                .collect(),
            _ => BTreeMap::new(),
        };
        let matte = instance.inspection.matte;
        let mut parts = Vec::new();
        let mut part_nodes = Vec::new();
        let mut mattes = Vec::new();
        for &node_index in &asset.model.order {
            let node = &asset.model.nodes[node_index];
            let Some(mesh) = node.mesh else { continue };
            for (primitive_index, primitive) in asset.model.meshes[mesh].iter().enumerate() {
                let mesh_ref = if primitive.joints.is_some() && node.skin.is_some() {
                    crate::tables::MeshRef::AnimatedSkinned(
                        handle,
                        node_index as u32,
                        primitive_index as u32,
                    )
                } else {
                    crate::tables::MeshRef::AnimatedRigid(
                        instance.asset.clone(),
                        mesh as u32,
                        primitive_index as u32,
                    )
                };
                let material = match primitive.material {
                    Some(glb_material) => {
                        let overridden = asset
                            .slots
                            .iter()
                            .find(|(_, source)| **source == glb_material)
                            .and_then(|(slot, _)| overrides.get(slot));
                        let id = overridden
                            .cloned()
                            .unwrap_or_else(|| asset.materials[glb_material].clone());
                        if matte {
                            mattes.push(id.clone());
                            MaterialRef::Retained(format!("{id}#matte"))
                        } else {
                            MaterialRef::Retained(id)
                        }
                    }
                    None => MaterialRef::LitFallback,
                };
                parts.push((mesh_ref, primitive.indices.len() as u32, material));
                part_nodes.push(node_index);
            }
        }
        if let Some(instance) = self.tables.animated.get_mut(&handle) {
            instance.part_nodes = part_nodes;
        }
        // Matte inspection: roughness 1, metalness 0 variants of the bound
        // materials, as Three's inspection material did.
        for id in mattes {
            let matte_id = format!("{id}#matte");
            if self.tables.materials.contains_key(&matte_id) {
                continue;
            }
            if let Some(row) = self.tables.materials.get(&id) {
                let mut descriptor = row.descriptor.clone();
                descriptor.id = matte_id;
                descriptor.roughness = 1.0;
                self.define_material_with(descriptor, 0.0, false);
            }
        }
        parts
    }

    /// Upload a vertex buffer the CPU rewrites (skinned primitives).
    fn upload_dynamic_vertices(&self, label: &str, vertices: &[f32], indices: &[u32]) -> GpuMesh {
        use wgpu::util::DeviceExt;
        let device = &self.gpu.device;
        GpuMesh {
            bounds: vertex_bounds(vertices),
            cpu: std::sync::Arc::new(crate::tables::CpuGeometry {
                positions: vertex_positions(vertices),
                indices: indices.to_vec(),
            }),
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            topology: Topology::Triangles,
            groups: vec![(0, 0, indices.len() as u32)],
            slots: BTreeMap::new(),
        }
    }

    /// The GPU mesh an animated part draws.
    pub(crate) fn animated_mesh(&self, mesh: &crate::tables::MeshRef) -> Option<&GpuMesh> {
        match mesh {
            crate::tables::MeshRef::AnimatedRigid(asset, mesh, primitive) => self
                .tables
                .animated_assets
                .get(asset)
                .and_then(|asset| asset.rigid.get(&(*mesh, *primitive))),
            crate::tables::MeshRef::AnimatedSkinned(handle, node, primitive) => self
                .tables
                .animated
                .get(handle)
                .and_then(|instance| instance.skinned.get(&(*node, *primitive))),
            _ => None,
        }
    }

    pub(crate) fn remove_animated_instance(&mut self, handle: RenderHandle) {
        self.tables.animated.remove(&handle);
    }

    /// Material with GLB extras: metalness and the unlit extension.
    pub(crate) fn define_material_with(
        &mut self,
        descriptor: RenderMaterialDescriptor,
        metalness: f32,
        unlit: bool,
    ) {
        let texture = descriptor
            .texture
            .as_ref()
            .and_then(|id| self.tables.textures.get(id));
        let mut params = MaterialParams::of(&descriptor, texture.map(|texture| texture.size));
        params.metalness = metalness;
        if unlit {
            params.flags |= crate::apply::FLAG_UNLIT;
        }
        self.insert_material(descriptor, &params);
    }
}

impl AnimatedInstance {
    /// Whether the pose moves with time.
    fn varies(&self) -> bool {
        if self.fade.is_some() {
            return true;
        }
        match &self.playback.timeline {
            // A `once` clip settles once its completion is reported.
            AnimatedMeshPlaybackTimeline::Playing { r#loop, .. } => {
                *r#loop != AnimationLoopMode::Once || self.completion.is_some()
            }
            _ => false,
        }
    }
}

fn current_clip(playback: &Playback) -> Option<String> {
    match &playback.timeline {
        AnimatedMeshPlaybackTimeline::Playing { clip, .. }
        | AnimatedMeshPlaybackTimeline::Paused { clip, .. }
        | AnimatedMeshPlaybackTimeline::Sampled { clip, .. } => Some(clip.clone()),
        _ => None,
    }
}

/// Clip times for a playback state at `now`, each wrapped by its loop mode,
/// and whether a `once` clip has reached its end.
fn actions_of(
    asset: &AnimatedAssetRow,
    playback: &Playback,
    now: f64,
) -> (Vec<(String, f32, f32)>, bool) {
    let duration = |clip: &str| asset.clips.get(clip).map_or(0.0, |clip| clip.duration);
    if let Some(pose) = &playback.pose {
        let actions = pose
            .iter()
            .map(|clip| {
                let time = wrap(
                    clip.time_seconds,
                    duration(&clip.clip),
                    AnimationLoopMode::Repeat,
                );
                (clip.clip.clone(), time, clip.weight)
            })
            .collect();
        return (actions, false);
    }
    match &playback.timeline {
        AnimatedMeshPlaybackTimeline::Playing {
            clip,
            r#loop,
            speed,
            weight,
            base_offset_seconds,
            anchor_seconds,
        } => {
            let raw = base_offset_seconds + (now - anchor_seconds) * f64::from(*speed);
            let length = duration(clip);
            let finished = *r#loop == AnimationLoopMode::Once && raw >= f64::from(length);
            (
                vec![(clip.clone(), wrap(raw, length, *r#loop), *weight)],
                finished,
            )
        }
        AnimatedMeshPlaybackTimeline::Paused {
            clip,
            r#loop,
            weight,
            offset_seconds,
            ..
        } => (
            vec![(
                clip.clone(),
                wrap(*offset_seconds, duration(clip), *r#loop),
                *weight,
            )],
            false,
        ),
        AnimatedMeshPlaybackTimeline::Sampled {
            clip,
            normalized_time,
        } => (
            vec![(clip.clone(), duration(clip) * normalized_time, 1.0)],
            false,
        ),
        AnimatedMeshPlaybackTimeline::Unset | AnimatedMeshPlaybackTimeline::Stopped => {
            (Vec::new(), false)
        }
    }
}

/// A clip time wrapped by loop mode: `once` clamps, `repeat` wraps,
/// `pingPong` reflects on odd loops.
fn wrap(time: f64, duration: f32, mode: AnimationLoopMode) -> f32 {
    let duration = f64::from(duration);
    if duration <= 0.0 {
        return 0.0;
    }
    let wrapped = match mode {
        AnimationLoopMode::Once => time.clamp(0.0, duration),
        AnimationLoopMode::Repeat => time.rem_euclid(duration),
        AnimationLoopMode::PingPong => {
            let cycle = (time / duration).floor();
            let within = time - cycle * duration;
            if (cycle as i64).rem_euclid(2) == 1 {
                duration - within
            } else {
                within
            }
        }
    };
    wrapped as f32
}

/// Local TRS per node after blending the weighted clips over the rest pose,
/// composed into instance-space node transforms.
fn evaluate_pose(
    model: &GlbModel,
    clips: &HashMap<String, GlbClip>,
    actions: &[(String, f32, f32)],
) -> Vec<Mat4> {
    #[derive(Clone, Copy)]
    struct Mix<T> {
        value: T,
        weight: f32,
    }
    let count = model.nodes.len();
    let mut translation: Vec<Option<Mix<Vec3>>> = vec![None; count];
    let mut rotation: Vec<Option<Mix<Quat>>> = vec![None; count];
    let mut scale: Vec<Option<Mix<Vec3>>> = vec![None; count];
    for (clip, time, weight) in actions {
        if *weight <= 0.0 {
            continue;
        }
        let Some(clip) = clips.get(clip) else {
            continue;
        };
        for channel in &clip.channels {
            match channel.path {
                Path::Translation | Path::Scale => {
                    let value = channel.sample_vec3(*time);
                    let slot = if channel.path == Path::Translation {
                        &mut translation[channel.node]
                    } else {
                        &mut scale[channel.node]
                    };
                    *slot = Some(match *slot {
                        None => Mix {
                            value,
                            weight: *weight,
                        },
                        Some(mix) => Mix {
                            value: mix.value.lerp(value, weight / (mix.weight + weight)),
                            weight: mix.weight + weight,
                        },
                    });
                }
                Path::Rotation => {
                    let value = channel.sample_quat(*time);
                    let slot = &mut rotation[channel.node];
                    *slot = Some(match *slot {
                        None => Mix {
                            value,
                            weight: *weight,
                        },
                        Some(mix) => Mix {
                            value: mix.value.slerp(value, weight / (mix.weight + weight)),
                            weight: mix.weight + weight,
                        },
                    });
                }
            }
        }
    }
    let settle_vec = |rest: Vec3, mix: Option<Mix<Vec3>>| match mix {
        None => rest,
        Some(mix) if mix.weight < 1.0 => rest.lerp(mix.value, mix.weight),
        Some(mix) => mix.value,
    };
    let mut world = vec![Mat4::IDENTITY; count];
    for &index in &model.order {
        let node = &model.nodes[index];
        let local = Trs {
            translation: settle_vec(node.rest.translation, translation[index]),
            rotation: match rotation[index] {
                None => node.rest.rotation,
                Some(mix) if mix.weight < 1.0 => node.rest.rotation.slerp(mix.value, mix.weight),
                Some(mix) => mix.value,
            },
            scale: settle_vec(node.rest.scale, scale[index]),
        }
        .matrix();
        world[index] = match node.parent {
            Some(parent) => world[parent] * local,
            None => local,
        };
    }
    world
}

fn decode_clips(model: &GlbModel) -> Vec<Option<GlbClip>> {
    model
        .clips
        .iter()
        .map(|clip| {
            Some(GlbClip {
                name: clip.name.clone(),
                duration: clip.duration,
                channels: clip
                    .channels
                    .iter()
                    .map(|channel| glb::Channel {
                        node: channel.node,
                        path: channel.path,
                        interpolation: channel.interpolation,
                        times: channel.times.clone(),
                        values: channel.values.clone(),
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Interleave a primitive's streams into the renderer's vertex layout.
fn interleave(
    primitive: &glb::GlbPrimitive,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
) -> Vec<f32> {
    let mut vertices = Vec::with_capacity(positions.len() * VERTEX_FLOATS);
    for (vertex, position) in positions.iter().enumerate() {
        vertices.extend_from_slice(position);
        vertices.extend_from_slice(&normals[vertex]);
        match primitive.uvs.as_ref().and_then(|uvs| uvs.get(vertex)) {
            Some(uv) => vertices.extend_from_slice(uv),
            None => vertices.extend_from_slice(&[0.0, 0.0]),
        }
        match primitive
            .colors
            .as_ref()
            .and_then(|colors| colors.get(vertex))
        {
            Some(color) => vertices.extend_from_slice(color),
            None => vertices.extend_from_slice(&[1.0; 4]),
        }
    }
    vertices
}

fn vertex_positions(vertices: &[f32]) -> Vec<Vec3> {
    vertices
        .chunks_exact(VERTEX_FLOATS)
        .map(|vertex| Vec3::new(vertex[0], vertex[1], vertex[2]))
        .collect()
}

fn vertex_bounds(vertices: &[f32]) -> Aabb {
    let mut bounds = Aabb::EMPTY;
    for vertex in vertices.chunks_exact(VERTEX_FLOATS) {
        bounds.include(Vec3::new(vertex[0], vertex[1], vertex[2]));
    }
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_modes_wrap_clip_time() {
        assert_eq!(wrap(1.5, 1.0, AnimationLoopMode::Once), 1.0);
        assert_eq!(wrap(1.25, 1.0, AnimationLoopMode::Repeat), 0.25);
        assert_eq!(wrap(1.25, 1.0, AnimationLoopMode::PingPong), 0.75);
        assert_eq!(wrap(2.25, 1.0, AnimationLoopMode::PingPong), 0.25);
    }
}
