//! Readings of the committed scene for inspection (`engine.scene.*`): each
//! retained node with its parent, layer, transforms and bounds, read from
//! the world as committed. Nothing here is kept up to date: a reading
//! composes what it reports when it is asked.

use serde::Serialize;

use super::{NodeKind, PresentationNode, PresentationWorld, Resource};
use render_model::*;

/// One committed node, as listed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneObject<'a> {
    pub handle: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
    /// The parent's rig joint the node hangs from. Its world transform then
    /// leaves out the joint's evaluated pose, which only the renderer has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_joint: Option<&'a str>,
    pub kind: &'static str,
    /// The layer it draws in: its root's, for a node that has a parent.
    pub layer: RenderLayer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<&'a str>,
    /// The asset it draws.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<Transform>,
    /// Composed through its parents.
    pub world: Transform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<SceneBounds>,
    pub children: usize,
    /// The world revision that last changed it.
    pub changed_revision: u64,
}

/// Bounds of what a node draws: in its own space, and the world box around
/// them at its world transform. An animated mesh's are its asset's rest
/// bounds, not its pose's.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneBounds {
    pub local_min: [f32; 3],
    pub local_max: [f32; 3],
    pub world_min: [f32; 3],
    pub world_max: [f32; 3],
}

/// One committed node in full: its listing and the description it was
/// committed with.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneObjectDetail<'a> {
    #[serde(flatten)]
    pub object: SceneObject<'a>,
    pub description: SceneObjectDescription<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material_override: Option<&'a Material>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub material_parameters: &'a std::collections::BTreeMap<u16, MaterialInstanceParameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub morph_focus: Option<[f64; 2]>,
    /// The pose controls in force on an animated mesh.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pose: Option<&'a AnimatedMeshPose>,
    /// A mesh's material slots as committed: the asset's, with the
    /// instance's overrides over them, each resolved to its retained
    /// material, its shader features and any product shader.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<SceneMaterialBinding<'a>>,
    pub child_handles: Vec<u64>,
}

/// One material slot of a mesh as committed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneMaterialBinding<'a> {
    pub slot: u16,
    pub material: std::borrow::Cow<'a, str>,
    /// The instance overrides the asset's material in this slot.
    pub overridden: bool,
    /// The retained material; `None` while it is not defined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<&'a RenderMaterialDescriptor>,
    /// The standard shader features the material turns on, by keyword.
    pub features: Vec<&'static str>,
    /// The product shader that shades it in place of the standard one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shader: Option<SceneShader<'a>>,
    /// The terrain layers of the material a payload chunk draws, narrowed
    /// to them in this order (the payload's `layer_palette`); empty for
    /// the material's own layers.
    #[serde(skip_serializing_if = "<[u8]>::is_empty")]
    pub layer_palette: &'a [u8],
}

/// A product shader as committed: its identity and keywords.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneShader<'a> {
    pub id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<&'a str>,
    pub keywords: &'a [String],
    /// The shader is not defined in the committed scene.
    pub missing: bool,
}

