//! Appearance values, the resource catalog, and the render operations the
//! retained runtime projector emits for them.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use render_model::{
    AnimatedMeshAsset, AnimatedMeshInstanceDescriptor, AnimatedMeshPlaybackCommand, Geometry,
    LightDescriptor, Material, MaterialInstanceParameters, MeshMaterialSlot, RenderDiff,
    RenderHandle, RenderLayer, RenderMaterialDescriptor, RenderMetadata, RenderNode,
    ShaderDescriptor, ShadowCasting, SpriteAtlasDescriptor, SpriteBatchDescriptor,
    SpriteInstanceDescriptor, StaticMeshAsset, StaticMeshInstanceDescriptor, TextureDescriptor,
    Transform,
};

use crate::{HandleAllocationError, ResourceList};

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
        /// Per material slot values over the slot's material (base colour,
        /// texture tint, emission). A change updates the instance in place.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        material_parameters: BTreeMap<u16, MaterialInstanceParameters>,
    },
    AnimatedMesh {
        #[serde(default)]
        inspection: render_model::AnimatedMeshInspection,
        asset: String,
        material_overrides: Vec<MeshMaterialSlot>,
        playback: Option<AnimatedMeshPlaybackCommand>,
        /// Per embedded material slot values over the slot's material. A
        /// change updates the instance in place.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        material_parameters: BTreeMap<u16, MaterialInstanceParameters>,
    },
    Sprite {
        sprite: SpriteInstanceDescriptor,
    },
    /// Many sprites from one atlas shown as one object.
    SpriteBatch {
        batch: SpriteBatchDescriptor,
    },
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceResources {
    #[serde(default)]
    pub materials: ResourceList<RenderMaterialDescriptor>,
    #[serde(default)]
    pub textures: ResourceList<TextureDescriptor>,
    /// Product shaders the materials name.
    #[serde(default)]
    pub shaders: ResourceList<ShaderDescriptor>,
    #[serde(default)]
    pub sprite_atlases: ResourceList<SpriteAtlasDescriptor>,
    /// Admitted mesh bodies are immutable and shared; cloning a catalog
    /// copies pointers, not vertex data.
    #[serde(default)]
    pub static_meshes: ResourceList<Arc<StaticMeshAsset>>,
    #[serde(default)]
    pub animated_meshes: ResourceList<Arc<AnimatedMeshAsset>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ResourceSnapshot {
    pub(crate) materials: BTreeMap<String, RenderMaterialDescriptor>,
    pub(crate) textures: BTreeMap<String, TextureDescriptor>,
    pub(crate) shaders: BTreeMap<String, ShaderDescriptor>,
    pub(crate) atlases: BTreeMap<String, SpriteAtlasDescriptor>,
    pub(crate) static_meshes: BTreeMap<String, Arc<StaticMeshAsset>>,
    pub(crate) animated_meshes: BTreeMap<String, Arc<AnimatedMeshAsset>>,
}

/// Meshes are validated when first projected; a body already retained by the
/// previous projection (the same shared allocation) is not scanned again.
pub(crate) fn validate_resources(
    input: &AppearanceResources,
    previous: &ResourceSnapshot,
) -> Result<ResourceSnapshot, AppearanceProjectionError> {
    let mut resources = ResourceSnapshot::default();
    for material in &input.materials {
        material
            .validate()
            .map_err(|source| AppearanceProjectionError::InvalidMaterial {
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
            .map_err(|source| AppearanceProjectionError::InvalidTexture {
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
    for shader in &input.shaders {
        shader
            .validate()
            .map_err(|source| AppearanceProjectionError::InvalidShader {
                id: shader.id.clone(),
                source,
            })?;
        insert_unique(&mut resources.shaders, &shader.id, shader.clone(), "shader")?;
    }
    for atlas in &input.sprite_atlases {
        atlas
            .validate()
            .map_err(|source| AppearanceProjectionError::InvalidAtlas {
                id: atlas.id.clone(),
                source,
            })?;
        insert_unique(&mut resources.atlases, &atlas.id, atlas.clone(), "atlas")?;
    }
    for mesh in &input.static_meshes {
        if !retained(&previous.static_meshes, &mesh.asset, mesh) {
            mesh.validate()
                .map_err(|source| AppearanceProjectionError::InvalidStaticMesh {
                    id: mesh.asset.clone(),
                    source,
                })?;
        }
        insert_unique(
            &mut resources.static_meshes,
            &mesh.asset,
            Arc::clone(mesh),
            "static mesh",
        )?;
    }
    for mesh in &input.animated_meshes {
        if !retained(&previous.animated_meshes, &mesh.asset, mesh) {
            mesh.validate()
                .map_err(|source| AppearanceProjectionError::InvalidAnimatedMesh {
                    id: mesh.asset.clone(),
                    source,
                })?;
        }
        insert_unique(
            &mut resources.animated_meshes,
            &mesh.asset,
            Arc::clone(mesh),
            "animated mesh",
        )?;
    }

    for material in resources.materials.values() {
        if material
            .texture
            .as_ref()
            .is_some_and(|id| !resources.textures.contains_key(id))
        {
            return Err(AppearanceProjectionError::MissingTexture {
                owner: material.id.clone(),
                texture: material.texture.clone().unwrap_or_default(),
            });
        }
    }
    for atlas in resources.atlases.values() {
        if !resources.textures.contains_key(&atlas.texture) {
            return Err(AppearanceProjectionError::MissingTexture {
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
) -> Result<(), AppearanceProjectionError> {
    if map.insert(id.to_string(), value).is_some() {
        return Err(AppearanceProjectionError::DuplicateResource {
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
) -> Result<(), AppearanceProjectionError> {
    if let Some(slot) = slots
        .iter()
        .find(|slot| !materials.contains_key(&slot.material))
    {
        return Err(AppearanceProjectionError::MissingMaterial {
            owner: owner.to_string(),
            material: slot.material.clone(),
        });
    }
    Ok(())
}

/// The renderer-facing values of one retained object.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NodeValues<'a> {
    pub(crate) appearance: &'a Appearance,
    pub(crate) transform: Transform,
    pub(crate) visible: bool,
    pub(crate) layer: RenderLayer,
    pub(crate) shadow_casting: ShadowCasting,
}

pub(crate) fn validate_appearance(
    id: u64,
    node: NodeValues<'_>,
    metadata: &RenderMetadata,
    resources: &ResourceSnapshot,
) -> Result<(), AppearanceProjectionError> {
    match node.appearance {
        Appearance::Primitive { geometry, material } => RenderNode {
            geometry: *geometry,
            material: *material,
            transform: node.transform,
            visible: node.visible,
            layer: node.layer,
            shadow_casting: node.shadow_casting,
            metadata: metadata.clone(),
        }
        .validate()
        .map_err(|source| AppearanceProjectionError::InvalidPrimitive { id, source }),
        Appearance::StaticMesh {
            asset,
            material_overrides,
            ..
        } => {
            if !resources.static_meshes.contains_key(asset) {
                return Err(AppearanceProjectionError::MissingStaticMesh {
                    id,
                    asset: asset.clone(),
                });
            }
            validate_material_references(asset, material_overrides, &resources.materials)?;
            StaticMeshInstanceDescriptor {
                asset: asset.clone(),
                transform: node.transform,
                visible: node.visible,
                material_overrides: material_overrides.clone(),
                metadata: metadata.clone(),
                layer: node.layer,
                shadow_casting: Default::default(),
            }
            .validate()
            .map_err(|source| AppearanceProjectionError::InvalidStaticMeshInstance { id, source })
        }
        Appearance::AnimatedMesh {
            asset,
            material_overrides,
            playback,
            inspection,
            material_parameters,
        } => {
            if !resources.animated_meshes.contains_key(asset) {
                return Err(AppearanceProjectionError::MissingAnimatedMesh {
                    id,
                    asset: asset.clone(),
                });
            }
            validate_material_references(asset, material_overrides, &resources.materials)?;
            if let Some((&slot, _)) = material_parameters
                .iter()
                .find(|(_, parameters)| parameters.validate().is_err())
            {
                return Err(AppearanceProjectionError::InvalidMaterialParameters { id, slot });
            }
            AnimatedMeshInstanceDescriptor {
                inspection: inspection.clone(),
                asset: asset.clone(),
                transform: node.transform,
                visible: node.visible,
                material_overrides: material_overrides.clone(),
                playback: playback.clone(),
                metadata: metadata.clone(),
                layer: node.layer,
                shadow_casting: Default::default(),
            }
            .validate()
            .map_err(|source| AppearanceProjectionError::InvalidAnimatedMeshInstance { id, source })
        }
        Appearance::Sprite { sprite } => {
            if !resources.atlases.contains_key(&sprite.asset) {
                return Err(AppearanceProjectionError::MissingSpriteAtlas {
                    id,
                    asset: sprite.asset.clone(),
                });
            }
            let mut projected = sprite.clone();
            projected.transform = node.transform;
            projected.visible = node.visible;
            projected.metadata = metadata.clone();
            projected
                .validate()
                .map_err(|source| AppearanceProjectionError::InvalidSprite { id, source })
        }
        Appearance::SpriteBatch { batch } => {
            if !resources.atlases.contains_key(&batch.sprite.asset) {
                return Err(AppearanceProjectionError::MissingSpriteAtlas {
                    id,
                    asset: batch.sprite.asset.clone(),
                });
            }
            let mut projected = batch.clone();
            projected.sprite.transform = node.transform;
            projected.sprite.visible = node.visible;
            projected.sprite.metadata = metadata.clone();
            projected
                .validate()
                .map_err(|source| AppearanceProjectionError::InvalidSpriteBatch { id, source })
        }
    }
}

pub(crate) fn resource_diffs(
    previous: &ResourceSnapshot,
    next: &ResourceSnapshot,
) -> Vec<RenderDiff> {
    let mut operations = Vec::new();
    for (id, value) in &next.textures {
        if previous.textures.get(id) != Some(value) {
            operations.push(RenderDiff::DefineTexture {
                texture: value.clone(),
            });
        }
    }
    for (id, value) in &next.shaders {
        if previous.shaders.get(id) != Some(value) {
            operations.push(RenderDiff::DefineShader {
                shader: value.clone(),
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
        if !unchanged(previous.static_meshes.get(id), value) {
            operations.push(RenderDiff::DefineStaticMesh {
                asset: StaticMeshAsset::clone(value),
            });
        }
    }
    for (id, value) in &next.animated_meshes {
        if !unchanged(previous.animated_meshes.get(id), value) {
            operations.push(RenderDiff::DefineAnimatedMesh {
                asset: AnimatedMeshAsset::clone(value),
            });
        }
    }
    // Replacements may stop referring to a removed dependency. Install them
    // before releasing old definitions, then retire dependents before sources.
    for id in previous.static_meshes.keys() {
        if !next.static_meshes.contains_key(id) {
            operations.push(RenderDiff::ReleaseStaticMesh { asset: id.clone() });
        }
    }
    for id in previous.animated_meshes.keys() {
        if !next.animated_meshes.contains_key(id) {
            operations.push(RenderDiff::ReleaseAnimatedMesh { asset: id.clone() });
        }
    }
    for id in previous.atlases.keys() {
        if !next.atlases.contains_key(id) {
            operations.push(RenderDiff::ReleaseSpriteAtlas { id: id.clone() });
        }
    }
    for id in previous.materials.keys() {
        if !next.materials.contains_key(id) {
            operations.push(RenderDiff::ReleaseMaterial { id: id.clone() });
        }
    }
    for id in previous.textures.keys() {
        if !next.textures.contains_key(id) {
            operations.push(RenderDiff::ReleaseTexture { id: id.clone() });
        }
    }
    for id in previous.shaders.keys() {
        if !next.shaders.contains_key(id) {
            operations.push(RenderDiff::ReleaseShader { id: id.clone() });
        }
    }
    operations
}

/// Ids of shared resources whose definition changed. The same allocation is
/// unchanged without comparing its body.
pub(crate) fn changed_shared_ids<T: PartialEq>(
    previous: &BTreeMap<String, Arc<T>>,
    next: &BTreeMap<String, Arc<T>>,
) -> BTreeSet<String> {
    next.iter()
        .filter(|(id, value)| previous.contains_key(*id) && !unchanged(previous.get(*id), value))
        .map(|(id, _)| id.clone())
        .collect()
}

fn unchanged<T: PartialEq>(previous: Option<&Arc<T>>, next: &Arc<T>) -> bool {
    previous.is_some_and(|previous| Arc::ptr_eq(previous, next) || **previous == **next)
}

fn retained<T>(previous: &BTreeMap<String, Arc<T>>, id: &str, next: &Arc<T>) -> bool {
    previous
        .get(id)
        .is_some_and(|previous| Arc::ptr_eq(previous, next))
}

/// Whether an appearance change needs a new renderer object rather than an
/// update. Parent and layer changes are checked by the caller.
pub(crate) fn requires_recreate(previous: &Appearance, next: &Appearance) -> bool {
    match (previous, next) {
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
                ..
            },
            Appearance::StaticMesh {
                asset: next,
                material_overrides: next_slots,
                ..
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
            sprite_requires_recreate(previous, next)
        }
        (Appearance::SpriteBatch { batch: previous }, Appearance::SpriteBatch { batch: next }) => {
            sprite_requires_recreate(&previous.sprite, &next.sprite)
                || previous.instances != next.instances
        }
        _ => true,
    }
}

fn sprite_requires_recreate(
    previous: &SpriteInstanceDescriptor,
    next: &SpriteInstanceDescriptor,
) -> bool {
    previous.asset != next.asset
        || previous.pivot != next.pivot
        || previous.size != next.size
        || previous.size_mode != next.size_mode
        || previous.billboard != next.billboard
        || previous.depth != next.depth
        || previous.viewport_placement != next.viewport_placement
        || previous.shading != next.shading
        || previous.material != next.material
        || previous.attachment != next.attachment
}

pub(crate) fn light_kind(light: &LightDescriptor) -> u8 {
    match light {
        LightDescriptor::Ambient { .. } => 0,
        LightDescriptor::Directional { .. } => 1,
        LightDescriptor::Point { .. } => 2,
        LightDescriptor::Spot { .. } => 3,
        LightDescriptor::Hemisphere { .. } => 4,
    }
}

pub(crate) fn create_node(
    handle: RenderHandle,
    parent: Option<RenderHandle>,
    node: NodeValues<'_>,
    metadata: RenderMetadata,
) -> RenderDiff {
    match node.appearance {
        Appearance::Primitive { geometry, material } => RenderDiff::Create {
            handle,
            parent,
            node: RenderNode {
                geometry: *geometry,
                material: *material,
                transform: node.transform,
                visible: node.visible,
                layer: node.layer,
                shadow_casting: node.shadow_casting,
                metadata,
            },
        },
        Appearance::StaticMesh {
            asset,
            material_overrides,
            ..
        } => RenderDiff::CreateStaticMeshInstance {
            handle,
            parent,
            instance: StaticMeshInstanceDescriptor {
                asset: asset.clone(),
                transform: node.transform,
                visible: node.visible,
                material_overrides: material_overrides.clone(),
                metadata,
                layer: node.layer,
                shadow_casting: node.shadow_casting,
            },
        },
        Appearance::AnimatedMesh {
            asset,
            material_overrides,
            playback,
            inspection,
            ..
        } => RenderDiff::CreateAnimatedMeshInstance {
            handle,
            parent,
            instance: AnimatedMeshInstanceDescriptor {
                inspection: inspection.clone(),
                asset: asset.clone(),
                transform: node.transform,
                visible: node.visible,
                material_overrides: material_overrides.clone(),
                playback: playback.clone(),
                metadata,
                layer: node.layer,
                shadow_casting: node.shadow_casting,
            },
        },
        Appearance::Sprite { sprite } => {
            let mut sprite = sprite.clone();
            sprite.transform = node.transform;
            sprite.visible = node.visible;
            sprite.layer = node.layer;
            sprite.metadata = metadata;
            RenderDiff::CreateSprite {
                handle,
                parent,
                sprite,
            }
        }
        Appearance::SpriteBatch { batch } => {
            let mut batch = batch.clone();
            batch.sprite.transform = node.transform;
            batch.sprite.visible = node.visible;
            batch.sprite.layer = node.layer;
            batch.sprite.metadata = metadata;
            RenderDiff::CreateSpriteBatch {
                handle,
                parent,
                batch,
            }
        }
    }
}

/// The parameter operations that take a node's material slots from
/// `previous` to `next`.
pub(crate) fn append_material_parameters(
    operations: &mut Vec<RenderDiff>,
    handle: RenderHandle,
    previous: &BTreeMap<u16, MaterialInstanceParameters>,
    next: &BTreeMap<u16, MaterialInstanceParameters>,
) {
    for slot in previous.keys().filter(|slot| !next.contains_key(slot)) {
        operations.push(RenderDiff::SetMaterialInstanceParameters {
            handle,
            slot: *slot,
            parameters: None,
        });
    }
    for (slot, parameters) in next {
        if previous.get(slot) != Some(parameters) {
            operations.push(RenderDiff::SetMaterialInstanceParameters {
                handle,
                slot: *slot,
                parameters: Some(*parameters),
            });
        }
    }
}

/// The changed values of an object that keeps its renderer object.
pub(crate) struct NodeUpdate {
    pub(crate) transform: Option<Transform>,
    pub(crate) visible: Option<bool>,
    pub(crate) metadata: Option<RenderMetadata>,
}

pub(crate) fn append_node_updates(
    operations: &mut Vec<RenderDiff>,
    handle: RenderHandle,
    previous: &Appearance,
    next: &Appearance,
    update: NodeUpdate,
) {
    let NodeUpdate {
        transform,
        visible,
        metadata,
    } = update;
    match (previous, next) {
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
                inspection: old_inspection,
                material_parameters: old_parameters,
                ..
            },
            Appearance::AnimatedMesh {
                material_overrides: new_slots,
                playback: new_playback,
                inspection: new_inspection,
                material_parameters: new_parameters,
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
            if old_inspection != new_inspection {
                operations.push(RenderDiff::SetAnimatedMeshInspection {
                    handle,
                    inspection: new_inspection.clone(),
                });
            }
            append_material_parameters(operations, handle, old_parameters, new_parameters);
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
                material_parameters: old_parameters,
                ..
            },
            Appearance::StaticMesh {
                material_overrides: new_slots,
                material_parameters: new_parameters,
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
            append_material_parameters(operations, handle, old_parameters, new_parameters);
        }
        (Appearance::Sprite { sprite: old }, Appearance::Sprite { sprite: new }) => {
            append_sprite_updates(
                operations, handle, old, new, transform, visible, metadata, true,
            );
        }
        (Appearance::SpriteBatch { batch: old }, Appearance::SpriteBatch { batch: new }) => {
            // Each instance keeps its own frame.
            append_sprite_updates(
                operations,
                handle,
                &old.sprite,
                &new.sprite,
                transform,
                visible,
                metadata,
                false,
            );
        }
        _ => unreachable!("appearance kind changes require recreation"),
    }
}

#[allow(clippy::too_many_arguments, reason = "one sprite's changed values")]
fn append_sprite_updates(
    operations: &mut Vec<RenderDiff>,
    handle: RenderHandle,
    old: &SpriteInstanceDescriptor,
    new: &SpriteInstanceDescriptor,
    transform: Option<Transform>,
    visible: Option<bool>,
    metadata: Option<RenderMetadata>,
    frames: bool,
) {
    if transform.is_some() || metadata.is_some() {
        operations.push(RenderDiff::Update {
            handle,
            transform,
            material: None,
            visible: None,
            metadata,
        });
    }
    let frame = (frames && old.frame != new.frame).then_some(new.frame);
    if frame.is_some()
        || old.tint != new.tint
        || old.render_order != new.render_order
        || visible.is_some()
    {
        operations.push(RenderDiff::UpdateSprite {
            handle,
            frame,
            tint: (old.tint != new.tint).then_some(new.tint),
            render_order: (old.render_order != new.render_order).then_some(new.render_order),
            visible,
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppearanceProjectionError {
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
    InvalidShader {
        id: String,
        source: render_model::RenderAssetError,
    },
    MissingTexture {
        owner: String,
        texture: String,
    },
    MissingMaterial {
        owner: String,
        material: String,
    },
    DuplicateObject {
        id: u64,
    },
    UnsafeObjectId {
        id: u64,
    },
    UnknownAppearance {
        id: u64,
        appearance: String,
    },
    JointAttachment {
        id: u64,
        joint: String,
        problem: JointAttachmentProblem,
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
    InvalidMaterialParameters {
        id: u64,
        slot: u16,
    },
    InvalidSprite {
        id: u64,
        source: render_model::SpriteError,
    },
    InvalidSpriteBatch {
        id: u64,
        source: render_model::SpriteBatchError,
    },
    DuplicateLight {
        id: u64,
    },
    UnsafeLightId {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointAttachmentProblem {
    NoParent,
    ParentNotAnimated,
    MissingJoint,
    AmbiguousJoint,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mesh appearance keeps its fact's layer, as primitives and sprites do,
    /// so a viewmodel mesh draws camera-local.
    #[test]
    fn mesh_instances_carry_the_appearance_layer() {
        let meshes = [
            Appearance::StaticMesh {
                asset: "mesh/held".to_owned(),
                material_overrides: Vec::new(),
                material_parameters: Default::default(),
            },
            Appearance::AnimatedMesh {
                asset: "animated/held".to_owned(),
                material_overrides: Vec::new(),
                playback: None,
                inspection: Default::default(),
                material_parameters: BTreeMap::new(),
            },
        ];
        for appearance in &meshes {
            let node = NodeValues {
                appearance,
                transform: Transform::IDENTITY,
                visible: true,
                layer: RenderLayer::Viewmodel,
                shadow_casting: Default::default(),
            };
            let layer = match create_node(RenderHandle::new(1), None, node, Default::default()) {
                RenderDiff::CreateStaticMeshInstance { instance, .. } => instance.layer,
                RenderDiff::CreateAnimatedMeshInstance { instance, .. } => instance.layer,
                other => panic!("unexpected op {other:?}"),
            };
            assert_eq!(layer, RenderLayer::Viewmodel);
        }
    }
}
