use super::*;
use svc_mesh::scatter::{surface_points, ChunkSurface, SurfaceSampling};

const FIELD: u64 = 1;
const GRASS: u32 = 3;

fn layout() -> VegetationLayout {
    VegetationLayout {
        tile_size: 16.0,
        tile_cells: 8,
    }
}

fn grass() -> VegetationLayer {
    VegetationLayer {
        mesh: "mesh/grass".into(),
        material_overrides: Vec::new(),
        shadow_casting: ShadowCasting::None,
        density: 1.0,
        grounds: 1,
        slope_limit_degrees: 45.0,
        align: 0.5,
        scale: [0.8, 1.2],
        tints: [[0.8; 3], [1.0; 3]],
        seed: 11,
        reach: 40.0,
        fade: 4.0,
        near: 0.0,
        near_fade: 0.0,
        maximum_instances: 1_000_000,
        lods: Vec::new(),
        lod_band: 0.0,
        thinning: ScatterThinning::default(),
    }
}

fn fields(placements: u32) -> VegetationFields {
    let mut fields = VegetationFields::default();
    fields.create(FIELD, layout(), placements).unwrap();
    fields.set_layer(FIELD, GRASS, grass()).unwrap();
    fields
}

/// A tile's samples from a height function of world (x, z).
fn sampled(request: &VegetationRequest, height: impl Fn(f64, f64) -> f32) -> VegetationTileContent {
    let cells = layout().tile_cells as i64;
    let cell = layout().tile_size / cells as f64;
    let mut heights = Vec::new();
    for j in -1..=cells + 1 {
        for i in -1..=cells + 1 {
            heights.push(height(
                request.origin[0] + i as f64 * cell,
                request.origin[1] + j as f64 * cell,
            ));
        }
    }
    VegetationTileContent {
        samples: TileSamples {
            heights,
            ..TileSamples::default()
        },
        cover: None,
        records: Vec::new(),
    }
}

/// Answer every request with `height`, then settle at `focus`.
fn fill(
    fields: &mut VegetationFields,
    focus: [f64; 3],
    height: impl Fn(f64, f64) -> f32,
) -> Vec<RenderDiff> {
    fields.settle(FIELD, [0.0; 3], Some(focus)).unwrap();
    for request in fields.requests(FIELD, 0) {
        let content = sampled(&request, &height);
        assert!(fields
            .commit(FIELD, (request.x, request.z), request.revision, content)
            .unwrap());
    }
    fields.settle(FIELD, [0.0; 3], Some(focus)).unwrap()
}

fn patches(operations: &[RenderDiff]) -> Vec<&ScatterPatchDescriptor> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            RenderDiff::CreateScatterPatch { patch, .. } => Some(patch),
            _ => None,
        })
        .collect()
}

fn destroyed(operations: &[RenderDiff]) -> usize {
    operations
        .iter()
        .filter(|operation| matches!(operation, RenderDiff::Destroy { .. }))
        .count()
}

