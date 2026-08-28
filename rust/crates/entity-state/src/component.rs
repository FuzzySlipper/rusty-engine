use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use core_ids::EntityId;

use crate::components::{
    AssetBindingComponent, BoundsComponent, CollisionComponent, ControllerComponent,
    KinematicComponent, RenderableComponent, TransformComponent,
};

mod registration;

use registration::RegistrationPersistence;
pub use registration::{
    ComponentAccessError, ComponentCodec, ComponentCodecError, ComponentIdentityError,
    ComponentKindInspection, ComponentPersistence, ComponentRegistration,
    ComponentRegistrationError, ComponentRevision, ComponentStoreInspection, ComponentTypeId,
    ComponentValueSnapshot, EntityComponent, RegisteredComponentSnapshot,
    RegisteredComponentSnapshotError, ASSET_BINDING_COMPONENT_TYPE_ID, BOUNDS_COMPONENT_TYPE_ID,
    COLLISION_COMPONENT_TYPE_ID, CONTROLLER_COMPONENT_TYPE_ID, KINEMATIC_COMPONENT_TYPE_ID,
    MAX_COMPONENT_CODEC_ID_BYTES, MAX_COMPONENT_INSPECTION_ENTITIES, MAX_COMPONENT_TYPE_ID_BYTES,
    MAX_REGISTERED_COMPONENT_TYPES, RENDERABLE_COMPONENT_TYPE_ID, TRANSFORM_COMPONENT_TYPE_ID,
};

trait ErasedComponentTable: Send + Sync {
    fn clone_box(&self) -> Box<dyn ErasedComponentTable>;
    fn empty_box(&self) -> Box<dyn ErasedComponentTable>;
    fn rust_type_id(&self) -> TypeId;
    fn rust_type_name(&self) -> &'static str;
    fn component_type_id(&self) -> &ComponentTypeId;
    fn persistence(&self) -> ComponentPersistence;
    fn codec_signature(&self) -> Option<(&'static str, u32)>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn contains_entity(&self, entity: EntityId) -> bool;
    fn remove_entity(&mut self, entity: EntityId) -> bool;
    fn clear_revisions(&mut self);
    /// Rebase every entity slot, including absent values, past both this table's and the
    /// supplied live table's observed revisions. This is used by an in-process restore: a
    /// restored value must never make either a snapshot-era or pre-restore exact guard valid.
    fn rebase_revisions(
        &mut self,
        current: &dyn ErasedComponentTable,
        entities: &BTreeSet<EntityId>,
        persisted_revisions: &BTreeMap<(EntityId, ComponentTypeId), u64>,
    ) -> bool;
    fn len(&self) -> usize;
    fn entity_sample(&self) -> Vec<EntityId>;
    fn durable_snapshot(
        &self,
        included: &BTreeSet<EntityId>,
    ) -> Option<RegisteredComponentSnapshot>;
    fn restore_snapshot(
        &mut self,
        snapshot: &RegisteredComponentSnapshot,
        known_entities: &BTreeSet<EntityId>,
        tombstones: &BTreeSet<EntityId>,
    ) -> Result<(), RegisteredComponentSnapshotError>;
}

impl Clone for Box<dyn ErasedComponentTable> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[derive(Clone)]
struct ComponentTable<T: EntityComponent> {
    registration: ComponentRegistration<T>,
    values: BTreeMap<EntityId, T>,
    revisions: BTreeMap<EntityId, u64>,
}

impl<T: EntityComponent> ComponentTable<T> {
    fn new(registration: ComponentRegistration<T>) -> Self {
        Self {
            registration,
            values: BTreeMap::new(),
            revisions: BTreeMap::new(),
        }
    }

    fn bump_revision(&mut self, entity: EntityId) {
        let revision = self.revisions.entry(entity).or_default();
        *revision = revision.saturating_add(1);
    }
}

impl<T: EntityComponent> ErasedComponentTable for ComponentTable<T> {
    fn clone_box(&self) -> Box<dyn ErasedComponentTable> {
        Box::new(self.clone())
    }

