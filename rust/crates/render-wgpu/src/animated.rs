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
//! its target until destroyed (the first by handle, if several target it).
//!
//! **Blending**: each property takes its first
//! contributing clip, later clips mix in by `w / (Σw + w)` (slerp for
//! rotations), and a total weight below 1 mixes toward the rest pose.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use glam::{Mat4, Vec3};
use render_model::{
    AnimatedMeshAsset, AnimatedMeshInspection, AnimatedMeshInstanceDescriptor,
    AnimatedMeshPlaybackCommand, AnimatedMeshPlaybackTimeline, AnimatedMeshPose, AnimationLoopMode,
    IkChainReach, MaterialAlphaModeDescriptor, MaterialInstanceParameters, MaterialUvStrategy,
    PosedJoint, RenderHandle, RenderMaterialDescriptor, Transform,
};

use render_presentation::AnimationProjectionOp;

use crate::apply::{MapSlot, MaterialMaps, MaterialParams};
use crate::convert;
use crate::glb::{self, GlbAlpha, GlbClip, GlbModel, Trs};
use crate::pipelines::{EXTRA_VERTEX_FLOATS, VERTEX_FLOATS};
use crate::resources::ResourceSource;
use crate::tables::{Aabb, GpuMesh, MaterialRef, NodeKind, Topology};
use crate::Renderer;
use animation_pose::{ControllerPlayback, PlaybackState};

/// What the runtime reads back from animated meshes (animation feedback
/// facts), keyed by the instance's source entity and its
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
    /// The evaluated joints of an instance that reports them
    /// (`AnimatedMeshPose::report_joints`), each time it is posed.
    JointPose {
        object_id: u64,
        generation: u64,
        /// The Engine time the pose was evaluated at.
        seconds: f64,
        /// The instance's world placement.
        world: Transform,
        /// Each rig joint, in rig order.
        joints: Vec<PosedJoint>,
        /// The rest pose, in the same form: in the first report after
        /// reporting starts.
        rest: Option<Vec<PosedJoint>>,
        /// Each two-bone IK chain, in the order the pose gave them.
        ik_chains: Vec<IkChainReach>,
    },
}

/// An animation controller projected onto an animated instance.
pub(crate) struct ControllerRow {
    target: RenderHandle,
    playback: ControllerPlayback,
}

/// One admitted animated-mesh asset.
pub(crate) struct AnimatedAssetRow {
    model: GlbModel,
    /// Clips by index; `clip_ids` maps descriptor clip ids to them.
    clips: Vec<GlbClip>,
    clip_ids: HashMap<String, usize>,
    /// Uploaded unskinned primitives, by (mesh, primitive).
    rigid: HashMap<(u32, u32), GpuMesh>,
    /// Retained material id per GLB material.
    materials: Vec<String>,
    textures: Vec<String>,
    /// Linear copies of the textures normal and occlusion maps read.
    linear_textures: Vec<String>,
    /// Engine slot to GLB material index.
    slots: BTreeMap<u16, usize>,
    /// Joint (bone) names that resolve to exactly one node.
    joints: HashMap<String, usize>,
    /// The node of each rig joint, in rig order (`AnimationRigSignature`):
    /// what pose controls and joint reports index.
    rig: Vec<Option<usize>>,
}

pub(crate) struct AnimatedInstance {
    /// The asset's slot in `animated_assets` (its `names` id).
    slot: u32,
    object_id: Option<u64>,
    generation: u64,
    playback: PlaybackState,
    /// The `once` clip whose end is reported, until any new command.
    completion: Option<String>,
    pub(crate) inspection: AnimatedMeshInspection,
    bounds_pending: bool,
    /// Node transforms in instance space, per GLB node.
    pose: Vec<Mat4>,
    pose_dirty: bool,
    /// The Engine time the pose was evaluated at.
    posed_at: f64,
    /// The product's IK and joint overrides, and whether joints report.
    controls: AnimatedMeshPose,
    /// The instance's world placement the pose was evaluated with.
    posed_world: Mat4,
    /// Posed since its joints were last reported.
    joints_pending: bool,
    /// The rest pose has been reported since reporting started.
    rest_reported: bool,
    /// The IK chains of the evaluated pose, when it reports its joints.
    ik: Vec<animation_pose::IkSolve>,
    /// Per-instance skinned vertex buffers, by (node, primitive).
    skinned: HashMap<(u32, u32), GpuMesh>,
    /// The GLB node each of the node row's parts draws, in part order.
    part_nodes: Vec<usize>,
    /// Per material slot values over the slot's material
    /// (`SetMaterialInstanceParameters`).
    pub(crate) parameters: BTreeMap<u16, MaterialInstanceParameters>,
}

pub(crate) use animation_pose::resource_identity;

/// Decode an animated asset's GLB and resolve its clips by descriptor id:
/// its own clips by name, then clip packs, whose channels bind to the rig by
/// node name. Rendering and GLB export read the asset through this.
pub fn decode_animated_asset(
    asset: &AnimatedMeshAsset,
    resources: &dyn ResourceSource,
) -> Result<(GlbModel, HashMap<String, GlbClip>), String> {
    let identity = resource_identity("animated-mesh-resource", asset.content_hash.as_deref())
        .ok_or_else(|| format!("{} has no content hash", asset.asset))?;
    let bytes = resources
        .bytes(&identity)
        .ok_or_else(|| format!("resource {identity} is not available"))?;
    let model = glb::decode(&bytes)?;
    let clips = animation_pose::asset_clips(asset, &model, model.clips.clone(), |identity| {
        resources.bytes(identity)
    })?;
    Ok((model, clips))
}

