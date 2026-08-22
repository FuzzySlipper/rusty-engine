use serde::{de::DeserializeOwned, Deserialize, Serialize};

use entity_state::{
    ComponentCodec, ComponentRegistration, ComponentRegistrationError, ComponentRegistry,
    ComponentTypeId, EntityComponent,
};
use gameplay_standard::ContinuousValue;

use crate::{
    ContinuousCatalogVersion, ContinuousEffectDefinitionId, ContinuousEffectInstanceId,
    ContinuousSourceDefinitionId, ContinuousSourceInstanceId, ContinuousStatId, ContinuousTrackId,
};

pub const CONTINUOUS_STATS_COMPONENT_TYPE_ID: &str = "rusty.continuous-mechanics.stats";
pub const CONTINUOUS_TRACKS_COMPONENT_TYPE_ID: &str = "rusty.continuous-mechanics.tracks";
pub const CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_TYPE_ID: &str =
    "rusty.continuous-mechanics.intrinsic-sources";
pub const CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_TYPE_ID: &str =
    "rusty.continuous-mechanics.active-effects";
pub const CONTINUOUS_STATS_COMPONENT_CODEC_ID: &str = "rusty.continuous-mechanics.stats-bits-json";
pub const CONTINUOUS_TRACKS_COMPONENT_CODEC_ID: &str =
    "rusty.continuous-mechanics.tracks-bits-json";
pub const CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_CODEC_ID: &str =
    "rusty.continuous-mechanics.intrinsic-sources-json";
pub const CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_CODEC_ID: &str =
    "rusty.continuous-mechanics.active-effects-json";
