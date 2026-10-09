//! Typed, handle-keyed GPU tables. This is the layout family modules extend.
//!
//! | Table | Key | Row | Filled by |
//! |---|---|---|---|
//! | `textures` | texture id | GPU texture view, sampler and pixel size | `DefineTexture` / `ReleaseTexture` |
//! | `materials` | material id | descriptor and bind group | `DefineMaterial` / `ReleaseMaterial` |
//! | `static_meshes` | static mesh asset id | vertex/index buffers, groups, default slots, local bounds | `DefineStaticMesh` / `ReleaseStaticMesh` |
//! | `payload_meshes` | node handle | vertex/index buffers of a primitive's replaced payload or line | `ReplaceMeshPayload`, `Create` (line) |
//! | `nodes` | `RenderHandle` | parent, children, local and world transform, visibility, layer, kind, owned parts | `Create*`, `Update`, `Destroy`, `SetParentJoint`, `UpdateLight`, `SetMaterialInstanceParameters` |
//! | `parts` | `PartId` (dense) | one drawable (node, mesh group, material), its GPU `PartRow`, and its draw state (class, batch key, bounds, visibility, layer) | derived from nodes |
//! | `atlases` | atlas name id (`names`) | retained descriptor and its texture's name id | `DefineSpriteAtlas` / `ReleaseSpriteAtlas` |
//! | `names` | asset or texture id | dense name id, assigned at apply | any op naming an atlas or sprite texture |
//! | `voxel_objects` | voxel object asset id | uploaded meshes, frame-to-mesh table, slot materials | `DefineVoxelObject` / `ReleaseVoxelObject` |
//! | `animated_assets` | animated mesh asset id | decoded GLB (nodes, skins, clips), uploaded unskinned primitives, GLB materials and textures | `DefineAnimatedMesh` / `ReleaseAnimatedMesh` |
//! | `animated` | `RenderHandle` | playback, pose, skinned vertex buffers | `CreateAnimatedMeshInstance`, `SetAnimatedMeshPlayback`, `SetAnimatedMeshInspection` |
//! | `environment` | (single) | background colour or equirectangular sky (with blend) | `SetBackgroundColor` / `SetSkyBackground` |
//! | `fog`, `tone_mapping` | (single) | distance fog; exposure and tone mapping operator | `SetFog` / `SetToneMapping` |
//! | `bloom`, `auto_exposure` | (single) | the finish pass's bloom and auto exposure | `SetBloom` / `SetAutoExposure` |
//! | `color_grading` | (single) | white balance, contrast and saturation before the tone mapping operator | `SetColorGrading` |
//! | `atmosphere` | (single) | height fog, sun haze and the sun in the sky | `SetAtmosphere` |
//! | `sun_shafts` | (single) | the finish pass's sun shafts | `SetSunShafts` |
//! | `wind` | (single) | the scene's wind, in the frame uniform | `SetWind` |
//! | `clouds` | (single) | the sky's cloud layer and its shade on the ground | `SetClouds` |
//! | `wetness` | (single) | how wet the scene's surfaces are, in the frame uniform | `SetWetness` |
//! | `backdrop` | (single) | the backdrop layer's link to the world's cameras | `SetBackdrop` |
//! | `precipitation` | (single) | rain or snow around the camera, drawn after the world | `SetPrecipitation` |
//! | `image_effect` | (single) | a product image effect over each primary view's picture | `SetImageEffect` |
//! | `sky_light` | (single) | the sky's light: the background as an environment (`sky_light.rs`) | `SetSkyLight` |
//!
//! | `sprites` | `RenderHandle` | membership: the nodes of kind `Sprite` | `CreateSprite`, `Destroy` (subtree) |
//!
//! Animated meshes, voxel objects and sprites are nodes (they take part in the
//! hierarchy, transforms and visibility) whose kind a family module realizes;
//! their descriptors wait in the node row until then. Voxel objects are
//! realized (`voxel.rs`): a node draws its current frame's mesh.
//!
//! Draw lists (`batch.rs`) read the dense part state only: parts sharing a
//! batch key draw as one instanced run.
//!
//! Dirty marks are the only change tracking: a transform or visibility change
//! marks its node; `prepare` recomputes that subtree's world rows and uploads
//! only those `PartRow`s. Nothing is re-derived for unchanged nodes.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use glam::{Mat4, Vec3};
use render_model::{
    AnimatedMeshInstanceDescriptor, FogDescriptor, Geometry, LightDescriptor, Material,
    MaterialInstanceParameters, RenderHandle, RenderLayer, RenderMaterialDescriptor, ShadowCasting,
    SkyBackgroundDescriptor, SpriteAtlasDescriptor, SpriteInstanceDescriptor,
    ToneMappingDescriptor, VoxelObjectInstanceDescriptor,
};

