use std::fmt::Write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    CatalogVersion, DamageKindId, EffectDefinitionId, EquipmentSlotId, ExactRatio,
    ItemDefinitionId, MechanicsScalar, SourceDefinitionId, StackingGroupId, StatId, TrackId,
};

pub const MAX_CATALOG_STATS: usize = 128;
pub const MAX_CATALOG_TRACKS: usize = 128;
pub const MAX_CATALOG_SOURCES: usize = 256;
pub const MAX_CATALOG_DAMAGE_KINDS: usize = 64;
pub const MAX_CATALOG_EFFECTS: usize = 128;
pub const MAX_CATALOG_ITEMS: usize = 256;
pub const MAX_CATALOG_EQUIPMENT_SLOTS: usize = 64;
pub const MAX_STAT_CONTRIBUTIONS_PER_SOURCE: usize = 32;
pub const MAX_RESPONSES_PER_SOURCE: usize = 32;
pub const MAX_ABS_SOURCE_PRIORITY: i16 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StatDefinition {
    pub id: StatId,
    pub minimum: MechanicsScalar,
    pub maximum: MechanicsScalar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum TrackMaximum {
    Fixed { value: MechanicsScalar },
    Stat { stat: StatId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TrackDefinition {
    pub id: TrackId,
    pub minimum: MechanicsScalar,
    pub maximum: TrackMaximum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StackingPolicy {
    Sum,
    Highest,
    Lowest,
    UniqueBySource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StatContributionDefinition {
    pub stat: StatId,
    pub amount: MechanicsScalar,
    pub stacking_group: StackingGroupId,
    pub stacking: StackingPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum DamageKindSelector {
    Any,
    Exact { damage_kind: DamageKindId },
}

impl DamageKindSelector {
    pub fn matches(&self, damage_kind: &DamageKindId) -> bool {
        match self {
            Self::Any => true,
            Self::Exact {
                damage_kind: expected,
            } => expected == damage_kind,
        }
    }

    fn referenced_kind(&self) -> Option<&DamageKindId> {
        match self {
            Self::Any => None,
            Self::Exact { damage_kind } => Some(damage_kind),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum DamageResponseDefinition {
    Prevent {
        selector: DamageKindSelector,
        stacking_group: StackingGroupId,
        stacking: StackingPolicy,
    },
    FlatReduction {
        selector: DamageKindSelector,
        amount: MechanicsScalar,
        stacking_group: StackingGroupId,
        stacking: StackingPolicy,
    },
    Scale {
        selector: DamageKindSelector,
        ratio: ExactRatio,
        stacking_group: StackingGroupId,
        stacking: StackingPolicy,
    },
    Absorb {
        selector: DamageKindSelector,
        track: TrackId,
    },
}

impl DamageResponseDefinition {
    pub fn selector(&self) -> &DamageKindSelector {
        match self {
            Self::Prevent { selector, .. }
            | Self::FlatReduction { selector, .. }
            | Self::Scale { selector, .. }
            | Self::Absorb { selector, .. } => selector,
        }
    }

    pub fn stacking(&self) -> Option<(&StackingGroupId, StackingPolicy)> {
        match self {
            Self::Prevent {
                stacking_group,
                stacking,
                ..
            }
            | Self::FlatReduction {
                stacking_group,
                stacking,
                ..
            }
            | Self::Scale {
                stacking_group,
                stacking,
                ..
            } => Some((stacking_group, *stacking)),
            Self::Absorb { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourceDefinition {
    pub id: SourceDefinitionId,
    pub priority: i16,
    pub stat_contributions: Vec<StatContributionDefinition>,
    pub damage_responses: Vec<DamageResponseDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DamageKindDefinition {
    pub id: DamageKindId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EffectDefinition {
    pub id: EffectDefinitionId,
    pub sources: Vec<SourceDefinitionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    Fungible,
    Unique,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemDefinition {
    pub id: ItemDefinitionId,
    pub kind: ItemKind,
    pub sources: Vec<SourceDefinitionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EquipmentSlotDefinition {
    pub id: EquipmentSlotId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MechanicsCatalogDefinition {
    pub version: CatalogVersion,
    pub stats: Vec<StatDefinition>,
    pub tracks: Vec<TrackDefinition>,
    pub sources: Vec<SourceDefinition>,
    pub damage_kinds: Vec<DamageKindDefinition>,
    pub effects: Vec<EffectDefinition>,
    pub items: Vec<ItemDefinition>,
    pub equipment_slots: Vec<EquipmentSlotDefinition>,
}

#[derive(Debug, Clone)]
pub struct MechanicsCatalog {
    definition: MechanicsCatalogDefinition,
    fingerprint: String,
}

impl MechanicsCatalog {
    pub fn admit(mut definition: MechanicsCatalogDefinition) -> Result<Self, CatalogError> {
        enforce_quota("stats", definition.stats.len(), MAX_CATALOG_STATS)?;
        enforce_quota("tracks", definition.tracks.len(), MAX_CATALOG_TRACKS)?;
        enforce_quota("sources", definition.sources.len(), MAX_CATALOG_SOURCES)?;
        enforce_quota(
            "damageKinds",
            definition.damage_kinds.len(),
            MAX_CATALOG_DAMAGE_KINDS,
        )?;
        enforce_quota("effects", definition.effects.len(), MAX_CATALOG_EFFECTS)?;
        enforce_quota("items", definition.items.len(), MAX_CATALOG_ITEMS)?;
        enforce_quota(
            "equipmentSlots",
            definition.equipment_slots.len(),
            MAX_CATALOG_EQUIPMENT_SLOTS,
        )?;

        definition
            .stats
            .sort_by(|left, right| left.id.cmp(&right.id));
        definition
            .tracks
            .sort_by(|left, right| left.id.cmp(&right.id));
        definition
            .sources
            .sort_by(|left, right| left.id.cmp(&right.id));
        definition
            .damage_kinds
            .sort_by(|left, right| left.id.cmp(&right.id));
        definition
            .effects
            .sort_by(|left, right| left.id.cmp(&right.id));
        definition
            .items
            .sort_by(|left, right| left.id.cmp(&right.id));
        definition
            .equipment_slots
            .sort_by(|left, right| left.id.cmp(&right.id));

        reject_duplicates(&definition.stats, |value| value.id.as_str(), "stat")?;
        reject_duplicates(&definition.tracks, |value| value.id.as_str(), "track")?;
        reject_duplicates(&definition.sources, |value| value.id.as_str(), "source")?;
        reject_duplicates(
            &definition.damage_kinds,
            |value| value.id.as_str(),
            "damage kind",
        )?;
        reject_duplicates(&definition.effects, |value| value.id.as_str(), "effect")?;
        reject_duplicates(&definition.items, |value| value.id.as_str(), "item")?;
        reject_duplicates(
            &definition.equipment_slots,
            |value| value.id.as_str(),
            "equipment slot",
        )?;

        for stat in &definition.stats {
            if stat.minimum > stat.maximum {
                return Err(CatalogError::InvalidBounds {
                    definition: stat.id.to_string(),
                    minimum: stat.minimum.get(),
                    maximum: stat.maximum.get(),
                });
            }
        }
        for track in &definition.tracks {
            match &track.maximum {
                TrackMaximum::Fixed { value } => {
                    if track.minimum > *value {
                        return Err(CatalogError::InvalidBounds {
                            definition: track.id.to_string(),
                            minimum: track.minimum.get(),
                            maximum: value.get(),
                        });
                    }
                }
                TrackMaximum::Stat { stat } => {
                    if !contains_id(&definition.stats, stat, |value| &value.id) {
                        return Err(CatalogError::UnknownReference {
                            owner: track.id.to_string(),
                            namespace: "stat",
                            reference: stat.to_string(),
                        });
                    }
                }
            }
        }

        for source in &definition.sources {
            if source.priority.unsigned_abs() > MAX_ABS_SOURCE_PRIORITY as u16 {
                return Err(CatalogError::PriorityOutOfRange {
                    source: source.id.clone(),
                    priority: source.priority,
                });
            }
            enforce_quota(
                "statContributionsPerSource",
                source.stat_contributions.len(),
                MAX_STAT_CONTRIBUTIONS_PER_SOURCE,
            )?;
            enforce_quota(
                "damageResponsesPerSource",
                source.damage_responses.len(),
                MAX_RESPONSES_PER_SOURCE,
            )?;
            for contribution in &source.stat_contributions {
                if !contains_id(&definition.stats, &contribution.stat, |value| &value.id) {
                    return Err(CatalogError::UnknownReference {
                        owner: source.id.to_string(),
                        namespace: "stat",
                        reference: contribution.stat.to_string(),
                    });
                }
            }
            for response in &source.damage_responses {
                if let Some(kind) = response.selector().referenced_kind() {
                    if !contains_id(&definition.damage_kinds, kind, |value| &value.id) {
                        return Err(CatalogError::UnknownReference {
                            owner: source.id.to_string(),
                            namespace: "damage kind",
                            reference: kind.to_string(),
                        });
                    }
                }
                match response {
                    DamageResponseDefinition::FlatReduction { amount, .. } => {
                        amount.require_nonnegative().map_err(|_| {
                            CatalogError::NegativeResponseAmount {
                                source: source.id.clone(),
                                amount: amount.get(),
                            }
                        })?;
                    }
                    DamageResponseDefinition::Absorb { track, .. } => {
                        if !contains_id(&definition.tracks, track, |value| &value.id) {
                            return Err(CatalogError::UnknownReference {
                                owner: source.id.to_string(),
                                namespace: "track",
                                reference: track.to_string(),
                            });
                        }
                    }
                    DamageResponseDefinition::Prevent { .. }
                    | DamageResponseDefinition::Scale { .. } => {}
                }
            }
            validate_stacking_contract(source)?;
        }
        validate_global_stacking_contract(&definition.sources)?;

        for effect in &definition.effects {
            reject_duplicate_references(&effect.sources, effect.id.as_str(), "source")?;
            validate_source_references(&definition.sources, effect.id.as_str(), &effect.sources)?;
        }
        for item in &definition.items {
            reject_duplicate_references(&item.sources, item.id.as_str(), "source")?;
            validate_source_references(&definition.sources, item.id.as_str(), &item.sources)?;
        }

        let bytes = serde_json::to_vec(&definition)
            .expect("catalog definition serialization is infallible");
        let digest = Sha256::digest(bytes);
        let mut fingerprint = String::with_capacity(71);
        fingerprint.push_str("sha256:");
        for byte in digest {
            write!(&mut fingerprint, "{byte:02x}").expect("string formatting is infallible");
        }

        Ok(Self {
            definition,
            fingerprint,
        })
    }

    pub fn version(&self) -> &CatalogVersion {
        &self.definition.version
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn stat(&self, id: &StatId) -> Option<&StatDefinition> {
        find_by_id(&self.definition.stats, id, |value| &value.id)
    }

    pub fn track(&self, id: &TrackId) -> Option<&TrackDefinition> {
        find_by_id(&self.definition.tracks, id, |value| &value.id)
    }

    pub fn source(&self, id: &SourceDefinitionId) -> Option<&SourceDefinition> {
        find_by_id(&self.definition.sources, id, |value| &value.id)
    }

    pub fn damage_kind(&self, id: &DamageKindId) -> Option<&DamageKindDefinition> {
        find_by_id(&self.definition.damage_kinds, id, |value| &value.id)
    }

    pub fn effect(&self, id: &EffectDefinitionId) -> Option<&EffectDefinition> {
        find_by_id(&self.definition.effects, id, |value| &value.id)
    }

    pub fn item(&self, id: &ItemDefinitionId) -> Option<&ItemDefinition> {
        find_by_id(&self.definition.items, id, |value| &value.id)
    }

    pub fn equipment_slot(&self, id: &EquipmentSlotId) -> Option<&EquipmentSlotDefinition> {
        find_by_id(&self.definition.equipment_slots, id, |value| &value.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    QuotaExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    DuplicateDefinition {
        namespace: &'static str,
        identity: String,
    },
    DuplicateReference {
        owner: String,
        namespace: &'static str,
        reference: String,
    },
    UnknownReference {
        owner: String,
        namespace: &'static str,
        reference: String,
    },
    InvalidBounds {
        definition: String,
        minimum: i64,
        maximum: i64,
    },
    PriorityOutOfRange {
        source: SourceDefinitionId,
        priority: i16,
    },
    NegativeResponseAmount {
        source: SourceDefinitionId,
        amount: i64,
    },
    InconsistentStackingPolicy {
        source: SourceDefinitionId,
        group: StackingGroupId,
    },
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "mechanics catalog rejected: {self:?}")
    }
}

impl std::error::Error for CatalogError {}

fn enforce_quota(field: &'static str, actual: usize, maximum: usize) -> Result<(), CatalogError> {
    if actual > maximum {
        return Err(CatalogError::QuotaExceeded {
            field,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn reject_duplicates<T>(
    values: &[T],
    identity: impl Fn(&T) -> &str,
    namespace: &'static str,
) -> Result<(), CatalogError> {
    for pair in values.windows(2) {
        if identity(&pair[0]) == identity(&pair[1]) {
            return Err(CatalogError::DuplicateDefinition {
                namespace,
                identity: identity(&pair[0]).to_string(),
            });
        }
    }
    Ok(())
}

fn reject_duplicate_references<T: Ord + ToString + Clone>(
    values: &[T],
    owner: &str,
    namespace: &'static str,
) -> Result<(), CatalogError> {
    let mut sorted = values.to_vec();
    sorted.sort();
    for pair in sorted.windows(2) {
        if pair[0] == pair[1] {
            return Err(CatalogError::DuplicateReference {
                owner: owner.to_string(),
                namespace,
                reference: pair[0].to_string(),
            });
        }
    }
    Ok(())
}

fn validate_source_references(
    sources: &[SourceDefinition],
    owner: &str,
    references: &[SourceDefinitionId],
) -> Result<(), CatalogError> {
    for reference in references {
        if !contains_id(sources, reference, |value| &value.id) {
            return Err(CatalogError::UnknownReference {
                owner: owner.to_string(),
                namespace: "source",
                reference: reference.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_stacking_contract(source: &SourceDefinition) -> Result<(), CatalogError> {
    let mut policies: Vec<(&StackingGroupId, StackingPolicy)> = source
        .stat_contributions
        .iter()
        .map(|value| (&value.stacking_group, value.stacking))
        .chain(
            source
                .damage_responses
                .iter()
                .filter_map(DamageResponseDefinition::stacking),
        )
        .collect();
    policies.sort_by(|left, right| left.0.cmp(right.0));
    for pair in policies.windows(2) {
        if pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1 {
            return Err(CatalogError::InconsistentStackingPolicy {
                source: source.id.clone(),
                group: pair[0].0.clone(),
            });
        }
    }
    Ok(())
}

fn validate_global_stacking_contract(sources: &[SourceDefinition]) -> Result<(), CatalogError> {
    let mut policies: Vec<(&StackingGroupId, StackingPolicy, &SourceDefinitionId)> = sources
        .iter()
        .flat_map(|source| {
            source
                .stat_contributions
                .iter()
                .map(|value| (&value.stacking_group, value.stacking, &source.id))
                .chain(source.damage_responses.iter().filter_map(|value| {
                    value
                        .stacking()
                        .map(|(group, policy)| (group, policy, &source.id))
                }))
        })
        .collect();
    policies.sort_by(|left, right| left.0.cmp(right.0));
    for pair in policies.windows(2) {
        if pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1 {
            return Err(CatalogError::InconsistentStackingPolicy {
                source: pair[1].2.clone(),
                group: pair[1].0.clone(),
            });
        }
    }
    Ok(())
}

fn contains_id<'a, T, I: Ord + 'a>(
    values: &'a [T],
    id: &I,
    identity: impl Fn(&'a T) -> &'a I,
) -> bool {
    find_by_id(values, id, identity).is_some()
}

fn find_by_id<'a, T, I: Ord + 'a>(
    values: &'a [T],
    id: &I,
    identity: impl Fn(&'a T) -> &'a I,
) -> Option<&'a T> {
    values
        .binary_search_by(|value| identity(value).cmp(id))
        .ok()
        .map(|index| &values[index])
}

#[cfg(test)]
mod tests {
    use super::{
        CatalogError, DamageKindDefinition, EquipmentSlotDefinition, ItemDefinition, ItemKind,
        MechanicsCatalog, MechanicsCatalogDefinition, StatDefinition, TrackDefinition,
        TrackMaximum,
    };
    use crate::{
        CatalogVersion, DamageKindId, EquipmentSlotId, ItemDefinitionId, MechanicsScalar, StatId,
        TrackId,
    };

    fn scalar(value: i64) -> MechanicsScalar {
        MechanicsScalar::new(value).unwrap()
    }

    fn minimal_definition() -> MechanicsCatalogDefinition {
        MechanicsCatalogDefinition {
            version: CatalogVersion::parse("test.v1").unwrap(),
            stats: vec![StatDefinition {
                id: StatId::parse("maximum_health").unwrap(),
                minimum: scalar(1),
                maximum: scalar(1_000),
            }],
            tracks: vec![TrackDefinition {
                id: TrackId::parse("health").unwrap(),
                minimum: scalar(0),
                maximum: TrackMaximum::Stat {
                    stat: StatId::parse("maximum_health").unwrap(),
                },
            }],
            sources: vec![],
            damage_kinds: vec![DamageKindDefinition {
                id: DamageKindId::parse("impact").unwrap(),
            }],
            effects: vec![],
            items: vec![ItemDefinition {
                id: ItemDefinitionId::parse("armor").unwrap(),
                kind: ItemKind::Unique,
                sources: vec![],
            }],
            equipment_slots: vec![EquipmentSlotDefinition {
                id: EquipmentSlotId::parse("body").unwrap(),
            }],
        }
    }

    #[test]
    fn admission_sorts_definitions_and_has_stable_fingerprint() {
        let first = MechanicsCatalog::admit(minimal_definition()).unwrap();
        let second = MechanicsCatalog::admit(minimal_definition()).unwrap();
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert!(first.track(&TrackId::parse("health").unwrap()).is_some());
    }

    #[test]
    fn admission_rejects_duplicate_and_unresolved_definitions() {
        let mut duplicate = minimal_definition();
        duplicate.stats.push(duplicate.stats[0].clone());
        assert!(matches!(
            MechanicsCatalog::admit(duplicate),
            Err(CatalogError::DuplicateDefinition { .. })
        ));

        let mut unresolved = minimal_definition();
        unresolved.tracks[0].maximum = TrackMaximum::Stat {
            stat: StatId::parse("missing").unwrap(),
        };
        assert!(matches!(
            MechanicsCatalog::admit(unresolved),
            Err(CatalogError::UnknownReference { .. })
        ));
    }
}
