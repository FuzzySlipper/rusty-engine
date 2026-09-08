//! Release-mode dual-contouring baseline.
//!
//! Run with:
//! `cargo run --release -p svc-mesh --example dual_contouring_performance`
//!
//! Each output line is one machine-readable `RUSTY_PERF` record. Sample-field
//! construction is deliberately outside the timed region; timings measure the
//! public `mesh_scalar_samples` dual-contouring operation only.

use std::time::{Duration, Instant};

use svc_mesh::{mesh_scalar_samples, MeshPayload, SurfaceMeshLimits, SurfaceMode};

const WORKLOAD_VERSION: &str = "dc-voxel-v1";
const WARMUP_SAMPLES: usize = 3;
const MEASURED_SAMPLES: usize = 20;
const RUNS_PER_WORKLOAD: usize = 3;
const NOISY_CARVED_SEED: u64 = 0x5C01_7ED0_CAFE_0001;

#[derive(Clone, Copy)]
enum FieldKind {
    Sculpted,
    NoisyCarved,
}

impl FieldKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Sculpted => "sculpted_tunnel_lobes",
            Self::NoisyCarved => "noisy_carved_volume",
        }
    }
}

#[derive(Clone, Copy)]
struct Workload {
    name: &'static str,
    kind: FieldKind,
    seed: u64,
    dimensions: [usize; 3],
}

fn main() {
    let workloads = [
        Workload {
            name: "sculpted-32",
            kind: FieldKind::Sculpted,
            seed: 0x5C01_7ED0_0000_0032,
            dimensions: [32, 32, 32],
        },
        Workload {
            name: "noisy-carved-40",
            kind: FieldKind::NoisyCarved,
            seed: NOISY_CARVED_SEED,
            dimensions: [40, 40, 40],
        },
        Workload {
            name: "noisy-carved-56",
            kind: FieldKind::NoisyCarved,
            seed: NOISY_CARVED_SEED,
            dimensions: [56, 56, 56],
        },
    ];

    for run_index in 1..=RUNS_PER_WORKLOAD {
        for workload in workloads {
            run(run_index, workload);
        }
    }
}

fn run(run_index: usize, workload: Workload) {
    let samples = scalar_samples(workload);
    let mesh = extract_mesh(workload, &samples);
    assert_mesh_is_renderable(&mesh);
    let expected_counts = (mesh.stats.vertices, mesh.stats.triangles);

    for _ in 0..WARMUP_SAMPLES {
        let mesh = extract_mesh(workload, &samples);
        assert_mesh_is_renderable(&mesh);
        assert_eq!(
            (mesh.stats.vertices, mesh.stats.triangles),
            expected_counts,
            "deterministic workload changed output during warmup"
        );
    }

    let mut elapsed = Vec::with_capacity(MEASURED_SAMPLES);
    for _ in 0..MEASURED_SAMPLES {
        let started = Instant::now();
        let mesh = extract_mesh(workload, &samples);
        elapsed.push(started.elapsed());
        assert_mesh_is_renderable(&mesh);
        assert_eq!(
            (mesh.stats.vertices, mesh.stats.triangles),
            expected_counts,
            "deterministic workload changed output during measurement"
        );
    }

    emit_record(run_index, workload, &mesh, &elapsed);
}

fn extract_mesh(workload: Workload, samples: &[f32]) -> MeshPayload {
    mesh_scalar_samples(
        [0.0; 3],
        1.0,
        workload.dimensions,
        samples,
        0.0,
        SurfaceMeshLimits::default(),
    )
    .expect("deterministic baseline field must be meshable")
}

fn scalar_samples(workload: Workload) -> Vec<f32> {
    let [width, height, depth] = workload.dimensions;
    let mut samples = Vec::with_capacity(width * height * depth);
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let point = [
                    normalized_coordinate(x, width),
                    normalized_coordinate(y, height),
                    normalized_coordinate(z, depth),
                ];
                samples.push(match workload.kind {
                    FieldKind::Sculpted => sculpted_field(point),
                    FieldKind::NoisyCarved => noisy_carved_field(point, workload.seed),
                });
            }
        }
    }
    samples
}

fn normalized_coordinate(index: usize, dimension: usize) -> f32 {
    index as f32 * 2.0 / (dimension - 1) as f32 - 1.0
}