use crate::animated::{AnimatedAssetRow, AnimatedInstance, ControllerRow};
use crate::voxel::VoxelObjectRow;

pub(crate) struct GpuTexture {
    /// The base level, for sky, sprites, particles and any other sampling
    /// whose coordinates jump (a panorama's seam, atlas frames).
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    /// Pixel size; voxel atlas regions are resolved against it.
    pub size: (u32, u32),
    /// Every mip level with a trilinear sampler, for material sampling; None
    /// for nearest-filtered and single-level textures.
    pub mipped: Option<(wgpu::TextureView, wgpu::Sampler)>,
}

impl GpuTexture {
    /// What a material samples: the mip chain when there is one.
    pub fn material_binding(&self) -> (&wgpu::TextureView, &wgpu::Sampler) {
        match &self.mipped {
            Some((view, sampler)) => (view, sampler),
            None => (&self.view, &self.sampler),
        }
    }
}

pub(crate) struct MaterialRow {
    pub descriptor: RenderMaterialDescriptor,
    pub bind_group: wgpu::BindGroup,
    /// The standard shader features its pipelines compile.
    pub features: crate::shaders::Features,
    /// A GLB material's maps, kept for variants (matte inspection).
    pub maps: crate::apply::MaterialMaps,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Topology {
    Triangles,
    Lines,
}

/// A payload mesh's coarse distance field: its box in mesh space, its bytes,
/// and the atlas brick it holds while the renderer traces fields
/// (`AmbientOcclusionPath::DistanceField`). The bytes stay so the setting
/// can turn on later without a republish.
pub(crate) struct MeshField {
    pub field_box: crate::distance_fields::FieldBox,
    pub data: Vec<u8>,
    pub slot: Option<u32>,
}

pub(crate) struct GpuMesh {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub topology: Topology,
    /// (material slot, first index, index count)
    pub groups: Vec<(u16, u32, u32)>,
    /// Static mesh default slot bindings; empty for payload meshes.
    pub slots: BTreeMap<u16, String>,
    /// Local bounds of every vertex, for culling.
    pub bounds: Aabb,
    /// Positions and indices kept for bounds and wireframe edges.
    pub cpu: std::sync::Arc<CpuGeometry>,
    /// Line-list indices for wireframe parts, built on first use: every
    /// triangle's three edges.
    pub edges: std::sync::OnceLock<wgpu::Buffer>,
    /// A second vertex stream, tangent (xyz, handedness w) and `TEXCOORD_1`
    /// per vertex: only GLB primitives whose material reads them.
    pub extra: Option<wgpu::Buffer>,
    /// The payload's texture space, for triplanar materials.
    pub texture_space: Option<render_model::MeshTextureSpace>,
    /// The payload's distance field: its atlas brick and its box in mesh
    /// space (`distance_fields.rs`).
    pub distance_field: Option<MeshField>,
    /// Its vertex colours are terrain layer weights, not a tint: a voxel
    /// chunk meshed with terrain layers.
    pub layer_weights: bool,
    /// The terrain layers its weights stand for, when its session has more
    /// than four: its groups draw their materials narrowed to them.
    pub layer_palette: Vec<u8>,
    /// Its vertex colours' alpha is ambient occlusion: a voxel chunk meshed
    /// with vertex occlusion.
    pub vertex_occlusion: bool,
}

impl GpuMesh {
    /// The standard shader features the mesh's vertex streams need.
    pub fn features(&self) -> crate::shaders::Features {
        crate::shaders::Features::default()
            .with(
                crate::shaders::Features::VERTEX_TANGENTS,
                self.extra.is_some(),
            )
            .with(crate::shaders::Features::LAYER_WEIGHTS, self.layer_weights)
            .with(
                crate::shaders::Features::VERTEX_OCCLUSION,
                self.vertex_occlusion,
            )
    }
}

#[derive(Default)]
pub(crate) struct CpuGeometry {
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
}

/// An axis-aligned box. `EMPTY` (min > max) contains nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const EMPTY: Self = Self {
        min: Vec3::splat(f32::MAX),
        max: Vec3::splat(f32::MIN),
    };

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }

    pub fn include(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    /// The box holding both (an empty one adds nothing).
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// The world box of this local box under `world`.
    pub fn transformed(&self, world: &Mat4) -> Self {
        if self.is_empty() {
            return *self;
        }
        let center = world.transform_point3((self.min + self.max) * 0.5);
        let half = (self.max - self.min) * 0.5;
        let abs = glam::Mat3::from_cols(
            world.x_axis.truncate().abs(),
            world.y_axis.truncate().abs(),
            world.z_axis.truncate().abs(),
        );
        let half = abs * half;
        Self {
            min: center - half,
            max: center + half,
        }
    }
}

