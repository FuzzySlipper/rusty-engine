use super::*;

/// Rock filling 0..20 × 0..4 × 0..20 but where `open`.
fn rock(open: impl Fn(i64, i64, i64) -> bool) -> VoxelCollisionScene {
    let mut solids = Vec::new();
    for x in 0..20 {
        for y in 0..4 {
            for z in 0..20 {
                if !open(x, y, z) {
                    solids.push([x, y, z]);
                }
            }
        }
    }
    VoxelCollisionScene::from_solid_voxels(1.0, 8, solids).unwrap()
}

/// A passage two metres square cut along +x from (2, 1, 2) to x = 16, then
/// turning along +z to z = 16.
fn bending_passage() -> VoxelCollisionScene {
    rock(|x, y, z| {
        (1..3).contains(&y)
            && (((2..16).contains(&x) && (2..4).contains(&z))
                || ((14..16).contains(&x) && (2..16).contains(&z)))
    })
}

#[test]
fn solid_metres_measures_the_rock_a_line_crosses() {
    let wall = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[4, 0, 0], [5, 0, 0]]).unwrap();
    let crossed = wall.solid_metres([0.5, 0.5, 0.5], [9.5, 0.5, 0.5]);
    assert!((crossed - 2.0).abs() < 1e-9, "{crossed}");
    assert!(!wall.segment_clear([0.5, 0.5, 0.5], [9.5, 0.5, 0.5]));
    assert!(wall.segment_clear([0.5, 1.5, 0.5], [9.5, 1.5, 0.5]));
    assert_eq!(wall.solid_metres([0.5, 1.5, 0.5], [9.5, 1.5, 0.5]), 0.0);
    // Backwards it is the same rock.
    let back = wall.solid_metres([9.5, 0.5, 0.5], [0.5, 0.5, 0.5]);
    assert!((back - 2.0).abs() < 1e-9, "{back}");
}

/// A triangle mesh's positions and triangles.
type Geometry = (Vec<[f64; 3]>, Vec<[u32; 3]>);

/// Closed boxes, each `[min, max]`, as one triangle mesh wound outward by
/// the right-hand rule.
fn box_geometry(boxes: &[[[f64; 3]; 2]]) -> Geometry {
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    for [min, max] in boxes {
        let base = positions.len() as u32;
        for corner in 0..8 {
            positions.push([
                if corner & 1 == 0 { min[0] } else { max[0] },
                if corner & 2 == 0 { min[1] } else { max[1] },
                if corner & 4 == 0 { min[2] } else { max[2] },
            ]);
        }
        for [a, b, c, d] in [
            [0, 2, 6, 4],
            [1, 5, 7, 3],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 1, 3, 2],
            [4, 6, 7, 5],
        ] {
            triangles.push([base + a, base + c, base + b]);
            triangles.push([base + a, base + d, base + c]);
        }
    }
    (positions, triangles)
}

/// A scene whose static collision is the given meshes, each its own instance.
fn mesh_scene(meshes: Vec<Geometry>) -> VoxelCollisionScene {
    let (assets, instances): (Vec<_>, Vec<_>) = meshes
        .into_iter()
        .enumerate()
        .map(|(index, (positions, triangles))| {
            let id = index as u64 + 1;
            (
                crate::StaticMeshColliderAsset::new(
                    crate::StaticMeshAssetId(id),
                    positions,
                    triangles,
                )
                .unwrap(),
                crate::StaticMeshColliderInstance {
                    id: crate::StaticMeshInstanceId(id),
                    asset: crate::StaticMeshAssetId(id),
                    transform: Default::default(),
                },
            )
        })
        .unzip();
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    scene
        .replace_static_mesh_colliders(assets, instances)
        .unwrap();
    scene
}

/// Closed boxes in one static mesh.
fn mesh_boxes(boxes: &[[[f64; 3]; 2]]) -> VoxelCollisionScene {
    mesh_scene(vec![box_geometry(boxes)])
}

