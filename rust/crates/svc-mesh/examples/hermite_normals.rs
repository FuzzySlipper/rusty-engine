//! What stored crossing normals (#9504) change for a sharp cut.
//!
//! Run with:
//! `cargo run --release -p svc-mesh --example hermite_normals`
//!
//! A slab of rock (2 × 2 × 2 chunks of 16³ one-metre voxels, its top at
//! 10.3 m) has a box cut out of it, once axis-aligned and once turned 30
//! degrees about y, with exact signed densities. Each case is meshed at Sharp
//! placement twice: with the trilinear normals estimated from each cell's
//! densities, and with the exact normal of the cut at every crossing stored
//! on the chunks (their place along the edge and their normal). The report gives how far the vertices lie from the true
//! surface (the CSG distance at each vertex), over the vertices of the
//! slab's top and the cut (not its outer walls at the world's edge) and over
//! those within a voxel of the cut's edges, the stored normals' count and
//! memory, and the meshing time.

use std::time::{Duration, Instant};

use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_mesh::{
    mesh_chunk_in_world_with_options, MaterialSurface, SurfaceCharacter, SurfaceMaterials,
    SurfaceMeshOptions, SurfaceMode, VertexPlacement,
};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

const CHUNK: u32 = 16;
const CHUNKS: i64 = 2;
const ROCK: u16 = 1;
const GROUND: f64 = 10.3;
const ROUNDS: usize = 5;

/// The cut: a box of half extents `half` about `centre`, turned `yaw` about y.
#[derive(Clone, Copy)]
struct Cut {
    centre: [f64; 3],
    half: [f64; 3],
    yaw: f64,
}

impl Cut {
    fn local(&self, point: [f64; 3]) -> [f64; 3] {
        let (sin, cos) = self.yaw.sin_cos();
        let dx = point[0] - self.centre[0];
        let dz = point[2] - self.centre[2];
        [
            cos * dx + sin * dz,
            point[1] - self.centre[1],
            -sin * dx + cos * dz,
        ]
    }

    /// The box's signed distance and its gradient (world frame).
    fn distance(&self, point: [f64; 3]) -> (f64, [f64; 3]) {
        let local = self.local(point);
        let q: [f64; 3] = std::array::from_fn(|axis| local[axis].abs() - self.half[axis]);
        let outside: [f64; 3] = q.map(|value| value.max(0.0));
        let length =
            (outside[0] * outside[0] + outside[1] * outside[1] + outside[2] * outside[2]).sqrt();
        let (distance, local_gradient) = if length > 0.0 {
            (
                length,
                std::array::from_fn(|axis| outside[axis] / length * local[axis].signum()),
            )
        } else {
            let axis = (0..3).max_by(|a, b| q[*a].total_cmp(&q[*b])).unwrap();
            let mut gradient = [0.0; 3];
            gradient[axis] = local[axis].signum();
            (q[axis], gradient)
        };
        let (sin, cos) = self.yaw.sin_cos();
        let gradient = [
            cos * local_gradient[0] - sin * local_gradient[2],
            local_gradient[1],
            sin * local_gradient[0] + cos * local_gradient[2],
        ];
        (distance, gradient)
    }

    /// The carved slab: ground below GROUND minus the box. Negative inside
    /// the rock; the gradient points out of it.
    fn rock(&self, point: [f64; 3]) -> (f64, [f64; 3]) {
        let ground = (point[1] - GROUND, [0.0, 1.0, 0.0]);
        let (inside, gradient) = self.distance(point);
        let carved = (-inside, gradient.map(|value| -value));
        if ground.0 >= carved.0 {
            ground
        } else {
            carved
        }
    }
}

fn world(cut: Cut) -> (VoxelWorld, usize) {
    let dims = ChunkDims::cubic(CHUNK).unwrap();
    let grid = VoxelGridSpec::new(GridId::new(0), 1.0, dims).unwrap();
    let mut world = VoxelWorld::new(grid);
    let centre = |voxel: [i64; 3]| voxel.map(|value| value as f64 + 0.5);
    for cx in 0..CHUNKS {
        for cy in 0..CHUNKS {
            for cz in 0..CHUNKS {
                let coord = ChunkCoord::new(cx, cy, cz);
                let mut chunk = VoxelChunk::from_spec(&grid);
                for x in 0..CHUNK {
                    for y in 0..CHUNK {
                        for z in 0..CHUNK {
                            let local = LocalVoxelCoord::new(x, y, z);
                            let voxel = grid.chunk_local_to_voxel(coord, local).to_array();
                            let (density, _) = cut.rock(centre(voxel));
                            if density < 0.0 {
                                chunk
                                    .set(local, VoxelValue::solid(VoxelMaterialId::new(ROCK)))
                                    .unwrap();
                            }
                            chunk.set_density(local, density as f32).unwrap();
                            // The exact normal where the cut crosses each of
                            // the voxel's +x, +y and +z edges.
                            for axis in 0..3 {
                                let mut next = voxel;
                                next[axis] += 1;
                                let (other, _) = cut.rock(centre(next));
                                if (density < 0.0) == (other < 0.0) {
                                    continue;
                                }
                                let t = density / (density - other);
                                let mut at = centre(voxel);
                                at[axis] += t;
                                let (_, normal) = cut.rock(at);
                                chunk
                                    .set_edge_crossing(
                                        local,
                                        axis,
                                        Some(svc_volume::EdgeCrossing {
                                            at: t as f32,
                                            normal: normal.map(|v| v as f32),
                                        }),
                                    )
                                    .unwrap();
                            }
                        }
                    }
                }
                world.insert(coord, chunk);
            }
        }
    }
    let stored = world
        .resident_chunks()
        .map(|(_, chunk)| chunk.edge_crossing_count())
        .sum();
    (world, stored)
}

