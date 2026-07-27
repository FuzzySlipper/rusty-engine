use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use entity_state::{ComponentRevision, EntityAuthoringService, EntityState};

use crate::{
    source::collect_active_sources, stat::track_bounds, track::ensure_revision, CombinedRatio,
    DamageKindId, DamageResponseDefinition, DecisionOutcome, ExactRatio, MechanicsCatalog,
    MechanicsComponentKind, MechanicsError, MechanicsScalar, ObservedComponentRevision,
    OperationId, RequestSource, RoundingPolicy, SourceCollectionCost, SourceDefinitionId,
    SourceInstanceIdentity, StackingGroupId, StackingPolicy, TrackId, TracksComponent,
    MAX_REQUEST_SOURCES,
};

pub const MAX_DAMAGE_PARTS: usize = 8;
pub const MAX_DAMAGE_REQUEST_SOURCES: usize = MAX_REQUEST_SOURCES;
pub const MAX_DAMAGE_RECEIPT_DECISIONS: usize = 256;
pub const MAX_DAMAGE_FACTS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamagePart {
    pub amount: MechanicsScalar,
    pub kind: DamageKindId,
}

#[derive(Debug, Clone)]
pub struct DamageRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub actor: Option<EntityId>,
    pub target: EntityId,
    pub target_track: TrackId,
    pub parts: Vec<DamagePart>,
    pub request_sources: Vec<RequestSource>,
    pub expected_tracks_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseDecisionKind {
    NoDamageResponse,
    Prevent,
    FlatReduction { amount: MechanicsScalar },
    Scale { ratio: ExactRatio },
    Absorb { track: TrackId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseDecision {
    pub part_index: u16,
    pub source: SourceInstanceIdentity,
    pub source_definition: SourceDefinitionId,
    pub response_index: Option<u16>,
    pub kind: ResponseDecisionKind,
    pub outcome: DecisionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamagePartReceipt {
    pub index: u16,
    pub kind: DamageKindId,
    pub original: MechanicsScalar,
    pub prevented: bool,
    pub after_flat: MechanicsScalar,
    pub combined_scale_numerator: u128,
    pub combined_scale_denominator: u128,
    pub after_scale: MechanicsScalar,
    pub absorbed: MechanicsScalar,
    pub applied: MechanicsScalar,
    pub unapplied: MechanicsScalar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackDamageChange {
    pub track: TrackId,
    pub before: MechanicsScalar,
    pub after: MechanicsScalar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageFact {
    ProtectionTrackDepleted { track: TrackId, part_index: u16 },
    TargetTrackDepleted { track: TrackId, part_index: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageReceipt {
    pub catalog_version: crate::CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub actor: Option<EntityId>,
    pub target: EntityId,
    pub target_track: TrackId,
    pub observed_tracks_revision: u64,
    pub committed_tracks_revision: Option<u64>,
    pub parts: Vec<DamagePartReceipt>,
    pub decisions: Vec<ResponseDecision>,
    pub track_changes: Vec<TrackDamageChange>,
    pub facts: Vec<DamageFact>,
    pub observed_revisions: Vec<ObservedComponentRevision>,
    pub source_cost: SourceCollectionCost,
}

#[derive(Debug, Clone)]
pub struct DamagePreview {
    receipt: DamageReceipt,
    candidate: TracksComponent,
    revision: ComponentRevision,
}

impl DamagePreview {
    pub fn receipt(&self) -> &DamageReceipt {
        &self.receipt
    }

    pub fn observed_revision(&self) -> &ComponentRevision {
        &self.revision
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DamageService;

impl DamageService {
    pub fn preview(
        state: &EntityState,
        catalog: &MechanicsCatalog,
        request: &DamageRequest,
    ) -> Result<DamagePreview, MechanicsError> {
        validate_request(catalog, request)?;
        let actual_revision = state.component_revision::<TracksComponent>(request.target)?;
        if let Some(expected) = &request.expected_tracks_revision {
            ensure_revision(expected, &actual_revision)?;
        }
        let component = state.component::<TracksComponent>(request.target)?.ok_or(
            MechanicsError::MissingComponent {
                entity: request.target,
                component: TracksComponent::LABEL,
            },
        )?;
        crate::source::ensure_catalog_version(
            catalog,
            request.target,
            TracksComponent::LABEL,
            component.catalog_version(),
        )?;
        if component.current(&request.target_track).is_none() {
            return Err(MechanicsError::MissingTrack {
                entity: request.target,
                track: request.target_track.clone(),
            });
        }

        let (sources, mut source_cost, mut observed_revisions) = collect_active_sources(
            state,
            catalog,
            request.target,
            &request.operation,
            &request.request_sources,
        )?;
        observed_revisions.push(ObservedComponentRevision {
            entity: request.target,
            component: MechanicsComponentKind::Tracks,
            revision: actual_revision.revision(),
        });

        let mut candidate = component.clone();
        let mut parts = Vec::with_capacity(request.parts.len());
        let mut decisions = Vec::new();
        let mut facts = Vec::new();
        let mut original_tracks = BTreeMap::new();

        for (part_index, part) in request.parts.iter().enumerate() {
            let part_index = part_index as u16;
            let mut candidates =
                collect_response_candidates(catalog, &sources, part, part_index, &mut decisions)?;
            if decisions.len() > MAX_DAMAGE_RECEIPT_DECISIONS {
                return Err(MechanicsError::ReceiptQuotaExceeded {
                    actual: decisions.len(),
                    maximum: MAX_DAMAGE_RECEIPT_DECISIONS,
                });
            }
            select_responses(&mut candidates, &mut decisions);

            let prevented = candidates.iter().any(|candidate| {
                candidate.stage == ResponseStage::Prevent
                    && decisions[candidate.decision_index].outcome == DecisionOutcome::Applied
            });
            if prevented {
                for candidate in &candidates {
                    if candidate.stage != ResponseStage::Prevent
                        && decisions[candidate.decision_index].outcome
                            != DecisionOutcome::Inapplicable
                    {
                        decisions[candidate.decision_index].outcome = DecisionOutcome::Inapplicable;
                    }
                }
                parts.push(DamagePartReceipt {
                    index: part_index,
                    kind: part.kind.clone(),
                    original: part.amount,
                    prevented: true,
                    after_flat: MechanicsScalar::zero(),
                    combined_scale_numerator: 1,
                    combined_scale_denominator: 1,
                    after_scale: MechanicsScalar::zero(),
                    absorbed: MechanicsScalar::zero(),
                    applied: MechanicsScalar::zero(),
                    unapplied: MechanicsScalar::zero(),
                });
                continue;
            }

            let mut flat_total = MechanicsScalar::zero();
            for candidate in &candidates {
                if candidate.stage != ResponseStage::Flat
                    || decisions[candidate.decision_index].outcome != DecisionOutcome::Applied
                {
                    continue;
                }
                if let DamageResponseDefinition::FlatReduction { amount, .. } = &candidate.response
                {
                    flat_total = flat_total.checked_add(*amount)?;
                }
            }
            let after_flat = if flat_total >= part.amount {
                MechanicsScalar::zero()
            } else {
                part.amount.checked_sub(flat_total)?
            };

            let mut combined_scale = CombinedRatio::one();
            for candidate in &candidates {
                if candidate.stage != ResponseStage::Scale
                    || decisions[candidate.decision_index].outcome != DecisionOutcome::Applied
                {
                    continue;
                }
                if let DamageResponseDefinition::Scale { ratio, .. } = &candidate.response {
                    combined_scale.include(*ratio)?;
                }
            }
            let after_scale =
                combined_scale.apply_nonnegative(after_flat, RoundingPolicy::TowardZero)?;

            let mut remaining = after_scale;
            let mut absorbed = MechanicsScalar::zero();
            for candidate_response in &candidates {
                if candidate_response.stage != ResponseStage::Absorb
                    || decisions[candidate_response.decision_index].outcome
                        != DecisionOutcome::Applied
                {
                    continue;
                }
                let DamageResponseDefinition::Absorb { track, .. } = &candidate_response.response
                else {
                    unreachable!("absorption stage contains absorption response");
                };
                if track == &request.target_track {
                    decisions[candidate_response.decision_index].outcome =
                        DecisionOutcome::Inapplicable;
                    continue;
                }
                let before =
                    candidate
                        .current(track)
                        .ok_or_else(|| MechanicsError::MissingTrack {
                            entity: request.target,
                            track: track.clone(),
                        })?;
                let (minimum, maximum, observed, cost) =
                    track_bounds(state, catalog, request.target, track, &request.operation)?;
                merge_evidence(&mut observed_revisions, &mut source_cost, observed, cost);
                validate_current(request.target, track, before, minimum, maximum)?;
                original_tracks.entry(track.clone()).or_insert(before);
                let available = before.checked_sub(minimum)?;
                let used = available.min(remaining);
                let after = before.checked_sub(used)?;
                assert!(candidate.set_current(track, after));
                absorbed = absorbed.checked_add(used)?;
                remaining = remaining.checked_sub(used)?;
                if before > minimum && after == minimum {
                    facts.push(DamageFact::ProtectionTrackDepleted {
                        track: track.clone(),
                        part_index,
                    });
                }
            }

            let target_before = candidate.current(&request.target_track).ok_or_else(|| {
                MechanicsError::MissingTrack {
                    entity: request.target,
                    track: request.target_track.clone(),
                }
            })?;
            let (target_minimum, target_maximum, observed, cost) = track_bounds(
                state,
                catalog,
                request.target,
                &request.target_track,
                &request.operation,
            )?;
            merge_evidence(&mut observed_revisions, &mut source_cost, observed, cost);
            validate_current(
                request.target,
                &request.target_track,
                target_before,
                target_minimum,
                target_maximum,
            )?;
            original_tracks
                .entry(request.target_track.clone())
                .or_insert(target_before);
            let available = target_before.checked_sub(target_minimum)?;
            let applied = available.min(remaining);
            let target_after = target_before.checked_sub(applied)?;
            assert!(candidate.set_current(&request.target_track, target_after));
            if target_before > target_minimum && target_after == target_minimum {
                facts.push(DamageFact::TargetTrackDepleted {
                    track: request.target_track.clone(),
                    part_index,
                });
            }
            let unapplied = remaining.checked_sub(applied)?;
            parts.push(DamagePartReceipt {
                index: part_index,
                kind: part.kind.clone(),
                original: part.amount,
                prevented: false,
                after_flat,
                combined_scale_numerator: combined_scale.numerator(),
                combined_scale_denominator: combined_scale.denominator(),
                after_scale,
                absorbed,
                applied,
                unapplied,
            });
        }

        if facts.len() > MAX_DAMAGE_FACTS {
            return Err(MechanicsError::ReceiptQuotaExceeded {
                actual: facts.len(),
                maximum: MAX_DAMAGE_FACTS,
            });
        }
        let track_changes = original_tracks
            .into_iter()
            .map(|(track, before)| {
                let after = candidate
                    .current(&track)
                    .expect("touched tracks remain in the staged component");
                TrackDamageChange {
                    track,
                    before,
                    after,
                }
            })
            .collect();
        observed_revisions.sort_by_key(|value| (value.entity, value.component));
        observed_revisions.dedup();

        Ok(DamagePreview {
            receipt: DamageReceipt {
                catalog_version: catalog.version().clone(),
                catalog_fingerprint: catalog.fingerprint().to_string(),
                operation: request.operation.clone(),
                source: request.source.clone(),
                actor: request.actor,
                target: request.target,
                target_track: request.target_track.clone(),
                observed_tracks_revision: actual_revision.revision(),
                committed_tracks_revision: None,
                parts,
                decisions,
                track_changes,
                facts,
                observed_revisions,
                source_cost,
            },
            candidate,
            revision: actual_revision,
        })
    }

    pub fn apply(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: DamageRequest,
    ) -> Result<DamageReceipt, MechanicsError> {
        let preview = Self::preview(state, catalog, &request)?;
        let publish_revision = request
            .expected_tracks_revision
            .clone()
            .unwrap_or_else(|| preview.revision.clone());
        EntityAuthoringService.replace_component(
            state,
            publish_revision,
            request.target,
            preview.candidate,
        )?;
        let committed_revision = state
            .component_revision::<TracksComponent>(request.target)?
            .revision();
        let mut receipt = preview.receipt;
        receipt.committed_tracks_revision = Some(committed_revision);
        Ok(receipt)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ResponseStage {
    Prevent,
    Flat,
    Scale,
    Absorb,
}

#[derive(Clone)]
struct ResponseCandidate {
    decision_index: usize,
    source_definition: SourceDefinitionId,
    stage: ResponseStage,
    group: Option<StackingGroupId>,
    stacking: Option<StackingPolicy>,
    response: DamageResponseDefinition,
}

fn validate_request(
    catalog: &MechanicsCatalog,
    request: &DamageRequest,
) -> Result<(), MechanicsError> {
    if request.parts.is_empty() || request.parts.len() > MAX_DAMAGE_PARTS {
        return Err(MechanicsError::RequestQuotaExceeded {
            field: "damageParts",
            actual: request.parts.len(),
            maximum: MAX_DAMAGE_PARTS,
        });
    }
    if request.request_sources.len() > MAX_DAMAGE_REQUEST_SOURCES {
        return Err(MechanicsError::RequestQuotaExceeded {
            field: "requestSources",
            actual: request.request_sources.len(),
            maximum: MAX_DAMAGE_REQUEST_SOURCES,
        });
    }
    if catalog.track(&request.target_track).is_none() {
        return Err(MechanicsError::UnknownTrack {
            track: request.target_track.clone(),
        });
    }
    for part in &request.parts {
        part.amount.require_nonnegative()?;
        if catalog.damage_kind(&part.kind).is_none() {
            return Err(MechanicsError::UnknownDamageKind {
                kind: part.kind.clone(),
            });
        }
    }
    Ok(())
}

fn collect_response_candidates(
    catalog: &MechanicsCatalog,
    sources: &[crate::ActiveSource],
    part: &DamagePart,
    part_index: u16,
    decisions: &mut Vec<ResponseDecision>,
) -> Result<Vec<ResponseCandidate>, MechanicsError> {
    let mut candidates = Vec::new();
    for source in sources {
        let definition = catalog
            .source(&source.definition)
            .expect("source collection admits catalog definitions");
        if definition.damage_responses.is_empty() {
            decisions.push(ResponseDecision {
                part_index,
                source: source.identity.clone(),
                source_definition: source.definition.clone(),
                response_index: None,
                kind: ResponseDecisionKind::NoDamageResponse,
                outcome: DecisionOutcome::Inapplicable,
            });
            continue;
        }
        for (response_index, response) in definition.damage_responses.iter().enumerate() {
            let (stage, kind) = match response {
                DamageResponseDefinition::Prevent { .. } => {
                    (ResponseStage::Prevent, ResponseDecisionKind::Prevent)
                }
                DamageResponseDefinition::FlatReduction { amount, .. } => (
                    ResponseStage::Flat,
                    ResponseDecisionKind::FlatReduction { amount: *amount },
                ),
                DamageResponseDefinition::Scale { ratio, .. } => (
                    ResponseStage::Scale,
                    ResponseDecisionKind::Scale { ratio: *ratio },
                ),
                DamageResponseDefinition::Absorb { track, .. } => (
                    ResponseStage::Absorb,
                    ResponseDecisionKind::Absorb {
                        track: track.clone(),
                    },
                ),
            };
            let applicable = response.selector().matches(&part.kind);
            let decision_index = decisions.len();
            decisions.push(ResponseDecision {
                part_index,
                source: source.identity.clone(),
                source_definition: source.definition.clone(),
                response_index: Some(response_index as u16),
                kind,
                outcome: if applicable {
                    if stage == ResponseStage::Absorb {
                        DecisionOutcome::Applied
                    } else {
                        DecisionOutcome::Suppressed
                    }
                } else {
                    DecisionOutcome::Inapplicable
                },
            });
            if applicable {
                let stacking = response.stacking();
                candidates.push(ResponseCandidate {
                    decision_index,
                    source_definition: source.definition.clone(),
                    stage,
                    group: stacking.map(|(group, _)| group.clone()),
                    stacking: stacking.map(|(_, policy)| policy),
                    response: response.clone(),
                });
            }
        }
    }
    Ok(candidates)
}

fn select_responses(candidates: &mut [ResponseCandidate], decisions: &mut [ResponseDecision]) {
    let mut groups: BTreeMap<(ResponseStage, &StackingGroupId), Vec<usize>> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if let Some(group) = &candidate.group {
            groups
                .entry((candidate.stage, group))
                .or_default()
                .push(index);
        }
    }
    for indexes in groups.values() {
        let policy = candidates[indexes[0]]
            .stacking
            .expect("grouped response has stacking policy");
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
                        let ordering =
                            compare_response_value(&candidates[candidate], &candidates[selected]);
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
                    .expect("response stacking groups are nonempty");
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

fn compare_response_value(
    left: &ResponseCandidate,
    right: &ResponseCandidate,
) -> std::cmp::Ordering {
    match (&left.response, &right.response) {
        (
            DamageResponseDefinition::FlatReduction { amount: left, .. },
            DamageResponseDefinition::FlatReduction { amount: right, .. },
        ) => left.cmp(right),
        (
            DamageResponseDefinition::Scale { ratio: left, .. },
            DamageResponseDefinition::Scale { ratio: right, .. },
        ) => {
            let left_product = u64::from(left.numerator()) * u64::from(right.denominator());
            let right_product = u64::from(right.numerator()) * u64::from(left.denominator());
            left_product.cmp(&right_product)
        }
        (DamageResponseDefinition::Prevent { .. }, DamageResponseDefinition::Prevent { .. }) => {
            std::cmp::Ordering::Equal
        }
        _ => unreachable!("response groups are stage-homogeneous"),
    }
}

fn validate_current(
    entity: EntityId,
    track: &TrackId,
    current: MechanicsScalar,
    minimum: MechanicsScalar,
    maximum: MechanicsScalar,
) -> Result<(), MechanicsError> {
    if current < minimum || current > maximum {
        return Err(MechanicsError::TrackOutOfBounds {
            entity,
            track: track.clone(),
            attempted: current.get(),
            minimum: minimum.get(),
            maximum: maximum.get(),
        });
    }
    Ok(())
}

fn merge_evidence(
    observed_revisions: &mut Vec<ObservedComponentRevision>,
    source_cost: &mut SourceCollectionCost,
    mut observed: Vec<ObservedComponentRevision>,
    cost: SourceCollectionCost,
) {
    observed_revisions.append(&mut observed);
    source_cost.intrinsic_entries_visited += cost.intrinsic_entries_visited;
    source_cost.effect_entries_visited += cost.effect_entries_visited;
    source_cost.equipment_entries_visited += cost.equipment_entries_visited;
    source_cost.item_components_read += cost.item_components_read;
    source_cost.request_entries_visited += cost.request_entries_visited;
}