    fn empty_box(&self) -> Box<dyn ErasedComponentTable> {
        Box::new(Self::new(self.registration.clone()))
    }

    fn rust_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn rust_type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn component_type_id(&self) -> &ComponentTypeId {
        self.registration.type_id()
    }

    fn persistence(&self) -> ComponentPersistence {
        self.registration.persistence()
    }

    fn codec_signature(&self) -> Option<(&'static str, u32)> {
        self.registration.codec_signature()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn contains_entity(&self, entity: EntityId) -> bool {
        self.values.contains_key(&entity)
    }

    fn remove_entity(&mut self, entity: EntityId) -> bool {
        let removed = self.values.remove(&entity).is_some();
        if removed {
            self.bump_revision(entity);
        }
        removed
    }

    fn clear_revisions(&mut self) {
        self.revisions.clear();
    }

    fn rebase_revisions(
        &mut self,
        current: &dyn ErasedComponentTable,
        entities: &BTreeSet<EntityId>,
        persisted_revisions: &BTreeMap<(EntityId, ComponentTypeId), u64>,
    ) -> bool {
        let Some(current) = current.as_any().downcast_ref::<ComponentTable<T>>() else {
            return false;
        };
        for entity in entities {
            let snapshot_revision = self.revisions.get(entity).copied().unwrap_or(0);
            let current_revision = current.revisions.get(entity).copied().unwrap_or(0);
            let persisted_revision = persisted_revisions
                .get(&(*entity, self.registration.type_id().clone()))
                .copied()
                .unwrap_or(0);
            let Some(remapped) = snapshot_revision
                .max(current_revision)
                .max(persisted_revision)
                .checked_add(1)
            else {
                return false;
            };
            self.revisions.insert(*entity, remapped);
        }
        true
    }

    fn len(&self) -> usize {
        self.values.len()
    }

    fn entity_sample(&self) -> Vec<EntityId> {
        self.values
            .keys()
            .copied()
            .take(MAX_COMPONENT_INSPECTION_ENTITIES)
            .collect()
    }

    fn durable_snapshot(
        &self,
        included: &BTreeSet<EntityId>,
    ) -> Option<RegisteredComponentSnapshot> {
        let RegistrationPersistence::Durable(codec) = &self.registration.persistence else {
            return None;
        };
        let values: Vec<_> = self
            .values
            .iter()
            .filter(|(entity, _)| included.contains(entity))
            .map(|(entity, value)| ComponentValueSnapshot {
                entity: entity.raw(),
                value: (codec.encode)(value),
            })
            .collect();
        (!values.is_empty()).then(|| RegisteredComponentSnapshot {
            type_id: self.registration.type_id.as_str().to_string(),
            codec: codec.identity.to_string(),
            version: codec.version,
            required: true,
            values,
        })
    }

    fn restore_snapshot(
        &mut self,
        snapshot: &RegisteredComponentSnapshot,
        known_entities: &BTreeSet<EntityId>,
        tombstones: &BTreeSet<EntityId>,
    ) -> Result<(), RegisteredComponentSnapshotError> {
        let component = self.registration.type_id.clone();
        let RegistrationPersistence::Durable(codec) = &self.registration.persistence else {
            return Err(RegisteredComponentSnapshotError::PersistenceMismatch { component });
        };
        let version_is_current = snapshot.version == codec.version;
        let version_is_migratable = snapshot.version < codec.version && codec.migrate.is_some();
        if snapshot.codec != codec.identity || (!version_is_current && !version_is_migratable) {
            return Err(RegisteredComponentSnapshotError::CodecMismatch {
                component,
                expected_codec: codec.identity.to_string(),
                expected_version: codec.version,
                actual_codec: snapshot.codec.clone(),
                actual_version: snapshot.version,
            });
        }

        let mut decoded = BTreeMap::new();
        for item in &snapshot.values {
            let entity = EntityId::new(item.entity);
            if !known_entities.contains(&entity) {
                return Err(RegisteredComponentSnapshotError::UnknownEntity {
                    component: component.clone(),
                    entity,
                });
            }
            if tombstones.contains(&entity) {
                return Err(RegisteredComponentSnapshotError::TombstonedEntity {
                    component: component.clone(),
                    entity,
                });
            }
            if decoded.contains_key(&entity) || self.values.contains_key(&entity) {
                return Err(RegisteredComponentSnapshotError::DuplicateEntityValue {
                    component: component.clone(),
                    entity,
                });
            }
            let value = if version_is_current {
                (codec.decode)(item.value.clone())
            } else {
                // The version check above guarantees a migrator exists here.
                (codec
                    .migrate
                    .expect("migratable codec has a migration hook"))(
                    snapshot.version,
                    item.value.clone(),
                )
            }
            .map_err(|reason| RegisteredComponentSnapshotError::DecodeFailed {
                component: component.clone(),
                entity,
                reason,
            })?;
            (self.registration.validator)(&value).map_err(|reason| {
                RegisteredComponentSnapshotError::InvalidValue {
                    component: component.clone(),
                    entity,
                    reason,
                }
            })?;
            decoded.insert(entity, value);
        }
        self.values.extend(decoded);
        Ok(())
    }
}

#[derive(Clone)]
pub struct ComponentRegistry {
    prototypes: BTreeMap<ComponentTypeId, Box<dyn ErasedComponentTable>>,
    rust_types: HashMap<TypeId, ComponentTypeId>,
}

impl fmt::Debug for ComponentRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComponentRegistry")
            .field("registered_kind_count", &self.prototypes.len())
            .field("type_ids", &self.prototypes.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        let mut registry = Self {
            prototypes: BTreeMap::new(),
            rust_types: HashMap::new(),
        };
        register_builtin_components(&mut registry)
            .expect("built-in component registrations are unique and bounded");
        registry
    }
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: EntityComponent>(
        &mut self,
        registration: ComponentRegistration<T>,
    ) -> Result<(), ComponentRegistrationError> {
        let requested_id = registration.type_id.clone();
        let requested_rust_type = std::any::type_name::<T>();
        let requested_type_id = TypeId::of::<T>();

        if let Some(existing) = self.prototypes.get(&requested_id) {
            if existing.rust_type_id() != requested_type_id {
                return Err(ComponentRegistrationError::StableIdConflict {
                    component: requested_id,
                    registered_rust_type: existing.rust_type_name(),
                    requested_rust_type,
                });
            }
            if existing.persistence() != registration.persistence()
                || existing.codec_signature() != registration.codec_signature()
            {
                return Err(ComponentRegistrationError::IncompatibleCodec {
                    component: requested_id,
                });
            }
            return Err(ComponentRegistrationError::DuplicateStableId {
                component: requested_id,
            });
        }
        if let Some(existing_id) = self.rust_types.get(&requested_type_id) {
            return Err(ComponentRegistrationError::RustTypeConflict {
                rust_type: requested_rust_type,
                registered_component: existing_id.clone(),
                requested_component: requested_id,
            });
        }
        if self.prototypes.len() >= MAX_REGISTERED_COMPONENT_TYPES {
            return Err(ComponentRegistrationError::TypeLimitExceeded {
                limit: MAX_REGISTERED_COMPONENT_TYPES,
            });
        }

        self.rust_types
            .insert(requested_type_id, requested_id.clone());
        self.prototypes
            .insert(requested_id, Box::new(ComponentTable::new(registration)));
        Ok(())
    }

