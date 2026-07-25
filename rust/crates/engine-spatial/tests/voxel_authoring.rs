use core_math::Vec3;
use core_space::Face;
use engine_spatial::{
    decode_voxel_edit_history, encode_voxel_edit_history, MaterialVoxel, VoxelCollisionScene,
    VoxelEdit, VoxelEditHistory, VoxelEditHistoryCodecError, VoxelEditHistoryDiffOptions,
    VoxelEditHistoryError, VoxelEditHistoryLimits, VoxelEditService, VoxelEditTransaction,
    VoxelPickError, VoxelPickHint, VoxelPickService,
};
use entity_state::{EntityTransform, Quat};

#[test]
fn edit_preview_rebuilds_without_mutation_then_commits_exact_candidate() {
    let mut scene = scene();
    let revision = scene.source_revision();
    let hash = scene.authority_hash();
    let edits = [VoxelEdit::Set {
        address: [3, 0, 0],
        material_slot: 2,
    }];
    let prepared = VoxelEditService::preview(
        &scene,
        VoxelEditTransaction {
            expected_revision: revision,
            edits: &edits,
        },
    )
    .unwrap();
    assert_eq!(scene.source_revision(), revision);
    assert_eq!(scene.authority_hash(), hash);
    assert_eq!(prepared.deltas()[0].before_material, None);
    assert_eq!(prepared.deltas()[0].after_material, Some(2));
    let projected_hash = prepared.receipt().authority_hash;

    let receipt = VoxelEditService::commit(&mut scene, prepared).unwrap();
    assert_eq!(scene.authority_hash(), projected_hash);
    assert_eq!(receipt.authority_hash, projected_hash);
    assert!(receipt
        .projections
        .is_coherent_with(receipt.accepted_revision));
}

#[test]
fn renderer_hints_are_revalidated_for_local_and_transformed_instances() {
    let scene = scene();
    let local_hint = VoxelPickHint {
        origin: [1.1, 0.5, 0.5],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        claimed_voxel: [2, 0, 0],
        claimed_face: Face::NegX,
    };
    let anchor = VoxelPickService::validate(&scene, local_hint).unwrap();
    assert_eq!(anchor.place_voxel, [1, 0, 0]);
    assert_eq!(
        anchor.place_edit(3),
        VoxelEdit::Set {
            address: [1, 0, 0],
            material_slot: 3
        }
    );
    let mut stale = local_hint;
    stale.claimed_voxel = [3, 0, 0];
    assert!(matches!(
        VoxelPickService::validate(&scene, stale),
        Err(VoxelPickError::HintMismatch { .. })
    ));

    let transform = EntityTransform {
        translation: Vec3::new(10.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::new(2.0, 1.0, 1.0),
    };
    let world = VoxelPickHint {
        origin: [12.2, 0.5, 0.5],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        ..local_hint
    };
    let instance = VoxelPickService::validate_instance(&scene, transform, world).unwrap();
    assert_eq!(instance.local.hit_voxel, [2, 0, 0]);
    assert!((instance.world_point[0] - 14.0).abs() < 1.0e-9);
    assert!((instance.world_distance - 1.8).abs() < 1.0e-9);
}

#[test]
fn bounded_history_undo_redo_revert_and_fork_keep_one_coherent_scene() {
    let mut scene = scene();
    let mut history = VoxelEditHistory::new(&scene);
    history
        .apply(
            &mut scene,
            &[VoxelEdit::Set {
                address: [3, 0, 0],
                material_slot: 2,
            }],
        )
        .unwrap();
    history
        .apply(
            &mut scene,
            &[VoxelEdit::Set {
                address: [4, 0, 0],
                material_slot: 3,
            }],
        )
        .unwrap();
    let second_hash = scene.authority_hash();
    let undo = history.undo_one(&mut scene).unwrap();
    assert!(undo.applied);
    assert_eq!(undo.diff.changed_voxels, 1);
    assert_eq!(history.cursor().redo_depth, 1);
    assert!(!has_voxel(&scene, [4, 0, 0]));

    let preview = history
        .preview_revert_to_cursor(&scene, 2, VoxelEditHistoryDiffOptions { max_samples: 8 })
        .unwrap();
    assert!(!preview.receipt().applied);
    assert_eq!(preview.receipt().diff.samples.len(), 1);
    assert!(!has_voxel(&scene, [4, 0, 0]));
    let redo = history.commit_revert(&mut scene, preview).unwrap();
    assert!(redo.applied);
    assert_eq!(scene.authority_hash(), second_hash);

    history.undo_one(&mut scene).unwrap();
    let fork = history
        .apply(
            &mut scene,
            &[VoxelEdit::Set {
                address: [5, 0, 0],
                material_slot: 4,
            }],
        )
        .unwrap();
    assert_eq!(fork.invalidated_redo_count, 1);
    assert_eq!(history.cursor().redo_depth, 0);
    assert!(matches!(
        history.redo_one(&mut scene),
        Err(VoxelEditHistoryError::EmptyRedoStack)
    ));
    assert!(has_voxel(&scene, [5, 0, 0]));
    assert!(!has_voxel(&scene, [4, 0, 0]));
}

#[test]
fn history_codec_preserves_redo_tail_and_rejects_corruption() {
    let mut scene = scene();
    let mut history = VoxelEditHistory::new(&scene);
    for (x, material_slot) in [(3, 2), (4, 3)] {
        history
            .apply(
                &mut scene,
                &[VoxelEdit::Set {
                    address: [x, 0, 0],
                    material_slot,
                }],
            )
            .unwrap();
    }
    history.undo_one(&mut scene).unwrap();
    let encoded = encode_voxel_edit_history(&history).unwrap();
    let mut restored =
        decode_voxel_edit_history(&encoded, VoxelEditHistoryLimits::default()).unwrap();
    assert_eq!(restored.history.cursor().index, 1);
    assert_eq!(restored.history.cursor().redo_depth, 1);
    restored.history.redo_one(&mut restored.scene).unwrap();
    assert!(has_voxel(&restored.scene, [4, 0, 0]));

    let mut corrupt: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    corrupt["entries"][0]["deltas"][0]["afterMaterial"] = 9.into();
    let error = decode_voxel_edit_history(
        &serde_json::to_string(&corrupt).unwrap(),
        VoxelEditHistoryLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        VoxelEditHistoryCodecError::InvalidContentHash
    ));
}

