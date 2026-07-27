use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use entity_state::EntityState;

use crate::source::collect_active_sources;
use crate::{
    DecisionOutcome, MechanicsCatalog, MechanicsComponentKind, MechanicsError, MechanicsScalar,
    ObservedComponentRevision, OperationId, RequestSource, SourceCollectionCost,
    SourceDefinitionId, SourceInstanceIdentity, StackingGroupId, StackingPolicy, StatId,
    StatsComponent, TrackDefinition, TrackId, TrackMaximum, MAX_REQUEST_SOURCES,
};

pub const MAX_STAT_DECISIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatDecision {
    pub source: SourceInstanceIdentity,
    pub source_definition: SourceDefinitionId,
    pub contribution_index: Option<u16>,
    pub outcome: DecisionOutcome,
    pub amount: Option<MechanicsScalar>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatEvaluation {
    pub catalog_version: crate::CatalogVersion,
    pub catalog_fingerprint: String,
    pub entity: EntityId,
    pub stat: StatId,
    pub base: MechanicsScalar,
    pub unconstrained: MechanicsScalar,
    pub value: MechanicsScalar,
    pub decisions: Vec<StatDecision>,
    pub observed_revisions: Vec<ObservedComponentRevision>,
    pub source_cost: SourceCollectionCost,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StatService;

impl StatService {
    pub fn evaluate(
        state: &EntityState,
        catalog: &MechanicsCatalog,
        entity: EntityId,
        stat: &StatId,
        operation: &OperationId,
        request_sources: &[RequestSource],
    ) -> Result<StatEvaluation, MechanicsError> {
        if request_sources.len() > MAX_REQUEST_SOURCES {
            return Err(MechanicsError::RequestQuotaExceeded {
                field: "requestSources",
                actual: request_sources.len(),
                maximum: MAX_REQUEST_SOURCES,
            });
        }
        let stat_definition = catalog
            .stat(stat)
            .ok_or_else(|| MechanicsError::UnknownStat { stat: stat.clone() })?;
        let component =
            state
                .component::<StatsComponent>(entity)?
                .ok_or(MechanicsError::MissingComponent {
                    entity,
                    component: StatsComponent::LABEL,
                })?;
        crate::source::ensure_catalog_version(
            catalog,
            entity,
            StatsComponent::LABEL,
            component.catalog_version(),
        )?;
        let base = component
            .base(stat)
            .ok_or_else(|| MechanicsError::MissingStat {
                entity,
                stat: stat.clone(),
            })?;

        let (sources, source_cost, mut observed_revisions) =
            collect_active_sources(state, catalog, entity, operation, request_sources)?;
        observed_revisions.push(ObservedComponentRevision {
            entity,
            component: MechanicsComponentKind::Stats,
            revision: state
                .component_revision::<StatsComponent>(entity)?
                .revision(),
        });
        observed_revisions.sort_by_key(|value| (value.entity, value.component));
        observed_revisions.dedup();

        let mut decisions = Vec::new();
        let mut candidates = Vec::new();
        for source in &sources {
            let definition = catalog
                .source(&source.definition)
                .expect("source collection admits catalog definitions");
            let mut matched = false;
            for (index, contribution) in definition.stat_contributions.iter().enumerate() {
                if &contribution.stat != stat {
                    continue;
                }
                matched = true;
                let decision_index = decisions.len();
                decisions.push(StatDecision {
                    source: source.identity.clone(),
                    source_definition: source.definition.clone(),
                    contribution_index: Some(index as u16),
                    outcome: DecisionOutcome::Suppressed,
                    amount: Some(contribution.amount),
                });
                candidates.push(StatCandidate {
                    decision_index,
                    source_definition: source.definition.clone(),
                    group: contribution.stacking_group.clone(),
                    stacking: contribution.stacking,
                    amount: contribution.amount,
                });
            }
            if !matched {
                decisions.push(StatDecision {
                    source: source.identity.clone(),
                    source_definition: source.definition.clone(),
                    contribution_index: None,
                    outcome: DecisionOutcome::Inapplicable,
                    amount: None,
                });
            }
        }
        if decisions.len() > MAX_STAT_DECISIONS {
            return Err(MechanicsError::ReceiptQuotaExceeded {
                actual: decisions.len(),
                maximum: MAX_STAT_DECISIONS,
            });
        }

        select_stat_candidates(&candidates, &mut decisions);
        let mut unconstrained = base;
        for candidate in &candidates {
            if decisions[candidate.decision_index].outcome == DecisionOutcome::Applied {
                unconstrained = unconstrained.checked_add(candidate.amount)?;
            }
        }
        let value = unconstrained.clamp(stat_definition.minimum, stat_definition.maximum);

        Ok(StatEvaluation {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            entity,
            stat: stat.clone(),
            base,
            unconstrained,
            value,
            decisions,
            observed_revisions,
            source_cost,
        })
    }
}

struct StatCandidate {
    decision_index: usize,
    source_definition: SourceDefinitionId,
    group: StackingGroupId,
    stacking: StackingPolicy,
    amount: MechanicsScalar,
}

fn select_stat_candidates(candidates: &[StatCandidate], decisions: &mut [StatDecision]) {
    let mut groups: BTreeMap<&StackingGroupId, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        groups.entry(&candidate.group).or_default().push(index);
    }
    for indexes in groups.values() {
        let policy = candidates[indexes[0]].stacking;
        match policy {
            StackingPolicy::Sum => {
                for index in indexes {
                    decisions[candidates[*index].decision_index].outcome = DecisionOutcome::Applied;
                }
            }
            StackingPolicy::Highest | StackingPolicy::Lowest => {
                let selected = indexes
                    .iter()
                    .copied()
                    .reduce(|selected, candidate| {
                        let ordering = candidates[candidate]
                            .amount
                            .cmp(&candidates[selected].amount);
                        let replace = match policy {
                            StackingPolicy::Highest => ordering.is_gt(),
                            StackingPolicy::Lowest => ordering.is_lt(),
                            StackingPolicy::Sum | StackingPolicy::UniqueBySource => unreachable!(),
                        };
                        if replace {
                            candidate
                        } else {
                            selected
                        }
                    })
                    .expect("stacking groups are nonempty");
                decisions[candidates[selected].decision_index].outcome = DecisionOutcome::Applied;
            }
            StackingPolicy::UniqueBySource => {
                let mut retained = BTreeSet::new();
                for index in indexes {
                    if retained.insert(&candidates[*index].source_definition) {
                        decisions[candidates[*index].decision_index].outcome =
                            DecisionOutcome::Applied;
                    }
                }
            }
        }
    }
}

pub(crate) fn track_bounds(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
    track: &TrackId,
    operation: &OperationId,
) -> Result<
    (
        MechanicsScalar,
        MechanicsScalar,
        Vec<ObservedComponentRevision>,
        SourceCollectionCost,
    ),
    MechanicsError,
> {
    let definition: &TrackDefinition =
        catalog
            .track(track)
            .ok_or_else(|| MechanicsError::UnknownTrack {
                track: track.clone(),
            })?;
    match &definition.maximum {
        TrackMaximum::Fixed { value } => {
            ensure_resolved_bounds(entity, track, definition.minimum, *value)?;
            Ok((
                definition.minimum,
                *value,
                Vec::new(),
                SourceCollectionCost::default(),
            ))
        }
        TrackMaximum::Stat { stat } => {
            let evaluated = StatService::evaluate(state, catalog, entity, stat, operation, &[])?;
            ensure_resolved_bounds(entity, track, definition.minimum, evaluated.value)?;
            Ok((
                definition.minimum,
                evaluated.value,
                evaluated.observed_revisions,
                evaluated.source_cost,
            ))
        }
    }
}

fn ensure_resolved_bounds(
    entity: EntityId,
    track: &TrackId,
    minimum: MechanicsScalar,
    maximum: MechanicsScalar,
) -> Result<(), MechanicsError> {
    if minimum > maximum {
        return Err(MechanicsError::InvalidResolvedTrackBounds {
            entity,
            track: track.clone(),
            minimum: minimum.get(),
            maximum: maximum.get(),
        });
    }
    Ok(())
}