#[test]
fn tiles_in_reach_are_asked_for_nearest_first_and_grow_their_density() {
    let mut fields = fields(1_000);
    fields
        .settle(FIELD, [0.0; 3], Some([8.0, 2.0, 8.0]))
        .unwrap();
    let requests = fields.requests(FIELD, 0);
    assert_eq!((requests[0].x, requests[0].z), (0, 0));
    let distance = |request: &VegetationRequest| {
        let gap = |low: f64| (low - 8.0).max(8.0 - (low + 16.0)).max(0.0);
        gap(request.origin[0]).hypot(gap(request.origin[1]))
    };
    assert!(requests
        .windows(2)
        .all(|pair| distance(&pair[0]) <= distance(&pair[1])));
    // Within the reach, widened by a selection step (a sixteenth of a tile).
    assert!(requests
        .iter()
        .all(|request| distance(request) <= 40.0 + 16.0 * VEGETATION_SELECTION_STEP));
    assert_eq!(fields.requests(FIELD, 3).len(), 3);

    let operations = fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    let readout = fields.readout(FIELD).unwrap();
    assert_eq!(readout.pending_tiles, 0);
    assert_eq!(readout.patches as usize, patches(&operations).len());
    // A whole flat tile at one copy per square metre grows about its area.
    let tile = fields.patch_handle(FIELD, (0, 0), GRASS).unwrap();
    let held = operations
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::CreateScatterPatch { handle, patch, .. } if *handle == tile => Some(patch),
            _ => None,
        })
        .unwrap();
    assert!(
        (230..=280).contains(&held.instances.len()),
        "{}",
        held.instances.len()
    );
    assert!(held
        .instances
        .iter()
        .all(|copy| (copy.translation[1] - 2.0).abs() < 1e-5
            && (0.0..=16.0).contains(&copy.translation[0])
            && (0.0..=16.0).contains(&copy.translation[2])));
    assert_eq!(held.fade.end, 40.0);
    assert_eq!(held.fade.start, 36.0);
    assert_eq!(readout.placed, readout.patches);
    // Nothing changed: nothing more to do.
    let again = fields
        .settle(FIELD, [0.0; 3], Some([8.0, 2.0, 8.0]))
        .unwrap();
    assert!(again.is_empty(), "{again:?}");
    assert_eq!(fields.readout(FIELD).unwrap().placed, 0);
}

#[test]
fn a_layer_grows_the_spots_voxel_scatter_grows_on_the_same_ground() {
    // Voxel scatter's spots on a flat square of ground with the same seed
    // and density: the same x and z, so one can hand copies to the other.
    let (positions, indices) = (
        vec![
            0.0, 2.0, 0.0, 0.0, 2.0, 16.0, 16.0, 2.0, 16.0, 16.0, 2.0, 0.0,
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    let surface = ChunkSurface {
        positions: &positions,
        indices: &indices,
        groups: &[(0, 0, 6)],
        origin: [0.0; 3],
        band: 1.0,
    };
    let mut voxel: Vec<[u32; 2]> = surface_points(
        &surface,
        &SurfaceSampling {
            density: 1.0,
            seed: 11,
            minimum_normal_y: 0.5,
        },
    )
    .iter()
    .map(|point| [point.position[0].to_bits(), point.position[2].to_bits()])
    .collect();
    let mut fields = fields(1_000);
    let operations = fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    let tile = fields.patch_handle(FIELD, (0, 0), GRASS).unwrap();
    let mut grown: Vec<[u32; 2]> = operations
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::CreateScatterPatch { handle, patch, .. } if *handle == tile => Some(patch),
            _ => None,
        })
        .unwrap()
        .instances
        .iter()
        .map(|copy| [copy.translation[0].to_bits(), copy.translation[2].to_bits()])
        .collect();
    voxel.sort_unstable();
    grown.sort_unstable();
    // The square's far edges hold spots voxel scatter keeps and the next
    // tile owns; within the tile they agree.
    voxel.retain(|spot| f32::from_bits(spot[0]) < 16.0 && f32::from_bits(spot[1]) < 16.0);
    assert_eq!(voxel, grown);
}

#[test]
fn copies_stand_on_the_surface_a_heightfield_of_the_lattice_draws() {
    let hills = |x: f64, z: f64| (2.0 + (x * 0.3).sin() * 1.5 + (z * 0.2).cos()) as f32;
    let mut fields = fields(1_000);
    let operations = fill(&mut fields, [8.0, 2.0, 8.0], hills);
    let request = VegetationRequest {
        x: 0,
        z: 0,
        revision: 0,
        origin: [0.0, 0.0],
    };
    let heightfield = fields.fields[&FIELD].heightfield.clone();
    let surface = TileSurface::new(&heightfield, 0, sampled(&request, hills).samples);
    let tile = fields.patch_handle(FIELD, (0, 0), GRASS).unwrap();
    let copies = operations
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::CreateScatterPatch { handle, patch, .. } if *handle == tile => Some(patch),
            _ => None,
        })
        .unwrap();
    assert!(!copies.instances.is_empty());
    for copy in &copies.instances {
        let [x, y, z] = copy.translation;
        let (drawn, _) = surface.surface(f64::from(x), f64::from(z)).unwrap();
        assert!((y - drawn).abs() < 1e-4, "{y} {drawn}");
    }
}

