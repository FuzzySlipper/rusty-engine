use core_space::Direction6;
use engine_spatial::*;

#[test]
fn state_only_edit_changes_hash_and_mesh_and_inverse_edits_restore_it() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[0, 0, 0]]).unwrap();
    let old_hash = scene.authority_hash();
    let old_mesh = scene.mesh_chunks().cloned().collect::<Vec<_>>()[0].content_hash;
    let state = (17 << 2) | 1;
    VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::SetState {
            address: [0, 0, 0],
            material_slot: 1,
            state,
        }],
    )
    .unwrap();
    assert_eq!(scene.material_voxels()[0].state, state);
    assert_ne!(scene.authority_hash(), old_hash);
    assert_ne!(
        scene.mesh_chunks().cloned().collect::<Vec<_>>()[0].content_hash,
        old_mesh
    );
    assert!(scene.mesh_chunks().cloned().collect::<Vec<_>>()[0]
        .groups
        .iter()
        .all(|g| g.state == state));
    let stated_hash = scene.authority_hash();
    // The product owns undo: it applies the inverse edits itself.
    VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::SetState {
            address: [0, 0, 0],
            material_slot: 1,
            state: 0,
        }],
    )
    .unwrap();
    assert_eq!(scene.authority_hash(), old_hash);
    VoxelEditService::apply(&mut scene, &[VoxelEdit::Clear { address: [0, 0, 0] }]).unwrap();
    VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::SetState {
            address: [0, 0, 0],
            material_slot: 1,
            state,
        }],
    )
    .unwrap();
    assert_eq!(scene.material_voxels()[0].state, state);
    assert_eq!(scene.authority_hash(), stated_hash);
}

#[test]
fn differing_states_do_not_greedily_merge_and_rotation_selects_local_face() {
    let scene = VoxelCollisionScene::from_material_voxels(
        1.0,
        8,
        [
            MaterialVoxel {
                address: [0, 0, 0],
                material_slot: 1,
                state: 0,
            },
            MaterialVoxel {
                address: [1, 0, 0],
                material_slot: 1,
                state: 5,
            },
        ],
    )
    .unwrap();
    let chunk = &scene.mesh_chunks().cloned().collect::<Vec<_>>()[0];
    assert_eq!(
        chunk.indices.len(),
        60,
        "ten exposed faces separated by state"
    );
    let rotated_right = chunk
        .groups
        .iter()
        .find(|g| g.state == 5 && g.direction == Some(Direction6::PosZ))
        .unwrap();
    let index = chunk.indices[rotated_right.start as usize] as usize;
    assert_eq!(
        &chunk.normals[index * 3..index * 3 + 3],
        &[1.0, 0.0, 0.0],
        "world +X uses authored +Z after a quarter turn"
    );
    let standard = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[0, 0, 0], [1, 0, 0]]).unwrap();
    assert_eq!(
        standard.mesh_chunks().cloned().collect::<Vec<_>>()[0]
            .indices
            .len(),
        36
    );
}

#[test]
fn residency_preserves_states_and_rejects_invalid_empty_cell_state() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 2, []).unwrap();
    let mut payload = VoxelChunkPayload::new([2; 3], vec![1; 8]);
    payload.states = (0..8).collect();
    VoxelChunkResidencyService::apply(
        &mut scene,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: VoxelChunkIdentity::ORIGIN,
            payload,
        }],
    )
    .unwrap();
    assert_eq!(
        scene
            .material_voxels()
            .iter()
            .map(|v| v.state)
            .collect::<std::collections::BTreeSet<_>>(),
        (0..8).collect()
    );
    let mut invalid = VoxelChunkPayload::new([2; 3], vec![0; 8]);
    invalid.states = vec![1; 8];
    let before = scene.authority_hash();
    assert!(VoxelChunkResidencyService::apply(
        &mut scene,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: VoxelChunkIdentity::new(1, 0, 0),
            payload: invalid
        }]
    )
    .is_err());
    assert_eq!(scene.authority_hash(), before);
    assert_eq!(scene.resident_chunk_count(), 1);
}

#[test]
fn invalid_state_is_rejected_without_publication() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[0, 0, 0]]).unwrap();
    let hash = scene.authority_hash();
    assert!(VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::SetState {
            address: [0, 0, 0],
            material_slot: 1,
            state: 0x8000
        }]
    )
    .is_err());
    assert_eq!(scene.authority_hash(), hash);
}
