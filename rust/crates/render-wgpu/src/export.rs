//! GLB export of an output job's frozen frame.
//!
//! The export holds the selected node, its descendants, and the ancestor path
//! that places it, with each node's local transform. The geometry is the
//! geometry render-wgpu draws: built-in primitives, mesh payloads, static
//! meshes by material slot, and animated meshes with their rig. Materials
//! map as follows:
//! - retained materials become metallic-roughness with metalness 0;
//! - primitives become `KHR_materials_unlit`;
//! - animated meshes keep their embedded materials unless overridden.
//!
//! Textures embed their PNG bytes. With `include_animations`, skinned meshes
//! keep their skins and every resolved clip.
//!
//! Some nodes fail the job, naming the node, where glTF has no counterpart:
//! sprites, voxel-surface materials and voxel objects, and ambient lights.
//! Hidden nodes are exported too.
//! No GPU is involved.

use std::collections::{BTreeMap, HashMap};

use glam::{Quat, Vec3};
use render_host_contracts::{RenderOutputJob, RenderOutputOperation};
use render_model::{
    AnimatedMeshAsset, Geometry, LightDescriptor, Material, MaterialAlphaModeDescriptor,
    MaterialInstanceParameters, MeshPayloadDescriptor, RenderDiff, RenderHandle,
    RenderMaterialDescriptor, StaticMeshAsset, TextureDescriptor, TextureFilter,
    TexturePayloadSource, TextureWrap, Transform,
};
use serde_json::{json, Value};

use crate::animated::{decode_animated_asset, joint_nodes};
use crate::glb::{GlbAlpha, GlbClip, GlbModel, GlbTexture, Interp, Path};
use crate::pipelines::VERTEX_FLOATS;
use crate::primitives;
use crate::resources::{self, encode_png, ResourceSource};
use crate::tables::Builtin;

/// The material a static mesh or payload slot without one draws with.
const FALLBACK_ROUGHNESS: f32 = 1.0;

const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const FLOAT: u32 = 5126;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const MODE_LINES: u32 = 1;
const NEAREST: u32 = 9728;
const LINEAR: u32 = 9729;
const CLAMP_TO_EDGE: u32 = 33071;
const REPEAT: u32 = 10497;

/// Export `job`'s frozen frame as a self-contained binary glTF.
pub fn export_glb(
    job: &RenderOutputJob,
    resources: &dyn ResourceSource,
) -> Result<Vec<u8>, String> {
    let RenderOutputOperation::Glb { include_animations } = job.operation else {
        return Err("exportGlb: the job is an image capture".to_owned());
    };
    let scene = Frozen::read(&job.frame);
    if !scene.nodes.contains_key(&job.source) {
        return Err(format!(
            "exportGlb: retained handle {} is not in the captured frame",
            job.source.raw()
        ));
    }
    let mut writer = Writer {
        scene: &scene,
        resources,
        include_animations,
        document: Document::default(),
        textures: HashMap::new(),
        static_geometry: HashMap::new(),
        builtin_geometry: HashMap::new(),
        joints: HashMap::new(),
    };
    let root = writer.export(job.source)?;
    writer.document.finish(root)
}

// ── The frozen frame ────────────────────────────────────────────────────────

enum Kind {
    Primitive {
        geometry: Geometry,
        material: Material,
        payload: Option<MeshPayloadDescriptor>,
    },
    StaticMesh {
        asset: String,
        overrides: BTreeMap<u16, String>,
    },
    AnimatedMesh {
        asset: String,
        overrides: BTreeMap<u16, String>,
    },
    VoxelObject,
    Sprite,
    Light(LightDescriptor),
}

struct Node {
    parent: Option<RenderHandle>,
    parent_joint: Option<String>,
    transform: Transform,
    label: Option<String>,
    kind: Kind,
    parameters: BTreeMap<u16, MaterialInstanceParameters>,
}

/// The frame's definitions and nodes, as its ops leave them.
#[derive(Default)]
struct Frozen {
    textures: HashMap<String, TextureDescriptor>,
    materials: HashMap<String, RenderMaterialDescriptor>,
    static_meshes: HashMap<String, StaticMeshAsset>,
    animated_meshes: HashMap<String, AnimatedMeshAsset>,
    nodes: BTreeMap<RenderHandle, Node>,
}

