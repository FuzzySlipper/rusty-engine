use svc_mesh::SurfaceMode;
use voxel_asset::{
    encode_voxel_object, with_computed_voxel_object_hashes, VoxelAssetBounds,
    VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelCoordinateSystem, VoxelFrame,
    VoxelObjectAnimationFrame, VoxelObjectAsset, VoxelObjectClip, VoxelObjectCollisionPrimitive,
    VoxelObjectFrameAnchor, VoxelObjectFrameCollision, VoxelObjectGrid, VoxelObjectHitRegion,
    VoxelObjectProvenance, VoxelObjectProvenanceKind, VoxelRepresentation, VoxelRepresentationKind,
    VoxelSparseRun, VOXEL_OBJECT_SCHEMA_VERSION,
};
use voxel_object_runtime::{
    admit_voxel_object, admit_voxel_object_json, admit_voxel_object_with_options,
    VoxelObjectAdmissionError, VoxelObjectAdmissionOptions, VoxelObjectCollisionPolicy,
    VoxelObjectCollisionResolution, VoxelObjectLoopMode, VoxelObjectPlaybackRate,
    VoxelObjectPlaybackStatus, VoxelObjectPlayer, VoxelObjectPlayerError, VoxelObjectRuntimeLimits,
};

#[test]
fn admission_resolves_readouts_and_deduplicates_identical_frame_meshes() {
    let object = object();
    let admitted = admit_voxel_object(&object, VoxelObjectRuntimeLimits::default()).unwrap();

    assert_eq!(admitted.readout().asset_id, "voxel-object/test-runner");
    assert_eq!(admitted.readout().frame_count, 4);
    assert_eq!(admitted.readout().clip_count, 2);
    assert_eq!(admitted.readout().unique_mesh_count, 3);
    assert_eq!(
        admitted.frames()[0].mesh_index,
        admitted.frames()[1].mesh_index
    );
    assert_eq!(
        admitted.frames()[1].anchor("head").unwrap().position,
        [0.0, 2.0, 0.0]
    );
    assert!(admitted.frames()[1].anchor("missing").is_none());
    assert!(matches!(
        admitted.frames()[1]
            .collision
            .as_ref()
            .unwrap()
            .body
            .as_ref(),
        Some(VoxelObjectCollisionPrimitive::Capsule { radius: 0.75, .. })
    ));
    assert_eq!(
        admitted.frames()[1]
            .collision
            .as_ref()
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .capsule_axis_endpoints(),
        Some(([0.0, 0.0, 0.0], [0.0, 2.0, 0.0]))
    );
    assert_eq!(
        admitted.frames()[1]
            .collision
            .as_ref()
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .local_bounds(),
        ([-0.75, -0.75, -0.75], [0.75, 2.75, 0.75])
    );
    assert_eq!(
        admitted.frames()[1].collision.as_ref().unwrap().hit_regions[0].id,
        "head"
    );
    assert!(
        admitted.frames()[0].anchors.is_empty() && admitted.frames()[0].collision.is_none(),
        "default geometry has no implicit authored facts"
    );
    assert_eq!(admitted.clip("walk").unwrap().frame_indices, [2, 3]);
    assert_eq!(
        admitted.clip("walk").unwrap().frame_durations_micros,
        [100_000, 83_333]
    );
    assert_eq!(admitted.meshes()[0].bounds.min, [-0.125, 0.0, 0.0]);
    assert_eq!(admitted.meshes()[0].bounds.max, [0.375, 0.25, 0.25]);

    let again = admit_voxel_object(&object, VoxelObjectRuntimeLimits::default()).unwrap();
    assert_eq!(admitted.meshes().len(), again.meshes().len());
    for (left, right) in admitted.meshes().iter().zip(again.meshes()) {
        assert_eq!(left.as_ref(), right.as_ref());
    }
}

#[test]
fn strict_json_and_runtime_work_limits_fail_closed() {
    let encoded = encode_voxel_object(&object()).unwrap();
    let with_unknown = encoded.replacen(
        "\"schemaVersion\": 1,",
        "\"schemaVersion\": 1, \"ambientRuntime\": true,",
        1,
    );
    assert!(matches!(
        admit_voxel_object_json(&with_unknown, VoxelObjectRuntimeLimits::default()),
        Err(VoxelObjectAdmissionError::Asset(_))
    ));

    let tiny = VoxelObjectRuntimeLimits {
        max_frames: 3,
        ..VoxelObjectRuntimeLimits::default()
    };
    assert!(matches!(
        admit_voxel_object(&object(), tiny),
        Err(VoxelObjectAdmissionError::FrameLimit { count: 4, limit: 3 })
    ));

    let tiny = VoxelObjectRuntimeLimits {
        max_unique_mesh_faces: 5,
        ..VoxelObjectRuntimeLimits::default()
    };
    assert!(matches!(
        admit_voxel_object(&object(), tiny),
        Err(VoxelObjectAdmissionError::MeshFaceLimit { limit: 5, .. })
    ));
}

