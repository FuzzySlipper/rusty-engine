use svc_implicit::{Bounds, Field, GenerateOptions, Node};

fn cave() -> (Field, Node) {
    let mut f = Field::new();
    let spine = [
        [0., 5.3, -4.],
        [0., 5.3, 1.],
        [-3.36, 5.6, 6.],
        [1.64, 5.4, 12.],
        [0., 5.6, 19.],
    ];
    let mut air = f.capsule(spine[0], spine[1], 3.1).unwrap();
    for i in 1..4 {
        let b = f.capsule(spine[i], spine[i + 1], 3.1).unwrap();
        air = f.smooth_union(air, b, 0.65).unwrap();
    }
    let b = f.capsule(spine[2], [-6., 5.6, 13.], 2.7).unwrap();
    air = f.smooth_union(air, b, 0.65).unwrap();
    for (p, r) in [
        ([-3.36, 6.6, 6.], [4.8, 5.2, 5.]),
        ([0., 6.2, 19.], [5.5, 4.6, 4.5]),
        ([-6., 5.6, 13.], [3.3, 3.8, 3.6]),
    ] {
        let b = f.ellipsoid(p, r).unwrap();
        air = f.smooth_union(air, b, 0.65).unwrap();
    }
    for i in 0..5 {
        let y = 4.2 + i as f32 * 1.35;
        let w = 4.7 + (i % 2) as f32 * 0.65;
        for (p, r) in [
            ([-3.36, y, 6.], [w, 0.48, 4.6]),
            ([0., y, 19.], [w + 0.6, 0.40, 3.9]),
        ] {
            let b = f.ellipsoid(p, r).unwrap();
            air = f.union(air, b).unwrap();
        }
    }
    let b = f.ellipsoid([-3.36, 9.8, 6.], [1.8, 4., 2.]).unwrap();
    air = f.smooth_union(air, b, 0.35).unwrap();
    let b = f
        .box_shape(Bounds {
            min: [-10.8, 3., -4.8],
            max: [10.8, 12.8, 22.8],
        })
        .unwrap();
    air = f.intersection(air, b).unwrap();
    let b = f
        .box_shape(Bounds {
            min: [-2., 3., -8.],
            max: [2., 7.2, -3.5],
        })
        .unwrap();
    air = f.union(air, b).unwrap();
    let rock = f
        .box_shape(Bounds {
            min: [-12., 0., -6.],
            max: [12., 14., 24.],
        })
        .unwrap();
    let root = f.difference(rock, air).unwrap();
    (f, root)
}

#[test]
fn carved_volume_recovers_escaped_leaf_vertices_without_opening_edges() {
    let (f, root) = cave();
    let mesh = f
        .generate(
            root,
            GenerateOptions {
                bounds: Bounds {
                    min: [-12.25, -0.25, -6.25],
                    max: [12.25, 14.25, 24.25],
                },
                cell_size: 0.28,
                max_vertices: 262144,
                max_triangles: 262144,
            },
        )
        .unwrap();
    let values = f.sample(root, &mesh.positions).unwrap();
    // Before the bounded leaf repair, this cave's maximum absolute field
    // residual was 0.25767, with visible spikes around the thin lateral cuts.
    // Field values are not Euclidean distances; this is a recipe regression.
    let max_residual = values.iter().map(|v| v.abs()).fold(0f32, f32::max);
    assert!(max_residual < 0.06, "escaped cave geometry: {max_residual}");
    assert!(mesh.bounded_leaf_vertices > 0);
    assert_eq!(mesh.topology(), svc_implicit::TopologyReadout::default());
}

#[test]
fn sampled_carved_volume_reports_closed_boundary_and_cell_ambiguity() {
    let (field, root) = cave();
    let mut volume =
        svc_implicit::volume::SampledVolume::new([-12.5, -0.5, -6.5], 0.4, [64, 39, 79], 1.)
            .unwrap();
    volume.rasterize(&field, root).unwrap();
    let mesh = volume.generate(0., 262144, 262144).unwrap();
    assert!(!mesh.triangles.is_empty());
    // The uniform path retains one vertex per active cell. This deliberately
    // thin-cut fixture has one aliased internal edge at 0.4 spacing; preserve
    // the honest diagnostic while ensuring it has no open shell or bad winding.
    assert_eq!(
        mesh.topology(),
        svc_implicit::TopologyReadout {
            boundary_edges: 0,
            non_manifold_edges: 1,
            inconsistent_winding_edges: 0,
        }
    );
    assert!(volume.sample([0., 5., -6.]).unwrap() > 0.);
    assert!(volume.sample([0., 0.3, 6.]).unwrap() < 0.);
}
