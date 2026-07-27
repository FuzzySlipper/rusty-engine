use core_ids::EntityId;
use entity_state::{
    ComponentCodec, ComponentRegistration, ComponentRegistrationError, ComponentRegistry,
    ComponentTypeId, EntityComponent,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    ActiveEffectsComponent, CapacityMetricId, CatalogVersion, EffectInstanceId, EquipmentSlotId,
    ItemDefinitionId, MechanicsScalar, SourceDefinitionId, SourceInstanceId, StatId, TrackId,
    MAX_ACTIVE_EFFECT_INSTANCES, MAX_EFFECT_STACKS, MAX_EQUIPMENT_ASSIGNMENTS,
    MAX_INTRINSIC_SOURCE_BINDINGS, MAX_INVENTORY_STACKS,
};

pub const STATS_COMPONENT_TYPE_ID: &str = "rusty.mechanics.stats";
pub const TRACKS_COMPONENT_TYPE_ID: &str = "rusty.mechanics.tracks";
pub const INTRINSIC_SOURCES_COMPONENT_TYPE_ID: &str = "rusty.mechanics.intrinsic-sources";
pub const ACTIVE_EFFECTS_COMPONENT_TYPE_ID: &str = "rusty.mechanics.active-effects";
pub const INVENTORY_COMPONENT_TYPE_ID: &str = "rusty.mechanics.inventory";
pub const ITEM_COMPONENT_TYPE_ID: &str = "rusty.mechanics.item";
pub const EQUIPMENT_COMPONENT_TYPE_ID: &str = "rusty.mechanics.equipment";

const STATS_CODEC_ID: &str = "rusty.mechanics.stats-json";
const TRACKS_CODEC_ID: &str = "rusty.mechanics.tracks-json";
const INTRINSIC_SOURCES_CODEC_ID: &str = "rusty.mechanics.intrinsic-sources-json";
const ACTIVE_EFFECTS_CODEC_ID: &str = "rusty.mechanics.active-effects-json";
const INVENTORY_CODEC_ID: &str = "rusty.mechanics.inventory-json";
const ITEM_CODEC_ID: &str = "rusty.mechanics.item-json";
const EQUIPMENT_CODEC_ID: &str = "rusty.mechanics.equipment-json";
const COMPONENT_CODEC_VERSION: u32 = 1;
const ACTIVE_EFFECTS_CODEC_VERSION: u32 = 2;
const INVENTORY_CODEC_VERSION: u32 = 2;