#[test]
fn an_edit_samples_and_places_again_only_the_tiles_it_touches() {
    let mut fields = fields(1_000);
    fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    let before = fields.readout(FIELD).unwrap();
    let edited = fields.patch_handle(FIELD, (1, 0), GRASS).unwrap();
    fields.invalidate(FIELD, [20.0, 4.0], [22.0, 6.0]).unwrap();
    let requests = fields.requests(FIELD, 0);
    assert_eq!(requests.len(), 1);
    assert_eq!((requests[0].x, requests[0].z), (1, 0));
    // Until the new samples land, the tile draws what it had.
    let waiting = fields.settle(FIELD, [0.0; 3], None).unwrap();
    assert!(waiting.is_empty());
    assert_eq!(fields.readout(FIELD).unwrap().pending_tiles, 1);
    let content = sampled(&requests[0], |x, _| if x < 24.0 { 3.0 } else { 2.0 });
    assert!(fields
        .commit(FIELD, (1, 0), requests[0].revision, content)
        .unwrap());
    let operations = fields.settle(FIELD, [0.0; 3], None).unwrap();
    assert_eq!(destroyed(&operations), 1);
    assert_eq!(patches(&operations).len(), 1);
    assert!(operations
        .iter()
        .any(|operation| matches!(operation, RenderDiff::Destroy { handle } if *handle == edited)));
    let after = fields.readout(FIELD).unwrap();
    assert_eq!(after.placed, 1);
    assert_eq!(after.invalidated_tiles, 1);
    assert_eq!(after.replaced_patches, 1);
    assert_eq!(after.patches, before.patches);
}

#[test]
fn a_commit_for_an_old_revision_is_ignored() {
    let mut fields = fields(1_000);
    fields
        .settle(FIELD, [0.0; 3], Some([8.0, 2.0, 8.0]))
        .unwrap();
    let request = fields.requests(FIELD, 1)[0];
    fields.invalidate(FIELD, [0.0, 0.0], [1.0, 1.0]).unwrap();
    let content = sampled(&request, |_, _| 2.0);
    assert!(!fields
        .commit(
            FIELD,
            (request.x, request.z),
            request.revision,
            content.clone()
        )
        .unwrap());
    let renewed = fields.requests(FIELD, 1)[0];
    assert_ne!(renewed.revision, request.revision);
    assert!(fields
        .commit(FIELD, (renewed.x, renewed.z), renewed.revision, content)
        .unwrap());
}

#[test]
fn a_layer_budget_goes_to_the_nearest_tiles_and_the_rest_are_counted() {
    let mut fields = fields(1_000);
    fields
        .set_layer(
            FIELD,
            GRASS,
            VegetationLayer {
                maximum_instances: 600,
                ..grass()
            },
        )
        .unwrap();
    fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    let readout = fields.readout(FIELD).unwrap();
    assert!(readout.instances <= 600);
    assert!(readout.over_budget > 0);
    assert!(fields.patch_handle(FIELD, (0, 0), GRASS).is_some());
    assert!(fields.patch_handle(FIELD, (-2, -2), GRASS).is_none());
}

#[test]
fn placement_beyond_the_settle_budget_waits_and_is_counted() {
    let mut fields = fields(2);
    fields
        .settle(FIELD, [0.0; 3], Some([8.0, 2.0, 8.0]))
        .unwrap();
    for request in fields.requests(FIELD, 0) {
        let content = sampled(&request, |_, _| 2.0);
        fields
            .commit(FIELD, (request.x, request.z), request.revision, content)
            .unwrap();
    }
    let first = fields.settle(FIELD, [0.0; 3], None).unwrap();
    assert_eq!(patches(&first).len(), 2);
    let readout = fields.readout(FIELD).unwrap();
    assert_eq!(readout.placed, 2);
    assert!(readout.deferred > 0);
    let mut settles = 1;
    while fields.readout(FIELD).unwrap().deferred > 0 {
        fields.settle(FIELD, [0.0; 3], None).unwrap();
        settles += 1;
        assert!(settles < 100);
    }
    assert_eq!(
        fields.readout(FIELD).unwrap().patches as usize,
        fields.fields[&FIELD].tiles.len()
    );
}