fn sculpted_field([x, y, z]: [f32; 3]) -> f32 {
    let sphere = length(x, y, z) - 0.61;
    let lobe = length(x + 0.23, y - 0.14, z + 0.04) - 0.39;
    let tunnel = (x * x + z * z).sqrt() - 0.17;
    sphere.min(lobe).max(-tunnel)
}

fn noisy_carved_field([x, y, z]: [f32; 3], seed: u64) -> f32 {
    let radial = length(x, y, z) - 0.72;
    let waves =
        (x * 11.0 + y * 3.0).sin() + (y * 13.0 - z * 5.0).sin() + (z * 17.0 + x * 7.0).sin();
    let noise = lattice_noise(x, y, z, seed) * 0.09 + waves * 0.035;
    let vertical_bore = ((x - 0.15) * (x - 0.15) + (z + 0.10) * (z + 0.10)).sqrt() - 0.18;
    let side_cave = length(x + 0.34, y - 0.08, z) - 0.24;
    (radial + noise).max(-vertical_bore).max(-side_cave)
}

fn length(x: f32, y: f32, z: f32) -> f32 {
    (x * x + y * y + z * z).sqrt()
}

fn lattice_noise(x: f32, y: f32, z: f32, seed: u64) -> f32 {
    let mut value = seed
        ^ (x.to_bits() as u64).wrapping_mul(0x9E37_79B1_85EB_CA87)
        ^ (y.to_bits() as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z.to_bits() as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    (value as u32) as f32 / u32::MAX as f32 * 2.0 - 1.0
}

fn assert_mesh_is_renderable(mesh: &MeshPayload) {
    assert_eq!(mesh.surface_mode, SurfaceMode::DualContouring);
    assert!(mesh.stats.vertices > 0, "workload emitted no vertices");
    assert!(mesh.stats.triangles > 0, "workload emitted no triangles");
    assert_eq!(mesh.positions.len(), mesh.stats.vertices as usize * 3);
    assert_eq!(mesh.normals.len(), mesh.stats.vertices as usize * 3);
    assert_eq!(mesh.indices.len(), mesh.stats.triangles as usize * 3);
    assert!(mesh.positions.iter().all(|value| value.is_finite()));
    assert!(mesh.normals.iter().all(|value| value.is_finite()));
    assert!(mesh
        .indices
        .iter()
        .all(|index| *index < mesh.stats.vertices));
}

fn emit_record(run_index: usize, workload: Workload, mesh: &MeshPayload, samples: &[Duration]) {
    let milliseconds = samples
        .iter()
        .map(|sample| sample.as_secs_f64() * 1_000.0)
        .collect::<Vec<_>>();
    let mut sorted = milliseconds.clone();
    sorted.sort_by(f64::total_cmp);
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let median = percentile(&sorted, 0.5);
    let p95 = percentile(&sorted, 0.95);
    let mean = milliseconds.iter().sum::<f64>() / milliseconds.len() as f64;
    let sample_values = milliseconds
        .iter()
        .map(|sample| format!("{sample:.6}"))
        .collect::<Vec<_>>()
        .join(",");
    let [width, height, depth] = workload.dimensions;
    let seed = format!("0x{:016X}", workload.seed);

    println!(
        "RUSTY_PERF {{\"schemaVersion\":1,\"lane\":\"voxel-dc-meshing\",\"run\":{run_index},\"workload\":{{\"id\":\"{}\",\"version\":\"{WORKLOAD_VERSION}\",\"field\":\"{}\",\"seed\":\"{seed}\",\"gridDimensions\":[{width},{height},{depth}],\"warmupSamples\":{WARMUP_SAMPLES},\"measuredSamples\":{MEASURED_SAMPLES},\"pipeline\":\"svc-mesh.mesh_scalar_samples\",\"surfaceMode\":\"dualContouring\"}},\"timedRegion\":\"mesh_scalar_samples_only\",\"iterations\":{MEASURED_SAMPLES},\"unit\":\"milliseconds\",\"samples\":[{sample_values}],\"minimum\":{min:.6},\"median\":{median:.6},\"p95\":{p95:.6},\"maximum\":{max:.6},\"mean\":{mean:.6},\"vertices\":{},\"triangles\":{}}}",
        workload.name,
        workload.kind.as_str(),
        mesh.stats.vertices,
        mesh.stats.triangles,
    );
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * percentile).round() as usize]
}
