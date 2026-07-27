use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use entity_state::{ComponentRevision, EntityAuthoringService, EntityState};

use crate::source::{collect_active_sources_with_effects_override, ensure_receipt_capacity};
use crate::{
    ActiveEffectsComponent, CombinedRatio, DecisionOutcome, MechanicsCatalog,
    MechanicsComponentKind, MechanicsError, MechanicsScalar, ObservedComponentRevision,
    OperationId, RequestSource, RoundingPolicy, SourceCollectionCost, SourceDefinitionId,
    SourceInstanceIdentity, StackingGroupId, StackingPolicy, StatContribution, StatId,
    StatsComponent, TrackDefinition, TrackId, TrackMaximum, TracksComponent, MAX_REQUEST_SOURCES,
};

pub const MAX_STAT_DECISIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatDecision {
    pub source: SourceInstanceIdentity,
    pub source_definition: SourceDefinitionId,
    pub contribution_index: Option<u16>,
    pub outcome: DecisionOutcome,
    pub stacking_group: Option<StackingGroupId>,
    pub stacking: Option<StackingPolicy>,
    pub contribution: Option<StatContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatEvaluation {
    pub catalog_version: crate::CatalogVersion,
    pub catalog_fingerprint: String,
    pub entity: EntityId,
    pub stat: StatId,
    pub base: MechanicsScalar,
    pub after_additions: MechanicsScalar,
    pub combined_scale_numerator: u128,
    pub combined_scale_denominator: u128,
    pub after_scaling: MechanicsScalar,
    pub unconstrained: MechanicsScalar,
    pub minimum: MechanicsScalar,
    pub maximum: MechanicsScalar,
    pub value: MechanicsScalar,
    pub decisions: Vec<StatDecision>,
    pub observed_revisions: Vec<ObservedComponentRevision>,
    pub source_cost: SourceCollectionCost,
}

#[derive(Debug, Clone)]
pub struct StatBaseMutationRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub entity: EntityId,
    pub stat: StatId,
    pub base: MechanicsScalar,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatBaseMutationReceipt {
    pub catalog_version: crate::CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub entity: EntityId,
    pub stat: StatId,
    pub before: MechanicsScalar,
    pub after: MechanicsScalar,
    pub minimum: MechanicsScalar,
    pub maximum: MechanicsScalar,
    pub observed_stats_revision: u64,
    pub committed_stats_revision: u64,
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
        evaluate_with_stats_override(
            state,
            catalog,
            entity,
            stat,
            operation,
            request_sources,
            EvaluationOverrides::default(),
        )
    }

    pub fn set_base(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: StatBaseMutationRequest,
    ) -> Result<StatBaseMutationReceipt, MechanicsError> {
        let actual_revision = state.component_revision::<StatsComponent>(request.entity)?;
        if let Some(expected) = &request.expected_revision {
            crate::track::ensure_revision(expected, &actual_revision)?;
        }
        let publish_revision = request
            .expected_revision
            .clone()
            .unwrap_or_else(|| actual_revision.clone());
        let component = state.component::<StatsComponent>(request.entity)?.ok_or(
            MechanicsError::MissingComponent {
                entity: request.entity,
                component: StatsComponent::LABEL,
            },
        )?;
        crate::source::ensure_catalog_version(
            catalog,
            request.entity,
            StatsComponent::LABEL,
            component.catalog_version(),
        )?;
        let definition =
            catalog
                .stat(&request.stat)
                .ok_or_else(|| MechanicsError::UnknownStat {
                    stat: request.stat.clone(),
                })?;
        ensure_base_in_bounds(request.entity, &request.stat, request.base, definition)?;
        let before = component
            .base(&request.stat)
            .ok_or_else(|| MechanicsError::MissingStat {
                entity: request.entity,
                stat: request.stat.clone(),
            })?;
        let mut candidate = component.clone();
        assert!(candidate.set_base(&request.stat, request.base));

        let (mut observed_revisions, source_cost) = validate_tracks_with_stats_override(
            state,
            catalog,
            request.entity,
            &request.operation,
            &candidate,
        )?;
        EntityAuthoringService.replace_component(
            state,
            publish_revision,
            request.entity,
            candidate,
        )?;
        let committed_revision = state.component_revision::<StatsComponent>(request.entity)?;
        observed_revisions.push(ObservedComponentRevision {
            entity: request.entity,
            component: MechanicsComponentKind::Stats,
            revision: actual_revision.revision(),
        });
        canonicalize_observed_revisions(&mut observed_revisions);

        Ok(StatBaseMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            source: request.source,
            entity: request.entity,
            stat: request.stat,
            before,
            after: request.base,
            minimum: definition.minimum,
            maximum: definition.maximum,
            observed_stats_revision: actual_revision.revision(),
            committed_stats_revision: committed_revision.revision(),
            observed_revisions,
            source_cost,
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct EvaluationOverrides<'a> {
    stats: Option<&'a StatsComponent>,
    active_effects: Option<&'a ActiveEffectsComponent>,
}

fn evaluate_with_stats_override(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
    stat: &StatId,
    operation: &OperationId,
    request_sources: &[RequestSource],
    overrides: EvaluationOverrides<'_>,
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
    let stored_component =
        state
            .component::<StatsComponent>(entity)?
            .ok_or(MechanicsError::MissingComponent {
                entity,
                component: StatsComponent::LABEL,
            })?;
    let component = overrides.stats.unwrap_or(stored_component);
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
    ensure_base_in_bounds(entity, stat, base, stat_definition)?;

    let (sources, source_cost, mut observed_revisions) =
        collect_active_sources_with_effects_override(
            state,
            catalog,
            entity,
            operation,
            request_sources,
            MAX_STAT_DECISIONS,
            overrides.active_effects,
        )?;
    observed_revisions.push(ObservedComponentRevision {
        entity,
        component: MechanicsComponentKind::Stats,
        revision: state
            .component_revision::<StatsComponent>(entity)?
            .revision(),
    });
    canonicalize_observed_revisions(&mut observed_revisions);

    let mut decisions = Vec::new();
    let mut candidates = Vec::new();
    for source in &sources {
        let definition = catalog
            .source(&source.definition)
            .expect("source collection admits catalog definitions");
        let matching_contributions = definition
            .stat_contributions
            .iter()
            .filter(|contribution| &contribution.stat == stat)
            .count();
        ensure_receipt_capacity(
            decisions.len(),
            matching_contributions.max(1),
            MAX_STAT_DECISIONS,
        )?;
        let matched = matching_contributions != 0;
        for (index, contribution) in definition.stat_contributions.iter().enumerate() {
            if &contribution.stat != stat {
                continue;
            }
            let decision_index = decisions.len();
            decisions.push(StatDecision {
                source: source.identity.clone(),
                source_definition: source.definition.clone(),
                contribution_index: Some(index as u16),
                outcome: DecisionOutcome::Suppressed,
                stacking_group: Some(contribution.stacking_group.clone()),
                stacking: Some(contribution.stacking),
                contribution: Some(contribution.contribution.clone()),
            });
            candidates.push(StatCandidate {
                decision_index,
                source_definition: source.definition.clone(),
                group: contribution.stacking_group.clone(),
                stacking: contribution.stacking,
                contribution: contribution.contribution.clone(),
            });
        }
        if !matched {
            decisions.push(StatDecision {
                source: source.identity.clone(),
                source_definition: source.definition.clone(),
                contribution_index: None,
                outcome: DecisionOutcome::Inapplicable,
                stacking_group: None,
                stacking: None,
                contribution: None,
            });
        }
    }

    select_stat_candidates(&candidates, &mut decisions);
    let mut additive_delta = 0_i128;
    let mut combined_scale = CombinedRatio::one();
    let mut minimum = stat_definition.minimum;
    let mut maximum = stat_definition.maximum;
    for candidate in &candidates {
        if decisions[candidate.decision_index].outcome != DecisionOutcome::Applied {
            continue;
        }
        match candidate.contribution {
            StatContribution::Add { amount } => {
                additive_delta = additive_delta
                    .checked_add(i128::from(amount.get()))
                    .ok_or(crate::MechanicsArithmeticError::Overflow)?;
            }
            StatContribution::Scale { ratio } => combined_scale.include(ratio)?,
            StatContribution::Minimum { value } => minimum = minimum.max(value),
            StatContribution::Maximum { value } => maximum = maximum.min(value),
        }
    }
    if minimum > maximum {
        return Err(MechanicsError::InvalidResolvedStatBounds {
            entity,
            stat: stat.clone(),
            minimum: minimum.get(),
            maximum: maximum.get(),
        });
    }
    let after_additions = i128::from(base.get())
        .checked_add(additive_delta)
        .ok_or(crate::MechanicsArithmeticError::Overflow)?;
    let after_additions =
        i64::try_from(after_additions).map_err(|_| crate::MechanicsArithmeticError::Overflow)?;
    let after_additions = MechanicsScalar::new(after_additions)?;
    let after_scaling = combined_scale.apply_signed(after_additions, RoundingPolicy::TowardZero)?;
    let value = after_scaling.clamp(minimum, maximum);

    Ok(StatEvaluation {
        catalog_version: catalog.version().clone(),
        catalog_fingerprint: catalog.fingerprint().to_string(),
        entity,
        stat: stat.clone(),
        base,
        after_additions,
        combined_scale_numerator: combined_scale.numerator(),
        combined_scale_denominator: combined_scale.denominator(),
        after_scaling,
        unconstrained: after_scaling,
        minimum,
        maximum,
        value,
        decisions,
        observed_revisions,
        source_cost,
    })
}

struct StatCandidate {
    decision_index: usize,
    source_definition: SourceDefinitionId,
    group: StackingGroupId,
    stacking: StackingPolicy,
    contribution: StatContribution,
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
                            .contribution
                            .cmp(&candidates[selected].contribution);
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
    track_bounds_with_overrides(state, catalog, entity, track, operation, None, None)
}

fn track_bounds_with_overrides(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
    track: &TrackId,
    operation: &OperationId,
    stats_override: Option<&StatsComponent>,
    active_effects_override: Option<&ActiveEffectsComponent>,
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
            ensure_resolved_track_bounds(entity, track, definition.minimum, *value)?;
            Ok((
                definition.minimum,
                *value,
                Vec::new(),
                SourceCollectionCost::default(),
            ))
        }
        TrackMaximum::Stat { stat } => {
            let evaluated = evaluate_with_stats_override(
                state,
                catalog,
                entity,
                stat,
                operation,
                &[],
                EvaluationOverrides {
                    stats: stats_override,
                    active_effects: active_effects_override,
                },
            )?;
            ensure_resolved_track_bounds(entity, track, definition.minimum, evaluated.value)?;
            Ok((
                definition.minimum,
                evaluated.value,
                evaluated.observed_revisions,
                evaluated.source_cost,
            ))
        }
    }
}