/// Which mesh a part draws. Asset names are resolved to their `names` ids
/// when the part is built, so drawing indexes tables and hashes no string.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum MeshRef {
    Static(u32),
    Payload(RenderHandle),
    Builtin(Builtin),
    /// A voxel object asset's mesh (by index).
    Voxel(u32, u32),
    /// An animated asset's unskinned primitive: (asset, mesh, primitive).
    AnimatedRigid(u32, u32, u32),
    /// An instance's CPU-skinned primitive: (instance, GLB node, primitive).
    AnimatedSkinned(RenderHandle, u32, u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Builtin {
    Cube,
    Sphere,
    Quad,
    Point,
}

/// Which material bind group a part uses (a retained material by its
/// `names` id).
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum MaterialRef {
    Retained(u32),
    /// Primitive nodes: flat colour, no lighting (a basic material).
    Unlit,
    /// Payload groups with no `voxel-material/<slot>` descriptor: lit, untextured.
    LitFallback,
}

pub(crate) enum NodeKind {
    Group,
    Primitive {
        geometry: Geometry,
        material: Material,
        has_payload: bool,
    },
    StaticMesh {
        asset: String,
        overrides: BTreeMap<u16, String>,
        parameters: BTreeMap<u16, MaterialInstanceParameters>,
    },
    Light(LightDescriptor),
    AnimatedMesh(Box<AnimatedMeshInstanceDescriptor>),
    VoxelObject(Box<VoxelObjectInstanceDescriptor>),
    Sprite(Box<SpriteRow>),
    /// Scattered copies of a static mesh (`render_model::scatter`).
    Scatter(Box<render_model::ScatterPatchDescriptor>),
}

/// One node's GPU-side row: a derived twin of `PresentationWorld`'s node,
/// rebuilt from deltas, never an authority. It holds only what encoding a
/// view needs: hierarchy links (parent, joint parent, children),
/// local and propagated world matrices, visibility and layer, the realized
/// kind, and the node's parts. Gameplay-facing facts, entity or product
/// identities and anything read back by another crate do not belong here and
/// are refused at review.
/// World matrices are propagated here each frame (dirty subtrees only)
/// because joint attachments follow poses only this crate evaluates; other
/// consumers ask `PresentationWorld` for positions instead (#8848).
/// Dense ids for atlas and texture names, assigned at apply time so frame
/// paths index and compare integers instead of hashing strings. An id stays
/// bound to its name; the table grows with the distinct names seen.
#[derive(Default)]
pub(crate) struct Names {
    ids: HashMap<String, u32>,
    names: Vec<String>,
}

impl Names {
    pub fn id(&mut self, name: &str) -> u32 {
        if let Some(id) = self.ids.get(name) {
            return *id;
        }
        let id = self.names.len() as u32;
        self.ids.insert(name.to_owned(), id);
        self.names.push(name.to_owned());
        id
    }

    pub fn get(&self, name: &str) -> Option<u32> {
        self.ids.get(name).copied()
    }

    pub fn name(&self, id: u32) -> &str {
        &self.names[id as usize]
    }
}

/// Rows by `Names` id. A name keeps its slot across redefinition and a
/// release empties it, so frame paths index instead of hashing names.
pub(crate) struct Slots<T> {
    rows: Vec<Option<T>>,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Self { rows: Vec::new() }
    }
}

