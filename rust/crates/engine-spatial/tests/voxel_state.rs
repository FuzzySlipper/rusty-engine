use core_space::Direction6;
use engine_spatial::*;

#[test]
fn state_only_edit_changes_hash_mesh_and_round_trips_history() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[0, 0, 0]]).unwrap();
    let old_hash = scene.authority_hash();
    let old_mesh = scene.mesh_chunks()[0].content_hash;
    let mut history = VoxelEditHistory::new(&scene);
    let state = (17 << 2) | 1;
    history
        .apply(
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
    assert_ne!(scene.mesh_chunks()[0].content_hash, old_mesh);
    assert!(scene.mesh_chunks()[0]
        .groups
        .iter()
        .all(|g| g.state == state));
    let encoded = encode_voxel_edit_history(&history).unwrap();
    let mut restored =
        decode_voxel_edit_history(&encoded, VoxelEditHistoryLimits::default()).unwrap();
    assert_eq!(restored.scene.material_voxels(), scene.material_voxels());
    restored.history.undo_one(&mut restored.scene).unwrap();
    assert_eq!(restored.scene.material_voxels()[0].state, 0);
    restored.history.redo_one(&mut restored.scene).unwrap();
    assert_eq!(restored.scene.material_voxels()[0].state, state);
    history
        .apply(&mut scene, &[VoxelEdit::Clear { address: [0, 0, 0] }])
        .unwrap();
    history.undo_one(&mut scene).unwrap();
    assert_eq!(scene.material_voxels()[0].state, state);
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
    let chunk = &scene.mesh_chunks()[0];
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
    assert_eq!(standard.mesh_chunks()[0].indices.len(), 36);
}

#[test]
fn residency_preserves_states_and_rejects_invalid_empty_cell_state() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 2, []).unwrap();
    let leases = VoxelChunkLeaseRegistry::default();
    let mut payload = VoxelChunkPayload::new([2; 3], vec![1; 8]);
    payload.states = (0..8).collect();
    let revision = scene.source_revision();
    VoxelChunkResidencyService::apply(
        &mut scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: revision,
            operations: &[VoxelChunkResidencyOperation::Admit {
                chunk: VoxelChunkIdentity::ORIGIN,
                payload,
            }],
        },
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
    assert!(VoxelChunkResidencyService::prepare(
        &scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: scene.source_revision(),
            operations: &[VoxelChunkResidencyOperation::Admit {
                chunk: VoxelChunkIdentity::new(1, 0, 0),
                payload: invalid
            }]
        }
    )
    .is_err());
}

#[test]
fn invalid_state_is_rejected_without_publication() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[0, 0, 0]]).unwrap();
    let hash = scene.authority_hash();
    let mut history = VoxelEditHistory::new(&scene);
    assert!(history
        .apply(
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