#[test]
fn ground_classes_and_cover_choose_and_thin_the_spots() {
    let mut fields = fields(1_000);
    fields
        .settle(FIELD, [0.0; 3], Some([8.0, 2.0, 8.0]))
        .unwrap();
    let side = (layout().tile_cells + 1) as usize;
    for request in fields.requests(FIELD, 0) {
        let mut content = sampled(&request, |_, _| 2.0);
        // Tile (0, 0) is stone (class 1) on its left half; tile (1, 0) has
        // half cover; tile (0, 1) none.
        if (request.x, request.z) == (0, 0) {
            content.samples.materials = Some(
                (0..side * side)
                    .map(|at| u32::from(at % side < 4))
                    .collect(),
            );
        }
        if (request.x, request.z) == (1, 0) {
            content.cover = Some(vec![0.5; side * side]);
        }
        if (request.x, request.z) == (0, 1) {
            content.cover = Some(vec![0.0; side * side]);
        }
        fields
            .commit(FIELD, (request.x, request.z), request.revision, content)
            .unwrap();
    }
    let operations = fields.settle(FIELD, [0.0; 3], None).unwrap();
    let copies = |tile: (i64, i64)| {
        fields.patch_handle(FIELD, tile, GRASS).map_or(0, |wanted| {
            operations
                .iter()
                .find_map(|operation| match operation {
                    RenderDiff::CreateScatterPatch { handle, patch, .. } if *handle == wanted => {
                        Some(patch.instances.len())
                    }
                    _ => None,
                })
                .unwrap()
        })
    };
    let whole = copies((-1, 0));
    assert!(
        (60..=200).contains(&(whole - copies((0, 0)))),
        "{whole} {}",
        copies((0, 0))
    );
    let half = copies((1, 0));
    assert!(
        half > whole / 3 && half < whole * 2 / 3,
        "{half} of {whole}"
    );
    assert_eq!(copies((0, 1)), 0);
    assert!(fields.patch_handle(FIELD, (0, 1), GRASS).is_none());
}

#[test]
fn records_stand_in_their_layer_as_given() {
    let mut fields = fields(1_000);
    fields
        .set_layer(
            FIELD,
            7,
            VegetationLayer {
                density: 0.0,
                align: 0.0,
                ..grass()
            },
        )
        .unwrap();
    fields
        .settle(FIELD, [0.0; 3], Some([8.0, 2.0, 8.0]))
        .unwrap();
    for request in fields.requests(FIELD, 0) {
        let mut content = sampled(&request, |_, _| 2.0);
        if (request.x, request.z) == (0, 0) {
            content.records = vec![
                VegetationRecord {
                    layer: 7,
                    position: [3.0, 2.0, 5.0],
                    yaw: std::f32::consts::FRAC_PI_2,
                    scale: 2.5,
                },
                VegetationRecord {
                    layer: 99,
                    position: [4.0, 2.0, 5.0],
                    yaw: 0.0,
                    scale: 1.0,
                },
            ];
        }
        fields
            .commit(FIELD, (request.x, request.z), request.revision, content)
            .unwrap();
    }
    let operations = fields.settle(FIELD, [0.0; 3], None).unwrap();
    let tree = fields.patch_handle(FIELD, (0, 0), 7).unwrap();
    let placed = operations
        .iter()
        .find_map(|operation| match operation {
            RenderDiff::CreateScatterPatch { handle, patch, .. } if *handle == tree => Some(patch),
            _ => None,
        })
        .unwrap();
    assert_eq!(placed.instances.len(), 1);
    let copy = placed.instances[0];
    assert_eq!(copy.translation, [3.0, 2.0, 5.0]);
    assert_eq!(copy.scale, 2.5);
    let half = std::f32::consts::FRAC_PI_4;
    assert!((copy.rotation[1] - half.sin()).abs() < 1e-6);
    assert!((copy.rotation[3] - half.cos()).abs() < 1e-6);
    // A record of a layer the field lacks grows nothing.
    assert!(fields.patch_handle(FIELD, (1, 0), 7).is_none());
}