/// The descriptor a node was committed with.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SceneObjectDescription<'a> {
    Primitive(&'a RenderNode),
    StaticMesh(&'a StaticMeshInstanceDescriptor),
    AnimatedMesh(&'a AnimatedMeshInstanceDescriptor),
    VoxelObject(&'a VoxelObjectInstanceDescriptor),
    Sprite(&'a SpriteInstanceDescriptor),
    SpriteBatch(&'a SpriteBatchDescriptor),
    Light(&'a LightDescriptor),
    /// A scatter patch's copies are not listed; how many there are is.
    Scatter {
        copies: usize,
    },
}

/// One committed resource descriptor: identity and kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneResource<'a> {
    pub id: &'a str,
    pub kind: &'static str,
    /// Nodes, resources and captures that name it.
    pub users: u32,
}

impl PresentationWorld {
    /// The committed root nodes, in handle order.
    pub fn scene_roots(&self) -> impl Iterator<Item = RenderHandle> + '_ {
        self.retained
            .nodes
            .iter()
            .filter(|(_, node)| node.parent.is_none())
            .map(|(handle, _)| *handle)
    }

    /// A committed node's children, in handle order.
    pub fn scene_children(&self, handle: RenderHandle) -> impl Iterator<Item = RenderHandle> + '_ {
        self.retained
            .children
            .get(&handle)
            .into_iter()
            .flatten()
            .copied()
    }

    /// One committed node, as listed; `None` when there is none.
    pub fn scene_object(&self, handle: RenderHandle) -> Option<SceneObject<'_>> {
        let node = self.retained.nodes.get(&handle)?;
        let world = self.composed(handle)?;
        let metadata = node.kind.metadata();
        let bounds = self.local_bounds(node).map(|(min, max)| {
            let (world_min, world_max) = world_box(world, min, max);
            SceneBounds {
                local_min: min,
                local_max: max,
                world_min,
                world_max,
            }
        });
        Some(SceneObject {
            handle: handle.raw(),
            parent: node.parent.map(RenderHandle::raw),
            parent_joint: node.parent_joint.as_deref(),
            kind: kind_name(&node.kind),
            layer: self.scene_layer(handle),
            visible: visible(&node.kind),
            entity: metadata.and_then(|metadata| metadata.source_entity),
            label: metadata.and_then(|metadata| metadata.label.as_deref()),
            resource: resource(&node.kind),
            local: node.kind.transform().copied(),
            world: transform_of(world),
            bounds,
            children: self.retained.children.get(&handle).map_or(0, |c| c.len()),
            changed_revision: node.changed,
        })
    }

    /// One committed node in full; `None` when there is none.
    pub fn scene_object_detail(&self, handle: RenderHandle) -> Option<SceneObjectDetail<'_>> {
        let object = self.scene_object(handle)?;
        let node = self.retained.nodes.get(&handle)?;
        Some(SceneObjectDetail {
            object,
            description: match &node.kind {
                NodeKind::Primitive(value) => SceneObjectDescription::Primitive(value),
                NodeKind::StaticMesh(value) => SceneObjectDescription::StaticMesh(value),
                NodeKind::AnimatedMesh(value) => SceneObjectDescription::AnimatedMesh(value),
                NodeKind::VoxelObject(value) => SceneObjectDescription::VoxelObject(value),
                NodeKind::Sprite(value) => SceneObjectDescription::Sprite(value),
                NodeKind::SpriteBatch(value) => SceneObjectDescription::SpriteBatch(value),
                NodeKind::Light(value) => SceneObjectDescription::Light(value),
                NodeKind::Scatter(value) => SceneObjectDescription::Scatter {
                    copies: value.instances.len(),
                },
            },
            material_override: node.material_override.as_ref(),
            material_parameters: &node.material_parameters,
            morph_focus: node.morph_focus,
            materials: self.material_bindings(node),
            pose: matches!(node.kind, NodeKind::AnimatedMesh(_))
                .then_some(&node.pose)
                .filter(|pose| **pose != AnimatedMeshPose::default()),
            child_handles: self.scene_children(handle).map(RenderHandle::raw).collect(),
        })
    }

    /// The committed resource descriptors, by kind then identity.
    pub fn scene_resources(&self) -> Vec<SceneResource<'_>> {
        fn list<'a>(
            resources: &mut Vec<SceneResource<'a>>,
            kind: &'static str,
            users: &std::collections::BTreeMap<String, u32>,
            ids: impl Iterator<Item = &'a String>,
        ) {
            resources.extend(ids.map(|id| SceneResource {
                id,
                kind,
                users: users.get(id).copied().unwrap_or(0),
            }));
        }
        let retained = &*self.retained;
        let users = |kind: Resource| &retained.users[kind as usize];
        let mut resources = Vec::new();
        list(
            &mut resources,
            "texture",
            users(Resource::Texture),
            retained.textures.keys(),
        );
        list(
            &mut resources,
            "shader",
            users(Resource::Shader),
            retained.shaders.keys(),
        );
        list(
            &mut resources,
            "material",
            users(Resource::Material),
            retained.materials.keys(),
        );
        list(
            &mut resources,
            "spriteAtlas",
            users(Resource::Atlas),
            retained.atlases.keys(),
        );
        list(
            &mut resources,
            "staticMesh",
            users(Resource::StaticMesh),
            retained.static_meshes.keys(),
        );
        list(
            &mut resources,
            "animatedMesh",
            users(Resource::AnimatedMesh),
            retained.animated_meshes.keys(),
        );
        list(
            &mut resources,
            "voxelObject",
            users(Resource::VoxelObject),
            retained.voxel_objects.keys(),
        );
        resources
    }

    /// Where a committed node's origin is in the world, composed through its
    /// parents: the one composition `entity_world_position` and the readings
    /// share.
    pub(super) fn scene_world_translation(&self, handle: RenderHandle) -> Option<[f32; 3]> {
        let world = self.composed(handle)?;
        Some([world[3][0], world[3][1], world[3][2]])
    }

    /// `handle`'s world matrix, composed through its parents.
    fn composed(&self, handle: RenderHandle) -> Option<Mat4> {
        let mut world = IDENTITY;
        let mut cursor = Some(handle);
        // Parents are admitted acyclic; the bound only stops a corrupt chain.
        for _ in 0..=self.retained.nodes.len() {
            let Some(handle) = cursor else {
                return Some(world);
            };
            let node = self.retained.nodes.get(&handle)?;
            if let Some(transform) = node.kind.transform() {
                world = multiply(&matrix(transform), &world);
            }
            cursor = node.parent;
        }
        None
    }

    /// The layer a committed node draws in: its root's.
    pub fn scene_layer(&self, handle: RenderHandle) -> RenderLayer {
        let mut cursor = handle;
        for _ in 0..=self.retained.nodes.len() {
            let Some(node) = self.retained.nodes.get(&cursor) else {
                break;
            };
            match node.parent {
                Some(parent) => cursor = parent,
                None => return layer(&node.kind),
            }
        }
        RenderLayer::Scene
    }

    /// A mesh node's slots: its asset's materials with the instance's
    /// overrides over them, or a payload's groups' slot materials.
    fn material_bindings<'a>(
        &'a self,
        node: &'a PresentationNode,
    ) -> Vec<SceneMaterialBinding<'a>> {
        use std::borrow::Cow;
        let retained = &*self.retained;
        let (asset_slots, overrides): (&[MeshMaterialSlot], &[MeshMaterialSlot]) = match &node.kind
        {
            NodeKind::StaticMesh(instance) => (
                retained
                    .static_meshes
                    .get(&instance.asset)
                    .map_or(&[], |asset| &asset.material_slots[..]),
                &instance.material_overrides,
            ),
            NodeKind::AnimatedMesh(instance) => (
                retained
                    .animated_meshes
                    .get(&instance.asset)
                    .map_or(&[], |asset| &asset.material_slots[..]),
                &instance.material_overrides,
            ),
            NodeKind::VoxelObject(instance) => (
                retained
                    .voxel_objects
                    .get(&instance.asset)
                    .map_or(&[], |asset| &asset.material_slots[..]),
                &instance.material_overrides,
            ),
            NodeKind::Scatter(patch) => (
                retained
                    .static_meshes
                    .get(&patch.asset)
                    .map_or(&[], |asset| &asset.material_slots[..]),
                &patch.material_overrides,
            ),
            NodeKind::Primitive(_) => {
                let Some(payload) = &node.mesh_payload else {
                    return Vec::new();
                };
                let slots: std::collections::BTreeSet<u16> = payload
                    .groups
                    .iter()
                    .map(|group| group.material_slot)
                    .collect();
                return slots
                    .into_iter()
                    .map(|slot| {
                        let mut binding = self.material_binding(
                            slot,
                            Cow::Owned(format!("{PAYLOAD_SLOT_MATERIAL_PREFIX}{slot}")),
                            false,
                        );
                        binding.layer_palette = &payload.layer_palette;
                        binding
                    })
                    .collect();
            }
            _ => return Vec::new(),
        };
        let mut slots: std::collections::BTreeMap<u16, (&str, bool)> = asset_slots
            .iter()
            .map(|slot| (slot.slot, (slot.material.as_str(), false)))
            .collect();
        for slot in overrides {
            slots.insert(slot.slot, (slot.material.as_str(), true));
        }
        slots
            .into_iter()
            .map(|(slot, (material, overridden))| {
                self.material_binding(slot, Cow::Borrowed(material), overridden)
            })
            .collect()
    }

    /// One slot bound to `material`, with its retained descriptor and
    /// shader.
    fn material_binding<'a>(
        &'a self,
        slot: u16,
        material: std::borrow::Cow<'a, str>,
        overridden: bool,
    ) -> SceneMaterialBinding<'a> {
        let retained = &*self.retained;
        let descriptor = retained
            .materials
            .get(material.as_ref())
            .map(|material| material.as_ref());
        SceneMaterialBinding {
            slot,
            material,
            overridden,
            descriptor,
            features: descriptor.map(material_features).unwrap_or_default(),
            shader: descriptor
                .and_then(|material| material.shader.as_ref())
                .map(|shader| {
                    let defined = retained.shaders.get(&shader.shader);
                    SceneShader {
                        id: &shader.shader,
                        path: defined.map(|defined| defined.path.as_str()),
                        keywords: defined.map_or(&[], |defined| &defined.keywords[..]),
                        missing: defined.is_none(),
                    }
                }),
            layer_palette: &[],
        }
    }

    fn local_bounds(&self, node: &PresentationNode) -> Option<([f32; 3], [f32; 3])> {
        const HALF: f32 = 0.5;
        match &node.kind {
            // A replaced payload is what the node draws.
            NodeKind::Primitive(_) | NodeKind::StaticMesh(_) if node.mesh_payload.is_some() => node
                .mesh_payload
                .as_ref()
                .map(|payload| (payload.bounds.min, payload.bounds.max)),
            NodeKind::Primitive(primitive) => match primitive.geometry {
                Geometry::Group | Geometry::Point => None,
                Geometry::Cube | Geometry::Sphere => Some(([-HALF; 3], [HALF; 3])),
                Geometry::Quad => Some(([-HALF, -HALF, 0.0], [HALF, HALF, 0.0])),
                Geometry::Line { a, b } => Some((
                    std::array::from_fn(|axis| a[axis].min(b[axis])),
                    std::array::from_fn(|axis| a[axis].max(b[axis])),
                )),
            },
            NodeKind::StaticMesh(instance) => self
                .retained
                .static_meshes
                .get(&instance.asset)
                .map(|asset| (asset.payload.bounds.min, asset.payload.bounds.max)),
            NodeKind::AnimatedMesh(instance) => self
                .retained
                .animated_meshes
                .get(&instance.asset)
                .map(|asset| (asset.bounds.min, asset.bounds.max)),
            _ => None,
        }
    }
}