fn validate_tracks_with_stats_override(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
    operation: &OperationId,
    stats_override: &StatsComponent,
) -> Result<(Vec<ObservedComponentRevision>, SourceCollectionCost), MechanicsError> {
    let (_, revisions, cost) = validate_tracks_with_overrides(
        state,
        catalog,
        entity,
        operation,
        Some(stats_override),
        None,
    )?;
    Ok((revisions, cost))
}

pub(crate) fn validate_tracks_with_effects_override(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
    operation: &OperationId,
    active_effects_override: &ActiveEffectsComponent,
) -> Result<(usize, Vec<ObservedComponentRevision>, SourceCollectionCost), MechanicsError> {
    validate_tracks_with_overrides(
        state,
        catalog,
        entity,
        operation,
        None,
        Some(active_effects_override),
    )
}

fn validate_tracks_with_overrides(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    entity: EntityId,
    operation: &OperationId,
    stats_override: Option<&StatsComponent>,
    active_effects_override: Option<&ActiveEffectsComponent>,
) -> Result<(usize, Vec<ObservedComponentRevision>, SourceCollectionCost), MechanicsError> {
    let Some(tracks) = state.component::<TracksComponent>(entity)? else {
        return Ok((0, Vec::new(), SourceCollectionCost::default()));
    };
    crate::source::ensure_catalog_version(
        catalog,
        entity,
        TracksComponent::LABEL,
        tracks.catalog_version(),
    )?;
    let mut observed_revisions = vec![ObservedComponentRevision {
        entity,
        component: MechanicsComponentKind::Tracks,
        revision: state
            .component_revision::<TracksComponent>(entity)?
            .revision(),
    }];
    let mut source_cost = SourceCollectionCost::default();
    for value in tracks.values() {
        let (minimum, maximum, revisions, cost) = track_bounds_with_overrides(
            state,
            catalog,
            entity,
            value.track(),
            operation,
            stats_override,
            active_effects_override,
        )?;
        merge_source_cost(&mut source_cost, cost);
        observed_revisions.extend(revisions);
        if value.current() < minimum || value.current() > maximum {
            return Err(MechanicsError::TrackOutOfBounds {
                entity,
                track: value.track().clone(),
                attempted: value.current().get(),
                minimum: minimum.get(),
                maximum: maximum.get(),
            });
        }
    }
    canonicalize_observed_revisions(&mut observed_revisions);
    Ok((tracks.values().len(), observed_revisions, source_cost))
}

fn ensure_base_in_bounds(
    entity: EntityId,
    stat: &StatId,
    base: MechanicsScalar,
    definition: &crate::StatDefinition,
) -> Result<(), MechanicsError> {
    if base < definition.minimum || base > definition.maximum {
        return Err(MechanicsError::StatOutOfBounds {
            entity,
            stat: stat.clone(),
            attempted: base.get(),
            minimum: definition.minimum.get(),
            maximum: definition.maximum.get(),
        });
    }
    Ok(())
}

fn ensure_resolved_track_bounds(
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

fn merge_source_cost(target: &mut SourceCollectionCost, value: SourceCollectionCost) {
    target.intrinsic_entries_visited += value.intrinsic_entries_visited;
    target.effect_entries_visited += value.effect_entries_visited;
    target.effect_source_activations_visited += value.effect_source_activations_visited;
    target.equipment_entries_visited += value.equipment_entries_visited;
    target.item_components_read += value.item_components_read;
    target.request_entries_visited += value.request_entries_visited;
}

fn canonicalize_observed_revisions(revisions: &mut Vec<ObservedComponentRevision>) {
    revisions.sort_by_key(|value| (value.entity, value.component));
    revisions.dedup();
}
