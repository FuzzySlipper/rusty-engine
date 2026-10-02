//! Per-material surface modes and character, voxel densities, and scalar
//! regions with per-sample materials.

use std::collections::BTreeSet;

use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_mesh::{
    mesh_cells_standalone_with_options, mesh_chunk_in_world_with_options, mesh_scalar_samples,
    mesh_scalar_surface, MaterialSurface, MeshPayload, MeshVoxelCell, ScalarRegion, ScalarVolume,
    SurfaceCharacter, SurfaceMaterials, SurfaceMeshLimits, SurfaceMeshOptions, SurfaceMode,
    VertexPlacement,
};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

const STONE: u16 = 1;
const BRICK: u16 = 2;

fn cube(min: i64, max: i64, slot: u16) -> Vec<MeshVoxelCell> {
    let mut cells = Vec::new();
    for x in min..max {
        for y in min..max {
            for z in min..max {
                cells.push(MeshVoxelCell {
                    coordinate: [x, y, z],
                    material_slot: slot,
                });
            }
        }
    }
    cells
}

fn character(placement: VertexPlacement, crease: f32, roughness: f32) -> SurfaceCharacter {
    SurfaceCharacter {
        placement,
        crease_angle_degrees: crease,
        roughness,
    }
}

fn options(entries: &[(u16, SurfaceMode, SurfaceCharacter)]) -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        mode: SurfaceMode::DualContouring,
        materials: SurfaceMaterials::new(entries.iter().map(|(slot, mode, character)| {
            (
                *slot,
                MaterialSurface {
                    mode: *mode,
                    character: *character,
                },
            )
        }))
        .unwrap(),
        ..SurfaceMeshOptions::default()
    }
}

fn positions(mesh: &MeshPayload) -> Vec<[f32; 3]> {
    mesh.positions
        .chunks(3)
        .map(|p| [p[0], p[1], p[2]])
        .collect()
}

fn triangles(mesh: &MeshPayload) -> impl Iterator<Item = ([f32; 3], [f32; 3], [f32; 3])> + '_ {
    let p = positions(mesh);
    mesh.indices
        .chunks(3)
        .map(move |t| (p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]))
        .collect::<Vec<_>>()
        .into_iter()
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    n.map(|value| value / length)
}

#[test]
fn a_blocky_cube_of_voxels_meshes_as_a_cube_with_flat_faces() {
    let blocky = character(VertexPlacement::Blocky, 0.0, 0.0);
    let mesh = mesh_cells_standalone_with_options(
        1.0,
        [0.0; 3],
        &cube(0, 3, BRICK),
        options(&[(BRICK, SurfaceMode::DualContouring, blocky)]),
    )
    .unwrap();
    let points = positions(&mesh);
    assert!(!points.is_empty());
    for point in &points {
        assert!(
            point.iter().any(|value| *value == 0.0 || *value == 3.0),
            "vertex {point:?} is off the cube's faces"
        );
        assert!(point.iter().all(|value| (0.0..=3.0).contains(value)));
    }
    for corner in [
        [0.0, 0.0, 0.0],
        [3.0, 3.0, 3.0],
        [0.0, 3.0, 0.0],
        [3.0, 0.0, 3.0],
    ] {
        assert!(points.contains(&corner), "missing cube corner {corner:?}");
    }
    // Flat shading: every vertex normal is its facet's normal, on an axis.
    for (triangle, chunk) in mesh.indices.chunks(3).enumerate() {
        let p = |i: u32| points[i as usize];
        let face = face_normal(p(chunk[0]), p(chunk[1]), p(chunk[2]));
        for index in chunk {
            let n = &mesh.normals[*index as usize * 3..*index as usize * 3 + 3];
            for axis in 0..3 {
                assert!((n[axis] - face[axis]).abs() < 1.0e-5, "triangle {triangle}");
            }
        }
        assert_eq!(face.iter().filter(|v| v.abs() > 0.999).count(), 1);
    }
}

#[test]
fn smooth_placement_rounds_the_corners_that_sharp_placement_keeps() {
    let corner = |placement| {
        let mesh = mesh_cells_standalone_with_options(
            1.0,
            [0.0; 3],
            &cube(0, 3, STONE),
            options(&[(
                STONE,
                SurfaceMode::DualContouring,
                character(placement, 180.0, 0.0),
            )]),
        )
        .unwrap();
        positions(&mesh)
            .into_iter()
            .map(|p| p[0] + p[1] + p[2])
            .fold(f32::INFINITY, f32::min)
    };
    let blocky = corner(VertexPlacement::Blocky);
    let smooth = corner(VertexPlacement::Smooth);
    assert_eq!(blocky, 0.0);
    assert!(smooth > 0.5, "smooth corner sum {smooth}");
}

