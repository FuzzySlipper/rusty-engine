use engine_spatial::{
    decode_voxel_edit_history, encode_voxel_edit_history, MaterialVoxel, SurfaceMeshOptions,
    SurfaceMode, VoxelChunkContentHash, VoxelChunkIdentity, VoxelChunkLeaseRegistry,
    VoxelChunkPayload, VoxelChunkResidencyApplyError, VoxelChunkResidencyOperation,
    VoxelChunkResidencyRejection, VoxelChunkResidencyService, VoxelChunkResidencyTransaction,
    VoxelCollisionScene, VoxelEdit, VoxelEditHistory, VoxelEditHistoryError,
    VoxelEditHistoryLimits, VoxelResidencyHistoryPolicy, VoxelSourceRevision, MAX_SOLID_VOXELS,
    MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION, MAX_VOXEL_CHUNK_PAYLOAD_SLOTS_PER_TRANSACTION,
};

fn identity(x: i64, y: i64, z: i64) -> VoxelChunkIdentity {
    VoxelChunkIdentity::new(x, y, z)
}

fn payload(chunk_size: u32, voxels: &[([u32; 3], u16)]) -> VoxelChunkPayload {
    let mut slots = vec![0; chunk_size.pow(3) as usize];
    for &([x, y, z], material_slot) in voxels {
        let index =
            x as usize + chunk_size as usize * (y as usize + chunk_size as usize * z as usize);
        slots[index] = material_slot;
    }
    VoxelChunkPayload::new([chunk_size; 3], slots)
}

fn empty_scene(chunk_size: u32, mode: SurfaceMode) -> VoxelCollisionScene {
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        chunk_size,
        [],
        SurfaceMeshOptions {
            mode,
            ..SurfaceMeshOptions::default()
        },
    )
    .unwrap()
}

fn apply(
    scene: &mut VoxelCollisionScene,
    leases: &VoxelChunkLeaseRegistry,
    operations: &[VoxelChunkResidencyOperation],
) -> Result<engine_spatial::VoxelChunkResidencyReceipt, VoxelChunkResidencyApplyError> {
    VoxelChunkResidencyService::apply(
        scene,
        leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: scene.source_revision(),
            operations,
        },
    )
}

fn resident_hash(scene: &VoxelCollisionScene, chunk: VoxelChunkIdentity) -> VoxelChunkContentHash {
    VoxelChunkResidencyService::resident_chunk(scene, chunk)
        .unwrap()
        .content_hash
}

#[test]
fn first_admission_publishes_one_coherent_revision() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let leases = VoxelChunkLeaseRegistry::default();
    let chunk = identity(0, 0, 0);
    let operations = [VoxelChunkResidencyOperation::Admit {
        chunk,
        payload: payload(2, &[([0, 0, 0], 3)]),
    }];

    let receipt = apply(&mut scene, &leases, &operations).unwrap();

    assert_eq!(receipt.revision_before, VoxelSourceRevision::INITIAL);
    assert_eq!(receipt.accepted_revision, VoxelSourceRevision::new(1));
    assert_eq!(receipt.admitted, [chunk]);
    assert!(receipt.replaced.is_empty());
    assert!(receipt.evicted.is_empty());
    assert!(receipt.retained.is_empty());
    assert_eq!(receipt.dirty_chunks, [chunk]);
    assert_eq!(receipt.resident_chunk_count, 1);
    assert_eq!(receipt.resident_solid_voxel_count, 1);
    assert_eq!(receipt.rebuilt_mesh_chunks, 1);
    assert!(receipt
        .projections
        .is_coherent_with(receipt.accepted_revision));
    assert_eq!(scene.source_revision(), receipt.accepted_revision);
    assert!(scene.has_collider_chunk(chunk.to_array()));
    assert_eq!(scene.mesh_chunks().len(), 1);
}

#[test]
fn replacement_requires_the_exact_local_chunk_hash() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let leases = VoxelChunkLeaseRegistry::default();
    let chunk = identity(2, 0, -1);
    apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Admit {
            chunk,
            payload: payload(2, &[([0, 0, 0], 1)]),
        }],
    )
    .unwrap();
    let expected_content_hash = resident_hash(&scene, chunk);
    let operations = [VoxelChunkResidencyOperation::Replace {
        chunk,
        expected_content_hash,
        payload: payload(2, &[([1, 1, 1], 7)]),
    }];

    let receipt = apply(&mut scene, &leases, &operations).unwrap();

    assert_eq!(receipt.replaced, [chunk]);
    assert_ne!(resident_hash(&scene, chunk), expected_content_hash);
    assert_eq!(scene.source_revision(), VoxelSourceRevision::new(2));
}