/// Closed boxes, each its own static mesh.
fn mesh_instances(boxes: &[[[f64; 3]; 2]]) -> VoxelCollisionScene {
    mesh_scene(boxes.iter().map(|one| box_geometry(&[*one])).collect())
}

#[test]
fn solid_metres_measures_static_mesh_walls_either_way() {
    let wall = [[4.0, -2.0, -2.0], [5.0, 2.0, 2.0]];
    let near = [0.5, 0.0, 0.0];
    let far = [9.5, 0.0, 0.0];
    let one = mesh_boxes(&[wall]);
    // The air beyond the wall's far face is air, whichever way the line runs.
    for (from, to) in [(near, far), (far, near)] {
        let crossed = one.solid_metres(from, to);
        assert!(
            (crossed - 1.0).abs() < 1e-6,
            "{from:?} to {to:?}: {crossed}"
        );
    }
    assert!(!one.segment_clear(near, far));
    assert!(one.segment_clear([0.5, 3.0, 0.0], [9.5, 3.0, 0.0]));
    // Ending inside the wall counts the rock up to the end.
    let into = one.solid_metres(near, [4.5, 0.0, 0.0]);
    assert!((into - 0.5).abs() < 1e-6, "{into}");
    let two = mesh_boxes(&[wall, [[7.0, -2.0, -2.0], [8.0, 2.0, 2.0]]]);
    for (from, to) in [(near, far), (far, near)] {
        let crossed = two.solid_metres(from, to);
        assert!(
            (crossed - 2.0).abs() < 1e-6,
            "{from:?} to {to:?}: {crossed}"
        );
    }
}

#[test]
fn overlapping_meshes_count_as_one_solid() {
    // Two boxes overlapping over x 5..6: 3 m of rock, not 2.
    let near = [0.5, 0.0, 0.0];
    let far = [9.5, 0.0, 0.0];
    let overlapping = mesh_boxes(&[
        [[4.0, -2.0, -2.0], [6.0, 2.0, 2.0]],
        [[5.0, -2.0, -2.0], [7.0, 2.0, 2.0]],
    ]);
    for (from, to) in [(near, far), (far, near)] {
        let crossed = overlapping.solid_metres(from, to);
        assert!(
            (crossed - 3.0).abs() < 1e-6,
            "{from:?} to {to:?}: {crossed}"
        );
    }
    // Starting inside the overlap.
    let out = overlapping.solid_metres([5.5, 0.0, 0.0], far);
    assert!((out - 1.5).abs() < 1e-6, "{out}");
}

#[test]
fn touching_meshes_cross_their_shared_face() {
    let near = [0.5, 0.0, 0.0];
    let far = [9.5, 0.0, 0.0];
    let a = [[4.0, -2.0, -2.0], [5.0, 2.0, 2.0]];
    let b = [[5.0, -2.0, -2.0], [6.0, 2.0, 2.0]];
    // In one mesh, and as two instances.
    for scene in [mesh_boxes(&[a, b]), mesh_instances(&[a, b])] {
        for (from, to) in [(near, far), (far, near)] {
            let crossed = scene.solid_metres(from, to);
            assert!(
                (crossed - 2.0).abs() < 1e-6,
                "{from:?} to {to:?}: {crossed}"
            );
        }
        // Through the edge two faces share, and through a corner.
        let edge = scene.solid_metres([0.5, 0.0, 2.0], [9.5, 0.0, 2.0]);
        assert!((edge - 2.0).abs() < 1e-6, "{edge}");
        let corner = scene.solid_metres([0.5, 2.0, 2.0], [9.5, 2.0, 2.0]);
        assert!((corner - 2.0).abs() < 1e-6, "{corner}");
    }
}

