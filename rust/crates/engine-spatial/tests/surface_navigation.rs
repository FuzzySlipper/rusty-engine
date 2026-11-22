//! Surface navigation (#10967): surface cells on floors, walls and ceilings
//! of cube and dual-contoured collision, and the edges a crawler's ball
//! takes between them.

use std::collections::{BTreeSet, VecDeque};

use core_space::VoxelCoord;
use engine_spatial::{
    derive_surface_navigation, surface_edge, surface_edge_kind, CrawlerConfig, CrawlerSurface,
    MaterialSurface, MaterialVoxel, StaticMeshAssetId, StaticMeshColliderAsset,
    StaticMeshColliderInstance, StaticMeshInstanceId, StaticMeshTransform, SurfaceCell,
    SurfaceCharacter, SurfaceEdgeKind, SurfaceEdgeOutcome, SurfaceFace, SurfaceMaterials,
    SurfaceMeshOptions, SurfaceMode, SurfaceNavigation, SurfaceNavigationConfig, SurfaceRegion,
    VertexPlacement, VoxelCollisionScene,
};

/// A closed shell of `size`-metre voxels: inside x and z from −3 to 3 and y
/// from 1 to 5, in metres.
fn room_voxels(size: f64) -> Vec<[i64; 3]> {
    let per = (1.0 / size).round() as i64;
    let mut voxels = Vec::new();
    for x in -4 * per..4 * per {
        for y in 0..6 * per {
            for z in -4 * per..4 * per {
                let inside = (-3 * per..3 * per).contains(&x)
                    && (per..5 * per).contains(&y)
                    && (-3 * per..3 * per).contains(&z);
                if !inside {
                    voxels.push([x, y, z]);
                }
            }
        }
    }
    voxels
}

fn dual_contoured(voxels: Vec<[i64; 3]>) -> VoxelCollisionScene {
    let options = SurfaceMeshOptions {
        mode: SurfaceMode::DualContouring,
        materials: SurfaceMaterials::new([(
            1,
            MaterialSurface {
                mode: SurfaceMode::DualContouring,
                character: SurfaceCharacter {
                    placement: VertexPlacement::Smooth,
                    crease_angle_degrees: 40.0,
                    roughness: 0.0,
                },
            },
        )])
        .unwrap(),
        ..SurfaceMeshOptions::default()
    };
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        8,
        voxels.into_iter().map(|address| MaterialVoxel {
            state: 0,
            address,
            material_slot: 1,
        }),
        options,
    )
    .unwrap()
}

fn config() -> SurfaceNavigationConfig {
    SurfaceNavigationConfig {
        cell_size: 1.0,
        crawler: CrawlerConfig::default(),
    }
}

/// The room's inside, in 1 m cells.
fn inside() -> SurfaceRegion {
    SurfaceRegion {
        min: VoxelCoord::new(-3, 1, -3),
        max: VoxelCoord::new(2, 4, 2),
    }
}

fn cell(x: i64, y: i64, z: i64, face: SurfaceFace) -> SurfaceCell {
    SurfaceCell {
        cell: VoxelCoord::new(x, y, z),
        face,
    }
}

/// The surface cells reachable from `from`.
fn reachable(navigation: &SurfaceNavigation, from: SurfaceCell) -> BTreeSet<SurfaceCell> {
    let start = navigation.index[&from];
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(at) = queue.pop_front() {
        for edge in navigation.edges.iter().filter(|edge| edge.from == at) {
            if seen.insert(edge.to) {
                queue.push_back(edge.to);
            }
        }
    }
    seen.into_iter()
        .map(|slot| navigation.nodes[slot as usize].cell)
        .collect()
}

#[test]
fn a_cube_room_has_a_surface_cell_on_every_inner_face_all_joined() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, room_voxels(1.0)).unwrap();
    let navigation = derive_surface_navigation(&scene, &config(), inside()).unwrap();
    // 36 floor, 36 ceiling and four walls of 6 by 4.
    assert_eq!(navigation.nodes.len(), 36 + 36 + 4 * 24);
    for node in &navigation.nodes {
        let expected = match node.cell.face {
            SurfaceFace::PositiveY => CrawlerSurface::Floor,
            SurfaceFace::NegativeY => CrawlerSurface::Ceiling,
            _ => CrawlerSurface::Wall,
        };
        assert_eq!(node.surface, expected, "{node:?}");
        let normal = node.cell.face.normal();
        assert!((node.normal.x - normal.x).abs() < 1.0e-9);
        assert!((node.normal.y - normal.y).abs() < 1.0e-9);
        assert!((node.normal.z - normal.z).abs() < 1.0e-9);
    }
    let floor = navigation.index[&cell(0, 1, 0, SurfaceFace::PositiveY)];
    let rest = navigation.nodes[floor as usize].rest;
    assert!((rest.y - 1.27).abs() < 1.0e-6, "{rest:?}");
    let all = reachable(&navigation, cell(0, 1, 0, SurfaceFace::PositiveY));
    assert_eq!(all.len(), navigation.nodes.len());
    // Floor into wall and wall into ceiling are concave edges in one cell.
    let kinds: BTreeSet<_> = navigation.edges.iter().map(|edge| edge.kind).collect();
    assert_eq!(
        kinds,
        BTreeSet::from([SurfaceEdgeKind::Along, SurfaceEdgeKind::Concave])
    );
}