pub const CONTINUOUS_STATS_COMPONENT_CODEC_VERSION: u32 = 1;
pub const CONTINUOUS_TRACKS_COMPONENT_CODEC_VERSION: u32 = 1;
pub const CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_CODEC_VERSION: u32 = 1;
pub const CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_CODEC_VERSION: u32 = 1;
pub const MAX_CONTINUOUS_STATS_PER_ENTITY: usize = 128;
pub const MAX_CONTINUOUS_TRACKS_PER_ENTITY: usize = 128;
pub const MAX_CONTINUOUS_INTRINSIC_SOURCES_PER_ENTITY: usize = 64;
pub const MAX_CONTINUOUS_ACTIVE_EFFECTS_PER_ENTITY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContinuousMechanicsComponentKind {
    Stats,
    Tracks,
    IntrinsicSources,
    ActiveEffects,
}
impl ContinuousMechanicsComponentKind {
    pub const ALL: [Self; 4] = [
        Self::Stats,
        Self::Tracks,
        Self::IntrinsicSources,
        Self::ActiveEffects,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stats => ContinuousStatsComponent::LABEL,
            Self::Tracks => ContinuousTracksComponent::LABEL,
            Self::IntrinsicSources => ContinuousIntrinsicSourcesComponent::LABEL,
            Self::ActiveEffects => ContinuousActiveEffectsComponent::LABEL,
        }
    }
    pub const fn type_id(self) -> &'static str {
        match self {
            Self::Stats => CONTINUOUS_STATS_COMPONENT_TYPE_ID,
            Self::Tracks => CONTINUOUS_TRACKS_COMPONENT_TYPE_ID,
            Self::IntrinsicSources => CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_TYPE_ID,
            Self::ActiveEffects => CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_TYPE_ID,
        }
    }
    pub const fn codec_id(self) -> &'static str {
        match self {
            Self::Stats => CONTINUOUS_STATS_COMPONENT_CODEC_ID,
            Self::Tracks => CONTINUOUS_TRACKS_COMPONENT_CODEC_ID,
            Self::IntrinsicSources => CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_CODEC_ID,
            Self::ActiveEffects => CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_CODEC_ID,
        }
    }

    pub const fn codec_version(self) -> u32 {
        match self {
            Self::Stats => CONTINUOUS_STATS_COMPONENT_CODEC_VERSION,
            Self::Tracks => CONTINUOUS_TRACKS_COMPONENT_CODEC_VERSION,
            Self::IntrinsicSources => CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_CODEC_VERSION,
            Self::ActiveEffects => CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_CODEC_VERSION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousStatValue {
    stat: ContinuousStatId,
    #[serde(with = "crate::bits")]
    base: ContinuousValue,
}
impl ContinuousStatValue {
    pub fn new(stat: ContinuousStatId, base: ContinuousValue) -> Self {
        Self { stat, base }
    }
    pub fn stat(&self) -> &ContinuousStatId {
        &self.stat
    }
    pub fn base(&self) -> ContinuousValue {
        self.base
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousStatsComponent {
    catalog_version: ContinuousCatalogVersion,
    values: Vec<ContinuousStatValue>,
}
impl ContinuousStatsComponent {
    pub const LABEL: &'static str = "ContinuousStatsComponent";
    pub fn new(
        catalog_version: ContinuousCatalogVersion,
        mut values: Vec<ContinuousStatValue>,
    ) -> Result<Self, ContinuousMechanicsComponentError> {
        values.sort_by(|a, b| a.stat.cmp(&b.stat));
        validate_unique(&values, MAX_CONTINUOUS_STATS_PER_ENTITY, "stats", |v| {
            v.stat.as_str()
        })?;
        Ok(Self {
            catalog_version,
            values,
        })
    }
    pub fn catalog_version(&self) -> &ContinuousCatalogVersion {
        &self.catalog_version
    }
    pub fn values(&self) -> &[ContinuousStatValue] {
        &self.values
    }
    pub fn base(&self, id: &ContinuousStatId) -> Option<ContinuousValue> {
        self.values
            .binary_search_by(|v| v.stat.cmp(id))
            .ok()
            .map(|i| self.values[i].base())
    }
    pub(crate) fn set_base(&mut self, id: &ContinuousStatId, value: ContinuousValue) -> bool {
        let Ok(i) = self.values.binary_search_by(|v| v.stat.cmp(id)) else {
            return false;
        };
        self.values[i].base = value;
        true
    }
}
impl EntityComponent for ContinuousStatsComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousTrackValue {
    track: ContinuousTrackId,
    #[serde(with = "crate::bits")]
    current: ContinuousValue,
}
impl ContinuousTrackValue {
    pub fn new(track: ContinuousTrackId, current: ContinuousValue) -> Self {
        Self { track, current }
    }
    pub fn track(&self) -> &ContinuousTrackId {
        &self.track
    }
    pub fn current(&self) -> ContinuousValue {
        self.current
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousTracksComponent {
    catalog_version: ContinuousCatalogVersion,
    values: Vec<ContinuousTrackValue>,
}
impl ContinuousTracksComponent {
    pub const LABEL: &'static str = "ContinuousTracksComponent";
    pub fn new(
        catalog_version: ContinuousCatalogVersion,
        mut values: Vec<ContinuousTrackValue>,
    ) -> Result<Self, ContinuousMechanicsComponentError> {
        values.sort_by(|a, b| a.track.cmp(&b.track));
        validate_unique(&values, MAX_CONTINUOUS_TRACKS_PER_ENTITY, "tracks", |v| {
            v.track.as_str()
        })?;
        Ok(Self {
            catalog_version,
            values,
        })
    }
    pub fn catalog_version(&self) -> &ContinuousCatalogVersion {
        &self.catalog_version
    }
    pub fn values(&self) -> &[ContinuousTrackValue] {
        &self.values
    }
    pub fn current(&self, id: &ContinuousTrackId) -> Option<ContinuousValue> {
        self.values
            .binary_search_by(|v| v.track.cmp(id))
            .ok()
            .map(|i| self.values[i].current())
    }
    pub(crate) fn set_current(&mut self, id: &ContinuousTrackId, value: ContinuousValue) -> bool {
        let Ok(i) = self.values.binary_search_by(|v| v.track.cmp(id)) else {
            return false;
        };
        self.values[i].current = value;
        true
    }
}
impl EntityComponent for ContinuousTracksComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousIntrinsicSourceBinding {
    instance: ContinuousSourceInstanceId,
    definition: ContinuousSourceDefinitionId,
}
impl ContinuousIntrinsicSourceBinding {
    pub fn new(
        instance: ContinuousSourceInstanceId,
        definition: ContinuousSourceDefinitionId,
    ) -> Self {
        Self {
            instance,
            definition,
        }
    }
    pub fn instance(&self) -> &ContinuousSourceInstanceId {
        &self.instance
    }
    pub fn definition(&self) -> &ContinuousSourceDefinitionId {
        &self.definition
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousIntrinsicSourcesComponent {
    catalog_version: ContinuousCatalogVersion,
    bindings: Vec<ContinuousIntrinsicSourceBinding>,
}
impl ContinuousIntrinsicSourcesComponent {
    pub const LABEL: &'static str = "ContinuousIntrinsicSourcesComponent";
    pub fn new(
        catalog_version: ContinuousCatalogVersion,
        mut bindings: Vec<ContinuousIntrinsicSourceBinding>,
    ) -> Result<Self, ContinuousMechanicsComponentError> {
        bindings.sort_by(|a, b| a.instance.cmp(&b.instance));
        validate_unique(
            &bindings,
            MAX_CONTINUOUS_INTRINSIC_SOURCES_PER_ENTITY,
            "intrinsicSources",
            |v| v.instance.as_str(),
        )?;
        Ok(Self {
            catalog_version,
            bindings,
        })
    }
    pub fn catalog_version(&self) -> &ContinuousCatalogVersion {
        &self.catalog_version
    }
    pub fn bindings(&self) -> &[ContinuousIntrinsicSourceBinding] {
        &self.bindings
    }
}
impl EntityComponent for ContinuousIntrinsicSourcesComponent {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousActiveEffectInstance {
    instance: ContinuousEffectInstanceId,
    definition: ContinuousEffectDefinitionId,
}
impl ContinuousActiveEffectInstance {
    pub fn new(
        instance: ContinuousEffectInstanceId,
        definition: ContinuousEffectDefinitionId,
    ) -> Self {
        Self {
            instance,
            definition,
        }
    }
    pub fn instance(&self) -> &ContinuousEffectInstanceId {
        &self.instance
    }
    pub fn definition(&self) -> &ContinuousEffectDefinitionId {
        &self.definition
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousActiveEffectsComponent {
    catalog_version: ContinuousCatalogVersion,
    effects: Vec<ContinuousActiveEffectInstance>,
}
impl ContinuousActiveEffectsComponent {
    pub const LABEL: &'static str = "ContinuousActiveEffectsComponent";
    pub fn new(
        catalog_version: ContinuousCatalogVersion,
        mut effects: Vec<ContinuousActiveEffectInstance>,
    ) -> Result<Self, ContinuousMechanicsComponentError> {
        effects.sort_by(|a, b| a.instance.cmp(&b.instance));
        validate_unique(
            &effects,
            MAX_CONTINUOUS_ACTIVE_EFFECTS_PER_ENTITY,
            "activeEffects",
            |v| v.instance.as_str(),
        )?;
        Ok(Self {
            catalog_version,
            effects,
        })
    }
    pub fn catalog_version(&self) -> &ContinuousCatalogVersion {
        &self.catalog_version
    }
    pub fn effects(&self) -> &[ContinuousActiveEffectInstance] {
        &self.effects
    }
    pub(crate) fn insert(
        &mut self,
        value: ContinuousActiveEffectInstance,
    ) -> Result<(), ContinuousMechanicsComponentError> {
        self.effects.push(value);
        self.effects.sort_by(|a, b| a.instance.cmp(&b.instance));
        validate_unique(
            &self.effects,
            MAX_CONTINUOUS_ACTIVE_EFFECTS_PER_ENTITY,
            "activeEffects",
            |v| v.instance.as_str(),
        )
    }
    pub(crate) fn remove(&mut self, id: &ContinuousEffectInstanceId) -> bool {
        let Ok(index) = self.effects.binary_search_by(|v| v.instance.cmp(id)) else {
            return false;
        };
        self.effects.remove(index);
        true
    }
}
impl EntityComponent for ContinuousActiveEffectsComponent {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousMechanicsComponentError {
    QuotaExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    DuplicateIdentity {
        field: &'static str,
        identity: String,
    },
    InvalidValueBits {
        field: &'static str,
        bits: u64,
    },
}
impl std::fmt::Display for ContinuousMechanicsComponentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "continuous mechanics component rejected: {self:?}")
    }
}
impl std::error::Error for ContinuousMechanicsComponentError {}

pub fn continuous_mechanics_component_registry(
) -> Result<ComponentRegistry, ComponentRegistrationError> {
    let mut registry = ComponentRegistry::new();
    register_continuous_mechanics_components(&mut registry)?;
    Ok(registry)
}
pub fn combined_gameplay_component_registry(
) -> Result<ComponentRegistry, ComponentRegistrationError> {
    let mut registry = gameplay_mechanics::gameplay_component_registry()?;
    register_continuous_mechanics_components(&mut registry)?;
    Ok(registry)
}
pub fn register_continuous_mechanics_components(
    registry: &mut ComponentRegistry,
) -> Result<(), ComponentRegistrationError> {
    let mut staged = registry.clone();
    staged.register(durable::<ContinuousStatsComponent>(
        CONTINUOUS_STATS_COMPONENT_TYPE_ID,
        CONTINUOUS_STATS_COMPONENT_CODEC_ID,
        CONTINUOUS_STATS_COMPONENT_CODEC_VERSION,
        validate_stats,
    ))?;
    staged.register(durable::<ContinuousTracksComponent>(
        CONTINUOUS_TRACKS_COMPONENT_TYPE_ID,
        CONTINUOUS_TRACKS_COMPONENT_CODEC_ID,
        CONTINUOUS_TRACKS_COMPONENT_CODEC_VERSION,
        validate_tracks,
    ))?;
    staged.register(durable::<ContinuousIntrinsicSourcesComponent>(
        CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_TYPE_ID,
        CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_CODEC_ID,
        CONTINUOUS_INTRINSIC_SOURCES_COMPONENT_CODEC_VERSION,
        validate_intrinsic,
    ))?;
    staged.register(durable::<ContinuousActiveEffectsComponent>(
        CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_TYPE_ID,
        CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_CODEC_ID,
        CONTINUOUS_ACTIVE_EFFECTS_COMPONENT_CODEC_VERSION,
        validate_effects,
    ))?;
    *registry = staged;
    Ok(())
}
fn durable<T>(
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
        |value| serde_json::to_value(value).expect("codec encode"),
        |value| serde_json::from_value(value).map_err(|error| error.to_string()),
    )
    .expect("fixed codec");
    ComponentRegistration::durable(ComponentTypeId::parse(type_id).expect("fixed id"), codec)
        .with_validator(validator)
}
fn validate_stats(value: &ContinuousStatsComponent) -> Result<(), String> {
    validate_unique(
        &value.values,
        MAX_CONTINUOUS_STATS_PER_ENTITY,
        "stats",
        |v| v.stat.as_str(),
    )
    .map_err(|e| e.to_string())
}
fn validate_tracks(value: &ContinuousTracksComponent) -> Result<(), String> {
    validate_unique(
        &value.values,
        MAX_CONTINUOUS_TRACKS_PER_ENTITY,
        "tracks",
        |v| v.track.as_str(),
    )
    .map_err(|e| e.to_string())
}
fn validate_intrinsic(value: &ContinuousIntrinsicSourcesComponent) -> Result<(), String> {
    validate_unique(
        &value.bindings,
        MAX_CONTINUOUS_INTRINSIC_SOURCES_PER_ENTITY,
        "intrinsicSources",
        |v| v.instance.as_str(),
    )
    .map_err(|e| e.to_string())
}
fn validate_effects(value: &ContinuousActiveEffectsComponent) -> Result<(), String> {
    validate_unique(
        &value.effects,
        MAX_CONTINUOUS_ACTIVE_EFFECTS_PER_ENTITY,
        "activeEffects",
        |v| v.instance.as_str(),
    )
    .map_err(|e| e.to_string())
}
fn validate_unique<T, F: Fn(&T) -> &str>(
    values: &[T],
    maximum: usize,
    field: &'static str,
    key: F,
) -> Result<(), ContinuousMechanicsComponentError> {
    if values.len() > maximum {
        return Err(ContinuousMechanicsComponentError::QuotaExceeded {
            field,
            actual: values.len(),
            maximum,
        });
    }
    for pair in values.windows(2) {
        if key(&pair[0]) >= key(&pair[1]) {
            return Err(ContinuousMechanicsComponentError::DuplicateIdentity {
                field,
                identity: key(&pair[0]).to_string(),
            });
        }
    }
    Ok(())
}