/// The standard shader features a material turns on, by the keywords the
/// standard shader names them with.
fn material_features(material: &RenderMaterialDescriptor) -> Vec<&'static str> {
    [
        (material.unlit, "UNLIT"),
        (material.flat_shading, "FLAT_SHADING"),
        (material.keep_dry, "KEEP_DRY"),
        (material.sinks_under_snow, "SINKS_UNDER_SNOW"),
        (material.wind.is_some(), "WIND"),
        (material.water.is_some(), "WATER"),
        (material.normal_map.is_some(), "NORMAL_MAP"),
        (material.emission_map.is_some(), "EMISSION_MAP"),
        (material.occlusion_map.is_some(), "OCCLUSION_MAP"),
        (material.triplanar.is_some(), "TRIPLANAR"),
        (material.stochastic_tiling.is_some(), "STOCHASTIC_TILING"),
        (material.terrain_layers.is_some(), "TERRAIN_LAYERS"),
        (material.voxel_surface.is_some(), "VOXEL_SURFACE"),
        (material.double_sided, "DOUBLE_SIDED"),
        (
            material.alpha_mode != MaterialAlphaModeDescriptor::Opaque,
            "ALPHA",
        ),
        (material.translucent_shadow, "TRANSLUCENT_SHADOW"),
        (material.shader.is_some(), "PRODUCT_SHADER"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect()
}

fn kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Primitive(_) => "primitive",
        NodeKind::StaticMesh(_) => "staticMesh",
        NodeKind::AnimatedMesh(_) => "animatedMesh",
        NodeKind::VoxelObject(_) => "voxelObject",
        NodeKind::Sprite(_) => "sprite",
        NodeKind::SpriteBatch(_) => "spriteBatch",
        NodeKind::Light(_) => "light",
        NodeKind::Scatter(_) => "scatter",
    }
}