impl<T> Slots<T> {
    pub fn get(&self, id: u32) -> Option<&T> {
        self.rows.get(id as usize)?.as_ref()
    }

    /// Insert or replace the row, returning the one it replaced.
    pub fn insert(&mut self, id: u32, row: T) -> Option<T> {
        let index = id as usize;
        if self.rows.len() <= index {
            self.rows.resize_with(index + 1, || None);
        }
        self.rows[index].replace(row)
    }

    pub fn remove(&mut self, id: u32) -> Option<T> {
        self.rows.get_mut(id as usize)?.take()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.rows.iter().flatten()
    }

    pub fn len(&self) -> usize {
        self.values().count()
    }
}

/// A row by name, with its id: apply-time lookups. A free function, so
/// callers holding another table mutably keep the borrows disjoint.
pub(crate) fn named<'a, T>(names: &Names, rows: &'a Slots<T>, name: &str) -> Option<(u32, &'a T)> {
    let id = names.get(name)?;
    Some((id, rows.get(id)?))
}

pub(crate) struct AtlasRow {
    pub descriptor: SpriteAtlasDescriptor,
    /// Name id of the atlas texture.
    pub texture: u32,
}

/// A sprite node's descriptor with its names resolved at apply.
pub(crate) struct SpriteRow {
    pub descriptor: SpriteInstanceDescriptor,
    /// Name id of the sprite's atlas.
    pub atlas: u32,
    /// Name id of an authored normal or depth texture, by lighting mode.
    pub detail: Option<u32>,
}

pub(crate) struct NodeRow {
    pub parent: Option<RenderHandle>,
    pub parent_joint: Option<String>,
    /// `parent_joint` resolved to its node in the parent's animated asset,
    /// when the joint is set or the parent's asset changes; the frame reads
    /// the parent's pose by this index.
    pub parent_joint_node: Option<usize>,
    pub children: Vec<RenderHandle>,
    pub local: Mat4,
    pub world: Mat4,
    pub visible: bool,
    pub world_visible: bool,
    pub layer: RenderLayer,
    /// The root ancestor's layer: children draw in their root's scene.
    pub world_layer: RenderLayer,
    /// Whether the node's own parts cast shadows; not inherited.
    pub shadow_casting: ShadowCasting,
    pub kind: NodeKind,
    pub parts: Vec<PartId>,
}

pub(crate) type PartId = u32;

pub(crate) struct Part {
    pub node: RenderHandle,
    pub mesh: MeshRef,
    /// Triangle index range; a wireframe part draws its edges instead.
    pub first_index: u32,
    pub index_count: u32,
    pub material: MaterialRef,
    /// Draw the range's triangle edges as lines (`Material.wireframe`,
    /// animated mesh inspection).
    pub wireframe: bool,
}

