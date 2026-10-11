use super::*;

fn ground(x: f64, z: f64) -> f32 {
    (10.0 * (x / 37.0).sin() + 7.0 * (z / 23.0).cos() + 0.01 * x) as f32
}

/// Two rings of 16-cell tiles: 2 m cells within 100 m, 8 m within 600 m.
fn two_rings() -> HeightfieldLayout {
    HeightfieldLayout::new(
        16,
        &[
            HeightfieldRingSpec {
                cell_size: 2.0,
                reach: 100.0,
            },
            HeightfieldRingSpec {
                cell_size: 8.0,
                reach: 600.0,
            },
        ],
    )
    .unwrap()
}

fn sample(
    layout: &HeightfieldLayout,
    key: TileKey,
    height: impl Fn(f64, f64) -> f32,
) -> TileSamples {
    let cell = layout.rings[key.ring as usize].cell_size;
    let origin = layout.origin(key);
    let side = layout.height_side() as i64;
    let heights = (0..side * side)
        .map(|index| {
            let (i, j) = (index % side - 1, index / side - 1);
            height(origin[0] + i as f64 * cell, origin[1] + j as f64 * cell)
        })
        .collect();
    TileSamples {
        heights,
        ..TileSamples::default()
    }
}

fn surface(layout: &HeightfieldLayout, key: TileKey) -> TileSurface {
    TileSurface::new(layout, key.ring as usize, sample(layout, key, ground))
}

#[test]
fn a_layout_nests_its_rings_and_refuses_what_cannot_morph() {
    let layout = two_rings();
    assert_eq!(layout.rings[0].tile_size, 32.0);
    assert_eq!(layout.rings[0].per_parent, 4);
    // 100 m in 128 m region tiles: one each side.
    assert_eq!(layout.rings[0].reach_tiles, 1);
    assert_eq!(layout.rings[0].morph, Some((64.0, 128.0)));
    assert_eq!(layout.rings[1].morph, None);
    let spec = |cell_size, reach| HeightfieldRingSpec { cell_size, reach };
    assert_eq!(
        HeightfieldLayout::new(16, &[spec(2.0, 100.0), spec(5.0, 600.0)]),
        Err(HeightfieldLayoutError::NotNested(1))
    );
    assert_eq!(
        HeightfieldLayout::new(16, &[spec(2.0, 400.0), spec(8.0, 300.0)]),
        Err(HeightfieldLayoutError::ReachTooShort(1))
    );
    assert_eq!(
        HeightfieldLayout::new(0, &[spec(2.0, 100.0)]),
        Err(HeightfieldLayoutError::TileCells)
    );
    assert_eq!(
        HeightfieldLayout::new(16, &[spec(2.0, f64::NAN)]),
        Err(HeightfieldLayoutError::Ring(0))
    );
    // A tile's ratio need only be whole; cells need not divide evenly.
    assert!(HeightfieldLayout::new(64, &[spec(31.25, 4000.0), spec(250.0, 40_000.0)]).is_ok());
}

/// Wherever the focus stands, a ring's region reaches at least its morph's
/// end from it, and the finer ring's region never reaches past the coarser
/// ring's morph start: tiles enter and leave fully morphed, and a finer
/// ring's edge meets its coarser ring unmorphed.
#[test]
fn every_focus_keeps_a_rings_edge_past_its_morph_and_inside_the_coarser_rings_whole_reach() {
    let spec = |cell_size, reach| HeightfieldRingSpec { cell_size, reach };
    let layout =
        HeightfieldLayout::new(16, &[spec(1.0, 40.0), spec(4.0, 300.0), spec(16.0, 3000.0)])
            .unwrap();
    for step in 0..400 {
        let focus = [step as f64 * 7.37 - 1400.0, step as f64 * -3.11 + 500.0];
        for ring in 0..layout.rings.len() {
            let region = layout.region(ring, focus);
            let size = layout.rings[ring].tile_size;
            let lo = region.min.map(|value| value as f64 * size);
            let hi = region.max.map(|value| (value + 1) as f64 * size);
            let nearest_edge = (0..2)
                .map(|axis| (focus[axis] - lo[axis]).min(hi[axis] - focus[axis]))
                .fold(f64::INFINITY, f64::min);
            let farthest_edge = (0..2)
                .map(|axis| (focus[axis] - lo[axis]).max(hi[axis] - focus[axis]))
                .fold(0.0, f64::max);
            if let Some((_, end)) = layout.rings[ring].morph {
                assert!(nearest_edge >= end - 1e-9, "ring {ring} at {focus:?}");
            }
            if let Some((start, _)) = layout.rings.get(ring + 1).and_then(|coarser| coarser.morph) {
                assert!(farthest_edge <= start + 1e-9, "ring {ring} at {focus:?}");
            }
            if let Some(coarser) = layout.rings.get(ring + 1) {
                let outer = layout.region(ring + 1, focus);
                let k = layout.rings[ring].per_parent;
                assert!(
                    outer.min[0] * k <= region.min[0] && region.max[0] < (outer.max[0] + 1) * k
                );
                assert_eq!(coarser.tile_size, size * k as f64);
            }
        }
    }
}

