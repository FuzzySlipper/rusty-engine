//! Animated meshes: GLB assets, instances drawn in the poses the Engine
//! evaluated for them, skinning, joint attachments, and the bounds facts
//! the runtime reads back.
//!
//! **Poses.** This renderer never evaluates a pose. The Engine evaluates
//! each instance's pose once per settled call (`animation_pose::AnimatedScene`)
//! and publishes it as `SetEvaluatedPose`: each GLB node's transform in the
//! instance's space, a severed piece's nodes, and the IK chains for
//! inspection. An instance without a pose that fits its mesh draws at rest.
//!
//! **Cost.** A pose is realized when a new one arrives and a frame draws:
//! rigid (unskinned) GLB nodes upload their geometry once per asset and move
//! by their part's local matrix; skinned primitives are skinned on the CPU
//! into the instance's own vertex buffer. A held pose costs nothing per
//! frame.
//!
//! **Motion.** While frames draw motion (`motion.rs`), a new pose is skinned
//! into a second buffer, so the one the last frame drew stays as it was and
//! the motion reads each vertex's previous position from it
//! (`SkinnedHistory`). Once the new pose has drawn, the motion reads the
//! current vertices again: a held pose has no skinning motion. Nodes drawn
//! rigid, severed pieces and children on joints are parts that move, so
//! their previous models (`motion.rs`) are the previous pose's; a ragdoll is
//! a pose like any other.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use glam::{Mat4, Vec3};
use render_model::{
    AnimatedMeshAsset, AnimatedMeshInspection, AnimatedMeshInstanceDescriptor, EvaluatedPose,
    MaterialAlphaModeDescriptor, MaterialInstanceParameters, MaterialUvStrategy, RenderHandle,
    RenderMaterialDescriptor,
};

use crate::apply::{MapSlot, MaterialMaps, MaterialParams};
use crate::glb::{self, GlbAlpha, GlbClip, GlbModel, Trs};
use crate::pipelines::{EXTRA_VERTEX_FLOATS, VERTEX_FLOATS};
use crate::resources::ResourceSource;
use crate::tables::{Aabb, GpuMesh, MaterialRef, NodeKind, SkinnedHistory, Topology};
use crate::Renderer;

/// What the runtime reads back from animated meshes (animation feedback
/// facts), keyed by the instance's source entity and its
/// per-entity realization generation.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationFact {
    /// Posed world bounds for a changed `bounds_request`; `None` when the
    /// instance draws nothing.
    MeshInspection {
        object_id: u64,
        generation: u64,
        request: u32,
        bounds: Option<([f32; 3], [f32; 3])>,
    },
}

/// One admitted animated-mesh asset.
pub(crate) struct AnimatedAssetRow {
    model: GlbModel,
    /// Each node at rest, in the instance's space: what an instance without
    /// a fitting evaluated pose draws.
    rest: Vec<Mat4>,
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
    /// what cuts and the joint overlay index.
    rig: Vec<Option<usize>>,
}

pub(crate) struct AnimatedInstance {
    /// The asset's slot in `animated_assets` (its `names` id).
    slot: u32,
    object_id: Option<u64>,
    generation: u64,
    pub(crate) inspection: AnimatedMeshInspection,
    bounds_pending: bool,
    /// The pose the Engine evaluated for it last.
    pose: Option<EvaluatedPose>,
    /// A pose arrived that its skinned buffers and parts do not show yet.
    pose_pending: bool,
    /// Each node's severed piece, by the pose's pieces: the piece whose
    /// nodes place it.
    owners: Vec<Option<usize>>,
    /// Per-instance skinned vertex buffers, by (node, primitive).
    skinned: HashMap<(u32, u32), GpuMesh>,
    /// The frames with motion drawn when `skinned` was last written
    /// (`Temporal::motion_frames`): a later frame has drawn it.
    skinned_at: u64,
    /// The GLB node each of the node row's parts draws, in part order.
    part_nodes: Vec<usize>,
    /// The skinned triangles split between the instance and its severed
    /// pieces, for the cut joints it was made for.
    split: Option<CutSplit>,
    /// Per material slot values over the slot's material
    /// (`SetMaterialInstanceParameters`).
    pub(crate) parameters: BTreeMap<u16, MaterialInstanceParameters>,
    /// Skinned primitives the device had no room for: they draw nothing and
    /// are not skinned again until the instance is made again.
    refused: BTreeSet<(u32, u32)>,
    /// Why the last of them was refused, until the operation that made the
    /// instance reports it.
    refusal: Option<String>,
}

