use std::collections::BTreeMap;

use core_ids::EntityId;
use entity_state::EntityState;

use crate::{
    SpatialOcclusionError, SpatialOcclusionQuery, SpatialOcclusionService, VoxelCollisionScene,
};

/// Maximum number of observer facts accepted by one perception query.
pub const MAX_PERCEPTION_OBSERVERS: usize = 64;
/// Maximum number of target facts accepted by one perception query.
pub const MAX_PERCEPTION_TARGETS: usize = 256;
/// Maximum number of distance-qualified observer/target pairs retained by one query.
pub const MAX_PERCEPTION_PAIRS: usize = 1_024;
/// Maximum number of targets retained in the deterministic reduction.
pub const MAX_PERCEPTION_AGGREGATES: usize = 256;

/// A caller-owned world-space sensing origin and policy-independent observation fact.
///
/// `maximum_distance`, `minimum_facing_cosine`, and `evidence` are facts supplied by the
/// product. The Engine evaluates them but does not interpret evidence as an alert, threat, or
/// other gameplay mutation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialPerceptionObserver {
    pub entity: EntityId,
    pub origin: [f64; 3],
    pub forward: [f64; 3],
    pub maximum_distance: f64,
    pub minimum_facing_cosine: f64,
    pub evidence: f64,
}

/// A caller-owned world-space target center.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialPerceptionTarget {
    pub entity: EntityId,
    pub center: [f64; 3],
}

/// Result of one distance-qualified observer/target evaluation.
///
/// Distance-rejected pairs are omitted from this collection and counted in the enclosing
/// readout. A pair that passes distance but fails facing is retained with `FacingRejected`; a
/// pair that passes both tests is retained with either `Visible` or `Occluded`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialPerceptionPairKind {
    Visible,
    FacingRejected,
    Occluded,
}

/// Typed distance, facing, and occlusion facts for one distance-qualified pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialPerceptionPair {
    pub observer: EntityId,
    pub target: EntityId,
    pub distance: f64,
    pub facing_cosine: f64,
    pub kind: SpatialPerceptionPairKind,
    pub evidence: f64,
}

/// Deterministic visible-observer reduction for one target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialPerceptionAggregate {
    pub target: EntityId,
    pub visible_observer_count: u64,
    pub evidence_total: f64,
}

/// All typed facts produced by one bounded spatial perception query.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialPerceptionReadout {
    pub pairs: Vec<SpatialPerceptionPair>,
    pub aggregates: Vec<SpatialPerceptionAggregate>,
    pub selected_observers: usize,
    pub selected_targets: usize,
    pub selection_comparisons: usize,
    pub distance_rejects: usize,
    pub facing_rejects: usize,
    pub visibility_casts: usize,
    pub occlusion_rejects: usize,
}

/// Inputs for one read-only perception evaluation.
pub struct SpatialPerceptionQuery<'a> {
    pub scene: &'a VoxelCollisionScene,
    pub entities: &'a EntityState,
    pub observers: &'a [SpatialPerceptionObserver],
    pub targets: &'a [SpatialPerceptionTarget],
}

/// Read-only spatial sensing over the canonical voxel scene and caller-owned active colliders.
///
/// This is intentionally a mechanism rather than an AI system: it returns facts, does not retain
/// observer/target roles, and never publishes or mutates product state.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpatialPerceptionService;

