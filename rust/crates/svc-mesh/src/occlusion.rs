//! Per-vertex ambient occlusion from voxel occupancy (#9506).
//!
//! Each vertex of a chunk's surface is darkened by the solid voxels around
//! it, at mesh time and from the lattice alone, so corners, ledges and
//! crevices read as such without a screen-space pass. A reconstructed
//! vertex looks out from its surface along a fan of directions over its
//! normal and counts the solid voxels it meets near and far; a cube face
//! corner takes the classic voxel rule from the three voxels beside it. Both
//! read absolute voxel positions and the same voxels on both sides of a
//! chunk seam, so neighbouring chunks give a shared vertex the same value
//! and a world-origin rebase changes none. The occlusion only shades the
//! surface: geometry, material slots and collision are unchanged.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use core_space::{ChunkCoord, Direction6, LocalVoxelCoord, VoxelCoord, VoxelGridSpec};
use svc_spatial::VoxelWorld;

/// How far beyond a chunk the samples read, in voxels: a vertex within a
/// voxel of the chunk looks `FAR` voxels out, plus a voxel of margin.
pub(crate) const REACH: i64 = 3;

/// The near and far sample distances along each direction, in voxels, and
/// the far samples' weight against the near ones'.
const NEAR: f64 = 0.75;
const FAR: f64 = 1.75;
const FAR_WEIGHT: f64 = 0.5;
/// How dark a cube corner closed on every side gets (level 0 of 3).
const CUBE_CLOSED_VISIBILITY: f32 = 0.35;
/// The fan of directions over the normal: straight out, and two rings of
/// eight around it, at these angles from the normal with these weights. The
/// outer ring lies near the surface, where a wall beside a floor stands.
const FAN: usize = 8;
const RINGS: [(f64, f64); 2] = [(45.0, 1.0), (75.0, 0.7)];

/// The fan in the vertex's own frame (tangent, bitangent, normal
/// coefficients) with each direction's weight, made once.
static DIRECTIONS: LazyLock<Vec<([f64; 3], f64)>> = LazyLock::new(|| {
    let mut directions = vec![([0.0, 0.0, 1.0], 1.0)];
    for (angle_from_normal, ring_weight) in RINGS {
        let (out, up) = angle_from_normal.to_radians().sin_cos();
        for step in 0..FAN {
            let angle = (step as f64 + 0.5) * std::f64::consts::TAU / FAN as f64;
            let (sin, cos) = angle.sin_cos();
            directions.push(([cos * out, sin * out, up], ring_weight));
        }
    }
    directions
});

/// The solid voxels around one chunk, and how strongly their occlusion
/// darkens the surface.
pub(crate) struct OcclusionField {
    low: [i64; 3],
    dims: [usize; 3],
    solid: Vec<bool>,
    strength: f32,
}