/// How a part draws, fixed when its node's parts are derived.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PartClass {
    pub blend: bool,
    pub double_sided: bool,
    pub lines: bool,
    /// Whether the material casts: an opaque one, or a blended one that
    /// asks to (`translucent_shadow`).
    pub shadow: bool,
    /// The material's features: opaque draws group by them, so parts sharing
    /// a pipeline draw together.
    pub features: crate::shaders::Features,
}

/// Dense per-part draw state: what the draw list reads without touching the
/// node table. Refreshed by `Parts::write` whenever the part's node is.
#[derive(Clone, Copy)]
pub(crate) struct PartState {
    pub class: PartClass,
    /// Parts with equal keys (mesh, index range, material) batch together.
    pub key: u32,
    pub local_bounds: Aabb,
    pub world_bounds: Aabb,
    /// Negative world determinant: faces wind the other way.
    pub mirrored: bool,
    /// A transform under the node's world (an animated GLB node's pose).
    pub local: Option<Mat4>,
    /// Effectively visible.
    pub shown: bool,
    /// The root's layer.
    pub layer: RenderLayer,
    /// The part casts into shadow layers (`ShadowCasting::Cast`).
    pub casts_shadows: bool,
    /// A scattered copy (`NodeKind::Scatter`): the first copy of its mesh
    /// group, which leads the run the CPU lists cull and draw as one.
    pub leader: Option<PartId>,
}

/// GPU row per part: model matrix, normal matrix (3 columns), linear colour
/// (rgb, alpha), emission (rgb pre-multiplied by intensity) and the distance
/// fade (start, end).
pub(crate) const PART_ROW_FLOATS: usize = 16 + 12 + 4 + 4 + 4;

#[derive(Clone, Copy)]
pub(crate) struct PartRow {
    pub color: [f32; 4],
    pub emission: [f32; 3],
    /// Texture-space origin (xyz) and cells per unit (w): where triplanar
    /// materials project the part's positions from. Set from its mesh.
    pub texture_space: [f32; 4],
    /// The part shrinks into its origin between these distances from the
    /// camera (`ScatterFade`); an end of 0 keeps it whole.
    pub fade: [f32; 2],
}

impl PartRow {
    pub fn new(color: [f32; 4], emission: [f32; 3]) -> Self {
        Self {
            color,
            emission,
            texture_space: OBJECT_TEXTURE_SPACE,
            fade: [0.0; 2],
        }
    }
}

/// A run of scattered copies drawn as one: its parts and their world bounds.
pub(crate) struct Run {
    pub ids: Vec<PartId>,
    pub bounds: Aabb,
}

/// Object-space positions, for meshes without a texture space.
pub(crate) const OBJECT_TEXTURE_SPACE: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// What makes two parts one instanced draw.
#[derive(Clone, PartialEq, Eq, Hash)]
struct BatchKey {
    mesh: MeshRef,
    first_index: u32,
    index_count: u32,
    material: MaterialRef,
    wireframe: bool,
}

#[derive(Default)]
pub(crate) struct Parts {
    pub meta: Vec<Option<Part>>,
    pub rows: Vec<PartRow>,
    pub state: Vec<PartState>,
    /// CPU copy of the dense GPU rows, uploaded where dirty.
    pub gpu: Vec<f32>,
    pub free: Vec<PartId>,
    pub dirty: HashSet<PartId>,
    /// Rows past the GPU buffer's capacity force a buffer reallocation.
    pub grown: bool,
    /// A part was added or removed, or changed visibility, layer or winding,
    /// since the draw lists were built: batches may regroup.
    pub regrouped: bool,
    /// Parts whose world transform was rewritten: culling and blend order
    /// may change, the batches do not.
    pub moved: HashSet<PartId>,
    /// The former world bounds of parts removed or moved since the last
    /// frame, for the indirect light volume's dirty bricks (`probes`): a
    /// part that left a brick is a change there too.
    pub former_bounds: Vec<Aabb>,
    /// Batch keys in use: key, reference count.
    keys: HashMap<BatchKey, (u32, u32)>,
    free_keys: Vec<u32>,
    next_key: u32,
    /// Scattered runs by their leader.
    pub runs: HashMap<PartId, Run>,
    /// Runs whose copies were written since their bounds were.
    dirty_runs: HashSet<PartId>,
}

