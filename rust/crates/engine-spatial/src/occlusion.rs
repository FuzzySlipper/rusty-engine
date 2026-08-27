use std::cmp::Ordering;

use core_ids::EntityId;
use entity_state::{BoundsComponent, EntityState};

use crate::active_collision::active_entity_colliders;
use crate::{CollisionRayHit, VoxelCollisionScene};

/// Maximum number of entity records one combined occlusion query will inspect.
pub const MAX_OCCLUSION_QUERY_ENTITIES: usize = 4_096;
/// Maximum number of caller-owned endpoint or source identities omitted by one query.
pub const MAX_OCCLUSION_IGNORED_ENTITIES: usize = 8;
/// Maximum number of caller-owned world-space hitboxes that one combined
/// query may override. The list is deliberately bounded like the ignored set;
/// ownership of hitbox policy remains with the product while hit testing and
/// ordering remain Engine-owned.
pub const MAX_OCCLUSION_HITBOX_OVERRIDES: usize = MAX_OCCLUSION_QUERY_ENTITIES;

/// A caller-owned world-space AABB used for one occlusion query. The entity
/// must also be an active collider in the supplied [`EntityState`]; this value
/// only replaces that entity's ordinary bounds for this call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialOcclusionHitboxOverride {
    pub entity: EntityId,
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// One bounded ray against canonical voxel geometry and current active entity
/// colliders. Callers normally ignore the source and intended target identities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialOcclusionQuery<'a> {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
    pub max_distance: f64,
    pub ignored_entities: &'a [EntityId],
}

/// The nearest canonical occluder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialOcclusionHit {
    Entity {
        entity: EntityId,
        point: [f64; 3],
        distance: f64,
    },
    Voxel(CollisionRayHit),
}

impl SpatialOcclusionHit {
    pub const fn distance(self) -> f64 {
        match self {
            Self::Entity { distance, .. } => distance,
            Self::Voxel(hit) => hit.distance,
        }
    }

    pub const fn point(self) -> [f64; 3] {
        match self {
            Self::Entity { point, .. } => point,
            Self::Voxel(hit) => hit.point,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialOcclusionError {
    InvalidOrigin,
    InvalidDirection,
    InvalidMaxDistance,
    TooManyIgnoredEntities { actual: usize, limit: usize },
    TooManyEntities { actual: usize, limit: usize },
    TooManyHitboxOverrides { actual: usize, limit: usize },
    InvalidHitboxOverride { entity: EntityId },
}

impl std::fmt::Display for SpatialOcclusionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "spatial occlusion query rejected: {self:?}")
    }
}

impl std::error::Error for SpatialOcclusionError {}

/// Read-only owner for combined voxel and retained-entity occlusion.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpatialOcclusionService;

impl SpatialOcclusionService {
    /// Return the nearest hit using strict distance order. Exact cross-domain
    /// ties prefer an entity so stable entity identity is not lost to coincident
    /// voxel geometry; exact entity ties prefer the lower [`EntityId`].
    pub fn cast_ray(
        self,
        scene: &VoxelCollisionScene,
        entities: &EntityState,
        query: SpatialOcclusionQuery<'_>,
    ) -> Result<Option<SpatialOcclusionHit>, SpatialOcclusionError> {
        let _direction = validate_and_normalize(query)?;
        if query.ignored_entities.len() > MAX_OCCLUSION_IGNORED_ENTITIES {
            return Err(SpatialOcclusionError::TooManyIgnoredEntities {
                actual: query.ignored_entities.len(),
                limit: MAX_OCCLUSION_IGNORED_ENTITIES,
            });
        }
        let entity_count = entities.total_count();
        if entity_count > MAX_OCCLUSION_QUERY_ENTITIES {
            return Err(SpatialOcclusionError::TooManyEntities {
                actual: entity_count,
                limit: MAX_OCCLUSION_QUERY_ENTITIES,
            });
        }

        Self::cast_ray_with_overrides(scene, entities, query, &[])
    }