impl Frozen {
    fn read(frame: &render_model::RenderFrameDiff) -> Self {
        let mut scene = Self::default();
        let overrides = |slots: &[render_model::MeshMaterialSlot]| {
            slots
                .iter()
                .map(|slot| (slot.slot, slot.material.clone()))
                .collect()
        };
        for op in &frame.ops {
            let (handle, parent, transform, label, kind) = match op {
                RenderDiff::DefineTexture { texture } => {
                    scene.textures.insert(texture.id.clone(), texture.clone());
                    continue;
                }
                RenderDiff::DefineMaterial { material } => {
                    scene
                        .materials
                        .insert(material.id.clone(), material.clone());
                    continue;
                }
                RenderDiff::DefineStaticMesh { asset } => {
                    scene
                        .static_meshes
                        .insert(asset.asset.clone(), asset.clone());
                    continue;
                }
                RenderDiff::DefineAnimatedMesh { asset } => {
                    scene
                        .animated_meshes
                        .insert(asset.asset.clone(), asset.clone());
                    continue;
                }
                RenderDiff::Create {
                    handle,
                    parent,
                    node,
                } => (
                    *handle,
                    *parent,
                    node.transform,
                    node.metadata.label.clone(),
                    Kind::Primitive {
                        geometry: node.geometry,
                        material: node.material,
                        payload: None,
                    },
                ),
                RenderDiff::CreateStaticMeshInstance {
                    handle,
                    parent,
                    instance,
                } => (
                    *handle,
                    *parent,
                    instance.transform,
                    instance.metadata.label.clone(),
                    Kind::StaticMesh {
                        asset: instance.asset.clone(),
                        overrides: overrides(&instance.material_overrides),
                    },
                ),
                RenderDiff::CreateAnimatedMeshInstance {
                    handle,
                    parent,
                    instance,
                } => (
                    *handle,
                    *parent,
                    instance.transform,
                    instance.metadata.label.clone(),
                    Kind::AnimatedMesh {
                        asset: instance.asset.clone(),
                        overrides: overrides(&instance.material_overrides),
                    },
                ),
                RenderDiff::CreateVoxelObjectInstance {
                    handle,
                    parent,
                    instance,
                } => (
                    *handle,
                    *parent,
                    instance.transform,
                    instance.metadata.label.clone(),
                    Kind::VoxelObject,
                ),
                RenderDiff::CreateSprite {
                    handle,
                    parent,
                    sprite,
                } => (
                    *handle,
                    *parent,
                    sprite.transform,
                    sprite.metadata.label.clone(),
                    Kind::Sprite,
                ),
                RenderDiff::CreateLight {
                    handle,
                    parent,
                    light,
                } => (
                    *handle,
                    *parent,
                    Transform::default(),
                    None,
                    Kind::Light(light.clone()),
                ),
                RenderDiff::Update {
                    handle,
                    transform,
                    material,
                    ..
                } => {
                    if let Some(node) = scene.nodes.get_mut(handle) {
                        if let Some(transform) = transform {
                            node.transform = *transform;
                        }
                        if let (Some(value), Kind::Primitive { material, .. }) =
                            (material, &mut node.kind)
                        {
                            *material = *value;
                        }
                    }
                    continue;
                }
                RenderDiff::UpdateLight { handle, light } => {
                    if let Some(node) = scene.nodes.get_mut(handle) {
                        node.kind = Kind::Light(light.clone());
                    }
                    continue;
                }
                RenderDiff::ReplaceMeshPayload { handle, payload } => {
                    if let Some(Node {
                        kind: Kind::Primitive { payload: slot, .. },
                        ..
                    }) = scene.nodes.get_mut(handle)
                    {
                        *slot = Some(payload.clone());
                    }
                    continue;
                }
                RenderDiff::SetParentJoint { handle, joint } => {
                    if let Some(node) = scene.nodes.get_mut(handle) {
                        node.parent_joint = joint.clone();
                    }
                    continue;
                }
                RenderDiff::SetMaterialInstanceParameters {
                    handle,
                    slot,
                    parameters,
                } => {
                    if let Some(node) = scene.nodes.get_mut(handle) {
                        match parameters {
                            Some(parameters) => node.parameters.insert(*slot, *parameters),
                            None => node.parameters.remove(slot),
                        };
                    }
                    continue;
                }
                // Background, playback, inspection and the rest change nothing
                // glTF holds.
                _ => continue,
            };
            scene.nodes.insert(
                handle,
                Node {
                    parent,
                    parent_joint: None,
                    transform,
                    label,
                    kind,
                    parameters: BTreeMap::new(),
                },
            );
        }
        scene
    }

    fn children(&self, handle: RenderHandle) -> impl Iterator<Item = RenderHandle> + '_ {
        self.nodes
            .iter()
            .filter(move |(_, node)| node.parent == Some(handle))
            .map(|(child, _)| *child)
    }
}

// ── The glTF document ───────────────────────────────────────────────────────

#[derive(Default)]
struct Document {
    nodes: Vec<Value>,
    children: Vec<Vec<usize>>,
    meshes: Vec<Value>,
    materials: Vec<Value>,
    textures: Vec<Value>,
    images: Vec<Value>,
    samplers: Vec<Value>,
    accessors: Vec<Value>,
    views: Vec<Value>,
    skins: Vec<Value>,
    animations: Vec<Value>,
    lights: Vec<Value>,
    extensions: std::collections::BTreeSet<&'static str>,
    bin: Vec<u8>,
}

