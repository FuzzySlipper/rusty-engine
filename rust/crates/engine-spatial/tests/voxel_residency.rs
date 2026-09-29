use engine_spatial::{
    MaterialVoxel, SurfaceMeshOptions, SurfaceMode, VoxelChunkIdentity, VoxelChunkPayload,
    VoxelChunkResidencyApplyError, VoxelChunkResidencyOperation, VoxelChunkResidencyRejection,
    VoxelChunkResidencyService, VoxelCollisionScene, VoxelEdit, VoxelEditService,
    VoxelSourceRevision,
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
    operations: &[VoxelChunkResidencyOperation],
) -> Result<engine_spatial::VoxelChunkResidencyReceipt, VoxelChunkResidencyApplyError> {
    VoxelChunkResidencyService::apply(scene, operations)
}

#[test]
fn first_admission_publishes_one_coherent_revision() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let chunk = identity(0, 0, 0);
    let operations = [VoxelChunkResidencyOperation::Admit {
        chunk,
        payload: payload(2, &[([0, 0, 0], 3)]),
    }];

    let receipt = apply(&mut scene, &operations).unwrap();

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
fn streamed_sparse_boundary_hit_is_readable_and_clearable_at_the_same_revision() {
    let mut scene = empty_scene(8, SurfaceMode::GreedyCubes);
    let chunk = identity(0, 0, 0);
    let admitted = apply(
        &mut scene,
        &[VoxelChunkResidencyOperation::Admit {
            chunk,
            payload: payload(8, &[([7, 3, 6], 1)]),
        }],
    )
    .unwrap();

    // The downward ray reaches the sparse solid's top face at the boundary of
    // the empty cells (8,3,7). Its reported coordinate must remain the
    // Compound child that owns the collision, rather than that empty neighbour.
    let hit = scene
        .raycast([8.0, 5.0, 7.0], [0.0, -1.0, 0.0], 10.0)
        .expect("streamed sparse voxel should be hit");
    assert_eq!(hit.voxel, [7, 3, 6]);
    assert_eq!(hit.face, core_space::Face::PosY);
    assert!(scene
        .material_voxels()
        .iter()
        .any(|voxel| voxel.address == hit.voxel && voxel.material_slot == 1));
    assert_eq!(scene.source_revision(), admitted.accepted_revision);

    let cleared =
        VoxelEditService::apply(&mut scene, &[VoxelEdit::Clear { address: hit.voxel }]).unwrap();
    assert_eq!(cleared.revision_before, admitted.accepted_revision);
    assert!(!scene
        .material_voxels()
        .iter()
        .any(|voxel| voxel.address == hit.voxel));
}

#[test]
fn eviction_removes_authority_collision_and_retained_mesh_together() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let chunk = identity(0, 0, 0);
    apply(
        &mut scene,
        &[VoxelChunkResidencyOperation::Admit {
            chunk,
            payload: payload(2, &[([0, 0, 0], 1)]),
        }],
    )
    .unwrap();
    let operations = [VoxelChunkResidencyOperation::Evict { chunk }];

    let receipt = apply(&mut scene, &operations).unwrap();

    assert_eq!(receipt.evicted, [chunk]);
    assert_eq!(receipt.removed_mesh_chunks, 1);
    assert!(VoxelChunkResidencyService::resident_chunk(&scene, chunk).is_none());
    assert!(!scene.has_collider_chunk(chunk.to_array()));
    assert!(scene.mesh_chunks().len() == 0);
}

#[test]
fn empty_chunks_are_resident_without_mesh_payloads() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let chunk = identity(-2, 3, -4);

    let receipt = apply(
        &mut scene,
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
    assert!(scene.mesh_chunks().len() == 0);
}

#[test]
fn negative_chunk_coordinates_and_world_bounds_are_exact() {
    let mut scene = empty_scene(2, SurfaceMode::GreedyCubes);
    let lowest_valid = identity(-500_000, -1, -2);
    apply(
        &mut scene,
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
fn dimensions_material_and_payload_bounds_are_atomic() {
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
        apply(&mut dimensions_scene, &operations),
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
        apply(&mut material_scene, &operations),
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
    let receipt = apply(&mut aggregate_scene, &operations).unwrap();
    assert_eq!(receipt.resident_chunk_count, 1);
    assert_eq!(aggregate_scene.resident_chunk_count(), 1);
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
                            state: 0,
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

        let receipt = apply(
            &mut scene,
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
fn mixed_retained_operations_are_reported_but_all_retained_is_no_change() {
    let existing_payload = payload(2, &[([0, 0, 0], 1)]);
    let mut scene = VoxelCollisionScene::from_material_voxels(
        1.0,
        2,
        [MaterialVoxel {
            state: 0,
            address: [0, 0, 0],
            material_slot: 1,
        }],
    )
    .unwrap();
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

    let receipt = apply(&mut scene, &operations).unwrap();
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
            payload: existing_payload,
        },
    ];
    let error = apply(&mut scene, &operations).unwrap_err();
    assert!(matches!(
        error,
        VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::NoChanges { retained }
        ) if retained == vec![existing, admitted]
    ));
    assert_eq!(scene.source_revision(), revision);
    assert_eq!(scene.resident_chunk_count(), 2);
}