    /// Variant of [`Self::cast_ray`] that replaces the active bounds for named
    /// entities with bounded caller-owned world-space boxes. This is the
    /// Engine implementation for product hitboxes: target eligibility and the
    /// box dimensions stay product-owned, while normalization, filtering,
    /// nearest ordering, and voxel/entity ties remain one shared service.
    pub fn cast_ray_with_overrides(
        scene: &VoxelCollisionScene,
        entities: &EntityState,
        query: SpatialOcclusionQuery<'_>,
        overrides: &[SpatialOcclusionHitboxOverride],
    ) -> Result<Option<SpatialOcclusionHit>, SpatialOcclusionError> {
        let direction = validate_and_normalize(query)?;
        if query.ignored_entities.len() > MAX_OCCLUSION_IGNORED_ENTITIES {
            return Err(SpatialOcclusionError::TooManyIgnoredEntities {
                actual: query.ignored_entities.len(),
                limit: MAX_OCCLUSION_IGNORED_ENTITIES,
            });
        }
        let entity_count = entities.total_count();
        if entity_count > MAX_OCCLUSION_QUERY_ENTITIES {
            return Err(SpatialOcclusionError::TooManyEntities {
                actual: entity_count,
                limit: MAX_OCCLUSION_QUERY_ENTITIES,
            });
        }
        if overrides.len() > MAX_OCCLUSION_HITBOX_OVERRIDES {
            return Err(SpatialOcclusionError::TooManyHitboxOverrides {
                actual: overrides.len(),
                limit: MAX_OCCLUSION_HITBOX_OVERRIDES,
            });
        }
        for value in overrides {
            if !value.min.into_iter().chain(value.max).all(f64::is_finite)
                || value.min.iter().zip(value.max).any(|(min, max)| min > &max)
            {
                return Err(SpatialOcclusionError::InvalidHitboxOverride {
                    entity: value.entity,
                });
            }
        }
        let mut nearest = scene
            .raycast(query.origin, direction, query.max_distance)
            .map(SpatialOcclusionHit::Voxel);
        for collider in active_entity_colliders(entities) {
            if query.ignored_entities.contains(&collider.entity) {
                continue;
            }
            let bounds = overrides
                .iter()
                .find(|value| value.entity == collider.entity)
                .map(|value| (value.min, value.max))
                .unwrap_or_else(|| bounds_as_f64(collider.bounds));
            let Some(distance) = ray_aabb_distance(
                query.origin,
                direction,
                query.max_distance,
                bounds.0,
                bounds.1,
            ) else {
                continue;
            };
            let candidate = SpatialOcclusionHit::Entity {
                entity: collider.entity,
                point: point_at(query.origin, direction, distance),
                distance,
            };
            if nearest.is_none_or(|current| hit_precedes(candidate, current)) {
                nearest = Some(candidate);
            }
        }
        Ok(nearest)
    }
}

fn validate_and_normalize(
    query: SpatialOcclusionQuery<'_>,
) -> Result<[f64; 3], SpatialOcclusionError> {
    if !query.origin.iter().all(|value| value.is_finite()) {
        return Err(SpatialOcclusionError::InvalidOrigin);
    }
    if !query.direction.iter().all(|value| value.is_finite()) {
        return Err(SpatialOcclusionError::InvalidDirection);
    }
    if !query.max_distance.is_finite() || query.max_distance <= 0.0 {
        return Err(SpatialOcclusionError::InvalidMaxDistance);
    }
    let length_squared = query
        .direction
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    let length = length_squared.sqrt();
    if !length.is_finite() || length <= 0.0 {
        return Err(SpatialOcclusionError::InvalidDirection);
    }
    Ok(query.direction.map(|value| value / length))
}

fn bounds_as_f64(bounds: BoundsComponent) -> ([f64; 3], [f64; 3]) {
    let min = bounds.min.to_array().map(f64::from);
    let max = bounds.max.to_array().map(f64::from);
    (min, max)
}

fn ray_aabb_distance(
    origin: [f64; 3],
    direction: [f64; 3],
    max_distance: f64,
    min: [f64; 3],
    max: [f64; 3],
) -> Option<f64> {
    let mut near = 0.0_f64;
    let mut far = max_distance;
    for axis in 0..3 {
        if direction[axis] == 0.0 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let first = (min[axis] - origin[axis]) / direction[axis];
        let second = (max[axis] - origin[axis]) / direction[axis];
        near = near.max(first.min(second));
        far = far.min(first.max(second));
        if near > far {
            return None;
        }
    }
    Some(near)
}

fn point_at(origin: [f64; 3], direction: [f64; 3], distance: f64) -> [f64; 3] {
    [
        origin[0] + direction[0] * distance,
        origin[1] + direction[1] * distance,
        origin[2] + direction[2] * distance,
    ]
}

fn hit_precedes(candidate: SpatialOcclusionHit, current: SpatialOcclusionHit) -> bool {
    match candidate.distance().total_cmp(&current.distance()) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => tie_key(candidate) < tie_key(current),
    }
}

fn tie_key(hit: SpatialOcclusionHit) -> (u8, u64) {
    match hit {
        SpatialOcclusionHit::Entity { entity, .. } => (0, entity.raw()),
        SpatialOcclusionHit::Voxel(_) => (1, 0),
    }
}
