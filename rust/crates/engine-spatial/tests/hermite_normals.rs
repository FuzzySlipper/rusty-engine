//! Stored crossing normals (#9504): a density brush records the exact normal
//! of the shape it cuts at every crossing it sets, Sharp placement puts the
//! cut's edges where they are, both chunks at a seam read the same normals,
//! and anything else that changes a voxel clears its edges' normals.

use engine_spatial::{
    MaterialSurface, MaterialVoxel, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions,
    SurfaceMode, VertexPlacement, VoxelCollisionScene, VoxelDensityEdit, VoxelDensityEditService,
    VoxelDensityOperation, VoxelDensityShape, VoxelEdit, VoxelEditService,
};

const ROCK: u16 = 1;
const TOP: i64 = 12;
/// The cut: an axis-aligned box straddling the chunk border at x = 16.
const CUT_MIN: [f64; 3] = [10.3, 6.4, 5.6];
const CUT_MAX: [f64; 3] = [21.7, 14.0, 13.3];

fn options(ignore_stored_normals: bool) -> SurfaceMeshOptions {
    SurfaceMeshOptions {
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
        ignore_stored_normals,
        ..SurfaceMeshOptions::default()
    }
}

/// A rock slab, 32 × 12 × 24, in chunks of 8.
fn slab() -> VoxelCollisionScene {
    let voxels = (0..32).flat_map(|x| {
        (0..24).flat_map(move |z| {
            (0..TOP).map(move |y| MaterialVoxel {
                state: 0,
                address: [x, y, z],
                material_slot: ROCK,
            })
        })
    });
    VoxelCollisionScene::from_material_voxels_with_mesh_options(1.0, 8, voxels, options(false))
        .unwrap()
}

fn cut() -> VoxelDensityEdit {
    VoxelDensityEdit::Brush {
        shape: VoxelDensityShape::Box {
            min: CUT_MIN,
            max: CUT_MAX,
        },
        operation: VoxelDensityOperation::Subtract,
        material_slot: ROCK,
    }
}

/// The slab with the box cut out of it.
fn carved() -> (VoxelCollisionScene, usize) {
    let mut scene = slab();
    let receipt = VoxelDensityEditService::apply(&mut scene, &[cut()]).unwrap();
    (scene, receipt.hermite_normals)
}

fn crossing_count(scene: &VoxelCollisionScene) -> usize {
    scene
        .voxel_world()
        .resident_chunks()
        .map(|(_, chunk)| chunk.edge_crossing_count())
        .sum()
}

fn mesh_positions(scene: &VoxelCollisionScene) -> Vec<u32> {
    scene
        .mesh_chunks()
        .flat_map(|chunk| chunk.positions.iter().map(|value| value.to_bits()))
        .collect()
}

/// The cut's distance at a point: positive in the rock it left.
fn cut_distance(point: [f64; 3]) -> f64 {
    let q: [f64; 3] = std::array::from_fn(|axis| {
        let centre = (CUT_MIN[axis] + CUT_MAX[axis]) * 0.5;
        (point[axis] - centre).abs() - (CUT_MAX[axis] - CUT_MIN[axis]) * 0.5
    });
    let outside = q.map(|value| value.max(0.0));
    (outside[0] * outside[0] + outside[1] * outside[1] + outside[2] * outside[2]).sqrt()
        + q[0].max(q[1]).max(q[2]).min(0.0)
}

/// The mean and largest distance from the cut's surface over the vertices
/// that lie on it within a voxel of one of its edges below the slab's top.
fn edge_error(scene: &VoxelCollisionScene) -> (f64, f64) {
    let mut largest: f64 = 0.0;
    let mut sum = 0.0;
    let mut count = 0;
    for chunk in scene.mesh_chunks() {
        for vertex in chunk.positions.as_chunks::<3>().0 {
            let point: [f64; 3] = std::array::from_fn(|axis| {
                f64::from(chunk.translation[axis]) + f64::from(vertex[axis])
            });
            let near_faces = (0..3)
                .flat_map(|axis| {
                    [CUT_MIN[axis], CUT_MAX[axis]].map(move |face| (point[axis] - face).abs())
                })
                .filter(|distance| *distance < 1.0)
                .count();
            if near_faces >= 2 && point[1] < TOP as f64 - 1.0 && cut_distance(point).abs() < 1.5 {
                largest = largest.max(cut_distance(point).abs());
                sum += cut_distance(point).abs();
                count += 1;
            }
        }
    }
    (sum / f64::from(count.max(1)), largest)
}

