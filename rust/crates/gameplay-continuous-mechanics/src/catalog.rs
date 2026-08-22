use gameplay_standard::ContinuousValue;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ContinuousCatalogVersion, ContinuousEffectDefinitionId, ContinuousSourceDefinitionId,
    ContinuousStackingGroupId, ContinuousStatId, ContinuousTrackId,
};

pub const MAX_CONTINUOUS_CATALOG_STATS: usize = 128;
pub const MAX_CONTINUOUS_CATALOG_TRACKS: usize = 128;
pub const MAX_CONTINUOUS_CATALOG_SOURCES: usize = 256;
pub const MAX_CONTINUOUS_CATALOG_EFFECTS: usize = 128;
pub const MAX_CONTINUOUS_CONTRIBUTIONS_PER_SOURCE: usize = 32;
pub const MAX_CONTINUOUS_SOURCES_PER_EFFECT: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousStatDefinition {
    pub id: ContinuousStatId,
    #[serde(with = "crate::bits")]
    minimum: ContinuousValue,
    #[serde(with = "crate::bits")]
    maximum: ContinuousValue,
}
impl ContinuousStatDefinition {
    pub fn new(
        id: ContinuousStatId,
        minimum: ContinuousValue,
        maximum: ContinuousValue,
    ) -> Result<Self, ContinuousCatalogError> {
        if minimum > maximum {
            return Err(ContinuousCatalogError::InvalidBounds {
                subject: id.to_string(),
                minimum: minimum.bits(),
                maximum: maximum.bits(),
            });
        }
        Ok(Self {
            id,
            minimum,
            maximum,
        })
    }
    pub fn minimum(&self) -> ContinuousValue {
        self.minimum
    }
    pub fn maximum(&self) -> ContinuousValue {
        self.maximum
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContinuousTrackMaximum {
    Fixed {
        #[serde(with = "crate::bits")]
        value: ContinuousValue,
    },
    Stat {
        stat: ContinuousStatId,
    },
}
impl ContinuousTrackMaximum {
    pub fn fixed(value: ContinuousValue) -> Self {
        Self::Fixed { value }
    }
    pub fn fixed_value(&self) -> Option<ContinuousValue> {
        match self {
            Self::Fixed { value } => Some(*value),
            Self::Stat { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousTrackDefinition {
    pub id: ContinuousTrackId,
    #[serde(with = "crate::bits")]
    minimum: ContinuousValue,
    pub maximum: ContinuousTrackMaximum,
}
impl ContinuousTrackDefinition {
    pub fn new(
        id: ContinuousTrackId,
        minimum: ContinuousValue,
        maximum: ContinuousTrackMaximum,
    ) -> Result<Self, ContinuousCatalogError> {
        if let ContinuousTrackMaximum::Fixed { value } = maximum {
            if minimum > value {
                return Err(ContinuousCatalogError::InvalidBounds {
                    subject: id.to_string(),
                    minimum: minimum.bits(),
                    maximum: value.bits(),
                });
            }
        }
        Ok(Self {
            id,
            minimum,
            maximum,
        })
    }
    pub fn minimum(&self) -> ContinuousValue {
        self.minimum
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContinuousStackingPolicy {
    Sum,
    Highest,
    Lowest,
    UniqueBySource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContinuousStatContribution {
    Add {
        #[serde(with = "crate::bits")]
        amount: ContinuousValue,
    },
    Minimum {
        #[serde(with = "crate::bits")]
        value: ContinuousValue,
    },
    Maximum {
        #[serde(with = "crate::bits")]
        value: ContinuousValue,
    },
}
impl ContinuousStatContribution {
    pub fn add(amount: ContinuousValue) -> Self {
        Self::Add { amount }
    }
    pub fn minimum(value: ContinuousValue) -> Self {
        Self::Minimum { value }
    }
    pub fn maximum(value: ContinuousValue) -> Self {
        Self::Maximum { value }
    }
    pub fn value(&self) -> ContinuousValue {
        match self {
            Self::Add { amount } => *amount,
            Self::Minimum { value } | Self::Maximum { value } => *value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousStatContributionDefinition {
    pub stat: ContinuousStatId,
    pub contribution: ContinuousStatContribution,
    pub stacking_group: ContinuousStackingGroupId,
    pub stacking: ContinuousStackingPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousSourceDefinition {
    pub id: ContinuousSourceDefinitionId,
    pub priority: i16,
    pub stat_contributions: Vec<ContinuousStatContributionDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousEffectDefinition {
    pub id: ContinuousEffectDefinitionId,
    pub sources: Vec<ContinuousSourceDefinitionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContinuousMechanicsCatalogDefinition {
    pub version: ContinuousCatalogVersion,
    pub stats: Vec<ContinuousStatDefinition>,
    pub tracks: Vec<ContinuousTrackDefinition>,
    pub sources: Vec<ContinuousSourceDefinition>,
    pub effects: Vec<ContinuousEffectDefinition>,
}

#[derive(Debug, Clone)]
pub struct ContinuousMechanicsCatalog {
    definition: ContinuousMechanicsCatalogDefinition,
    fingerprint: String,
}
impl ContinuousMechanicsCatalog {
    pub fn admit(
        mut definition: ContinuousMechanicsCatalogDefinition,
    ) -> Result<Self, ContinuousCatalogError> {
        quota(
            "stats",
            definition.stats.len(),
            MAX_CONTINUOUS_CATALOG_STATS,
        )?;
        quota(
            "tracks",
            definition.tracks.len(),
            MAX_CONTINUOUS_CATALOG_TRACKS,
        )?;
        quota(
            "sources",
            definition.sources.len(),
            MAX_CONTINUOUS_CATALOG_SOURCES,
        )?;
        quota(
            "effects",
            definition.effects.len(),
            MAX_CONTINUOUS_CATALOG_EFFECTS,
        )?;
        definition.stats.sort_by(|a, b| a.id.cmp(&b.id));
        definition.tracks.sort_by(|a, b| a.id.cmp(&b.id));
        definition.sources.sort_by(|a, b| a.id.cmp(&b.id));
        definition.effects.sort_by(|a, b| a.id.cmp(&b.id));
        unique("stats", &definition.stats, |v| v.id.as_str())?;
        unique("tracks", &definition.tracks, |v| v.id.as_str())?;
        unique("sources", &definition.sources, |v| v.id.as_str())?;
        unique("effects", &definition.effects, |v| v.id.as_str())?;
        for stat in &definition.stats {
            if stat.minimum() > stat.maximum() {
                return Err(ContinuousCatalogError::InvalidBounds {
                    subject: stat.id.to_string(),
                    minimum: stat.minimum().bits(),
                    maximum: stat.maximum().bits(),
                });
            }
        }
        for track in &definition.tracks {
            if let ContinuousTrackMaximum::Fixed { value } = track.maximum {
                if track.minimum() > value {
                    return Err(ContinuousCatalogError::InvalidBounds {
                        subject: track.id.to_string(),
                        minimum: track.minimum().bits(),
                        maximum: value.bits(),
                    });
                }
            } else if let ContinuousTrackMaximum::Stat { ref stat } = track.maximum {
                if !definition
                    .stats
                    .iter()
                    .any(|candidate| &candidate.id == stat)
                {
                    return Err(ContinuousCatalogError::UnknownReference {
                        field: "track maximum stat",
                        reference: stat.to_string(),
                    });
                }
            }
        }
        for source in &mut definition.sources {
            quota(
                "source stat contributions",
                source.stat_contributions.len(),
                MAX_CONTINUOUS_CONTRIBUTIONS_PER_SOURCE,
            )?;
            source.stat_contributions.sort_by(|a, b| {
                (a.stat.as_str(), a.stacking_group.as_str(), &a.contribution).cmp(&(
                    b.stat.as_str(),
                    b.stacking_group.as_str(),
                    &b.contribution,
                ))
            });
            for contribution in &source.stat_contributions {
                if !definition
                    .stats
                    .iter()
                    .any(|stat| stat.id == contribution.stat)
                {
                    return Err(ContinuousCatalogError::UnknownReference {
                        field: "source stat",
                        reference: contribution.stat.to_string(),
                    });
                }
            }
        }
        let mut policies = std::collections::BTreeMap::new();
        for source in &definition.sources {
            for contribution in &source.stat_contributions {
                let kind = match contribution.contribution {
                    ContinuousStatContribution::Add { .. } => "add",
                    ContinuousStatContribution::Minimum { .. } => "minimum",
                    ContinuousStatContribution::Maximum { .. } => "maximum",
                };
                let key = (
                    contribution.stat.to_string(),
                    contribution.stacking_group.to_string(),
                    kind,
                );
                if let Some(existing) = policies.insert(key.clone(), contribution.stacking) {
                    if existing != contribution.stacking {
                        return Err(ContinuousCatalogError::MixedStackingPolicy {
                            stat: key.0,
                            group: key.1,
                        });
                    }
                }
            }
        }
        for effect in &mut definition.effects {
            quota(
                "effect sources",
                effect.sources.len(),
                MAX_CONTINUOUS_SOURCES_PER_EFFECT,
            )?;
            effect.sources.sort();
            unique("effect sources", &effect.sources, |v| v.as_str())?;
            for source in &effect.sources {
                if !definition
                    .sources
                    .iter()
                    .any(|candidate| &candidate.id == source)
                {
                    return Err(ContinuousCatalogError::UnknownReference {
                        field: "effect source",
                        reference: source.to_string(),
                    });
                }
            }
        }
        let canonical = serde_json::to_vec(&FingerprintDefinition {
            stats: &definition.stats,
            tracks: &definition.tracks,
            sources: &definition.sources,
            effects: &definition.effects,
        })
        .map_err(|_| ContinuousCatalogError::CanonicalEncoding)?;
        let mut hash = Sha256::new();
        hash.update(canonical);
        let fingerprint = format!("sha256:{:x}", hash.finalize());
        Ok(Self {
            definition,
            fingerprint,
        })
    }
    pub fn version(&self) -> &ContinuousCatalogVersion {
        &self.definition.version
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub fn stat(&self, id: &ContinuousStatId) -> Option<&ContinuousStatDefinition> {
        self.definition
            .stats
            .binary_search_by(|v| v.id.cmp(id))
            .ok()
            .map(|i| &self.definition.stats[i])
    }
    pub fn track(&self, id: &ContinuousTrackId) -> Option<&ContinuousTrackDefinition> {
        self.definition
            .tracks
            .binary_search_by(|v| v.id.cmp(id))
            .ok()
            .map(|i| &self.definition.tracks[i])
    }
    pub fn source(&self, id: &ContinuousSourceDefinitionId) -> Option<&ContinuousSourceDefinition> {
        self.definition
            .sources
            .binary_search_by(|v| v.id.cmp(id))
            .ok()
            .map(|i| &self.definition.sources[i])
    }
    pub fn effect(&self, id: &ContinuousEffectDefinitionId) -> Option<&ContinuousEffectDefinition> {
        self.definition
            .effects
            .binary_search_by(|v| v.id.cmp(id))
            .ok()
            .map(|i| &self.definition.effects[i])
    }
    pub fn definition(&self) -> &ContinuousMechanicsCatalogDefinition {
        &self.definition
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintDefinition<'a> {
    stats: &'a [ContinuousStatDefinition],
    tracks: &'a [ContinuousTrackDefinition],
    sources: &'a [ContinuousSourceDefinition],
    effects: &'a [ContinuousEffectDefinition],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousCatalogError {
    QuotaExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    DuplicateIdentity {
        field: &'static str,
        identity: String,
    },
    InvalidBits {
        field: &'static str,
        bits: u64,
    },
    InvalidBounds {
        subject: String,
        minimum: u64,
        maximum: u64,
    },
    UnknownReference {
        field: &'static str,
        reference: String,
    },
    MixedStackingPolicy {
        stat: String,
        group: String,
    },
    CanonicalEncoding,
}
impl std::fmt::Display for ContinuousCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "continuous mechanics catalog rejected: {self:?}")
    }
}
impl std::error::Error for ContinuousCatalogError {}
fn quota(field: &'static str, actual: usize, maximum: usize) -> Result<(), ContinuousCatalogError> {
    if actual > maximum {
        Err(ContinuousCatalogError::QuotaExceeded {
            field,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}
fn unique<T, F: Fn(&T) -> &str>(
    field: &'static str,
    values: &[T],
    key: F,
) -> Result<(), ContinuousCatalogError> {
    for pair in values.windows(2) {
        if key(&pair[0]) == key(&pair[1]) {
            return Err(ContinuousCatalogError::DuplicateIdentity {
                field,
                identity: key(&pair[0]).to_string(),
            });
        }
    }
    Ok(())
}