#[test]
fn neighbouring_tiles_share_their_edge_vertices_and_normals() {
    let layout = two_rings();
    let (left, right) = (
        TileKey {
            ring: 0,
            x: 3,
            z: -2,
        },
        TileKey {
            ring: 0,
            x: 4,
            z: -2,
        },
    );
    let mesh = |key| mesh_tile(&layout, key, &surface(&layout, key), None).unwrap();
    let (a, b) = (mesh(left), mesh(right));
    let side = layout.side();
    let at = |mesh: &TileMesh, key: TileKey, i: usize, j: usize| {
        let index = mesh.grid[j * side + i] as usize;
        let origin = layout.origin(key);
        (
            [
                origin[0] + f64::from(mesh.positions[index * 3]),
                f64::from(mesh.positions[index * 3 + 1]),
                origin[1] + f64::from(mesh.positions[index * 3 + 2]),
            ],
            mesh.normals[index * 3..index * 3 + 3].to_vec(),
        )
    };
    for j in 0..side {
        assert_eq!(at(&a, left, side - 1, j), at(&b, right, 0, j));
    }
    assert_eq!(a.groups, vec![(0, 0, a.indices.len() as u32)]);
}

/// A fine tile's targets put every vertex on the coarser drawn surface, so
/// at its morph's end the fine ring's edge lies on the coarser ring's grid
/// lines exactly, between vertices as at them.
#[test]
fn targets_lie_on_the_coarser_surface_and_a_full_morph_meets_it_along_the_edge() {
    let layout = two_rings();
    let fine = TileKey {
        ring: 0,
        x: 7,
        z: -1,
    };
    let parent = layout.parent(fine).unwrap();
    assert_eq!(
        parent,
        TileKey {
            ring: 1,
            x: 1,
            z: -1
        }
    );
    let coarse = surface(&layout, parent);
    let at = layout.origin(parent);
    let mesh = mesh_tile(&layout, fine, &surface(&layout, fine), Some((&coarse, at))).unwrap();
    let morph = mesh.morph.as_ref().unwrap();
    let origin = layout.origin(fine);
    let side = layout.side();
    for index in 0..side * side {
        let vertex = mesh.grid[index] as usize;
        let p = &mesh.positions[vertex * 3..vertex * 3 + 3];
        let (x, z) = (origin[0] + f64::from(p[0]), origin[1] + f64::from(p[2]));
        let (target, _) = coarse.surface(x - at[0], z - at[1]).unwrap();
        assert!((p[1] + morph[vertex * 3] - target).abs() < 1e-4);
    }
    // The fine tile's last column lies on the coarse tile's grid line x =
    // 32 m: along it, midway between fully morphed vertices, the fine edge
    // is the coarse surface.
    for j in 0..side - 1 {
        let morphed = |j: usize| {
            let vertex = mesh.grid[j * side + side - 1] as usize;
            mesh.positions[vertex * 3 + 1] + morph[vertex * 3]
        };
        let z = origin[1] + (j as f64 + 0.5) * layout.rings[0].cell_size;
        let x = origin[0] + layout.rings[0].tile_size;
        let (coarse_height, _) = coarse.surface(x - at[0], z - at[1]).unwrap();
        assert!(((morphed(j) + morphed(j + 1)) / 2.0 - coarse_height).abs() < 1e-4);
    }
    assert!(
        mesh.max[1]
            >= mesh
                .positions
                .chunks(3)
                .map(|p| p[1])
                .fold(f32::MIN, f32::max)
    );
}

