use std::collections::{BTreeMap, BTreeSet};

use render_model::{
    AnimatedMeshAsset, AnimatedMeshInstanceDescriptor, AnimatedMeshPlaybackCommand, Geometry,
    LightDescriptor, Material, MeshMaterialSlot, RenderDiff, RenderFrameDiff, RenderFrameError,
    RenderHandle, RenderMaterialDescriptor, RenderMetadata, RenderNode, SpriteAtlasDescriptor,
    SpriteInstanceDescriptor, StaticMeshAsset, StaticMeshInstanceDescriptor, TextureDescriptor,
    Transform,
};

use crate::{HandleAllocationError, RenderHandleNamespace, StableHandleRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectionMode {
    AuthoredPreview,
    Runtime,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectionAvailability {
    #[default]
    Both,
    AuthoredOnly,
    RuntimeOnly,
}

impl ProjectionAvailability {
    fn includes(self, mode: ProjectionMode) -> bool {
        matches!(self, Self::Both)
            || matches!(
                (self, mode),
                (Self::AuthoredOnly, ProjectionMode::AuthoredPreview)
            )
            || matches!((self, mode), (Self::RuntimeOnly, ProjectionMode::Runtime))
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
// Appearance values are low-volume authored candidates. Keeping the complete
// sprite descriptor inline preserves the existing direct composition API and
// avoids a mandatory heap allocation in every unlit/default sprite.
#[allow(clippy::large_enum_variant)]
pub enum Appearance {
    Primitive {
        geometry: Geometry,
        material: Material,
    },
    StaticMesh {
        asset: String,
        material_overrides: Vec<MeshMaterialSlot>,
    },
    AnimatedMesh {
        asset: String,
        material_overrides: Vec<MeshMaterialSlot>,
        playback: Option<AnimatedMeshPlaybackCommand>,
    },
    Sprite {
        sprite: SpriteInstanceDescriptor,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceNode {
    pub id: u64,
    pub parent: Option<u64>,
    pub transform: Transform,
    pub visible: bool,
    #[serde(default)]
    pub layer: render_model::RenderLayer,
    pub metadata: RenderMetadata,
    pub availability: ProjectionAvailability,
    pub appearance: Appearance,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceLight {
    pub id: u64,
    pub parent: Option<u64>,
    pub availability: ProjectionAvailability,
    pub light: LightDescriptor,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceResources {
    #[serde(default)]
    pub materials: Vec<RenderMaterialDescriptor>,
    #[serde(default)]
    pub textures: Vec<TextureDescriptor>,
    #[serde(default)]
    pub sprite_atlases: Vec<SpriteAtlasDescriptor>,
    #[serde(default)]
    pub static_meshes: Vec<StaticMeshAsset>,
    #[serde(default)]
    pub animated_meshes: Vec<AnimatedMeshAsset>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceScene {
    pub resources: AppearanceResources,
    pub nodes: Vec<AppearanceNode>,
    pub lights: Vec<AppearanceLight>,
}

#[derive(Debug, Clone, PartialEq)]
struct ProjectedNode {
    parent: Option<u64>,
    transform: Transform,
    visible: bool,
    layer: render_model::RenderLayer,
    metadata: RenderMetadata,
    appearance: Appearance,
}

#[derive(Debug, Clone, PartialEq)]
struct ProjectedLight {
    parent: Option<u64>,
    light: LightDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum AuthoredRenderKey {
    Node(u64),
    Light(u64),
}

#[derive(Debug, Clone, Default, PartialEq)]
struct ResourceSnapshot {
    materials: BTreeMap<String, RenderMaterialDescriptor>,
    textures: BTreeMap<String, TextureDescriptor>,
    atlases: BTreeMap<String, SpriteAtlasDescriptor>,
    static_meshes: BTreeMap<String, StaticMeshAsset>,
    animated_meshes: BTreeMap<String, AnimatedMeshAsset>,
}

#[derive(Debug, Clone)]
pub struct SceneAppearanceProjector {
    registry: StableHandleRegistry<AuthoredRenderKey>,
    last_nodes: BTreeMap<u64, ProjectedNode>,
    last_lights: BTreeMap<u64, ProjectedLight>,
    last_resources: ResourceSnapshot,
}

impl Default for SceneAppearanceProjector {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneAppearanceProjector {
    pub fn new() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::AUTHORED),
            last_nodes: BTreeMap::new(),
            last_lights: BTreeMap::new(),
            last_resources: ResourceSnapshot::default(),
        }
    }

    pub fn project(
        &mut self,
        scene: &AppearanceScene,
        mode: ProjectionMode,
    ) -> Result<SceneProjectionResult, SceneProjectionError> {
        let validated = validate_scene(scene, mode)?;
        let mut registry = self.registry.clone();
        let mut operations = Vec::new();

        let changed_static_meshes = changed_resource_ids(
            &self.last_resources.static_meshes,
            &validated.resources.static_meshes,
        );
        let changed_animated_meshes = changed_resource_ids(
            &self.last_resources.animated_meshes,
            &validated.resources.animated_meshes,
        );

        let mut recreate_nodes = BTreeSet::new();
        for (id, previous) in &self.last_nodes {
            if let Some(next) = validated.nodes.get(id) {
                let resource_changed = match &next.appearance {
                    Appearance::StaticMesh { asset, .. } => changed_static_meshes.contains(asset),
                    Appearance::AnimatedMesh { asset, .. } => {
                        changed_animated_meshes.contains(asset)
                    }
                    _ => false,
                };
                if requires_recreate(previous, next) || resource_changed {
                    recreate_nodes.insert(*id);
                }
            }
        }
        expand_descendants(&validated.nodes, &mut recreate_nodes);

        let mut recreate_lights = BTreeSet::new();
        for (id, previous) in &self.last_lights {
            if let Some(next) = validated.lights.get(id) {
                if previous.parent != next.parent || light_kind(previous) != light_kind(next) {
                    recreate_lights.insert(*id);
                }
            }
        }
        for (id, light) in &validated.lights {
            if light
                .parent
                .is_some_and(|parent| recreate_nodes.contains(&parent))
            {
                recreate_lights.insert(*id);
            }
        }

        // Lights must be removed before their parent nodes. Both retained consumers
        // recursively remove children with a parent; emitting the explicit light
        // destroy afterwards would therefore address a stale handle.
        for id in self
            .last_lights
            .keys()
            .copied()
            .filter(|id| !validated.lights.contains_key(id) || recreate_lights.contains(id))
        {
            if let Some(handle) = registry.remove(&AuthoredRenderKey::Light(id)) {
                operations.push(RenderDiff::Destroy { handle });
            }
        }
        let mut node_destroys: Vec<u64> = self
            .last_nodes
            .keys()
            .copied()
            .filter(|id| !validated.nodes.contains_key(id) || recreate_nodes.contains(id))
            .collect();
        node_destroys.sort_by_key(|id| std::cmp::Reverse(depth(*id, &self.last_nodes)));
        for id in node_destroys {
            if let Some(handle) = registry.remove(&AuthoredRenderKey::Node(id)) {
                operations.push(RenderDiff::Destroy { handle });
            }
        }

        // Asset definitions that changed under a stable id cannot replace a live
        // mesh. Emit all dependent destroys first, then redefine, then recreate.
        operations.extend(resource_diffs(&self.last_resources, &validated.resources));

        // Reserve every handle before creating children, while operation order
        // still follows topological depth.
        for id in validated.nodes.keys() {
            if registry.handle_of(&AuthoredRenderKey::Node(*id)).is_none() {
                registry
                    .allocate(AuthoredRenderKey::Node(*id))
                    .map_err(SceneProjectionError::Handle)?;
            }
        }
        for id in validated.lights.keys() {
            if registry.handle_of(&AuthoredRenderKey::Light(*id)).is_none() {
                registry
                    .allocate(AuthoredRenderKey::Light(*id))
                    .map_err(SceneProjectionError::Handle)?;
            }
        }

        let mut node_ids: Vec<u64> = validated.nodes.keys().copied().collect();
        node_ids.sort_by_key(|id| (depth(*id, &validated.nodes), *id));
        for id in node_ids {
            let node = &validated.nodes[&id];
            let key = AuthoredRenderKey::Node(id);
            let handle = registry
                .handle_of(&key)
                .expect("validated authored node has a reserved handle");
            let is_new = !self.last_nodes.contains_key(&id) || recreate_nodes.contains(&id);
            if is_new {
                let parent = node.parent.map(|parent| {
                    registry
                        .handle_of(&AuthoredRenderKey::Node(parent))
                        .expect("validated parent has a reserved handle")
                });
                operations.push(create_node(handle, parent, node));
            } else if let Some(previous) = self.last_nodes.get(&id) {
                append_node_updates(&mut operations, handle, previous, node);
            }
        }

        for (id, light) in &validated.lights {
            let handle = registry
                .handle_of(&AuthoredRenderKey::Light(*id))
                .expect("validated authored light has a reserved handle");
            if !self.last_lights.contains_key(id) || recreate_lights.contains(id) {
                let parent = light.parent.map(|parent| {
                    registry
                        .handle_of(&AuthoredRenderKey::Node(parent))
                        .expect("validated light parent has a reserved handle")
                });
                operations.push(RenderDiff::CreateLight {
                    handle,
                    parent,
                    light: light.light.clone(),
                });
            } else if self.last_lights.get(id) != Some(light) {
                operations.push(RenderDiff::UpdateLight {
                    handle,
                    light: light.light.clone(),
                });
            }
        }

        let frame =
            RenderFrameDiff::try_from_ops(operations).map_err(SceneProjectionError::Frame)?;
        self.registry = registry;
        self.last_nodes = validated.nodes;
        self.last_lights = validated.lights;
        self.last_resources = validated.resources;
        Ok(SceneProjectionResult {
            frame,
            readout: SceneProjectionReadout {
                mode,
                retained_nodes: self.last_nodes.len(),
                retained_lights: self.last_lights.len(),
                retained_resources: self.last_resources.count(),
            },
        })
    }

    pub fn node_handle(&self, id: u64) -> Option<RenderHandle> {
        self.registry.handle_of(&AuthoredRenderKey::Node(id))
    }
}

impl ResourceSnapshot {
    fn count(&self) -> usize {
        self.materials.len()
            + self.textures.len()
            + self.atlases.len()
            + self.static_meshes.len()
            + self.animated_meshes.len()
    }
}

struct ValidatedScene {
    resources: ResourceSnapshot,
    nodes: BTreeMap<u64, ProjectedNode>,
    lights: BTreeMap<u64, ProjectedLight>,
}

fn validate_scene(
    scene: &AppearanceScene,
    mode: ProjectionMode,
) -> Result<ValidatedScene, SceneProjectionError> {
    let resources = validate_resources(&scene.resources)?;
    let mut nodes = BTreeMap::new();
    for node in scene
        .nodes
        .iter()
        .filter(|node| node.availability.includes(mode))
    {
        if nodes.contains_key(&node.id) {
            return Err(SceneProjectionError::DuplicateNode { id: node.id });
        }
        node.transform
            .validate()
            .map_err(|source| SceneProjectionError::InvalidNodeTransform {
                id: node.id,
                source,
            })?;
        node.metadata
            .validate()
            .map_err(|source| SceneProjectionError::InvalidNodeMetadata {
                id: node.id,
                source,
            })?;
        validate_appearance(node, &resources)?;
        let mut appearance = node.appearance.clone();
        if let Appearance::Sprite { sprite } = &mut appearance {
            sprite.transform = node.transform;
            sprite.visible = node.visible;
            sprite.metadata = node.metadata.clone();
        }
        nodes.insert(
            node.id,
            ProjectedNode {
                parent: node.parent,
                transform: node.transform,
                visible: node.visible,
                layer: node.layer,
                metadata: node.metadata.clone(),
                appearance,
            },
        );
    }
    for (id, node) in &nodes {
        if node
            .parent
            .is_some_and(|parent| !nodes.contains_key(&parent))
        {
            return Err(SceneProjectionError::MissingParent {
                id: *id,
                parent: node.parent.unwrap_or_default(),
            });
        }
    }
    ensure_acyclic(&nodes)?;

    let mut lights = BTreeMap::new();
    for light in scene
        .lights
        .iter()
        .filter(|light| light.availability.includes(mode))
    {
        if lights.contains_key(&light.id) {
            return Err(SceneProjectionError::DuplicateLight { id: light.id });
        }
        if light
            .parent
            .is_some_and(|parent| !nodes.contains_key(&parent))
        {
            return Err(SceneProjectionError::MissingLightParent {
                id: light.id,
                parent: light.parent.unwrap_or_default(),
            });
        }
        light
            .light
            .validate()
            .map_err(|source| SceneProjectionError::InvalidLight {
                id: light.id,
                source,
            })?;
        lights.insert(
            light.id,
            ProjectedLight {
                parent: light.parent,
                light: light.light.clone(),
            },
        );
    }
    Ok(ValidatedScene {
        resources,
        nodes,
        lights,
    })
}

fn validate_resources(
    input: &AppearanceResources,
) -> Result<ResourceSnapshot, SceneProjectionError> {
    let mut resources = ResourceSnapshot::default();
    for material in &input.materials {
        material
            .validate()
            .map_err(|source| SceneProjectionError::InvalidMaterial {
                id: material.id.clone(),
                source,
            })?;
        insert_unique(
            &mut resources.materials,
            &material.id,
            material.clone(),
            "material",
        )?;
    }
    for texture in &input.textures {
        texture
            .validate()
            .map_err(|source| SceneProjectionError::InvalidTexture {
                id: texture.id.clone(),
                source,
            })?;
        insert_unique(
            &mut resources.textures,
            &texture.id,
            texture.clone(),
            "texture",
        )?;
    }
    for atlas in &input.sprite_atlases {
        atlas
            .validate()
            .map_err(|source| SceneProjectionError::InvalidAtlas {
                id: atlas.id.clone(),
                source,
            })?;
        insert_unique(&mut resources.atlases, &atlas.id, atlas.clone(), "atlas")?;
    }
    for mesh in &input.static_meshes {
        mesh.validate()
            .map_err(|source| SceneProjectionError::InvalidStaticMesh {
                id: mesh.asset.clone(),
                source,
            })?;
        insert_unique(
            &mut resources.static_meshes,
            &mesh.asset,
            mesh.clone(),
            "static mesh",
        )?;
    }
    for mesh in &input.animated_meshes {
        mesh.validate()
            .map_err(|source| SceneProjectionError::InvalidAnimatedMesh {
                id: mesh.asset.clone(),
                source,
            })?;
        insert_unique(
            &mut resources.animated_meshes,
            &mesh.asset,
            mesh.clone(),
            "animated mesh",
        )?;
    }

    for material in resources.materials.values() {
        if material
            .texture
            .as_ref()
            .is_some_and(|id| !resources.textures.contains_key(id))
        {
            return Err(SceneProjectionError::MissingTexture {
                owner: material.id.clone(),
                texture: material.texture.clone().unwrap_or_default(),
            });
        }
    }
    for atlas in resources.atlases.values() {
        if !resources.textures.contains_key(&atlas.texture) {
            return Err(SceneProjectionError::MissingTexture {
                owner: atlas.id.clone(),
                texture: atlas.texture.clone(),
            });
        }
    }
    for mesh in resources.static_meshes.values() {
        validate_material_references(&mesh.asset, &mesh.material_slots, &resources.materials)?;
    }
    for mesh in resources.animated_meshes.values() {
        validate_material_references(&mesh.asset, &mesh.material_slots, &resources.materials)?;
    }
    Ok(resources)
}

fn insert_unique<T>(
    map: &mut BTreeMap<String, T>,
    id: &str,
    value: T,
    kind: &'static str,
) -> Result<(), SceneProjectionError> {
    if map.insert(id.to_string(), value).is_some() {
        return Err(SceneProjectionError::DuplicateResource {
            kind,
            id: id.to_string(),
        });
    }
    Ok(())
}

fn validate_material_references(
    owner: &str,
    slots: &[MeshMaterialSlot],
    materials: &BTreeMap<String, RenderMaterialDescriptor>,
) -> Result<(), SceneProjectionError> {
    if let Some(slot) = slots
        .iter()
        .find(|slot| !materials.contains_key(&slot.material))
    {
        return Err(SceneProjectionError::MissingMaterial {
            owner: owner.to_string(),
            material: slot.material.clone(),
        });
    }
    Ok(())
}

fn validate_appearance(
    node: &AppearanceNode,
    resources: &ResourceSnapshot,
) -> Result<(), SceneProjectionError> {
    match &node.appearance {
        Appearance::Primitive { geometry, material } => RenderNode {
            geometry: *geometry,
            material: *material,
            transform: node.transform,
            visible: node.visible,
            layer: node.layer,
            metadata: node.metadata.clone(),
        }
        .validate()
        .map_err(|source| SceneProjectionError::InvalidPrimitive {
            id: node.id,
            source,
        }),
        Appearance::StaticMesh {
            asset,
            material_overrides,
        } => {
            if !resources.static_meshes.contains_key(asset) {
                return Err(SceneProjectionError::MissingStaticMesh {
                    id: node.id,
                    asset: asset.clone(),
                });
            }
            validate_material_references(asset, material_overrides, &resources.materials)?;
            StaticMeshInstanceDescriptor {
                asset: asset.clone(),
                transform: node.transform,
                visible: node.visible,
                material_overrides: material_overrides.clone(),
                metadata: node.metadata.clone(),
            }
            .validate()
            .map_err(|source| SceneProjectionError::InvalidStaticMeshInstance {
                id: node.id,
                source,
            })
        }
        Appearance::AnimatedMesh {
            asset,
            material_overrides,
            playback,
        } => {
            if !resources.animated_meshes.contains_key(asset) {
                return Err(SceneProjectionError::MissingAnimatedMesh {
                    id: node.id,
                    asset: asset.clone(),
                });
            }
            validate_material_references(asset, material_overrides, &resources.materials)?;
            AnimatedMeshInstanceDescriptor {
                asset: asset.clone(),
                transform: node.transform,
                visible: node.visible,
                material_overrides: material_overrides.clone(),
                playback: playback.clone(),
                metadata: node.metadata.clone(),
            }
            .validate()
            .map_err(|source| SceneProjectionError::InvalidAnimatedMeshInstance {
                id: node.id,
                source,
            })
        }
        Appearance::Sprite { sprite } => {
            if !resources.atlases.contains_key(&sprite.asset) {
                return Err(SceneProjectionError::MissingSpriteAtlas {
                    id: node.id,
                    asset: sprite.asset.clone(),
                });
            }
            let mut projected = sprite.clone();
            projected.transform = node.transform;
            projected.visible = node.visible;
            projected.metadata = node.metadata.clone();
            projected
                .validate()
                .map_err(|source| SceneProjectionError::InvalidSprite {
                    id: node.id,
                    source,
                })
        }
    }
}

fn ensure_acyclic(nodes: &BTreeMap<u64, ProjectedNode>) -> Result<(), SceneProjectionError> {
    for start in nodes.keys() {
        let mut seen = BTreeSet::new();
        let mut cursor = Some(*start);
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return Err(SceneProjectionError::ParentCycle { id });
            }
            cursor = nodes.get(&id).and_then(|node| node.parent);
        }
    }
    Ok(())
}

fn resource_diffs(previous: &ResourceSnapshot, next: &ResourceSnapshot) -> Vec<RenderDiff> {
    let mut operations = Vec::new();
    for (id, value) in &next.textures {
        if previous.textures.get(id) != Some(value) {
            operations.push(RenderDiff::DefineTexture {
                texture: value.clone(),
            });
        }
    }
    for (id, value) in &next.materials {
        if previous.materials.get(id) != Some(value) {
            operations.push(RenderDiff::DefineMaterial {
                material: value.clone(),
            });
        }
    }
    for (id, value) in &next.atlases {
        if previous.atlases.get(id) != Some(value) {
            operations.push(RenderDiff::DefineSpriteAtlas {
                atlas: value.clone(),
            });
        }
    }
    for (id, value) in &next.static_meshes {
        if previous.static_meshes.get(id) != Some(value) {
            operations.push(RenderDiff::DefineStaticMesh {
                asset: value.clone(),
            });
        }
    }
    for (id, value) in &next.animated_meshes {
        if previous.animated_meshes.get(id) != Some(value) {
            operations.push(RenderDiff::DefineAnimatedMesh {
                asset: value.clone(),
            });
        }
    }
    operations
}

fn changed_resource_ids<T: PartialEq>(
    previous: &BTreeMap<String, T>,
    next: &BTreeMap<String, T>,
) -> BTreeSet<String> {
    next.iter()
        .filter(|(id, value)| previous.get(*id).is_some_and(|previous| previous != *value))
        .map(|(id, _)| id.clone())
        .collect()
}

fn requires_recreate(previous: &ProjectedNode, next: &ProjectedNode) -> bool {
    if previous.parent != next.parent {
        return true;
    }
    match (&previous.appearance, &next.appearance) {
        (
            Appearance::Primitive {
                geometry: previous, ..
            },
            Appearance::Primitive { geometry: next, .. },
        ) => previous != next,
        (
            Appearance::StaticMesh {
                asset: previous,
                material_overrides: previous_slots,
            },
            Appearance::StaticMesh {
                asset: next,
                material_overrides: next_slots,
            },
        ) => previous != next || previous_slots != next_slots,
        (
            Appearance::AnimatedMesh {
                asset: previous,
                material_overrides: previous_slots,
                ..
            },
            Appearance::AnimatedMesh {
                asset: next,
                material_overrides: next_slots,
                ..
            },
        ) => previous != next || previous_slots != next_slots,
        (Appearance::Sprite { sprite: previous }, Appearance::Sprite { sprite: next }) => {
            previous.asset != next.asset
                || previous.pivot != next.pivot
                || previous.size != next.size
                || previous.size_mode != next.size_mode
                || previous.billboard != next.billboard
                || previous.depth != next.depth
                || previous.shading != next.shading
                || previous.material != next.material
                || previous.attachment != next.attachment
        }
        _ => true,
    }
}

fn expand_descendants(nodes: &BTreeMap<u64, ProjectedNode>, values: &mut BTreeSet<u64>) {
    loop {
        let before = values.len();
        for (id, node) in nodes {
            if node.parent.is_some_and(|parent| values.contains(&parent)) {
                values.insert(*id);
            }
        }
        if before == values.len() {
            break;
        }
    }
}

fn depth(id: u64, nodes: &BTreeMap<u64, ProjectedNode>) -> usize {
    let mut result = 0;
    let mut cursor = nodes.get(&id).and_then(|node| node.parent);
    while let Some(parent) = cursor {
        result += 1;
        cursor = nodes.get(&parent).and_then(|node| node.parent);
    }
    result
}

fn light_kind(light: &ProjectedLight) -> u8 {
    match light.light {
        LightDescriptor::Ambient { .. } => 0,
        LightDescriptor::Directional { .. } => 1,
        LightDescriptor::Point { .. } => 2,
        LightDescriptor::Spot { .. } => 3,
    }
}

fn create_node(
    handle: RenderHandle,
    parent: Option<RenderHandle>,
    node: &ProjectedNode,
) -> RenderDiff {
    match &node.appearance {
        Appearance::Primitive { geometry, material } => RenderDiff::Create {
            handle,
            parent,
            node: RenderNode {
                geometry: *geometry,
                material: *material,
                transform: node.transform,
                visible: node.visible,
                layer: node.layer,
                metadata: node.metadata.clone(),
            },
        },
        Appearance::StaticMesh {
            asset,
            material_overrides,
        } => RenderDiff::CreateStaticMeshInstance {
            handle,
            parent,
            instance: StaticMeshInstanceDescriptor {
                asset: asset.clone(),
                transform: node.transform,
                visible: node.visible,
                material_overrides: material_overrides.clone(),
                metadata: node.metadata.clone(),
            },
        },
        Appearance::AnimatedMesh {
            asset,
            material_overrides,
            playback,
        } => RenderDiff::CreateAnimatedMeshInstance {
            handle,
            parent,
            instance: AnimatedMeshInstanceDescriptor {
                asset: asset.clone(),
                transform: node.transform,
                visible: node.visible,
                material_overrides: material_overrides.clone(),
                playback: playback.clone(),
                metadata: node.metadata.clone(),
            },
        },
        Appearance::Sprite { sprite } => {
            let mut sprite = sprite.clone();
            sprite.transform = node.transform;
            sprite.visible = node.visible;
            sprite.metadata = node.metadata.clone();
            RenderDiff::CreateSprite {
                handle,
                parent,
                sprite,
            }
        }
    }
}

fn append_node_updates(
    operations: &mut Vec<RenderDiff>,
    handle: RenderHandle,
    previous: &ProjectedNode,
    next: &ProjectedNode,
) {
    let transform = (previous.transform != next.transform).then_some(next.transform);
    let visible = (previous.visible != next.visible).then_some(next.visible);
    let metadata = (previous.metadata != next.metadata).then(|| next.metadata.clone());
    match (&previous.appearance, &next.appearance) {
        (
            Appearance::Primitive { material: old, .. },
            Appearance::Primitive { material: new, .. },
        ) => {
            if transform.is_some() || visible.is_some() || metadata.is_some() || old != new {
                operations.push(RenderDiff::Update {
                    handle,
                    transform,
                    material: (old != new).then_some(*new),
                    visible,
                    metadata,
                });
            }
        }
        (
            Appearance::AnimatedMesh {
                material_overrides: old_slots,
                playback: old_playback,
                ..
            },
            Appearance::AnimatedMesh {
                material_overrides: new_slots,
                playback: new_playback,
                ..
            },
        ) => {
            if old_slots != new_slots {
                // Slot rebindings are creation-time structural values.
                unreachable!("material override changes require recreation")
            }
            if transform.is_some() || visible.is_some() || metadata.is_some() {
                operations.push(RenderDiff::Update {
                    handle,
                    transform,
                    material: None,
                    visible,
                    metadata,
                });
            }
            if old_playback != new_playback {
                operations.push(RenderDiff::SetAnimatedMeshPlayback {
                    handle,
                    playback: new_playback
                        .clone()
                        .unwrap_or(AnimatedMeshPlaybackCommand::Stop { fade_seconds: None }),
                });
            }
        }
        (
            Appearance::StaticMesh {
                material_overrides: old_slots,
                ..
            },
            Appearance::StaticMesh {
                material_overrides: new_slots,
                ..
            },
        ) => {
            if old_slots != new_slots {
                unreachable!("material override changes require recreation")
            }
            if transform.is_some() || visible.is_some() || metadata.is_some() {
                operations.push(RenderDiff::Update {
                    handle,
                    transform,
                    material: None,
                    visible,
                    metadata,
                });
            }
        }
        (Appearance::Sprite { sprite: old }, Appearance::Sprite { sprite: new }) => {
            if transform.is_some() || metadata.is_some() {
                operations.push(RenderDiff::Update {
                    handle,
                    transform,
                    material: None,
                    visible: None,
                    metadata,
                });
            }
            if old.frame != new.frame
                || old.tint != new.tint
                || old.render_order != new.render_order
                || visible.is_some()
            {
                operations.push(RenderDiff::UpdateSprite {
                    handle,
                    frame: (old.frame != new.frame).then_some(new.frame),
                    tint: (old.tint != new.tint).then_some(new.tint),
                    render_order: (old.render_order != new.render_order)
                        .then_some(new.render_order),
                    visible,
                });
            }
        }
        _ => unreachable!("appearance kind changes require recreation"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneProjectionResult {
    pub frame: RenderFrameDiff,
    pub readout: SceneProjectionReadout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneProjectionReadout {
    pub mode: ProjectionMode,
    pub retained_nodes: usize,
    pub retained_lights: usize,
    pub retained_resources: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneProjectionError {
    DuplicateResource {
        kind: &'static str,
        id: String,
    },
    InvalidMaterial {
        id: String,
        source: render_model::MaterialDescriptorError,
    },
    InvalidTexture {
        id: String,
        source: render_model::TextureError,
    },
    InvalidAtlas {
        id: String,
        source: render_model::SpriteAtlasError,
    },
    InvalidStaticMesh {
        id: String,
        source: render_model::StaticMeshError,
    },
    InvalidAnimatedMesh {
        id: String,
        source: render_model::AnimatedMeshAssetError,
    },
    MissingTexture {
        owner: String,
        texture: String,
    },
    MissingMaterial {
        owner: String,
        material: String,
    },
    DuplicateNode {
        id: u64,
    },
    MissingParent {
        id: u64,
        parent: u64,
    },
    ParentCycle {
        id: u64,
    },
    InvalidNodeTransform {
        id: u64,
        source: render_model::TransformError,
    },
    InvalidNodeMetadata {
        id: u64,
        source: render_model::NodeError,
    },
    InvalidPrimitive {
        id: u64,
        source: render_model::NodeError,
    },
    MissingStaticMesh {
        id: u64,
        asset: String,
    },
    MissingAnimatedMesh {
        id: u64,
        asset: String,
    },
    MissingSpriteAtlas {
        id: u64,
        asset: String,
    },
    InvalidStaticMeshInstance {
        id: u64,
        source: render_model::StaticMeshInstanceError,
    },
    InvalidAnimatedMeshInstance {
        id: u64,
        source: render_model::AnimatedMeshInstanceError,
    },
    InvalidSprite {
        id: u64,
        source: render_model::SpriteError,
    },
    DuplicateLight {
        id: u64,
    },
    MissingLightParent {
        id: u64,
        parent: u64,
    },
    InvalidLight {
        id: u64,
        source: render_model::LightDescriptorError,
    },
    Handle(HandleAllocationError),
    Frame(RenderFrameError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_model::{
        AnimatedMeshRuntimeFormat, AnimationClipDescriptor, LightShadowIntent, MaterialUvStrategy,
        MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor,
        MeshBufferLayout, MeshCollisionPolicy, MeshGroupDescriptor, MeshIndexWidth,
        MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance,
    };

    fn material() -> RenderMaterialDescriptor {
        RenderMaterialDescriptor {
            schema_version: 2,
            id: "material/plain".to_string(),
            color: [0.4, 0.5, 0.6, 1.0],
            texture: None,
            roughness: 1.0,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: MaterialUvStrategy::Flat,
            alpha_mode: Default::default(),
            double_sided: false,
            voxel_surface: None,
        }
    }

    fn mesh() -> StaticMeshAsset {
        StaticMeshAsset {
            asset: "mesh/triangle".to_string(),
            payload: MeshPayloadDescriptor {
                layout: MeshBufferLayout {
                    vertex_count: 3,
                    index_count: 3,
                    index_width: MeshIndexWidth::U32,
                    attributes: vec![
                        MeshAttribute {
                            name: MeshAttributeName::Position,
                            components: 3,
                            kind: MeshAttributeKind::F32,
                        },
                        MeshAttribute {
                            name: MeshAttributeName::Normal,
                            components: 3,
                            kind: MeshAttributeKind::F32,
                        },
                    ],
                },
                groups: vec![MeshGroupDescriptor {
                    material_slot: 0,
                    start: 0,
                    count: 3,
                }],
                bounds: MeshBoundsDescriptor {
                    min: [0.0; 3],
                    max: [1.0, 1.0, 0.0],
                },
                source: MeshPayloadSource::Inline {
                    positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                    uvs: None,
                    colors: None,
                    indices: vec![0, 1, 2],
                },
                provenance: MeshProvenance::StaticAsset,
            },
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_string(),
            }],
            collision: MeshCollisionPolicy::VisualOnly,
        }
    }

    fn animated_mesh() -> AnimatedMeshAsset {
        AnimatedMeshAsset {
            asset: "mesh-animation/character".to_string(),
            runtime_format: AnimatedMeshRuntimeFormat::Glb,
            content_hash: Some("first".to_string()),
            clips: vec![AnimationClipDescriptor {
                id: "idle".to_string(),
                name: Some("Idle".to_string()),
                duration_seconds: Some(1.0),
            }],
            rig: None,
            clip_packs: vec![],
            default_clip: Some("idle".to_string()),
            embedded_material_slots: vec![],
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_string(),
            }],
            bounds: MeshBoundsDescriptor {
                min: [-0.5, 0.0, -0.5],
                max: [0.5, 2.0, 0.5],
            },
        }
    }

    #[test]
    fn authored_scene_defines_resources_before_hierarchical_instances_and_lights() {
        let scene = AppearanceScene {
            resources: AppearanceResources {
                materials: vec![material()],
                static_meshes: vec![mesh()],
                ..AppearanceResources::default()
            },
            nodes: vec![
                AppearanceNode {
                    id: 1,
                    parent: None,
                    transform: Transform::IDENTITY,
                    visible: true,
                    metadata: RenderMetadata::default(),
                    availability: ProjectionAvailability::Both,
                    appearance: Appearance::Primitive {
                        geometry: Geometry::Cube,
                        material: Material::DEFAULT,
                    },
                },
                AppearanceNode {
                    id: 2,
                    parent: Some(1),
                    transform: Transform::IDENTITY,
                    visible: true,
                    metadata: RenderMetadata::default(),
                    availability: ProjectionAvailability::Both,
                    appearance: Appearance::StaticMesh {
                        asset: "mesh/triangle".to_string(),
                        material_overrides: Vec::new(),
                    },
                },
            ],
            lights: vec![AppearanceLight {
                id: 8,
                parent: Some(1),
                availability: ProjectionAvailability::Both,
                light: LightDescriptor::Point {
                    color: [1.0; 3],
                    intensity: 2.0,
                    enabled: true,
                    position: [0.0, 2.0, 0.0],
                    range: Some(10.0),
                    decay: 2.0,
                    shadow_intent: LightShadowIntent::Requested,
                },
            }],
        };
        let mut projector = SceneAppearanceProjector::new();
        let result = projector
            .project(&scene, ProjectionMode::AuthoredPreview)
            .unwrap();
        assert!(matches!(
            result.frame.ops[0],
            RenderDiff::DefineMaterial { .. }
        ));
        assert!(matches!(
            result.frame.ops[1],
            RenderDiff::DefineStaticMesh { .. }
        ));
        assert!(result
            .frame
            .ops
            .iter()
            .any(|operation| matches!(operation, RenderDiff::CreateLight { .. })));
        assert!(projector
            .project(&scene, ProjectionMode::AuthoredPreview)
            .unwrap()
            .frame
            .is_empty());
    }

    #[test]
    fn static_mesh_edits_destroy_dependents_before_redefinition_and_recreation() {
        let mut scene = AppearanceScene {
            resources: AppearanceResources {
                materials: vec![material()],
                static_meshes: vec![mesh()],
                ..AppearanceResources::default()
            },
            nodes: vec![
                AppearanceNode {
                    id: 1,
                    parent: None,
                    transform: Transform::IDENTITY,
                    visible: true,
                    metadata: RenderMetadata::default(),
                    availability: ProjectionAvailability::Both,
                    appearance: Appearance::StaticMesh {
                        asset: "mesh/triangle".to_string(),
                        material_overrides: Vec::new(),
                    },
                },
                AppearanceNode {
                    id: 2,
                    parent: Some(1),
                    transform: Transform::IDENTITY,
                    visible: true,
                    metadata: RenderMetadata::default(),
                    availability: ProjectionAvailability::Both,
                    appearance: Appearance::Primitive {
                        geometry: Geometry::Cube,
                        material: Material::DEFAULT,
                    },
                },
                AppearanceNode {
                    id: 4,
                    parent: None,
                    transform: Transform::IDENTITY,
                    visible: true,
                    metadata: RenderMetadata::default(),
                    availability: ProjectionAvailability::Both,
                    appearance: Appearance::StaticMesh {
                        asset: "mesh/triangle".to_string(),
                        material_overrides: Vec::new(),
                    },
                },
            ],
            lights: vec![AppearanceLight {
                id: 3,
                parent: Some(1),
                availability: ProjectionAvailability::Both,
                light: LightDescriptor::Ambient {
                    color: [1.0; 3],
                    intensity: 1.0,
                    enabled: true,
                    shadow_intent: LightShadowIntent::Disabled,
                },
            }],
        };
        let mut projector = SceneAppearanceProjector::new();
        projector
            .project(&scene, ProjectionMode::AuthoredPreview)
            .unwrap();

        scene.resources.static_meshes[0].payload.bounds.max[0] = 2.0;
        let edited = projector
            .project(&scene, ProjectionMode::AuthoredPreview)
            .unwrap();
        let redefine = edited
            .frame
            .ops
            .iter()
            .position(|operation| matches!(operation, RenderDiff::DefineStaticMesh { .. }))
            .unwrap();

        assert_eq!(
            edited.frame.ops[..redefine]
                .iter()
                .filter(|operation| matches!(operation, RenderDiff::Destroy { .. }))
                .count(),
            4
        );
        assert!(edited.frame.ops[..redefine]
            .iter()
            .all(|operation| matches!(operation, RenderDiff::Destroy { .. })));
        assert!(edited.frame.ops[redefine + 1..]
            .iter()
            .all(|operation| matches!(
                operation,
                RenderDiff::CreateStaticMeshInstance { .. }
                    | RenderDiff::Create { .. }
                    | RenderDiff::CreateLight { .. }
            )));
    }

    #[test]
    fn animated_mesh_edits_destroy_instances_before_redefinition_and_recreation() {
        let mut scene = AppearanceScene {
            resources: AppearanceResources {
                materials: vec![material()],
                animated_meshes: vec![animated_mesh()],
                ..AppearanceResources::default()
            },
            nodes: vec![AppearanceNode {
                id: 1,
                parent: None,
                transform: Transform::IDENTITY,
                visible: true,
                metadata: RenderMetadata::default(),
                availability: ProjectionAvailability::Both,
                appearance: Appearance::AnimatedMesh {
                    asset: "mesh-animation/character".to_string(),
                    material_overrides: Vec::new(),
                    playback: None,
                },
            }],
            ..AppearanceScene::default()
        };
        let mut projector = SceneAppearanceProjector::new();
        projector
            .project(&scene, ProjectionMode::AuthoredPreview)
            .unwrap();

        scene.resources.animated_meshes[0].content_hash = Some("second".to_string());
        let edited = projector
            .project(&scene, ProjectionMode::AuthoredPreview)
            .unwrap();

        assert!(matches!(edited.frame.ops[0], RenderDiff::Destroy { .. }));
        assert!(matches!(
            edited.frame.ops[1],
            RenderDiff::DefineAnimatedMesh { .. }
        ));
        assert!(matches!(
            edited.frame.ops[2],
            RenderDiff::CreateAnimatedMeshInstance { .. }
        ));
    }

    #[test]
    fn availability_modes_are_explicit_and_parent_cycles_fail_atomically() {
        let mut scene = AppearanceScene {
            nodes: vec![AppearanceNode {
                id: 1,
                parent: None,
                transform: Transform::IDENTITY,
                visible: true,
                metadata: RenderMetadata::default(),
                availability: ProjectionAvailability::AuthoredOnly,
                appearance: Appearance::Primitive {
                    geometry: Geometry::Cube,
                    material: Material::DEFAULT,
                },
            }],
            ..AppearanceScene::default()
        };
        let mut projector = SceneAppearanceProjector::new();
        assert_eq!(
            projector
                .project(&scene, ProjectionMode::Runtime)
                .unwrap()
                .readout
                .retained_nodes,
            0
        );
        scene.nodes[0].availability = ProjectionAvailability::Both;
        scene.nodes[0].parent = Some(1);
        assert!(matches!(
            projector.project(&scene, ProjectionMode::Runtime),
            Err(SceneProjectionError::ParentCycle { .. })
        ));
        assert_eq!(projector.node_handle(1), None);
    }
}