pub const MAX_STATS_PER_ENTITY: usize = 128;
pub const MAX_TRACKS_PER_ENTITY: usize = 128;
pub const MAX_STACK_QUANTITY: u64 = 1_000_000_000;
pub const MAX_INVENTORY_CAPACITY_LIMITS: usize = 32;
pub const MAX_CAPACITY_LIMIT_UNITS: u64 = 1_000_000_000_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MechanicsComponentKind {
    Stats,
    Tracks,
    IntrinsicSources,
    ActiveEffects,
    Inventory,
    Item,
    Equipment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedComponentRevision {
    pub entity: EntityId,
    pub component: MechanicsComponentKind,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StatValue {
    stat: StatId,
    base: MechanicsScalar,
}

impl StatValue {
    pub const fn new(stat: StatId, base: MechanicsScalar) -> Self {
        Self { stat, base }
    }

    pub const fn stat(&self) -> &StatId {
        &self.stat
    }

    pub const fn base(&self) -> MechanicsScalar {
        self.base
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StatsComponent {
    catalog_version: CatalogVersion,
    values: Vec<StatValue>,
}

impl StatsComponent {
    pub const LABEL: &'static str = "StatsComponent";

    pub fn new(
        catalog_version: CatalogVersion,
        mut values: Vec<StatValue>,
    ) -> Result<Self, MechanicsComponentDataError> {
        values.sort_by(|left, right| left.stat.cmp(&right.stat));
        validate_unique_quota(&values, MAX_STATS_PER_ENTITY, "stats", |value| {
            value.stat.as_str()
        })?;
        Ok(Self {
            catalog_version,
            values,
        })
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn values(&self) -> &[StatValue] {
        &self.values
    }

    pub fn base(&self, stat: &StatId) -> Option<MechanicsScalar> {
        self.values
            .binary_search_by(|value| value.stat.cmp(stat))
            .ok()
            .map(|index| self.values[index].base)
    }

    pub(crate) fn set_base(&mut self, stat: &StatId, base: MechanicsScalar) -> bool {
        let Ok(index) = self.values.binary_search_by(|value| value.stat.cmp(stat)) else {
            return false;
        };
        self.values[index].base = base;
        true
    }
}

impl EntityComponent for StatsComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TrackValue {
    track: TrackId,
    current: MechanicsScalar,
}

impl TrackValue {
    pub const fn new(track: TrackId, current: MechanicsScalar) -> Self {
        Self { track, current }
    }

    pub const fn track(&self) -> &TrackId {
        &self.track
    }

    pub const fn current(&self) -> MechanicsScalar {
        self.current
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TracksComponent {
    catalog_version: CatalogVersion,
    values: Vec<TrackValue>,
}

impl TracksComponent {
    pub const LABEL: &'static str = "TracksComponent";

    pub fn new(
        catalog_version: CatalogVersion,
        mut values: Vec<TrackValue>,
    ) -> Result<Self, MechanicsComponentDataError> {
        values.sort_by(|left, right| left.track.cmp(&right.track));
        validate_unique_quota(&values, MAX_TRACKS_PER_ENTITY, "tracks", |value| {
            value.track.as_str()
        })?;
        Ok(Self {
            catalog_version,
            values,
        })
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn values(&self) -> &[TrackValue] {
        &self.values
    }

    pub fn current(&self, track: &TrackId) -> Option<MechanicsScalar> {
        self.index_of(track).map(|index| self.values[index].current)
    }

    pub(crate) fn set_current(&mut self, track: &TrackId, current: MechanicsScalar) -> bool {
        let Some(index) = self.index_of(track) else {
            return false;
        };
        self.values[index].current = current;
        true
    }

    fn index_of(&self, track: &TrackId) -> Option<usize> {
        self.values
            .binary_search_by(|value| value.track.cmp(track))
            .ok()
    }
}

impl EntityComponent for TracksComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct IntrinsicSourceBinding {
    instance: SourceInstanceId,
    definition: SourceDefinitionId,
}

impl IntrinsicSourceBinding {
    pub const fn new(instance: SourceInstanceId, definition: SourceDefinitionId) -> Self {
        Self {
            instance,
            definition,
        }
    }

    pub const fn instance(&self) -> &SourceInstanceId {
        &self.instance
    }

    pub const fn definition(&self) -> &SourceDefinitionId {
        &self.definition
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct IntrinsicSourcesComponent {
    catalog_version: CatalogVersion,
    bindings: Vec<IntrinsicSourceBinding>,
}

impl IntrinsicSourcesComponent {
    pub const LABEL: &'static str = "IntrinsicSourcesComponent";

    pub fn new(
        catalog_version: CatalogVersion,
        mut bindings: Vec<IntrinsicSourceBinding>,
    ) -> Result<Self, MechanicsComponentDataError> {
        bindings.sort_by(|left, right| left.instance.cmp(&right.instance));
        validate_unique_quota(
            &bindings,
            MAX_INTRINSIC_SOURCE_BINDINGS,
            "intrinsicSources",
            |value| value.instance.as_str(),
        )?;
        Ok(Self {
            catalog_version,
            bindings,
        })
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn bindings(&self) -> &[IntrinsicSourceBinding] {
        &self.bindings
    }
}

impl EntityComponent for IntrinsicSourcesComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemStack {
    pub definition: ItemDefinitionId,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InventoryCapacityLimit {
    metric: CapacityMetricId,
    maximum: u64,
}

impl InventoryCapacityLimit {
    pub const fn new(metric: CapacityMetricId, maximum: u64) -> Self {
        Self { metric, maximum }
    }

    pub const fn metric(&self) -> &CapacityMetricId {
        &self.metric
    }

    pub const fn maximum(&self) -> u64 {
        self.maximum
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InventoryComponent {
    catalog_version: CatalogVersion,
    stacks: Vec<ItemStack>,
    capacity_limits: Vec<InventoryCapacityLimit>,
}

impl InventoryComponent {
    pub const LABEL: &'static str = "InventoryComponent";

    pub fn new(
        catalog_version: CatalogVersion,
        stacks: Vec<ItemStack>,
    ) -> Result<Self, MechanicsComponentDataError> {
        Self::with_capacity_limits(catalog_version, stacks, Vec::new())
    }

    pub fn with_capacity_limits(
        catalog_version: CatalogVersion,
        mut stacks: Vec<ItemStack>,
        mut capacity_limits: Vec<InventoryCapacityLimit>,
    ) -> Result<Self, MechanicsComponentDataError> {
        stacks.sort_by(|left, right| left.definition.cmp(&right.definition));
        validate_unique_quota(&stacks, MAX_INVENTORY_STACKS, "inventoryStacks", |value| {
            value.definition.as_str()
        })?;
        if let Some(value) = stacks
            .iter()
            .find(|value| value.quantity == 0 || value.quantity > MAX_STACK_QUANTITY)
        {
            return Err(MechanicsComponentDataError::InvalidQuantity {
                definition: value.definition.clone(),
                quantity: value.quantity,
            });
        }
        capacity_limits.sort_by(|left, right| left.metric.cmp(&right.metric));
        validate_unique_quota(
            &capacity_limits,
            MAX_INVENTORY_CAPACITY_LIMITS,
            "inventoryCapacityLimits",
            |value| value.metric.as_str(),
        )?;
        if let Some(value) = capacity_limits
            .iter()
            .find(|value| value.maximum > MAX_CAPACITY_LIMIT_UNITS)
        {
            return Err(MechanicsComponentDataError::InvalidCapacityLimit {
                metric: value.metric.clone(),
                maximum: value.maximum,
                allowed: MAX_CAPACITY_LIMIT_UNITS,
            });
        }
        Ok(Self {
            catalog_version,
            stacks,
            capacity_limits,
        })
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn stacks(&self) -> &[ItemStack] {
        &self.stacks
    }

    pub fn capacity_limits(&self) -> &[InventoryCapacityLimit] {
        &self.capacity_limits
    }
}

impl EntityComponent for InventoryComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemComponent {
    catalog_version: CatalogVersion,
    definition: ItemDefinitionId,
}

impl ItemComponent {
    pub const LABEL: &'static str = "ItemComponent";

    pub fn new(catalog_version: CatalogVersion, definition: ItemDefinitionId) -> Self {
        Self {
            catalog_version,
            definition,
        }
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn definition(&self) -> &ItemDefinitionId {
        &self.definition
    }
}

impl EntityComponent for ItemComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EquipmentAssignment {
    pub slot: EquipmentSlotId,
    #[serde(with = "crate::source::entity_id_serde")]
    pub item: EntityId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EquipmentComponent {
    catalog_version: CatalogVersion,
    assignments: Vec<EquipmentAssignment>,
}

impl EquipmentComponent {
    pub const LABEL: &'static str = "EquipmentComponent";

    pub fn new(
        catalog_version: CatalogVersion,
        mut assignments: Vec<EquipmentAssignment>,
    ) -> Result<Self, MechanicsComponentDataError> {
        assignments.sort_by(|left, right| left.slot.cmp(&right.slot));
        validate_unique_quota(
            &assignments,
            MAX_EQUIPMENT_ASSIGNMENTS,
            "equipmentAssignments",
            |value| value.slot.as_str(),
        )?;
        Ok(Self {
            catalog_version,
            assignments,
        })
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn assignments(&self) -> &[EquipmentAssignment] {
        &self.assignments
    }

    pub fn assignment(&self, slot: &EquipmentSlotId) -> Option<&EquipmentAssignment> {
        self.assignments
            .binary_search_by(|value| value.slot.cmp(slot))
            .ok()
            .map(|index| &self.assignments[index])
    }
}

impl EntityComponent for EquipmentComponent {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MechanicsComponentDataError {
    QuotaExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    DuplicateIdentity {
        field: &'static str,
        identity: String,
    },
    InvalidQuantity {
        definition: ItemDefinitionId,
        quantity: u64,
    },
    InvalidCapacityLimit {
        metric: CapacityMetricId,
        maximum: u64,
        allowed: u64,
    },
    InvalidEffectStacks {
        instance: EffectInstanceId,
        stacks: u16,
        maximum: u16,
    },
}

impl std::fmt::Display for MechanicsComponentDataError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "mechanics component data rejected: {self:?}")
    }
}

impl std::error::Error for MechanicsComponentDataError {}

pub fn gameplay_component_registry() -> Result<ComponentRegistry, ComponentRegistrationError> {
    let mut registry = ComponentRegistry::new();
    register_gameplay_components(&mut registry)?;
    Ok(registry)
}

pub fn register_gameplay_components(
    registry: &mut ComponentRegistry,
) -> Result<(), ComponentRegistrationError> {
    let mut staged = registry.clone();
    staged.register(durable_registration::<StatsComponent>(
        STATS_COMPONENT_TYPE_ID,
        STATS_CODEC_ID,
        COMPONENT_CODEC_VERSION,
        validate_stats,
    ))?;
    staged.register(durable_registration::<TracksComponent>(
        TRACKS_COMPONENT_TYPE_ID,
        TRACKS_CODEC_ID,
        COMPONENT_CODEC_VERSION,
        validate_tracks,
    ))?;
    staged.register(durable_registration::<IntrinsicSourcesComponent>(
        INTRINSIC_SOURCES_COMPONENT_TYPE_ID,
        INTRINSIC_SOURCES_CODEC_ID,
        COMPONENT_CODEC_VERSION,
        validate_intrinsic_sources,
    ))?;
    staged.register(durable_registration::<ActiveEffectsComponent>(
        ACTIVE_EFFECTS_COMPONENT_TYPE_ID,
        ACTIVE_EFFECTS_CODEC_ID,
        ACTIVE_EFFECTS_CODEC_VERSION,
        validate_active_effects,
    ))?;
    staged.register(durable_registration::<InventoryComponent>(
        INVENTORY_COMPONENT_TYPE_ID,
        INVENTORY_CODEC_ID,
        INVENTORY_CODEC_VERSION,
        validate_inventory,
    ))?;
    staged.register(durable_registration::<ItemComponent>(
        ITEM_COMPONENT_TYPE_ID,
        ITEM_CODEC_ID,
        COMPONENT_CODEC_VERSION,
        |_| Ok(()),
    ))?;
    staged.register(durable_registration::<EquipmentComponent>(
        EQUIPMENT_COMPONENT_TYPE_ID,
        EQUIPMENT_CODEC_ID,
        COMPONENT_CODEC_VERSION,
        validate_equipment,
    ))?;
    *registry = staged;
    Ok(())
}

fn durable_registration<T>(
    type_id: &'static str,
    codec_id: &'static str,
    codec_version: u32,
    validator: fn(&T) -> Result<(), String>,
) -> ComponentRegistration<T>
where
    T: EntityComponent + Serialize + DeserializeOwned,
{
    let codec = ComponentCodec::new(
        codec_id,
        codec_version,
        |value| serde_json::to_value(value).expect("mechanics component codec is infallible"),
        |value| serde_json::from_value(value).map_err(|error| error.to_string()),
    )
    .expect("mechanics codec identity and version are fixed and valid");
    ComponentRegistration::durable(
        ComponentTypeId::parse(type_id).expect("mechanics component identity is fixed and valid"),
        codec,
    )
    .with_validator(validator)
}

fn validate_stats(value: &StatsComponent) -> Result<(), String> {
    validate_sorted_unique_quota(&value.values, MAX_STATS_PER_ENTITY, |entry| {
        entry.stat.as_str()
    })
}

fn validate_tracks(value: &TracksComponent) -> Result<(), String> {
    validate_sorted_unique_quota(&value.values, MAX_TRACKS_PER_ENTITY, |entry| {
        entry.track.as_str()
    })
}

fn validate_intrinsic_sources(value: &IntrinsicSourcesComponent) -> Result<(), String> {
    validate_sorted_unique_quota(&value.bindings, MAX_INTRINSIC_SOURCE_BINDINGS, |entry| {
        entry.instance.as_str()
    })
}

fn validate_active_effects(value: &ActiveEffectsComponent) -> Result<(), String> {
    validate_sorted_unique_quota(value.effects(), MAX_ACTIVE_EFFECT_INSTANCES, |entry| {
        entry.instance().as_str()
    })?;
    if value
        .effects()
        .iter()
        .any(|entry| entry.stacks() == 0 || entry.stacks() > MAX_EFFECT_STACKS)
    {
        return Err("active effect stacks are zero or exceed the bound".to_string());
    }
    Ok(())
}

fn validate_inventory(value: &InventoryComponent) -> Result<(), String> {
    validate_sorted_unique_quota(&value.stacks, MAX_INVENTORY_STACKS, |entry| {
        entry.definition.as_str()
    })?;
    if value
        .stacks
        .iter()
        .any(|entry| entry.quantity == 0 || entry.quantity > MAX_STACK_QUANTITY)
    {
        return Err("inventory quantity is zero or exceeds the bound".to_string());
    }
    validate_sorted_unique_quota(
        &value.capacity_limits,
        MAX_INVENTORY_CAPACITY_LIMITS,
        |entry| entry.metric.as_str(),
    )?;
    if value
        .capacity_limits
        .iter()
        .any(|entry| entry.maximum > MAX_CAPACITY_LIMIT_UNITS)
    {
        return Err("inventory capacity limit exceeds the bound".to_string());
    }
    Ok(())
}

fn validate_equipment(value: &EquipmentComponent) -> Result<(), String> {
    validate_sorted_unique_quota(&value.assignments, MAX_EQUIPMENT_ASSIGNMENTS, |entry| {
        entry.slot.as_str()
    })
}

fn validate_unique_quota<T>(
    values: &[T],
    maximum: usize,
    field: &'static str,
    identity: impl Fn(&T) -> &str,
) -> Result<(), MechanicsComponentDataError> {
    if values.len() > maximum {
        return Err(MechanicsComponentDataError::QuotaExceeded {
            field,
            actual: values.len(),
            maximum,
        });
    }
    for pair in values.windows(2) {
        if identity(&pair[0]) == identity(&pair[1]) {
            return Err(MechanicsComponentDataError::DuplicateIdentity {
                field,
                identity: identity(&pair[0]).to_string(),
            });
        }
    }
    Ok(())
}

fn validate_sorted_unique_quota<T>(
    values: &[T],
    maximum: usize,
    identity: impl Fn(&T) -> &str,
) -> Result<(), String> {
    if values.len() > maximum {
        return Err(format!("component entry count exceeds {maximum}"));
    }
    for pair in values.windows(2) {
        if identity(&pair[0]) >= identity(&pair[1]) {
            return Err("component entries are not in strict canonical order".to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use entity_state::{ComponentPersistence, ComponentRegistry};

    use super::{register_gameplay_components, StatsComponent, STATS_COMPONENT_TYPE_ID};

    #[test]
    fn registration_is_explicit_durable_and_fail_atomic() {
        let mut registry = ComponentRegistry::new();
        register_gameplay_components(&mut registry).unwrap();
        let before = format!("{registry:?}");
        assert!(register_gameplay_components(&mut registry).is_err());
        assert_eq!(format!("{registry:?}"), before);

        let state = entity_state::EntityState::with_registry(registry);
        let inspection = state.component_inspection();
        let stats = inspection
            .kinds
            .iter()
            .find(|kind| kind.type_id.as_str() == STATS_COMPONENT_TYPE_ID)
            .unwrap();
        assert_eq!(
            stats.persistence,
            ComponentPersistence::Durable { version: 1 }
        );
        assert!(state
            .component::<StatsComponent>(core_ids::EntityId::new(1))
            .unwrap()
            .is_none());
    }
}