#[test]
fn a_dual_contoured_room_joins_floor_walls_and_ceiling() {
    let scene = dual_contoured(room_voxels(1.0));
    let navigation = derive_surface_navigation(&scene, &config(), inside()).unwrap();
    let floor = cell(0, 1, 0, SurfaceFace::PositiveY);
    let ceiling = cell(0, 4, 0, SurfaceFace::NegativeY);
    let wall = cell(2, 2, 0, SurfaceFace::NegativeX);
    for (at, surface) in [
        (floor, CrawlerSurface::Floor),
        (ceiling, CrawlerSurface::Ceiling),
        (wall, CrawlerSurface::Wall),
    ] {
        let node = navigation.nodes[navigation.index[&at] as usize];
        assert_eq!(node.surface, surface, "{node:?}");
    }
    let joined = reachable(&navigation, floor);
    assert!(joined.contains(&ceiling) && joined.contains(&wall));
}

/// The room with a trench two cells wide across x = −1..1 cut through the
/// floor and both side walls up to the ceiling, which alone spans it.
fn trench_voxels() -> Vec<[i64; 3]> {
    room_voxels(1.0)
        .into_iter()
        .filter(|&[x, y, _]| !((-1..1).contains(&x) && y < 5))
        .collect()
}

#[test]
fn across_a_trench_the_floors_join_only_over_the_ceiling() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, trench_voxels()).unwrap();
    let navigation = derive_surface_navigation(&scene, &config(), inside()).unwrap();
    let start = cell(-3, 1, 0, SurfaceFace::PositiveY);
    let goal = cell(2, 1, 0, SurfaceFace::PositiveY);
    assert!(!navigation
        .index
        .contains_key(&cell(-1, 1, 0, SurfaceFace::PositiveY)));
    assert!(reachable(&navigation, start).contains(&goal));
    // Without the ceiling over the trench, the halves are apart.
    let open: Vec<_> = trench_voxels()
        .into_iter()
        .filter(|&[x, y, _]| !((-1..1).contains(&x) && y == 5))
        .collect();
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, open).unwrap();
    let navigation = derive_surface_navigation(&scene, &config(), inside()).unwrap();
    assert!(!reachable(&navigation, start).contains(&goal));
}

/// A step a metre high for x ≥ 0 on a floor at y = 1, of half-metre voxels,
/// and with `beam` a bar of voxels half a metre over its lip.
fn step_voxels(beam: bool) -> Vec<[i64; 3]> {
    let mut voxels = Vec::new();
    for x in -8..8 {
        for z in -4..4 {
            voxels.push([x, 1, z]);
            if x >= 0 {
                voxels.push([x, 2, z]);
                voxels.push([x, 3, z]);
            }
            if beam && x == -1 {
                voxels.push([x, 5, z]);
            }
        }
    }
    voxels
}

#[test]
fn a_crawler_rounds_a_convex_lip_unless_a_bar_over_it_blocks_the_corner() {
    let config = config();
    let region = SurfaceRegion {
        min: VoxelCoord::new(-3, 0, -1),
        max: VoxelCoord::new(2, 3, 1),
    };
    let top = cell(0, 2, 0, SurfaceFace::PositiveY);
    let side = cell(-1, 1, 0, SurfaceFace::NegativeX);
    assert_eq!(surface_edge_kind(top, side), Some(SurfaceEdgeKind::Convex));
    for beam in [false, true] {
        let scene = VoxelCollisionScene::from_solid_voxels(0.5, 8, step_voxels(beam)).unwrap();
        let navigation = derive_surface_navigation(&scene, &config, region).unwrap();
        let node = |at: SurfaceCell| navigation.nodes[navigation.index[&at] as usize];
        let (from, to) = (node(top), node(side));
        assert!((from.rest.y - 2.27).abs() < 1.0e-6, "{from:?}");
        assert!((to.rest.x + 0.27).abs() < 1.0e-6, "{to:?}");
        let outcome = surface_edge(&scene, &config, &from, &to, SurfaceEdgeKind::Convex).unwrap();
        let joined = navigation.edges.iter().any(|edge| {
            edge.kind == SurfaceEdgeKind::Convex
                && navigation.nodes[edge.from as usize].cell == top
                && navigation.nodes[edge.to as usize].cell == side
        });
        if beam {
            let SurfaceEdgeOutcome::Blocked(block) = outcome else {
                panic!("the bar blocks the corner: {outcome:?}");
            };
            let corner = block.corner.expect("a convex edge names its corner");
            assert!(corner.x.abs() < 1.0e-6 && (corner.y - 2.0).abs() < 1.0e-6);
            assert!(block.point.y >= 2.5 - 1.0e-6, "{block:?}");
            assert!(!joined);
        } else {
            assert_eq!(outcome, SurfaceEdgeOutcome::Traversable);
            assert!(joined);
        }
    }
}