#[test]
fn reconstructed_surface_admission_covers_animation_and_aggregate_limits() {
    for surface_mode in [SurfaceMode::MarchingCubes, SurfaceMode::DualContouring] {
        let admitted = admit_voxel_object_with_options(
            &object(),
            VoxelObjectAdmissionOptions {
                surface_mode,
                ..VoxelObjectAdmissionOptions::default()
            },
        )
        .unwrap();
        assert_eq!(admitted.surface_mode(), surface_mode);
        assert_eq!(admitted.readout().surface_mode, surface_mode);
        assert!(admitted
            .meshes()
            .iter()
            .all(|mesh| mesh.surface_mode == surface_mode && mesh.tile_coordinates.is_empty()));
        assert_ne!(
            admitted.frames()[2].mesh_index,
            admitted.frames()[3].mesh_index,
            "animation-frame replacement retains distinct reconstructed resources"
        );
    }

    let error = admit_voxel_object_with_options(
        &object(),
        VoxelObjectAdmissionOptions {
            surface_mode: SurfaceMode::DualContouring,
            limits: VoxelObjectRuntimeLimits {
                max_sampled_cells: 1,
                ..VoxelObjectRuntimeLimits::default()
            },
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        VoxelObjectAdmissionError::SampledCellLimit { limit: 1, .. }
    ));
}

#[test]
fn caller_driven_play_pause_resume_stop_and_reload_are_explicit_time() {
    let admitted = admit_voxel_object(&object(), VoxelObjectRuntimeLimits::default()).unwrap();
    let mut player = VoxelObjectPlayer::new();
    player
        .play(
            &admitted,
            "walk",
            VoxelObjectLoopMode::Repeat,
            VoxelObjectPlaybackRate::NORMAL,
            1_000_000,
        )
        .unwrap();
    assert_eq!(
        player.sample_at(&admitted, 1_099_999).unwrap().clip_frame,
        Some(0)
    );
    assert_eq!(
        player.sample_at(&admitted, 1_100_000).unwrap().clip_frame,
        Some(1)
    );
    assert_eq!(
        player.sample_at(&admitted, 1_183_333).unwrap().clip_frame,
        Some(0)
    );

    player.pause(1_100_000).unwrap();
    assert_eq!(
        player.sample_at(&admitted, 9_000_000).unwrap().clip_frame,
        Some(1)
    );
    let paused = player.posture_at(9_000_000).unwrap();
    assert_eq!(paused.status, VoxelObjectPlaybackStatus::Paused);
    assert_eq!(paused.elapsed_micros, 100_000);

    player.resume(10_000_000).unwrap();
    let durable = player.posture_at(10_050_000).unwrap();
    let restored = VoxelObjectPlayer::restore(&admitted, durable.clone(), 99).unwrap();
    assert_eq!(restored.posture_at(99).unwrap(), durable);
    assert_eq!(
        restored.sample_at(&admitted, 33_432).unwrap().clip_frame,
        Some(0)
    );

    player.stop();
    let stopped = player.sample_at(&admitted, 20_000_000).unwrap();
    assert_eq!(stopped.status, VoxelObjectPlaybackStatus::Stopped);
    assert_eq!(stopped.frame, 0);
    assert_eq!(stopped.clip, None);
}

#[test]
fn scrub_selects_an_exact_clip_frame_and_resume_uses_explicit_time() {
    let admitted = admit_voxel_object(&object(), VoxelObjectRuntimeLimits::default()).unwrap();
    let mut player = VoxelObjectPlayer::new();

    player
        .scrub(&admitted, "walk", 1, VoxelObjectLoopMode::Repeat)
        .unwrap();
    let scrubbed = player.sample_at(&admitted, 9_000_000).unwrap();
    assert_eq!(scrubbed.status, VoxelObjectPlaybackStatus::Paused);
    assert_eq!(scrubbed.clip_frame, Some(1));
    assert_eq!(scrubbed.frame, 3);
    assert_eq!(scrubbed.elapsed_micros, 100_000);

    player.resume(9_000_000).unwrap();
    let resumed = player.sample_at(&admitted, 9_100_000).unwrap();
    assert_eq!(resumed.status, VoxelObjectPlaybackStatus::Playing);
    assert_eq!(resumed.clip_frame, Some(0));
    assert_eq!(resumed.frame, 2);

    assert!(matches!(
        player.scrub(&admitted, "walk", 99, VoxelObjectLoopMode::Repeat),
        Err(VoxelObjectPlayerError::ClipFrameOutOfRange {
            frame: 99,
            frame_count: 2,
            ..
        })
    ));
}

