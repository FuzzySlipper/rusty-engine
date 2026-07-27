use core_ids::EntityId;
use entity_state::{
    ComponentCodec, ComponentRegistration, ComponentRegistrationError, ComponentRegistry,
    ComponentTypeId, EntityComponent,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    CatalogVersion, EffectDefinitionId, EffectInstanceId, EquipmentSlotId, ItemDefinitionId,
    MechanicsScalar, SourceDefinitionId, SourceInstanceId, StatId, TrackId,
    MAX_ACTIVE_EFFECT_INSTANCES, MAX_EQUIPMENT_ASSIGNMENTS, MAX_INTRINSIC_SOURCE_BINDINGS,
    MAX_INVENTORY_STACKS,
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

pub const MAX_STATS_PER_ENTITY: usize = 128;
pub const MAX_TRACKS_PER_ENTITY: usize = 128;
pub const MAX_STACK_QUANTITY: u64 = 1_000_000_000;

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
    pub stat: StatId,
    pub base: MechanicsScalar,
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
}

impl EntityComponent for StatsComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TrackValue {
    pub track: TrackId,
    pub current: MechanicsScalar,
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
    pub instance: SourceInstanceId,
    pub definition: SourceDefinitionId,
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
pub struct ActiveEffectInstance {
    pub instance: EffectInstanceId,
    pub definition: EffectDefinitionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ActiveEffectsComponent {
    catalog_version: CatalogVersion,
    effects: Vec<ActiveEffectInstance>,
}

impl ActiveEffectsComponent {
    pub const LABEL: &'static str = "ActiveEffectsComponent";

    pub fn new(
        catalog_version: CatalogVersion,
        mut effects: Vec<ActiveEffectInstance>,
    ) -> Result<Self, MechanicsComponentDataError> {
        effects.sort_by(|left, right| left.instance.cmp(&right.instance));
        validate_unique_quota(
            &effects,
            MAX_ACTIVE_EFFECT_INSTANCES,
            "activeEffects",
            |value| value.instance.as_str(),
        )?;
        Ok(Self {
            catalog_version,
            effects,
        })
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn effects(&self) -> &[ActiveEffectInstance] {
        &self.effects
    }
}

impl EntityComponent for ActiveEffectsComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemStack {
    pub definition: ItemDefinitionId,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InventoryComponent {
    catalog_version: CatalogVersion,
    stacks: Vec<ItemStack>,
}

impl InventoryComponent {
    pub const LABEL: &'static str = "InventoryComponent";

    pub fn new(
        catalog_version: CatalogVersion,
        mut stacks: Vec<ItemStack>,
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
        Ok(Self {
            catalog_version,
            stacks,
        })
    }

    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }

    pub fn stacks(&self) -> &[ItemStack] {
        &self.stacks
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
        let mut items: Vec<_> = assignments.iter().map(|value| value.item).collect();
        items.sort();
        if let Some(pair) = items.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(MechanicsComponentDataError::DuplicateItem { item: pair[0] });
        }
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
    DuplicateItem {
        item: EntityId,
    },
    InvalidQuantity {
        definition: ItemDefinitionId,
        quantity: u64,
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
        validate_stats,
    ))?;
    staged.register(durable_registration::<TracksComponent>(
        TRACKS_COMPONENT_TYPE_ID,
        TRACKS_CODEC_ID,
        validate_tracks,
    ))?;
    staged.register(durable_registration::<IntrinsicSourcesComponent>(
        INTRINSIC_SOURCES_COMPONENT_TYPE_ID,
        INTRINSIC_SOURCES_CODEC_ID,
        validate_intrinsic_sources,
    ))?;
    staged.register(durable_registration::<ActiveEffectsComponent>(
        ACTIVE_EFFECTS_COMPONENT_TYPE_ID,
        ACTIVE_EFFECTS_CODEC_ID,
        validate_active_effects,
    ))?;
    staged.register(durable_registration::<InventoryComponent>(
        INVENTORY_COMPONENT_TYPE_ID,
        INVENTORY_CODEC_ID,
        validate_inventory,
    ))?;
    staged.register(durable_registration::<ItemComponent>(
        ITEM_COMPONENT_TYPE_ID,
        ITEM_CODEC_ID,
        |_| Ok(()),
    ))?;
    staged.register(durable_registration::<EquipmentComponent>(
        EQUIPMENT_COMPONENT_TYPE_ID,
        EQUIPMENT_CODEC_ID,
        validate_equipment,
    ))?;
    *registry = staged;
    Ok(())
}

fn durable_registration<T>(
    type_id: &'static str,
    codec_id: &'static str,
    validator: fn(&T) -> Result<(), String>,
) -> ComponentRegistration<T>
where
    T: EntityComponent + Serialize + DeserializeOwned,
{
    let codec = ComponentCodec::new(
        codec_id,
        COMPONENT_CODEC_VERSION,
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
    validate_sorted_unique_quota(&value.effects, MAX_ACTIVE_EFFECT_INSTANCES, |entry| {
        entry.instance.as_str()
    })
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
    Ok(())
}

fn validate_equipment(value: &EquipmentComponent) -> Result<(), String> {
    validate_sorted_unique_quota(&value.assignments, MAX_EQUIPMENT_ASSIGNMENTS, |entry| {
        entry.slot.as_str()
    })?;
    let mut items: Vec<_> = value.assignments.iter().map(|entry| entry.item).collect();
    items.sort();
    if items.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("equipment item is assigned more than once".to_string());
    }
    Ok(())
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