/// Mean and largest distance from the vertices to the true surface, over all
/// vertices and over those within a voxel of a cut edge; and the meshing time.
fn measure(world: &VoxelWorld, cut: Cut, ignore: bool) -> ([f64; 4], usize, Duration) {
    let options = SurfaceMeshOptions {
        mode: SurfaceMode::DualContouring,
        materials: SurfaceMaterials::new([(
            ROCK,
            MaterialSurface {
                mode: SurfaceMode::DualContouring,
                character: SurfaceCharacter {
                    placement: VertexPlacement::Sharp,
                    crease_angle_degrees: 40.0,
                    roughness: 0.0,
                },
            },
        )])
        .unwrap(),
        ignore_stored_normals: ignore,
        ..SurfaceMeshOptions::default()
    };
    let chunks: Vec<ChunkCoord> = world.resident_chunks().map(|(coord, _)| coord).collect();
    let mut best = Duration::MAX;
    let mut errors = Vec::new();
    let mut near_edges = Vec::new();
    for round in 0..ROUNDS {
        let started = Instant::now();
        let meshes: Vec<_> = chunks
            .iter()
            .map(|coord| {
                (
                    *coord,
                    mesh_chunk_in_world_with_options(world, *coord, &options)
                        .unwrap()
                        .unwrap(),
                )
            })
            .collect();
        best = best.min(started.elapsed());
        if round > 0 {
            continue;
        }
        for (coord, mesh) in meshes {
            let origin = [coord.x, coord.y, coord.z].map(|value| (value * i64::from(CHUNK)) as f64);
            for vertex in mesh.positions.chunks_exact(3) {
                let point: [f64; 3] =
                    std::array::from_fn(|axis| origin[axis] + f64::from(vertex[axis]));
                let error = cut.rock(point).0.abs();
                // The slab's own sides and floor at the edge of the world are
                // not the cut's surface.
                let world_edge = f64::from(CHUNK) * CHUNKS as f64 - 1.0;
                if point[1] > 2.0
                    && (0..3)
                        .step_by(2)
                        .all(|axis| point[axis] > 1.0 && point[axis] < world_edge)
                {
                    errors.push(error);
                }
                // Near an edge: two of the cut's faces (or a face and the
                // ground) lie within a voxel.
                let local = cut.local(point);
                let near: usize = (0..3)
                    .filter(|axis| (local[*axis].abs() - cut.half[*axis]).abs() < 1.0)
                    .count()
                    + usize::from((point[1] - GROUND).abs() < 1.0);
                let inside_footprint = (0..3).all(|axis| local[axis].abs() < cut.half[axis] + 1.0);
                if near >= 2 && inside_footprint {
                    near_edges.push(error);
                }
            }
        }
    }
    let stats = |values: &[f64]| {
        let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
        let max = values.iter().copied().fold(0.0, f64::max);
        (mean, max)
    };
    let (mean, max) = stats(&errors);
    let (edge_mean, edge_max) = stats(&near_edges);
    ([mean, max, edge_mean, edge_max], near_edges.len(), best)
}

fn main() {
    for (name, cut) in [
        (
            "aligned box",
            Cut {
                centre: [16.0, 10.3, 16.0],
                half: [6.3, 4.2, 4.7],
                yaw: 0.0,
            },
        ),
        (
            "box turned 30°",
            Cut {
                centre: [16.0, 10.3, 16.0],
                half: [6.3, 4.2, 4.7],
                yaw: 30f64.to_radians(),
            },
        ),
    ] {
        let (world, stored) = world(cut);
        let (estimated, edge_vertices, estimated_time) = measure(&world, cut, true);
        let (exact, _, exact_time) = measure(&world, cut, false);
        // 4 bytes of key and 8 of packed crossing, in a B-tree map of
        // about 20 bytes an entry.
        println!(
            "{name}: {stored} stored crossings, about {} bytes per chunk",
            stored * 20 / 8
        );
        for (label, values, time) in [
            ("trilinear", estimated, estimated_time),
            ("stored", exact, exact_time),
        ] {
            println!(
                "  {label:>9}: vertex error mean {:.3} max {:.3} voxels; near edges ({edge_vertices}) mean {:.3} max {:.3}; meshing {:.2} ms for 8 chunks",
                values[0],
                values[1],
                values[2],
                values[3],
                time.as_secs_f64() * 1000.0
            );
        }
    }
}