fn layer(kind: &NodeKind) -> RenderLayer {
    match kind {
        NodeKind::Primitive(node) => node.layer,
        NodeKind::StaticMesh(instance) => instance.layer,
        NodeKind::AnimatedMesh(instance) => instance.layer,
        NodeKind::Sprite(sprite) => sprite.layer,
        NodeKind::SpriteBatch(batch) => batch.sprite.layer,
        NodeKind::VoxelObject(_) | NodeKind::Light(_) | NodeKind::Scatter(_) => RenderLayer::Scene,
    }
}

fn visible(kind: &NodeKind) -> Option<bool> {
    match kind {
        NodeKind::Primitive(node) => Some(node.visible),
        NodeKind::StaticMesh(instance) => Some(instance.visible),
        NodeKind::AnimatedMesh(instance) => Some(instance.visible),
        NodeKind::VoxelObject(instance) => Some(instance.visible),
        NodeKind::Sprite(sprite) => Some(sprite.visible),
        NodeKind::SpriteBatch(batch) => Some(batch.sprite.visible),
        NodeKind::Light(_) | NodeKind::Scatter(_) => None,
    }
}

fn resource(kind: &NodeKind) -> Option<&str> {
    match kind {
        NodeKind::StaticMesh(instance) => Some(&instance.asset),
        NodeKind::AnimatedMesh(instance) => Some(&instance.asset),
        NodeKind::VoxelObject(instance) => Some(&instance.asset),
        NodeKind::Sprite(sprite) => Some(&sprite.asset),
        NodeKind::SpriteBatch(batch) => Some(&batch.sprite.asset),
        NodeKind::Scatter(patch) => Some(&patch.asset),
        NodeKind::Primitive(_) | NodeKind::Light(_) => None,
    }
}

