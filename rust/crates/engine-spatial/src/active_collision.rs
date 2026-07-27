use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{BoundsComponent, EntityState};

/// One active entity collider expressed in the same translation-offset AABB
/// used by [`crate::EntityMotionService`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ActiveEntityCollider {
    pub entity: EntityId,
    pub bounds: BoundsComponent,
}

pub(crate) fn active_entity_colliders(
    state: &EntityState,
) -> impl Iterator<Item = ActiveEntityCollider> + '_ {
    state
        .entities()
        .filter_map(|core| active_entity_collider(state, core.id))
}

fn active_entity_collider(state: &EntityState, entity: EntityId) -> Option<ActiveEntityCollider> {
    state.active_collision(entity)?;
    let bounds = *state.bounds(entity)?;
    let translation = state.world_transform(entity)?.translation;
    Some(ActiveEntityCollider {
        entity,
        bounds: offset_bounds(bounds, translation),
    })
}

fn offset_bounds(bounds: BoundsComponent, origin: Vec3) -> BoundsComponent {
    BoundsComponent {
        min: bounds.min + origin,
        max: bounds.max + origin,
    }
}