#[test]
fn a_cut_stores_its_normals_and_its_sharp_edges_land_on_the_cut() {
    let (mut scene, stored) = carved();
    assert!(stored > 100, "{stored} normals stored");
    let with = edge_error(&scene);
    scene.set_mesh_options(options(true)).unwrap();
    let without = edge_error(&scene);
    // Mean and largest distance from the cut, in voxels.
    assert!(
        with.1 < 0.15,
        "edge vertices within {with:?} voxels of the cut"
    );
    assert!(
        without.0 > 2.0 * with.0 && without.1 > with.1,
        "estimated normals leave the edges {without:?} off, stored {with:?}"
    );
}

#[test]
fn both_chunks_at_a_seam_place_the_same_vertices() {
    let (scene, _) = carved();
    // Every vertex a chunk shares with its neighbour across x = 16 (the
    // halo cells both mesh) lies at the same place in both meshes.
    let world = |coord: [i64; 3]| -> Vec<[u32; 3]> {
        let chunk = scene.mesh_chunk(coord).expect("resident");
        chunk
            .positions
            .as_chunks::<3>()
            .0
            .iter()
            .map(|vertex| {
                std::array::from_fn(|axis| {
                    (f64::from(chunk.translation[axis]) + f64::from(vertex[axis])) as f32
                })
            })
            .map(|point: [f32; 3]| point.map(f32::to_bits))
            .collect()
    };
    for (y, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let left = world([1, y, z]);
        let right = world([2, y, z]);
        let shared: Vec<_> = left
            .iter()
            .filter(|point| {
                let x = f32::from_bits(point[0]);
                (15.0..17.0).contains(&x)
            })
            .collect();
        for point in shared {
            let x = f32::from_bits(point[0]);
            if (16.0..17.0).contains(&x) {
                assert!(
                    right.contains(point),
                    "chunk [1,{y},{z}]'s vertex {:?} at the seam",
                    point.map(f32::from_bits)
                );
            }
        }
    }
}

#[test]
fn a_voxel_edit_clears_the_normals_of_the_edges_it_touches() {
    let (mut scene, _) = carved();
    let count = |scene: &VoxelCollisionScene| -> usize {
        scene
            .voxel_world()
            .resident_chunks()
            .map(|(_, chunk)| chunk.edge_crossing_count())
            .sum()
    };
    let before = count(&scene);
    // A voxel of rock on the cut's floor: its edges crossing into the cut
    // carried normals.
    VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::Clear {
            address: [12, 5, 8],
        }],
    )
    .unwrap();
    let after = count(&scene);
    assert!(
        after < before && after + 6 >= before,
        "{before} then {after}"
    );
    // A smoothing pass over the cut clears what it changes.
    let receipt = VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Brush {
            shape: VoxelDensityShape::Sphere {
                center: [10.3, 6.4, 9.0],
                radius: 3.0,
            },
            operation: VoxelDensityOperation::Smooth { strength: 1.0 },
            material_slot: ROCK,
        }],
    )
    .unwrap();
    assert_eq!(receipt.hermite_normals, 0);
    assert!(count(&scene) < after);
}

