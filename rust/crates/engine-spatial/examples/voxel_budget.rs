//! Reproducible CPU/resident mesh sizing probe; run each case in a fresh process.
use engine_spatial::{
    MaterialVoxel, SurfaceMeshOptions, VoxelChunkIdentity, VoxelChunkLeaseRegistry,
    VoxelChunkPayload, VoxelChunkResidencyOperation, VoxelChunkResidencyService,
    VoxelChunkResidencyTransaction, VoxelCollisionScene, VoxelEdit, VoxelEditService,
    VoxelEditTransaction,
};
use std::{mem::size_of, time::Instant};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let shape = args.get(1).map(String::as_str).unwrap_or("solid");
    let count: i64 = args
        .get(2)
        .map(|s| s.parse().expect("chunk count"))
        .unwrap_or(16);
    assert!(matches!(shape, "solid" | "checker" | "sparse" | "stateful"));
    assert!((1..=4096).contains(&count));
    const EDGE: i64 = 16;
    let voxels = (0..count).flat_map(|chunk| {
        (0..EDGE).flat_map(move |x| {
            (0..EDGE).flat_map(move |y| {
                (0..EDGE).filter_map(move |z| {
                    let occupied = match shape {
                        "solid" | "stateful" => true,
                        "checker" => (x + y + z) % 2 == 0,
                        _ => x == 0 && y == 0 && z == 0,
                    };
                    occupied.then_some(MaterialVoxel {
                        state: if shape == "stateful" {
                            ((y % 4) * 4 + (x + z) % 4) as u16
                        } else {
                            0
                        },
                        address: [chunk * EDGE * 2 + x, y, z],
                        material_slot: 1,
                    })
                })
            })
        })
    });
    let start = Instant::now();
    let mut scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        EDGE as u32,
        voxels,
        SurfaceMeshOptions::default(),
    )
    .expect("admitted scene");
    let build_us = start.elapsed().as_micros();
    let initial_solids = scene.solid_voxel_count();
    let mesh_bytes: usize = scene
        .mesh_chunks()
        .iter()
        .map(|m| {
            (m.positions.capacity() + m.normals.capacity() + m.tile_coordinates.capacity())
                * size_of::<f32>()
                + m.indices.capacity() * size_of::<u32>()
                + m.groups.capacity() * size_of::<engine_spatial::VoxelMeshGroup>()
        })
        .sum();
    let quads: u64 = scene.mesh_chunks().iter().map(|m| u64::from(m.quads)).sum();
    let mut samples = Vec::new();
    let mut rebuilt = 0;
    for index in 0..7 {
        let edits = [if index % 2 == 0 {
            VoxelEdit::Clear { address: [0, 0, 0] }
        } else {
            VoxelEdit::Set {
                address: [0, 0, 0],
                material_slot: 1,
            }
        }];
        let expected_revision = scene.source_revision();
        let start = Instant::now();
        let receipt = VoxelEditService::apply(
            &mut scene,
            VoxelEditTransaction {
                expected_revision,
                edits: &edits,
            },
        )
        .expect("edit");
        samples.push(start.elapsed().as_micros());
        rebuilt = receipt.rebuilt_mesh_chunks;
    }
    samples.sort_unstable();
    let mut replacement = Vec::new();
    let leases = VoxelChunkLeaseRegistry::default();
    for index in 0..7 {
        let chunk = VoxelChunkIdentity::ORIGIN;
        let expected_content_hash = VoxelChunkResidencyService::resident_chunk(&scene, chunk)
            .expect("resident")
            .content_hash;
        let mut slots = vec![0; (EDGE * EDGE * EDGE) as usize];
        for z in 0..EDGE {
            for y in 0..EDGE {
                for x in 0..EDGE {
                    slots[(x + EDGE * (y + EDGE * z)) as usize] = u16::from(match shape {
                        "solid" | "stateful" => true,
                        "checker" => (x + y + z) % 2 == 0,
                        _ => x == 0 && y == 0 && z == 0,
                    });
                }
            }
        }
        // Change one cell while retaining the same resident chunk identity.
        slots[0] = if index % 2 == 0 { 1 } else { 0 };
        let mut payload = VoxelChunkPayload::new([EDGE as u32; 3], slots);
        if shape == "stateful" {
            payload.states = payload
                .material_slots
                .iter()
                .enumerate()
                .map(|(index, material)| {
                    let x = index as i64 % EDGE;
                    let y = index as i64 / EDGE % EDGE;
                    let z = index as i64 / (EDGE * EDGE);
                    if *material == 0 {
                        0
                    } else {
                        ((y % 4) * 4 + (x + z) % 4) as u16
                    }
                })
                .collect();
        }
        let operations = [VoxelChunkResidencyOperation::Replace {
            chunk,
            expected_content_hash,
            payload,
        }];
        let expected_scene_source_revision = scene.source_revision();
        let start = Instant::now();
        VoxelChunkResidencyService::apply(
            &mut scene,
            &leases,
            VoxelChunkResidencyTransaction {
                expected_scene_source_revision,
                operations: &operations,
            },
        )
        .expect("replace");
        replacement.push(start.elapsed().as_micros());
    }
    replacement.sort_unstable();
    let mut scene = std::sync::Arc::new(scene);
    let mut start_samples = Vec::new();
    let mut preparation_samples = Vec::new();
    let mut commit_samples = Vec::new();
    let mut observations = 0u64;
    for _ in 0..7 {
        let chunk = VoxelChunkIdentity::ORIGIN;
        let expected_content_hash = VoxelChunkResidencyService::resident_chunk(&scene, chunk)
            .unwrap()
            .content_hash;
        let mut payload =
            VoxelChunkPayload::new([EDGE as u32; 3], vec![0; (EDGE * EDGE * EDGE) as usize]);
        payload.states = vec![0; payload.material_slots.len()];
        for voxel in scene
            .material_voxels()
            .iter()
            .filter(|v| v.address[0] < EDGE)
        {
            let [x, y, z] = voxel.address;
            let index = (x + EDGE * (y + EDGE * z)) as usize;
            payload.material_slots[index] = voxel.material_slot;
            payload.states[index] = voxel.state;
        }
        payload.material_slots[0] = u16::from(payload.material_slots[0] == 0);
        payload.states[0] = 0;
        let operations = vec![VoxelChunkResidencyOperation::Replace {
            chunk,
            expected_content_hash,
            payload,
        }];
        let start = Instant::now();
        let mut worker = engine_spatial::VoxelResidencyPreparation::start(
            std::sync::Arc::clone(&scene),
            leases.clone(),
            scene.source_revision(),
            operations,
        )
        .unwrap();
        start_samples.push(start.elapsed().as_micros());
        let prepared = loop {
            match worker.poll() {
                engine_spatial::VoxelPreparationPoll::Ready(result) => break (*result).unwrap(),
                engine_spatial::VoxelPreparationPoll::WorkerFailed => panic!("preparation failed"),
                engine_spatial::VoxelPreparationPoll::Pending => {
                    std::hint::black_box(scene.source_revision());
                    observations += 1;
                    std::thread::yield_now();
                }
            }
        };
        preparation_samples.push(start.elapsed().as_micros());
        let start = Instant::now();
        let (candidate, _) =
            VoxelChunkResidencyService::finish_prepared(&scene, &leases, prepared).unwrap();
        scene = std::sync::Arc::new(candidate);
        drop(worker);
        commit_samples.push(start.elapsed().as_micros());
    }
    start_samples.sort_unstable();
    preparation_samples.sort_unstable();
    commit_samples.sort_unstable();
    println!("shape,chunks,solid_cells,build_us,edit_median_us,edit_max_us,rebuilt,quads,mesh_capacity_bytes,replace_median_us,replace_max_us,start_median_us,prepare_median_us,commit_median_us,commit_max_us,concurrent_reads,voxel_value_bytes,material_voxel_bytes");
    println!(
        "{shape},{count},{initial_solids},{build_us},{},{},{rebuilt},{quads},{mesh_bytes},{},{},{},{},{},{},{observations},{},{}",
        samples[3], samples[6], replacement[3], replacement[6], start_samples[3], preparation_samples[3], commit_samples[3], commit_samples[6], size_of::<core_voxel::VoxelValue>(), size_of::<MaterialVoxel>()
    );
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        if let Some(peak) = status.lines().find(|line| line.starts_with("VmHWM:")) {
            eprintln!("{peak}");
        }
    }
}