#[test]
fn once_repeat_ping_pong_speed_and_invalid_clips_are_deterministic() {
    let mut source = object_source();
    let walk = source
        .clips
        .iter_mut()
        .find(|clip| clip.id == "walk")
        .unwrap();
    walk.frames_per_second = 10.0;
    walk.frames[1].duration_seconds = Some(0.1);
    walk.frames.push(VoxelObjectAnimationFrame {
        duration_seconds: Some(0.1),
        anchors: Vec::new(),
        collision: None,
        frame: frame([0, 0, 1], 1, 1),
    });
    source.bounds.max[2] = 1;
    let source = with_computed_voxel_object_hashes(source).unwrap();
    let admitted = admit_voxel_object(&source, VoxelObjectRuntimeLimits::default()).unwrap();

    let mut player = VoxelObjectPlayer::new();
    player
        .play(
            &admitted,
            "walk",
            VoxelObjectLoopMode::Once,
            VoxelObjectPlaybackRate::new(2, 1).unwrap(),
            0,
        )
        .unwrap();
    assert_eq!(
        player.sample_at(&admitted, 50_000).unwrap().clip_frame,
        Some(1)
    );
    let ended = player.sample_at(&admitted, 150_000).unwrap();
    assert_eq!(ended.clip_frame, Some(2));
    assert!(ended.ended);

    player
        .play(
            &admitted,
            "walk",
            VoxelObjectLoopMode::PingPong,
            VoxelObjectPlaybackRate::NORMAL,
            0,
        )
        .unwrap();
    assert_eq!(player.sample_at(&admitted, 0).unwrap().clip_frame, Some(0));
    assert_eq!(
        player.sample_at(&admitted, 100_000).unwrap().clip_frame,
        Some(1)
    );
    assert_eq!(
        player.sample_at(&admitted, 200_000).unwrap().clip_frame,
        Some(2)
    );
    assert_eq!(
        player.sample_at(&admitted, 300_000).unwrap().clip_frame,
        Some(1)
    );
    assert_eq!(
        player.sample_at(&admitted, 400_000).unwrap().clip_frame,
        Some(0)
    );

    assert!(matches!(
        player.play(
            &admitted,
            "missing",
            VoxelObjectLoopMode::Repeat,
            VoxelObjectPlaybackRate::NORMAL,
            0,
        ),
        Err(VoxelObjectPlayerError::UnknownClip { .. })
    ));
}

#[test]
fn presentation_frames_never_change_explicit_collision_selection() {
    let admitted = admit_voxel_object(&object(), VoxelObjectRuntimeLimits::default()).unwrap();
    let policy = VoxelObjectCollisionPolicy::StableClipFrame {
        clip: "walk".to_owned(),
        frame: 0,
    };
    let VoxelObjectCollisionResolution::StableFrame(collision_before) =
        admitted.resolve_collision(&policy).unwrap()
    else {
        panic!("expected stable collision frame")
    };

    let mut player = VoxelObjectPlayer::new();
    player
        .play(
            &admitted,
            "walk",
            VoxelObjectLoopMode::Repeat,
            VoxelObjectPlaybackRate::NORMAL,
            0,
        )
        .unwrap();
    assert_ne!(
        player.sample_at(&admitted, 0).unwrap().frame,
        player.sample_at(&admitted, 100_000).unwrap().frame
    );
    let VoxelObjectCollisionResolution::StableFrame(collision_after) =
        admitted.resolve_collision(&policy).unwrap()
    else {
        panic!("expected stable collision frame")
    };
    assert!(std::ptr::eq(collision_before, collision_after));
    assert_eq!(
        collision_before.voxel_data_hash,
        collision_after.voxel_data_hash
    );

    let posture = player.posture_at(100_000).unwrap();
    let encoded = serde_json::to_string(&posture).unwrap();
    assert_eq!(
        serde_json::from_str::<voxel_object_runtime::VoxelObjectPlaybackPosture>(&encoded).unwrap(),
        posture
    );
}

