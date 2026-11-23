use super::*;
use crate::{StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance};

/// Rock filling 0..20 × 0..6 × 0..20 but where `open`.
fn rock(open: impl Fn(i64, i64, i64) -> bool) -> VoxelCollisionScene {
    let mut solids = Vec::new();
    for x in 0..20 {
        for y in 0..6 {
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
/// turning along +z out of the rock at z = 20.
fn bending_passage() -> VoxelCollisionScene {
    rock(|x, y, z| {
        (1..3).contains(&y)
            && (((2..16).contains(&x) && (2..4).contains(&z))
                || ((14..16).contains(&x) && (2..20).contains(&z)))
    })
}

/// Two rooms, x 1..9 and 11..19, parted by rock two metres thick: where
/// `doorway`, a doorway through it at z 15..17, y 1..4; without `wall`, no
/// rock between them at all.
fn two_rooms(doorway: bool, wall: bool) -> VoxelCollisionScene {
    rock(|x, y, z| {
        let room = (1..5).contains(&y) && (1..19).contains(&z);
        let side = (1..9).contains(&x) || (11..19).contains(&x);
        let door = doorway && (9..11).contains(&x) && (15..17).contains(&z) && (1..4).contains(&y);
        let gap = !wall && (9..11).contains(&x);
        room && (side || door || gap)
    })
}

type Mesh = (StaticMeshColliderAsset, StaticMeshColliderInstance);

/// Closed boxes, each `[min, max]`, as one mesh wound outward.
fn box_mesh(id: u64, boxes: &[[[f64; 3]; 2]]) -> Mesh {
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
    (
        StaticMeshColliderAsset::new(StaticMeshAssetId(id), positions, triangles).unwrap(),
        StaticMeshColliderInstance {
            id: StaticMeshInstanceId(id),
            asset: StaticMeshAssetId(id),
            transform: Default::default(),
        },
    )
}

fn place(scene: &mut VoxelCollisionScene, meshes: Vec<Mesh>) {
    let (assets, instances): (Vec<_>, Vec<_>) = meshes.into_iter().unzip();
    scene
        .replace_static_mesh_colliders(assets, instances)
        .unwrap();
}

fn found(routes: &AirRoutes, index: usize) -> &AirRoute {
    routes.routes[index]
        .as_ref()
        .unwrap_or_else(|failure| panic!("no route: {failure:?} in {routes:?}"))
}

#[test]
fn a_route_follows_a_bending_passage_across_chunks() {
    let scene = bending_passage();
    let start = [3.0, 2.0, 3.0];
    let end = [15.0, 2.0, 17.5];
    assert!(!scene.segment_clear(start, end));
    let graph = AirGraph::around(&scene, &[start, end], 24.0);
    let routes = graph.route(&scene, start, &[end], 60.0);
    let route = found(&routes, 0);
    // Round the inside corner at (14, 4) the path is about 25 m, against
    // 19 m straight through the rock.
    assert!((24.0..28.0).contains(&route.metres), "{route:?}");
    // It leaves the start along +x and arrives at the end along +z.
    let leaves = route.near_start();
    let arrives = route.near_end();
    assert!(leaves[0] > start[0] + 1.5, "{leaves:?}");
    assert!(arrives[2] < end[2] - 1.5, "{arrives:?}");
    assert!(!routes.incomplete);
    // Either way round it is as long.
    let back = graph.route(&scene, end, &[start], 60.0);
    assert!(
        (found(&back, 0).metres - route.metres).abs() < 1e-4,
        "{back:?}"
    );
}

#[test]
fn a_sealed_room_is_unreachable_and_an_unbuilt_one_is_not_known() {
    let scene = rock(|x, y, z| {
        (1..3).contains(&y)
            && (((2..12).contains(&x) && (2..4).contains(&z))
                || ((14..16).contains(&x) && (8..16).contains(&z)))
    });
    let (start, end) = ([3.0, 2.0, 3.0], [15.0, 2.0, 15.0]);
    let graph = AirGraph::around(&scene, &[start, end], 24.0);
    let routes = graph.route(&scene, start, &[end], 60.0);
    assert_eq!(routes.routes[0], Err(AirRouteFailure::Unreachable));
    let only_start =
        AirGraph::default().with_chunk([0, 0, 0], Arc::new(AirChunk::build(&scene, [0, 0, 0])));
    let far = [15.0, 2.0, 40.0];
    assert_eq!(
        only_start.route(&scene, start, &[far], 60.0).routes[0],
        Err(AirRouteFailure::NotBuilt)
    );
}

#[test]
fn a_pocket_holds_its_air_and_opens_into_the_next_chunk() {
    let scene = bending_passage();
    let graph = AirGraph::around(&scene, &[[8.0, 2.0, 8.0]], 20.0);
    let id = graph.pocket_at([3.0, 2.0, 3.0]).expect("in the passage");
    assert_eq!(id.chunk, [0, 0, 0]);
    let pocket = graph.pocket(id).unwrap();
    // 14 × 2 × 2 m along x and 2 × 12 × 2 m on along z within the chunk.
    assert!((pocket.volume_m3 - 104.0).abs() < 1e-9, "{pocket:?}");
    assert_eq!(pocket.min, [2.0, 1.0, 2.0]);
    assert_eq!(pocket.max, [16.0, 3.0, 16.0]);
    let openings = graph.openings(id);
    assert_eq!(openings.len(), 1, "{openings:?}");
    assert!((openings[0].area_m2 - 4.0).abs() < 1e-9, "{openings:?}");
    assert_eq!(openings[0].leads_to.len(), 1);
    assert_eq!(openings[0].leads_to[0].chunk, [0, 0, 1]);
    assert!(graph.pocket_at([0.5, 0.5, 0.5]).is_none(), "rock");
    // A chunk with nothing in it is one pocket of open air.
    let open = AirGraph::around(&scene, &[[100.0, 100.0, 100.0]], 1.0);
    let sky = open.pocket_at([100.0, 100.0, 100.0]).unwrap();
    assert!(open.chunk(sky.chunk).unwrap().is_open());
    assert!((open.pocket(sky).unwrap().volume_m3 - 4096.0).abs() < 1e-9);
}

#[test]
fn a_thin_door_closes_a_doorway_and_opening_it_rebuilds_only_its_chunks() {
    let mut scene = two_rooms(true, true);
    place(
        &mut scene,
        vec![box_mesh(1, &[[[9.95, 1.0, 15.0], [10.05, 4.0, 17.0]]])],
    );
    let (source, target) = ([5.0, 2.0, 3.0], [15.0, 2.0, 3.0]);
    let mut watch = AirSceneWatch::default();
    assert_eq!(
        watch.observe(&scene),
        AirSceneChange::Everything { moved: true }
    );
    let mut graph = AirGraph::around(&scene, &[source, target], 24.0);
    let built = graph.chunk_count();
    assert_eq!(
        graph.route(&scene, source, &[target], 60.0).routes[0],
        Err(AirRouteFailure::Unreachable),
        "a door thinner than a cell parts the rooms"
    );

    // The door goes.
    place(&mut scene, Vec::new());
    let change = watch.observe(&scene);
    assert!(matches!(change, AirSceneChange::Regions(_)), "{change:?}");
    let touched: Vec<AirChunkKey> = graph
        .chunk_keys()
        .filter(|key| change.reaches(*key))
        .collect();
    assert!(
        !touched.is_empty() && touched.len() <= 2,
        "{touched:?} of {built}"
    );
    for key in touched {
        graph = graph.with_chunk(key, Arc::new(AirChunk::build(&scene, key)));
    }
    let routes = graph.route(&scene, source, &[target], 60.0);
    let route = found(&routes, 0);
    // Out to the doorway at (10, 16) and back: about 27 m.
    assert!((26.0..31.0).contains(&route.metres), "{route:?}");
    assert!(route.near_start()[2] > source[2] + 1.5, "{route:?}");
}

#[test]
fn a_crack_wider_than_a_cell_stays_open() {
    // The rooms joined across their whole side, but for a static-mesh wall
    // with a 0.6 m slit off the cell grid.
    let mut scene = two_rooms(false, false);
    place(
        &mut scene,
        vec![box_mesh(
            1,
            &[
                [[9.0, 1.0, 1.0], [11.0, 5.0, 8.13]],
                [[9.0, 1.0, 8.73], [11.0, 5.0, 19.0]],
            ],
        )],
    );
    let (source, target) = ([5.0, 2.0, 3.0], [15.0, 2.0, 3.0]);
    let graph = AirGraph::around(&scene, &[source, target], 24.0);
    let routes = graph.route(&scene, source, &[target], 60.0);
    let route = found(&routes, 0);
    assert!((13.0..18.0).contains(&route.metres), "{route:?}");
}

#[test]
fn a_watch_names_where_collision_changed() {
    let mut scene = two_rooms(true, true);
    let mut watch = AirSceneWatch::default();
    watch.observe(&scene);
    assert_eq!(watch.observe(&scene), AirSceneChange::None);
    place(
        &mut scene,
        vec![box_mesh(7, &[[[3.0, 1.0, 3.0], [4.0, 2.0, 4.0]]])],
    );
    let change = watch.observe(&scene);
    assert_eq!(
        change,
        AirSceneChange::Regions(vec![([3.0, 1.0, 3.0], [4.0, 2.0, 4.0])])
    );
    assert!(change.reaches([0, 0, 0]));
    assert!(!change.reaches([1, 0, 0]));
}

#[test]
fn a_watch_names_the_chunks_a_voxel_edit_reaches() {
    let mut scene = two_rooms(true, true);
    let mut watch = AirSceneWatch::default();
    watch.observe(&scene);
    crate::VoxelEditService::apply(
        &mut scene,
        &[crate::VoxelEdit::Set {
            address: [3, 2, 3],
            material_slot: 1,
        }],
    )
    .unwrap();
    let change = watch.observe(&scene);
    assert!(matches!(change, AirSceneChange::Regions(_)), "{change:?}");
    assert!(change.reaches([0, 0, 0]));
    assert!(!change.reaches([2, 0, 0]) && !change.reaches([0, 0, 2]));
}