#[test]
fn roughness_displaces_vertices_deterministically_within_their_cells() {
    let mesh = |roughness| {
        mesh_cells_standalone_with_options(
            1.0,
            [0.0; 3],
            &cube(0, 4, STONE),
            options(&[(
                STONE,
                SurfaceMode::DualContouring,
                character(VertexPlacement::Smooth, 180.0, roughness),
            )]),
        )
        .unwrap()
    };
    let calm = mesh(0.0);
    let rough = mesh(0.4);
    assert_eq!(rough, mesh(0.4));
    assert_eq!(calm.indices.len(), rough.indices.len());
    assert_ne!(calm.positions, rough.positions);
    for point in positions(&rough) {
        assert!(point.iter().all(|value| (-0.5..=4.5).contains(value)));
    }
}

#[test]
fn cube_materials_keep_greedy_faces_beside_a_dual_contoured_material() {
    // A brick column standing in a stone slab: brick drawn as cubes, stone
    // dual contoured.
    let mut cells = Vec::new();
    for x in 0..6 {
        for z in 0..6 {
            for y in 0..2 {
                cells.push(MeshVoxelCell {
                    coordinate: [x, y, z],
                    material_slot: STONE,
                });
            }
        }
    }
    for y in 2..5 {
        cells.push(MeshVoxelCell {
            coordinate: [2, y, 2],
            material_slot: BRICK,
        });
    }
    let mesh = mesh_cells_standalone_with_options(
        1.0,
        [0.0; 3],
        &cells,
        options(&[(BRICK, SurfaceMode::GreedyCubes, SurfaceCharacter::default())]),
    )
    .unwrap();
    let modes: BTreeSet<_> = mesh
        .groups
        .iter()
        .map(|g| (g.material_slot, g.surface_mode))
        .collect();
    assert_eq!(
        modes,
        BTreeSet::from([
            (STONE, SurfaceMode::DualContouring),
            (BRICK, SurfaceMode::GreedyCubes)
        ])
    );
    // The column's four sides and top are cube faces; its base sits on stone
    // and is drawn too, since the stone surface need not cover it.
    let brick_triangles: u32 = mesh
        .groups
        .iter()
        .filter(|g| g.material_slot == BRICK)
        .map(|g| g.count / 3)
        .sum();
    assert_eq!(brick_triangles, 6 * 2);
    // Stone quads never belong to a brick voxel.
    for group in mesh.groups.iter().filter(|g| g.material_slot == STONE) {
        for triangle in group.start / 3..(group.start + group.count) / 3 {
            let owner = mesh.triangle_owners[triangle as usize];
            assert!(owner[1] < 2, "stone triangle owned by {owner:?}");
        }
    }
    // The stone surface meets the column on its face planes (x = 2, 3; z =
    // 2, 3): no stone vertex lies strictly inside the column's footprint
    // above the slab.
    for point in positions(&mesh) {
        let inside_column = point[0] > 2.0 && point[0] < 3.0 && point[2] > 2.0 && point[2] < 3.0;
        assert!(!(inside_column && point[1] > 2.0 + 1.0e-4), "{point:?}");
    }
}

fn world_with(fill: impl Fn([i64; 3]) -> Option<(u16, f32)>) -> VoxelWorld {
    let grid = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(4).unwrap()).unwrap();
    let mut world = VoxelWorld::new(grid);
    for cx in 0..2 {
        let coord = ChunkCoord::new(cx, 0, 0);
        let mut chunk = VoxelChunk::from_spec(&grid);
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    let local = LocalVoxelCoord::new(x, y, z);
                    let voxel = grid.chunk_local_to_voxel(coord, local).to_array();
                    if let Some((slot, density)) = fill(voxel) {
                        if slot != 0 {
                            chunk
                                .set(local, VoxelValue::solid(VoxelMaterialId::new(slot)))
                                .unwrap();
                        }
                        chunk.set_density(local, density).unwrap();
                    }
                }
            }
        }
        world.insert(coord, chunk);
    }
    world
}

#[test]
fn voxel_densities_place_the_surface_between_samples() {
    // A floor two voxels deep whose top voxels are mostly empty: the surface
    // sits a quarter cell above the solid voxel centres, not on the face.
    let world = world_with(|[_, y, _]| match y {
        0 => Some((STONE, -1.0)),
        1 => Some((STONE, -0.25)),
        _ => Some((0, 0.75)),
    });
    let options = SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring);
    let mesh = mesh_chunk_in_world_with_options(&world, ChunkCoord::new(0, 0, 0), &options)
        .unwrap()
        .unwrap();
    let tops: Vec<f32> = positions(&mesh)
        .into_iter()
        // Away from the world's sides, where absent chunks read as empty.
        .filter(|p| p[1] > 1.0 && (1.0..=4.0).contains(&p[0]) && (1.0..=3.0).contains(&p[2]))
        .map(|p| p[1])
        .collect();
    assert!(!tops.is_empty());
    // Centres at y = 1.5 (density -0.25) and 2.5 (+0.75): crossing at 1.75.
    assert!(tops.iter().all(|y| (y - 1.75).abs() < 1.0e-5), "{tops:?}");
}