/// A 4x4 matrix by columns. The readings compose a few transforms per
/// object on request; the renderer's matrix library stays the renderer's.
type Mat4 = [[f32; 4]; 4];

const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// Scale, then rotate (`[x, y, z, w]`, normalized), then translate.
fn matrix(transform: &Transform) -> Mat4 {
    let [x, y, z, w] = transform.rotation;
    let length = (x * x + y * y + z * z + w * w).sqrt();
    let (x, y, z, w) = if length > 0.0 {
        (x / length, y / length, z / length, w / length)
    } else {
        (0.0, 0.0, 0.0, 1.0)
    };
    let rotation = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
        ],
        [
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
        ],
        [
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    let mut out = IDENTITY;
    for (column, axis) in rotation.iter().enumerate() {
        for row in 0..3 {
            out[column][row] = axis[row] * transform.scale[column];
        }
    }
    out[3][..3].copy_from_slice(&transform.translation);
    out
}

fn multiply(a: &Mat4, b: &Mat4) -> Mat4 {
    std::array::from_fn(|column| {
        std::array::from_fn(|row| (0..4).map(|k| a[k][row] * b[column][k]).sum())
    })
}

fn transform_point(m: &Mat4, point: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|row| {
        m[0][row] * point[0] + m[1][row] * point[1] + m[2][row] * point[2] + m[3][row]
    })
}

/// The translation, rotation and scale of an affine matrix; a mirroring
/// matrix gives a negative x scale.
fn transform_of(m: Mat4) -> Transform {
    let length = |column: usize| {
        (0..3)
            .map(|row| m[column][row] * m[column][row])
            .sum::<f32>()
            .sqrt()
    };
    let determinant = m[0][0] * (m[1][1] * m[2][2] - m[2][1] * m[1][2])
        - m[1][0] * (m[0][1] * m[2][2] - m[2][1] * m[0][2])
        + m[2][0] * (m[0][1] * m[1][2] - m[1][1] * m[0][2]);
    let mut scale = [length(0), length(1), length(2)];
    if determinant < 0.0 {
        scale[0] = -scale[0];
    }
    // The rotation's columns, unscaled.
    let r = |column: usize, row: usize| {
        if scale[column] == 0.0 {
            0.0
        } else {
            m[column][row] / scale[column]
        }
    };
    let trace = r(0, 0) + r(1, 1) + r(2, 2);
    let (x, y, z, w) = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        (
            (r(1, 2) - r(2, 1)) / s,
            (r(2, 0) - r(0, 2)) / s,
            (r(0, 1) - r(1, 0)) / s,
            s / 4.0,
        )
    } else if r(0, 0) > r(1, 1) && r(0, 0) > r(2, 2) {
        let s = (1.0 + r(0, 0) - r(1, 1) - r(2, 2)).sqrt() * 2.0;
        (
            s / 4.0,
            (r(1, 0) + r(0, 1)) / s,
            (r(2, 0) + r(0, 2)) / s,
            (r(1, 2) - r(2, 1)) / s,
        )
    } else if r(1, 1) > r(2, 2) {
        let s = (1.0 + r(1, 1) - r(0, 0) - r(2, 2)).sqrt() * 2.0;
        (
            (r(1, 0) + r(0, 1)) / s,
            s / 4.0,
            (r(2, 1) + r(1, 2)) / s,
            (r(2, 0) - r(0, 2)) / s,
        )
    } else {
        let s = (1.0 + r(2, 2) - r(0, 0) - r(1, 1)).sqrt() * 2.0;
        (
            (r(2, 0) + r(0, 2)) / s,
            (r(2, 1) + r(1, 2)) / s,
            s / 4.0,
            (r(0, 1) - r(1, 0)) / s,
        )
    };
    Transform {
        translation: [m[3][0], m[3][1], m[3][2]],
        rotation: [x, y, z, w],
        scale,
    }
}

