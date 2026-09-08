//! A compact, closed stress field derived from CraftSurvive's `Disrupt` bands.
//!
//! The enclosing rock solid keeps the generated surface away from the sampling
//! domain. These regressions exercise ambiguous face arcs and vertex links
//! before attribute seams; no boundary caps or finer sampling hide defects.
use svc_implicit::{Bounds, Field, GenerateOptions, TopologyReadout, WaveDisplacement};

const BANDS: [([f32; 3], f32, u32, u64); 5] = [
    ([0.035, 0.055, 0.035], 1.15, 3, 0x243f6a8885a308d3),
    ([0.09, 0.14, 0.09], 0.65, 4, 0x13198a2e03707344),
    ([0.22, 0.32, 0.22], 0.30, 3, 0xa4093822299f31d0),
    ([0.55, 0.8, 0.55], 0.12, 3, 0x082efa98ec4e6c89),
    ([1.3, 1.7, 1.3], 0.035, 2, 0x452821e638d01377),
];

fn disrupted_closed_field(seed: u64) -> (Field, svc_implicit::Node) {
    let mut field = Field::new();
    let mut air = field
        .capsule([-1.15, 0.0, 0.0], [1.15, 0.0, 0.0], 0.34)
        .unwrap();
    for (start, end, radius) in [
        ([-0.25, 0.0, 0.0], [0.34, 0.86, 0.25], 0.28),
        ([0.20, 0.0, 0.0], [-0.42, -0.72, 0.37], 0.24),
    ] {
        let branch = field.capsule(start, end, radius).unwrap();
        air = field.smooth_union(air, branch, 0.22).unwrap();
    }
    for (frequency, amplitude, octaves, salt) in BANDS {
        air = field
            .displace_waves(
                air,
                WaveDisplacement {
                    frequency,
                    amplitude,
                    octaves,
                    lacunarity: 1.9,
                    gain: 0.55,
                    seed: seed ^ salt,
                },
            )
            .unwrap();
    }
    let protected_core = field
        .capsule([-0.7, 0.0, 0.0], [0.7, 0.0, 0.0], 0.3)
        .unwrap();
    air = field.union(air, protected_core).unwrap();
    let enclosure = field
        .box_shape(Bounds {
            min: [-1.8; 3],
            max: [1.8; 3],
        })
        .unwrap();
    let rock = field.difference(enclosure, air).unwrap();
    (field, rock)
}

#[test]
fn five_band_disrupted_closed_field_has_manifold_links() {
    for seed in [10, 11, 29, 47] {
        let (field, root) = disrupted_closed_field(seed);
        let mesh = field
            .generate(
                root,
                GenerateOptions {
                    bounds: Bounds {
                        min: [-2.0; 3],
                        max: [2.0; 3],
                    },
                    cell_size: 0.125,
                    max_vertices: 200_000,
                    max_triangles: 400_000,
                },
            )
            .unwrap();

        assert_eq!(
            mesh.topology(),
            TopologyReadout {
                boundary_edges: 0,
                non_manifold_edges: 0,
                inconsistent_winding_edges: 0,
            }
        );
        assert_vertex_links(mesh.positions.len(), &mesh.triangles);
    }
}

fn assert_vertex_links(vertex_count: usize, triangles: &[[u32; 3]]) {
    use std::collections::{BTreeMap, BTreeSet};
    let mut links = vec![BTreeMap::<u32, Vec<u32>>::new(); vertex_count];
    for &[a, b, c] in triangles {
        for (center, x, y) in [(a, b, c), (b, c, a), (c, a, b)] {
            links[center as usize].entry(x).or_default().push(y);
            links[center as usize].entry(y).or_default().push(x);
        }
    }
    for (vertex, link) in links.iter().enumerate() {
        if link.is_empty() {
            continue;
        }
        assert!(
            link.values().all(|neighbors| neighbors.len() == 2),
            "non-cyclic link at {vertex}"
        );
        let mut visited = BTreeSet::new();
        let mut pending = vec![*link.keys().next().unwrap()];
        while let Some(v) = pending.pop() {
            if visited.insert(v) {
                pending.extend(&link[&v]);
            }
        }
        assert_eq!(visited.len(), link.len(), "disconnected link at {vertex}");
    }
}

#[test]
fn raw_checkerboard_faces_keep_closed_links_across_axes_and_depths() {
    use fidget::{
        context::Tree,
        jit::JitShape,
        mesh::{Octree, Settings},
    };
    for depth in [4, 5] {
        for permutation in 0..3 {
            let axes = [Tree::x(), Tree::y(), Tree::z()];
            let x = axes[permutation].clone();
            let y = axes[(permutation + 1) % 3].clone();
            let z = axes[(permutation + 2) % 3].clone();
            let waves = ((x.clone() * 11.0 + 0.07).sin() * (y.clone() * 7.0 + 0.13).sin())
                + (z.clone() * 9.0 + 0.17).sin() * 0.6;
            let enclosure = x.abs().max(y.abs()).max(z.abs()) - 0.83;
            let shape = JitShape::from(waves.max(enclosure));
            let bound = shape.try_into().unwrap();
            let mesh = Octree::build(
                &bound,
                &Settings {
                    depth,
                    ..Default::default()
                },
            )
            .unwrap()
            .walk_dual();
            assert!(
                !mesh.face_arc_vertices.is_empty(),
                "fixture must exercise face arcs"
            );
            let triangles: Vec<_> = mesh
                .triangles
                .iter()
                .map(|t| [t.x as u32, t.y as u32, t.z as u32])
                .collect();
            assert_vertex_links(mesh.vertices.len(), &triangles);
            let mut edges = std::collections::BTreeMap::new();
            for &[a, b, c] in &triangles {
                for (a, b) in [(a, b), (b, c), (c, a)] {
                    *edges.entry((a, b)).or_insert(0) += 1;
                }
            }
            for (&(a, b), &count) in &edges {
                assert_eq!(count, 1, "repeated raw directed edge at depth {depth}");
                assert_eq!(
                    edges.get(&(b, a)),
                    Some(&1),
                    "unpaired raw edge at depth {depth}"
                );
            }
        }
    }
}