/// A skinned mesh's vertices as the last frame with motion drew them: their
/// positions, and the index of each copy by its source vertex and cut joint.
struct DrawnVertices {
    positions: std::sync::Arc<crate::tables::CpuGeometry>,
    copies: HashMap<(u32, u32), usize>,
}

/// The triangles of each skinned primitive split by an instance's cuts
/// ([`render_model::JointCut`]): the instance keeps its own vertices and the
/// triangles left to it, and each piece draws copies of its triangles'
/// vertices, skinned at the piece's placement.
struct CutSplit {
    /// The cut joints, in the order of the pose's pieces.
    joints: Vec<u32>,
    /// By (node, primitive); a primitive no cut reaches is absent.
    primitives: HashMap<(u32, u32), SplitPrimitive>,
}

struct SplitPrimitive {
    /// The vertices drawn after the primitive's own: each one's source
    /// vertex and the cut whose piece draws it.
    copies: Vec<(u32, usize)>,
    /// The instance's triangles, then each piece's.
    indices: Vec<u32>,
}

impl CutSplit {
    fn new(model: &GlbModel, rig: &[Option<usize>], joints: Vec<u32>) -> Self {
        let owners = animation_pose::node_cuts(model, rig, &joints);
        let mut primitives = HashMap::new();
        for &node_index in &model.order {
            let node = &model.nodes[node_index];
            let (Some(mesh), Some(skin)) = (node.mesh, node.skin) else {
                continue;
            };
            let skin = &model.skins[skin];
            for (primitive_index, primitive) in model.meshes[mesh].iter().enumerate() {
                let (Some(influences), Some(weights)) = (&primitive.joints, &primitive.weights)
                else {
                    continue;
                };
                // Each vertex goes where the larger part of its weight is;
                // a tie stays with the instance.
                let vertex_cuts: Vec<Option<usize>> = influences
                    .iter()
                    .zip(weights)
                    .map(|(joints, weights)| {
                        let mut shares: [(Option<usize>, f32); 4] = [(None, 0.0); 4];
                        let mut kept = 0.0;
                        for (joint, weight) in joints.iter().zip(weights) {
                            let owner = skin
                                .joints
                                .get(usize::from(*joint))
                                .and_then(|node| owners[*node]);
                            match owner {
                                None => kept += weight,
                                Some(cut) => {
                                    if let Some(share) =
                                        shares.iter_mut().find(|(owner, _)| *owner == Some(cut))
                                    {
                                        share.1 += weight;
                                    } else if let Some(share) =
                                        shares.iter_mut().find(|(owner, _)| owner.is_none())
                                    {
                                        *share = (Some(cut), *weight);
                                    }
                                }
                            }
                        }
                        let (cut, weight) =
                            shares.iter().copied().fold((None, 0.0), |best, share| {
                                if share.1 > best.1 {
                                    share
                                } else {
                                    best
                                }
                            });
                        (weight > kept).then_some(cut).flatten()
                    })
                    .collect();
                let mut kept = Vec::with_capacity(primitive.indices.len());
                let mut severed: Vec<(usize, [u32; 3])> = Vec::new();
                for triangle in primitive.indices.as_chunks::<3>().0 {
                    let [a, b, c] = [0, 1, 2].map(|corner| vertex_cuts[triangle[corner] as usize]);
                    let cut = if a == b || a == c {
                        a
                    } else if b == c {
                        b
                    } else {
                        None
                    };
                    match cut {
                        Some(cut) => severed.push((cut, *triangle)),
                        None => kept.extend_from_slice(triangle),
                    }
                }
                if severed.is_empty() {
                    continue;
                }
                let own = primitive.positions.len() as u32;
                let mut copies = Vec::new();
                let mut copied: HashMap<(u32, usize), u32> = HashMap::new();
                severed.sort_by_key(|(cut, _)| *cut);
                for (cut, triangle) in severed {
                    for vertex in triangle {
                        let index = *copied.entry((vertex, cut)).or_insert_with(|| {
                            copies.push((vertex, cut));
                            own + copies.len() as u32 - 1
                        });
                        kept.push(index);
                    }
                }
                primitives.insert(
                    (node_index as u32, primitive_index as u32),
                    SplitPrimitive {
                        copies,
                        indices: kept,
                    },
                );
            }
        }
        Self { joints, primitives }
    }
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
    let model = glb::decode(&bytes, true)?;
    let clips = animation_pose::asset_clips(asset, &model, model.clips.clone(), |identity| {
        resources.bytes(identity)
    })?;
    Ok((model, clips))
}

