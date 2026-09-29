//! Same-scene, same-location edit-size probe. Timings are observations, not gates.
use engine_spatial::{VoxelCollisionScene, VoxelEdit, VoxelEditService};
use std::time::Instant;

fn main() {
    let chunks: i64 = std::env::args().nth(1).map_or(16, |s| s.parse().unwrap());
    assert!((1..=128).contains(&chunks));
    const EDGE: i64 = 16;
    let base = VoxelCollisionScene::from_solid_voxels(
        1.0,
        EDGE as u32,
        (0..chunks).flat_map(|chunk| {
            (0..EDGE).flat_map(move |x| {
                (0..4).flat_map(move |y| (0..EDGE).map(move |z| [chunk * EDGE * 2 + x, y, z]))
            })
        }),
    )
    .unwrap();
    println!("chunks,cells,changed,rebuilt,reused,median_apply_us");
    for count in [1, 7, 33, 41, 123] {
        let edits: Vec<_> = (0..count)
            .map(|i| VoxelEdit::Clear {
                address: [1 + i % 14, 3, 1 + i / 14],
            })
            .collect();
        let mut apply_samples = Vec::new();
        let mut counts = (0, 0, 0);
        for _ in 0..7 {
            // A fresh copy per sample; only the edit itself is timed.
            let mut candidate = base.clone();
            let started = Instant::now();
            let receipt = VoxelEditService::apply(&mut candidate, &edits).unwrap();
            apply_samples.push(started.elapsed().as_micros());
            counts = (
                receipt.fact.changed_voxels,
                receipt.rebuilt_mesh_chunks,
                receipt.reused_mesh_chunks,
            );
            assert_eq!(
                candidate.solid_voxel_count(),
                base.solid_voxel_count() - count as usize
            );
        }
        apply_samples.sort();
        println!(
            "{chunks},{count},{},{},{},{}",
            counts.0, counts.1, counts.2, apply_samples[3]
        );
    }
}