impl Document {
    fn node(&mut self, value: Value) -> usize {
        self.nodes.push(value);
        self.children.push(Vec::new());
        self.nodes.len() - 1
    }

    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let mut view = json!({
            "buffer": 0,
            "byteOffset": self.bin.len(),
            "byteLength": bytes.len(),
        });
        if let Some(target) = target {
            view["target"] = json!(target);
        }
        self.bin.extend_from_slice(bytes);
        self.views.push(view);
        self.views.len() - 1
    }

    fn accessor(&mut self, view: usize, component: u32, count: usize, kind: &str) -> usize {
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": component,
            "count": count,
            "type": kind,
        }));
        self.accessors.len() - 1
    }

    /// `components` floats per element; `bounds` records min and max, which
    /// glTF requires of positions and animation inputs.
    fn floats(
        &mut self,
        data: &[f32],
        components: usize,
        target: Option<u32>,
        bounds: bool,
    ) -> usize {
        let view = self.view(bytemuck::cast_slice(data), target);
        let kind = match components {
            1 => "SCALAR",
            2 => "VEC2",
            3 => "VEC3",
            4 => "VEC4",
            _ => "MAT4",
        };
        let count = data.len() / components;
        let index = self.accessor(view, FLOAT, count, kind);
        if bounds && count > 0 {
            let (mut min, mut max) = (vec![f32::MAX; components], vec![f32::MIN; components]);
            for element in data.chunks_exact(components) {
                for (axis, value) in element.iter().enumerate() {
                    min[axis] = min[axis].min(*value);
                    max[axis] = max[axis].max(*value);
                }
            }
            self.accessors[index]["min"] = json!(min);
            self.accessors[index]["max"] = json!(max);
        }
        index
    }

    fn indices(&mut self, indices: &[u32]) -> usize {
        let view = self.view(bytemuck::cast_slice(indices), Some(ELEMENT_ARRAY_BUFFER));
        self.accessor(view, UNSIGNED_INT, indices.len(), "SCALAR")
    }

    fn joints(&mut self, joints: &[[u16; 4]]) -> usize {
        let view = self.view(bytemuck::cast_slice(joints), Some(ARRAY_BUFFER));
        self.accessor(view, UNSIGNED_SHORT, joints.len(), "VEC4")
    }

    fn material(&mut self, value: Value) -> usize {
        self.materials.push(value);
        self.materials.len() - 1
    }

    fn finish(mut self, root: usize) -> Result<Vec<u8>, String> {
        for (node, children) in self.children.iter().enumerate() {
            if !children.is_empty() {
                self.nodes[node]["children"] = json!(children);
            }
        }
        let mut document = json!({
            "asset": { "version": "2.0", "generator": "Rusty Engine render-wgpu" },
            "scene": 0,
            "scenes": [{ "nodes": [root] }],
            "nodes": self.nodes,
        });
        for (key, values) in [
            ("meshes", self.meshes),
            ("materials", self.materials),
            ("textures", self.textures),
            ("images", self.images),
            ("samplers", self.samplers),
            ("accessors", self.accessors),
            ("bufferViews", self.views),
            ("skins", self.skins),
            ("animations", self.animations),
        ] {
            if !values.is_empty() {
                document[key] = Value::Array(values);
            }
        }
        if !self.bin.is_empty() {
            document["buffers"] = json!([{ "byteLength": self.bin.len() }]);
        }
        if !self.lights.is_empty() {
            document["extensions"] = json!({ "KHR_lights_punctual": { "lights": self.lights } });
            self.extensions.insert("KHR_lights_punctual");
        }
        if !self.extensions.is_empty() {
            document["extensionsUsed"] = json!(self.extensions);
        }
        let text = serde_json::to_vec(&document).map_err(|error| format!("exportGlb: {error}"))?;
        Ok(glb_container(&text, &self.bin))
    }
}