#[test]
fn holes_drop_their_cells_and_an_unsampled_tile_meshes_to_nothing() {
    let layout = two_rings();
    let key = TileKey {
        ring: 0,
        x: 0,
        z: 0,
    };
    let holed = sample(&layout, key, |x, _| {
        if x < 10.0 {
            f32::NAN
        } else {
            ground(x, 0.0)
        }
    });
    let mesh = mesh_tile(&layout, key, &TileSurface::new(&layout, 0, holed), None).unwrap();
    // x 0 to 8 m (five columns of vertices) are holes: 11 columns of 16 cells
    // remain, two triangles each.
    let skirts = mesh.indices.len() - 11 * 16 * 6;
    assert_eq!(skirts, (11 + 11 + 16) * 6);
    let empty = sample(&layout, key, |_, _| f32::NAN);
    assert!(mesh_tile(&layout, key, &TileSurface::new(&layout, 0, empty), None).is_none());
}

#[test]
fn materials_group_triangles_by_majority_and_samples_are_validated() {
    let layout = two_rings();
    let key = TileKey {
        ring: 0,
        x: 0,
        z: 0,
    };
    let side = layout.side();
    let mut samples = sample(&layout, key, |_, _| 0.0);
    // Water (2) where x > 16 m, ground (0) elsewhere.
    samples.materials = Some(
        (0..side * side)
            .map(|index| if index % side > 8 { 2 } else { 0 })
            .collect(),
    );
    samples.colors = Some(vec![[0.2, 0.5, 0.1, 1.0]; side * side]);
    assert_eq!(samples.validate(&layout), Ok(()));
    let mesh = mesh_tile(
        &layout,
        key,
        &TileSurface::new(&layout, 0, samples.clone()),
        None,
    )
    .unwrap();
    assert_eq!(
        mesh.groups.iter().map(|group| group.0).collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(
        mesh.colors.as_ref().unwrap().len(),
        mesh.positions.len() / 3 * 4
    );
    assert!(!mesh.layer_weights);
    assert_eq!(triangle_material([1, 3, 2]), 3);
    assert_eq!(triangle_material([1, 3, 3]), 3);
    let mut both = samples.clone();
    both.layer_weights = Some(vec![[1.0, 0.0, 0.0, 0.0]; side * side]);
    assert_eq!(
        both.validate(&layout),
        Err(TileSamplesError::ColorsAndWeights)
    );
    let mut short = samples;
    short.heights.pop();
    assert!(matches!(
        short.validate(&layout),
        Err(TileSamplesError::HeightCount { .. })
    ));
}

#[test]
fn a_pick_meets_the_drawn_surface_with_its_morph_applied() {
    let layout = two_rings();
    let fine = TileKey {
        ring: 0,
        x: 0,
        z: 0,
    };
    let parent = layout.parent(fine).unwrap();
    let coarse = TileSurface::new(&layout, 1, sample(&layout, parent, |_, _| 5.0));
    let mesh = mesh_tile(
        &layout,
        fine,
        &TileSurface::new(&layout, 0, sample(&layout, fine, |_, _| 1.0)),
        Some((&coarse, layout.origin(parent))),
    )
    .unwrap();
    let down = [0.0, -1.0, 0.0];
    let hit = mesh.pick([10.3, 50.0, 7.9], down, |_, _| 0.0).unwrap();
    assert!((hit.point[1] - 1.0).abs() < 1e-9 && (hit.distance - 49.0).abs() < 1e-9);
    assert!((hit.normal[1] - 1.0).abs() < 1e-9);
    let morphed = mesh.pick([10.3, 50.0, 7.9], down, |_, _| 0.5).unwrap();
    assert!((morphed.point[1] - 3.0).abs() < 1e-6);
    // A slanting ray from outside the tile meets the surface inside it.
    let slant = mesh
        .pick([-20.0, 21.0, 10.0], [1.0, -1.0, 0.0], |_, _| 0.0)
        .unwrap();
    assert!((slant.point[0] - 0.0).abs() < 1e-9 && (slant.point[1] - 1.0).abs() < 1e-9);
    // Upward and outside rays miss.
    assert!(mesh
        .pick([10.0, -5.0, 10.0], [0.0, 1.0, 0.0], |_, _| 0.0)
        .is_none());
    assert!(mesh.pick([100.0, 50.0, 10.0], down, |_, _| 0.0).is_none());
}
