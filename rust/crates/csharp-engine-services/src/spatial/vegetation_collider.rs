//! Vegetation colliders (#10951): the copies of a vegetation layer that has
//! a collider, on tiles within its reach of the field's focus, stand in the
//! field's session as upright cylinders. They ride deferred collision batches
//! (`static_deferred`) as heightfield collider tiles do, so movement,
//! queries, contacts, Dynamics and navigation read them as any static
//! collision, and a world-origin rebase moves them with the rest. Which
//! copies are wanted is `render_projection::VegetationFields::colliders`.

use std::collections::BTreeMap;

use engine_spatial::{
    StaticMeshAssetId, StaticMeshColliderInstance, StaticMeshInstanceId, StaticMeshTransform,
};
use render_projection::VegetationColliderTile;

use super::{static_deferred, SpatialSession};

/// Vegetation collision takes its ids from here up, past heightfield
/// colliders' tiles: the cylinder first, then a copy each.
const CYLINDER: u64 = 0xF000_0000_0000_0000;
/// The cylinder's sides: enough that a body sliding round a trunk meets no
/// corner it would catch on.
const SIDES: u32 = 8;

/// A field's tile layer: the field, the layer's key and the tile.
type Key = (u64, u32, (i64, i64));

/// A session's vegetation colliders: each tile layer's, with the stamp of
/// the placement they stand for and their copies' instance ids.
#[derive(Default)]
pub(super) struct SessionVegetation {
    tiles: BTreeMap<Key, (u64, Vec<u64>)>,
    next: u64,
}

/// The upright cylinder of radius 1 and height 1 standing on its foot at
/// the origin, closed at both ends, that every copy is a scaled instance of:
/// a prism whose faces stand at the radius, one facing each axis.
fn cylinder() -> static_deferred::CopiedAsset {
    let corner = 1.0 / (std::f64::consts::PI / f64::from(SIDES)).cos();
    let mut positions = Vec::with_capacity(SIDES as usize * 2);
    for side in 0..SIDES {
        let angle = std::f64::consts::TAU * (f64::from(side) + 0.5) / f64::from(SIDES);
        let (sin, cos) = angle.sin_cos();
        positions.push([cos * corner, 0.0, sin * corner]);
        positions.push([cos * corner, 1.0, sin * corner]);
    }
    let mut triangles = Vec::new();
    for side in 0..SIDES {
        let [bottom, top] = [2 * side, 2 * side + 1];
        let next = 2 * ((side + 1) % SIDES);
        // Outward, counter-clockwise seen from outside.
        triangles.push([bottom, top, next]);
        triangles.push([next, top, next + 1]);
    }
    for side in 1..SIDES - 1 {
        triangles.push([0, 2 * side, 2 * (side + 1)]);
        triangles.push([1, 2 * (side + 1) + 1, 2 * side + 1]);
    }
    (CYLINDER, positions, triangles)
}

impl SpatialSession {
    /// Whether no deferred collision or content batch waits, for tests.
    #[cfg(test)]
    pub(crate) fn static_deferred_is_empty(&self) -> bool {
        self.static_deferred.is_empty()
    }

    /// Stand a field's wanted colliders in the session: tile layers newly
    /// wanted, or placed again, go in, and those no longer wanted go, in one
    /// deferred batch. `origin` is where the field's (0, 0, 0) stands from
    /// the world's origin, `scale` the metres a field unit spans; none
    /// wanted takes the field's colliders out.
    pub(crate) fn set_vegetation_colliders(
        &mut self,
        field: u64,
        origin: [f64; 3],
        scale: f64,
        wanted: &[((u32, (i64, i64)), VegetationColliderTile)],
    ) {
        let wanted: BTreeMap<Key, &VegetationColliderTile> = wanted
            .iter()
            .map(|((layer, tile), colliders)| ((field, *layer, *tile), colliders))
            .collect();
        let mut batch = static_deferred::StaticBatch {
            assets: Vec::new(),
            instances: Vec::new(),
            removed_assets: Vec::new(),
            removed_instances: Vec::new(),
        };
        let stands = &mut self.vegetation_colliders;
        let leaving: Vec<Key> = stands
            .tiles
            .range((field, 0, (i64::MIN, i64::MIN))..=(field, u32::MAX, (i64::MAX, i64::MAX)))
            .filter(|(key, (stamp, _))| wanted.get(key).is_none_or(|tile| tile.stamp != *stamp))
            .map(|(key, _)| *key)
            .collect();
        for key in leaving {
            let (_, ids) = stands.tiles.remove(&key).expect("listed above");
            batch
                .removed_instances
                .extend(ids.into_iter().map(StaticMeshInstanceId));
        }
        let frame = self.world_origin.origin().cell();
        let place: [f64; 3] = std::array::from_fn(|axis| origin[axis] - frame[axis] as f64);
        for (key, tile) in wanted {
            if stands.tiles.contains_key(&key) {
                continue;
            }
            let collider = tile.collider;
            let mut ids = Vec::with_capacity(tile.copies.len());
            for copy in tile.copies.iter() {
                stands.next += 1;
                let id = CYLINDER + stands.next;
                ids.push(id);
                let size = f64::from(copy.scale) * scale;
                let [x, y, z] = copy.foot.map(f64::from);
                batch.instances.push(StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(id),
                    asset: StaticMeshAssetId(CYLINDER),
                    transform: StaticMeshTransform {
                        translation: [
                            place[0] + (tile.origin[0] + x) * scale,
                            place[1] + y * scale,
                            place[2] + (tile.origin[1] + z) * scale,
                        ],
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        scale: [
                            f64::from(collider.radius) * size,
                            f64::from(collider.height) * size,
                            f64::from(collider.radius) * size,
                        ],
                    },
                });
            }
            stands.tiles.insert(key, (tile.stamp, ids));
        }
        if batch.instances.is_empty() && batch.removed_instances.is_empty() {
            return;
        }
        // The cylinder goes with every batch that stands copies: already
        // resident, it is kept as it is.
        if !batch.instances.is_empty() {
            batch.assets.push(cylinder());
        }
        self.static_deferred
            .push(static_deferred::DeferredBatch::Collision(batch));
        self.start_deferred();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cylinder_is_closed_and_faces_out() {
        let (_, positions, triangles) = cylinder();
        // Every edge is shared by two triangles, once each way: closed.
        let mut edges = BTreeMap::new();
        for [a, b, c] in &triangles {
            for edge in [(*a, *b), (*b, *c), (*c, *a)] {
                *edges.entry(edge).or_insert(0) += 1;
            }
        }
        assert!(edges
            .iter()
            .all(|(&(a, b), &count)| count == 1 && edges.get(&(b, a)) == Some(&1)));
        // Each side's normal points away from the axis.
        for [a, b, c] in &triangles {
            let [p, q, r] = [
                positions[*a as usize],
                positions[*b as usize],
                positions[*c as usize],
            ];
            let u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
            let v = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
            let normal = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let centre = [
                (p[0] + q[0] + r[0]) / 3.0,
                (p[1] + q[1] + r[1]) / 3.0 - 0.5,
                (p[2] + q[2] + r[2]) / 3.0,
            ];
            let outward = normal[0] * centre[0] + normal[1] * centre[1] + normal[2] * centre[2];
            assert!(outward > 0.0);
        }
    }
}
