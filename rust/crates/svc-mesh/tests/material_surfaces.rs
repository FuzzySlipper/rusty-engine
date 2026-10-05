//! Per-material surface modes and character, voxel densities, and scalar
//! regions with per-sample materials.

use std::collections::BTreeSet;

use core_space::{ChunkCoord, ChunkDims, Direction6, GridId, LocalVoxelCoord, VoxelGridSpec};
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

const WATER: u16 = 3;
const GLASS: u16 = 4;

/// The area of `slot`'s cube faces looking along `direction`.
fn face_area(mesh: &MeshPayload, slot: u16, direction: Direction6) -> f32 {
    let p = positions(mesh);
    mesh.groups
        .iter()
        .filter(|group| group.material_slot == slot && group.direction == Some(direction))
        .flat_map(|group| {
            mesh.indices[group.start as usize..(group.start + group.count) as usize].chunks(3)
        })
        .map(|t| {
            let [a, b, c] = [p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]];
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
        })
        .sum()
}

#[test]
fn non_occluding_materials_show_the_faces_behind_them_but_not_their_own_inner_faces() {
    // A stone bed under two layers of water spanning both chunks, with one
    // glass voxel on the water at the world's corner.
    let world = world_with(|[x, y, z]| match y {
        0 => Some((STONE, -1.0)),
        1 | 2 => Some((WATER, -1.0)),
        3 if [x, z] == [0, 0] => Some((GLASS, -1.0)),
        _ => None,
    });
    let non_occluding = BTreeSet::from([WATER, GLASS]);
    let greedy = SurfaceMeshOptions {
        non_occluding: non_occluding.clone(),
        ..SurfaceMeshOptions::default()
    };
    // A reconstructed material elsewhere takes the mixed path.
    let mixed = SurfaceMeshOptions {
        materials: SurfaceMaterials::new([(
            BRICK,
            MaterialSurface {
                mode: SurfaceMode::DualContouring,
                character: SurfaceCharacter::default(),
            },
        )])
        .unwrap(),
        non_occluding,
        ..SurfaceMeshOptions::default()
    };
    for options in [greedy, mixed] {
        let mesh = mesh_chunk_in_world_with_options(&world, ChunkCoord::new(0, 0, 0), &options)
            .unwrap()
            .unwrap();
        // The bed is seen through the water; water never draws against stone.
        assert_eq!(face_area(&mesh, STONE, Direction6::PosY), 16.0);
        assert_eq!(face_area(&mesh, WATER, Direction6::NegY), 0.0);
        // One water surface, and no faces between water voxels, including
        // across the chunk border.
        assert_eq!(face_area(&mesh, WATER, Direction6::PosY), 16.0);
        assert_eq!(face_area(&mesh, WATER, Direction6::PosX), 0.0);
        assert_eq!(face_area(&mesh, WATER, Direction6::NegX), 8.0);
        // Different non-occluding materials both draw their shared face.
        assert_eq!(face_area(&mesh, GLASS, Direction6::NegY), 1.0);
    }

    // Undeclared, every material hides its neighbours.
    let mesh = mesh_chunk_in_world_with_options(
        &world,
        ChunkCoord::new(0, 0, 0),
        &SurfaceMeshOptions::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(face_area(&mesh, STONE, Direction6::PosY), 0.0);
    assert_eq!(face_area(&mesh, WATER, Direction6::PosY), 15.0);

    // Standalone cells follow the same declaration.
    let mut cells = cube(0, 2, STONE);
    cells.retain(|cell| cell.coordinate[1] == 0);
    cells.extend(
        cube(0, 2, WATER)
            .into_iter()
            .filter(|cell| cell.coordinate[1] == 1)
            .map(|cell| MeshVoxelCell {
                coordinate: [cell.coordinate[0], 1, cell.coordinate[2]],
                ..cell
            }),
    );
    let mesh = mesh_cells_standalone_with_options(
        1.0,
        [0.0; 3],
        &cells,
        SurfaceMeshOptions {
            non_occluding: BTreeSet::from([WATER]),
            ..SurfaceMeshOptions::default()
        },
    )
    .unwrap();
    assert_eq!(face_area(&mesh, STONE, Direction6::PosY), 4.0);
    assert_eq!(face_area(&mesh, WATER, Direction6::NegY), 0.0);
    assert_eq!(face_area(&mesh, WATER, Direction6::PosY), 4.0);
}

#[test]
fn a_reconstructed_bed_shows_under_a_non_occluding_material() {
    // A dual-contoured stone floor two voxels deep under one layer of water.
    let world = world_with(|[_, y, _]| match y {
        0 | 1 => Some((STONE, -0.5)),
        2 => Some((WATER, -0.5)),
        _ => None,
    });
    let heights = |mesh: &MeshPayload, slot: u16| {
        let p = positions(mesh);
        mesh.groups
            .iter()
            .filter(|group| group.material_slot == slot)
            .flat_map(|group| {
                mesh.indices[group.start as usize..(group.start + group.count) as usize].iter()
            })
            .map(|index| p[*index as usize])
            // Away from the world's sides, where absent chunks read as empty.
            // and above the floor's underside, where the world ends.
            .filter(|point| (1.0..=3.0).contains(&point[0]) && (1.0..=3.0).contains(&point[2]))
            .filter(|point| point[1] > 1.0)
            .map(|point| point[1])
            .collect::<Vec<_>>()
    };
    let mesh = |non_occluding: BTreeSet<u16>| {
        let options = SurfaceMeshOptions {
            non_occluding,
            ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
        };
        mesh_chunk_in_world_with_options(&world, ChunkCoord::new(0, 0, 0), &options)
            .unwrap()
            .unwrap()
    };

    // Undeclared, the water hides the floor: only the water's top surfaces.
    let hidden = mesh(BTreeSet::new());
    assert!(heights(&hidden, STONE).is_empty());
    assert!(heights(&hidden, WATER)
        .iter()
        .all(|y| (y - 3.0).abs() < 1.0e-4));

    // Declared, the floor meets the water as it meets air, and the water
    // draws only its own top, never a face against the floor.
    let shown = mesh(BTreeSet::from([WATER]));
    let floor = heights(&shown, STONE);
    assert!(!floor.is_empty());
    assert!(floor.iter().all(|y| (y - 2.0).abs() < 1.0e-4), "{floor:?}");
    let water = heights(&shown, WATER);
    assert!(!water.is_empty());
    assert!(water.iter().all(|y| (y - 3.0).abs() < 1.0e-4), "{water:?}");
}

#[test]
fn greedy_water_on_a_reconstructed_bed_draws_no_face_where_the_bed_lies_but_keeps_its_shore() {
    // A dual-contoured stone bed (y 0..2, top at y = 2) under one layer of
    // greedy, non-occluding water (y = 2), with equal densities, so the bed
    // lies exactly on the water's underside. A step at x = 2 (stone only at
    // y = 0, water at y = 1 and 2) deepens the bed, and from x = 6 the stone
    // rises through the water layer to a shore with air above it.
    let world = world_with(|[x, y, _]| match (x, y) {
        (2, 0) => Some((STONE, -0.5)),
        (2, 1 | 2) => Some((WATER, -0.5)),
        (6.., 0..=2) | (..=5, 0 | 1) => Some((STONE, -0.5)),
        (..=5, 2) => Some((WATER, -0.5)),
        _ => None,
    });
    let options = SurfaceMeshOptions {
        materials: SurfaceMaterials::new([(
            WATER,
            MaterialSurface {
                mode: SurfaceMode::GreedyCubes,
                character: SurfaceCharacter::default(),
            },
        )])
        .unwrap(),
        non_occluding: BTreeSet::from([WATER]),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    };
    // Cube face area away from the world's sides, where absent chunks read
    // as air. Faces merge along rows, so a face at a side stays out too.
    let inner_area = |mesh: &MeshPayload, direction: Direction6| {
        let p = positions(mesh);
        mesh.groups
            .iter()
            .filter(|group| group.material_slot == WATER && group.direction == Some(direction))
            .flat_map(|group| {
                mesh.indices[group.start as usize..(group.start + group.count) as usize].chunks(3)
            })
            .map(|t| [p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]])
            .filter(|[a, b, c]| {
                let z = (a[2] + b[2] + c[2]) / 3.0;
                let x = (a[0] + b[0] + c[0]) / 3.0;
                (1.0..3.0).contains(&z) && x > 1.0
            })
            .map(|[a, b, c]| {
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
            })
            .sum::<f32>()
    };
    let lake = mesh_chunk_in_world_with_options(&world, ChunkCoord::new(0, 0, 0), &options)
        .unwrap()
        .unwrap();
    let shore = mesh_chunk_in_world_with_options(&world, ChunkCoord::new(1, 0, 0), &options)
        .unwrap()
        .unwrap();
    // The bed and the step under the water are the stone's surface alone,
    // flat or stepped: no water face beneath it to fight it in depth.
    for (mesh, name) in [(&lake, "lake"), (&shore, "shore")] {
        assert_eq!(inner_area(mesh, Direction6::NegY), 0.0, "{name} underside");
        assert_eq!(inner_area(mesh, Direction6::NegX), 0.0, "{name} step");
    }
    assert!(lake.groups.iter().any(|group| group.material_slot == STONE));
    // The water's whole top stays (x 0..6 by z 0..4).
    assert_eq!(
        face_area(&lake, WATER, Direction6::PosY) + face_area(&shore, WATER, Direction6::PosY),
        24.0
    );
    // At the shore, the stone's side meets the water but air lies above it,
    // so the water's side face toward it stays, where the stone's rounded
    // edge may leave it exposed.
    assert_eq!(face_area(&shore, WATER, Direction6::PosX), 4.0);
}