#[test]
fn moving_away_drops_far_tiles_and_asks_for_new_ones() {
    let mut fields = fields(1_000);
    fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    let held = fields.fields[&FIELD].tiles.len();
    let operations = fields
        .settle(FIELD, [0.0; 3], Some([200.0, 2.0, 8.0]))
        .unwrap();
    assert_eq!(destroyed(&operations), held);
    let requests = fields.requests(FIELD, 0);
    assert!(!requests.is_empty());
    assert!(requests.iter().all(|request| request.x >= 9));
    // Coming back within the hysteresis keeps what it holds.
    let mut fields = self::fields(1_000);
    fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    let operations = fields
        .settle(FIELD, [0.0; 3], Some([10.0, 2.0, 8.0]))
        .unwrap();
    assert_eq!(destroyed(&operations), 0);
}

#[test]
fn the_near_band_skips_tiles_the_cut_hides_and_fades_in_past_it() {
    let mut fields = fields(1_000);
    fields
        .set_layer(
            FIELD,
            GRASS,
            VegetationLayer {
                near: 20.0,
                near_fade: 2.0,
                ..grass()
            },
        )
        .unwrap();
    let operations = fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    // The focus's own tile lies wholly within the cut.
    assert!(fields.patch_handle(FIELD, (0, 0), GRASS).is_none());
    let created = patches(&operations);
    assert!(!created.is_empty());
    assert!(created
        .iter()
        .all(|patch| patch.fade.near_start == 20.0 && patch.fade.near_end == 22.0));
    // From above, every copy is past the cut: the near tile grows.
    let operations = fields
        .settle(FIELD, [0.0; 3], Some([8.0, 24.0, 8.0]))
        .unwrap();
    assert!(fields.patch_handle(FIELD, (0, 0), GRASS).is_some());
    assert!(!patches(&operations).is_empty());
}

#[test]
fn removing_a_layer_or_the_field_removes_what_it_drew() {
    let mut fields = fields(1_000);
    fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    let drawn = fields.readout(FIELD).unwrap().patches as usize;
    let operations = fields.remove_layer(FIELD, GRASS).unwrap();
    assert_eq!(destroyed(&operations), drawn);
    let operations = fields.destroy(FIELD);
    assert_eq!(destroyed(&operations), 1);
    assert!(fields.readout(FIELD).is_none());
}

#[test]
fn refuses_bad_layers_and_samples() {
    assert_eq!(
        VegetationLayer {
            near: 30.0,
            near_fade: 8.0,
            ..grass()
        }
        .validate(),
        Err(VegetationLayerError::Distances)
    );
    assert_eq!(
        VegetationLayer {
            density: 100.0,
            ..grass()
        }
        .validate(),
        Err(VegetationLayerError::Density)
    );
    let mut fields = fields(1_000);
    fields
        .settle(FIELD, [0.0; 3], Some([8.0, 2.0, 8.0]))
        .unwrap();
    let request = fields.requests(FIELD, 1)[0];
    let mut content = sampled(&request, |_, _| 2.0);
    content.samples.heights.pop();
    assert!(matches!(
        fields.commit(FIELD, (request.x, request.z), request.revision, content),
        Err(VegetationError::SampleCount { .. })
    ));
    let mut content = sampled(&request, |_, _| 2.0);
    content.samples.materials = Some(vec![64; 81]);
    assert_eq!(
        fields.commit(FIELD, (request.x, request.z), request.revision, content),
        Err(VegetationError::Value)
    );
}

#[test]
fn a_rebase_moves_the_root_and_nothing_else() {
    let mut fields = fields(1_000);
    fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    // The world's origin moved 1000 m: the root stands at −1000 in the
    // scene and the focus is the same world point.
    let operations = fields
        .settle(FIELD, [-1000.0, 0.0, 0.0], Some([8.0, 2.0, 8.0]))
        .unwrap();
    assert_eq!(operations.len(), 1);
    assert!(matches!(
        &operations[0],
        RenderDiff::Update { transform: Some(transform), .. }
            if transform.translation == [-1000.0, 0.0, 0.0]
    ));
}