#[test]
fn eviction_removes_authority_collision_and_retained_mesh_together() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let leases = VoxelChunkLeaseRegistry::default();
    let chunk = identity(0, 0, 0);
    apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Admit {
            chunk,
            payload: payload(2, &[([0, 0, 0], 1)]),
        }],
    )
    .unwrap();
    let operations = [VoxelChunkResidencyOperation::Evict {
        chunk,
        expected_content_hash: resident_hash(&scene, chunk),
    }];

    let receipt = apply(&mut scene, &leases, &operations).unwrap();

    assert_eq!(receipt.evicted, [chunk]);
    assert_eq!(receipt.removed_mesh_chunks, 1);
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, chunk).is_none());
    assert!(!scene.has_collider_chunk(chunk.to_array()));
    assert!(scene.mesh_chunks().is_empty());
}

#[test]
fn empty_chunks_are_resident_without_mesh_payloads() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let leases = VoxelChunkLeaseRegistry::default();
    let chunk = identity(-2, 3, -4);

    let receipt = apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Admit {
            chunk,
            payload: payload(2, &[]),
        }],
    )
    .unwrap();

    assert_eq!(receipt.admitted, [chunk]);
    assert_eq!(receipt.rebuilt_mesh_chunks, 0);
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, chunk)
        .unwrap()
        .is_empty());
    assert_eq!(scene.resident_chunk_count(), 1);
    assert!(scene.mesh_chunks().is_empty());

    let history = VoxelEditHistory::new(&scene);
    let encoded = encode_voxel_edit_history(&history).unwrap();
    let restored = decode_voxel_edit_history(&encoded, VoxelEditHistoryLimits::default()).unwrap();
    assert_eq!(
        restored.scene.resident_chunk_coordinates(),
        vec![chunk.to_array()]
    );
    assert!(restored.scene.mesh_chunks().is_empty());
}

#[test]
fn negative_chunk_coordinates_and_world_bounds_are_exact() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let leases = VoxelChunkLeaseRegistry::default();
    let lowest_valid = identity(-500_000, -1, -2);
    apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: lowest_valid,
            payload: payload(2, &[([1, 0, 1], 1)]),
        }],
    )
    .unwrap();
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, lowest_valid).is_some());

    let revision = scene.source_revision();
    let error = apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: identity(500_000, 0, 0),
            payload: payload(2, &[]),
        }],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::ChunkCoordinateOutOfBounds {
                operation_index: 0,
                axis: 0,
                voxel_min: 1_000_000,
                voxel_max_inclusive: 1_000_001,
                ..
            }
        )
    ));
    assert_eq!(scene.source_revision(), revision);
    assert_eq!(scene.resident_chunk_count(), 1);
}

#[test]
fn stale_duplicate_and_stale_chunk_preconditions_fail_atomically() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let leases = VoxelChunkLeaseRegistry::default();
    let chunk = identity(0, 0, 0);
    let admit = VoxelChunkResidencyOperation::Admit {
        chunk,
        payload: payload(2, &[([0, 0, 0], 1)]),
    };

    let stale = VoxelChunkResidencyService::apply(
        &mut scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: VoxelSourceRevision::new(1),
            operations: std::slice::from_ref(&admit),
        },
    )
    .unwrap_err();
    assert!(matches!(
        stale,
        VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::StaleSceneSourceRevision { .. }
        )
    ));
    assert_eq!(scene.resident_chunk_count(), 0);

    let duplicate = [admit.clone(), admit];
    let error = apply(&mut scene, &leases, &duplicate).unwrap_err();
    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::Rejected(VoxelChunkResidencyRejection::DuplicateChunk {
            first_operation_index: 0,
            duplicate_operation_index: 1,
            ..
        })
    ));
    assert_eq!(scene.resident_chunk_count(), 0);

    apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Admit {
            chunk,
            payload: payload(2, &[([0, 0, 0], 1)]),
        }],
    )
    .unwrap();
    let before = resident_hash(&scene, chunk);
    let error = apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Replace {
            chunk,
            expected_content_hash: VoxelChunkContentHash::new(before.raw().wrapping_add(1)),
            payload: payload(2, &[([1, 1, 1], 2)]),
        }],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::StaleChunkContentHash { .. }
        )
    ));
    assert_eq!(resident_hash(&scene, chunk), before);
}