/// The binary glTF container: header, JSON chunk (space padded) and BIN
/// chunk (zero padded).
fn glb_container(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let padded = |length: usize| length.next_multiple_of(4);
    let bin_chunk = if bin.is_empty() {
        0
    } else {
        8 + padded(bin.len())
    };
    let total = 12 + 8 + padded(json.len()) + bin_chunk;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(padded(json.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(json);
    out.resize(out.len() + padded(json.len()) - json.len(), b' ');
    if !bin.is_empty() {
        out.extend_from_slice(&(padded(bin.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(bin);
        out.resize(total, 0);
    }
    out
}

// ── Writing nodes ───────────────────────────────────────────────────────────

/// (material slot, first index, index count) of a mesh's groups.
type Groups = Vec<(u16, u32, u32)>;

/// Geometry streams written once and shared by every primitive using them.
#[derive(Clone)]
struct Streams {
    attributes: Value,
    indices: usize,
    lines: bool,
}

struct Writer<'a> {
    scene: &'a Frozen,
    resources: &'a dyn ResourceSource,
    include_animations: bool,
    document: Document,
    /// Retained texture ids, and animated textures by (asset, index).
    textures: HashMap<String, usize>,
    /// A static mesh asset's streams and its groups.
    static_geometry: HashMap<String, (Streams, Groups)>,
    builtin_geometry: HashMap<Builtin, Streams>,
    /// Per written animated instance, its skin joints by name, which
    /// retained children attach to.
    joints: HashMap<RenderHandle, HashMap<String, usize>>,
}

impl<'a> Writer<'a> {
    /// Write the selection and return the root node: the ancestor path down
    /// to `source`, then its whole subtree.
    fn export(&mut self, source: RenderHandle) -> Result<usize, String> {
        let mut path = vec![source];
        while let Some(parent) = self.scene.nodes[path.last().unwrap()].parent {
            if !self.scene.nodes.contains_key(&parent) {
                break;
            }
            path.push(parent);
        }
        path.reverse();
        let root = self.node(path[0])?;
        let mut above = (path[0], root);
        for handle in &path[1..] {
            let index = self.node(*handle)?;
            self.attach(above, *handle, index);
            above = (*handle, index);
        }
        self.subtree(above).map(|()| root)
    }

    /// Write the descendants of a written node.
    fn subtree(&mut self, (handle, index): (RenderHandle, usize)) -> Result<(), String> {
        let children: Vec<RenderHandle> = self.scene.children(handle).collect();
        for child in children {
            let child_index = self.node(child)?;
            self.attach((handle, index), child, child_index);
            self.subtree((child, child_index))?;
        }
        Ok(())
    }

    /// Parent a written child under its written parent, or under the parent
    /// rig's joint the child is attached to.
    fn attach(
        &mut self,
        (parent, index): (RenderHandle, usize),
        child: RenderHandle,
        child_index: usize,
    ) {
        let joint = self.scene.nodes[&child]
            .parent_joint
            .as_ref()
            .and_then(|joint| self.joints.get(&parent)?.get(joint))
            .copied();
        self.document.children[joint.unwrap_or(index)].push(child_index);
    }

    /// Write one node without its retained children.
    fn node(&mut self, handle: RenderHandle) -> Result<usize, String> {
        let scene: &'a Frozen = self.scene;
        let node = &scene.nodes[&handle];
        let name = node
            .label
            .clone()
            .unwrap_or_else(|| format!("node {}", handle.raw()));
        let transform = node.transform;
        let index = self.document.node(json!({
            "name": name,
            "translation": transform.translation,
            "rotation": transform.rotation,
            "scale": transform.scale,
        }));
        let mesh = match &node.kind {
            Kind::Primitive {
                geometry,
                material,
                payload,
            } => self.primitive(node, *geometry, *material, payload.as_ref())?,
            Kind::StaticMesh { asset, overrides } => Some(self.static_mesh(node, asset, overrides)?),
            Kind::AnimatedMesh { asset, overrides } => {
                self.animated(handle, index, asset, overrides, &node.parameters)?;
                None
            }
            Kind::VoxelObject => {
                return Err(format!(
                    "exportGlb: voxel object {} draws with voxel surface materials, which glTF cannot represent",
                    handle.raw()
                ))
            }
            Kind::Sprite => {
                return Err(format!(
                    "exportGlb: sprite {} is not representable as a glTF node",
                    handle.raw()
                ))
            }
            Kind::Light(light) => {
                self.light(index, handle, light)?;
                None
            }
        };
        if let Some(mesh) = mesh {
            self.document.nodes[index]["mesh"] = json!(mesh);
        }
        Ok(index)
    }

    fn primitive(
        &mut self,
        node: &Node,
        geometry: Geometry,
        material: Material,
        payload: Option<&MeshPayloadDescriptor>,
    ) -> Result<Option<usize>, String> {
        if let Some(payload) = payload {
            // An uploaded payload draws each group with its slot material,
            // tinted by the node's view material.
            let (streams, groups) = self.payload_streams(payload)?;
            let mut primitives = Vec::new();
            for (slot, start, count) in groups {
                let id = format!("voxel-material/{slot}");
                let parameters = node.parameters.get(&slot);
                let material =
                    self.retained_material(Some(&id), slot, parameters, material.color)?;
                primitives.push(self.subset(&streams, start, count, material));
            }
            return Ok(Some(self.mesh(primitives)));
        }
        let streams = match geometry {
            Geometry::Group => return Ok(None),
            Geometry::Line { a, b } => {
                let line = primitives::line(a, b);
                let mut streams = self.interleaved(&line.vertices, &line.indices);
                streams.lines = true;
                streams
            }
            // A point draws as the small cube render-wgpu draws.
            Geometry::Cube => self.builtin(Builtin::Cube),
            Geometry::Sphere => self.builtin(Builtin::Sphere),
            Geometry::Quad => self.builtin(Builtin::Quad),
            Geometry::Point => self.builtin(Builtin::Point),
        };
        let unlit = self.unlit(material.color);
        let mut primitive = json!({
            "attributes": streams.attributes,
            "indices": streams.indices,
            "material": unlit,
        });
        if streams.lines {
            primitive["mode"] = json!(MODE_LINES);
        }
        Ok(Some(self.mesh(vec![primitive])))
    }

    fn static_mesh(
        &mut self,
        node: &Node,
        asset: &str,
        overrides: &BTreeMap<u16, String>,
    ) -> Result<usize, String> {
        let definition = self.scene.static_meshes.get(asset).ok_or_else(|| {
            format!("exportGlb: static mesh {asset} is not in the captured frame")
        })?;
        if !self.static_geometry.contains_key(asset) {
            let streams = self.payload_streams(&definition.payload)?;
            self.static_geometry.insert(asset.to_owned(), streams);
        }
        let (streams, groups) = self.static_geometry[asset].clone();
        let slots: BTreeMap<u16, &String> = definition
            .material_slots
            .iter()
            .map(|slot| (slot.slot, &slot.material))
            .collect();
        let mut primitives = Vec::new();
        for (slot, start, count) in groups {
            let id = overrides.get(&slot).or(slots.get(&slot).copied());
            let parameters = node.parameters.get(&slot);
            let material =
                self.retained_material(id.map(String::as_str), slot, parameters, [1.0; 4])?;
            primitives.push(self.subset(&streams, start, count, material));
        }
        Ok(self.mesh(primitives))
    }

    /// A primitive drawing `count` indices from `start` of shared streams.
    fn subset(&mut self, streams: &Streams, start: u32, count: u32, material: usize) -> Value {
        let accessor = &self.document.accessors[streams.indices];
        let whole = accessor["count"].as_u64() == Some(u64::from(count)) && start == 0;
        let indices = if whole {
            streams.indices
        } else {
            let view = accessor["bufferView"].clone();
            let offset = start as usize * 4;
            self.document.accessors.push(json!({
                "bufferView": view,
                "byteOffset": offset,
                "componentType": UNSIGNED_INT,
                "count": count,
                "type": "SCALAR",
            }));
            self.document.accessors.len() - 1
        };
        json!({ "attributes": streams.attributes, "indices": indices, "material": material })
    }

    fn mesh(&mut self, primitives: Vec<Value>) -> usize {
        self.document
            .meshes
            .push(json!({ "primitives": primitives }));
        self.document.meshes.len() - 1
    }

    fn builtin(&mut self, kind: Builtin) -> Streams {
        if let Some(streams) = self.builtin_geometry.get(&kind) {
            return streams.clone();
        }
        let geometry = primitives::builtin(kind);
        let streams = self.interleaved(&geometry.vertices, &geometry.indices);
        self.builtin_geometry.insert(kind, streams.clone());
        streams
    }

    /// Write render-wgpu's interleaved built-in vertices as separate streams
    /// (their colour is constant white and is left out).
    fn interleaved(&mut self, vertices: &[f32], indices: &[u32]) -> Streams {
        let stream = |offset: usize, width: usize| -> Vec<f32> {
            vertices
                .as_chunks::<VERTEX_FLOATS>()
                .0
                .iter()
                .flat_map(|vertex| vertex[offset..offset + width].to_vec())
                .collect()
        };
        let position = self
            .document
            .floats(&stream(0, 3), 3, Some(ARRAY_BUFFER), true);
        let normal = self
            .document
            .floats(&stream(3, 3), 3, Some(ARRAY_BUFFER), false);
        let uv = self
            .document
            .floats(&stream(6, 2), 2, Some(ARRAY_BUFFER), false);
        Streams {
            attributes: json!({ "POSITION": position, "NORMAL": normal, "TEXCOORD_0": uv }),
            indices: self.document.indices(indices),
            lines: false,
        }
    }

    fn payload_streams(
        &mut self,
        payload: &MeshPayloadDescriptor,
    ) -> Result<(Streams, Groups), String> {
        let streams = resources::mesh_streams(payload, self.resources)
            .map_err(|detail| format!("exportGlb: mesh payload: {detail}"))?;
        let position = self
            .document
            .floats(&streams.positions, 3, Some(ARRAY_BUFFER), true);
        let normal = self
            .document
            .floats(&streams.normals, 3, Some(ARRAY_BUFFER), false);
        let mut attributes = json!({ "POSITION": position, "NORMAL": normal });
        if let Some(uvs) = &streams.uvs {
            attributes["TEXCOORD_0"] =
                json!(self.document.floats(uvs, 2, Some(ARRAY_BUFFER), false));
        }
        if let Some(colors) = &streams.colors {
            attributes["COLOR_0"] =
                json!(self.document.floats(colors, 4, Some(ARRAY_BUFFER), false));
        }
        let groups = if payload.groups.is_empty() {
            vec![(0, 0, streams.indices.len() as u32)]
        } else {
            payload
                .groups
                .iter()
                .map(|group| (group.material_slot, group.start, group.count))
                .collect()
        };
        Ok((
            Streams {
                attributes,
                indices: self.document.indices(&streams.indices),
                lines: false,
            },
            groups,
        ))
    }

    // ── Materials ───────────────────────────────────────────────────────────

    /// An unlit material for a primitive's view colour.
    fn unlit(&mut self, color: [f32; 4]) -> usize {
        self.document.extensions.insert("KHR_materials_unlit");
        let mut material = json!({
            "pbrMetallicRoughness": {
                "baseColorFactor": color,
                "metallicFactor": 0.0,
                "roughnessFactor": 1.0,
            },
            "extensions": { "KHR_materials_unlit": {} },
        });
        if color[3] < 1.0 {
            material["alphaMode"] = json!("BLEND");
        }
        self.document.material(material)
    }

    /// A metallic-roughness material for a retained material, with the
    /// node's instance parameters for its slot and a view colour multiplier.
    fn retained_material(
        &mut self,
        id: Option<&str>,
        slot: u16,
        parameters: Option<&MaterialInstanceParameters>,
        tint: [f32; 4],
    ) -> Result<usize, String> {
        let Some(descriptor) = id.and_then(|id| self.scene.materials.get(id)) else {
            let color = crate::apply::slot_color(slot);
            return Ok(self.document.material(json!({
                "pbrMetallicRoughness": {
                    "baseColorFactor": mul(color, tint),
                    "metallicFactor": 0.0,
                    "roughnessFactor": FALLBACK_ROUGHNESS,
                },
            })));
        };
        if descriptor.voxel_surface.is_some() {
            return Err(format!(
                "exportGlb: material {} uses a voxel surface mapping, which glTF cannot represent",
                descriptor.id
            ));
        }
        let texture_tint = parameters.map_or(descriptor.texture_tint, |p| p.texture_tint);
        let emission_color = parameters.map_or(descriptor.emission_color, |p| p.emission_color);
        let emission_intensity =
            parameters.map_or(descriptor.emission_intensity, |p| p.emission_intensity);
        let color = mul(mul(descriptor.color, texture_tint), tint);
        let blend =
            matches!(descriptor.alpha_mode, MaterialAlphaModeDescriptor::Blend) || color[3] < 1.0;
        let mut material = json!({
            "name": descriptor.id,
            "pbrMetallicRoughness": {
                "baseColorFactor": color,
                "metallicFactor": 0.0,
                "roughnessFactor": descriptor.roughness,
            },
            "doubleSided": descriptor.double_sided,
        });
        self.emission(&mut material, emission_color, emission_intensity);
        match (blend, descriptor.alpha_mode) {
            (true, _) => material["alphaMode"] = json!("BLEND"),
            (false, MaterialAlphaModeDescriptor::Mask { cutoff }) => {
                material["alphaMode"] = json!("MASK");
                material["alphaCutoff"] = json!(cutoff);
            }
            _ => {}
        }
        if let Some(texture) = &descriptor.texture {
            if let Some(index) = self.retained_texture(texture)? {
                material["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": index });
            }
        }
        Ok(self.document.material(material))
    }

    /// Emission: intensity above 1 moves into
    /// `KHR_materials_emissive_strength`.
    fn emission(&mut self, material: &mut Value, color: [f32; 3], intensity: f32) {
        if intensity <= 1.0 {
            material["emissiveFactor"] = json!(color.map(|channel| channel * intensity));
        } else {
            material["emissiveFactor"] = json!(color);
            material["extensions"] =
                json!({ "KHR_materials_emissive_strength": { "emissiveStrength": intensity } });
            self.document
                .extensions
                .insert("KHR_materials_emissive_strength");
        }
    }

    /// A retained texture's PNG bytes; `None` for a metadata-only descriptor,
    /// which draws as its colour.
    fn retained_texture(&mut self, id: &str) -> Result<Option<usize>, String> {
        if let Some(index) = self.textures.get(id) {
            return Ok(Some(*index));
        }
        let descriptor = self
            .scene
            .textures
            .get(id)
            .ok_or_else(|| format!("exportGlb: texture {id} is not in the captured frame"))?;
        let Some(payload) = &descriptor.payload else {
            return Ok(None);
        };
        let bytes = match &payload.source {
            TexturePayloadSource::Inline { encoded_bytes } => encoded_bytes.clone(),
            TexturePayloadSource::Resource { resource } => self
                .resources
                .bytes(resource)
                .ok_or_else(|| format!("exportGlb: resource {resource} is not available"))?
                .into_owned(),
        };
        let index = self.texture(
            &bytes,
            descriptor.filter == TextureFilter::Nearest,
            descriptor.wrap == TextureWrap::Repeat,
        );
        self.textures.insert(id.to_owned(), index);
        Ok(Some(index))
    }

    fn texture(&mut self, png: &[u8], nearest: bool, repeat: bool) -> usize {
        let view = self.document.view(png, None);
        self.document
            .images
            .push(json!({ "bufferView": view, "mimeType": "image/png" }));
        let (filter, wrap) = (
            if nearest { NEAREST } else { LINEAR },
            if repeat { REPEAT } else { CLAMP_TO_EDGE },
        );
        self.document.samplers.push(json!({
            "magFilter": filter,
            "minFilter": filter,
            "wrapS": wrap,
            "wrapT": wrap,
        }));
        self.document.textures.push(json!({
            "source": self.document.images.len() - 1,
            "sampler": self.document.samplers.len() - 1,
        }));
        self.document.textures.len() - 1
    }

    // ── Animated meshes ─────────────────────────────────────────────────────

    /// Write an animated instance's rig under its node `index`: the GLB's
    /// nodes at rest, its meshes, skins and (with `include_animations`) its
    /// resolved clips. Records the joints retained children attach to.
    fn animated(
        &mut self,
        handle: RenderHandle,
        index: usize,
        asset: &str,
        overrides: &BTreeMap<u16, String>,
        parameters: &BTreeMap<u16, MaterialInstanceParameters>,
    ) -> Result<(), String> {
        let definition = self.scene.animated_meshes.get(asset).ok_or_else(|| {
            format!("exportGlb: animated mesh {asset} is not in the captured frame")
        })?;
        let (model, clips) = decode_animated_asset(definition, self.resources)
            .map_err(|detail| format!("exportGlb: animated mesh {asset}: {detail}"))?;
        let nodes: Vec<usize> = model
            .nodes
            .iter()
            .enumerate()
            .map(|(glb_index, node)| {
                self.document.node(json!({
                    "name": node.name.clone().unwrap_or_else(|| format!("{asset} node {glb_index}")),
                    "translation": node.rest.translation.to_array(),
                    "rotation": node.rest.rotation.to_array(),
                    "scale": node.rest.scale.to_array(),
                }))
            })
            .collect();
        for (glb_index, node) in model.nodes.iter().enumerate() {
            let parent = node.parent.map_or(index, |parent| nodes[parent]);
            self.document.children[parent].push(nodes[glb_index]);
        }
        // Embedded materials, unless a slot override replaces them.
        let slots: HashMap<usize, u16> = definition
            .embedded_material_slots
            .iter()
            .map(|slot| (usize::from(slot.source_material_slot), slot.slot))
            .collect();
        let mut materials = HashMap::new();
        let mut skins = HashMap::new();
        for (glb_index, node) in model.nodes.iter().enumerate() {
            let Some(mesh) = node.mesh else { continue };
            let skin = node.skin.map(|skin| {
                *skins
                    .entry(skin)
                    .or_insert_with(|| self.skin(&model, skin, &nodes))
            });
            let mut primitives = Vec::new();
            for primitive in &model.meshes[mesh] {
                let material = match primitive.material {
                    Some(source) => match materials.get(&source) {
                        Some(written) => Some(*written),
                        None => {
                            let slot = slots.get(&source).copied();
                            let written = match slot.and_then(|slot| overrides.get(&slot)) {
                                Some(id) => {
                                    let slot = slot.unwrap_or_default();
                                    self.retained_material(
                                        Some(id),
                                        slot,
                                        parameters.get(&slot),
                                        [1.0; 4],
                                    )?
                                }
                                None => self.embedded_material(&model, asset, source),
                            };
                            materials.insert(source, written);
                            Some(written)
                        }
                    },
                    None => None,
                };
                primitives.push(self.skinned_primitive(primitive, material, skin.is_some()));
            }
            let mesh = self.mesh(primitives);
            self.document.nodes[nodes[glb_index]]["mesh"] = json!(mesh);
            if let Some(skin) = skin {
                self.document.nodes[nodes[glb_index]]["skin"] = json!(skin);
            }
        }
        if self.include_animations {
            let mut ids: Vec<&String> = clips.keys().collect();
            ids.sort();
            for id in ids {
                self.animation(id, &clips[id], &nodes);
            }
        }
        // Retained children attach to skin joints as the renderer attaches
        // them.
        let joints = joint_nodes(&model)
            .into_iter()
            .map(|(name, glb_index)| (name, nodes[glb_index]))
            .collect();
        self.joints.insert(handle, joints);
        Ok(())
    }

    fn skinned_primitive(
        &mut self,
        primitive: &crate::glb::GlbPrimitive,
        material: Option<usize>,
        skinned: bool,
    ) -> Value {
        let flat = |values: &[[f32; 3]]| values.iter().flatten().copied().collect::<Vec<f32>>();
        let position =
            self.document
                .floats(&flat(&primitive.positions), 3, Some(ARRAY_BUFFER), true);
        let normal = self
            .document
            .floats(&flat(&primitive.normals), 3, Some(ARRAY_BUFFER), false);
        let mut attributes = json!({ "POSITION": position, "NORMAL": normal });
        if let Some(uvs) = &primitive.uvs {
            let data: Vec<f32> = uvs.iter().flatten().copied().collect();
            attributes["TEXCOORD_0"] =
                json!(self.document.floats(&data, 2, Some(ARRAY_BUFFER), false));
        }
        if let Some(colors) = &primitive.colors {
            let data: Vec<f32> = colors.iter().flatten().copied().collect();
            attributes["COLOR_0"] =
                json!(self.document.floats(&data, 4, Some(ARRAY_BUFFER), false));
        }
        if let (true, Some(joints), Some(weights)) =
            (skinned, &primitive.joints, &primitive.weights)
        {
            attributes["JOINTS_0"] = json!(self.document.joints(joints));
            let data: Vec<f32> = weights.iter().flatten().copied().collect();
            attributes["WEIGHTS_0"] =
                json!(self.document.floats(&data, 4, Some(ARRAY_BUFFER), false));
        }
        let mut value = json!({
            "attributes": attributes,
            "indices": self.document.indices(&primitive.indices),
        });
        if let Some(material) = material {
            value["material"] = json!(material);
        }
        value
    }

    fn skin(&mut self, model: &GlbModel, skin: usize, nodes: &[usize]) -> usize {
        let skin = &model.skins[skin];
        let matrices: Vec<f32> = skin
            .inverse_binds
            .iter()
            .flat_map(|matrix| matrix.to_cols_array())
            .collect();
        let inverse = self.document.floats(&matrices, 16, None, false);
        self.document.skins.push(json!({
            "joints": skin.joints.iter().map(|joint| nodes[*joint]).collect::<Vec<_>>(),
            "inverseBindMatrices": inverse,
        }));
        self.document.skins.len() - 1
    }

    fn embedded_material(&mut self, model: &GlbModel, asset: &str, source: usize) -> usize {
        let material = &model.materials[source];
        let mut value = json!({
            "pbrMetallicRoughness": {
                "baseColorFactor": material.base_color,
                "metallicFactor": material.metallic,
                "roughnessFactor": material.roughness,
            },
            "emissiveFactor": material.emissive,
            "doubleSided": material.double_sided,
        });
        match material.alpha {
            GlbAlpha::Opaque => {}
            GlbAlpha::Mask(cutoff) => {
                value["alphaMode"] = json!("MASK");
                value["alphaCutoff"] = json!(cutoff);
            }
            GlbAlpha::Blend => value["alphaMode"] = json!("BLEND"),
        }
        if material.unlit {
            value["extensions"] = json!({ "KHR_materials_unlit": {} });
            self.document.extensions.insert("KHR_materials_unlit");
        }
        if let Some(texture) = material.base_color_texture {
            let key = format!("{asset}#texture/{texture}");
            let index = match self.textures.get(&key) {
                Some(index) => Some(*index),
                None => self
                    .embedded_texture(&model.textures[texture])
                    .inspect(|index| {
                        self.textures.insert(key, *index);
                    }),
            };
            if let Some(index) = index {
                value["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": index });
            }
        }
        self.document.material(value)
    }

    fn embedded_texture(&mut self, texture: &GlbTexture) -> Option<usize> {
        let image = texture.image.as_ref()?;
        let png = encode_png(image.width, image.height, &image.rgba).ok()?;
        Some(self.texture(&png, texture.nearest, texture.repeat))
    }

    fn animation(&mut self, id: &str, clip: &GlbClip, nodes: &[usize]) {
        let mut samplers = Vec::new();
        let mut channels = Vec::new();
        for channel in &clip.channels {
            let (path, width) = match channel.path {
                Path::Translation => ("translation", 3),
                Path::Rotation => ("rotation", 4),
                Path::Scale => ("scale", 3),
            };
            let input = self.document.floats(&channel.times, 1, None, true);
            let output = self.document.floats(&channel.values, width, None, false);
            samplers.push(json!({
                "input": input,
                "output": output,
                "interpolation": match channel.interpolation {
                    Interp::Step => "STEP",
                    Interp::Linear => "LINEAR",
                    Interp::CubicSpline => "CUBICSPLINE",
                },
            }));
            channels.push(json!({
                "sampler": samplers.len() - 1,
                "target": { "node": nodes[channel.node], "path": path },
            }));
        }
        if !channels.is_empty() {
            self.document.animations.push(json!({
                "name": id,
                "channels": channels,
                "samplers": samplers,
            }));
        }
    }

    // ── Lights ──────────────────────────────────────────────────────────────

    /// `KHR_lights_punctual` point, spot and directional lights: a child node
    /// placed at the light and aimed down its direction.
    fn light(
        &mut self,
        index: usize,
        handle: RenderHandle,
        light: &LightDescriptor,
    ) -> Result<(), String> {
        let (kind, color, intensity, enabled, position, direction, range, spot) = match *light {
            LightDescriptor::Ambient { .. } => {
                return Err(format!(
                    "exportGlb: ambient light {} is not representable in glTF",
                    handle.raw()
                ))
            }
            LightDescriptor::Directional {
                color,
                intensity,
                enabled,
                direction,
                ..
            } => (
                "directional",
                color,
                intensity,
                enabled,
                [0.0; 3],
                Some(direction),
                None,
                None,
            ),
            LightDescriptor::Point {
                color,
                intensity,
                enabled,
                position,
                range,
                ..
            } => (
                "point", color, intensity, enabled, position, None, range, None,
            ),
            LightDescriptor::Spot {
                color,
                intensity,
                enabled,
                position,
                direction,
                range,
                outer_angle_radians,
                penumbra,
                ..
            } => (
                "spot",
                color,
                intensity,
                enabled,
                position,
                Some(direction),
                range,
                Some((outer_angle_radians * (1.0 - penumbra), outer_angle_radians)),
            ),
        };
        let mut value = json!({
            "type": kind,
            "color": color,
            "intensity": if enabled { intensity } else { 0.0 },
        });
        if let Some(range) = range.filter(|range| *range > 0.0) {
            value["range"] = json!(range);
        }
        if let Some((inner, outer)) = spot {
            value["spot"] = json!({ "innerConeAngle": inner, "outerConeAngle": outer });
        }
        self.document.lights.push(value);
        let rotation = direction.map_or(Quat::IDENTITY, |direction| {
            Quat::from_rotation_arc(
                Vec3::NEG_Z,
                crate::convert::vec3(direction).normalize_or(Vec3::NEG_Z),
            )
        });
        let light_node = self.document.node(json!({
            "name": format!("light {}", handle.raw()),
            "translation": position,
            "rotation": rotation.to_array(),
            "extensions": { "KHR_lights_punctual": { "light": self.document.lights.len() - 1 } },
        }));
        self.document.children[index].push(light_node);
        Ok(())
    }
}

fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]]
}
