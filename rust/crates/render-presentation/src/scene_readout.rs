//! Readings of the committed scene for inspection (`engine.scene.*`): each
//! retained node with its parent, layer, transforms and bounds, read from
//! the world as committed. Nothing here is kept up to date: a reading
//! composes what it reports when it is asked.

use glam::{Mat4, Quat, Vec3};
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
    pub child_handles: Vec<u64>,
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

    /// `handle`'s world matrix, composed through its parents.
    fn composed(&self, handle: RenderHandle) -> Option<Mat4> {
        let mut world = Mat4::IDENTITY;
        let mut cursor = Some(handle);
        // Parents are admitted acyclic; the bound only stops a corrupt chain.
        for _ in 0..=self.retained.nodes.len() {
            let Some(handle) = cursor else {
                return Some(world);
            };
            let node = self.retained.nodes.get(&handle)?;
            if let Some(transform) = node.kind.transform() {
                world = matrix(transform) * world;
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

    fn local_bounds(&self, node: &PresentationNode) -> Option<([f32; 3], [f32; 3])> {
        const HALF: f32 = 0.5;
        match &node.kind {
            NodeKind::Primitive(primitive) => match primitive.geometry {
                Geometry::Group | Geometry::Point => None,
                Geometry::Cube | Geometry::Sphere => Some(([-HALF; 3], [HALF; 3])),
                Geometry::Quad => Some(([-HALF, -HALF, 0.0], [HALF, HALF, 0.0])),
                Geometry::Line { a, b } => Some((
                    std::array::from_fn(|axis| a[axis].min(b[axis])),
                    std::array::from_fn(|axis| a[axis].max(b[axis])),
                )),
            }
            .or_else(|| {
                node.mesh_payload
                    .as_ref()
                    .map(|payload| (payload.bounds.min, payload.bounds.max))
            }),
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

fn matrix(transform: &Transform) -> Mat4 {
    let [x, y, z, w] = transform.rotation;
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        Quat::from_xyzw(x, y, z, w).normalize(),
        Vec3::from_array(transform.translation),
    )
}

fn transform_of(matrix: Mat4) -> Transform {
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    Transform {
        translation: translation.to_array(),
        rotation: rotation.normalize().to_array(),
        scale: scale.to_array(),
    }
}

/// The world box around a local box's eight corners.
fn world_box(world: Mat4, min: [f32; 3], max: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let (mut low, mut high) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for corner in 0..8 {
        let pick = |axis: usize| {
            if corner & (1 << axis) == 0 {
                min[axis]
            } else {
                max[axis]
            }
        };
        let point = world.transform_point3(Vec3::new(pick(0), pick(1), pick(2)));
        low = low.min(point);
        high = high.max(point);
    }
    (low.to_array(), high.to_array())
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