impl Parts {
    pub fn insert(&mut self, part: Part, row: PartRow, bounds: Aabb, class: PartClass) -> PartId {
        let key = self.acquire_key(&part);
        let state = PartState {
            class,
            key,
            local_bounds: bounds,
            world_bounds: Aabb::EMPTY,
            mirrored: false,
            local: None,
            shown: false,
            layer: RenderLayer::Scene,
            casts_shadows: true,
            leader: None,
        };
        let id = if let Some(id) = self.free.pop() {
            self.meta[id as usize] = Some(part);
            self.rows[id as usize] = row;
            self.state[id as usize] = state;
            id
        } else {
            self.meta.push(Some(part));
            self.rows.push(row);
            self.state.push(state);
            self.gpu.resize(self.meta.len() * PART_ROW_FLOATS, 0.0);
            self.grown = true;
            (self.meta.len() - 1) as PartId
        };
        self.dirty.insert(id);
        self.regrouped = true;
        id
    }

    pub fn remove(&mut self, id: PartId) {
        if let Some(part) = self.meta[id as usize].take() {
            self.release_key(part);
            if self.state[id as usize].leader.is_none() {
                self.former_bounds
                    .push(self.state[id as usize].world_bounds);
            }
        }
        if self.state[id as usize].leader.take() == Some(id) {
            self.runs.remove(&id);
            self.dirty_runs.remove(&id);
        }
        self.dirty.remove(&id);
        self.free.push(id);
        self.regrouped = true;
    }

    /// Write a part's world state into its GPU row and mark it for upload.
    pub fn write(
        &mut self,
        id: PartId,
        world: &Mat4,
        shown: bool,
        layer: RenderLayer,
        casts_shadows: bool,
    ) {
        let world = &match self.state[id as usize].local {
            Some(local) => *world * local,
            None => *world,
        };
        let row = self.rows[id as usize];
        let normal = glam::Mat3::from_mat4(*world).inverse().transpose();
        let start = id as usize * PART_ROW_FLOATS;
        let out = &mut self.gpu[start..start + PART_ROW_FLOATS];
        out[..16].copy_from_slice(&world.to_cols_array());
        for (column, values) in [normal.x_axis, normal.y_axis, normal.z_axis]
            .iter()
            .enumerate()
        {
            out[16 + column * 4..16 + column * 4 + 3].copy_from_slice(&values.to_array());
            // The normal columns' spare lanes carry the texture origin.
            out[16 + column * 4 + 3] = row.texture_space[column];
        }
        out[28..32].copy_from_slice(&row.color);
        out[32..35].copy_from_slice(&row.emission);
        out[35] = row.texture_space[3];
        out[36..38].copy_from_slice(&row.fade);
        let state = &mut self.state[id as usize];
        let mirrored = world.determinant() < 0.0;
        if state.mirrored != mirrored || state.shown != shown || state.layer != layer {
            self.regrouped = true;
        }
        let bounds = state.local_bounds.transformed(world);
        // Scattered copies are not the indirect light's scene: they mark no
        // bricks, and their run re-culls as a whole.
        let leader = state.leader;
        if leader.is_none() && !state.world_bounds.is_empty() && state.world_bounds != bounds {
            self.former_bounds.push(state.world_bounds);
        }
        state.world_bounds = bounds;
        state.mirrored = mirrored;
        state.shown = shown;
        state.layer = layer;
        state.casts_shadows = casts_shadows;
        self.dirty.insert(id);
        match leader {
            Some(leader) => {
                self.dirty_runs.insert(leader);
                self.regrouped = true;
            }
            None => {
                self.moved.insert(id);
            }
        }
    }

