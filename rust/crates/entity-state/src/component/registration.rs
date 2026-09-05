use std::fmt;

use core_ids::EntityId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_COMPONENT_TYPE_ID_BYTES: usize = 128;
pub const MAX_COMPONENT_CODEC_ID_BYTES: usize = 128;
pub const MAX_REGISTERED_COMPONENT_TYPES: usize = 256;
pub const MAX_COMPONENT_INSPECTION_ENTITIES: usize = 64;

pub const TRANSFORM_COMPONENT_TYPE_ID: &str = "rusty.entity.transform";
pub const BOUNDS_COMPONENT_TYPE_ID: &str = "rusty.entity.bounds";
pub const COLLISION_COMPONENT_TYPE_ID: &str = "rusty.entity.collision";
pub const RENDERABLE_COMPONENT_TYPE_ID: &str = "rusty.entity.renderable";
pub const KINEMATIC_COMPONENT_TYPE_ID: &str = "rusty.entity.kinematic";
pub const CONTROLLER_COMPONENT_TYPE_ID: &str = "rusty.entity.controller";
pub const ASSET_BINDING_COMPONENT_TYPE_ID: &str = "rusty.entity.asset-binding";

/// Marker for inert data that may be attached to an [`EntityState`](crate::EntityState).
///
/// Implementing this trait does not register, schedule, persist, or execute the type. Each
/// `EntityState` instance must receive an explicit [`ComponentRegistration`] before the type can
/// be used.
pub trait EntityComponent: Clone + fmt::Debug + Send + Sync + 'static {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentTypeId(String);

impl ComponentTypeId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ComponentIdentityError> {
        let value = value.into();
        validate_identity(&value, MAX_COMPONENT_TYPE_ID_BYTES).map_err(|reason| {
            ComponentIdentityError {
                value: value.clone(),
                reason,
            }
        })?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ComponentTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentIdentityError {
    pub value: String,
    pub reason: &'static str,
}

impl fmt::Display for ComponentIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid component identity {:?}: {}",
            self.value, self.reason
        )
    }
}

impl std::error::Error for ComponentIdentityError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentPersistence {
    RuntimeOnly,
    Durable { version: u32 },
    LegacySnapshot,
}

impl ComponentPersistence {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeOnly => "runtimeOnly",
            Self::Durable { .. } => "durable",
            Self::LegacySnapshot => "legacySnapshot",
        }
    }
}

type ComponentMigration<T> = fn(u32, Value) -> Result<T, String>;

#[derive(Clone, Copy)]
pub struct ComponentCodec<T: EntityComponent> {
    pub(super) identity: &'static str,
    pub(super) version: u32,
    pub(super) encode: fn(&T) -> Value,
    pub(super) decode: fn(Value) -> Result<T, String>,
    /// Optional narrow compatibility hook for older versions of this same
    /// codec. Newer snapshots are never accepted, and a codec without this
    /// hook remains exact-version only.
    pub(super) migrate: Option<ComponentMigration<T>>,
}

impl<T: EntityComponent> fmt::Debug for ComponentCodec<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComponentCodec")
            .field("identity", &self.identity)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl<T: EntityComponent> ComponentCodec<T> {
    pub fn new(
        identity: &'static str,
        version: u32,
        encode: fn(&T) -> Value,
        decode: fn(Value) -> Result<T, String>,
    ) -> Result<Self, ComponentCodecError> {
        validate_identity(identity, MAX_COMPONENT_CODEC_ID_BYTES).map_err(|reason| {
            ComponentCodecError::InvalidIdentity {
                value: identity.to_string(),
                reason,
            }
        })?;
        if version == 0 {
            return Err(ComponentCodecError::InvalidVersion { version });
        }
        Ok(Self {
            identity,
            version,
            encode,
            decode,
            migrate: None,
        })
    }

    /// Add a compatibility decoder for older versions of this codec.
    ///
    /// The callback owns the version transition and must reject versions it
    /// does not understand. It is intentionally separate from the current
    /// decoder so current snapshots remain exact and newer snapshots never
    /// get guessed at.
    pub fn with_migration(mut self, migrate: fn(u32, Value) -> Result<T, String>) -> Self {
        self.migrate = Some(migrate);
        self
    }

    pub const fn identity(&self) -> &'static str {
        self.identity
    }

    pub const fn version(&self) -> u32 {
        self.version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentCodecError {
    InvalidIdentity { value: String, reason: &'static str },
    InvalidVersion { version: u32 },
}

impl fmt::Display for ComponentCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid component codec: {self:?}")
    }
}

impl std::error::Error for ComponentCodecError {}

pub(super) type ComponentValidator<T> = fn(&T) -> Result<(), String>;

#[derive(Clone)]
pub(super) enum RegistrationPersistence<T: EntityComponent> {
    RuntimeOnly,
    Durable(ComponentCodec<T>),
    LegacySnapshot,
}

#[derive(Clone)]
pub struct ComponentRegistration<T: EntityComponent> {
    pub(super) type_id: ComponentTypeId,
    pub(super) persistence: RegistrationPersistence<T>,
    pub(super) validator: ComponentValidator<T>,
}

impl<T: EntityComponent> fmt::Debug for ComponentRegistration<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComponentRegistration")
            .field("type_id", &self.type_id)
            .field("persistence", &self.persistence())
            .finish_non_exhaustive()
    }
}