#[test]
fn a_shore_face_never_lies_on_the_bank_it_is_kept_beside() {
    // Water (greedy, non-occluding) one voxel deep at y = 2 up to x = 5, on
    // a stone bed; from x = 6 the stone bank's top is flush with the water's
    // and air lies above both. Chunks around z = 0 keep the world's sides
    // away from the measured shore.
    let grid = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(4).unwrap()).unwrap();
    let mut world = VoxelWorld::new(grid);
    for cx in 0..2 {
        for cz in -1..2 {
            let coord = ChunkCoord::new(cx, 0, cz);
            let mut chunk = VoxelChunk::from_spec(&grid);
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        let local = LocalVoxelCoord::new(x, y, z);
                        let [x, y, _] = grid.chunk_local_to_voxel(coord, local).to_array();
                        let slot = match (x, y) {
                            (6.., 0..=2) | (..=5, 0 | 1) => STONE,
                            (..=5, 2) => WATER,
                            _ => continue,
                        };
                        chunk
                            .set(local, VoxelValue::solid(VoxelMaterialId::new(slot)))
                            .unwrap();
                        chunk.set_density(local, -0.5).unwrap();
                    }
                }
            }
            world.insert(coord, chunk);
        }
    }
    // Each slot's area lying in the shore's plane, x = 6 (2 in the chunk).
    let in_plane = |mesh: &MeshPayload, slot: u16| -> f32 {
        let p = positions(mesh);
        mesh.groups
            .iter()
            .filter(|group| group.material_slot == slot)
            .flat_map(|group| {
                mesh.indices[group.start as usize..(group.start + group.count) as usize].chunks(3)
            })
            .map(|t| [p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]])
            .filter(|corners| {
                corners
                    .iter()
                    .all(|corner| (corner[0] - 2.0).abs() < 1.0e-4)
            })
            .map(|[a, b, c]| {
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
            })
            .sum()
    };
    for (placement, roughness, bank, water) in [
        // Rounded or jittered banks leave the face partly exposed and never
        // lie on it, so it stays.
        (VertexPlacement::Smooth, 0.0, 0.0, 4.0),
        (VertexPlacement::Sharp, 0.0, 0.0, 4.0),
        (VertexPlacement::Blocky, 0.3, 0.0, 4.0),
        // An exact block bank is the face, which goes.
        (VertexPlacement::Blocky, 0.0, 4.0, 0.0),
    ] {
        let options = SurfaceMeshOptions {
            materials: SurfaceMaterials::new([
                (
                    WATER,
                    MaterialSurface {
                        mode: SurfaceMode::GreedyCubes,
                        character: SurfaceCharacter::default(),
                    },
                ),
                (
                    STONE,
                    MaterialSurface {
                        mode: SurfaceMode::DualContouring,
                        character: character(placement, 30.0, roughness),
                    },
                ),
            ])
            .unwrap(),
            non_occluding: BTreeSet::from([WATER]),
            ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
        };
        let mesh = mesh_chunk_in_world_with_options(&world, ChunkCoord::new(1, 0, 0), &options)
            .unwrap()
            .unwrap();
        assert_eq!(
            [in_plane(&mesh, STONE), in_plane(&mesh, WATER)],
            [bank, water],
            "{placement:?} roughness {roughness}: bank and water area on the shore's plane"
        );
    }
}