#[test]
fn dimensions_material_and_payload_bounds_are_atomic() {
    let leases = VoxelChunkLeaseRegistry::default();

    let mut dimensions_scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let operations = [
        VoxelChunkResidencyOperation::Admit {
            chunk: identity(0, 0, 0),
            payload: payload(2, &[([0, 0, 0], 1)]),
        },
        VoxelChunkResidencyOperation::Admit {
            chunk: identity(1, 0, 0),
            payload: VoxelChunkPayload::new([1, 2, 2], vec![0; 4]),
        },
    ];
    assert!(matches!(
        apply(&mut dimensions_scene, &leases, &operations),
        Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::PayloadDimensionsMismatch {
                operation_index: 1,
                ..
            }
        ))
    ));
    assert_eq!(dimensions_scene.resident_chunk_count(), 0);

    let mut material_scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let operations = [
        VoxelChunkResidencyOperation::Admit {
            chunk: identity(0, 0, 0),
            payload: payload(2, &[([0, 0, 0], 1)]),
        },
        VoxelChunkResidencyOperation::Admit {
            chunk: identity(1, 0, 0),
            payload: payload(2, &[([1, 1, 1], 4_096)]),
        },
    ];
    assert!(matches!(
        apply(&mut material_scene, &leases, &operations),
        Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::InvalidMaterialSlot {
                operation_index: 1,
                slot_index: 7,
                ..
            }
        ))
    ));
    assert_eq!(material_scene.resident_chunk_count(), 0);

    let mut aggregate_scene = empty_scene(64, SurfaceMode::GreedyCubes);
    let empty = payload(64, &[]);
    let operations = [VoxelChunkResidencyOperation::Admit {
        chunk: identity(0, 0, 0),
        payload: empty,
    }];
    assert_eq!(
        MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION * 64_usize.pow(3),
        MAX_VOXEL_CHUNK_PAYLOAD_SLOTS_PER_TRANSACTION
    );
    let receipt = apply(&mut aggregate_scene, &leases, &operations).unwrap();
    assert_eq!(receipt.resident_chunk_count, 1);
    assert_eq!(aggregate_scene.resident_chunk_count(), 1);
}

#[test]
fn aggregate_resident_solids_are_bounded_before_projection_build() {
    let mut scene = empty_scene(64, SurfaceMode::GreedyCubes);
    let leases = VoxelChunkLeaseRegistry::default();
    let full = VoxelChunkPayload::new([64; 3], vec![1; 64 * 64 * 64]);
    let operations: Vec<_> = (0..4)
        .map(|x| VoxelChunkResidencyOperation::Admit {
            chunk: identity(x, 0, 0),
            payload: full.clone(),
        })
        .collect();

    let error = apply(&mut scene, &leases, &operations).unwrap_err();

    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::ResidentSolidVoxelLimitExceeded {
                limit: MAX_SOLID_VOXELS,
                actual,
            }
        ) if actual > MAX_SOLID_VOXELS
    ));
    assert_eq!(scene.source_revision(), VoxelSourceRevision::INITIAL);
    assert_eq!(scene.resident_chunk_count(), 0);
}

#[test]
fn leases_block_replace_and_evict_with_typed_evidence() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let mut leases = VoxelChunkLeaseRegistry::default();
    let chunk = identity(0, 0, 0);
    apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Admit {
            chunk,
            payload: payload(2, &[([0, 0, 0], 1)]),
        }],
    )
    .unwrap();
    let expected_content_hash = resident_hash(&scene, chunk);
    let lease = leases.acquire(&scene, chunk).unwrap();

    for operation in [
        VoxelChunkResidencyOperation::Replace {
            chunk,
            expected_content_hash,
            payload: payload(2, &[([1, 1, 1], 2)]),
        },
        VoxelChunkResidencyOperation::Evict {
            chunk,
            expected_content_hash,
        },
    ] {
        let error = apply(&mut scene, &leases, &[operation]).unwrap_err();
        assert!(matches!(
            error,
            VoxelChunkResidencyApplyError::Rejected(
                VoxelChunkResidencyRejection::ChunkPinned {
                    operation_index: 0,
                    leases: ref evidence,
                    ..
                }
            ) if evidence == &[lease]
        ));
        assert_eq!(resident_hash(&scene, chunk), expected_content_hash);
    }

    assert_eq!(leases.release(lease.lease_id).unwrap(), lease);
    apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Evict {
            chunk,
            expected_content_hash,
        }],
    )
    .unwrap();
    assert!(!leases.is_pinned(chunk));
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, chunk).is_none());
}