    fn instantiate(&self) -> ComponentStore {
        ComponentStore {
            tables: self
                .prototypes
                .iter()
                .map(|(id, table)| (id.clone(), table.empty_box()))
                .collect(),
            rust_types: self.rust_types.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ComponentStore {
    tables: BTreeMap<ComponentTypeId, Box<dyn ErasedComponentTable>>,
    rust_types: HashMap<TypeId, ComponentTypeId>,
}

impl fmt::Debug for ComponentStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComponentStore")
            .field("registered_kind_count", &self.tables.len())
            .field(
                "component_count",
                &self.tables.values().map(|table| table.len()).sum::<usize>(),
            )
            .finish()
    }
}

impl Default for ComponentStore {
    fn default() -> Self {
        ComponentRegistry::default().instantiate()
    }
}

impl ComponentStore {
    pub(crate) fn from_registry(registry: &ComponentRegistry) -> Self {
        registry.instantiate()
    }

    pub(crate) fn register<T: EntityComponent>(
        &mut self,
        registration: ComponentRegistration<T>,
    ) -> Result<(), ComponentRegistrationError> {
        let mut registry = self.registry();
        registry.register(registration)?;
        *self = self.with_registry(registry);
        Ok(())
    }

    pub(crate) fn type_id_for<T: EntityComponent>(
        &self,
    ) -> Result<&ComponentTypeId, ComponentAccessError> {
        self.rust_types
            .get(&TypeId::of::<T>())
            .ok_or(ComponentAccessError::UnregisteredRustType {
                rust_type: std::any::type_name::<T>(),
            })
    }