impl Renderer {
    /// The Engine presentation timeline in seconds (`PresentationWorld`'s
    /// elapsed time). Animated poses advance on it, not on display time.
    /// Frames draw at it too, until [`Self::set_presentation_time`] moves
    /// them on.
    pub fn set_animation_time(&mut self, seconds: f64) {
        debug_assert!(seconds.is_finite() && seconds >= 0.0, "{seconds}");
        self.animation_time = seconds;
        self.presentation_time = seconds;
    }

    /// The presentation time the next frames draw at: their `frame.time`,
    /// cloud drift, wind sway, precipitation and fog. A host whose world
    /// runs on between committed steps gives the time it has reached, so
    /// what moves with time moves every frame; poses, video and their facts
    /// stay on the committed time ([`Self::set_animation_time`]).
    pub fn set_presentation_time(&mut self, seconds: f64) {
        debug_assert!(seconds.is_finite() && seconds >= 0.0, "{seconds}");
        self.presentation_time = seconds;
    }

    /// Apply one animation controller op (`PresentationOp::Animation`).
    pub(crate) fn apply_animation_op(&mut self, op: &AnimationProjectionOp) -> Result<(), String> {
        let now = self.animation_time;
        let target = match op {
            AnimationProjectionOp::Create { handle, descriptor } => {
                let replaced = self.tables.controllers.insert(
                    handle.raw(),
                    ControllerRow {
                        target: descriptor.target,
                        playback: ControllerPlayback::new(descriptor.controller.clone(), now),
                    },
                );
                // A replaced controller's former target poses without it.
                if let Some(replaced) = replaced {
                    self.unlink_controller(handle.raw(), replaced.target);
                    if let Some(instance) = self.tables.animated.get_mut(&replaced.target) {
                        instance.pose_dirty = true;
                    }
                }
                self.tables
                    .controlled
                    .entry(descriptor.target)
                    .or_default()
                    .insert(handle.raw());
                self.resolve_controllers_of(descriptor.target);
                descriptor.target
            }
            AnimationProjectionOp::Update { handle, controller } => {
                let row = self
                    .tables
                    .controllers
                    .get_mut(&handle.raw())
                    .ok_or_else(|| format!("unknown animation controller {}", handle.raw()))?;
                row.playback = ControllerPlayback::new(controller.clone(), now);
                let target = row.target;
                self.resolve_controllers_of(target);
                target
            }
            AnimationProjectionOp::Destroy { handle } => {
                let row = self
                    .tables
                    .controllers
                    .remove(&handle.raw())
                    .ok_or_else(|| format!("unknown animation controller {}", handle.raw()))?;
                self.unlink_controller(handle.raw(), row.target);
                row.target
            }
        };
        if let Some(instance) = self.tables.animated.get_mut(&target) {
            instance.pose_dirty = true;
        }
        Ok(())
    }

    fn unlink_controller(&mut self, controller: u64, target: RenderHandle) {
        if let Some(controllers) = self.tables.controlled.get_mut(&target) {
            controllers.remove(&controller);
            if controllers.is_empty() {
                self.tables.controlled.remove(&target);
            }
        }
    }

