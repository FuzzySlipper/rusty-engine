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

use crate::resource_list::ResourceChanges;
use crate::{CatalogEntry, HandleAllocationError, ResourceList};

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
    /// How many retained materials and atlases name each texture.
    texture_users: BTreeMap<String, u32>,
    /// How many retained meshes' slots name each material.
    material_users: BTreeMap<String, u32>,
}

/// What one projection changed in the retained resources: each changed id,
/// in id order per kind, with the value it had before.
#[derive(Default)]
pub(crate) struct ResourceUpdate {
    materials: Vec<(String, Option<RenderMaterialDescriptor>)>,
    textures: Vec<(String, Option<TextureDescriptor>)>,
    shaders: Vec<(String, Option<ShaderDescriptor>)>,
    atlases: Vec<(String, Option<SpriteAtlasDescriptor>)>,
    static_meshes: Vec<(String, Option<Arc<StaticMeshAsset>>)>,
    animated_meshes: Vec<(String, Option<Arc<AnimatedMeshAsset>>)>,
}

impl AppearanceResources {
    /// Every entry counts as changed, for a projection that holds none.
    pub(crate) fn mark_whole(&mut self) {
        self.materials.mark_whole();
        self.textures.mark_whole();
        self.shaders.mark_whole();
        self.sprite_atlases.mark_whole();
        self.static_meshes.mark_whole();
        self.animated_meshes.mark_whole();
    }

    /// The projection holds every change so far.
    pub(crate) fn clear_changes(&mut self) {
        self.materials.clear_changes();
        self.textures.clear_changes();
        self.shaders.clear_changes();
        self.sprite_atlases.clear_changes();
        self.static_meshes.clear_changes();
        self.animated_meshes.clear_changes();
    }
}

/// Brings `retained` up to the catalog's changed entries, validating each
/// changed entry and the references to and from it. The work follows the
/// changes, not the catalog. On failure `retained` is as it was.
pub(crate) fn update_resources(
    input: &AppearanceResources,
    retained: &mut ResourceSnapshot,
) -> Result<ResourceUpdate, AppearanceProjectionError> {
    let mut update = ResourceUpdate::default();
    let applied = apply_changes(input, retained, &mut update);
    if applied.is_ok() {
        update.count_references(retained, true);
    }
    match applied.and_then(|()| check_references(&update, retained)) {
        Ok(()) => Ok(update),
        Err(error) => {
            update.undo(retained);
            Err(error)
        }
    }
}

fn apply_changes(
    input: &AppearanceResources,
    retained: &mut ResourceSnapshot,
    update: &mut ResourceUpdate,
) -> Result<(), AppearanceProjectionError> {
    apply_kind(
        &input.materials,
        &mut retained.materials,
        &mut update.materials,
        |a, b| a == b,
        |material| {
            material
                .validate()
                .map_err(|source| AppearanceProjectionError::InvalidMaterial {
                    id: material.id.clone(),
                    source,
                })
        },
    )?;
    apply_kind(
        &input.textures,
        &mut retained.textures,
        &mut update.textures,
        |a, b| a == b,
        |texture| {
            texture
                .validate()
                .map_err(|source| AppearanceProjectionError::InvalidTexture {
                    id: texture.id.clone(),
                    source,
                })
        },
    )?;
    apply_kind(
        &input.shaders,
        &mut retained.shaders,
        &mut update.shaders,
        |a, b| a == b,
        |shader| {
            shader
                .validate()
                .map_err(|source| AppearanceProjectionError::InvalidShader {
                    id: shader.id.clone(),
                    source,
                })
        },
    )?;
    apply_kind(
        &input.sprite_atlases,
        &mut retained.atlases,
        &mut update.atlases,
        |a, b| a == b,
        |atlas| {
            atlas
                .validate()
                .map_err(|source| AppearanceProjectionError::InvalidAtlas {
                    id: atlas.id.clone(),
                    source,
                })
        },
    )?;
    // A mesh body is validated when it first changes; the same shared
    // allocation is not compared or scanned again.
    apply_kind(
        &input.static_meshes,
        &mut retained.static_meshes,
        &mut update.static_meshes,
        |a, b| unchanged(Some(a), b),
        |mesh| {
            mesh.validate()
                .map_err(|source| AppearanceProjectionError::InvalidStaticMesh {
                    id: mesh.asset.clone(),
                    source,
                })
        },
    )?;
    apply_kind(
        &input.animated_meshes,
        &mut retained.animated_meshes,
        &mut update.animated_meshes,
        |a, b| unchanged(Some(a), b),
        |mesh| {
            mesh.validate()
                .map_err(|source| AppearanceProjectionError::InvalidAnimatedMesh {
                    id: mesh.asset.clone(),
                    source,
                })
        },
    )
}