/// Where a carved sphere's rim meets walls it did not make, their crossings
/// keep their own normals: the rim gains no spikes over the estimated
/// normals, wherever the sphere sits against a wall corner.
#[test]
fn a_crater_rim_keeps_the_walls_it_meets() {
    // Two walls two voxels thick meeting at a corner: x 8 to 10 along z, and
    // z 8 to 10 along x.
    let solid = |x: i64, z: i64| (8..10).contains(&x) || ((8..10).contains(&z) && x >= 8);
    let mut worst = (0.0_f64, 0.0_f64, [0.0; 3]);
    for (dx, dy, dz, radius) in [
        (1.2, 0.0, 0.0, 2.5),
        (2.3, 0.3, 1.7, 2.4),
        (1.6, 0.6, 1.6, 2.2),
        (0.9, 0.2, 2.6, 1.8),
        (2.45, 0.45, 2.45, 2.5),
        (1.05, 0.5, 0.95, 1.5),
    ] {
        let voxels = (0..16).flat_map(move |x| {
            (0..16).flat_map(move |z| {
                (0..12)
                    .filter(move |_| solid(x, z))
                    .map(move |y| MaterialVoxel {
                        state: 0,
                        address: [x, y, z],
                        material_slot: ROCK,
                    })
            })
        });
        let centre = [10.0 + dx, 6.0 + dy, 10.0 + dz];
        let mut scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
            1.0,
            8,
            voxels,
            options(false),
        )
        .unwrap();
        VoxelDensityEditService::apply(
            &mut scene,
            &[VoxelDensityEdit::Brush {
                shape: VoxelDensityShape::Sphere {
                    center: centre,
                    radius,
                },
                operation: VoxelDensityOperation::Subtract,
                material_slot: ROCK,
            }],
        )
        .unwrap();
        let truth = |point: [f64; 3]| {
            let wall_x = (point[0] - 9.0).abs() - 1.0;
            let wall_z = ((point[2] - 9.0).abs() - 1.0).max(8.0 - point[0]);
            let d: [f64; 3] = std::array::from_fn(|axis| point[axis] - centre[axis]);
            let sphere = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - radius;
            wall_x.min(wall_z).max(-sphere)
        };
        let largest = |scene: &VoxelCollisionScene| -> f64 {
            let mut largest: f64 = 0.0;
            for chunk in scene.mesh_chunks() {
                for vertex in chunk.positions.as_chunks::<3>().0 {
                    let point: [f64; 3] = std::array::from_fn(|axis| {
                        f64::from(chunk.translation[axis]) + f64::from(vertex[axis])
                    });
                    let d: [f64; 3] = std::array::from_fn(|axis| point[axis] - centre[axis]);
                    if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < radius + 1.5 {
                        largest = largest.max(truth(point).abs());
                    }
                }
            }
            largest
        };
        let stored = largest(&scene);
        scene.set_mesh_options(options(true)).unwrap();
        let estimated = largest(&scene);
        if stored - estimated > worst.0 - worst.1 {
            worst = (stored, estimated, centre);
        }
    }
    assert!(
        worst.0 <= worst.1 + 0.1,
        "rim vertices {} voxels off with stored normals, {} without, sphere at {:?}",
        worst.0,
        worst.1,
        worst.2
    );
}

#[test]
fn a_later_region_in_the_same_batch_clears_the_crossings_it_overwrites() {
    let region = VoxelDensityEdit::Region {
        min: [12, 5, 8],
        size: [1, 1, 1],
        densities: vec![-0.05],
        materials: vec![],
    };
    let mut batched = slab();
    VoxelDensityEditService::apply(&mut batched, &[cut(), region.clone()]).unwrap();
    let mut separate = slab();
    VoxelDensityEditService::apply(&mut separate, &[cut()]).unwrap();
    VoxelDensityEditService::apply(&mut separate, &[region]).unwrap();
    assert_eq!(crossing_count(&batched), crossing_count(&separate));
    assert_eq!(mesh_positions(&batched), mesh_positions(&separate));
}

#[test]
fn a_refused_rebuild_leaves_the_crossings_as_they_were() {
    let mut scene = slab();
    let mut limited = options(false);
    limited.limits.max_indices = scene
        .mesh_chunks()
        .map(|chunk| chunk.indices.len() as u32)
        .max()
        .unwrap();
    scene.set_mesh_options(limited).unwrap();
    let crossings = crossing_count(&scene);
    let positions = mesh_positions(&scene);
    // The sphere's cut needs more indices than the limit allows.
    let refused = VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Brush {
            shape: VoxelDensityShape::Sphere {
                center: [3.8, 3.7, 3.9],
                radius: 2.0,
            },
            operation: VoxelDensityOperation::Subtract,
            material_slot: ROCK,
        }],
    );
    assert!(refused.is_err());
    assert_eq!(crossing_count(&scene), crossings);
    assert_eq!(mesh_positions(&scene), positions);
}
