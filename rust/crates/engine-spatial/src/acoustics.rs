//! How sound meets a collision scene: the solid a straight line crosses
//! and how enclosed a point is. The open-air path round solid is the air
//! graph's ([`crate::AirGraph`]).
//!
//! These are geometric readings of the scene's collidable voxels and static
//! meshes; the audio model that turns them into level, filtering and reverb
//! lives with the audio service. Entity boxes take no part.

use core_space::{ChunkCoord, VoxelCoord, WorldPos, WorldVec};
use svc_collision::Ray;
use svc_volume::VoxelChunk;

use crate::VoxelCollisionScene;

/// How far an enclosure ray looks.
pub const ENCLOSURE_METRES: f64 = 24.0;
/// How far past a segment's end a solid measurement reads surfaces to learn
/// whether the segment starts inside a mesh.
const MESH_BEYOND: f64 = 64.0;

/// How enclosed a point is: the share of its level and upward rays that meet
/// collision within [`ENCLOSURE_METRES`], and their mean free length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enclosure {
    pub closed: f64,
    pub mean_free_metres: f64,
}

impl VoxelCollisionScene {
    /// Metres of collidable solid between `from` and `to`: collidable voxels
    /// and static-mesh interiors, where they overlap counted once.
    pub fn solid_metres(&self, from: [f64; 3], to: [f64; 3]) -> f64 {
        let Some((direction, length)) = unit(sub(to, from)) else {
            return 0.0;
        };
        let mut spans = Vec::new();
        self.voxel_spans(from, direction, length, false, &mut spans);
        self.mesh_spans(from, direction, length, &mut spans);
        union_length(&mut spans)
    }

    /// The stretches of collidable solid along a unit `direction` from
    /// `from` for `length` metres, as distances from `from`, nearest first
    /// and overlaps joined, added to `spans`.
    pub(crate) fn solid_spans(
        &self,
        from: [f64; 3],
        direction: [f64; 3],
        length: f64,
        spans: &mut Vec<(f64, f64)>,
    ) {
        let mut raw = Vec::new();
        self.voxel_spans(from, direction, length, false, &mut raw);
        self.mesh_spans(from, direction, length, &mut raw);
        raw.sort_by(|left, right| left.0.total_cmp(&right.0));
        for (start, end) in raw {
            match spans.last_mut() {
                Some(open) if start <= open.1 => open.1 = open.1.max(end),
                _ => spans.push((start, end)),
            }
        }
    }

    /// Whether the segment from `from` to `to` meets nothing collidable.
    pub fn segment_clear(&self, from: [f64; 3], to: [f64; 3]) -> bool {
        let Some((direction, length)) = unit(sub(to, from)) else {
            return true;
        };
        let mut spans = Vec::new();
        self.voxel_spans(from, direction, length, true, &mut spans);
        spans.is_empty()
            && self
                .static_mesh_projection()
                .raycast(ray(from, direction), length)
                .is_none()
    }

    /// How enclosed `at` is, from thirteen rays: straight up, eight level
    /// bearings and four rising ones. Ground below does not count.
    pub fn enclosure(&self, at: [f64; 3]) -> Enclosure {
        const LEVEL: f64 = std::f64::consts::FRAC_1_SQRT_2;
        let mut directions = vec![[0.0, 1.0, 0.0]];
        for step in 0..8 {
            let angle = f64::from(step) * std::f64::consts::FRAC_PI_4;
            directions.push([angle.cos(), 0.0, angle.sin()]);
        }
        for step in 0..4 {
            let angle = (f64::from(step) + 0.5) * std::f64::consts::FRAC_PI_2;
            directions.push([angle.cos() * LEVEL, LEVEL, angle.sin() * LEVEL]);
        }
        let mut hits = 0_u32;
        let mut free = 0.0;
        for direction in &directions {
            match self.raycast_world(at, *direction, ENCLOSURE_METRES) {
                Some(hit) => {
                    hits += 1;
                    free += hit_distance(&hit);
                }
                None => free += ENCLOSURE_METRES,
            }
        }
        let rays = directions.len() as f64;
        Enclosure {
            closed: f64::from(hits) / rays,
            mean_free_metres: free / rays,
        }
    }