    /// Resolve the clip tables of every controller driving `target` against
    /// its current asset: when a controller's state or the asset changes.
    fn resolve_controllers_of(&mut self, target: RenderHandle) {
        let asset = self
            .tables
            .animated
            .get(&target)
            .and_then(|instance| self.tables.animated_assets.get(instance.slot));
        for controller in self.tables.controlled.get(&target).into_iter().flatten() {
            if let Some(row) = self.tables.controllers.get_mut(controller) {
                row.playback.resolve(&clip_index(asset));
            }
        }
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
        let (model, clips) = decode_animated_asset(asset, resources)?;

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
                    texture.mipmaps,
                ),
                None => crate::apply::upload_rgba_texture(
                    &self.gpu, id, 1, 1, &[255; 4], true, true, false, false,
                ),
            };
            self.tables.textures.insert(id.clone(), uploaded);
        }
        // Normal and occlusion maps hold data, not colour: they read linear
        // copies of their images. A slot whose image did not decode is absent.
        let mut linear_textures = HashMap::new();
        for material in &model.materials {
            let data = [
                material.normal_texture.map(|(slot, _)| slot),
                material.occlusion_texture.map(|(slot, _)| slot),
            ];
            for slot in data.into_iter().flatten() {
                let (Some(texture), false) = (
                    model.textures.get(slot.texture),
                    linear_textures.contains_key(&slot.texture),
                ) else {
                    continue;
                };
                let Some(image) = &texture.image else {
                    continue;
                };
                let id = format!("{label}#texture/{}#linear", slot.texture);
                let uploaded = crate::apply::upload_rgba_texture(
                    &self.gpu,
                    &id,
                    image.width,
                    image.height,
                    &image.rgba,
                    false,
                    texture.nearest,
                    texture.repeat,
                    texture.mipmaps,
                );
                self.tables.textures.insert(id.clone(), uploaded);
                linear_textures.insert(slot.texture, id);
            }
        }
        let decoded = |slot: &glb::GlbTextureSlot| {
            model
                .textures
                .get(slot.texture)
                .is_some_and(|texture| texture.image.is_some())
        };
        let materials: Vec<String> = (0..model.materials.len())
            .map(|index| format!("{label}#material/{index}"))
            .collect();
        for (id, material) in materials.iter().zip(&model.materials) {
            let texture = material
                .base_color_texture
                .map(|slot| textures[slot.texture].clone());
            let maps = MaterialMaps {
                base: material.base_color_texture.map(|slot| slot.transform),
                base_tex_coord: material.base_color_texture.map_or(0, |slot| slot.tex_coord),
                emissive: material
                    .emissive_texture
                    .filter(|slot| decoded(slot))
                    .map(|slot| MapSlot {
                        texture: textures[slot.texture].clone(),
                        transform: slot.transform,
                        tex_coord: slot.tex_coord,
                    }),
                normal: material.normal_texture.and_then(|(slot, scale)| {
                    Some((
                        MapSlot {
                            texture: linear_textures.get(&slot.texture)?.clone(),
                            transform: slot.transform,
                            tex_coord: slot.tex_coord,
                        },
                        scale,
                    ))
                }),
                occlusion: material.occlusion_texture.and_then(|(slot, strength)| {
                    Some((
                        MapSlot {
                            texture: linear_textures.get(&slot.texture)?.clone(),
                            transform: slot.transform,
                            tex_coord: slot.tex_coord,
                        },
                        strength,
                    ))
                }),
                occlusion_roughness_metalness: false,
            };
            let descriptor = RenderMaterialDescriptor {
                texture_transform: None,
                stochastic_tiling: None,
                terrain_layers: None,
                id: id.clone(),
                color: material.base_color,
                texture,
                roughness: material.roughness,
                metalness: material.metallic,
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
                normal_map: None,
                triplanar: None,
                shader: None,
                emission_map: Default::default(),
                occlusion_map: Default::default(),
                unlit: false,
                flat_shading: false,
                keep_dry: false,
                wind: None,
                water: None,
                translucent_shadow: false,
            };
            self.define_material_with(descriptor, material.unlit, maps);
        }
        let linear_textures = linear_textures.into_values().collect();

        let mut rigid = HashMap::new();
        for (mesh_index, primitives) in model.meshes.iter().enumerate() {
            for (primitive_index, primitive) in primitives.iter().enumerate() {
                if primitive.joints.is_some() {
                    continue;
                }
                let vertices = interleave(primitive, &primitive.positions, &primitive.normals);
                let mut mesh = self.upload_vertices(
                    &format!("{label} mesh {mesh_index}/{primitive_index}"),
                    &vertices,
                    &primitive.indices,
                    Topology::Triangles,
                    vec![(0, 0, primitive.indices.len() as u32)],
                    BTreeMap::new(),
                );
                mesh.extra = extra_stream(&model, primitive, primitive.tangents.as_deref())
                    .map(|extra| self.vertex_buffer(&format!("{label} tangents"), &extra, false));
                rigid.insert((mesh_index as u32, primitive_index as u32), mesh);
            }
        }
        // A material drawn with the second stream needs its own pipelines:
        // make them now rather than at its first draw.
        let streamed: BTreeSet<usize> = model
            .meshes
            .iter()
            .flatten()
            .filter(|primitive| extra_stream(&model, primitive, None).is_some())
            .filter_map(|primitive| primitive.material)
            .collect();
        for index in streamed {
            let prepared = crate::tables::named(
                &self.tables.names,
                &self.tables.materials,
                &materials[index],
            )
            .map(|(_, row)| {
                (
                    row.features | crate::shaders::Features::VERTEX_TANGENTS,
                    crate::apply::blends(&row.descriptor),
                    row.descriptor.double_sided,
                )
            });
            if let Some((features, blend, double_sided)) = prepared {
                self.prepare_material(features, blend, double_sided);
            }
        }
        let joints = joint_nodes(&model);
        let rig = animation_pose::rig_nodes(asset, &model);
        let slots = asset
            .embedded_material_slots
            .iter()
            .map(|slot| (slot.slot, usize::from(slot.source_material_slot)))
            .collect();
        let mut clip_ids = HashMap::with_capacity(clips.len());
        let clips: Vec<GlbClip> = clips
            .into_iter()
            .enumerate()
            .map(|(index, (id, clip))| {
                clip_ids.insert(id, index);
                clip
            })
            .collect();
        let slot = self.tables.names.id(&asset.asset);
        self.tables.animated_assets.insert(
            slot,
            AnimatedAssetRow {
                model,
                clips,
                clip_ids,
                rigid,
                materials,
                textures,
                linear_textures,
                slots,
                joints,
                rig,
            },
        );
        // Live instances (a redefinition keeps them) re-pose on the new asset.
        for handle in self.instances_of(slot) {
            self.reset_animated_instance(handle);
        }
        Ok(())
    }

    pub(crate) fn release_animated_mesh(&mut self, asset: &str) {
        self.release_animated_mesh_resources(asset);
        if let Some(slot) = self.tables.names.get(asset) {
            for handle in self.instances_of(slot) {
                self.reset_animated_instance(handle);
            }
        }
    }

    /// The instances of the animated asset in `slot`.
    fn instances_of(&self, slot: u32) -> Vec<RenderHandle> {
        self.users_of(slot)
            .into_iter()
            .filter(|handle| {
                self.tables
                    .animated
                    .get(handle)
                    .is_some_and(|instance| instance.slot == slot)
            })
            .collect()
    }

    fn release_animated_mesh_resources(&mut self, asset: &str) {
        let slot = self.tables.names.get(asset);
        if let Some(row) = slot.and_then(|slot| self.tables.animated_assets.remove(slot)) {
            let released: Vec<u32> = row
                .materials
                .iter()
                .filter_map(|id| self.tables.names.get(id))
                .filter(|id| self.tables.materials.remove(*id).is_some())
                .collect();
            for id in row.textures.into_iter().chain(row.linear_textures) {
                self.tables.textures.remove(&id);
            }
            self.rebuild_users(&released);
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
        let slot = self.tables.names.id(&descriptor.asset);
        self.tables.animated.insert(
            handle,
            AnimatedInstance {
                slot,
                object_id,
                generation,
                playback: PlaybackState::default(),
                completion: None,
                inspection: AnimatedMeshInspection::default(),
                bounds_pending: false,
                pose: Vec::new(),
                pose_dirty: true,
                posed_at: f64::NAN,
                controls: AnimatedMeshPose::default(),
                posed_world: Mat4::IDENTITY,
                joints_pending: false,
                rest_reported: false,
                ik: Vec::new(),
                skinned: HashMap::new(),
                part_nodes: Vec::new(),
                parameters: BTreeMap::new(),
            },
        );
        if let Some(playback) = &descriptor.playback {
            let _ = self.set_animated_playback(handle, playback);
        }
        let _ = self.set_animated_inspection(handle, &descriptor.inspection);
        self.reset_animated_instance(handle);
    }

    /// Re-derive an instance's pose buffers and parts from its asset.
    /// The instance's asset may have changed (created, redefined, released):
    /// its playback clips, its controllers' clips and its joint-attached
    /// children's joints are resolved again against it.
    fn reset_animated_instance(&mut self, handle: RenderHandle) {
        let Some(instance) = self.tables.animated.get_mut(&handle) else {
            return;
        };
        let asset = self.tables.animated_assets.get(instance.slot);
        instance.playback.resolve(&clip_index(asset));
        instance.skinned.clear();
        instance.pose.clear();
        instance.pose_dirty = true;
        self.resolve_controllers_of(handle);
        self.resolve_attached_joints(handle);
        self.pose_animated_instance(handle);
        self.rebuild_parts(handle);
    }

    /// Resolve each joint-attached child of `parent` to its joint's node in
    /// the parent's asset.
    pub(crate) fn resolve_attached_joints(&mut self, parent: RenderHandle) {
        let Some(children) = self
            .tables
            .nodes
            .get(&parent)
            .map(|node| node.children.clone())
        else {
            return;
        };
        for child in children {
            let joint = self
                .tables
                .nodes
                .get(&child)
                .and_then(|node| node.parent_joint.as_deref())
                .and_then(|name| self.joint_node(parent, name));
            if let Some(node) = self.tables.nodes.get_mut(&child) {
                node.parent_joint_node = joint;
            }
        }
    }

    /// A unique joint's node index in `parent`'s animated asset.
    pub(crate) fn joint_node(&self, parent: RenderHandle, joint: &str) -> Option<usize> {
        let instance = self.tables.animated.get(&parent)?;
        let asset = self.tables.animated_assets.get(instance.slot)?;
        asset.joints.get(joint).copied()
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
        instance.completion = None;
        instance.playback.apply(command, now)?;
        match command {
            AnimatedMeshPlaybackCommand::Play {
                clip,
                r#loop,
                start_offset_seconds,
                start_paused,
                ..
            } => {
                let duration = durations.get(clip).copied().unwrap_or(0.0);
                let started_at_end =
                    start_offset_seconds.is_some_and(|offset| offset >= f64::from(duration));
                if *r#loop == AnimationLoopMode::Once && !start_paused && !started_at_end {
                    instance.completion = Some(clip.clone());
                }
            }
            AnimatedMeshPlaybackCommand::Resume => {
                if let AnimatedMeshPlaybackTimeline::Playing {
                    clip,
                    r#loop: AnimationLoopMode::Once,
                    ..
                } = &instance.playback.playback.timeline
                {
                    instance.completion = Some(clip.clone());
                }
            }
            _ => {}
        }
        let asset = self.tables.animated_assets.get(instance.slot);
        instance.playback.resolve(&clip_index(asset));
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
        let restyled = inspection.matte != instance.inspection.matte
            || inspection.wireframe != instance.inspection.wireframe;
        instance.inspection = inspection.clone();
        if restyled {
            self.rebuild_parts(handle);
        }
        let mut unrealized = Vec::new();
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
            .and_then(|instance| self.tables.animated_assets.get(instance.slot))
            .map(|asset| {
                asset
                    .clip_ids
                    .iter()
                    .map(|(id, index)| (id.clone(), asset.clips[*index].duration))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Advance every instance whose pose can change to the current Engine
    /// time: by time, or for world-space pose controls by its placement.
    /// Called between two transform propagations: the first places the
    /// instances, the second their joint-attached children.
    pub(crate) fn advance_animations(&mut self) {
        self.advance_animations_where(|_| true);
    }

    fn advance_animations_where(&mut self, selected: impl Fn(&AnimatedInstance) -> bool) {
        let handles: Vec<RenderHandle> = self
            .tables
            .animated
            .iter()
            .filter(|(handle, instance)| {
                if !selected(instance) {
                    return false;
                }
                let controlled = self.tables.controlled.contains_key(*handle);
                let moved = instance.controls.reads_world()
                    && self
                        .tables
                        .nodes
                        .get(handle)
                        .is_some_and(|node| node.world != instance.posed_world);
                instance.pose_dirty
                    || moved
                    || ((controlled || instance.varies() || instance.controls.report_joints)
                        && instance.posed_at != self.animation_time)
            })
            .map(|(handle, _)| *handle)
            .collect();
        for handle in handles {
            self.pose_animated_instance(handle);
        }
        // A reporting instance that moved without a new pose reports again:
        // its world joints follow its placement.
        for (handle, instance) in self.tables.animated.iter_mut() {
            if !selected(instance) || !instance.controls.report_joints {
                continue;
            }
            if let Some(node) = self.tables.nodes.get(handle) {
                if node.world != instance.posed_world {
                    instance.posed_world = node.world;
                    instance.joints_pending = true;
                }
            }
        }
    }

    /// Pose and report every instance that reports its joints, at the
    /// time of the call just applied, so the Engine reads each call's pose
    /// before the next call whether or not a frame draws.
    pub(crate) fn pose_reporting_instances(&mut self) {
        if !self
            .tables
            .animated
            .values()
            .any(|instance| instance.controls.report_joints)
        {
            return;
        }
        self.propagate_transforms();
        self.advance_animations_where(|instance| instance.controls.report_joints);
        self.propagate_transforms();
        self.report_joint_poses();
    }

    /// Replace an instance's pose controls.
    pub(crate) fn set_animated_pose(
        &mut self,
        handle: RenderHandle,
        pose: &AnimatedMeshPose,
    ) -> Result<(), String> {
        let instance = self
            .tables
            .animated
            .get_mut(&handle)
            .ok_or_else(|| format!("unknown animated mesh {}", handle.raw()))?;
        if &instance.controls == pose {
            return Ok(());
        }
        if pose.report_joints && !instance.controls.report_joints {
            instance.rest_reported = false;
        }
        instance.controls = pose.clone();
        instance.pose_dirty = true;
        Ok(())
    }

    /// Report the joints of every reporting instance posed since its last
    /// report. Runs after transforms propagate, so the instance's world
    /// placement is current.
    pub(crate) fn report_joint_poses(&mut self) {
        let now = self.animation_time;
        let pending: Vec<RenderHandle> = self
            .tables
            .animated
            .iter()
            .filter(|(_, instance)| instance.joints_pending)
            .map(|(handle, _)| *handle)
            .collect();
        for handle in pending {
            let world = self
                .tables
                .nodes
                .get(&handle)
                .map_or(Mat4::IDENTITY, |node| node.world);
            let Some(instance) = self.tables.animated.get_mut(&handle) else {
                continue;
            };
            instance.joints_pending = false;
            let (Some(object_id), Some(asset)) = (
                instance.object_id,
                self.tables.animated_assets.get(instance.slot),
            ) else {
                continue;
            };
            let joints = |pose: &[Mat4]| animation_pose::posed_joints(&asset.rig, pose, world);
            let rest = (!instance.rest_reported).then(|| {
                let locals: Vec<Trs> = asset.model.nodes.iter().map(|node| node.rest).collect();
                joints(&animation_pose::compose(&asset.model, &locals))
            });
            instance.rest_reported = true;
            let ik_chains = instance
                .ik
                .iter()
                .map(|solve| {
                    let end = solve
                        .end
                        .and_then(|node| instance.pose.get(node))
                        .map_or(Vec3::ZERO, |end| end.w_axis.truncate());
                    let (target_world, end_world) = (
                        world.transform_point3(solve.target),
                        world.transform_point3(end),
                    );
                    IkChainReach {
                        target_model: solve.target.to_array(),
                        target_world: target_world.to_array(),
                        end_model: end.to_array(),
                        end_world: end_world.to_array(),
                        shortfall: end_world.distance(target_world),
                        straightened: solve.straightened,
                    }
                })
                .collect();
            self.animation_facts.push(AnimationFact::JointPose {
                object_id,
                generation: instance.generation,
                seconds: now,
                world: convert::transform_of(world),
                joints: joints(&instance.pose),
                rest,
                ik_chains,
            });
        }
    }

    /// Evaluate one instance's pose at the current time and push it to its
    /// parts, skinned buffers, joint-attached children and facts.
    fn pose_animated_instance(&mut self, handle: RenderHandle) {
        let now = self.animation_time;
        let Some(instance) = self.tables.animated.get_mut(&handle) else {
            return;
        };
        let Some(asset) = self.tables.animated_assets.get(instance.slot) else {
            instance.pose_dirty = false;
            return;
        };
        // The first of the controllers driving it, if any.
        let controller = self
            .tables
            .controlled
            .get(&handle)
            .and_then(|controllers| controllers.first())
            .and_then(|controller| self.tables.controllers.get(controller));
        let duration = |clip: usize| asset.clips[clip].duration;
        let (actions, finished) = instance.playback.actions(
            now,
            &duration,
            controller.map(|controller| controller.playback.actions(now, &duration)),
        );
        let world = self
            .tables
            .nodes
            .get(&handle)
            .map_or(Mat4::IDENTITY, |node| node.world);
        // Recorded for the joint report and for an IK overlay.
        let record =
            instance.controls.report_joints || self.overlays.shown.ik.contains(&handle.raw());
        if !record {
            instance.ik.clear();
        }
        let mut locals = animation_pose::sample_locals(&asset.model, &asset.clips, &actions);
        animation_pose::layer_clips(
            &asset.model,
            &asset.clips,
            &mut locals,
            &instance.controls.clip_layers,
            &asset.rig,
            &clip_index(Some(asset)),
        );
        let pose = animation_pose::controlled(
            &asset.model,
            &asset.rig,
            locals,
            &instance.controls,
            world,
            record.then_some(&mut instance.ik),
        );
        instance.posed_world = world;
        instance.joints_pending = instance.controls.report_joints;
        // A changed pose is a scene change: cached composition targets must
        // redraw it. An unchanged pose (held, sampled, finished) keeps them.
        let moved = pose != instance.pose;
        instance.pose = pose;
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
        if moved {
            self.scene_generation += 1;
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
        let Some(asset) = self.tables.animated_assets.get(instance.slot) else {
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
                let mut tangents =
                    Vec::with_capacity(primitive.tangents.as_ref().map_or(0, Vec::len));
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
                    positions.push(
                        matrix
                            .transform_point3(crate::convert::vec3(*position))
                            .to_array(),
                    );
                    let normal = matrix
                        .transform_vector3(crate::convert::vec3(primitive.normals[vertex]))
                        .normalize_or(Vec3::Y);
                    normals.push(normal.to_array());
                    if let Some([x, y, z, w]) =
                        primitive.tangents.as_ref().map(|tangents| tangents[vertex])
                    {
                        let [x, y, z] = matrix
                            .transform_vector3(Vec3::new(x, y, z))
                            .normalize_or(Vec3::X)
                            .to_array();
                        tangents.push([x, y, z, w]);
                    }
                }
                let vertices = interleave(primitive, &positions, &normals);
                let extra = extra_stream(
                    &asset.model,
                    primitive,
                    (!tangents.is_empty()).then_some(tangents.as_slice()),
                );
                updates.push(((node_index as u32, primitive_index as u32), vertices, extra));
            }
        }
        for (key, vertices, extra) in updates {
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
                if let (Some(buffer), Some(extra)) = (&mesh.extra, &extra) {
                    self.gpu
                        .queue
                        .write_buffer(buffer, 0, bytemuck::cast_slice(extra));
                }
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
                let slot = self.tables.animated[&handle].slot;
                let asset = self
                    .tables
                    .animated_assets
                    .get(slot)
                    .expect("a skinned instance's asset is defined");
                let node = &asset.model.nodes[key.0 as usize];
                let primitive = &asset.model.meshes[node.mesh.expect("skinned node has a mesh")]
                    [key.1 as usize];
                let indices = primitive.indices.clone();
                let label = format!("animated {} skin {}/{}", handle.raw(), key.0, key.1);
                let mut mesh = self.upload_dynamic_vertices(&label, &vertices, &indices);
                mesh.extra = extra.map(|extra| self.vertex_buffer(&label, &extra, true));
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
        // `rebuild_parts` sets both from one `animated_parts` call.
        debug_assert_eq!(node.parts.len(), instance.part_nodes.len());
        let asset = self.tables.animated_assets.get(instance.slot);
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
            self.tables.parts.write(
                *part,
                &node.world,
                node.world_visible,
                node.world_layer,
                node.shadow_casting.is_cast(),
            );
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
            bounds: bounds.map(|bounds| {
                (
                    crate::convert::array(bounds.min),
                    crate::convert::array(bounds.max),
                )
            }),
        });
    }

    /// Exact posed world bounds: every vertex at its current pose.
    pub(crate) fn animated_world_bounds(&self, handle: RenderHandle) -> Option<Aabb> {
        let instance = self.tables.animated.get(&handle)?;
        let asset = self.tables.animated_assets.get(instance.slot)?;
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
                    bounds.include(matrix.transform_point3(crate::convert::vec3(*position)));
                }
            }
        }
        let finite = bounds.min.is_finite() && bounds.max.is_finite();
        (!bounds.is_empty() && finite).then_some(bounds)
    }

    /// A joint's current transform in its instance's space.
    /// Every uploaded animated mesh: assets' rigid primitives and instances'
    /// skinned buffers.
    pub(crate) fn for_each_animated_mesh(&self, visit: &mut dyn FnMut(&GpuMesh)) {
        for asset in self.tables.animated_assets.values() {
            asset.rigid.values().for_each(&mut *visit);
        }
        for instance in self.tables.animated.values() {
            instance.skinned.values().for_each(&mut *visit);
        }
    }

    /// The posed transform of a node of `handle`'s asset, by index: what a
    /// joint-attached child hangs from each frame.
    pub(crate) fn joint_pose_at(&self, handle: RenderHandle, node: usize) -> Option<Mat4> {
        self.tables.animated.get(&handle)?.pose.get(node).copied()
    }

    /// The node of an animated instance's rig joint.
    pub(crate) fn rig_node(&self, handle: RenderHandle, joint: u32) -> Option<usize> {
        let instance = self.tables.animated.get(&handle)?;
        let asset = self.tables.animated_assets.get(instance.slot)?;
        asset.rig.get(joint as usize).copied().flatten()
    }

    /// Each rig joint of an animated instance with the nearest rig joint
    /// above it, as drawn, in the instance's space: its bones.
    pub(crate) fn joint_bones(&self, handle: RenderHandle) -> Vec<(Vec3, Vec3)> {
        let Some(instance) = self.tables.animated.get(&handle) else {
            return Vec::new();
        };
        let Some(asset) = self.tables.animated_assets.get(instance.slot) else {
            return Vec::new();
        };
        let in_rig: std::collections::HashSet<usize> =
            asset.rig.iter().flatten().copied().collect();
        let at = |node: usize| instance.pose.get(node).map(|pose| pose.w_axis.truncate());
        asset
            .rig
            .iter()
            .flatten()
            .filter_map(|&node| {
                let mut parent = asset.model.nodes[node].parent;
                while let Some(above) = parent.filter(|above| !in_rig.contains(above)) {
                    parent = asset.model.nodes[above].parent;
                }
                Some((at(node)?, at(parent?)?))
            })
            .collect()
    }

    /// An animated instance's IK chains from its last posing, in its space:
    /// each target, the drawn end joint, and whether the chain straightened.
    pub(crate) fn ik_reaches(&self, handle: RenderHandle) -> Vec<(Vec3, Option<Vec3>, bool)> {
        let Some(instance) = self.tables.animated.get(&handle) else {
            return Vec::new();
        };
        instance
            .ik
            .iter()
            .map(|solve| {
                let end = solve
                    .end
                    .and_then(|node| instance.pose.get(node))
                    .map(|pose| pose.w_axis.truncate());
                (solve.target, end, solve.straightened)
            })
            .collect()
    }

    /// Poses an animated instance again at the next frame.
    pub(crate) fn repose_animated(&mut self, handle: RenderHandle) {
        if let Some(instance) = self.tables.animated.get_mut(&handle) {
            instance.pose_dirty = true;
        }
    }

    /// The parts of an animated instance's node, in draw order: each GLB node
    /// with a mesh, each primitive, with its material binding.
    /// Each drawn primitive's mesh, index count, material and material slot;
    /// the materials it names, defined or not, are noted in `used`. A
    /// primitive whose mesh is not uploaded (skinned joints without
    /// weights) draws nothing.
    pub(crate) fn animated_parts(
        &mut self,
        handle: RenderHandle,
        used: &mut Vec<u32>,
    ) -> Vec<(crate::tables::MeshRef, u32, MaterialRef, Option<u16>)> {
        let Some(instance) = self.tables.animated.get(&handle) else {
            return Vec::new();
        };
        let Some(asset) = self.tables.animated_assets.get(instance.slot) else {
            // No parts, so no part nodes: a redefinition re-derives the parts
            // between retiring the old asset and admitting the new one.
            if let Some(instance) = self.tables.animated.get_mut(&handle) {
                instance.part_nodes.clear();
            }
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
                        instance.slot,
                        mesh as u32,
                        primitive_index as u32,
                    )
                };
                let uploaded = match &mesh_ref {
                    crate::tables::MeshRef::AnimatedSkinned(_, node, primitive) => {
                        instance.skinned.contains_key(&(*node, *primitive))
                    }
                    _ => asset
                        .rigid
                        .contains_key(&(mesh as u32, primitive_index as u32)),
                };
                if !uploaded {
                    continue;
                }
                let slot = primitive.material.and_then(|glb_material| {
                    asset
                        .slots
                        .iter()
                        .find(|(_, source)| **source == glb_material)
                        .map(|(slot, _)| *slot)
                });
                let material = match primitive.material {
                    Some(glb_material) => {
                        let overridden = slot.and_then(|slot| overrides.get(&slot));
                        let id = overridden
                            .cloned()
                            .unwrap_or_else(|| asset.materials[glb_material].clone());
                        used.push(self.tables.names.id(&id));
                        if matte {
                            let matte_id = self.tables.names.id(&format!("{id}#matte"));
                            used.push(matte_id);
                            mattes.push(id);
                            MaterialRef::Retained(matte_id)
                        } else {
                            MaterialRef::Retained(self.tables.names.id(&id))
                        }
                    }
                    None => MaterialRef::LitFallback,
                };
                parts.push((mesh_ref, primitive.indices.len() as u32, material, slot));
                part_nodes.push(node_index);
            }
        }
        if let Some(instance) = self.tables.animated.get_mut(&handle) {
            instance.part_nodes = part_nodes;
        }
        // Matte inspection: roughness 1, metalness 0 variants of the bound
        // materials.
        for id in mattes {
            let matte_id = format!("{id}#matte");
            if crate::tables::named(&self.tables.names, &self.tables.materials, &matte_id).is_some()
            {
                continue;
            }
            if let Some((_, row)) =
                crate::tables::named(&self.tables.names, &self.tables.materials, &id)
            {
                let mut descriptor = row.descriptor.clone();
                descriptor.id = matte_id;
                descriptor.roughness = 1.0;
                descriptor.metalness = 0.0;
                let maps = row.maps.clone();
                self.define_material_with(descriptor, false, maps);
            }
        }
        parts
    }

    /// Upload a vertex buffer the CPU rewrites (skinned primitives).
    /// A vertex buffer holding `data`, rewritable when `dynamic`.
    fn vertex_buffer(&self, label: &str, data: &[f32], dynamic: bool) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;
        let usage = if dynamic {
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::VERTEX
        };
        self.gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage,
            })
    }

    fn upload_dynamic_vertices(&self, label: &str, vertices: &[f32], indices: &[u32]) -> GpuMesh {
        use wgpu::util::DeviceExt;
        let device = &self.gpu.device;
        GpuMesh {
            bounds: vertex_bounds(vertices),
            cpu: std::sync::Arc::new(crate::tables::CpuGeometry {
                positions: vertex_positions(vertices),
                indices: indices.to_vec(),
            }),
            edges: Default::default(),
            extra: None,
            texture_space: None,
            distance_field: None,
            layer_weights: false,
            layer_palette: Vec::new(),
            vertex_occlusion: false,
            morph: None,
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
                .get(*asset)
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

    /// Material with GLB extras: the unlit extension and the emissive,
    /// normal and occlusion maps.
    pub(crate) fn define_material_with(
        &mut self,
        descriptor: RenderMaterialDescriptor,
        unlit: bool,
        maps: MaterialMaps,
    ) {
        let texture = descriptor
            .texture
            .as_ref()
            .and_then(|id| self.tables.textures.get(id));
        let mut params = MaterialParams::of(&descriptor, texture.map(|texture| texture.size));
        params.maps = maps;
        params.unlit = unlit;
        // GLB materials name no product shader, so all compose.
        let _ = self.insert_material(descriptor, &params);
    }
}

impl AnimatedInstance {
    /// Whether the pose moves with time.
    fn varies(&self) -> bool {
        if self.playback.fade.is_some() {
            return true;
        }
        match &self.playback.playback.timeline {
            // A `once` clip settles once its completion is reported.
            AnimatedMeshPlaybackTimeline::Playing { r#loop, .. } => {
                *r#loop != AnimationLoopMode::Once || self.completion.is_some()
            }
            _ => false,
        }
    }
}

/// An asset's clip index by descriptor id; no asset resolves nothing.
fn clip_index(asset: Option<&AnimatedAssetRow>) -> impl Fn(&str) -> Option<usize> + '_ {
    move |name| asset.and_then(|asset| asset.clip_ids.get(name).copied())
}

