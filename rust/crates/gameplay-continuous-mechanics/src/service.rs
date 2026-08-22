use std::collections::BTreeMap;

use core_ids::EntityId;
use entity_state::{
    ComponentAccessError, ComponentRevision, EntityAuthoringError, EntityAuthoringService,
    EntityState,
};
use gameplay_standard::{ContinuousValue, ContinuousValueError};

use crate::{
    ContinuousActiveEffectInstance, ContinuousActiveEffectsComponent, ContinuousCatalogVersion,
    ContinuousEffectDefinitionId, ContinuousEffectInstanceId, ContinuousMechanicsCatalog,
    ContinuousOperationId, ContinuousSourceDefinitionId, ContinuousSourceInstanceId,
    ContinuousStackingGroupId, ContinuousStackingPolicy, ContinuousStatContribution,
    ContinuousStatId, ContinuousStatsComponent, ContinuousTrackId, ContinuousTrackMaximum,
    ContinuousTracksComponent,
};

/// Runtime evaluation limits, separate from catalog admission because active effect instances
/// multiply otherwise-bounded definition lists.
pub const MAX_CONTINUOUS_SOURCE_ACTIVATIONS_PER_EVALUATION: usize = 2_048;
pub const MAX_CONTINUOUS_DECISIONS_PER_EVALUATION: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContinuousSourceIdentity {
    Intrinsic(ContinuousSourceInstanceId),
    Effect {
        effect: ContinuousEffectInstanceId,
        source: ContinuousSourceDefinitionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousDecisionOutcome {
    Applied,
    Suppressed,
    Inapplicable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousStatDecision {
    pub source: ContinuousSourceIdentity,
    pub source_definition: ContinuousSourceDefinitionId,
    pub contribution_index: u16,
    pub outcome: ContinuousDecisionOutcome,
    pub contribution: ContinuousStatContribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousStatEvaluation {
    pub catalog_version: ContinuousCatalogVersion,
    pub catalog_fingerprint: String,
    pub entity: EntityId,
    pub stat: ContinuousStatId,
    pub base: ContinuousValue,
    pub after_additions: ContinuousValue,
    pub unconstrained: ContinuousValue,
    pub minimum: ContinuousValue,
    pub maximum: ContinuousValue,
    pub value: ContinuousValue,
    pub decisions: Vec<ContinuousStatDecision>,
    pub observed_stats_revision: u64,
    pub observed_sources_revision: Option<u64>,
    pub observed_effects_revision: Option<u64>,
}

#[derive(Clone)]
struct Candidate {
    source: ContinuousSourceIdentity,
    definition: ContinuousSourceDefinitionId,
    index: u16,
    stat: ContinuousStatId,
    group: ContinuousStackingGroupId,
    stacking: ContinuousStackingPolicy,
    contribution: ContinuousStatContribution,
}

impl Candidate {
    fn kind(&self) -> &'static str {
        match self.contribution {
            ContinuousStatContribution::Add { .. } => "add",
            ContinuousStatContribution::Minimum { .. } => "minimum",
            ContinuousStatContribution::Maximum { .. } => "maximum",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContinuousStatBaseMutationRequest {
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub stat: ContinuousStatId,
    pub base: ContinuousValue,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousStatBaseMutationReceipt {
    pub catalog_version: ContinuousCatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub stat: ContinuousStatId,
    pub before: ContinuousValue,
    pub after: ContinuousValue,
    pub minimum: ContinuousValue,
    pub maximum: ContinuousValue,
    pub observed_revision: u64,
    pub committed_revision: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ContinuousStatService;

impl ContinuousStatService {
    pub fn evaluate(
        state: &EntityState,
        catalog: &ContinuousMechanicsCatalog,
        entity: EntityId,
        stat: &ContinuousStatId,
    ) -> Result<ContinuousStatEvaluation, ContinuousMechanicsError> {
        evaluate_with_effects(state, catalog, entity, stat, None)
    }
}

fn evaluate_with_effects(
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
    entity: EntityId,
    stat: &ContinuousStatId,
    effects_override: Option<&ContinuousActiveEffectsComponent>,
) -> Result<ContinuousStatEvaluation, ContinuousMechanicsError> {
    let stats_revision = state.component_revision::<ContinuousStatsComponent>(entity)?;
    let stats = state.component::<ContinuousStatsComponent>(entity)?.ok_or(
        ContinuousMechanicsError::MissingComponent {
            entity,
            component: ContinuousStatsComponent::LABEL,
        },
    )?;
    ensure_version(
        catalog,
        entity,
        ContinuousStatsComponent::LABEL,
        stats.catalog_version(),
    )?;
    let definition = catalog
        .stat(stat)
        .ok_or_else(|| ContinuousMechanicsError::UnknownStat(stat.clone()))?;
    let base = stats
        .base(stat)
        .ok_or_else(|| ContinuousMechanicsError::MissingStat {
            entity,
            stat: stat.clone(),
        })?;
    ensure_value_in_bounds(
        stat.to_string(),
        base,
        definition.minimum(),
        definition.maximum(),
    )?;

    let intrinsic = state.component::<crate::ContinuousIntrinsicSourcesComponent>(entity)?;
    let stored_effects = state.component::<ContinuousActiveEffectsComponent>(entity)?;
    let effects = effects_override.or(stored_effects);
    if let Some(value) = intrinsic {
        ensure_version(
            catalog,
            entity,
            crate::ContinuousIntrinsicSourcesComponent::LABEL,
            value.catalog_version(),
        )?;
    }
    if let Some(value) = effects {
        ensure_version(
            catalog,
            entity,
            ContinuousActiveEffectsComponent::LABEL,
            value.catalog_version(),
        )?;
    }
    let sources_revision = if intrinsic.is_some() {
        Some(
            state
                .component_revision::<crate::ContinuousIntrinsicSourcesComponent>(entity)?
                .revision(),
        )
    } else {
        None
    };
    let effects_revision = if stored_effects.is_some() {
        Some(
            state
                .component_revision::<ContinuousActiveEffectsComponent>(entity)?
                .revision(),
        )
    } else {
        None
    };

    let mut activations = Vec::new();
    if let Some(component) = intrinsic {
        for binding in component.bindings() {
            ensure_evaluation_quota(
                "activations",
                activations.len(),
                1,
                MAX_CONTINUOUS_SOURCE_ACTIVATIONS_PER_EVALUATION,
            )?;
            activations.push((
                ContinuousSourceIdentity::Intrinsic(binding.instance().clone()),
                binding.definition().clone(),
            ));
        }
    }
    if let Some(component) = effects {
        for effect in component.effects() {
            let definition = catalog.effect(effect.definition()).ok_or_else(|| {
                ContinuousMechanicsError::UnknownEffect(effect.definition().clone())
            })?;
            for source in &definition.sources {
                ensure_evaluation_quota(
                    "activations",
                    activations.len(),
                    1,
                    MAX_CONTINUOUS_SOURCE_ACTIVATIONS_PER_EVALUATION,
                )?;
                activations.push((
                    ContinuousSourceIdentity::Effect {
                        effect: effect.instance().clone(),
                        source: source.clone(),
                    },
                    source.clone(),
                ));
            }
        }
    }
    for (_, source) in &activations {
        if catalog.source(source).is_none() {
            return Err(ContinuousMechanicsError::UnknownSource(source.clone()));
        }
    }
    activations.sort_by_key(|activation| {
        (
            catalog
                .source(&activation.1)
                .map_or(i16::MAX, |definition| definition.priority),
            activation.0.clone(),
        )
    });

    let mut candidates = Vec::new();
    for (identity, source_id) in activations {
        let source = catalog
            .source(&source_id)
            .ok_or_else(|| ContinuousMechanicsError::UnknownSource(source_id.clone()))?;
        ensure_evaluation_quota(
            "decisions",
            candidates.len(),
            source.stat_contributions.len(),
            MAX_CONTINUOUS_DECISIONS_PER_EVALUATION,
        )?;
        for (index, contribution) in source.stat_contributions.iter().enumerate() {
            candidates.push(Candidate {
                source: identity.clone(),
                definition: source_id.clone(),
                index: index as u16,
                stat: contribution.stat.clone(),
                group: contribution.stacking_group.clone(),
                stacking: contribution.stacking,
                contribution: contribution.contribution.clone(),
            });
        }
    }
    let mut groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if &candidate.stat != stat {
            continue;
        }
        groups
            .entry((candidate.kind().to_string(), candidate.group.to_string()))
            .or_default()
            .push(index);
    }
    let mut selected = vec![false; candidates.len()];
    for indices in groups.into_values() {
        match candidates[indices[0]].stacking {
            ContinuousStackingPolicy::Sum => {
                for index in indices {
                    selected[index] = true;
                }
            }
            ContinuousStackingPolicy::UniqueBySource => {
                // Exact mechanics selects one contribution per source definition within a
                // shared stacking group; instances of the same definition do not multiply.
                let mut first_by_definition = BTreeMap::new();
                for index in indices {
                    first_by_definition
                        .entry(candidates[index].definition.clone())
                        .or_insert(index);
                }
                for index in first_by_definition.into_values() {
                    selected[index] = true;
                }
            }
            policy @ (ContinuousStackingPolicy::Highest | ContinuousStackingPolicy::Lowest) => {
                let mut chosen = indices[0];
                for index in indices.into_iter().skip(1) {
                    let order = candidates[index]
                        .contribution
                        .value()
                        .cmp(&candidates[chosen].contribution.value());
                    if (policy == ContinuousStackingPolicy::Highest && order.is_gt())
                        || (policy == ContinuousStackingPolicy::Lowest && order.is_lt())
                    {
                        chosen = index;
                    }
                }
                selected[chosen] = true;
            }
        }
    }
    let mut after_additions = base;
    let mut lower = None;
    let mut upper = None;
    let mut decisions = Vec::new();
    for (index, candidate) in candidates.into_iter().enumerate() {
        let applicable = &candidate.stat == stat;
        let applied = selected[index] && applicable;
        if applied {
            match candidate.contribution {
                ContinuousStatContribution::Add { .. } => {
                    after_additions = after_additions.checked_add(candidate.contribution.value())?
                }
                ContinuousStatContribution::Minimum { .. } => {
                    lower = Some(
                        lower.map_or(candidate.contribution.value(), |value: ContinuousValue| {
                            value.max(candidate.contribution.value())
                        }),
                    )
                }
                ContinuousStatContribution::Maximum { .. } => {
                    upper = Some(
                        upper.map_or(candidate.contribution.value(), |value: ContinuousValue| {
                            value.min(candidate.contribution.value())
                        }),
                    )
                }
            }
        }
        decisions.push(ContinuousStatDecision {
            source: candidate.source,
            source_definition: candidate.definition,
            contribution_index: candidate.index,
            outcome: if !applicable {
                ContinuousDecisionOutcome::Inapplicable
            } else if applied {
                ContinuousDecisionOutcome::Applied
            } else {
                ContinuousDecisionOutcome::Suppressed
            },
            contribution: candidate.contribution,
        });
    }
    let minimum = lower.map_or(definition.minimum(), |value| {
        value.max(definition.minimum())
    });
    let maximum = upper.map_or(definition.maximum(), |value| {
        value.min(definition.maximum())
    });
    if minimum > maximum {
        return Err(ContinuousMechanicsError::InvertedBounds {
            stat: stat.clone(),
            minimum: minimum.bits(),
            maximum: maximum.bits(),
        });
    }
    Ok(ContinuousStatEvaluation {
        catalog_version: catalog.version().clone(),
        catalog_fingerprint: catalog.fingerprint().to_string(),
        entity,
        stat: stat.clone(),
        base,
        after_additions,
        unconstrained: after_additions,
        minimum,
        maximum,
        value: after_additions.clamp(minimum, maximum),
        decisions,
        observed_stats_revision: stats_revision.revision(),
        observed_sources_revision: sources_revision,
        observed_effects_revision: effects_revision,
    })
}

impl ContinuousStatService {
    pub fn set_base(
        state: &mut EntityState,
        catalog: &ContinuousMechanicsCatalog,
        request: ContinuousStatBaseMutationRequest,
    ) -> Result<ContinuousStatBaseMutationReceipt, ContinuousMechanicsError> {
        let actual = state.component_revision::<ContinuousStatsComponent>(request.entity)?;
        check_revision(request.expected_revision.as_ref(), &actual)?;
        let component = state
            .component::<ContinuousStatsComponent>(request.entity)?
            .ok_or(ContinuousMechanicsError::MissingComponent {
                entity: request.entity,
                component: ContinuousStatsComponent::LABEL,
            })?;
        ensure_version(
            catalog,
            request.entity,
            ContinuousStatsComponent::LABEL,
            component.catalog_version(),
        )?;
        let definition = catalog
            .stat(&request.stat)
            .ok_or_else(|| ContinuousMechanicsError::UnknownStat(request.stat.clone()))?;
        ensure_value_in_bounds(
            request.stat.to_string(),
            request.base,
            definition.minimum(),
            definition.maximum(),
        )?;
        let before =
            component
                .base(&request.stat)
                .ok_or_else(|| ContinuousMechanicsError::MissingStat {
                    entity: request.entity,
                    stat: request.stat.clone(),
                })?;
        validate_dependent_tracks_for_base(
            state,
            catalog,
            request.entity,
            &request.stat,
            before,
            request.base,
        )?;
        let mut candidate = component.clone();
        assert!(candidate.set_base(&request.stat, request.base));
        EntityAuthoringService.replace_component(
            state,
            request.expected_revision.unwrap_or(actual.clone()),
            request.entity,
            candidate,
        )?;
        let committed = state.component_revision::<ContinuousStatsComponent>(request.entity)?;
        Ok(ContinuousStatBaseMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            entity: request.entity,
            stat: request.stat,
            before,
            after: request.base,
            minimum: definition.minimum(),
            maximum: definition.maximum(),
            observed_revision: actual.revision(),
            committed_revision: committed.revision(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuousTrackSetPolicy {
    RejectOutOfBounds,
    ClampToBounds,
}
#[derive(Debug, Clone)]
pub struct ContinuousTrackSetRequest {
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub track: ContinuousTrackId,
    pub value: ContinuousValue,
    pub policy: ContinuousTrackSetPolicy,
    pub expected_revision: Option<ComponentRevision>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousTrackMutationReceipt {
    pub catalog_version: ContinuousCatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub track: ContinuousTrackId,
    pub before: ContinuousValue,
    pub after: ContinuousValue,
    pub minimum: ContinuousValue,
    pub maximum: ContinuousValue,
    pub observed_revision: u64,
    pub observed_stats_revision: Option<u64>,
    pub observed_sources_revision: Option<u64>,
    pub observed_effects_revision: Option<u64>,
    pub committed_revision: u64,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct ContinuousTrackService;
impl ContinuousTrackService {
    pub fn set(
        state: &mut EntityState,
        catalog: &ContinuousMechanicsCatalog,
        request: ContinuousTrackSetRequest,
    ) -> Result<ContinuousTrackMutationReceipt, ContinuousMechanicsError> {
        let actual = state.component_revision::<ContinuousTracksComponent>(request.entity)?;
        check_revision(request.expected_revision.as_ref(), &actual)?;
        let component = state
            .component::<ContinuousTracksComponent>(request.entity)?
            .ok_or(ContinuousMechanicsError::MissingComponent {
                entity: request.entity,
                component: ContinuousTracksComponent::LABEL,
            })?;
        ensure_version(
            catalog,
            request.entity,
            ContinuousTracksComponent::LABEL,
            component.catalog_version(),
        )?;
        let before = component.current(&request.track).ok_or_else(|| {
            ContinuousMechanicsError::MissingTrack {
                entity: request.entity,
                track: request.track.clone(),
            }
        })?;
        let definition = catalog
            .track(&request.track)
            .ok_or_else(|| ContinuousMechanicsError::UnknownTrack(request.track.clone()))?;
        let (observed_stats_revision, observed_sources_revision, observed_effects_revision) =
            match &definition.maximum {
                ContinuousTrackMaximum::Stat { .. } => {
                    let sources_present = state
                        .component::<crate::ContinuousIntrinsicSourcesComponent>(request.entity)?
                        .is_some();
                    let effects_present = state
                        .component::<ContinuousActiveEffectsComponent>(request.entity)?
                        .is_some();
                    (
                        Some(
                            state
                                .component_revision::<ContinuousStatsComponent>(request.entity)?
                                .revision(),
                        ),
                        if sources_present {
                            Some(
                                state
                                    .component_revision::<crate::ContinuousIntrinsicSourcesComponent>(
                                        request.entity,
                                    )?
                                    .revision(),
                            )
                        } else {
                            None
                        },
                        if effects_present {
                            Some(
                                state
                                    .component_revision::<ContinuousActiveEffectsComponent>(
                                        request.entity,
                                    )?
                                    .revision(),
                            )
                        } else {
                            None
                        },
                    )
                }
                ContinuousTrackMaximum::Fixed { .. } => (None, None, None),
            };
        let (minimum, maximum) =
            continuous_track_bounds(state, catalog, request.entity, &request.track)?;
        ensure_value_in_bounds(request.track.to_string(), before, minimum, maximum)?;
        let after = match request.policy {
            ContinuousTrackSetPolicy::RejectOutOfBounds => {
                ensure_value_in_bounds(request.track.to_string(), request.value, minimum, maximum)?;
                request.value
            }
            ContinuousTrackSetPolicy::ClampToBounds => request.value.clamp(minimum, maximum),
        };
        let mut candidate = component.clone();
        assert!(candidate.set_current(&request.track, after));
        EntityAuthoringService.replace_component(
            state,
            request.expected_revision.unwrap_or(actual.clone()),
            request.entity,
            candidate,
        )?;
        let committed = state.component_revision::<ContinuousTracksComponent>(request.entity)?;
        Ok(ContinuousTrackMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            entity: request.entity,
            track: request.track,
            before,
            after,
            minimum,
            maximum,
            observed_revision: actual.revision(),
            observed_stats_revision,
            observed_sources_revision,
            observed_effects_revision,
            committed_revision: committed.revision(),
        })
    }
}
pub fn continuous_track_bounds(
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
    entity: EntityId,
    track: &ContinuousTrackId,
) -> Result<(ContinuousValue, ContinuousValue), ContinuousMechanicsError> {
    let definition = catalog
        .track(track)
        .ok_or_else(|| ContinuousMechanicsError::UnknownTrack(track.clone()))?;
    let maximum = match &definition.maximum {
        ContinuousTrackMaximum::Fixed { value } => *value,
        ContinuousTrackMaximum::Stat { stat } => {
            ContinuousStatService::evaluate(state, catalog, entity, stat)?.value
        }
    };
    if definition.minimum() > maximum {
        return Err(ContinuousMechanicsError::InvertedTrackBounds {
            track: track.clone(),
            minimum: definition.minimum().bits(),
            maximum: maximum.bits(),
        });
    }
    Ok((definition.minimum(), maximum))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuousTrackAdjustmentKind {
    Spend,
    Restore,
}
#[derive(Debug, Clone)]
pub struct ContinuousTrackAdjustmentRequest {
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub track: ContinuousTrackId,
    pub amount: ContinuousValue,
    pub kind: ContinuousTrackAdjustmentKind,
    pub expected_revision: Option<ComponentRevision>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousTrackAdjustmentReceipt {
    pub catalog_version: ContinuousCatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub track: ContinuousTrackId,
    pub kind: ContinuousTrackAdjustmentKind,
    pub requested_amount: ContinuousValue,
    pub applied_amount: ContinuousValue,
    pub before: ContinuousValue,
    pub after: ContinuousValue,
    pub minimum: ContinuousValue,
    pub maximum: ContinuousValue,
    pub observed_tracks_revision: u64,
    pub observed_stats_revision: Option<u64>,
    pub observed_sources_revision: Option<u64>,
    pub observed_effects_revision: Option<u64>,
    pub committed_tracks_revision: u64,
}
impl ContinuousTrackAdjustmentReceipt {
    /// Compare the durable continuation result while retaining ordinary `Eq` for the complete
    /// operation receipt, including its slot revision evidence.
    pub fn same_durable_result(&self, other: &Self) -> bool {
        self.catalog_version == other.catalog_version
            && self.catalog_fingerprint == other.catalog_fingerprint
            && self.operation == other.operation
            && self.entity == other.entity
            && self.track == other.track
            && self.kind == other.kind
            && self.requested_amount == other.requested_amount
            && self.applied_amount == other.applied_amount
            && self.before == other.before
            && self.after == other.after
            && self.minimum == other.minimum
            && self.maximum == other.maximum
    }
}
impl ContinuousTrackService {
    pub fn adjust(
        state: &mut EntityState,
        catalog: &ContinuousMechanicsCatalog,
        request: ContinuousTrackAdjustmentRequest,
    ) -> Result<ContinuousTrackAdjustmentReceipt, ContinuousMechanicsError> {
        if request.amount < ContinuousValue::new(0.0).expect("zero is finite") {
            return Err(ContinuousMechanicsError::NegativeAdjustment {
                bits: request.amount.bits(),
            });
        }
        let actual = state.component_revision::<ContinuousTracksComponent>(request.entity)?;
        check_revision(request.expected_revision.as_ref(), &actual)?;
        let component = state
            .component::<ContinuousTracksComponent>(request.entity)?
            .ok_or(ContinuousMechanicsError::MissingComponent {
                entity: request.entity,
                component: ContinuousTracksComponent::LABEL,
            })?;
        ensure_version(
            catalog,
            request.entity,
            ContinuousTracksComponent::LABEL,
            component.catalog_version(),
        )?;
        let before = component.current(&request.track).ok_or_else(|| {
            ContinuousMechanicsError::MissingTrack {
                entity: request.entity,
                track: request.track.clone(),
            }
        })?;
        let definition = catalog
            .track(&request.track)
            .ok_or_else(|| ContinuousMechanicsError::UnknownTrack(request.track.clone()))?;
        let stats_revision = match &definition.maximum {
            ContinuousTrackMaximum::Stat { .. } => Some(
                state
                    .component_revision::<ContinuousStatsComponent>(request.entity)?
                    .revision(),
            ),
            ContinuousTrackMaximum::Fixed { .. } => None,
        };
        let sources_revision = match &definition.maximum {
            ContinuousTrackMaximum::Stat { .. }
                if state
                    .component::<crate::ContinuousIntrinsicSourcesComponent>(request.entity)?
                    .is_some() =>
            {
                Some(
                    state
                        .component_revision::<crate::ContinuousIntrinsicSourcesComponent>(
                            request.entity,
                        )?
                        .revision(),
                )
            }
            _ => None,
        };
        let effects_revision = match &definition.maximum {
            ContinuousTrackMaximum::Stat { .. }
                if state
                    .component::<ContinuousActiveEffectsComponent>(request.entity)?
                    .is_some() =>
            {
                Some(
                    state
                        .component_revision::<ContinuousActiveEffectsComponent>(request.entity)?
                        .revision(),
                )
            }
            _ => None,
        };
        let (minimum, maximum) =
            continuous_track_bounds(state, catalog, request.entity, &request.track)?;
        ensure_value_in_bounds(request.track.to_string(), before, minimum, maximum)?;
        let attempted = match request.kind {
            ContinuousTrackAdjustmentKind::Spend => before.checked_sub(request.amount)?,
            // Cap before addition so a finite request near `f64::MAX` cannot overflow.
            ContinuousTrackAdjustmentKind::Restore => {
                before.checked_add(request.amount.min(maximum.checked_sub(before)?))?
            }
        };
        let after = match request.kind {
            ContinuousTrackAdjustmentKind::Spend if attempted < minimum => {
                return Err(ContinuousMechanicsError::InsufficientTrack {
                    track: request.track,
                    current: before.bits(),
                    requested: request.amount.bits(),
                    minimum: minimum.bits(),
                })
            }
            ContinuousTrackAdjustmentKind::Restore if attempted > maximum => maximum,
            _ => attempted,
        };
        let applied_amount = match request.kind {
            ContinuousTrackAdjustmentKind::Spend => before.checked_sub(after)?,
            ContinuousTrackAdjustmentKind::Restore => after.checked_sub(before)?,
        };
        let mut candidate = component.clone();
        assert!(candidate.set_current(&request.track, after));
        EntityAuthoringService.replace_component(
            state,
            request.expected_revision.unwrap_or(actual.clone()),
            request.entity,
            candidate,
        )?;
        let committed = state.component_revision::<ContinuousTracksComponent>(request.entity)?;
        Ok(ContinuousTrackAdjustmentReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            entity: request.entity,
            track: request.track,
            kind: request.kind,
            requested_amount: request.amount,
            applied_amount,
            before,
            after,
            minimum,
            maximum,
            observed_tracks_revision: actual.revision(),
            observed_stats_revision: stats_revision,
            observed_sources_revision: sources_revision,
            observed_effects_revision: effects_revision,
            committed_tracks_revision: committed.revision(),
        })
    }
    pub fn spend(
        state: &mut EntityState,
        catalog: &ContinuousMechanicsCatalog,
        mut request: ContinuousTrackAdjustmentRequest,
    ) -> Result<ContinuousTrackAdjustmentReceipt, ContinuousMechanicsError> {
        request.kind = ContinuousTrackAdjustmentKind::Spend;
        Self::adjust(state, catalog, request)
    }
    pub fn restore(
        state: &mut EntityState,
        catalog: &ContinuousMechanicsCatalog,
        mut request: ContinuousTrackAdjustmentRequest,
    ) -> Result<ContinuousTrackAdjustmentReceipt, ContinuousMechanicsError> {
        request.kind = ContinuousTrackAdjustmentKind::Restore;
        Self::adjust(state, catalog, request)
    }
}

#[derive(Debug, Clone)]
pub struct ContinuousEffectApplyRequest {
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub effect: ContinuousActiveEffectInstance,
    pub expected_revision: Option<ComponentRevision>,
}
#[derive(Debug, Clone)]
pub struct ContinuousEffectRemoveRequest {
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub instance: ContinuousEffectInstanceId,
    pub expected_revision: Option<ComponentRevision>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousEffectMutationReceipt {
    pub catalog_version: ContinuousCatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: ContinuousOperationId,
    pub entity: EntityId,
    pub instance: ContinuousEffectInstanceId,
    pub removed: bool,
    pub observed_revision: u64,
    pub committed_revision: u64,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct ContinuousEffectService;
impl ContinuousEffectService {
    pub fn apply(
        state: &mut EntityState,
        catalog: &ContinuousMechanicsCatalog,
        request: ContinuousEffectApplyRequest,
    ) -> Result<ContinuousEffectMutationReceipt, ContinuousMechanicsError> {
        let actual =
            state.component_revision::<ContinuousActiveEffectsComponent>(request.entity)?;
        check_revision(request.expected_revision.as_ref(), &actual)?;
        let component = state
            .component::<ContinuousActiveEffectsComponent>(request.entity)?
            .ok_or(ContinuousMechanicsError::MissingComponent {
                entity: request.entity,
                component: ContinuousActiveEffectsComponent::LABEL,
            })?;
        ensure_version(
            catalog,
            request.entity,
            ContinuousActiveEffectsComponent::LABEL,
            component.catalog_version(),
        )?;
        if catalog.effect(request.effect.definition()).is_none() {
            return Err(ContinuousMechanicsError::UnknownEffect(
                request.effect.definition().clone(),
            ));
        }
        let mut candidate = component.clone();
        candidate.insert(request.effect.clone())?;
        validate_dependent_tracks_for_effects(state, catalog, request.entity, &candidate)?;
        EntityAuthoringService.replace_component(
            state,
            request.expected_revision.unwrap_or(actual.clone()),
            request.entity,
            candidate,
        )?;
        let committed =
            state.component_revision::<ContinuousActiveEffectsComponent>(request.entity)?;
        Ok(ContinuousEffectMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            entity: request.entity,
            instance: request.effect.instance().clone(),
            removed: false,
            observed_revision: actual.revision(),
            committed_revision: committed.revision(),
        })
    }
    pub fn remove(
        state: &mut EntityState,
        catalog: &ContinuousMechanicsCatalog,
        request: ContinuousEffectRemoveRequest,
    ) -> Result<ContinuousEffectMutationReceipt, ContinuousMechanicsError> {
        let actual =
            state.component_revision::<ContinuousActiveEffectsComponent>(request.entity)?;
        check_revision(request.expected_revision.as_ref(), &actual)?;
        let component = state
            .component::<ContinuousActiveEffectsComponent>(request.entity)?
            .ok_or(ContinuousMechanicsError::MissingComponent {
                entity: request.entity,
                component: ContinuousActiveEffectsComponent::LABEL,
            })?;
        ensure_version(
            catalog,
            request.entity,
            ContinuousActiveEffectsComponent::LABEL,
            component.catalog_version(),
        )?;
        let mut candidate = component.clone();
        if !candidate.remove(&request.instance) {
            return Err(ContinuousMechanicsError::UnknownEffectInstance(
                request.instance,
            ));
        }
        validate_dependent_tracks_for_effects(state, catalog, request.entity, &candidate)?;
        EntityAuthoringService.replace_component(
            state,
            request.expected_revision.unwrap_or(actual.clone()),
            request.entity,
            candidate,
        )?;
        let committed =
            state.component_revision::<ContinuousActiveEffectsComponent>(request.entity)?;
        Ok(ContinuousEffectMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            entity: request.entity,
            instance: request.instance,
            removed: true,
            observed_revision: actual.revision(),
            committed_revision: committed.revision(),
        })
    }
}

#[derive(Debug)]
pub enum ContinuousMechanicsError {
    ComponentAccess(ComponentAccessError),
    Publication(EntityAuthoringError),
    Component(crate::ContinuousMechanicsComponentError),
    Value(ContinuousValueError),
    MissingComponent {
        entity: EntityId,
        component: &'static str,
    },
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    EvaluationQuotaExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    RevisionScopeMismatch {
        expected_entity: EntityId,
        expected_component: String,
        guard_entity: EntityId,
        guard_component: String,
    },
    CatalogVersion {
        entity: EntityId,
        component: &'static str,
        expected: String,
        actual: String,
    },
    UnknownStat(ContinuousStatId),
    UnknownTrack(ContinuousTrackId),
    UnknownSource(ContinuousSourceDefinitionId),
    UnknownEffect(ContinuousEffectDefinitionId),
    UnknownEffectInstance(ContinuousEffectInstanceId),
    MissingStat {
        entity: EntityId,
        stat: ContinuousStatId,
    },
    MissingTrack {
        entity: EntityId,
        track: ContinuousTrackId,
    },
    OutOfBounds {
        subject: String,
        bits: u64,
        minimum: u64,
        maximum: u64,
    },
    InvertedBounds {
        stat: ContinuousStatId,
        minimum: u64,
        maximum: u64,
    },
    InvertedTrackBounds {
        track: ContinuousTrackId,
        minimum: u64,
        maximum: u64,
    },
    NegativeAdjustment {
        bits: u64,
    },
    InsufficientTrack {
        track: ContinuousTrackId,
        current: u64,
        requested: u64,
        minimum: u64,
    },
    WouldInvalidateTrack {
        track: ContinuousTrackId,
        current: u64,
        prospective_maximum: u64,
    },
    WouldInvalidateTrackMinimum {
        track: ContinuousTrackId,
        current: u64,
        prospective_minimum: u64,
    },
}
impl std::fmt::Display for ContinuousMechanicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "continuous mechanics operation rejected: {self:?}")
    }
}
impl std::error::Error for ContinuousMechanicsError {}
impl From<ComponentAccessError> for ContinuousMechanicsError {
    fn from(value: ComponentAccessError) -> Self {
        Self::ComponentAccess(value)
    }
}
impl From<EntityAuthoringError> for ContinuousMechanicsError {
    fn from(value: EntityAuthoringError) -> Self {
        Self::Publication(value)
    }
}
impl From<crate::ContinuousMechanicsComponentError> for ContinuousMechanicsError {
    fn from(value: crate::ContinuousMechanicsComponentError) -> Self {
        Self::Component(value)
    }
}
impl From<ContinuousValueError> for ContinuousMechanicsError {
    fn from(value: ContinuousValueError) -> Self {
        Self::Value(value)
    }
}
fn check_revision(
    expected: Option<&ComponentRevision>,
    actual: &ComponentRevision,
) -> Result<(), ContinuousMechanicsError> {
    if let Some(expected) = expected {
        if expected.entity() != actual.entity() || expected.component() != actual.component() {
            return Err(ContinuousMechanicsError::RevisionScopeMismatch {
                expected_entity: actual.entity(),
                expected_component: actual.component().to_string(),
                guard_entity: expected.entity(),
                guard_component: expected.component().to_string(),
            });
        }
        if expected != actual {
            return Err(ContinuousMechanicsError::StaleRevision {
                expected: expected.revision(),
                actual: actual.revision(),
            });
        }
    }
    Ok(())
}
fn ensure_version(
    catalog: &ContinuousMechanicsCatalog,
    entity: EntityId,
    component: &'static str,
    actual: &ContinuousCatalogVersion,
) -> Result<(), ContinuousMechanicsError> {
    if actual != catalog.version() {
        return Err(ContinuousMechanicsError::CatalogVersion {
            entity,
            component,
            expected: catalog.version().to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}
fn ensure_value_in_bounds(
    subject: String,
    value: ContinuousValue,
    minimum: ContinuousValue,
    maximum: ContinuousValue,
) -> Result<(), ContinuousMechanicsError> {
    if value < minimum || value > maximum {
        return Err(ContinuousMechanicsError::OutOfBounds {
            subject,
            bits: value.bits(),
            minimum: minimum.bits(),
            maximum: maximum.bits(),
        });
    }
    Ok(())
}

/// Count-only preflight used immediately before each collector push/extension.
/// Keeping this independent of vectors proves a rejected `limit + 1` input allocates no next
/// activation or decision entry.
fn ensure_evaluation_quota(
    field: &'static str,
    existing: usize,
    incoming: usize,
    maximum: usize,
) -> Result<(), ContinuousMechanicsError> {
    let actual = existing.saturating_add(incoming);
    if actual > maximum {
        Err(ContinuousMechanicsError::EvaluationQuotaExceeded {
            field,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod quota_tests {
    use super::*;

    fn assert_boundary(field: &'static str, maximum: usize) {
        assert!(ensure_evaluation_quota(field, maximum, 0, maximum).is_ok());
        let error = ensure_evaluation_quota(field, maximum, 1, maximum).unwrap_err();
        assert!(
            matches!(error, ContinuousMechanicsError::EvaluationQuotaExceeded {
            field: actual_field, actual, maximum: actual_max
        } if actual_field == field && actual == maximum + 1 && actual_max == maximum)
        );
    }

    #[test]
    fn activation_quota_preflights_exact_limit_and_one_over_without_collection_allocation() {
        assert_boundary(
            "activations",
            MAX_CONTINUOUS_SOURCE_ACTIVATIONS_PER_EVALUATION,
        );
    }

    #[test]
    fn decision_quota_preflights_exact_limit_and_one_over_without_collection_allocation() {
        assert_boundary("decisions", MAX_CONTINUOUS_DECISIONS_PER_EVALUATION);
    }
}

fn validate_dependent_tracks_for_base(
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
    entity: EntityId,
    stat: &ContinuousStatId,
    before_base: ContinuousValue,
    after_base: ContinuousValue,
) -> Result<(), ContinuousMechanicsError> {
    let Some(tracks) = state.component::<ContinuousTracksComponent>(entity)? else {
        return Ok(());
    };
    let delta = after_base.checked_sub(before_base)?;
    for stored in tracks.values() {
        let definition = catalog
            .track(stored.track())
            .ok_or_else(|| ContinuousMechanicsError::UnknownTrack(stored.track().clone()))?;
        let ContinuousTrackMaximum::Stat { stat: bound_stat } = &definition.maximum else {
            continue;
        };
        if bound_stat != stat {
            continue;
        }
        let evaluation = ContinuousStatService::evaluate(state, catalog, entity, stat)?;
        let prospective_maximum = evaluation
            .after_additions
            .checked_add(delta)?
            .clamp(evaluation.minimum, evaluation.maximum);
        if stored.current() > prospective_maximum {
            return Err(ContinuousMechanicsError::WouldInvalidateTrack {
                track: stored.track().clone(),
                current: stored.current().bits(),
                prospective_maximum: prospective_maximum.bits(),
            });
        }
    }
    Ok(())
}

fn validate_dependent_tracks_for_effects(
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
    entity: EntityId,
    candidate_effects: &ContinuousActiveEffectsComponent,
) -> Result<(), ContinuousMechanicsError> {
    let Some(tracks) = state.component::<ContinuousTracksComponent>(entity)? else {
        return Ok(());
    };
    for stored in tracks.values() {
        let definition = catalog
            .track(stored.track())
            .ok_or_else(|| ContinuousMechanicsError::UnknownTrack(stored.track().clone()))?;
        let ContinuousTrackMaximum::Stat { stat } = &definition.maximum else {
            continue;
        };
        let prospective =
            evaluate_with_effects(state, catalog, entity, stat, Some(candidate_effects))?;
        if stored.current() < prospective.minimum {
            return Err(ContinuousMechanicsError::WouldInvalidateTrackMinimum {
                track: stored.track().clone(),
                current: stored.current().bits(),
                prospective_minimum: prospective.minimum.bits(),
            });
        }
        if stored.current() > prospective.value {
            return Err(ContinuousMechanicsError::WouldInvalidateTrack {
                track: stored.track().clone(),
                current: stored.current().bits(),
                prospective_maximum: prospective.value.bits(),
            });
        }
    }
    Ok(())
}