/// Fill every waiting tile flat and settle at `focus`, returning the field's
/// patches by tile after.
fn walk_to(fields: &mut VegetationFields, focus: [f64; 3]) -> Vec<RenderDiff> {
    let mut operations = fields.settle(FIELD, [0.0; 3], Some(focus)).unwrap();
    for request in fields.requests(FIELD, 0) {
        let content = sampled(&request, |_, _| 2.0);
        fields
            .commit(FIELD, (request.x, request.z), request.revision, content)
            .unwrap();
    }
    operations.extend(fields.settle(FIELD, [0.0; 3], Some(focus)).unwrap());
    operations
}

fn held(fields: &VegetationFields) -> std::collections::BTreeSet<(i64, i64)> {
    (-8..8)
        .flat_map(|x| (-8..8).map(move |z| (x, z)))
        .filter(|&tile| fields.patch_handle(FIELD, tile, GRASS).is_some())
        .collect()
}

#[test]
fn a_settle_with_nothing_changed_chooses_nothing_again() {
    let mut fields = fields(1_000);
    fill(&mut fields, [8.0, 2.0, 8.0], |_, _| 2.0);
    let before = fields.readout(FIELD).unwrap();
    // Still, and moving within a step: nothing to do.
    for focus in [[8.0, 2.0, 8.0], [8.5, 2.0, 8.3]] {
        let operations = fields.settle(FIELD, [0.0; 3], Some(focus)).unwrap();
        assert!(operations.is_empty(), "{operations:?}");
    }
    let still = fields.readout(FIELD).unwrap();
    assert_eq!(still.settles_skipped, before.settles_skipped + 2);
    assert_eq!(still.settles_reselected, before.settles_reselected);
    assert_eq!(
        (still.patches, still.instances),
        (before.patches, before.instances)
    );
    // Past a step it chooses again; so it does after a change.
    fields
        .settle(FIELD, [0.0; 3], Some([10.0, 2.0, 8.0]))
        .unwrap();
    assert_eq!(
        fields.readout(FIELD).unwrap().settles_reselected,
        before.settles_reselected + 1
    );
    fields.invalidate(FIELD, [0.0, 0.0], [1.0, 1.0]).unwrap();
    fields
        .settle(FIELD, [0.0; 3], Some([10.0, 2.0, 8.0]))
        .unwrap();
    assert_eq!(
        fields.readout(FIELD).unwrap().settles_reselected,
        before.settles_reselected + 2
    );
    let mut sparser = grass();
    sparser.density *= 0.5;
    fields.set_layer(FIELD, GRASS, sparser).unwrap();
    fields
        .settle(FIELD, [0.0; 3], Some([10.0, 2.0, 8.0]))
        .unwrap();
    assert_eq!(
        fields.readout(FIELD).unwrap().settles_reselected,
        before.settles_reselected + 3
    );
}

#[test]
fn a_field_walked_in_small_steps_holds_what_a_field_settled_there_holds() {
    // Walked across 30 m in half-step moves, the field holds every tile
    // layer a fresh field settled at the end holds, and nothing past the
    // widened reach.
    let mut walked = fields(1_000);
    let mut fresh = fields(1_000);
    let end = [38.0, 2.0, 8.0];
    let mut x = 8.0;
    while x < end[0] {
        walk_to(&mut walked, [x, 2.0, 8.0]);
        x += 0.5;
    }
    walk_to(&mut walked, end);
    walk_to(&mut fresh, end);
    let (walked, fresh_tiles) = (held(&walked), held(&fresh));
    assert!(
        fresh_tiles.is_subset(&walked),
        "fresh {fresh_tiles:?}, walked {walked:?}"
    );
    let reach = f64::from(grass().reach) * VEGETATION_HYSTERESIS + 16.0 * VEGETATION_SELECTION_STEP;
    let retained = &fresh.fields[&FIELD];
    for tile in &walked {
        assert!(retained.flat_distance(end, *tile) <= reach, "{tile:?}");
    }
}