    /// Spans of collidable voxels along the ray, as distances from `from`,
    /// walked voxel by voxel. `first` stops at the first one.
    fn voxel_spans(
        &self,
        from: [f64; 3],
        direction: [f64; 3],
        length: f64,
        first: bool,
        spans: &mut Vec<(f64, f64)>,
    ) {
        let grid = self.voxel_world.grid();
        let size = self.voxel_size;
        let start = grid.world_to_voxel(WorldPos::new(from[0], from[1], from[2]));
        let corner = grid.voxel_min_world(start);
        let corner = [corner.x, corner.y, corner.z];
        let mut cell = [start.x, start.y, start.z];
        let mut step = [0_i64; 3];
        let mut next = [f64::INFINITY; 3];
        let mut delta = [f64::INFINITY; 3];
        for axis in 0..3 {
            if direction[axis] > 0.0 {
                step[axis] = 1;
                next[axis] = (corner[axis] + size - from[axis]) / direction[axis];
                delta[axis] = size / direction[axis];
            } else if direction[axis] < 0.0 {
                step[axis] = -1;
                next[axis] = (corner[axis] - from[axis]) / direction[axis];
                delta[axis] = -size / direction[axis];
            }
        }
        let mut chunk: Option<(ChunkCoord, Option<&VoxelChunk>)> = None;
        let mut entered: Option<f64> = None;
        let mut at = 0.0;
        loop {
            let (coordinate, local) =
                grid.voxel_to_chunk_local(VoxelCoord::new(cell[0], cell[1], cell[2]));
            let resident = match chunk {
                Some((held, resident)) if held == coordinate => resident,
                _ => {
                    let resident = self.voxel_world.get(coordinate);
                    chunk = Some((coordinate, resident));
                    resident
                }
            };
            let solid = resident.is_some_and(|resident| {
                resident.get(local).is_some_and(|value| {
                    value.material().is_some_and(|material| {
                        !self.noncollidable_materials.contains(&material.raw())
                    })
                })
            });
            match (solid, entered) {
                (true, None) => {
                    entered = Some(at);
                    if first {
                        break;
                    }
                }
                (false, Some(start)) => {
                    spans.push((start, at));
                    entered = None;
                }
                _ => {}
            }
            let axis = if next[0] <= next[1] && next[0] <= next[2] {
                0
            } else if next[1] <= next[2] {
                1
            } else {
                2
            };
            if next[axis] >= length {
                break;
            }
            at = next[axis];
            cell[axis] += step[axis];
            next[axis] += delta[axis];
        }
        if let Some(start) = entered {
            spans.push((start, if first { start } else { length }));
        }
    }

    /// Spans inside static meshes along the ray, overlapping meshes as one.
    /// Each surface met adds one to a winding count going in (its triangle
    /// faces against the ray) and takes one off coming out; the ray is in
    /// solid wherever the count is above zero. The count the segment starts
    /// at is read from every surface on past its end to [`MESH_BEYOND`]
    /// metres, where the line is taken to be out of every mesh again, so a
    /// segment that starts in rock counts it.
    fn mesh_spans(
        &self,
        from: [f64; 3],
        direction: [f64; 3],
        length: f64,
        spans: &mut Vec<(f64, f64)>,
    ) {
        let surfaces = self
            .static_mesh_projection()
            .crossings(ray(from, direction), length + MESH_BEYOND);
        if surfaces.is_empty() {
            return;
        }
        // Out of every mesh past the last surface; never below zero inside.
        let total: i32 = surfaces.iter().map(|(_, turn)| *turn).sum();
        let lowest = surfaces
            .iter()
            .scan(0, |count, (_, turn)| {
                *count += turn;
                Some(*count)
            })
            .min()
            .unwrap_or(0);
        let mut count = (-total).max(-lowest).max(0);
        let mut entered = (count > 0).then_some(0.0);
        for (surface, turn) in surfaces {
            if surface >= length {
                break;
            }
            count += turn;
            match (entered, count > 0) {
                (None, true) => entered = Some(surface),
                (Some(start), false) => {
                    spans.push((start, surface));
                    entered = None;
                }
                _ => {}
            }
        }
        if let Some(start) = entered {
            spans.push((start, length));
        }
    }
}

fn hit_distance(hit: &crate::SpatialCollisionHit) -> f64 {
    match hit {
        crate::SpatialCollisionHit::Voxel(hit) => hit.distance,
        crate::SpatialCollisionHit::StaticMesh(hit) => hit.distance,
    }
}

fn ray(origin: [f64; 3], direction: [f64; 3]) -> Ray {
    Ray::new(
        WorldPos::new(origin[0], origin[1], origin[2]),
        WorldVec::new(direction[0], direction[1], direction[2]),
    )
}

fn union_length(spans: &mut [(f64, f64)]) -> f64 {
    spans.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut total = 0.0;
    let mut open: Option<(f64, f64)> = None;
    for &(start, end) in spans.iter() {
        match open {
            Some((from, to)) if start <= to => open = Some((from, to.max(end))),
            Some((from, to)) => {
                total += to - from;
                open = Some((start, end));
            }
            None => open = Some((start, end)),
        }
    }
    if let Some((from, to)) = open {
        total += to - from;
    }
    total
}

fn sub(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(value: [f64; 3], by: f64) -> [f64; 3] {
    value.map(|component| component * by)
}

fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    let difference = sub(left, right);
    (difference[0] * difference[0] + difference[1] * difference[1] + difference[2] * difference[2])
        .sqrt()
}

fn unit(value: [f64; 3]) -> Option<([f64; 3], f64)> {
    let length = distance(value, [0.0; 3]);
    (length.is_finite() && length > 0.0).then(|| (scale(value, 1.0 / length), length))
}

#[cfg(test)]
mod tests;