/// Applies one kind's changed entries to `retained`, recording each with
/// the value it replaced.
fn apply_kind<T: Clone + CatalogEntry>(
    input: &ResourceList<T>,
    retained: &mut BTreeMap<String, T>,
    record: &mut Vec<(String, Option<T>)>,
    same: impl Fn(&T, &T) -> bool,
    validate: impl Fn(&T) -> Result<(), AppearanceProjectionError>,
) -> Result<(), AppearanceProjectionError> {
    let ids: BTreeSet<String> = match input.changes() {
        ResourceChanges::Ids(ids) => ids.clone(),
        ResourceChanges::Whole => input
            .iter()
            .map(|entry| entry.catalog_id().to_owned())
            .chain(retained.keys().cloned())
            .collect(),
    };
    for id in ids {
        let next = input.get(&id);
        match (retained.get(&id), next) {
            (None, None) => continue,
            (Some(previous), Some(next)) if same(previous, next) => continue,
            _ => {}
        }
        let previous = match next {
            Some(next) => {
                validate(next)?;
                retained.insert(id.clone(), next.clone())
            }
            None => retained.remove(&id),
        };
        record.push((id, previous));
    }
    Ok(())
}

impl ResourceUpdate {
    /// Adds (or with `add` false, takes back) the references the changed
    /// entries make now in place of those they made before.
    fn count_references(&self, retained: &mut ResourceSnapshot, add: bool) {
        let adjust = |users: &mut BTreeMap<String, u32>, id: &str, more: bool| {
            let count = users.entry(id.to_owned()).or_default();
            if more {
                *count += 1;
            } else {
                *count -= 1;
            }
            if *count == 0 {
                users.remove(id);
            }
        };
        for (id, previous) in &self.materials {
            let next = retained.materials.get(id).and_then(|m| m.texture.clone());
            let previous = previous.as_ref().and_then(|m| m.texture.clone());
            if let Some(texture) = &previous {
                adjust(&mut retained.texture_users, texture, !add);
            }
            if let Some(texture) = &next {
                adjust(&mut retained.texture_users, texture, add);
            }
        }
        for (id, previous) in &self.atlases {
            let next = retained.atlases.get(id).map(|atlas| atlas.texture.clone());
            if let Some(previous) = previous {
                adjust(&mut retained.texture_users, &previous.texture, !add);
            }
            if let Some(texture) = &next {
                adjust(&mut retained.texture_users, texture, add);
            }
        }
        let slots = |slots: Option<&[MeshMaterialSlot]>| -> Vec<String> {
            slots
                .into_iter()
                .flatten()
                .map(|slot| slot.material.clone())
                .collect()
        };
        let mut mesh_slots = Vec::new();
        for (id, previous) in &self.static_meshes {
            mesh_slots.push((
                slots(
                    previous
                        .as_deref()
                        .map(|mesh| mesh.material_slots.as_slice()),
                ),
                slots(
                    retained
                        .static_meshes
                        .get(id)
                        .map(|mesh| mesh.material_slots.as_slice()),
                ),
            ));
        }
        for (id, previous) in &self.animated_meshes {
            mesh_slots.push((
                slots(
                    previous
                        .as_deref()
                        .map(|mesh| mesh.material_slots.as_slice()),
                ),
                slots(
                    retained
                        .animated_meshes
                        .get(id)
                        .map(|mesh| mesh.material_slots.as_slice()),
                ),
            ));
        }
        for (previous, next) in mesh_slots {
            for material in &previous {
                adjust(&mut retained.material_users, material, !add);
            }
            for material in &next {
                adjust(&mut retained.material_users, material, add);
            }
        }
    }

