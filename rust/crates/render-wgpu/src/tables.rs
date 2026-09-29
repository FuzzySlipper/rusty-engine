//! Typed, handle-keyed GPU tables. This is the layout family modules extend.
//!
//! | Table | Key | Row | Filled by |
//! |---|---|---|---|
//! | `textures` | texture id | GPU texture view and sampler | `DefineTexture` / `ReleaseTexture` |
//! | `materials` | material id | descriptor and bind group | `DefineMaterial` / `ReleaseMaterial` |
//! | `static_meshes` | static mesh asset id | vertex/index buffers, groups, default slots | `DefineStaticMesh` / `ReleaseStaticMesh` |
//! | `payload_meshes` | node handle | vertex/index buffers of a primitive's replaced payload or line | `ReplaceMeshPayload`, `Create` (line) |
//! | `nodes` | `RenderHandle` | parent, children, local and world transform, visibility, layer, kind, owned parts | `Create*`, `Update`, `Destroy`, `SetParentJoint`, `UpdateLight`, `SetMaterialInstanceParameters` |
//! | `parts` | `PartId` (dense) | one drawable (node, mesh group, material) and its GPU `PartRow` | derived from nodes |
//! | `atlases` | atlas id | retained descriptor | `DefineSpriteAtlas` / `ReleaseSpriteAtlas` |
//! | `environment` | (single) | background colour or equirectangular sky (with blend) | `SetBackgroundColor` / `SetSkyBackground` |
//!
//! Animated meshes, voxel objects and sprites are nodes (they take part in the
//! hierarchy, transforms and visibility) whose kind a family module realizes;
//! their descriptors wait in the node row until then.
//!
//! Dirty marks are the only change tracking: a transform or visibility change
//! marks its node; `prepare` recomputes that subtree's world rows and uploads
//! only those `PartRow`s. Nothing is re-derived for unchanged nodes.

use std::collections::{BTreeMap, HashMap, HashSet};

use glam::{Mat4, Quat, Vec3};
use render_model::{
    AnimatedMeshInstanceDescriptor, Geometry, LightDescriptor, Material,
    MaterialInstanceParameters, RenderHandle, RenderLayer, RenderMaterialDescriptor,
    SkyBackgroundDescriptor, SpriteAtlasDescriptor, SpriteInstanceDescriptor, Transform,
    VoxelObjectInstanceDescriptor,
};

pub(crate) fn transform_matrix(transform: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from(transform.scale),
        Quat::from_array(transform.rotation).normalize(),
        Vec3::from(transform.translation),
    )
}

pub(crate) struct GpuTexture {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

pub(crate) struct MaterialRow {
    pub descriptor: RenderMaterialDescriptor,
    pub bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Topology {
    Triangles,
    Lines,
}

pub(crate) struct GpuMesh {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub topology: Topology,
    /// (material slot, first index, index count)
    pub groups: Vec<(u16, u32, u32)>,
    /// Static mesh default slot bindings; empty for payload meshes.
    pub slots: BTreeMap<u16, String>,
}

/// Which mesh a part draws.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum MeshRef {
    Static(String),
    Payload(RenderHandle),
    Builtin(Builtin),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Builtin {
    Cube,
    Sphere,
    Quad,
    Point,
}

/// Which material bind group a part uses.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum MaterialRef {
    Retained(String),
    /// Primitive nodes: flat colour, no lighting (Three `MeshBasicMaterial`).
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
    // Realized by family modules (#8784, #8787, #8788).
    #[allow(dead_code, reason = "realized by #8788")]
    AnimatedMesh(Box<AnimatedMeshInstanceDescriptor>),
    VoxelObject(Box<VoxelObjectInstanceDescriptor>),
    Sprite(Box<SpriteInstanceDescriptor>),
}

pub(crate) struct NodeRow {
    pub parent: Option<RenderHandle>,
    pub parent_joint: Option<String>,
    pub children: Vec<RenderHandle>,
    pub local: Mat4,
    pub world: Mat4,
    pub visible: bool,
    pub world_visible: bool,
    pub layer: RenderLayer,
    /// The root ancestor's layer: children draw in their root's scene.
    pub world_layer: RenderLayer,
    pub kind: NodeKind,
    pub parts: Vec<PartId>,
}

pub(crate) type PartId = u32;

pub(crate) struct Part {
    pub node: RenderHandle,
    pub mesh: MeshRef,
    pub first_index: u32,
    pub index_count: u32,
    pub material: MaterialRef,
}

/// GPU row per part: model matrix, normal matrix (3 columns), linear colour
/// (rgb, alpha) and emission (rgb pre-multiplied by intensity).
pub(crate) const PART_ROW_FLOATS: usize = 16 + 12 + 4 + 4;

#[derive(Clone, Copy)]
pub(crate) struct PartRow {
    pub color: [f32; 4],
    pub emission: [f32; 3],
}

#[derive(Default)]
pub(crate) struct Parts {
    pub meta: Vec<Option<Part>>,
    pub rows: Vec<PartRow>,
    /// CPU copy of the dense GPU rows, uploaded where dirty.
    pub gpu: Vec<f32>,
    pub free: Vec<PartId>,
    pub dirty: HashSet<PartId>,
    /// Rows past the GPU buffer's capacity force a buffer reallocation.
    pub grown: bool,
}

impl Parts {
    pub fn insert(&mut self, part: Part, row: PartRow) -> PartId {
        let id = if let Some(id) = self.free.pop() {
            self.meta[id as usize] = Some(part);
            self.rows[id as usize] = row;
            id
        } else {
            self.meta.push(Some(part));
            self.rows.push(row);
            self.gpu.resize(self.meta.len() * PART_ROW_FLOATS, 0.0);
            self.grown = true;
            (self.meta.len() - 1) as PartId
        };
        self.dirty.insert(id);
        id
    }

    pub fn remove(&mut self, id: PartId) {
        self.meta[id as usize] = None;
        self.dirty.remove(&id);
        self.free.push(id);
    }

    /// Write a part's world state into its GPU row and mark it for upload.
    pub fn write(&mut self, id: PartId, world: &Mat4) {
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
            out[16 + column * 4 + 3] = 0.0;
        }
        out[28..32].copy_from_slice(&row.color);
        out[32..35].copy_from_slice(&row.emission);
        out[35] = 0.0;
        self.dirty.insert(id);
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
    pub materials: HashMap<String, MaterialRow>,
    pub static_meshes: HashMap<String, GpuMesh>,
    pub payload_meshes: HashMap<RenderHandle, GpuMesh>,
    pub nodes: HashMap<RenderHandle, NodeRow>,
    pub parts: Parts,
    pub atlases: HashMap<String, SpriteAtlasDescriptor>,
    pub environment: Environment,
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
            materials: HashMap::new(),
            static_meshes: HashMap::new(),
            payload_meshes: HashMap::new(),
            nodes: HashMap::new(),
            parts: Parts::default(),
            atlases: HashMap::new(),
            environment: Environment::Default,
            dirty_nodes: HashSet::new(),
            lights: HashSet::new(),
            lights_dirty: true,
            environment_dirty: true,
        }
    }
}