#[test]
fn reconstructed_tile_coordinates_are_continuous_across_chunks() {
    let world = world_with(|[x, y, _]| (y < 2 + (x % 3)).then_some((STONE, -0.5)));
    let options = SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring);
    let mut by_position = std::collections::BTreeMap::<([i32; 3], u8), [u32; 2]>::new();
    for cx in 0..2 {
        let mesh = mesh_chunk_in_world_with_options(&world, ChunkCoord::new(cx, 0, 0), &options)
            .unwrap()
            .unwrap();
        for group in &mesh.groups {
            for index in &mesh.indices[group.start as usize..(group.start + group.count) as usize] {
                let i = *index as usize;
                let world_position = [
                    ((mesh.positions[i * 3] + cx as f32 * 4.0) * 1024.0) as i32,
                    (mesh.positions[i * 3 + 1] * 1024.0) as i32,
                    (mesh.positions[i * 3 + 2] * 1024.0) as i32,
                ];
                let uv = [
                    mesh.tile_coordinates[i * 2].to_bits(),
                    mesh.tile_coordinates[i * 2 + 1].to_bits(),
                ];
                let key = (world_position, group.direction.unwrap() as u8);
                if let Some(previous) = by_position.insert(key, uv) {
                    assert_eq!(previous, uv, "tile coordinate seam at {key:?}");
                }
            }
        }
    }
}

fn scalar_field(dims: [usize; 3]) -> (Vec<f32>, Vec<u16>) {
    let mut samples = Vec::new();
    let mut materials = Vec::new();
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let p = [x as f32, y as f32, z as f32];
                // Rock: a wavy blob. Brick: a box SDF embedded in it.
                let rock = ((p[0] - 10.0).powi(2) + (p[1] - 9.0).powi(2) + (p[2] - 11.0).powi(2))
                    .sqrt()
                    - 7.5
                    + (p[0] * 0.7).sin() * 0.6;
                let q = [
                    (p[0] - 14.5).abs() - 3.0,
                    (p[1] - 9.5).abs() - 3.0,
                    (p[2] - 11.5).abs() - 3.0,
                ];
                let outside = q.map(|v| v.max(0.0));
                let brick = (outside[0].powi(2) + outside[1].powi(2) + outside[2].powi(2)).sqrt()
                    + q[0].max(q[1]).max(q[2]).min(0.0);
                samples.push(rock.min(brick));
                materials.push(if brick < rock { BRICK } else { STONE });
            }
        }
    }
    (samples, materials)
}

#[test]
fn scalar_regions_reproduce_the_whole_surface_and_carry_sample_materials() {
    let dims = [24, 20, 22];
    let (samples, materials) = scalar_field(dims);
    let volume = ScalarVolume {
        origin: [1.0, -2.0, 0.5],
        spacing: 0.5,
        dimensions: dims,
        samples: &samples,
        materials: Some(&materials),
        isovalue: 0.0,
    };
    let characters = SurfaceMaterials::new([(
        BRICK,
        MaterialSurface {
            mode: SurfaceMode::DualContouring,
            character: character(VertexPlacement::Blocky, 0.0, 0.0),
        },
    )])
    .unwrap();
    let limits = SurfaceMeshLimits::default();
    let whole = mesh_scalar_surface(volume, &characters, None, limits).unwrap();
    assert!(whole.slots.contains(&STONE) && whole.slots.contains(&BRICK));
    let canonical = |surface: &svc_mesh::ScalarSurface| {
        let mut set = Vec::new();
        for (triangle, halo) in surface.triangles.iter().zip(&surface.halo) {
            if *halo {
                continue;
            }
            let mut corners =
                triangle.map(|index| surface.positions[index as usize].map(f32::to_bits));
            let first = (0..3).min_by_key(|k| corners[*k]).unwrap();
            corners.rotate_left(first);
            set.push(corners);
        }
        set.sort();
        set
    };
    let mut regions = Vec::new();
    let block = 8;
    for z in (0..dims[2]).step_by(block) {
        for y in (0..dims[1]).step_by(block) {
            for x in (0..dims[0]).step_by(block) {
                let region = ScalarRegion {
                    min: [x, y, z],
                    max: [
                        (x + block).min(dims[0]),
                        (y + block).min(dims[1]),
                        (z + block).min(dims[2]),
                    ],
                };
                regions.extend(canonical(
                    &mesh_scalar_surface(volume, &characters, Some(region), limits).unwrap(),
                ));
            }
        }
    }
    regions.sort();
    assert_eq!(regions, canonical(&whole));
}