impl SpatialPerceptionService {
    pub fn evaluate(
        self,
        query: SpatialPerceptionQuery<'_>,
    ) -> Result<SpatialPerceptionReadout, SpatialPerceptionError> {
        if query.observers.len() > MAX_PERCEPTION_OBSERVERS {
            return Err(SpatialPerceptionError::TooManyObservers {
                actual: query.observers.len(),
                limit: MAX_PERCEPTION_OBSERVERS,
            });
        }
        if query.targets.len() > MAX_PERCEPTION_TARGETS {
            return Err(SpatialPerceptionError::TooManyTargets {
                actual: query.targets.len(),
                limit: MAX_PERCEPTION_TARGETS,
            });
        }

        let mut observers = query.observers.to_vec();
        observers.sort_by_key(|observer| observer.entity);
        ensure_unique_observers(&observers)?;
        for observer in &observers {
            validate_observer(*observer)?;
        }

        let mut targets = query.targets.to_vec();
        targets.sort_by_key(|target| target.entity);
        ensure_unique_targets(&targets)?;
        for target in &targets {
            if !finite_vec3(target.center) {
                return Err(SpatialPerceptionError::InvalidTarget(target.entity));
            }
        }

        let mut pairs = Vec::new();
        let mut selection_comparisons = 0usize;
        let mut distance_rejects = 0usize;
        let mut facing_rejects = 0usize;
        let mut visibility_casts = 0usize;
        let mut occlusion_rejects = 0usize;
        let mut reductions: BTreeMap<EntityId, (u64, f64)> = BTreeMap::new();

        for observer in &observers {
            for target in &targets {
                selection_comparisons = selection_comparisons
                    .checked_add(1)
                    .ok_or(SpatialPerceptionError::ArithmeticOverflow)?;
                let delta = subtract(target.center, observer.origin);
                let distance_squared = dot(delta, delta);
                if !distance_squared.is_finite()
                    || distance_squared > observer.maximum_distance * observer.maximum_distance
                {
                    distance_rejects = distance_rejects
                        .checked_add(1)
                        .ok_or(SpatialPerceptionError::ArithmeticOverflow)?;
                    continue;
                }

                if pairs.len() >= MAX_PERCEPTION_PAIRS {
                    return Err(SpatialPerceptionError::TooManyPairs {
                        actual: pairs.len() + 1,
                        limit: MAX_PERCEPTION_PAIRS,
                    });
                }
                let distance = distance_squared.sqrt();
                let facing_cosine = if distance <= 0.0 {
                    0.0
                } else {
                    dot(observer.forward, delta) / (vector_length(observer.forward) * distance)
                };
                if distance <= 0.0
                    || !facing_cosine.is_finite()
                    || facing_cosine < observer.minimum_facing_cosine
                {
                    facing_rejects = facing_rejects
                        .checked_add(1)
                        .ok_or(SpatialPerceptionError::ArithmeticOverflow)?;
                    pairs.push(SpatialPerceptionPair {
                        observer: observer.entity,
                        target: target.entity,
                        distance,
                        facing_cosine,
                        kind: SpatialPerceptionPairKind::FacingRejected,
                        evidence: observer.evidence,
                    });
                    continue;
                }

                visibility_casts = visibility_casts
                    .checked_add(1)
                    .ok_or(SpatialPerceptionError::ArithmeticOverflow)?;
                let hit = SpatialOcclusionService
                    .cast_ray(
                        query.scene,
                        query.entities,
                        SpatialOcclusionQuery {
                            origin: observer.origin,
                            direction: delta,
                            max_distance: distance,
                            ignored_entities: &[observer.entity, target.entity],
                        },
                    )
                    .map_err(SpatialPerceptionError::Occlusion)?;
                let kind = if hit.is_some() {
                    occlusion_rejects = occlusion_rejects
                        .checked_add(1)
                        .ok_or(SpatialPerceptionError::ArithmeticOverflow)?;
                    SpatialPerceptionPairKind::Occluded
                } else {
                    let entry = reductions.entry(target.entity).or_insert((0, 0.0));
                    entry.0 = entry
                        .0
                        .checked_add(1)
                        .ok_or(SpatialPerceptionError::ArithmeticOverflow)?;
                    entry.1 += observer.evidence;
                    if !entry.1.is_finite() {
                        return Err(SpatialPerceptionError::NonFiniteEvidence);
                    }
                    SpatialPerceptionPairKind::Visible
                };
                pairs.push(SpatialPerceptionPair {
                    observer: observer.entity,
                    target: target.entity,
                    distance,
                    facing_cosine,
                    kind,
                    evidence: observer.evidence,
                });
            }
        }

        if reductions.len() > MAX_PERCEPTION_AGGREGATES {
            return Err(SpatialPerceptionError::TooManyAggregates {
                actual: reductions.len(),
                limit: MAX_PERCEPTION_AGGREGATES,
            });
        }
        let aggregates = reductions
            .into_iter()
            .map(
                |(target, (visible_observer_count, evidence_total))| SpatialPerceptionAggregate {
                    target,
                    visible_observer_count,
                    evidence_total,
                },
            )
            .collect();
        Ok(SpatialPerceptionReadout {
            pairs,
            aggregates,
            selected_observers: observers.len(),
            selected_targets: targets.len(),
            selection_comparisons,
            distance_rejects,
            facing_rejects,
            visibility_casts,
            occlusion_rejects,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialPerceptionError {
    TooManyObservers { actual: usize, limit: usize },
    TooManyTargets { actual: usize, limit: usize },
    TooManyPairs { actual: usize, limit: usize },
    TooManyAggregates { actual: usize, limit: usize },
    DuplicateObserver(EntityId),
    DuplicateTarget(EntityId),
    InvalidObserver(EntityId),
    InvalidTarget(EntityId),
    NonFiniteEvidence,
    ArithmeticOverflow,
    Occlusion(SpatialOcclusionError),
}

impl std::fmt::Display for SpatialPerceptionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "spatial perception query rejected: {self:?}")
    }
}

impl std::error::Error for SpatialPerceptionError {}

fn ensure_unique_observers(
    observers: &[SpatialPerceptionObserver],
) -> Result<(), SpatialPerceptionError> {
    for pair in observers.windows(2) {
        if pair[0].entity == pair[1].entity {
            return Err(SpatialPerceptionError::DuplicateObserver(pair[0].entity));
        }
    }
    Ok(())
}

fn ensure_unique_targets(
    targets: &[SpatialPerceptionTarget],
) -> Result<(), SpatialPerceptionError> {
    for pair in targets.windows(2) {
        if pair[0].entity == pair[1].entity {
            return Err(SpatialPerceptionError::DuplicateTarget(pair[0].entity));
        }
    }
    Ok(())
}

fn validate_observer(observer: SpatialPerceptionObserver) -> Result<(), SpatialPerceptionError> {
    if !finite_vec3(observer.origin)
        || !finite_vec3(observer.forward)
        || vector_length_squared(observer.forward) <= 0.0
        || !observer.maximum_distance.is_finite()
        || observer.maximum_distance <= 0.0
        || !observer.minimum_facing_cosine.is_finite()
        || !(-1.0..=1.0).contains(&observer.minimum_facing_cosine)
        || !observer.evidence.is_finite()
    {
        return Err(SpatialPerceptionError::InvalidObserver(observer.entity));
    }
    Ok(())
}

fn finite_vec3(value: [f64; 3]) -> bool {
    value.into_iter().all(f64::is_finite)
}

fn vector_length_squared(value: [f64; 3]) -> f64 {
    dot(value, value)
}

fn vector_length(value: [f64; 3]) -> f64 {
    vector_length_squared(value).sqrt()
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}
