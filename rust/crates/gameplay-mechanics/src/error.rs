use core_ids::EntityId;
use entity_state::{
    ComponentAccessError, EntityAuthoringError, EntityStateSnapshotError, RelationshipError,
};

use crate::{
    CatalogVersion, DamageKindId, EffectDefinitionId, EquipmentSlotId, ItemDefinitionId,
    MechanicsArithmeticError, MechanicsComponentDataError, SourceDefinitionId,
    SourceInstanceIdentity, StatId, TrackId,
};

#[derive(Debug)]
pub enum MechanicsError {
    ComponentAccess(ComponentAccessError),
    ComponentMutation(EntityAuthoringError),
    Relationship(RelationshipError),
    Arithmetic(MechanicsArithmeticError),
    InvalidComponentData(MechanicsComponentDataError),
    MissingEntity {
        entity: EntityId,
    },
    MissingComponent {
        entity: EntityId,
        component: &'static str,
    },
    CatalogVersionMismatch {
        entity: EntityId,
        component: &'static str,
        expected: CatalogVersion,
        actual: CatalogVersion,
    },
    UnknownStat {
        stat: StatId,
    },
    MissingStat {
        entity: EntityId,
        stat: StatId,
    },
    StatOutOfBounds {
        entity: EntityId,
        stat: StatId,
        attempted: i64,
        minimum: i64,
        maximum: i64,
    },
    InvalidResolvedStatBounds {
        entity: EntityId,
        stat: StatId,
        minimum: i64,
        maximum: i64,
    },
    UnknownTrack {
        track: TrackId,
    },
    MissingTrack {
        entity: EntityId,
        track: TrackId,
    },
    UnknownSource {
        source: SourceDefinitionId,
    },
    UnknownEffect {
        effect: EffectDefinitionId,
    },
    UnknownItem {
        item: ItemDefinitionId,
    },
    UnknownEquipmentSlot {
        slot: EquipmentSlotId,
    },
    UnknownDamageKind {
        kind: DamageKindId,
    },
    DuplicateSource {
        source: SourceInstanceIdentity,
    },
    InvalidCatalogReference {
        entity: EntityId,
        component: &'static str,
        namespace: &'static str,
        reference: String,
    },
    TrackOutOfBounds {
        entity: EntityId,
        track: TrackId,
        attempted: i64,
        minimum: i64,
        maximum: i64,
    },
    InvalidResolvedTrackBounds {
        entity: EntityId,
        track: TrackId,
        minimum: i64,
        maximum: i64,
    },
    RequestQuotaExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    ComponentRevisionScopeMismatch {
        expected_entity: EntityId,
        actual_entity: EntityId,
        expected_component: String,
        actual_component: String,
    },
    StaleComponentRevision {
        expected: u64,
        actual: u64,
    },
    ItemNotContained {
        item: EntityId,
        expected_owner: EntityId,
        actual_owner: Option<EntityId>,
    },
    ItemEquipped {
        item: EntityId,
        owner: EntityId,
        slot: EquipmentSlotId,
    },
    EquipmentSlotOccupied {
        owner: EntityId,
        slot: EquipmentSlotId,
        item: EntityId,
    },
    EquipmentItemAlreadyAssigned {
        owner: EntityId,
        item: EntityId,
    },
    EquipmentSlotEmpty {
        owner: EntityId,
        slot: EquipmentSlotId,
    },
    IncompatibleItemKind {
        item: EntityId,
        definition: ItemDefinitionId,
    },
    ReceiptQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
}

impl std::fmt::Display for MechanicsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "gameplay mechanics rejected: {self:?}")
    }
}

impl std::error::Error for MechanicsError {}

impl From<ComponentAccessError> for MechanicsError {
    fn from(value: ComponentAccessError) -> Self {
        Self::ComponentAccess(value)
    }
}

impl From<EntityAuthoringError> for MechanicsError {
    fn from(value: EntityAuthoringError) -> Self {
        Self::ComponentMutation(value)
    }
}

impl From<RelationshipError> for MechanicsError {
    fn from(value: RelationshipError) -> Self {
        Self::Relationship(value)
    }
}

impl From<MechanicsArithmeticError> for MechanicsError {
    fn from(value: MechanicsArithmeticError) -> Self {
        Self::Arithmetic(value)
    }
}

impl From<MechanicsComponentDataError> for MechanicsError {
    fn from(value: MechanicsComponentDataError) -> Self {
        Self::InvalidComponentData(value)
    }
}

#[derive(Debug)]
pub enum MechanicsSnapshotError {
    EntityState(EntityStateSnapshotError),
    Mechanics(MechanicsError),
}

impl std::fmt::Display for MechanicsSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "mechanics snapshot rejected: {self:?}")
    }
}

impl std::error::Error for MechanicsSnapshotError {}

impl From<EntityStateSnapshotError> for MechanicsSnapshotError {
    fn from(value: EntityStateSnapshotError) -> Self {
        Self::EntityState(value)
    }
}

impl From<MechanicsError> for MechanicsSnapshotError {
    fn from(value: MechanicsError) -> Self {
        Self::Mechanics(value)
    }
}