/// The world box around a local box's eight corners.
fn world_box(world: Mat4, min: [f32; 3], max: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for corner in 0..8 {
        let pick = |axis: usize| {
            if corner & (1 << axis) == 0 {
                min[axis]
            } else {
                max[axis]
            }
        };
        let point = transform_point(&world, [pick(0), pick(1), pick(2)]);
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    (low, high)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(
        handle: u64,
        parent: Option<u64>,
        geometry: Geometry,
        transform: Transform,
    ) -> RenderDiff {
        let mut node = RenderNode::new(geometry);
        node.transform = transform;
        if parent.is_none() {
            node.layer = RenderLayer::Viewmodel;
        }
        RenderDiff::Create {
            handle: RenderHandle::new(handle),
            parent: parent.map(RenderHandle::new),
            node,
        }
    }

    fn moved(translation: [f32; 3], scale: f32) -> Transform {
        Transform {
            translation,
            scale: [scale; 3],
            ..Transform::IDENTITY
        }
    }

    #[test]
    fn an_entity_position_and_a_scene_reading_compose_alike() {
        let mut world = PresentationWorld::default();
        let mut parent = RenderNode::new(Geometry::Group);
        // Not a unit quaternion: both readings normalize it, as the renderer does.
        parent.transform.rotation = [0.0, 0.0, 2.0, 0.0];
        let mut child = RenderNode::new(Geometry::Cube);
        child.transform.translation = [1.0, 0.0, 0.0];
        child.metadata.source_entity = Some(5);
        world
            .apply(RenderFrameDiff::from_ops(vec![
                RenderDiff::Create {
                    handle: RenderHandle::new(1),
                    parent: None,
                    node: parent,
                },
                RenderDiff::Create {
                    handle: RenderHandle::new(2),
                    parent: Some(RenderHandle::new(1)),
                    node: child,
                },
            ]))
            .unwrap();
        let read = world
            .scene_object(RenderHandle::new(2))
            .unwrap()
            .world
            .translation;
        let entity = world.entity_world_position(5).unwrap();
        assert_eq!(read, entity);
        assert!(
            (entity[0] + 1.0).abs() < 1e-6 && entity[1].abs() < 1e-6,
            "{entity:?}"
        );
    }

    #[test]
    fn voxel_objects_and_payload_chunks_read_the_materials_they_draw() {
        let slot = |slot: u16, material: &str| MeshMaterialSlot {
            slot,
            material: material.to_owned(),
        };
        let mut payload = super::super::tests::static_mesh("mesh/chunk").payload;
        payload.groups = vec![
            MeshGroupDescriptor {
                material_slot: 1,
                start: 0,
                count: 3,
            },
            MeshGroupDescriptor {
                material_slot: 4,
                start: 3,
                count: 3,
            },
        ];
        payload.layer_palette = vec![2, 5];
        let voxel =
            |handle: u64, overrides: Vec<MeshMaterialSlot>| RenderDiff::CreateVoxelObjectInstance {
                handle: RenderHandle::new(handle),
                parent: None,
                instance: VoxelObjectInstanceDescriptor {
                    asset: "voxel-object/barrel".to_owned(),
                    frame: 0,
                    transform: Transform::IDENTITY,
                    visible: true,
                    material_overrides: overrides,
                    metadata: RenderMetadata::default(),
                },
            };
        let mut world = PresentationWorld::default();
        world
            .apply(RenderFrameDiff::from_ops(vec![
                RenderDiff::DefineMaterial {
                    material: material("material/wood", serde_json::json!({ "unlit": true })),
                },
                RenderDiff::DefineMaterial {
                    material: material("material/iron", serde_json::json!({})),
                },
                RenderDiff::DefineMaterial {
                    material: material(
                        "voxel-material/1",
                        serde_json::json!({ "flatShading": true }),
                    ),
                },
                RenderDiff::DefineVoxelObject {
                    asset: VoxelObjectRenderAsset {
                        asset: "voxel-object/barrel".to_owned(),
                        content_hash: "hash".to_owned(),
                        meshes: vec![VoxelObjectRenderMesh {
                            payload: payload.clone(),
                        }],
                        frames: vec![VoxelObjectRenderFrame {
                            id: "idle".to_owned(),
                            mesh: 0,
                        }],
                        material_slots: vec![slot(0, "material/wood"), slot(1, "material/wood")],
                    },
                },
                voxel(1, Vec::new()),
                voxel(2, vec![slot(1, "material/iron")]),
                create(3, None, Geometry::Cube, Transform::IDENTITY),
                RenderDiff::ReplaceMeshPayload {
                    handle: RenderHandle::new(3),
                    payload,
                },
            ]))
            .unwrap();
        /// Slot, material, overridden, features and layer palette.
        type Read = (u16, String, bool, Vec<&'static str>, Vec<u8>);
        let read = |handle: u64| -> Vec<Read> {
            let detail = world
                .scene_object_detail(RenderHandle::new(handle))
                .unwrap();
            detail
                .materials
                .iter()
                .map(|binding| {
                    (
                        binding.slot,
                        binding.material.to_string(),
                        binding.overridden,
                        binding.features.clone(),
                        binding.layer_palette.to_vec(),
                    )
                })
                .collect()
        };
        // The asset's slots, untouched.
        assert_eq!(
            read(1),
            [
                (0, "material/wood".to_owned(), false, vec!["UNLIT"], vec![]),
                (1, "material/wood".to_owned(), false, vec!["UNLIT"], vec![]),
            ]
        );
        // One slot overridden, the other the asset's.
        assert_eq!(
            read(2),
            [
                (0, "material/wood".to_owned(), false, vec!["UNLIT"], vec![]),
                (1, "material/iron".to_owned(), true, vec![], vec![]),
            ]
        );
        // A payload's groups draw their slot materials, narrowed to its
        // palette; slot 4 has none defined.
        assert_eq!(
            read(3),
            [
                (
                    1,
                    "voxel-material/1".to_owned(),
                    false,
                    vec!["FLAT_SHADING"],
                    vec![2, 5]
                ),
                (4, "voxel-material/4".to_owned(), false, vec![], vec![2, 5]),
            ]
        );
        let chunk = world.scene_object_detail(RenderHandle::new(3)).unwrap();
        assert!(chunk.materials[0].descriptor.is_some() && chunk.materials[1].descriptor.is_none());
    }

    #[test]
    fn a_replaced_payload_bounds_the_primitive_it_replaced() {
        let mut world = PresentationWorld::default();
        let mut payload = super::super::tests::static_mesh("mesh/payload").payload;
        payload.bounds = MeshBoundsDescriptor {
            min: [0.0; 3],
            max: [10.0, 20.0, 30.0],
        };
        world
            .apply(RenderFrameDiff::from_ops(vec![
                create(1, None, Geometry::Cube, Transform::IDENTITY),
                RenderDiff::ReplaceMeshPayload {
                    handle: RenderHandle::new(1),
                    payload,
                },
            ]))
            .unwrap();
        let bounds = world
            .scene_object(RenderHandle::new(1))
            .unwrap()
            .bounds
            .unwrap();
        assert_eq!(
            (bounds.local_min, bounds.local_max),
            ([0.0; 3], [10.0, 20.0, 30.0])
        );
    }

    /// A plain material `id` with `extra` descriptor fields.
    fn material(id: &str, extra: serde_json::Value) -> RenderMaterialDescriptor {
        let mut value = serde_json::json!({
            "id": id,
            "color": [1.0, 1.0, 1.0, 1.0],
            "texture": null,
            "roughness": 0.5,
            "textureTint": [1.0, 1.0, 1.0, 1.0],
            "emissionColor": [0.0, 0.0, 0.0],
            "emissionIntensity": 0.0,
            "uvStrategy": "flat",
        });
        value
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn an_object_detail_resolves_its_material_slots_features_and_shader() {
        let mut asset = super::super::tests::static_mesh("mesh/crate");
        asset.material_slots = vec![
            MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_owned(),
            },
            MeshMaterialSlot {
                slot: 1,
                material: "material/plain".to_owned(),
            },
        ];
        let mut world = PresentationWorld::default();
        world
            .apply(RenderFrameDiff::from_ops(vec![
                RenderDiff::DefineShader {
                    shader: ShaderDescriptor {
                        id: "shader/dissolve".to_owned(),
                        path: "shaders/dissolve.wgsl".to_owned(),
                        source: "fn x() {}".to_owned(),
                        keywords: vec!["DISSOLVE".to_owned()],
                    },
                },
                RenderDiff::DefineMaterial {
                    material: material("material/plain", serde_json::json!({})),
                },
                RenderDiff::DefineMaterial {
                    material: material(
                        "material/ghost",
                        serde_json::json!({
                            "unlit": true,
                            "alphaMode": { "kind": "blend" },
                            "shader": { "shader": "shader/dissolve", "parameters": [[0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0]] },
                        }),
                    ),
                },
                RenderDiff::DefineStaticMesh { asset },
                RenderDiff::CreateStaticMeshInstance {
                    handle: RenderHandle::new(1),
                    parent: None,
                    instance: StaticMeshInstanceDescriptor {
                        asset: "mesh/crate".to_owned(),
                        transform: Transform::IDENTITY,
                        visible: true,
                        material_overrides: vec![MeshMaterialSlot {
                            slot: 1,
                            material: "material/ghost".to_owned(),
                        }],
                        metadata: RenderMetadata::default(),
                        layer: RenderLayer::Scene,
                        shadow_casting: Default::default(),
                    },
                },
            ]))
            .unwrap();
        let detail = world.scene_object_detail(RenderHandle::new(1)).unwrap();
        let [plain, ghost] = detail.materials.as_slice() else {
            panic!("{:?}", detail.materials)
        };
        assert_eq!(
            (
                plain.slot,
                &*plain.material,
                plain.overridden,
                plain.features.len()
            ),
            (0, "material/plain", false, 0)
        );
        assert_eq!(
            (ghost.slot, &*ghost.material, ghost.overridden),
            (1, "material/ghost", true)
        );
        assert_eq!(ghost.features, ["UNLIT", "ALPHA", "PRODUCT_SHADER"]);
        let shader = ghost.shader.as_ref().unwrap();
        assert_eq!(
            (shader.id, shader.keywords, shader.missing),
            ("shader/dissolve", &["DISSOLVE".to_owned()][..], false)
        );
        let json = serde_json::to_value(&detail).unwrap();
        assert_eq!(json["materials"][1]["descriptor"]["unlit"], true);
    }

    #[test]
    fn a_composed_transform_reads_back_its_rotation_and_scale() {
        for (x, y, z, w) in [
            (0.0, 0.0, 0.0, 1.0),
            (
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
            ),
            (0.6, 0.0, 0.0, -0.8),
            (0.1, 0.2, 0.3, 0.9273618),
        ] {
            let transform = Transform {
                translation: [1.0, -2.0, 3.0],
                rotation: [x, y, z, w],
                scale: [2.0, 0.5, 3.0],
            };
            let read = transform_of(multiply(&IDENTITY, &matrix(&transform)));
            let same = |a: [f32; 4], b: [f32; 4]| {
                let dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
                dot.abs() > 0.99999
            };
            assert!(same(read.rotation, transform.rotation), "{read:?}");
            for axis in 0..3 {
                assert!((read.scale[axis] - transform.scale[axis]).abs() < 1e-5);
                assert!((read.translation[axis] - transform.translation[axis]).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn a_reading_lists_the_tree_with_composed_transforms_bounds_and_change_revisions() {
        let mut world = PresentationWorld::default();
        world
            .apply(RenderFrameDiff::from_ops(vec![
                create(1, None, Geometry::Group, moved([10.0, 0.0, 0.0], 2.0)),
                create(2, Some(1), Geometry::Cube, moved([1.0, 0.0, 0.0], 1.0)),
                create(3, Some(1), Geometry::Sphere, Transform::IDENTITY),
            ]))
            .unwrap();
        assert_eq!(
            world.scene_roots().collect::<Vec<_>>(),
            [RenderHandle::new(1)]
        );
        assert_eq!(
            world
                .scene_children(RenderHandle::new(1))
                .collect::<Vec<_>>(),
            [RenderHandle::new(2), RenderHandle::new(3)]
        );
        let cube = world.scene_object(RenderHandle::new(2)).unwrap();
        assert_eq!(
            (cube.kind, cube.parent, cube.layer),
            ("primitive", Some(1), RenderLayer::Viewmodel)
        );
        assert_eq!(
            cube.world.translation,
            [12.0, 0.0, 0.0],
            "the parent's scale applies"
        );
        let bounds = cube.bounds.unwrap();
        assert_eq!(
            (bounds.world_min, bounds.world_max),
            ([11.0, -1.0, -1.0], [13.0, 1.0, 1.0])
        );
        assert_eq!(cube.changed_revision, 1);
        assert!(world
            .scene_object(RenderHandle::new(1))
            .unwrap()
            .bounds
            .is_none());

        world
            .apply(RenderFrameDiff::from_ops(vec![RenderDiff::Update {
                handle: RenderHandle::new(3),
                transform: Some(moved([0.0, 5.0, 0.0], 1.0)),
                material: None,
                visible: None,
                metadata: None,
            }]))
            .unwrap();
        let sphere = world.scene_object(RenderHandle::new(3)).unwrap();
        assert_eq!(
            (sphere.changed_revision, sphere.world.translation),
            (2, [10.0, 10.0, 0.0])
        );
        assert_eq!(
            world
                .scene_object(RenderHandle::new(2))
                .unwrap()
                .changed_revision,
            1
        );
        let detail = world.scene_object_detail(RenderHandle::new(1)).unwrap();
        assert_eq!(detail.child_handles, [2, 3]);
        assert!(world.scene_object(RenderHandle::new(9)).is_none());
    }
}