    pub(crate) fn get<T: EntityComponent>(
        &self,
        entity: EntityId,
    ) -> Result<Option<&T>, ComponentAccessError> {
        Ok(self.table::<T>()?.values.get(&entity))
    }

    pub(crate) fn has<T: EntityComponent>(
        &self,
        entity: EntityId,
    ) -> Result<bool, ComponentAccessError> {
        Ok(self.table::<T>()?.values.contains_key(&entity))
    }

    pub(crate) fn iter<T: EntityComponent>(
        &self,
    ) -> Result<ComponentIter<'_, T>, ComponentAccessError> {
        Ok(ComponentIter {
            inner: self.table::<T>()?.values.iter(),
        })
    }

    pub(crate) fn validate<T: EntityComponent>(
        &self,
        value: &T,
    ) -> Result<(), ComponentValueError> {
        let table = self.table::<T>()?;
        (table.registration.validator)(value).map_err(|reason| ComponentValueError { reason })
    }

    pub(crate) fn revision<T: EntityComponent>(
        &self,
        entity: EntityId,
    ) -> Result<u64, ComponentAccessError> {
        Ok(self
            .table::<T>()?
            .revisions
            .get(&entity)
            .copied()
            .unwrap_or(0))
    }

    pub(crate) fn insert_unchecked<T: EntityComponent>(&mut self, entity: EntityId, value: T) {
        let table = self.table_mut::<T>().expect("component type registered");
        table.values.insert(entity, value);
        table.bump_revision(entity);
    }

    pub(crate) fn remove_unchecked<T: EntityComponent>(&mut self, entity: EntityId) -> Option<T> {
        let table = self.table_mut::<T>().expect("component type registered");
        let removed = table.values.remove(&entity);
        if removed.is_some() {
            table.bump_revision(entity);
        }
        removed
    }

    pub(crate) fn reset_revisions(&mut self) {
        for table in self.tables.values_mut() {
            table.clear_revisions();
        }
    }

    /// Moves every known entity/component slot to a revision strictly newer than either input.
    /// Component absence is a guarded fact, so absent slots are deliberately seeded too.
    pub(crate) fn rebase_revisions_from(
        &mut self,
        current: &Self,
        entities: &BTreeSet<EntityId>,
        persisted_revisions: &BTreeMap<(EntityId, ComponentTypeId), u64>,
    ) -> bool {
        if self.tables.len() != current.tables.len() || self.tables.keys().ne(current.tables.keys())
        {
            return false;
        }
        self.tables.iter_mut().all(|(type_id, table)| {
            current.tables.get(type_id).is_some_and(|current_table| {
                table.rebase_revisions(current_table.as_ref(), entities, persisted_revisions)
            })
        })
    }