impl OcclusionField {
    /// The voxels within `REACH` of a chunk at `origin` of `size` voxels,
    /// solid where a material stands that is not see-through
    /// (`non_occluding`). Absent chunks read as empty, as they do for the
    /// surface.
    pub(crate) fn around_chunk(
        world: &VoxelWorld,
        spec: &VoxelGridSpec,
        non_occluding: &BTreeSet<u16>,
        origin: [i64; 3],
        size: [i64; 3],
        strength: f32,
    ) -> Self {
        let low = origin.map(|value| value - REACH);
        let dims: [usize; 3] = std::array::from_fn(|axis| (size[axis] + 2 * REACH) as usize);
        let mut solid = vec![false; dims[0] * dims[1] * dims[2]];
        let high: [i64; 3] = std::array::from_fn(|axis| low[axis] + dims[axis] as i64 - 1);
        let first = spec.voxel_to_chunk(VoxelCoord::new(low[0], low[1], low[2]));
        let last = spec.voxel_to_chunk(VoxelCoord::new(high[0], high[1], high[2]));
        let extent = spec.chunk_dims().to_array().map(i64::from);
        for cz in first.z..=last.z {
            for cy in first.y..=last.y {
                for cx in first.x..=last.x {
                    let coord = ChunkCoord::new(cx, cy, cz);
                    let Some(chunk) = world.get(coord) else {
                        continue;
                    };
                    let chunk_origin = spec.chunk_origin_voxel(coord).to_array();
                    let from: [i64; 3] =
                        std::array::from_fn(|axis| low[axis].max(chunk_origin[axis]));
                    let to: [i64; 3] = std::array::from_fn(|axis| {
                        high[axis].min(chunk_origin[axis] + extent[axis] - 1)
                    });
                    for z in from[2]..=to[2] {
                        for y in from[1]..=to[1] {
                            for x in from[0]..=to[0] {
                                let local = LocalVoxelCoord::new(
                                    (x - chunk_origin[0]) as u32,
                                    (y - chunk_origin[1]) as u32,
                                    (z - chunk_origin[2]) as u32,
                                );
                                let occludes = chunk
                                    .get(local)
                                    .and_then(|value| value.material())
                                    .is_some_and(|material| {
                                        !non_occluding.contains(&material.raw())
                                    });
                                if occludes {
                                    let index = ((z - low[2]) as usize * dims[1]
                                        + (y - low[1]) as usize)
                                        * dims[0]
                                        + (x - low[0]) as usize;
                                    solid[index] = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        Self {
            low,
            dims,
            solid,
            strength,
        }
    }

    fn solid(&self, voxel: [i64; 3]) -> bool {
        let mut index = 0;
        for axis in [2, 1, 0] {
            let at = voxel[axis] - self.low[axis];
            if at < 0 || at >= self.dims[axis] as i64 {
                return false;
            }
            index = index * self.dims[axis] + at as usize;
        }
        self.solid[index]
    }

    /// The occlusion of a reconstructed vertex at `point` (lattice units,
    /// voxel `c` spanning `c..c + 1`) with outward `normal`, baked with the
    /// strength: 1 in the open, darker where the fan over the normal meets
    /// solid voxels, nearer ones weighing more.
    pub(crate) fn surface_occlusion(&self, point: [f64; 3], normal: [f64; 3]) -> f32 {
        let normal = normalize_or(normal, [0.0, 1.0, 0.0]);
        let seed = if normal[1].abs() < 0.9 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let tangent = normalize_or(cross(normal, seed), [1.0, 0.0, 0.0]);
        let bitangent = cross(normal, tangent);
        let (mut blocked, mut total) = (0.0, 0.0);
        for ([along_tangent, along_bitangent, along_normal], ring_weight) in DIRECTIONS.iter() {
            let direction: [f64; 3] = std::array::from_fn(|axis| {
                tangent[axis] * along_tangent
                    + bitangent[axis] * along_bitangent
                    + normal[axis] * along_normal
            });
            for (distance, distance_weight) in [(NEAR, 1.0), (FAR, FAR_WEIGHT)] {
                let sample: [i64; 3] = std::array::from_fn(|axis| {
                    (point[axis] + direction[axis] * distance).floor() as i64
                });
                let weight = ring_weight * distance_weight;
                total += weight;
                if self.solid(sample) {
                    blocked += weight;
                }
            }
        }
        self.bake((1.0 - blocked / total) as f32)
    }

    /// The occlusion levels (0 closed to 3 open) of a cube face's four
    /// corners, in the order `quad_corners` emits them: for each corner the
    /// two voxels beside it across the face's plane and the one diagonal
    /// (the classic rule: both sides solid closes the corner).
    pub(crate) fn cube_corner_levels(&self, voxel: VoxelCoord, dir: Direction6) -> [u8; 4] {
        let offset = dir.offset();
        let above = [
            voxel.x + i64::from(offset[0]),
            voxel.y + i64::from(offset[1]),
            voxel.z + i64::from(offset[2]),
        ];
        let (u_axis, v_axis) = crate::in_plane_axes(dir);
        let level = |su: i64, sv: i64| {
            let mut side_u = above;
            side_u[u_axis] += su;
            let mut side_v = above;
            side_v[v_axis] += sv;
            let mut corner = above;
            corner[u_axis] += su;
            corner[v_axis] += sv;
            let (side_u, side_v, corner) =
                (self.solid(side_u), self.solid(side_v), self.solid(corner));
            if side_u && side_v {
                0
            } else {
                3 - u8::from(side_u) - u8::from(side_v) - u8::from(corner)
            }
        };
        let mut levels = [level(-1, -1), level(1, -1), level(1, 1), level(-1, 1)];
        if !dir.is_positive() {
            levels.swap(1, 3);
        }
        levels
    }

    /// A cube corner's occlusion from its level, baked with the strength.
    pub(crate) fn cube_occlusion(&self, level: u8) -> f32 {
        self.bake(CUBE_CLOSED_VISIBILITY + (1.0 - CUBE_CLOSED_VISIBILITY) * f32::from(level) / 3.0)
    }

    fn bake(&self, visibility: f32) -> f32 {
        1.0 - self.strength * (1.0 - visibility.clamp(0.0, 1.0))
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize_or(vector: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if length > 1e-9 {
        vector.map(|value| value / length)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mesh_chunk_in_world_with_options, SurfaceMeshOptions, SurfaceMode};
    use core_voxel::VoxelValue;
    use svc_volume::VoxelChunk;

    fn spec() -> VoxelGridSpec {
        VoxelGridSpec::new(
            core_space::GridId::new(0),
            1.0,
            core_space::ChunkDims::cubic(8).unwrap(),
        )
        .unwrap()
    }

    /// A floor across two chunks with a wall rising along their far side: the
    /// reconstructed vertices on the seam get the same occlusion from both
    /// chunks, the floor against the wall is darker than the open floor,
    /// and nothing moves (cube faces merge into more quads over the same
    /// surface).
    #[test]
    fn occlusion_agrees_across_a_chunk_seam_and_darkens_the_corner() {
        for mode in [SurfaceMode::DualContouring, SurfaceMode::GreedyCubes] {
            let mut world = VoxelWorld::new(spec());
            for cx in 0..2 {
                let mut chunk = VoxelChunk::from_spec(&spec());
                for z in 0..8 {
                    for x in 0..8 {
                        chunk
                            .set(LocalVoxelCoord::new(x, 0, z), VoxelValue::solid_raw(1))
                            .unwrap();
                        // The wall along z = 7, two voxels high.
                        for y in 1..3 {
                            chunk
                                .set(LocalVoxelCoord::new(x, y, 7), VoxelValue::solid_raw(1))
                                .unwrap();
                        }
                    }
                }
                world.insert(ChunkCoord::new(cx, 0, 0), chunk);
            }
            world.drain_dirty();
            let render = |strength: f32| {
                let options = SurfaceMeshOptions {
                    mode,
                    vertex_occlusion: strength,
                    ..SurfaceMeshOptions::default()
                };
                [0, 1].map(|cx| {
                    mesh_chunk_in_world_with_options(&world, ChunkCoord::new(cx, 0, 0), &options)
                        .unwrap()
                        .unwrap()
                })
            };
            let plain = render(0.0);
            let occluded = render(1.0);
            for (plain, occluded) in plain.iter().zip(&occluded) {
                assert!(plain.occlusion.is_empty(), "{mode:?}: off emits none");
                assert_eq!(
                    occluded.occlusion.len(),
                    occluded.positions.len() / 3,
                    "{mode:?}: one per vertex"
                );
                if mode == SurfaceMode::GreedyCubes {
                    // Faces with different corners merge into separate
                    // quads, so there are more, over the same surface.
                    assert!(
                        occluded.indices.len() >= plain.indices.len(),
                        "{mode:?}: the same faces in more quads"
                    );
                } else {
                    assert_eq!(
                        plain.positions, occluded.positions,
                        "{mode:?}: geometry unchanged"
                    );
                    assert_eq!(
                        plain.indices, occluded.indices,
                        "{mode:?}: topology unchanged"
                    );
                }
            }
            // Vertices of the floor top (y 1) by absolute position: the seam
            // (x 8) from both chunks, the open middle and the wall foot.
            let mut at_seam: Vec<(i64, i64, f32)> = Vec::new();
            let (mut open, mut corner) = (Vec::new(), Vec::new());
            for (cx, chunk) in occluded.iter().enumerate() {
                for (vertex, occlusion) in chunk.occlusion.iter().enumerate() {
                    let position = &chunk.positions[vertex * 3..vertex * 3 + 3];
                    let normal = &chunk.normals[vertex * 3..vertex * 3 + 3];
                    if (position[1] - 1.0).abs() > 0.3 || normal[1] < 0.5 {
                        continue;
                    }
                    let x = position[0] as f64 + cx as f64 * 8.0;
                    let z = position[2] as f64;
                    if (x - 8.0).abs() < 1e-3 {
                        at_seam.push(((z * 1000.0) as i64, cx as i64, *occlusion));
                    }
                    if z < 5.0 {
                        open.push(*occlusion);
                    }
                    if z > 6.4 {
                        corner.push(*occlusion);
                    }
                }
            }
            // A reconstructed vertex on the seam is one vertex to both
            // chunks; cube quads keep their own corners, each from its own
            // face's side (the classic rule), so only reconstruction is
            // compared here.
            at_seam.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            if mode == SurfaceMode::GreedyCubes {
                at_seam.clear();
            } else {
                assert!(!at_seam.is_empty(), "{mode:?}: seam vertices found");
            }
            for pair in at_seam.chunk_by(|a, b| a.0 == b.0) {
                let values: Vec<f32> = pair.iter().map(|entry| entry.2).collect();
                assert!(
                    values.iter().all(|value| (value - values[0]).abs() < 1e-6),
                    "{mode:?}: seam vertex at z {} agrees: {pair:?}",
                    pair[0].0
                );
            }
            assert!(
                !open.is_empty() && !corner.is_empty(),
                "{mode:?}: samples found"
            );
            let mean = |values: &[f32]| values.iter().sum::<f32>() / values.len() as f32;
            assert!(
                mean(&open) > 0.95,
                "{mode:?}: the open floor stays lit: {}",
                mean(&open)
            );
            assert!(
                mean(&corner) < mean(&open) - 0.1,
                "{mode:?}: the wall's foot darkens: {} against {}",
                mean(&corner),
                mean(&open)
            );
        }
    }

    /// The classic cube rule: a corner with both sides solid is closed, one
    /// side open counts the diagonal, nothing solid is open.
    #[test]
    fn cube_corner_levels_follow_the_classic_rule() {
        let mut world = VoxelWorld::new(spec());
        let mut chunk = VoxelChunk::from_spec(&spec());
        // A floor voxel at (3,0,3) with a wall voxel at (4,1,3) beside its
        // top face's +x edge and a post at (2,1,2) diagonal to its -x,-z corner.
        chunk
            .set(LocalVoxelCoord::new(3, 0, 3), VoxelValue::solid_raw(1))
            .unwrap();
        chunk
            .set(LocalVoxelCoord::new(4, 1, 3), VoxelValue::solid_raw(1))
            .unwrap();
        chunk
            .set(LocalVoxelCoord::new(2, 1, 2), VoxelValue::solid_raw(1))
            .unwrap();
        world.insert(ChunkCoord::new(0, 0, 0), chunk);
        world.drain_dirty();
        let field =
            OcclusionField::around_chunk(&world, &spec(), &BTreeSet::new(), [0; 3], [8; 3], 1.0);
        // The top face (+y): u is z, v is x (`in_plane_axes`), corners in
        // loop order (-z,-x), (+z,-x), (+z,+x), (-z,+x).
        let levels = field.cube_corner_levels(VoxelCoord::new(3, 0, 3), Direction6::PosY);
        assert_eq!(levels, [2, 3, 2, 2]);
        assert!(field.cube_occlusion(0) < field.cube_occlusion(3));
        assert_eq!(field.cube_occlusion(3), 1.0);
    }
}
