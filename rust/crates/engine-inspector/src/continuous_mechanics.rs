use core_ids::EntityId;
use entity_state::EntityState;
use gameplay_continuous_mechanics::{
    ContinuousMechanicsCatalog, ContinuousMechanicsComponentKind, ContinuousStatsComponent,
    ContinuousTrackMaximum, ContinuousTracksComponent,
};
use serde::Serialize;

/// Continuous values are reported as normalized binary64 bits. Decimal rendering is deliberately
/// left to callers so the inspection identity is never mistaken for approximate equality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuousMechanicsComponentInspection {
    pub numeric_family: &'static str,
    pub kind: String,
    pub type_id: String,
    pub codec_id: String,
    pub codec_version: u32,
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    pub entry_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuousMechanicsStoredStatInspection {
    pub id: String,
    pub base_bits: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuousMechanicsStoredTrackInspection {
    pub id: String,
    pub current_bits: u64,
    pub declared_minimum_bits: u64,
    pub declared_maximum: ContinuousMechanicsTrackMaximumInspection,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContinuousMechanicsTrackMaximumInspection {
    Fixed { bits: u64 },
    Stat { stat: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuousMechanicsEntityInspection {
    pub numeric_family: &'static str,
    pub entity: u64,
    pub catalog_version: String,
    pub catalog_fingerprint: String,
    pub components: Vec<ContinuousMechanicsComponentInspection>,
    pub stored_stats: Vec<ContinuousMechanicsStoredStatInspection>,
    pub stored_tracks: Vec<ContinuousMechanicsStoredTrackInspection>,
}

pub fn inspect_continuous_mechanics_entity_structural(
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
    entity: EntityId,
) -> Result<
    ContinuousMechanicsEntityInspection,
    gameplay_continuous_mechanics::ContinuousMechanicsError,
> {
    let mut components = Vec::new();
    for kind in ContinuousMechanicsComponentKind::ALL {
        let (present, revision, entry_count) = match kind {
            ContinuousMechanicsComponentKind::Stats => match state.component::<ContinuousStatsComponent>(entity)? { Some(value) => (true, Some(state.component_revision::<ContinuousStatsComponent>(entity)?.revision()), value.values().len()), None => (false, None, 0) },
            ContinuousMechanicsComponentKind::Tracks => match state.component::<ContinuousTracksComponent>(entity)? { Some(value) => (true, Some(state.component_revision::<ContinuousTracksComponent>(entity)?.revision()), value.values().len()), None => (false, None, 0) },
            ContinuousMechanicsComponentKind::IntrinsicSources => match state.component::<gameplay_continuous_mechanics::ContinuousIntrinsicSourcesComponent>(entity)? { Some(value) => (true, Some(state.component_revision::<gameplay_continuous_mechanics::ContinuousIntrinsicSourcesComponent>(entity)?.revision()), value.bindings().len()), None => (false, None, 0) },
            ContinuousMechanicsComponentKind::ActiveEffects => match state.component::<gameplay_continuous_mechanics::ContinuousActiveEffectsComponent>(entity)? { Some(value) => (true, Some(state.component_revision::<gameplay_continuous_mechanics::ContinuousActiveEffectsComponent>(entity)?.revision()), value.effects().len()), None => (false, None, 0) },
        };
        components.push(ContinuousMechanicsComponentInspection {
            numeric_family: "continuous-binary64",
            kind: kind.label().to_string(),
            type_id: kind.type_id().to_string(),
            codec_id: kind.codec_id().to_string(),
            codec_version: kind.codec_version(),
            present,
            revision,
            entry_count,
        });
    }
    let stored_stats =
        state
            .component::<ContinuousStatsComponent>(entity)?
            .map_or(Vec::new(), |component| {
                component
                    .values()
                    .iter()
                    .map(|value| ContinuousMechanicsStoredStatInspection {
                        id: value.stat().to_string(),
                        base_bits: value.base().bits(),
                    })
                    .collect()
            });
    let mut stored_tracks = Vec::new();
    if let Some(component) = state.component::<ContinuousTracksComponent>(entity)? {
        for value in component.values() {
            let definition = catalog.track(value.track()).ok_or_else(|| {
                gameplay_continuous_mechanics::ContinuousMechanicsError::UnknownTrack(
                    value.track().clone(),
                )
            })?;
            let declared_maximum = match &definition.maximum {
                ContinuousTrackMaximum::Fixed { value } => {
                    ContinuousMechanicsTrackMaximumInspection::Fixed { bits: value.bits() }
                }
                ContinuousTrackMaximum::Stat { stat } => {
                    ContinuousMechanicsTrackMaximumInspection::Stat {
                        stat: stat.to_string(),
                    }
                }
            };
            stored_tracks.push(ContinuousMechanicsStoredTrackInspection {
                id: value.track().to_string(),
                current_bits: value.current().bits(),
                declared_minimum_bits: definition.minimum().bits(),
                declared_maximum,
            });
        }
    }
    Ok(ContinuousMechanicsEntityInspection {
        numeric_family: "continuous-binary64",
        entity: entity.raw(),
        catalog_version: catalog.version().to_string(),
        catalog_fingerprint: catalog.fingerprint().to_string(),
        components,
        stored_stats,
        stored_tracks,
    })
}