    /// Make `ids` (inserted, one mesh group's copies) a run led by the
    /// first.
    pub fn make_run(&mut self, ids: Vec<PartId>) {
        let Some(&leader) = ids.first() else { return };
        for &id in &ids {
            self.state[id as usize].leader = Some(leader);
        }
        self.dirty_runs.insert(leader);
        self.runs.insert(
            leader,
            Run {
                ids,
                bounds: Aabb::EMPTY,
            },
        );
    }

    /// Bring the bounds of runs whose copies moved up to date.
    pub fn settle_runs(&mut self) {
        for leader in std::mem::take(&mut self.dirty_runs) {
            if let Some(run) = self.runs.get_mut(&leader) {
                run.bounds = run.ids.iter().fold(Aabb::EMPTY, |bounds, id| {
                    bounds.union(&self.state[*id as usize].world_bounds)
                });
            }
        }
    }

    /// Whether the CPU lists skip this part: a scattered copy they draw
    /// through its run's leader.
    pub fn follows(&self, id: PartId) -> bool {
        self.state[id as usize]
            .leader
            .is_some_and(|leader| leader != id)
    }

    /// The bounds the CPU lists cull a part by: its run's for a leader.
    pub fn cull_bounds(&self, id: PartId) -> &Aabb {
        match self.runs.get(&id) {
            Some(run) => &run.bounds,
            None => &self.state[id as usize].world_bounds,
        }
    }

    fn acquire_key(&mut self, part: &Part) -> u32 {
        let key = BatchKey {
            mesh: part.mesh.clone(),
            first_index: part.first_index,
            index_count: part.index_count,
            material: part.material.clone(),
            wireframe: part.wireframe,
        };
        if let Some((id, references)) = self.keys.get_mut(&key) {
            *references += 1;
            return *id;
        }
        let id = self.free_keys.pop().unwrap_or_else(|| {
            self.next_key += 1;
            self.next_key - 1
        });
        self.keys.insert(key, (id, 1));
        id
    }

    fn release_key(&mut self, part: Part) {
        let key = BatchKey {
            mesh: part.mesh,
            first_index: part.first_index,
            index_count: part.index_count,
            material: part.material,
            wireframe: part.wireframe,
        };
        if let Some((id, references)) = self.keys.get_mut(&key) {
            *references -= 1;
            if *references == 0 {
                self.free_keys.push(*id);
                self.keys.remove(&key);
            }
        }
    }
}

pub(crate) enum Environment {
    /// Nothing selected: the Engine default clear colour.
    Default,
    Color([f32; 4]),
    Sky(SkyBackgroundDescriptor),
}

