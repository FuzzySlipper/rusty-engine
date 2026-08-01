use std::collections::BTreeSet;

use core_assets::{AssetHash, AssetId, AssetReference, AssetVersionReq};
use core_ids::{
    EntityId, PrefabId, PrefabInstanceId, PrefabPartId, ProcessId, SceneId, SceneNodeId, SubjectId,
    TagId,
};
use core_math::Vec3;
use serde::{Deserialize, Serialize};

use crate::component::{
    ComponentRegistry, RegisteredComponentSnapshot, RegisteredComponentSnapshotError,
};
use crate::model::{
    AssetBindingComponent, BoundsComponent, CollisionComponent, ControllerComponent,
    EntityDefinition, EntityLifecycle, EntitySource, EntityState, EntityTransform,
    KinematicComponent, Quat, RenderableComponent, TransformComponent,
};

pub const ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EntityStateSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub entities: Vec<EntitySnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registered_components: Vec<RegisteredComponentSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EntitySnapshot {
    pub id: u64,
    pub name: String,
    pub lifecycle: SnapshotLifecycle,
    pub source: EntitySourceSnapshot,
    pub labels: Vec<u64>,
    pub transform: Option<TransformSnapshot>,
    pub bounds: Option<BoundsSnapshot>,
    pub collision: Option<CollisionSnapshot>,
    pub renderable: Option<RenderableSnapshot>,
    pub kinematic: Option<KinematicSnapshot>,
    pub controller: Option<ControllerSnapshot>,
    pub asset_binding: Option<AssetReferenceSnapshot>,
    pub transform_parent: Option<u64>,
    pub contained_in: Option<u64>,
    pub derived_from: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotLifecycle {
    Active,
    Disabled,
    Tombstoned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum EntitySourceSnapshot {
    AuthoredScene {
        scene: u64,
        node: u64,
    },
    RuntimeCreated {
        by: Option<u64>,
    },
    Imported {
        asset: AssetReferenceSnapshot,
    },
    PrefabInstance {
        prefab: u64,
        instance: u64,
        part: u64,
        role: Option<String>,
    },
    DiagnosticTooling,
    PolicyProposed {
        by: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TransformSnapshot {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BoundsSnapshot {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CollisionSnapshot {
    pub enabled: bool,
    pub static_collider: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RenderableSnapshot {
    pub visible: bool,
    pub asset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_transform: Option<TransformSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KinematicSnapshot {
    pub half_extents: [f32; 3],
    pub velocity: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ControllerSnapshot {
    Process { id: u64, active: bool },
    Subject { id: u64, active: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssetReferenceSnapshot {
    pub id: String,
    pub version: AssetVersionSnapshot,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AssetVersionSnapshot {
    Any,
    Exact { value: u32 },
    AtLeast { value: u32 },
}

#[derive(Debug)]
pub enum EntityStateSnapshotError {
    Encode(serde_json::Error),
    Decode(serde_json::Error),
    MissingSchema,
    UnsupportedSchema { actual: u64 },
    DuplicateEntity { entity: u64 },
    InvalidLifecycleState { entity: u64, reason: &'static str },
    InvalidAssetReference { entity: u64, reason: String },
    InvalidDefinition(crate::model::EntityDefinitionError),
    RegisteredComponent(RegisteredComponentSnapshotError),
}

impl std::fmt::Display for EntityStateSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EntityStateSnapshotError {}

impl From<RegisteredComponentSnapshotError> for EntityStateSnapshotError {
    fn from(value: RegisteredComponentSnapshotError) -> Self {
        Self::RegisteredComponent(value)
    }
}

impl EntityState {
    pub fn snapshot(&self) -> EntityStateSnapshot {
        self.snapshot_filtered(false)
    }

    pub fn durable_snapshot(&self) -> EntityStateSnapshot {
        self.snapshot_filtered(true)
    }

    fn snapshot_filtered(&self, durable: bool) -> EntityStateSnapshot {
        let included: BTreeSet<_> = self
            .entities
            .iter()
            .filter_map(|(entity, core)| {
                (!durable || !core.source.is_save_excluded_by_default()).then_some(*entity)
            })
            .collect();
        let entities = included
            .iter()
            .map(|entity| {
                let core = self.entities.get(entity).expect("included entity exists");
                let parent = self.transform_parents.get(entity).copied();
                let parent_included = parent.is_none_or(|value| included.contains(&value));
                let transform = if parent_included {
                    self.transform(*entity).copied()
                } else {
                    self.world_transform(*entity)
                        .map(TransformComponent::from_transform)
                };
                EntitySnapshot {
                    id: entity.raw(),
                    name: core.name.clone(),
                    lifecycle: lifecycle_to_snapshot(core.lifecycle),
                    source: source_to_snapshot(&core.source),
                    labels: core.labels.iter().map(|label| label.raw()).collect(),
                    transform: transform.map(transform_to_snapshot),
                    bounds: self.bounds(*entity).map(|value| BoundsSnapshot {
                        min: value.min.to_array(),
                        max: value.max.to_array(),
                    }),
                    collision: self.collision(*entity).map(|value| CollisionSnapshot {
                        enabled: value.enabled,
                        static_collider: value.static_collider,
                    }),
                    renderable: self.renderable(*entity).map(|value| RenderableSnapshot {
                        visible: value.visible,
                        asset: value.asset.clone(),
                        local_transform: (value.local_transform != EntityTransform::IDENTITY).then(
                            || {
                                transform_to_snapshot(TransformComponent::from_transform(
                                    value.local_transform,
                                ))
                            },
                        ),
                    }),
                    kinematic: self.kinematic(*entity).map(|value| KinematicSnapshot {
                        half_extents: value.half_extents.to_array(),
                        velocity: value.velocity.to_array(),
                    }),
                    controller: self.controller(*entity).map(|value| {
                        let active = !self.inactive_controllers.contains(entity);
                        match value {
                            ControllerComponent::Process(id) => ControllerSnapshot::Process {
                                id: id.raw(),
                                active,
                            },
                            ControllerComponent::Subject(id) => ControllerSnapshot::Subject {
                                id: id.raw(),
                                active,
                            },
                        }
                    }),
                    asset_binding: self
                        .asset_binding(*entity)
                        .map(|value| asset_to_snapshot(&value.asset)),
                    transform_parent: parent
                        .filter(|target| included.contains(target))
                        .map(EntityId::raw),
                    contained_in: self
                        .containment
                        .get(entity)
                        .copied()
                        .filter(|target| included.contains(target))
                        .map(EntityId::raw),
                    derived_from: self
                        .derived_from
                        .get(entity)
                        .copied()
                        .filter(|target| included.contains(target))
                        .map(EntityId::raw),
                }
            })
            .collect();
        EntityStateSnapshot {
            schema_version: ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION,
            revision: self.revision,
            entities,
            registered_components: self.components.durable_snapshots(&included),
        }
    }

    pub fn from_snapshot(snapshot: EntityStateSnapshot) -> Result<Self, EntityStateSnapshotError> {
        Self::from_snapshot_with_registry(snapshot, ComponentRegistry::default())
    }

    pub fn from_snapshot_with_registry(
        snapshot: EntityStateSnapshot,
        registry: ComponentRegistry,
    ) -> Result<Self, EntityStateSnapshotError> {
        if snapshot.schema_version != ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION {
            return Err(EntityStateSnapshotError::UnsupportedSchema {
                actual: u64::from(snapshot.schema_version),
            });
        }
        let mut ids = BTreeSet::new();
        let registered_components = snapshot.registered_components;
        let tombstones: BTreeSet<_> = snapshot
            .entities
            .iter()
            .filter_map(|entity| {
                (entity.lifecycle == SnapshotLifecycle::Tombstoned).then_some(entity.id)
            })
            .collect();
        let mut lifecycles = Vec::with_capacity(snapshot.entities.len());
        let mut inactive_controllers = Vec::new();
        let mut definitions = Vec::with_capacity(snapshot.entities.len());

        for entity in snapshot.entities {
            if !ids.insert(entity.id) {
                return Err(EntityStateSnapshotError::DuplicateEntity { entity: entity.id });
            }
            validate_tombstone_shape(&entity)?;
            for target in [entity.transform_parent, entity.contained_in]
                .into_iter()
                .flatten()
            {
                if tombstones.contains(&target) {
                    return Err(EntityStateSnapshotError::InvalidLifecycleState {
                        entity: entity.id,
                        reason: "relationship targets a tombstone",
                    });
                }
            }
            let id = EntityId::new(entity.id);
            let source = source_from_snapshot(entity.id, entity.source)?;
            let mut definition = EntityDefinition::new(id, entity.name)
                .with_source(source)
                .with_labels(entity.labels.into_iter().map(TagId::new));
            definition.transform = entity.transform.map(transform_from_snapshot);
            definition.bounds = entity.bounds.map(|value| BoundsComponent {
                min: vec3(value.min),
                max: vec3(value.max),
            });
            definition.collision = entity.collision.map(|value| CollisionComponent {
                enabled: value.enabled,
                static_collider: value.static_collider,
            });
            definition.renderable = entity.renderable.map(|value| RenderableComponent {
                visible: value.visible,
                asset: value.asset,
                local_transform: value
                    .local_transform
                    .map(transform_from_snapshot)
                    .map(TransformComponent::transform)
                    .unwrap_or(EntityTransform::IDENTITY),
            });
            definition.kinematic = entity.kinematic.map(|value| KinematicComponent {
                half_extents: vec3(value.half_extents),
                velocity: vec3(value.velocity),
            });
            if let Some(controller) = entity.controller {
                let (value, active) = match controller {
                    ControllerSnapshot::Process { id, active } => {
                        (ControllerComponent::Process(ProcessId::new(id)), active)
                    }
                    ControllerSnapshot::Subject { id, active } => {
                        (ControllerComponent::Subject(SubjectId::new(id)), active)
                    }
                };
                definition.controller = Some(value);
                if !active {
                    inactive_controllers.push(id);
                }
            }
            definition.asset_binding = entity
                .asset_binding
                .map(|value| {
                    asset_from_snapshot(entity.id, value)
                        .map(|asset| AssetBindingComponent { asset })
                })
                .transpose()?;
            definition.transform_parent = entity.transform_parent.map(EntityId::new);
            definition.contained_in = entity.contained_in.map(EntityId::new);
            definition.derived_from = entity.derived_from.map(EntityId::new);
            lifecycles.push((id, entity.lifecycle));
            definitions.push(definition);
        }

        let mut state = EntityState::from_definitions_with_registry(registry, definitions)
            .map_err(EntityStateSnapshotError::InvalidDefinition)?;
        state.revision = snapshot.revision;
        for entity in inactive_controllers {
            state.inactive_controllers.insert(entity);
        }
        for (entity, lifecycle) in lifecycles {
            state
                .entities
                .get_mut(&entity)
                .expect("snapshot definition created entity")
                .lifecycle = lifecycle_from_snapshot(lifecycle);
        }
        let known_entities = state.entities.keys().copied().collect();
        let tombstoned_entities = state
            .entities
            .iter()
            .filter_map(|(entity, core)| {
                (core.lifecycle == EntityLifecycle::Tombstoned).then_some(*entity)
            })
            .collect();
        state.components.restore_registered_snapshots(
            &registered_components,
            &known_entities,
            &tombstoned_entities,
        )?;
        Ok(state)
    }
}

pub fn encode_snapshot(state: &EntityState) -> Result<String, EntityStateSnapshotError> {
    serde_json::to_string_pretty(&state.snapshot()).map_err(EntityStateSnapshotError::Encode)
}

pub fn encode_durable_snapshot(state: &EntityState) -> Result<String, EntityStateSnapshotError> {
    serde_json::to_string_pretty(&state.durable_snapshot())
        .map_err(EntityStateSnapshotError::Encode)
}

pub fn decode_snapshot(input: &str) -> Result<EntityState, EntityStateSnapshotError> {
    decode_snapshot_with_registry(input, ComponentRegistry::default())
}

pub fn decode_snapshot_with_registry(
    input: &str,
    registry: ComponentRegistry,
) -> Result<EntityState, EntityStateSnapshotError> {
    let header: serde_json::Value =
        serde_json::from_str(input).map_err(EntityStateSnapshotError::Decode)?;
    let schema = header
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .ok_or(EntityStateSnapshotError::MissingSchema)?;
    match schema {
        value if value == u64::from(ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION) => {
            let snapshot: EntityStateSnapshot =
                serde_json::from_str(input).map_err(EntityStateSnapshotError::Decode)?;
            EntityState::from_snapshot_with_registry(snapshot, registry)
        }
        2 => {
            let legacy: LegacyEntityStateSnapshot =
                serde_json::from_str(input).map_err(EntityStateSnapshotError::Decode)?;
            EntityState::from_snapshot_with_registry(legacy.upgrade(), registry)
        }
        actual => Err(EntityStateSnapshotError::UnsupportedSchema { actual }),
    }
}

fn lifecycle_to_snapshot(value: EntityLifecycle) -> SnapshotLifecycle {
    match value {
        EntityLifecycle::Active => SnapshotLifecycle::Active,
        EntityLifecycle::Disabled => SnapshotLifecycle::Disabled,
        EntityLifecycle::Tombstoned => SnapshotLifecycle::Tombstoned,
    }
}

fn lifecycle_from_snapshot(value: SnapshotLifecycle) -> EntityLifecycle {
    match value {
        SnapshotLifecycle::Active => EntityLifecycle::Active,
        SnapshotLifecycle::Disabled => EntityLifecycle::Disabled,
        SnapshotLifecycle::Tombstoned => EntityLifecycle::Tombstoned,
    }
}

fn transform_to_snapshot(value: TransformComponent) -> TransformSnapshot {
    TransformSnapshot {
        translation: value.translation.to_array(),
        rotation: [
            value.rotation.x,
            value.rotation.y,
            value.rotation.z,
            value.rotation.w,
        ],
        scale: value.scale.to_array(),
    }
}

fn transform_from_snapshot(value: TransformSnapshot) -> TransformComponent {
    TransformComponent::from_transform(EntityTransform {
        translation: vec3(value.translation),
        rotation: Quat::new(
            value.rotation[0],
            value.rotation[1],
            value.rotation[2],
            value.rotation[3],
        ),
        scale: vec3(value.scale),
    })
}

fn source_to_snapshot(value: &EntitySource) -> EntitySourceSnapshot {
    match value {
        EntitySource::AuthoredScene { scene, node } => EntitySourceSnapshot::AuthoredScene {
            scene: scene.raw(),
            node: node.raw(),
        },
        EntitySource::RuntimeCreated { by } => EntitySourceSnapshot::RuntimeCreated {
            by: by.map(ProcessId::raw),
        },
        EntitySource::Imported { asset } => EntitySourceSnapshot::Imported {
            asset: asset_to_snapshot(asset),
        },
        EntitySource::PrefabInstance {
            prefab,
            instance,
            part,
            role,
        } => EntitySourceSnapshot::PrefabInstance {
            prefab: prefab.raw(),
            instance: instance.raw(),
            part: part.raw(),
            role: role.clone(),
        },
        EntitySource::DiagnosticTooling => EntitySourceSnapshot::DiagnosticTooling,
        EntitySource::PolicyProposed { by } => {
            EntitySourceSnapshot::PolicyProposed { by: by.raw() }
        }
    }
}

fn source_from_snapshot(
    entity: u64,
    value: EntitySourceSnapshot,
) -> Result<EntitySource, EntityStateSnapshotError> {
    Ok(match value {
        EntitySourceSnapshot::AuthoredScene { scene, node } => EntitySource::AuthoredScene {
            scene: SceneId::new(scene),
            node: SceneNodeId::new(node),
        },
        EntitySourceSnapshot::RuntimeCreated { by } => EntitySource::RuntimeCreated {
            by: by.map(ProcessId::new),
        },
        EntitySourceSnapshot::Imported { asset } => EntitySource::Imported {
            asset: asset_from_snapshot(entity, asset)?,
        },
        EntitySourceSnapshot::PrefabInstance {
            prefab,
            instance,
            part,
            role,
        } => EntitySource::PrefabInstance {
            prefab: PrefabId::new(prefab),
            instance: PrefabInstanceId::new(instance),
            part: PrefabPartId::new(part),
            role,
        },
        EntitySourceSnapshot::DiagnosticTooling => EntitySource::DiagnosticTooling,
        EntitySourceSnapshot::PolicyProposed { by } => EntitySource::PolicyProposed {
            by: SubjectId::new(by),
        },
    })
}

fn asset_to_snapshot(value: &AssetReference) -> AssetReferenceSnapshot {
    AssetReferenceSnapshot {
        id: value.id().as_str().to_string(),
        version: match value.version() {
            AssetVersionReq::Any => AssetVersionSnapshot::Any,
            AssetVersionReq::Exact(value) => AssetVersionSnapshot::Exact { value },
            AssetVersionReq::AtLeast(value) => AssetVersionSnapshot::AtLeast { value },
        },
        hash: value.hash().map(|hash| hash.as_str().to_string()),
    }
}

fn asset_from_snapshot(
    entity: u64,
    value: AssetReferenceSnapshot,
) -> Result<AssetReference, EntityStateSnapshotError> {
    let id = AssetId::parse(&value.id).map_err(|error| {
        EntityStateSnapshotError::InvalidAssetReference {
            entity,
            reason: error.to_string(),
        }
    })?;
    let version = match value.version {
        AssetVersionSnapshot::Any => AssetVersionReq::Any,
        AssetVersionSnapshot::Exact { value } => AssetVersionReq::Exact(value),
        AssetVersionSnapshot::AtLeast { value } => AssetVersionReq::AtLeast(value),
    };
    let hash = value
        .hash
        .map(|hash| AssetHash::parse(&hash))
        .transpose()
        .map_err(|error| EntityStateSnapshotError::InvalidAssetReference {
            entity,
            reason: error.to_string(),
        })?;
    Ok(AssetReference::new(id, version, hash))
}

fn validate_tombstone_shape(entity: &EntitySnapshot) -> Result<(), EntityStateSnapshotError> {
    if entity.lifecycle == SnapshotLifecycle::Tombstoned
        && (entity.transform.is_some()
            || entity.bounds.is_some()
            || entity.collision.is_some()
            || entity.renderable.is_some()
            || entity.kinematic.is_some()
            || entity.controller.is_some()
            || entity.asset_binding.is_some()
            || entity.transform_parent.is_some()
            || entity.contained_in.is_some()
            || entity.derived_from.is_some())
    {
        return Err(EntityStateSnapshotError::InvalidLifecycleState {
            entity: entity.id,
            reason: "tombstone carries components or relationships",
        });
    }
    Ok(())
}

fn vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacyEntityStateSnapshot {
    schema_version: u32,
    revision: u64,
    entities: Vec<LegacyEntitySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacyEntitySnapshot {
    id: u64,
    name: String,
    lifecycle: LegacySnapshotLifecycle,
    translation: Option<[f32; 3]>,
    collision: Option<CollisionSnapshot>,
    renderable: Option<RenderableSnapshot>,
    kinematic: Option<KinematicSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum LegacySnapshotLifecycle {
    Active,
    Disabled,
}

impl LegacyEntityStateSnapshot {
    fn upgrade(self) -> EntityStateSnapshot {
        debug_assert_eq!(self.schema_version, 2);
        EntityStateSnapshot {
            schema_version: ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION,
            revision: self.revision,
            entities: self
                .entities
                .into_iter()
                .map(|entity| EntitySnapshot {
                    id: entity.id,
                    name: entity.name,
                    lifecycle: match entity.lifecycle {
                        LegacySnapshotLifecycle::Active => SnapshotLifecycle::Active,
                        LegacySnapshotLifecycle::Disabled => SnapshotLifecycle::Disabled,
                    },
                    source: EntitySourceSnapshot::RuntimeCreated { by: None },
                    labels: Vec::new(),
                    transform: entity.translation.map(|translation| TransformSnapshot {
                        translation,
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        scale: [1.0, 1.0, 1.0],
                    }),
                    bounds: None,
                    collision: entity.collision,
                    renderable: entity.renderable,
                    kinematic: entity.kinematic,
                    controller: None,
                    asset_binding: None,
                    transform_parent: None,
                    contained_in: None,
                    derived_from: None,
                })
                .collect(),
            registered_components: Vec::new(),
        }
    }
}