#[test]
fn a_segment_starting_in_a_mesh_counts_the_rock_it_starts_in() {
    let one = mesh_boxes(&[[[4.0, -2.0, -2.0], [5.0, 2.0, 2.0]]]);
    for (from, to, expected) in [
        ([4.5, 0.0, 0.0], [0.5, 0.0, 0.0], 0.5),
        ([4.5, 0.0, 0.0], [9.5, 0.0, 0.0], 0.5),
        ([0.5, 0.0, 0.0], [4.5, 0.0, 0.0], 0.5),
        ([4.2, 0.0, 0.0], [4.8, 0.0, 0.0], 0.6),
    ] {
        let crossed = one.solid_metres(from, to);
        assert!(
            (crossed - expected).abs() < 1e-6,
            "{from:?} to {to:?}: {crossed}"
        );
    }
}

#[test]
fn a_voxel_and_a_mesh_in_one_place_count_once() {
    let mut scene = mesh_boxes(&[[[4.0, 0.0, 0.0], [5.0, 1.0, 1.0]]]);
    let voxel = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[4, 0, 0]]).unwrap();
    scene.voxel_world = voxel.voxel_world.clone();
    let crossed = scene.solid_metres([0.5, 0.5, 0.5], [9.5, 0.5, 0.5]);
    assert!((crossed - 1.0).abs() < 1e-6, "{crossed}");
}

#[test]
fn air_search_follows_a_bending_passage() {
    let scene = bending_passage();
    let start = [3.0, 2.0, 3.0];
    let end = [15.0, 2.0, 15.0];
    assert!(!scene.segment_clear(start, end));
    let straight = distance(start, end);
    let search = AirSearch::run(&scene, start, 40.0, &[end]);
    let reach = search
        .reach(&scene, end)
        .expect("the passage reaches its end");
    // Round the inside corner at (14, 4) the path is about 22 m, against
    // 17 m straight through the rock.
    assert!(reach.metres > straight, "{reach:?}");
    assert!((21.0..25.5).contains(&reach.metres), "{reach:?}");
    // It leaves the start along +x and arrives at the end along +z.
    assert!(reach.near_root[0] > start[0] + 1.5, "{reach:?}");
    assert!(reach.near_point[2] < end[2] - 1.5, "{reach:?}");
    assert!(!search.truncated);
    assert!(search.settled_cells() < 200, "{}", search.settled_cells());
    // Searching toward the end finds the same path as searching everywhere.
    let everywhere = AirSearch::run(&scene, start, 40.0, &[]);
    let full = everywhere.reach(&scene, end).unwrap();
    assert!(
        (full.metres - reach.metres).abs() < 0.02 * full.metres,
        "{full:?} {reach:?}"
    );
}

#[test]
fn air_search_does_not_reach_a_sealed_room() {
    // The passage's two legs, with the rock between them left whole.
    let scene = rock(|x, y, z| {
        (1..3).contains(&y)
            && (((2..12).contains(&x) && (2..4).contains(&z))
                || ((14..16).contains(&x) && (8..16).contains(&z)))
    });
    let search = AirSearch::run(&scene, [3.0, 2.0, 3.0], 60.0, &[[15.0, 2.0, 15.0]]);
    assert!(search.reach(&scene, [15.0, 2.0, 15.0]).is_none());
    assert!(!search.truncated);
}

#[test]
fn enclosure_tells_a_closed_passage_from_open_ground() {
    let scene = bending_passage();
    let inside = scene.enclosure([3.0, 2.0, 3.0]);
    assert_eq!(inside.closed, 1.0, "{inside:?}");
    assert!(inside.mean_free_metres < 8.0, "{inside:?}");
    let ground: Vec<[i64; 3]> = (-30..30)
        .flat_map(|x| (-30..30).map(move |z| [x, -1, z]))
        .collect();
    let open = VoxelCollisionScene::from_solid_voxels(1.0, 8, ground).unwrap();
    let outside = open.enclosure([0.5, 1.5, 0.5]);
    assert_eq!(outside.closed, 0.0, "{outside:?}");
    assert_eq!(outside.mean_free_metres, ENCLOSURE_METRES);
}