/// The second vertex stream a primitive draws with, when its material reads
/// tangents (a normal map) or `TEXCOORD_1`: tangent (xyz, handedness) and
/// uv1 per vertex. Without TANGENT a normal-mapped primitive has MikkTSpace
/// tangents (`glb::decode`); a vertex without one gets +X.
fn extra_stream(
    model: &GlbModel,
    primitive: &glb::GlbPrimitive,
    tangents: Option<&[[f32; 4]]>,
) -> Option<Vec<f32>> {
    let material = model.materials.get(primitive.material?)?;
    let sets = [
        material.base_color_texture.map(|slot| slot.tex_coord),
        material.emissive_texture.map(|slot| slot.tex_coord),
        material.normal_texture.map(|(slot, _)| slot.tex_coord),
        material.occlusion_texture.map(|(slot, _)| slot.tex_coord),
    ];
    let second_set = sets.contains(&Some(1)) && primitive.uvs1.is_some();
    if material.normal_texture.is_none() && !second_set {
        return None;
    }
    let mut extra = Vec::with_capacity(primitive.positions.len() * EXTRA_VERTEX_FLOATS);
    for vertex in 0..primitive.positions.len() {
        let tangent = tangents
            .and_then(|tangents| tangents.get(vertex))
            .copied()
            .unwrap_or([1.0, 0.0, 0.0, 1.0]);
        extra.extend_from_slice(&tangent);
        let uv = primitive
            .uvs1
            .as_ref()
            .and_then(|uvs| uvs.get(vertex))
            .copied()
            .unwrap_or([0.0, 0.0]);
        extra.extend_from_slice(&uv);
    }
    Some(extra)
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
        .as_chunks::<VERTEX_FLOATS>()
        .0
        .iter()
        .map(|vertex| Vec3::new(vertex[0], vertex[1], vertex[2]))
        .collect()
}

fn vertex_bounds(vertices: &[f32]) -> Aabb {
    let mut bounds = Aabb::EMPTY;
    for vertex in vertices.as_chunks::<VERTEX_FLOATS>().0 {
        bounds.include(Vec3::new(vertex[0], vertex[1], vertex[2]));
    }
    bounds
}

/// The skin joints retained children attach to, by name
/// ([`animation_pose::joint_nodes`]).
pub fn joint_nodes(model: &GlbModel) -> HashMap<String, usize> {
    animation_pose::joint_nodes(model)
}