pub(crate) struct Tables {
    pub textures: HashMap<String, GpuTexture>,
    /// Each texture's colour reduced to a small grid, for the probe bake's
    /// albedo (`probes.rs`).
    pub texture_thumbs: HashMap<String, crate::probes::Thumb>,
    /// Each material's albedo factor for the probe bake, by material id.
    pub material_means: HashMap<String, [f32; 3]>,
    /// Product shaders by id, with the composer id materials shaded by
    /// them take (`Shaders::product`).
    pub shaders: HashMap<String, (render_model::ShaderDescriptor, u32)>,
    pub materials: Slots<MaterialRow>,
    /// The palettes each terrain layer material of more than four layers
    /// has a narrowed variant for (`apply.rs` `define_layer_variant`), by
    /// material id.
    pub layer_variants: HashMap<String, BTreeSet<Vec<u8>>>,
    pub static_meshes: Slots<GpuMesh>,
    pub payload_meshes: HashMap<RenderHandle, GpuMesh>,
    pub nodes: HashMap<RenderHandle, NodeRow>,
    /// The sprite nodes, so sprite preparation visits sprites, not the scene.
    pub sprites: BTreeSet<RenderHandle>,
    pub parts: Parts,
    /// Indexed by the atlas name's id in `names`.
    pub atlases: Slots<AtlasRow>,
    pub names: Names,
    pub voxel_objects: Slots<VoxelObjectRow>,
    pub animated_assets: Slots<AnimatedAssetRow>,
    pub animated: HashMap<RenderHandle, AnimatedInstance>,
    /// Animation controllers by projection handle.
    pub controllers: HashMap<u64, ControllerRow>,
    pub environment: Environment,
    pub fog: Option<FogDescriptor>,
    pub tone_mapping: ToneMappingDescriptor,
    pub bloom: Option<render_model::BloomDescriptor>,
    pub auto_exposure: Option<render_model::AutoExposureDescriptor>,
    pub color_grading: Option<render_model::ColorGradingDescriptor>,
    pub atmosphere: Option<render_model::AtmosphereDescriptor>,
    pub sun_shafts: Option<render_model::SunShaftsDescriptor>,
    pub wind: Option<render_model::WindDescriptor>,
    /// The sky's cloud layer (`sky.wgsl` `fs_clouds`).
    pub clouds: Option<render_model::CloudsDescriptor>,
    /// How wet the scene's surfaces are (`lighting.wgsl` `wetted`).
    pub wetness: Option<render_model::WetnessDescriptor>,
    /// The backdrop layer's link to the world's cameras (`frame.rs`
    /// `backdrop_camera`).
    pub backdrop: Option<render_model::BackdropDescriptor>,
    /// The cloud regions over the cloud layer (`clouds.wgsl`).
    pub cloud_regions: std::collections::BTreeMap<u32, render_model::CloudRegionDescriptor>,
    /// The volumetric fog's medium and its fog volumes
    /// (`volumetric_fog.rs`).
    pub volumetric_fog: render_model::VolumetricFogDescriptor,
    pub fog_volumes: std::collections::BTreeMap<u32, render_model::FogVolumeDescriptor>,
    /// The precipitation around the camera (`precipitation.rs`).
    pub precipitation: Option<render_model::PrecipitationDescriptor>,
    /// The image effect over each primary view (`image_effect.rs`).
    pub image_effect: Option<render_model::ImageEffectDescriptor>,
    /// The indirect light volume requested (`probes.rs`).
    pub indirect_light: Option<render_model::IndirectLightDescriptor>,
    pub sky_light: Option<render_model::SkyLightDescriptor>,
    /// Nodes whose transform, visibility or parent changed since `prepare`.
    pub dirty_nodes: HashSet<RenderHandle>,
    /// Light nodes; the light rows are rebuilt when any of them changes.
    pub lights: HashSet<RenderHandle>,
    pub lights_dirty: bool,
    pub environment_dirty: bool,
}

impl Tables {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            texture_thumbs: HashMap::new(),
            material_means: HashMap::new(),
            shaders: HashMap::new(),
            materials: Slots::default(),
            static_meshes: Slots::default(),
            payload_meshes: HashMap::new(),
            layer_variants: HashMap::new(),
            nodes: HashMap::new(),
            sprites: BTreeSet::new(),
            parts: Parts::default(),
            atlases: Slots::default(),
            names: Names::default(),
            voxel_objects: Slots::default(),
            animated_assets: Slots::default(),
            animated: HashMap::new(),
            controllers: HashMap::new(),
            environment: Environment::Default,
            fog: None,
            tone_mapping: ToneMappingDescriptor::NONE,
            bloom: None,
            auto_exposure: None,
            color_grading: None,
            atmosphere: None,
            sun_shafts: None,
            wind: None,
            clouds: None,
            wetness: None,
            backdrop: None,
            cloud_regions: std::collections::BTreeMap::new(),
            volumetric_fog: render_model::VolumetricFogDescriptor::DEFAULT,
            fog_volumes: std::collections::BTreeMap::new(),
            precipitation: None,
            image_effect: None,
            indirect_light: None,
            sky_light: None,
            dirty_nodes: HashSet::new(),
            lights: HashSet::new(),
            lights_dirty: true,
            environment_dirty: true,
        }
    }
}