fn object() -> VoxelObjectAsset {
    with_computed_voxel_object_hashes(object_source()).unwrap()
}

fn object_source() -> VoxelObjectAsset {
    let default_frame = frame([0, 0, 0], 2, 1);
    let idle = VoxelObjectClip {
        id: "idle".to_owned(),
        name: Some("Idle".to_owned()),
        frames_per_second: 6.0,
        frames: vec![VoxelObjectAnimationFrame {
            duration_seconds: None,
            anchors: vec![VoxelObjectFrameAnchor {
                id: "head".to_owned(),
                position: [0.0, 2.0, 0.0],
            }],
            collision: Some(VoxelObjectFrameCollision {
                body: Some(VoxelObjectCollisionPrimitive::Capsule {
                    center: [0.0, 1.0, 0.0],
                    radius: 0.75,
                    half_height: 1.0,
                }),
                hit_regions: vec![VoxelObjectHitRegion {
                    id: "head".to_owned(),
                    primitive: VoxelObjectCollisionPrimitive::Box {
                        center: [0.0, 2.0, 0.0],
                        half_extents: [0.5, 0.5, 0.5],
                    },
                }],
            }),
            frame: default_frame.clone(),
        }],
    };
    let walk = VoxelObjectClip {
        id: "walk".to_owned(),
        name: Some("Walk".to_owned()),
        frames_per_second: 12.0,
        frames: vec![
            VoxelObjectAnimationFrame {
                duration_seconds: Some(0.1),
                anchors: Vec::new(),
                collision: None,
                frame: frame([0, 0, 0], 1, 1),
            },
            VoxelObjectAnimationFrame {
                duration_seconds: None,
                anchors: Vec::new(),
                collision: None,
                frame: VoxelFrame {
                    bounds: VoxelAssetBounds {
                        min: [0, 0, 0],
                        max: [0, 1, 0],
                    },
                    representation: VoxelRepresentation {
                        kind: VoxelRepresentationKind::SparseRuns,
                        sparse_runs: vec![run([0, 1, 0], 1, 2), run([0, 0, 0], 1, 1)],
                    },
                    voxel_data_hash: String::new(),
                },
            },
        ],
    };
    VoxelObjectAsset {
        schema_version: VOXEL_OBJECT_SCHEMA_VERSION,
        asset_id: "voxel-object/test-runner".to_owned(),
        grid: VoxelObjectGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: 0.25,
            chunk_size: 16,
            pivot: [0.5, 0.0, 0.0],
        },
        bounds: VoxelAssetBounds {
            min: [0, 0, 0],
            max: [1, 1, 0],
        },
        default_frame,
        clips: vec![walk, idle],
        default_clip: Some("walk".to_owned()),
        material_palette: vec![
            VoxelAssetMaterialBinding {
                material_slot: 2,
                material_asset_id: "material/cloth".to_owned(),
                display_name: Some("Cloth".to_owned()),
            },
            VoxelAssetMaterialBinding {
                material_slot: 1,
                material_asset_id: "material/skin".to_owned(),
                display_name: Some("Skin".to_owned()),
            },
        ],
        material_map: vec![
            VoxelAssetMaterialMapping {
                source_material_slot: 1,
                source_material_name: Some("cloth".to_owned()),
                voxel_material_slot: 2,
            },
            VoxelAssetMaterialMapping {
                source_material_slot: 0,
                source_material_name: Some("skin".to_owned()),
                voxel_material_slot: 1,
            },
        ],
        provenance: VoxelObjectProvenance {
            kind: VoxelObjectProvenanceKind::ConvertedAnimatedMesh,
            source_path: "models/runner.glb".to_owned(),
            source_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            source_byte_count: 1024,
            converter: "rusty-engine.mesh-to-voxel-object.v1".to_owned(),
            settings_sha256:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            license_path: Some("models/LICENSE.txt".to_owned()),
            source_clips: vec![],
        },
        content_hash: String::new(),
    }
}

fn frame(start: [i64; 3], length: u32, material_slot: u16) -> VoxelFrame {
    VoxelFrame {
        bounds: VoxelAssetBounds {
            min: start,
            max: [start[0] + i64::from(length) - 1, start[1], start[2]],
        },
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs: vec![run(start, length, material_slot)],
        },
        voxel_data_hash: String::new(),
    }
}

fn run(start: [i64; 3], length: u32, material_slot: u16) -> VoxelSparseRun {
    VoxelSparseRun {
        start,
        length,
        material_slot,
    }
}