impl Renderer {
    /// The Engine presentation timeline in seconds (`PresentationWorld`'s
    /// elapsed time) the committed call reached: video and its facts advance
    /// on it, not on display time. Frames draw at it too, until
    /// [`Self::set_presentation_time`] moves them on.
    pub fn set_animation_time(&mut self, seconds: f64) {
        debug_assert!(seconds.is_finite() && seconds >= 0.0, "{seconds}");
        self.animation_time = seconds;
        self.presentation_time = seconds;
    }

    /// The presentation time the next frames draw at: their `frame.time`,
    /// cloud drift, wind sway, precipitation and fog. A host whose world
    /// runs on between committed steps gives the time it has reached, so
    /// what moves with time moves every frame; video and its facts stay on
    /// the committed time ([`Self::set_animation_time`]), and poses are the
    /// ones the Engine evaluated at it.
    pub fn set_presentation_time(&mut self, seconds: f64) {
        debug_assert!(seconds.is_finite() && seconds >= 0.0, "{seconds}");
        self.presentation_time = seconds;
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
        let model = glb::decode(&bytes, false)?;

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
                sinks_under_snow: false,
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
                let vertices = interleave(
                    primitive,
                    &primitive.positions,
                    &primitive.normals,
                    &|vertex| vertex,
                );
                let mut mesh = self.upload_vertices(
                    &format!("{label} mesh {mesh_index}/{primitive_index}"),
                    &vertices,
                    &primitive.indices,
                    Topology::Triangles,
                    vec![(0, 0, primitive.indices.len() as u32)],
                    BTreeMap::new(),
                )?;
                mesh.extra = extra_stream(
                    &model,
                    primitive,
                    primitive.positions.len(),
                    &|vertex| vertex,
                    primitive.tangents.as_deref(),
                )
                .map(|extra| self.vertex_buffer(&format!("{label} tangents"), &extra, false))
                .transpose()?;
                rigid.insert((mesh_index as u32, primitive_index as u32), mesh);
            }
        }
        // A material drawn with the second stream or skinned needs its own
        // pipelines: make them now rather than at its first draw.
        let streamed: BTreeSet<(usize, crate::shaders::Features)> = model
            .meshes
            .iter()
            .flatten()
            .filter_map(|primitive| {
                let tangents = extra_stream(&model, primitive, 0, &|vertex| vertex, None).is_some();
                let streams = crate::shaders::Features::default()
                    .with(crate::shaders::Features::VERTEX_TANGENTS, tangents)
                    .with(
                        crate::shaders::Features::SKINNED,
                        primitive.joints.is_some(),
                    );
                (streams != Default::default()).then_some((primitive.material?, streams))
            })
            .collect();
        for (index, streams) in streamed {
            let prepared = crate::tables::named(
                &self.tables.names,
                &self.tables.materials,
                &materials[index],
            )
            .map(|(_, row)| {
                (
                    row.features | streams,
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
        let rest = {
            let locals: Vec<Trs> = model.nodes.iter().map(|node| node.rest).collect();
            animation_pose::compose(&model, &locals)
        };
        let slots = asset
            .embedded_material_slots
            .iter()
            .map(|slot| (slot.slot, usize::from(slot.source_material_slot)))
            .collect();
        let slot = self.tables.names.id(&asset.asset);
        self.tables.animated_assets.insert(
            slot,
            AnimatedAssetRow {
                model,
                rest,
                rigid,
                materials,
                textures,
                linear_textures,
                slots,
                joints,
                rig,
            },
        );
        // Live instances (a redefinition keeps them) draw on the new asset.
        let mut refusal = None;
        for handle in self.instances_of(slot) {
            refusal = self.reset_animated_instance(handle).or(refusal);
        }
        refusal.map_or(Ok(()), Err)
    }

    pub(crate) fn release_animated_mesh(&mut self, asset: &str) {
        self.release_animated_mesh_resources(asset);
        if let Some(slot) = self.tables.names.get(asset) {
            for handle in self.instances_of(slot) {
                // Released, it draws nothing to refuse.
                let _ = self.reset_animated_instance(handle);
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
    /// exists: a new generation, then its parts, at rest until its pose
    /// arrives.
    pub(crate) fn create_animated_instance(&mut self, handle: RenderHandle) -> Result<(), String> {
        let Some(NodeKind::AnimatedMesh(descriptor)) =
            self.tables.nodes.get(&handle).map(|node| &node.kind)
        else {
            return Ok(());
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
                inspection: AnimatedMeshInspection::default(),
                bounds_pending: false,
                pose: None,
                pose_pending: false,
                owners: Vec::new(),
                skinned: HashMap::new(),
                skinned_at: 0,
                part_nodes: Vec::new(),
                split: None,
                parameters: BTreeMap::new(),
                refused: BTreeSet::new(),
                refusal: None,
            },
        );
        let _ = self.set_animated_inspection(handle, &descriptor.inspection);
        self.reset_animated_instance(handle).map_or(Ok(()), Err)
    }

    /// Re-derive an instance's skinned buffers and parts from its asset.
    /// The instance's asset may have changed (created, redefined, released):
    /// its joint-attached children's joints are resolved again against it.
    /// Returns why a skinned primitive was refused, if one was.
    fn reset_animated_instance(&mut self, handle: RenderHandle) -> Option<String> {
        let instance = self.tables.animated.get_mut(&handle)?;
        instance.skinned.clear();
        instance.refused.clear();
        instance.split = None;
        instance.pose_pending = false;
        self.resolve_attached_joints(handle);
        self.skin_animated_instance(handle);
        self.rebuild_parts(handle);
        self.tables
            .animated
            .get_mut(&handle)
            .and_then(|instance| instance.refusal.take())
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

    /// Draw an instance in the pose the Engine evaluated for it: its
    /// skinned buffers and parts follow at the next frame, its
    /// joint-attached children at the next propagation.
    pub(crate) fn set_evaluated_pose(
        &mut self,
        handle: RenderHandle,
        pose: &EvaluatedPose,
    ) -> Result<(), String> {
        let instance = self
            .tables
            .animated
            .get_mut(&handle)
            .ok_or_else(|| format!("unknown animated mesh {}", handle.raw()))?;
        let joints = |pose: Option<&EvaluatedPose>| -> Vec<u32> {
            pose.iter()
                .flat_map(|pose| &pose.pieces)
                .map(|piece| piece.joint)
                .collect()
        };
        let cuts = joints(Some(pose));
        if cuts != joints(instance.pose.as_ref()) {
            instance.owners = match self.tables.animated_assets.get(instance.slot) {
                Some(asset) if !cuts.is_empty() => {
                    animation_pose::node_cuts(&asset.model, &asset.rig, &cuts)
                }
                _ => Vec::new(),
            };
        }
        instance.pose = Some(pose.clone());
        instance.pose_pending = true;
        // A changed pose is a scene change: cached composition targets must
        // redraw it.
        self.scene_generation += 1;
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
        Ok(())
    }

    /// Skin every instance whose pose arrived since it was last drawn, and
    /// write its parts. Runs after transforms propagate.
    pub(crate) fn realize_poses(&mut self) {
        let pending: Vec<RenderHandle> = self
            .tables
            .animated
            .iter()
            .filter(|(_, instance)| instance.pose_pending)
            .map(|(handle, _)| *handle)
            .collect();
        for handle in pending {
            self.realize_pose(handle);
        }
    }

    fn realize_pose(&mut self, handle: RenderHandle) {
        if let Some(instance) = self.tables.animated.get_mut(&handle) {
            instance.pose_pending = false;
        }
        if self.skin_animated_instance(handle) {
            // The cuts changed the skinned triangles, so the parts' index
            // counts. The parts are the same primitives in the same order,
            // so each keeps its predecessor's previous model.
            let parts = |renderer: &Self| {
                renderer
                    .tables
                    .nodes
                    .get(&handle)
                    .map_or(Vec::new(), |node| node.parts.clone())
            };
            let before = parts(self);
            self.rebuild_parts(handle);
            self.temporal.carry(&before, &parts(self));
        }
        self.write_animated_parts(handle);
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
            if self
                .tables
                .animated
                .get(&handle)
                .is_some_and(|instance| instance.pose_pending)
            {
                self.realize_pose(handle);
            }
            self.report_animated_bounds(handle);
        }
    }

    /// CPU-skin every skinned primitive into the instance's vertex buffers,
    /// with its severed pieces' triangles skinned where each piece is.
    /// Returns whether the cuts changed which triangles each draws: the
    /// buffers are then made again and the parts must be.
    fn skin_animated_instance(&mut self, handle: RenderHandle) -> bool {
        // While frames draw motion, vertices a frame has drawn stay as they
        // were for the next frame's motion, and the new pose goes into the
        // other buffer.
        let keep = self.temporal.keeps_history();
        let frames = self.temporal.motion_frames();
        let Some(instance) = self.tables.animated.get_mut(&handle) else {
            return false;
        };
        let drawn = keep && instance.skinned_at < frames;
        let Some(asset) = self.tables.animated_assets.get(instance.slot) else {
            return false;
        };
        let joints: Vec<u32> = instance
            .pose
            .iter()
            .flat_map(|pose| &pose.pieces)
            .map(|piece| piece.joint)
            .collect();
        let resplit = instance
            .split
            .as_ref()
            .map_or(!joints.is_empty(), |split| split.joints != joints);
        // The vertices as the last frame with motion drew them, by the
        // vertex and cut joint each drew, for the meshes the new cuts make.
        let mut drawn_before: HashMap<(u32, u32), DrawnVertices> = HashMap::new();
        if resplit {
            if keep {
                for (key, mesh) in &instance.skinned {
                    let positions = if drawn {
                        Some(mesh.cpu.clone())
                    } else {
                        mesh.skinned
                            .as_ref()
                            .and_then(|history| history.previous_positions.clone())
                    };
                    let Some(positions) = positions else { continue };
                    let copies = instance
                        .split
                        .as_ref()
                        .and_then(|split| Some((split, split.primitives.get(key)?)))
                        .map(|(split, primitive)| {
                            let own = positions.positions.len() - primitive.copies.len();
                            primitive
                                .copies
                                .iter()
                                .enumerate()
                                .map(|(index, (vertex, cut))| {
                                    ((*vertex, split.joints[*cut]), own + index)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    drawn_before.insert(*key, DrawnVertices { positions, copies });
                }
            }
            instance.split =
                (!joints.is_empty()).then(|| CutSplit::new(&asset.model, &asset.rig, joints));
            instance.skinned.clear();
        }
        let instance = &self.tables.animated[&handle];
        let pieces = instance
            .pose
            .as_ref()
            .map_or(&[][..], |pose| pose.pieces.as_slice());
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
                .map(|(joint, inverse)| node_matrix(instance, asset, *joint) * *inverse)
                .collect();
            // Each piece's palette: every joint in the piece's own frozen
            // pose where the piece is, so a vertex weighted partly to a
            // joint another cut owns stays rigid with this piece.
            let pieces: Vec<Vec<Mat4>> = pieces
                .iter()
                .map(|piece| {
                    skin.joints
                        .iter()
                        .zip(&skin.inverse_binds)
                        .map(|(joint, inverse)| {
                            piece
                                .nodes
                                .get(*joint)
                                .map_or(Mat4::IDENTITY, Mat4::from_cols_array)
                                * *inverse
                        })
                        .collect()
                })
                .collect();
            for (primitive_index, primitive) in asset.model.meshes[mesh].iter().enumerate() {
                let (Some(joints), Some(weights)) = (&primitive.joints, &primitive.weights) else {
                    continue;
                };
                if instance
                    .refused
                    .contains(&(node_index as u32, primitive_index as u32))
                {
                    continue;
                }
                let split = instance.split.as_ref().and_then(|split| {
                    split
                        .primitives
                        .get(&(node_index as u32, primitive_index as u32))
                });
                let copies = split.map_or(&[][..], |split| split.copies.as_slice());
                let own = primitive.positions.len();
                let count = own + copies.len();
                let mut positions = Vec::with_capacity(count);
                let mut normals = Vec::with_capacity(count);
                let mut tangents =
                    Vec::with_capacity(primitive.tangents.as_ref().map_or(0, |_| count));
                for output in 0..count {
                    let (vertex, palette) = match output.checked_sub(own) {
                        None => (output, &palette),
                        Some(copy) => {
                            let (vertex, cut) = copies[copy];
                            (vertex as usize, &pieces[cut])
                        }
                    };
                    let position = &primitive.positions[vertex];
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
                let source = |output: usize| match output.checked_sub(own) {
                    None => output,
                    Some(copy) => copies[copy].0 as usize,
                };
                let vertices = interleave(primitive, &positions, &normals, &source);
                let extra = extra_stream(
                    &asset.model,
                    primitive,
                    count,
                    &source,
                    (!tangents.is_empty()).then_some(tangents.as_slice()),
                );
                updates.push(((node_index as u32, primitive_index as u32), vertices, extra));
            }
        }
        let mut kept = drawn;
        for (key, vertices, extra) in updates {
            let gpu = &self.gpu;
            let instance = self
                .tables
                .animated
                .get_mut(&handle)
                .expect("the instance skinned above");
            if let Some(mesh) = instance.skinned.get_mut(&key) {
                let history = mesh.skinned.get_or_insert_default();
                if !keep {
                    *history = SkinnedHistory::default();
                } else if drawn {
                    let next = history
                        .previous
                        .take()
                        .or_else(|| history.spare.take())
                        .map_or_else(
                            || {
                                crate::gpu::allocating(&gpu.device, || {
                                    gpu.device.create_buffer(&wgpu::BufferDescriptor {
                                        label: Some("render-wgpu skinned vertices"),
                                        size: mesh.vertices.size(),
                                        usage: wgpu::BufferUsages::VERTEX
                                            | wgpu::BufferUsages::COPY_DST,
                                        mapped_at_creation: false,
                                    })
                                })
                                .ok()
                            },
                            Some,
                        );
                    match next {
                        Some(next) => {
                            history.previous = Some(std::mem::replace(&mut mesh.vertices, next));
                            history.previous_positions = Some(mesh.cpu.clone());
                        }
                        // No room for a second buffer: the mesh draws
                        // without motion history, as on a frame without
                        // motion.
                        None => *history = SkinnedHistory::default(),
                    }
                }
                gpu.queue
                    .write_buffer(&mesh.vertices, 0, bytemuck::cast_slice(&vertices));
                if let (Some(buffer), Some(extra)) = (&mesh.extra, &extra) {
                    gpu.queue
                        .write_buffer(buffer, 0, bytemuck::cast_slice(extra));
                }
                mesh.bounds = vertex_bounds(&vertices);
                mesh.cpu = std::sync::Arc::new(crate::tables::CpuGeometry {
                    positions: vertex_positions(&vertices),
                    indices: mesh.cpu.indices.clone(),
                });
            } else {
                let slot = instance.slot;
                let asset = self
                    .tables
                    .animated_assets
                    .get(slot)
                    .expect("a skinned instance's asset is defined");
                let node = &asset.model.nodes[key.0 as usize];
                let primitive = &asset.model.meshes[node.mesh.expect("skinned node has a mesh")]
                    [key.1 as usize];
                let indices = self.tables.animated[&handle]
                    .split
                    .as_ref()
                    .and_then(|split| split.primitives.get(&key))
                    .map_or_else(|| primitive.indices.clone(), |split| split.indices.clone());
                let label = format!("animated {} skin {}/{}", handle.raw(), key.0, key.1);
                let uploaded = self
                    .upload_dynamic_vertices(&label, &vertices, &indices)
                    .and_then(|mut mesh| {
                        mesh.extra = extra
                            .map(|extra| self.vertex_buffer(&label, &extra, true))
                            .transpose()?;
                        Ok(mesh)
                    });
                let mut mesh = match uploaded {
                    Ok(mesh) => mesh,
                    Err(refusal) => {
                        if let Some(instance) = self.tables.animated.get_mut(&handle) {
                            instance.refused.insert(key);
                            instance.refusal = Some(refusal);
                        }
                        continue;
                    }
                };
                // Made for new cuts: each vertex's previous position is where
                // the last frame drew it, a copy's where its source vertex
                // was, in its cut's piece or in the body.
                if let Some(before) = drawn_before.get(&key) {
                    let instance = &self.tables.animated[&handle];
                    let own = primitive.positions.len();
                    let copies = instance
                        .split
                        .as_ref()
                        .and_then(|split| Some((split, split.primitives.get(&key)?)))
                        .map_or(Vec::new(), |(split, primitive)| {
                            primitive
                                .copies
                                .iter()
                                .map(|(vertex, cut)| (*vertex, split.joints[*cut]))
                                .collect()
                        });
                    let current = vertex_positions(&vertices);
                    let positions: Vec<Vec3> = (0..current.len())
                        .map(|output| {
                            let index = match output.checked_sub(own) {
                                None => output,
                                Some(copy) => {
                                    let (vertex, joint) = copies[copy];
                                    before
                                        .copies
                                        .get(&(vertex, joint))
                                        .copied()
                                        .unwrap_or(vertex as usize)
                                }
                            };
                            before
                                .positions
                                .positions
                                .get(index)
                                .copied()
                                .unwrap_or(current[output])
                        })
                        .collect();
                    let mut previous = vec![0.0; positions.len() * VERTEX_FLOATS];
                    for (vertex, position) in previous
                        .as_chunks_mut::<VERTEX_FLOATS>()
                        .0
                        .iter_mut()
                        .zip(&positions)
                    {
                        vertex[..3].copy_from_slice(&position.to_array());
                    }
                    // Without room for it, the mesh draws without motion
                    // history.
                    if let Ok(buffer) = self.vertex_buffer(&label, &previous, true) {
                        let history = mesh.skinned.get_or_insert_default();
                        history.previous = Some(buffer);
                        history.previous_positions =
                            Some(std::sync::Arc::new(crate::tables::CpuGeometry {
                                positions,
                                indices: Vec::new(),
                            }));
                        kept = true;
                    }
                }
                if let Some(instance) = self.tables.animated.get_mut(&handle) {
                    instance.skinned.insert(key, mesh);
                }
            }
        }
        if let Some(instance) = self.tables.animated.get_mut(&handle) {
            instance.skinned_at = frames;
        }
        if kept && !self.skinned_settling.contains(&handle) {
            self.skinned_settling.push(handle);
        }
        resplit
    }

    /// At the start of a frame with motion: skinned vertices a frame has
    /// drawn since their pose changed are their own previous vertices again,
    /// and the buffer that held the previous ones is kept for the next pose.
    pub(crate) fn settle_skinned_history(&mut self) {
        let frames = self.temporal.motion_frames();
        let animated = &mut self.tables.animated;
        self.skinned_settling.retain(|handle| {
            let Some(instance) = animated.get_mut(handle) else {
                return false;
            };
            if instance.skinned_at == frames {
                // Written for this frame: it draws its pose's motion.
                return true;
            }
            for history in instance
                .skinned
                .values_mut()
                .filter_map(|mesh| mesh.skinned.as_mut())
            {
                if let Some(previous) = history.previous.take() {
                    history.spare = Some(previous);
                }
                history.previous_positions = None;
            }
            false
        });
    }

    /// Frames no longer draw motion: no skinned vertices are kept.
    pub(crate) fn forget_skinned_history(&mut self) {
        self.skinned_settling.clear();
        for history in self
            .tables
            .animated
            .values_mut()
            .flat_map(|instance| instance.skinned.values_mut())
            .filter_map(|mesh| mesh.skinned.as_mut())
        {
            *history = SkinnedHistory::default();
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
            let local = match asset {
                Some(asset) if !skinned => placed_node(instance, asset, *glb_node),
                _ => Mat4::IDENTITY,
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
                let matrix = world * node_matrix(instance, asset, node_index);
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
    /// joint-attached child hangs from.
    pub(crate) fn joint_pose_at(&self, handle: RenderHandle, node: usize) -> Option<Mat4> {
        let instance = self.tables.animated.get(&handle)?;
        let asset = self.tables.animated_assets.get(instance.slot)?;
        (node < asset.rest.len()).then(|| placed_node(instance, asset, node))
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
        let at = |node: usize| node_matrix(instance, asset, node).w_axis.truncate();
        asset
            .rig
            .iter()
            .flatten()
            .filter_map(|&node| {
                let mut parent = asset.model.nodes[node].parent;
                while let Some(above) = parent.filter(|above| !in_rig.contains(above)) {
                    parent = asset.model.nodes[above].parent;
                }
                Some((at(node), at(parent?)))
            })
            .collect()
    }

    /// An animated instance's IK chains as its pose was evaluated, in its
    /// space: each target, the drawn end joint, and whether the chain
    /// straightened.
    pub(crate) fn ik_reaches(&self, handle: RenderHandle) -> Vec<(Vec3, Option<Vec3>, bool)> {
        let Some(instance) = self.tables.animated.get(&handle) else {
            return Vec::new();
        };
        let Some(asset) = self.tables.animated_assets.get(instance.slot) else {
            return Vec::new();
        };
        instance
            .pose
            .iter()
            .flat_map(|pose| &pose.ik)
            .map(|chain| {
                let end = chain
                    .end
                    .map(|node| node as usize)
                    .filter(|node| *node < asset.rest.len())
                    .map(|node| node_matrix(instance, asset, node).w_axis.truncate());
                (Vec3::from_array(chain.target), end, chain.straightened)
            })
            .collect()
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
                let index_count = match &mesh_ref {
                    crate::tables::MeshRef::AnimatedSkinned(_, node, primitive) => instance
                        .skinned
                        .get(&(*node, *primitive))
                        .map_or(0, |mesh| mesh.cpu.indices.len()),
                    _ => primitive.indices.len(),
                };
                parts.push((mesh_ref, index_count as u32, material, slot));
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

    /// A vertex buffer holding `data`, rewritable when `dynamic`; refused
    /// when the device has no room for it.
    fn vertex_buffer(
        &self,
        label: &str,
        data: &[f32],
        dynamic: bool,
    ) -> Result<wgpu::Buffer, String> {
        let usage = if dynamic {
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::VERTEX
        };
        let device = &self.gpu.device;
        crate::gpu::allocating(device, || {
            crate::gpu::buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytemuck::cast_slice(data),
                    usage,
                },
            )
        })
        .map_err(|error| format!("{label} could not be uploaded: {error}"))
    }

    /// Upload a skinned primitive's vertices, which the CPU rewrites;
    /// refused when the device has no room for them.
    fn upload_dynamic_vertices(
        &self,
        label: &str,
        vertices: &[f32],
        indices: &[u32],
    ) -> Result<GpuMesh, String> {
        let device = &self.gpu.device;
        let (vertex_buffer, index_buffer) = crate::gpu::allocating(device, || {
            (
                crate::gpu::buffer_init(
                    device,
                    &wgpu::util::BufferInitDescriptor {
                        label: Some(label),
                        contents: bytemuck::cast_slice(vertices),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    },
                ),
                crate::gpu::buffer_init(
                    device,
                    &wgpu::util::BufferInitDescriptor {
                        label: Some(label),
                        contents: bytemuck::cast_slice(indices),
                        usage: wgpu::BufferUsages::INDEX,
                    },
                ),
            )
        })
        .map_err(|error| format!("{label} could not be uploaded: {error}"))?;
        Ok(GpuMesh {
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
            skinned: Some(SkinnedHistory::default()),
            vertices: vertex_buffer,
            indices: index_buffer,
            topology: Topology::Triangles,
            groups: vec![(0, 0, indices.len() as u32)],
            slots: BTreeMap::new(),
        })
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

/// A node of an instance's mesh in the pose it draws, in the instance's
/// space: its evaluated pose, or the mesh at rest while no pose fits it.
fn node_matrix(instance: &AnimatedInstance, asset: &AnimatedAssetRow, node: usize) -> Mat4 {
    match instance
        .pose
        .as_ref()
        .filter(|pose| pose.nodes.len() == asset.rest.len())
    {
        Some(pose) => Mat4::from_cols_array(&pose.nodes[node]),
        None => asset.rest[node],
    }
}

/// A node as drawn: placed by the severed piece that owns it, if any.
fn placed_node(instance: &AnimatedInstance, asset: &AnimatedAssetRow, node: usize) -> Mat4 {
    let piece = instance
        .owners
        .get(node)
        .copied()
        .flatten()
        .zip(instance.pose.as_ref())
        .and_then(|(cut, pose)| pose.pieces.get(cut)?.nodes.get(node));
    match piece {
        Some(placed) => Mat4::from_cols_array(placed),
        None => node_matrix(instance, asset, node),
    }
}

/// The second vertex stream a primitive draws with, when its material reads
/// tangents (a normal map) or `TEXCOORD_1`: tangent (xyz, handedness) and
/// uv1 per vertex. Without TANGENT a normal-mapped primitive has MikkTSpace
/// tangents (`glb::decode`); a vertex without one gets +X.
fn extra_stream(
    model: &GlbModel,
    primitive: &glb::GlbPrimitive,
    count: usize,
    source: &dyn Fn(usize) -> usize,
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
    let mut extra = Vec::with_capacity(count * EXTRA_VERTEX_FLOATS);
    for output in 0..count {
        let tangent = tangents
            .and_then(|tangents| tangents.get(output))
            .copied()
            .unwrap_or([1.0, 0.0, 0.0, 1.0]);
        extra.extend_from_slice(&tangent);
        let uv = primitive
            .uvs1
            .as_ref()
            .and_then(|uvs| uvs.get(source(output)))
            .copied()
            .unwrap_or([0.0, 0.0]);
        extra.extend_from_slice(&uv);
    }
    Some(extra)
}

/// Interleave a primitive's streams into the renderer's vertex layout; each
/// output vertex takes the other streams of its `source` vertex.
fn interleave(
    primitive: &glb::GlbPrimitive,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    source: &dyn Fn(usize) -> usize,
) -> Vec<f32> {
    let mut vertices = Vec::with_capacity(positions.len() * VERTEX_FLOATS);
    for (output, position) in positions.iter().enumerate() {
        let vertex = source(output);
        vertices.extend_from_slice(position);
        vertices.extend_from_slice(&normals[output]);
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