    /// Puts `retained` back as it was before this update.
    pub(crate) fn undo(self, retained: &mut ResourceSnapshot) {
        self.count_references(retained, false);
        fn restore<T>(retained: &mut BTreeMap<String, T>, record: Vec<(String, Option<T>)>) {
            for (id, previous) in record.into_iter().rev() {
                match previous {
                    Some(previous) => retained.insert(id, previous),
                    None => retained.remove(&id),
                };
            }
        }
        restore(&mut retained.materials, self.materials);
        restore(&mut retained.textures, self.textures);
        restore(&mut retained.shaders, self.shaders);
        restore(&mut retained.atlases, self.atlases);
        restore(&mut retained.static_meshes, self.static_meshes);
        restore(&mut retained.animated_meshes, self.animated_meshes);
    }

    /// Meshes whose definition changed while staying retained: their
    /// instances are created again.
    pub(crate) fn redefined_static_meshes(&self, retained: &ResourceSnapshot) -> BTreeSet<String> {
        redefined(&self.static_meshes, &retained.static_meshes)
    }

    pub(crate) fn redefined_animated_meshes(
        &self,
        retained: &ResourceSnapshot,
    ) -> BTreeSet<String> {
        redefined(&self.animated_meshes, &retained.animated_meshes)
    }
}

fn redefined<T>(
    record: &[(String, Option<T>)],
    retained: &BTreeMap<String, T>,
) -> BTreeSet<String> {
    record
        .iter()
        .filter(|(id, previous)| previous.is_some() && retained.contains_key(id))
        .map(|(id, _)| id.clone())
        .collect()
}