#[test]
fn whole_chunk_dirty_halos_match_every_surface_mode() {
    for (mode, expected_dirty, expected_reused) in [
        (SurfaceMode::GreedyCubes, 7, 20),
        (SurfaceMode::MarchingCubes, 27, 0),
        (SurfaceMode::DualContouring, 27, 0),
    ] {
        let mut surrounding = Vec::new();
        for z in -1_i64..=1 {
            for y in -1_i64..=1 {
                for x in -1_i64..=1 {
                    if [x, y, z] != [0, 0, 0] {
                        surrounding.push(MaterialVoxel {
                            address: [x * 2, y * 2, z * 2],
                            material_slot: 1,
                        });
                    }
                }
            }
        }
        let mut scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
            1.0,
            2,
            surrounding,
            SurfaceMeshOptions {
                mode,
                ..SurfaceMeshOptions::default()
            },
        )
        .unwrap();
        let leases = VoxelChunkLeaseRegistry::default();

        let receipt = apply(
            &mut scene,
            &leases,
            &[VoxelChunkResidencyOperation::Admit {
                chunk: VoxelChunkIdentity::ORIGIN,
                payload: payload(2, &[([0, 0, 0], 1)]),
            }],
        )
        .unwrap();

        assert_eq!(receipt.dirty_chunks.len(), expected_dirty, "{mode:?}");
        assert_eq!(receipt.rebuilt_mesh_chunks, expected_dirty, "{mode:?}");
        assert_eq!(receipt.reused_mesh_chunks, expected_reused, "{mode:?}");
        assert!(receipt.dirty_chunks.contains(&VoxelChunkIdentity::ORIGIN));
        for face in [
            identity(-1, 0, 0),
            identity(1, 0, 0),
            identity(0, -1, 0),
            identity(0, 1, 0),
            identity(0, 0, -1),
            identity(0, 0, 1),
        ] {
            assert!(receipt.dirty_chunks.contains(&face), "{mode:?} {face:?}");
        }
        if mode != SurfaceMode::GreedyCubes {
            assert!(receipt.dirty_chunks.contains(&identity(1, 1, 1)));
            assert!(receipt.dirty_chunks.contains(&identity(-1, -1, -1)));
        }
    }
}

#[test]
fn preparation_and_commit_are_fail_atomic_and_guard_leases() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let mut leases = VoxelChunkLeaseRegistry::default();
    let first = identity(0, 0, 0);
    let second = identity(1, 0, 0);
    let first_operation = [VoxelChunkResidencyOperation::Admit {
        chunk: first,
        payload: payload(2, &[([0, 0, 0], 1)]),
    }];
    let prepared = VoxelChunkResidencyService::prepare(
        &scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: VoxelSourceRevision::INITIAL,
            operations: &first_operation,
        },
    )
    .unwrap();
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, first).is_none());
    assert!(
        VoxelChunkResidencyService::resident_chunk(prepared.candidate_scene(), first).is_some()
    );

    apply(
        &mut scene,
        &leases,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: second,
            payload: payload(2, &[([0, 0, 0], 2)]),
        }],
    )
    .unwrap();
    let error = VoxelChunkResidencyService::commit(&mut scene, &leases, prepared).unwrap_err();
    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::PreparedSceneChanged { .. }
    ));
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, first).is_none());
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, second).is_some());

    let hash = resident_hash(&scene, second);
    let eviction = [VoxelChunkResidencyOperation::Evict {
        chunk: second,
        expected_content_hash: hash,
    }];
    let prepared = VoxelChunkResidencyService::prepare(
        &scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: scene.source_revision(),
            operations: &eviction,
        },
    )
    .unwrap();
    leases.acquire(&scene, second).unwrap();
    let error = VoxelChunkResidencyService::commit(&mut scene, &leases, prepared).unwrap_err();
    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::PreparedLeaseRegistryChanged { .. }
    ));
    assert_eq!(resident_hash(&scene, second), hash);
}

#[test]
fn multi_chunk_operation_bounds_reject_before_any_admission() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let leases = VoxelChunkLeaseRegistry::default();
    let operations: Vec<_> = (0..=MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION)
        .map(|x| VoxelChunkResidencyOperation::Admit {
            chunk: identity(x as i64, 0, 0),
            payload: payload(2, &[]),
        })
        .collect();

    let error = apply(&mut scene, &leases, &operations).unwrap_err();

    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::TooManyOperations {
                limit: MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION,
                actual,
            }
        ) if actual == MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION + 1
    ));
    assert_eq!(scene.resident_chunk_count(), 0);
}