#[test]
fn a_cell_smaller_than_the_ball_and_its_skin_is_refused() {
    let config = SurfaceNavigationConfig {
        cell_size: 0.5,
        crawler: CrawlerConfig::default(),
    };
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, room_voxels(1.0)).unwrap();
    let error = derive_surface_navigation(&scene, &config, inside()).unwrap_err();
    assert_eq!(error.code(), "invalid-surface-navigation-config");
}

#[test]
fn no_surface_cell_lies_inside_dual_contoured_rock() {
    // Rolling ground eighteen metres deep: under its reconstructed surface
    // the deep voxels are cubes, which a line going on below the surface
    // meets inside the rock.
    let mut voxels = Vec::new();
    for x in -10..10_i64 {
        for z in -10..10_i64 {
            let ground = ((x * 7 + z * 3).rem_euclid(13) / 5) - 1;
            for y in -18..=ground {
                voxels.push([x, y, z]);
            }
        }
    }
    let scene = dual_contoured(voxels);
    let region = SurfaceRegion {
        min: VoxelCoord::new(-8, -16, -8),
        max: VoxelCoord::new(7, 4, 7),
    };
    let navigation = derive_surface_navigation(&scene, &config(), region).unwrap();
    assert!(!navigation.nodes.is_empty());
    for node in &navigation.nodes {
        let rest = [node.rest.x, node.rest.y, node.rest.z];
        assert!(!scene.contains_point(rest), "{node:?}");
        assert!(node.rest.y > -1.5, "a place under the ground: {node:?}");
    }
}

#[test]
fn a_ramp_inclined_diagonally_is_a_floor_crossed_from_corner_to_corner() {
    // A 49° ramp turned 45° about Y: its normal, about (0.534, 0.656, 0.534),
    // is more than 45° from every axis, and leans along Y most.
    let fall = (49.0_f64.to_radians()).tan() / 2.0_f64.sqrt();
    let height = |x: f64, z: f64| 6.0 - fall * (x + z);
    let corners = [(-3.0, -3.0), (3.0, -3.0), (-3.0, 3.0), (3.0, 3.0)];
    let positions: Vec<[f64; 3]> = corners.iter().map(|&(x, z)| [x, height(x, z), z]).collect();
    let asset =
        StaticMeshColliderAsset::new(StaticMeshAssetId(3), positions, vec![[0, 2, 1], [1, 2, 3]])
            .unwrap();
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[40, 40, 40]]).unwrap();
    scene
        .replace_static_mesh_colliders(
            [asset],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(3),
                asset: StaticMeshAssetId(3),
                transform: StaticMeshTransform::IDENTITY,
            }],
        )
        .unwrap();
    let region = SurfaceRegion {
        min: VoxelCoord::new(-3, 0, -3),
        max: VoxelCoord::new(2, 11, 2),
    };
    let navigation = derive_surface_navigation(&scene, &config(), region).unwrap();
    let floors: Vec<_> = navigation
        .nodes
        .iter()
        .filter(|node| node.cell.face == SurfaceFace::PositiveY)
        .collect();
    // One place on every column the ramp covers, and every one a floor.
    assert_eq!(floors.len(), 36, "{:?}", navigation.nodes.len());
    for node in &floors {
        assert_eq!(node.surface, CrawlerSurface::Floor, "{node:?}");
        assert!((node.normal.x - node.normal.z).abs() < 1.0e-6 && node.normal.y > 0.6);
    }
    let on = |x: i64, z: i64| {
        floors
            .iter()
            .find(|node| node.cell.cell.x == x && node.cell.cell.z == z)
            .expect("a place on the column")
            .cell
    };
    assert!(reachable(&navigation, on(2, 2)).contains(&on(-3, -3)));
}