/// Every changed entry's references resolve, and no removed texture or
/// material is still named.
fn check_references(
    update: &ResourceUpdate,
    retained: &ResourceSnapshot,
) -> Result<(), AppearanceProjectionError> {
    for (id, _) in &update.materials {
        if let Some(texture) = retained
            .materials
            .get(id)
            .and_then(|material| material.texture.as_ref())
            .filter(|texture| !retained.textures.contains_key(*texture))
        {
            return Err(AppearanceProjectionError::MissingTexture {
                owner: id.clone(),
                texture: texture.clone(),
            });
        }
    }
    for (id, _) in &update.atlases {
        if let Some(atlas) = retained
            .atlases
            .get(id)
            .filter(|atlas| !retained.textures.contains_key(&atlas.texture))
        {
            return Err(AppearanceProjectionError::MissingTexture {
                owner: id.clone(),
                texture: atlas.texture.clone(),
            });
        }
    }
    for (id, _) in &update.textures {
        if !retained.textures.contains_key(id) && retained.texture_users.contains_key(id) {
            // Refused, so the owner is found the slow way.
            let owner = retained
                .materials
                .values()
                .find(|material| material.texture.as_ref() == Some(id))
                .map(|material| material.id.clone())
                .or_else(|| {
                    retained
                        .atlases
                        .values()
                        .find(|atlas| &atlas.texture == id)
                        .map(|atlas| atlas.id.clone())
                })
                .unwrap_or_default();
            return Err(AppearanceProjectionError::MissingTexture {
                owner,
                texture: id.clone(),
            });
        }
    }
    for (id, _) in &update.static_meshes {
        if let Some(mesh) = retained.static_meshes.get(id) {
            validate_material_references(&mesh.asset, &mesh.material_slots, &retained.materials)?;
        }
    }
    for (id, _) in &update.animated_meshes {
        if let Some(mesh) = retained.animated_meshes.get(id) {
            validate_material_references(&mesh.asset, &mesh.material_slots, &retained.materials)?;
        }
    }
    for (id, _) in &update.materials {
        if !retained.materials.contains_key(id) && retained.material_users.contains_key(id) {
            let owner = retained
                .static_meshes
                .values()
                .map(|mesh| (&mesh.asset, &mesh.material_slots))
                .chain(
                    retained
                        .animated_meshes
                        .values()
                        .map(|mesh| (&mesh.asset, &mesh.material_slots)),
                )
                .find(|(_, slots)| slots.iter().any(|slot| &slot.material == id))
                .map(|(asset, _)| asset.clone())
                .unwrap_or_default();
            return Err(AppearanceProjectionError::MissingMaterial {
                owner,
                material: id.clone(),
            });
        }
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

/// The definitions and releases that take the renderer to `retained`
/// after `update`: definitions by kind (each kind in id order), then
/// releases with dependents before their sources.
pub(crate) fn resource_diffs(
    update: &ResourceUpdate,
    retained: &ResourceSnapshot,
) -> Vec<RenderDiff> {
    let mut operations = Vec::new();
    for (id, _) in &update.textures {
        if let Some(texture) = retained.textures.get(id) {
            operations.push(RenderDiff::DefineTexture {
                texture: texture.clone(),
            });
        }
    }
    for (id, _) in &update.shaders {
        if let Some(shader) = retained.shaders.get(id) {
            operations.push(RenderDiff::DefineShader {
                shader: shader.clone(),
            });
        }
    }
    for (id, _) in &update.materials {
        if let Some(material) = retained.materials.get(id) {
            operations.push(RenderDiff::DefineMaterial {
                material: material.clone(),
            });
        }
    }
    for (id, _) in &update.atlases {
        if let Some(atlas) = retained.atlases.get(id) {
            operations.push(RenderDiff::DefineSpriteAtlas {
                atlas: atlas.clone(),
            });
        }
    }
    for (id, _) in &update.static_meshes {
        if let Some(mesh) = retained.static_meshes.get(id) {
            operations.push(RenderDiff::DefineStaticMesh {
                asset: StaticMeshAsset::clone(mesh),
            });
        }
    }
    for (id, _) in &update.animated_meshes {
        if let Some(mesh) = retained.animated_meshes.get(id) {
            operations.push(RenderDiff::DefineAnimatedMesh {
                asset: AnimatedMeshAsset::clone(mesh),
            });
        }
    }
    // Replacements may stop referring to a removed dependency. Install them
    // before releasing old definitions, then retire dependents before sources.
    fn released<'a, T, U>(
        record: &'a [(String, Option<T>)],
        retained: &'a BTreeMap<String, U>,
    ) -> impl Iterator<Item = String> + 'a {
        record
            .iter()
            .filter(|(id, previous)| previous.is_some() && !retained.contains_key(id))
            .map(|(id, _)| id.clone())
    }
    operations.extend(
        released(&update.static_meshes, &retained.static_meshes)
            .map(|asset| RenderDiff::ReleaseStaticMesh { asset }),
    );
    operations.extend(
        released(&update.animated_meshes, &retained.animated_meshes)
            .map(|asset| RenderDiff::ReleaseAnimatedMesh { asset }),
    );
    operations.extend(
        released(&update.atlases, &retained.atlases)
            .map(|id| RenderDiff::ReleaseSpriteAtlas { id }),
    );
    operations.extend(
        released(&update.materials, &retained.materials)
            .map(|id| RenderDiff::ReleaseMaterial { id }),
    );
    operations.extend(
        released(&update.textures, &retained.textures).map(|id| RenderDiff::ReleaseTexture { id }),
    );
    operations.extend(
        released(&update.shaders, &retained.shaders).map(|id| RenderDiff::ReleaseShader { id }),
    );
    operations
}

fn unchanged<T: PartialEq>(previous: Option<&Arc<T>>, next: &Arc<T>) -> bool {
    previous.is_some_and(|previous| Arc::ptr_eq(previous, next) || **previous == **next)
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
