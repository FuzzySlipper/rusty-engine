//! A compact, closed stress field derived from CraftSurvive's `Disrupt` bands.
//!
//! The enclosing rock solid keeps the generated surface away from the sampling
//! domain. This records the known raw dual-contouring leaf-connectivity
//! limitation; it is not a boundary cap or post-generation attribute seam.
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
fn five_band_disrupted_closed_field_records_known_nonmanifold_limitation() {
    let (field, root) = disrupted_closed_field(10);
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
            non_manifold_edges: 2,
            inconsistent_winding_edges: 0,
        }
    );
}