#[test]
fn history_quota_and_stale_hash_rejections_do_not_mutate_state() {
    let mut scene = scene();
    let original_hash = scene.authority_hash();
    let mut bounded = VoxelEditHistory::with_limits(
        &scene,
        VoxelEditHistoryLimits {
            max_entries: 0,
            ..VoxelEditHistoryLimits::default()
        },
    );
    assert!(matches!(
        bounded.apply(
            &mut scene,
            &[VoxelEdit::Set {
                address: [3, 0, 0],
                material_slot: 2,
            }]
        ),
        Err(VoxelEditHistoryError::EntryQuotaExceeded { .. })
    ));
    assert_eq!(scene.authority_hash(), original_hash);
    assert!(bounded.entries().is_empty());

    let mut history = VoxelEditHistory::new(&scene);
    history
        .apply(
            &mut scene,
            &[VoxelEdit::Set {
                address: [3, 0, 0],
                material_slot: 2,
            }],
        )
        .unwrap();
    let direct = [VoxelEdit::Set {
        address: [6, 0, 0],
        material_slot: 4,
    }];
    let revision = scene.source_revision();
    VoxelEditService::apply(
        &mut scene,
        VoxelEditTransaction {
            expected_revision: revision,
            edits: &direct,
        },
    )
    .unwrap();
    let drifted = scene.authority_hash();
    assert!(matches!(
        history.undo_one(&mut scene),
        Err(VoxelEditHistoryError::StaleAuthority { .. })
    ));
    assert_eq!(scene.authority_hash(), drifted);
    assert_eq!(history.cursor().index, 1);
}

fn scene() -> VoxelCollisionScene {
    VoxelCollisionScene::from_material_voxels(
        1.0,
        8,
        [
            MaterialVoxel {
                address: [0, 0, 0],
                material_slot: 1,
            },
            MaterialVoxel {
                address: [2, 0, 0],
                material_slot: 1,
            },
        ],
    )
    .unwrap()
}

fn has_voxel(scene: &VoxelCollisionScene, address: [i64; 3]) -> bool {
    scene
        .material_voxels()
        .iter()
        .any(|voxel| voxel.address == address)
}