impl<T: EntityComponent> ComponentRegistration<T> {
    pub fn runtime_only(type_id: ComponentTypeId) -> Self {
        Self {
            type_id,
            persistence: RegistrationPersistence::RuntimeOnly,
            validator: |_| Ok(()),
        }
    }

    pub fn durable(type_id: ComponentTypeId, codec: ComponentCodec<T>) -> Self {
        Self {
            type_id,
            persistence: RegistrationPersistence::Durable(codec),
            validator: |_| Ok(()),
        }
    }

    pub fn with_validator(mut self, validator: fn(&T) -> Result<(), String>) -> Self {
        self.validator = validator;
        self
    }

    pub fn type_id(&self) -> &ComponentTypeId {
        &self.type_id
    }

    pub fn persistence(&self) -> ComponentPersistence {
        match &self.persistence {
            RegistrationPersistence::RuntimeOnly => ComponentPersistence::RuntimeOnly,
            RegistrationPersistence::Durable(codec) => ComponentPersistence::Durable {
                version: codec.version,
            },
            RegistrationPersistence::LegacySnapshot => ComponentPersistence::LegacySnapshot,
        }
    }

    pub(crate) fn legacy_snapshot(type_id: ComponentTypeId) -> Self {
        Self {
            type_id,
            persistence: RegistrationPersistence::LegacySnapshot,
            validator: |_| Ok(()),
        }
    }

    pub(super) fn codec_signature(&self) -> Option<(&'static str, u32)> {
        match &self.persistence {
            RegistrationPersistence::Durable(codec) => Some((codec.identity, codec.version)),
            RegistrationPersistence::RuntimeOnly | RegistrationPersistence::LegacySnapshot => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentRegistrationError {
    DuplicateStableId {
        component: ComponentTypeId,
    },
    StableIdConflict {
        component: ComponentTypeId,
        registered_rust_type: &'static str,
        requested_rust_type: &'static str,
    },
    RustTypeConflict {
        rust_type: &'static str,
        registered_component: ComponentTypeId,
        requested_component: ComponentTypeId,
    },
    IncompatibleCodec {
        component: ComponentTypeId,
    },
    TypeLimitExceeded {
        limit: usize,
    },
}

impl fmt::Display for ComponentRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "component registration rejected: {self:?}")
    }
}

impl std::error::Error for ComponentRegistrationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentAccessError {
    UnregisteredRustType { rust_type: &'static str },
}

impl fmt::Display for ComponentAccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "component access rejected: {self:?}")
    }
}

impl std::error::Error for ComponentAccessError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentKindInspection {
    pub type_id: ComponentTypeId,
    pub persistence: ComponentPersistence,
    pub count: usize,
    pub entity_sample: Vec<EntityId>,
    pub entity_sample_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentStoreInspection {
    pub registered_kind_count: usize,
    pub kinds: Vec<ComponentKindInspection>,
}

/// Instance-local optimistic-concurrency guard for one entity/component slot.
///
/// Component revisions are intentionally not durable. Callers reacquire them from the live
/// [`EntityState`](crate::EntityState) after construction or snapshot restoration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRevision {
    pub(crate) entity: EntityId,
    pub(crate) component: ComponentTypeId,
    pub(crate) revision: u64,
}

impl ComponentRevision {
    pub const fn entity(&self) -> EntityId {
        self.entity
    }

    pub fn component(&self) -> &ComponentTypeId {
        &self.component
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RegisteredComponentSnapshot {
    pub type_id: String,
    pub codec: String,
    pub version: u32,
    pub required: bool,
    pub values: Vec<ComponentValueSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ComponentValueSnapshot {
    pub entity: u64,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisteredComponentSnapshotError {
    InvalidTypeId {
        value: String,
        reason: &'static str,
    },
    DuplicateType {
        component: ComponentTypeId,
    },
    UnknownRequiredType {
        component: ComponentTypeId,
    },
    PersistenceMismatch {
        component: ComponentTypeId,
    },
    CodecMismatch {
        component: ComponentTypeId,
        expected_codec: String,
        expected_version: u32,
        actual_codec: String,
        actual_version: u32,
    },
    DuplicateEntityValue {
        component: ComponentTypeId,
        entity: EntityId,
    },
    UnknownEntity {
        component: ComponentTypeId,
        entity: EntityId,
    },
    TombstonedEntity {
        component: ComponentTypeId,
        entity: EntityId,
    },
    DecodeFailed {
        component: ComponentTypeId,
        entity: EntityId,
        reason: String,
    },
    InvalidValue {
        component: ComponentTypeId,
        entity: EntityId,
        reason: String,
    },
}

impl fmt::Display for RegisteredComponentSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "registered component snapshot rejected: {self:?}"
        )
    }
}

impl std::error::Error for RegisteredComponentSnapshotError {}

fn validate_identity(value: &str, max_bytes: usize) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("identity is empty");
    }
    if value.len() > max_bytes {
        return Err("identity exceeds its UTF-8 byte limit");
    }
    let mut characters = value.bytes();
    if !characters
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
    {
        return Err("identity must start with a lowercase ASCII letter");
    }
    if !characters.all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
    }) {
        return Err("identity contains unsupported characters");
    }
    Ok(())
}
