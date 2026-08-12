use std::{mem::size_of, time::Instant};

use rusty_engine::{
    core_voxel::VoxelValue,
    engine_spatial::{
        VoxelChunkIdentity, VoxelChunkLeaseRegistry, VoxelChunkPayload,
        VoxelChunkResidencyOperation, VoxelChunkResidencyService, VoxelChunkResidencyTransaction,
        VoxelCollisionScene,
    },
};

fn main() {
    println!(
        "chunk_size,width,admit_us,replace_us,evict_us,payload_bytes,resident_cell_bytes,dirty,reused"
    );
    for chunk_size in [16_u32, 32, 64] {
        for width in [1_usize, 8, 64] {
            measure(chunk_size, width);
        }
    }
}

fn measure(chunk_size: u32, width: usize) {
    let leases = VoxelChunkLeaseRegistry::default();
    let mut scene = VoxelCollisionScene::from_material_voxels(1.0, chunk_size, []).unwrap();
    let slot_count = chunk_size.pow(3) as usize;
    let admitted_payload = payload(slot_count, 0, 1, chunk_size);
    let replaced_payload = payload(slot_count, slot_count - 1, 2, chunk_size);
    let chunks: Vec<_> = (0..width)
        .map(|x| VoxelChunkIdentity::new(x as i64, 0, 0))
        .collect();
    let admissions: Vec<_> = chunks
        .iter()
        .copied()
        .map(|chunk| VoxelChunkResidencyOperation::Admit {
            chunk,
            payload: admitted_payload.clone(),
        })
        .collect();
    let expected_scene_source_revision = scene.source_revision();
    let started = Instant::now();
    let admitted = VoxelChunkResidencyService::apply(
        &mut scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision,
            operations: &admissions,
        },
    )
    .unwrap();
    let admit_us = started.elapsed().as_micros();

    let replacements: Vec<_> = chunks
        .iter()
        .copied()
        .map(|chunk| VoxelChunkResidencyOperation::Replace {
            chunk,
            expected_content_hash: VoxelChunkResidencyService::resident_chunk(&scene, chunk)
                .unwrap()
                .content_hash,
            payload: replaced_payload.clone(),
        })
        .collect();
    let expected_scene_source_revision = scene.source_revision();
    let started = Instant::now();
    let replaced = VoxelChunkResidencyService::apply(
        &mut scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision,
            operations: &replacements,
        },
    )
    .unwrap();
    let replace_us = started.elapsed().as_micros();

    let evictions: Vec<_> = chunks
        .iter()
        .copied()
        .map(|chunk| VoxelChunkResidencyOperation::Evict {
            chunk,
            expected_content_hash: VoxelChunkResidencyService::resident_chunk(&scene, chunk)
                .unwrap()
                .content_hash,
        })
        .collect();
    let expected_scene_source_revision = scene.source_revision();
    let started = Instant::now();
    VoxelChunkResidencyService::apply(
        &mut scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision,
            operations: &evictions,
        },
    )
    .unwrap();
    let evict_us = started.elapsed().as_micros();
    let payload_bytes = width * slot_count * size_of::<u16>();
    let resident_cell_bytes = width * slot_count * size_of::<VoxelValue>();
    println!(
        "{chunk_size},{width},{admit_us},{replace_us},{evict_us},{payload_bytes},{resident_cell_bytes},{},{}",
        replaced.dirty_chunks.len(),
        admitted.reused_mesh_chunks,
    );
}

fn payload(
    slot_count: usize,
    filled_index: usize,
    material: u16,
    chunk_size: u32,
) -> VoxelChunkPayload {
    let mut slots = vec![0; slot_count];
    slots[filled_index] = material;
    VoxelChunkPayload::new([chunk_size; 3], slots)
}