#[test]
fn scalar_volumes_without_materials_keep_slot_zero_and_default_character() {
    let dims = [12, 12, 12];
    let (samples, _) = scalar_field(dims);
    let plain = mesh_scalar_samples(
        [0.0; 3],
        1.0,
        dims,
        &samples,
        0.0,
        SurfaceMeshLimits::default(),
    )
    .unwrap();
    assert_eq!(plain.groups.len(), 1);
    assert_eq!(plain.groups[0].material_slot, 0);
    let surface = mesh_scalar_surface(
        ScalarVolume {
            origin: [0.0; 3],
            spacing: 1.0,
            dimensions: dims,
            samples: &samples,
            materials: None,
            isovalue: 0.0,
        },
        &SurfaceMaterials::default(),
        None,
        SurfaceMeshLimits::default(),
    )
    .unwrap();
    assert_eq!(
        surface.positions.into_iter().flatten().collect::<Vec<_>>(),
        plain.positions
    );
}

#[test]
fn invalid_characters_and_duplicate_slots_are_refused() {
    let surface = |crease, roughness| MaterialSurface {
        mode: SurfaceMode::DualContouring,
        character: character(VertexPlacement::Sharp, crease, roughness),
    };
    assert!(SurfaceMaterials::new([(1, surface(190.0, 0.0))]).is_err());
    assert!(SurfaceMaterials::new([(1, surface(30.0, 0.6))]).is_err());
    assert!(SurfaceMaterials::new([(1, surface(30.0, 0.1)), (1, surface(0.0, 0.0))]).is_err());
    assert!(SurfaceMaterials::new([(1, surface(30.0, 0.1)), (2, surface(0.0, 0.0))]).is_ok());
}

#[test]
fn triangles_carry_finite_geometry() {
    let mesh = mesh_cells_standalone_with_options(1.0, [0.0; 3], &cube(0, 2, STONE), options(&[]))
        .unwrap();
    for (a, b, c) in triangles(&mesh) {
        assert!(face_normal(a, b, c).iter().all(|v| v.is_finite()));
    }
}

#[test]
fn a_blocky_brick_box_in_smooth_rock_is_planar_and_closed_against_it() {
    let dims = [24, 20, 22];
    let (samples, materials) = scalar_field(dims);
    let characters = SurfaceMaterials::new([
        (
            BRICK,
            MaterialSurface {
                mode: SurfaceMode::DualContouring,
                character: character(VertexPlacement::Blocky, 0.0, 0.0),
            },
        ),
        (
            STONE,
            MaterialSurface {
                mode: SurfaceMode::DualContouring,
                character: character(VertexPlacement::Smooth, 180.0, 0.0),
            },
        ),
    ])
    .unwrap();
    let surface = mesh_scalar_surface(
        ScalarVolume {
            origin: [0.0; 3],
            spacing: 1.0,
            dimensions: dims,
            samples: &samples,
            materials: Some(&materials),
            isovalue: 0.0,
        },
        &characters,
        None,
        SurfaceMeshLimits::default(),
    )
    .unwrap();
    // Closed: every edge joins exactly two triangles, across materials too.
    let mut edges = std::collections::BTreeMap::<(u32, u32), u32>::new();
    for triangle in &surface.triangles {
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (a, b) = (triangle[a], triangle[b]);
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    assert!(edges.values().all(|uses| *uses == 2));
    // Away from the rock, the brick's faces are exact axis planes on the
    // sample grid; only triangles sharing a vertex with rock bend to meet it.
    let rock_vertices: std::collections::BTreeSet<u32> = surface
        .triangles
        .iter()
        .zip(&surface.slots)
        .filter(|(_, slot)| **slot == STONE)
        .flat_map(|(triangle, _)| *triangle)
        .collect();
    let mut faces = 0;
    for (triangle, slot) in surface.triangles.iter().zip(&surface.slots) {
        if *slot != BRICK || triangle.iter().any(|vertex| rock_vertices.contains(vertex)) {
            continue;
        }
        faces += 1;
        let [a, b, c] = triangle.map(|index| surface.positions[index as usize]);
        let normal = face_normal(a, b, c);
        let axis = (0..3)
            .max_by(|l, r| normal[*l].abs().total_cmp(&normal[*r].abs()))
            .unwrap();
        assert!(normal[axis].abs() > 0.99999, "{normal:?}");
        assert_eq!(a[axis].fract(), 0.5, "on a face between samples");
    }
    assert!(faces > 0);
}
