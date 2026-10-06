//! Merged faces of exact blocks: dual-contoured Blocky materials without
//! roughness.

use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_mesh::{
    mesh_chunk_in_world_with_options, MaterialSurface, MeshPayload, SurfaceCharacter,
    SurfaceMaterials, SurfaceMeshOptions, SurfaceMode, VertexPlacement,
};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

const BLOCK: u16 = 1;
const ROCK: u16 = 2;

fn options(placement: VertexPlacement) -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        mode: SurfaceMode::DualContouring,
        materials: SurfaceMaterials::new([
            (
                BLOCK,
                MaterialSurface {
                    mode: SurfaceMode::DualContouring,
                    character: SurfaceCharacter {
                        placement,
                        crease_angle_degrees: 0.0,
                        roughness: 0.0,
                    },
                },
            ),
            (
                ROCK,
                MaterialSurface {
                    mode: SurfaceMode::DualContouring,
                    character: SurfaceCharacter {
                        placement: VertexPlacement::Sharp,
                        crease_angle_degrees: 60.0,
                        roughness: 0.0,
                    },
                },
            ),
        ])
        .unwrap(),
        ..SurfaceMeshOptions::default()
    }
}

/// One 8³ chunk and its neighbours, solid where `fill` says.
fn world(fill: impl Fn([i64; 3]) -> Option<u16>) -> VoxelWorld {
    let grid = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap()).unwrap();
    let mut world = VoxelWorld::new(grid);
    for (cx, cy, cz) in
        (-1..=1).flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| (x, y, z))))
    {
        let coord = ChunkCoord::new(cx, cy, cz);
        let mut chunk = VoxelChunk::from_spec(&grid);
        for (x, y, z) in
            (0..8).flat_map(|x| (0..8).flat_map(move |y| (0..8).map(move |z| (x, y, z))))
        {
            let local = LocalVoxelCoord::new(x, y, z);
            if let Some(slot) = fill(grid.chunk_local_to_voxel(coord, local).to_array()) {
                chunk
                    .set(local, VoxelValue::solid(VoxelMaterialId::new(slot)))
                    .unwrap();
            }
        }
        world.insert(coord, chunk);
    }
    world
}

fn mesh(world: &VoxelWorld, options: &SurfaceMeshOptions) -> MeshPayload {
    mesh_chunk_in_world_with_options(world, ChunkCoord::new(0, 0, 0), options)
        .unwrap()
        .unwrap()
}

/// Area covered by the triangles facing up at `height`.
fn floor_area(mesh: &MeshPayload, height: f32) -> f32 {
    mesh.indices
        .chunks(3)
        .map(|triangle| [0, 1, 2].map(|k| &mesh.positions[triangle[k] as usize * 3..][..3]))
        .filter(|[a, b, c]| a[1] == height && b[1] == height && c[1] == height)
        .map(|[a, b, c]| {
            ((b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0])).abs() / 2.0
        })
        .sum()
}

#[test]
fn a_blocky_floor_merges_into_rectangles_owned_by_the_voxels_under_them() {
    let world = world(|[_, y, _]| (y < 3).then_some(BLOCK));
    let sharp = mesh(&world, &options(VertexPlacement::Sharp));
    let blocky = mesh(&world, &options(VertexPlacement::Blocky));
    // The same floor, drawn by far fewer triangles.
    assert_eq!(floor_area(&sharp, 3.0), 64.0);
    assert_eq!(floor_area(&blocky, 3.0), 64.0);
    assert_eq!(sharp.indices.len() / 3, 128);
    // The faces touching the chunk's border stay single, since the next
    // chunk's surfaces may share their corners; the 6 × 6 inside merge.
    assert_eq!(blocky.indices.len() / 3, 28 * 2 + 2);
    // Tile coordinates are still projected from absolute positions.
    for (position, tile) in blocky
        .positions
        .chunks(3)
        .zip(blocky.tile_coordinates.chunks(2))
    {
        if position[1] == 3.0 {
            let mut projected = [tile[0].abs(), tile[1].abs()];
            projected.sort_by(f32::total_cmp);
            let mut expected = [position[0], position[2]];
            expected.sort_by(f32::total_cmp);
            assert_eq!(projected, expected);
        }
    }
    // Each triangle is owned by the voxels under it.
    assert_eq!(blocky.triangle_owner_spans.len(), blocky.indices.len() / 3);
    let covered: u32 = blocky
        .triangle_owner_spans
        .iter()
        .zip(&blocky.triangle_owners)
        .map(|(span, owner)| {
            assert_eq!(owner[1], 2);
            assert_eq!(span[1], 1);
            span[0] * span[2]
        })
        .sum();
    // Two triangles per rectangle cover its cells.
    assert_eq!(covered, 2 * 64);
    assert!(sharp.triangle_owner_spans.is_empty());
}

/// Corners of `mesh`'s triangles.
fn corners(mesh: &MeshPayload, slot: u16) -> Vec<[[f32; 3]; 3]> {
    mesh.groups
        .iter()
        .filter(|group| group.material_slot == slot)
        .flat_map(|group| {
            mesh.indices[group.start as usize..(group.start + group.count) as usize]
                .chunks(3)
                .map(|triangle| {
                    [0, 1, 2].map(|k| {
                        let at = triangle[k] as usize * 3;
                        [
                            mesh.positions[at],
                            mesh.positions[at + 1],
                            mesh.positions[at + 2],
                        ]
                    })
                })
        })
        .collect()
}

#[test]
fn rock_standing_on_merged_blocks_meets_them_at_shared_corners() {
    // A sharp rock pillar on a blocky floor.
    let world = world(|[x, y, z]| {
        if y < 3 {
            Some(BLOCK)
        } else if y < 6 && (3..5).contains(&x) && (3..5).contains(&z) {
            Some(ROCK)
        } else {
            None
        }
    });
    let mesh = mesh(&world, &options(VertexPlacement::Blocky));
    let floor = corners(&mesh, BLOCK);
    let rock = corners(&mesh, ROCK);
    assert!(floor.len() < 128, "the open floor merges: {}", floor.len());
    // No rock corner lies inside a floor edge: no T-junction.
    let inside_edge = |point: [f32; 3], a: [f32; 3], b: [f32; 3]| {
        let along = |k: usize| {
            (point[k] - a[k]) * (b[(k + 1) % 3] - a[(k + 1) % 3])
                - (point[(k + 1) % 3] - a[(k + 1) % 3]) * (b[k] - a[k])
        };
        let collinear = (0..3).all(|k| along(k).abs() < 1.0e-6);
        let t = (0..3)
            .map(|k| (point[k] - a[k]) * (b[k] - a[k]))
            .sum::<f32>()
            / (0..3).map(|k| (b[k] - a[k]).powi(2)).sum::<f32>();
        collinear && t > 1.0e-6 && t < 1.0 - 1.0e-6
    };
    for point in rock.iter().flatten() {
        for triangle in &floor {
            for k in 0..3 {
                assert!(
                    !inside_edge(*point, triangle[k], triangle[(k + 1) % 3]),
                    "rock corner {point:?} splits floor edge {:?}-{:?}",
                    triangle[k],
                    triangle[(k + 1) % 3]
                );
            }
        }
    }
}