    /// Rebases only the supplied component slots.  Unlike the whole-world
    /// restore path, this preserves revision evidence for unrelated component
    /// families and entities while still treating absence as a guarded slot.
    pub(crate) fn rebase_component_revisions_from(
        &mut self,
        current: &Self,
        persisted_revisions: &BTreeMap<(EntityId, ComponentTypeId), u64>,
    ) -> bool {
        if self.tables.len() != current.tables.len() || self.tables.keys().ne(current.tables.keys())
        {
            return false;
        }
        self.tables.iter_mut().all(|(type_id, table)| {
            let entities = persisted_revisions
                .keys()
                .filter_map(|(entity, component)| (component == type_id).then_some(*entity))
                .collect::<BTreeSet<_>>();
            entities.is_empty()
                || current.tables.get(type_id).is_some_and(|current_table| {
                    table.rebase_revisions(current_table.as_ref(), &entities, persisted_revisions)
                })
        })
    }

    pub(crate) fn remove_entity(&mut self, entity: EntityId) {
        for table in self.tables.values_mut() {
            table.remove_entity(entity);
        }
    }

    pub(crate) fn inspection(&self) -> ComponentStoreInspection {
        let kinds = self
            .tables
            .values()
            .map(|table| {
                let count = table.len();
                let entity_sample = table.entity_sample();
                ComponentKindInspection {
                    type_id: table.component_type_id().clone(),
                    persistence: table.persistence(),
                    count,
                    entity_sample_truncated: count > entity_sample.len(),
                    entity_sample,
                }
            })
            .collect();
        ComponentStoreInspection {
            registered_kind_count: self.tables.len(),
            kinds,
        }
    }

    pub(crate) fn types_for_entity(&self, entity: EntityId) -> Vec<ComponentTypeId> {
        self.tables
            .values()
            .filter(|table| table.contains_entity(entity))
            .map(|table| table.component_type_id().clone())
            .collect()
    }

    pub(crate) fn durable_snapshots(
        &self,
        included: &BTreeSet<EntityId>,
    ) -> Vec<RegisteredComponentSnapshot> {
        self.tables
            .values()
            .filter_map(|table| table.durable_snapshot(included))
            .collect()
    }

    pub(crate) fn restore_registered_snapshots(
        &mut self,
        snapshots: &[RegisteredComponentSnapshot],
        known_entities: &BTreeSet<EntityId>,
        tombstones: &BTreeSet<EntityId>,
    ) -> Result<(), RegisteredComponentSnapshotError> {
        let mut seen = BTreeSet::new();
        for snapshot in snapshots {
            let component = ComponentTypeId::parse(snapshot.type_id.clone()).map_err(|error| {
                RegisteredComponentSnapshotError::InvalidTypeId {
                    value: error.value,
                    reason: error.reason,
                }
            })?;
            if !seen.insert(component.clone()) {
                return Err(RegisteredComponentSnapshotError::DuplicateType { component });
            }
            let Some(table) = self.tables.get_mut(&component) else {
                if snapshot.required {
                    return Err(RegisteredComponentSnapshotError::UnknownRequiredType {
                        component,
                    });
                }
                continue;
            };
            table.restore_snapshot(snapshot, known_entities, tombstones)?;
        }
        Ok(())
    }

    fn table<T: EntityComponent>(&self) -> Result<&ComponentTable<T>, ComponentAccessError> {
        let id = self.type_id_for::<T>()?;
        Ok(self
            .tables
            .get(id)
            .and_then(|table| table.as_any().downcast_ref::<ComponentTable<T>>())
            .expect("registered Rust type and table agree"))
    }

    fn table_mut<T: EntityComponent>(
        &mut self,
    ) -> Result<&mut ComponentTable<T>, ComponentAccessError> {
        let id = self.type_id_for::<T>()?.clone();
        Ok(self
            .tables
            .get_mut(&id)
            .and_then(|table| table.as_any_mut().downcast_mut::<ComponentTable<T>>())
            .expect("registered Rust type and table agree"))
    }