#[test]
fn mixed_retained_operations_are_reported_but_all_retained_is_no_change() {
    let existing_payload = payload(2, &[([0, 0, 0], 1)]);
    let mut scene = VoxelCollisionScene::from_material_voxels(
        1.0,
        2,
        [MaterialVoxel {
            address: [0, 0, 0],
            material_slot: 1,
        }],
    )
    .unwrap();
    let leases = VoxelChunkLeaseRegistry::default();
    let existing = identity(0, 0, 0);
    let admitted = identity(1, 0, 0);
    let operations = [
        VoxelChunkResidencyOperation::Admit {
            chunk: existing,
            payload: existing_payload.clone(),
        },
        VoxelChunkResidencyOperation::Admit {
            chunk: admitted,
            payload: existing_payload.clone(),
        },
    ];

    let receipt = apply(&mut scene, &leases, &operations).unwrap();
    assert_eq!(receipt.admitted, [admitted]);
    assert_eq!(receipt.retained, [existing]);
    assert_eq!(receipt.dirty_chunks, [existing, admitted]);

    let revision = scene.source_revision();
    let operations = [
        VoxelChunkResidencyOperation::Admit {
            chunk: existing,
            payload: existing_payload.clone(),
        },
        VoxelChunkResidencyOperation::Replace {
            chunk: admitted,
            expected_content_hash: resident_hash(&scene, admitted),
            payload: existing_payload,
        },
    ];
    let error = apply(&mut scene, &leases, &operations).unwrap_err();
    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::NoChanges { retained }
        ) if retained == vec![existing, admitted]
    ));
    assert_eq!(scene.source_revision(), revision);
    assert_eq!(scene.resident_chunk_count(), 2);
}

#[test]
fn residency_history_policy_rejects_or_resets_without_resurrection() {
    let mut scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        2,
        [
            MaterialVoxel {
                address: [0, 0, 0],
                material_slot: 1,
            },
            MaterialVoxel {
                address: [1, 0, 0],
                material_slot: 1,
            },
        ],
        SurfaceMeshOptions {
            mode: SurfaceMode::MarchingCubes,
            ..SurfaceMeshOptions::default()
        },
    )
    .unwrap();
    let mut history = VoxelEditHistory::new(&scene);
    history
        .apply(&mut scene, &[VoxelEdit::Clear { address: [1, 0, 0] }])
        .unwrap();
    let leases = VoxelChunkLeaseRegistry::default();
    let chunk = identity(0, 0, 0);
    let operation = VoxelChunkResidencyOperation::Evict {
        chunk,
        expected_content_hash: resident_hash(&scene, chunk),
    };
    let revision = scene.source_revision();

    let rejected = VoxelChunkResidencyService::apply_with_history(
        &mut scene,
        &leases,
        &mut history,
        VoxelResidencyHistoryPolicy::RejectIfNonEmpty,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: revision,
            operations: std::slice::from_ref(&operation),
        },
    )
    .unwrap_err();
    assert!(matches!(
        rejected,
        VoxelChunkResidencyApplyError::Rejected(VoxelChunkResidencyRejection::HistoryNotEmpty {
            entry_count: 1,
            cursor: 1,
        })
    ));
    assert_eq!(scene.source_revision(), revision);
    assert_eq!(history.entries().len(), 1);

    let receipt = VoxelChunkResidencyService::apply_with_history(
        &mut scene,
        &leases,
        &mut history,
        VoxelResidencyHistoryPolicy::ResetToPublishedAuthority,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: revision,
            operations: std::slice::from_ref(&operation),
        },
    )
    .unwrap();
    let reset = receipt.history_reset.unwrap();
    assert_eq!(reset.invalidated_entries, 1);
    assert!(history.entries().is_empty());
    assert!(matches!(
        history.undo_one(&mut scene),
        Err(VoxelEditHistoryError::EmptyUndoStack)
    ));
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, chunk).is_none());

    let encoded = encode_voxel_edit_history(&history).unwrap();
    let restored = decode_voxel_edit_history(&encoded, VoxelEditHistoryLimits::default()).unwrap();
    assert_eq!(
        restored.scene.resident_chunk_coordinates(),
        scene.resident_chunk_coordinates()
    );
    assert_eq!(restored.scene.mesh_options(), scene.mesh_options());
}