    fn registry(&self) -> ComponentRegistry {
        ComponentRegistry {
            prototypes: self
                .tables
                .iter()
                .map(|(id, table)| (id.clone(), table.empty_box()))
                .collect(),
            rust_types: self.rust_types.clone(),
        }
    }

    fn with_registry(&self, registry: ComponentRegistry) -> Self {
        let mut next = registry.instantiate();
        for (id, table) in &self.tables {
            if let Some(destination) = next.tables.get_mut(id) {
                *destination = table.clone_box();
            }
        }
        next
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentValueError {
    pub reason: String,
}

impl From<ComponentAccessError> for ComponentValueError {
    fn from(error: ComponentAccessError) -> Self {
        match error {
            ComponentAccessError::UnregisteredRustType { rust_type } => Self {
                reason: format!("component Rust type {rust_type} is not registered"),
            },
        }
    }
}

pub struct ComponentIter<'a, T: EntityComponent> {
    inner: std::collections::btree_map::Iter<'a, EntityId, T>,
}

impl<'a, T: EntityComponent> Iterator for ComponentIter<'a, T> {
    type Item = (EntityId, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(entity, value)| (*entity, value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T: EntityComponent> ExactSizeIterator for ComponentIter<'_, T> {}

fn register_builtin_components(
    registry: &mut ComponentRegistry,
) -> Result<(), ComponentRegistrationError> {
    registry.register(
        ComponentRegistration::<TransformComponent>::legacy_snapshot(builtin_id(
            TRANSFORM_COMPONENT_TYPE_ID,
        ))
        .with_validator(|value| {
            crate::definition::transform_is_valid(value.transform())
                .then_some(())
                .ok_or_else(|| "invalid transform".to_string())
        }),
    )?;
    registry.register(
        ComponentRegistration::<BoundsComponent>::legacy_snapshot(builtin_id(
            BOUNDS_COMPONENT_TYPE_ID,
        ))
        .with_validator(|value| {
            crate::definition::bounds_are_valid(*value)
                .then_some(())
                .ok_or_else(|| "invalid bounds".to_string())
        }),
    )?;
    registry.register(
        ComponentRegistration::<CollisionComponent>::legacy_snapshot(builtin_id(
            COLLISION_COMPONENT_TYPE_ID,
        )),
    )?;
    registry.register(
        ComponentRegistration::<RenderableComponent>::legacy_snapshot(builtin_id(
            RENDERABLE_COMPONENT_TYPE_ID,
        ))
        .with_validator(|value| {
            (!value.asset.trim().is_empty()
                && crate::definition::transform_is_valid(value.local_transform))
            .then_some(())
            .ok_or_else(|| "render asset is empty or local transform is invalid".to_string())
        }),
    )?;
    registry.register(
        ComponentRegistration::<KinematicComponent>::legacy_snapshot(builtin_id(
            KINEMATIC_COMPONENT_TYPE_ID,
        ))
        .with_validator(|value| {
            (crate::definition::half_extents_are_valid(value.half_extents)
                && crate::definition::velocity_is_valid(value.velocity))
            .then_some(())
            .ok_or_else(|| "invalid half extents or velocity".to_string())
        }),
    )?;
    registry.register(
        ComponentRegistration::<ControllerComponent>::legacy_snapshot(builtin_id(
            CONTROLLER_COMPONENT_TYPE_ID,
        )),
    )?;
    registry.register(
        ComponentRegistration::<AssetBindingComponent>::legacy_snapshot(builtin_id(
            ASSET_BINDING_COMPONENT_TYPE_ID,
        )),
    )?;
    registry.register(crate::rigid_body::rigid_body_registration())?;
    registry.register(crate::character_motion::character_motion_registration())?;
    Ok(())
}

fn builtin_id(value: &str) -> ComponentTypeId {
    ComponentTypeId::parse(value).expect("built-in component identity is valid")
}
