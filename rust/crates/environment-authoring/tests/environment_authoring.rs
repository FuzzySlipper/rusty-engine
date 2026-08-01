use authored_scene::{
    FlatSceneDocument, NodeMetadata, SceneBootstrapBindings, SceneGeneratorBinding, SceneMetadata,
    SceneNodeKind, SceneNodeRecord, SceneTransform, CURRENT_SCENE_SCHEMA_VERSION,
};
use core_assets::{AssetHash, AssetId, AssetReference, AssetVersionReq};
use core_ids::{SceneId, SceneNodeId};
use core_math::Vec3;
use environment_authoring::{
    generate_tunnel, materialize_environment, EnvironmentLimits, EnvironmentMarkerTarget,
    EnvironmentMaterializationError, EnvironmentMaterializationRequest, EnvironmentTarget,
    TunnelGenerationError, TunnelGeneratorConfig, TUNNEL_GENERATOR_ID,
};
use voxel_asset::{decode_voxel_asset, VoxelAssetMaterialBinding, VoxelAssetProvenanceKind};

#[test]
fn tunnel_generation_is_bounded_repeatable_and_seed_sensitive() {
    let first = generate_tunnel(TunnelGeneratorConfig::tiny_enclosed(41)).unwrap();
    let repeated = generate_tunnel(TunnelGeneratorConfig::tiny_enclosed(41)).unwrap();
    let variation = generate_tunnel(TunnelGeneratorConfig::tiny_enclosed(42)).unwrap();

    assert_eq!(first, repeated);
    assert_ne!(
        first.provenance.voxel_data_sha256,
        variation.provenance.voxel_data_sha256
    );
    assert_eq!(first.config.shell_dimensions(), [7, 6, 11]);
    assert_eq!(first.spawn_markers.len(), 2);
    assert_eq!(first.frame.local_offset, Vec3::new(-3.5, -1.0, -5.5));
    assert_eq!(first.frame.playable_min, Vec3::new(-2.5, 0.0, -4.5));
    assert_eq!(first.frame.playable_max, Vec3::new(2.5, 4.0, 4.5));
}

#[test]
fn generated_collision_cells_cover_the_shell_but_not_spawn_space() {
    let tunnel = generate_tunnel(TunnelGeneratorConfig::tiny_enclosed(7)).unwrap();
    let collision = tunnel.collision_aabbs().collect::<Vec<_>>();

    assert!(collision.iter().any(|cell| cell.address == [0, 0, 0]));
    assert!(!collision.iter().any(|cell| cell.address == [2, 2, 2]));
    assert!(collision
        .iter()
        .all(|cell| cell.max[0] > cell.min[0] && cell.max[1] > cell.min[1]));
}

#[test]
fn invalid_generator_configuration_is_classified_before_allocation() {
    let mut config = TunnelGeneratorConfig::tiny_enclosed(1);
    config.voxel_size = f64::NAN;
    assert!(matches!(
        generate_tunnel(config),
        Err(TunnelGenerationError::InvalidVoxelSize { .. })
    ));

    let mut config = TunnelGeneratorConfig::tiny_enclosed(1);
    config.chunk_size = 4;
    assert!(matches!(
        generate_tunnel(config),
        Err(TunnelGenerationError::ExceedsChunk { .. })
    ));

    let mut config = TunnelGeneratorConfig::tiny_enclosed(1);
    config.accent_material = config.wall_material;
    assert!(matches!(
        generate_tunnel(config),
        Err(TunnelGenerationError::DuplicateMaterial { .. })
    ));

    for voxel_size in [f64::MAX, f64::from(f32::MAX), f64::MIN_POSITIVE] {
        let mut config = TunnelGeneratorConfig::tiny_enclosed(1);
        config.voxel_size = voxel_size;
        assert!(matches!(
            generate_tunnel(config),
            Err(TunnelGenerationError::InvalidVoxelSize { .. })
        ));
    }
}

#[test]
fn materialization_produces_repeatable_native_asset_and_scene_candidates() {
    let scene = base_scene(false);
    let request = request(42, None);
    let first = materialize_environment(&scene, &request).unwrap();
    let repeated = materialize_environment(&scene, &request).unwrap();

    assert_eq!(first.asset_json, repeated.asset_json);
    assert_eq!(first.scene_json, repeated.scene_json);
    assert_eq!(
        first.asset.provenance.kind,
        VoxelAssetProvenanceKind::GeneratedEnvironment
    );
    assert_eq!(decode_voxel_asset(&first.asset_json).unwrap(), first.asset);
    assert_eq!(
        first.asset.voxel_data_hash,
        first.generation.provenance.voxel_data_sha256
    );
    assert_eq!(first.revision_before, 0);
    assert_eq!(first.revision_after, 1);
    assert_eq!(first.markers.len(), 2);
    assert!(first.scene.nodes.iter().all(|node| {
        !matches!(
            &node.kind,
            SceneNodeKind::Bootstrap(bindings) if bindings.generator.is_some()
        )
    }));
    assert!(first
        .scene
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, SceneNodeKind::Marker(_)))
        .all(|node| node.parent == Some(SceneNodeId::new(10))));
}

#[test]
fn scene_parent_and_generated_local_transforms_compose_explicitly() {
    let scene = base_scene(true);
    let request = request(42, Some(SceneNodeId::new(2)));
    let output = materialize_environment(&scene, &request).unwrap();
    let player = output
        .markers
        .iter()
        .find(|marker| marker.source_marker_id == "player_start")
        .unwrap();

    assert_eq!(
        output.voxel_world_transform.translation,
        Vec3::new(6.5, -1.0, -5.5)
    );
    assert_eq!(player.local_transform.translation, Vec3::new(2.5, 2.5, 2.5));
    assert_eq!(
        player.world_transform.translation,
        Vec3::new(9.0, 1.5, -3.0)
    );
}

#[test]
fn stale_bounded_and_invalid_palette_requests_leave_source_unchanged() {
    let scene = base_scene(false);
    let original = scene.clone();
    let mut stale = request(42, None);
    stale.expected_scene_revision = 9;
    assert!(matches!(
        materialize_environment(&scene, &stale),
        Err(EnvironmentMaterializationError::StaleSceneRevision { .. })
    ));

    let mut bounded = request(42, None);
    bounded.limits.max_voxels = 1;
    assert!(matches!(
        materialize_environment(&scene, &bounded),
        Err(EnvironmentMaterializationError::ResourceLimit {
            resource: "voxels",
            ..
        })
    ));

    let mut invalid_palette = request(42, None);
    invalid_palette.material_palette.pop();
    assert!(matches!(
        materialize_environment(&scene, &invalid_palette),
        Err(EnvironmentMaterializationError::InvalidVoxelAsset(_))
    ));
    assert_eq!(scene, original);
}

#[test]
fn materialization_rebuilds_dependencies_from_the_resulting_nodes() {
    let mut scene = base_scene(false);
    let old_reference = AssetReference::new(
        AssetId::parse("voxel-volume/old-tunnel").unwrap(),
        AssetVersionReq::Any,
        None,
    );
    scene.dependencies.push(old_reference.clone());
    scene.nodes.push(SceneNodeRecord {
        id: SceneNodeId::new(10),
        parent: None,
        child_order: 2,
        transform: SceneTransform::IDENTITY,
        renderable_transform: SceneTransform::IDENTITY,
        kind: SceneNodeKind::VoxelVolume(old_reference),
        metadata: NodeMetadata::default(),
    });

    let output = materialize_environment(&scene, &request(42, None)).unwrap();

    assert_eq!(output.scene.dependencies.len(), 1);
    assert_eq!(
        output.scene.dependencies[0].id().as_str(),
        "voxel-volume/generated-tunnel"
    );
}

#[test]
fn materialization_rejects_conflicting_constraints_for_a_shared_asset_id() {
    let mut scene = base_scene(false);
    let pinned = AssetReference::new(
        AssetId::parse("voxel-volume/generated-tunnel").unwrap(),
        AssetVersionReq::Exact(2),
        Some(AssetHash::parse("aa11").unwrap()),
    );
    scene.dependencies.push(pinned.clone());
    scene.nodes.push(SceneNodeRecord {
        id: SceneNodeId::new(20),
        parent: None,
        child_order: 2,
        transform: SceneTransform::IDENTITY,
        renderable_transform: SceneTransform::IDENTITY,
        kind: SceneNodeKind::VoxelVolume(pinned),
        metadata: NodeMetadata::default(),
    });

    let error = materialize_environment(&scene, &request(42, None)).unwrap_err();

    assert!(matches!(
        error,
        EnvironmentMaterializationError::ConflictingAssetDependency { asset_id }
            if asset_id == "voxel-volume/generated-tunnel"
    ));
}

fn base_scene(with_parent: bool) -> FlatSceneDocument {
    let mut scene = FlatSceneDocument {
        id: SceneId::new(7),
        revision: 0,
        schema_version: CURRENT_SCENE_SCHEMA_VERSION,
        metadata: SceneMetadata {
            name: Some("generated tunnel".to_string()),
            authoring_format_version: CURRENT_SCENE_SCHEMA_VERSION,
        },
        dependencies: Vec::new(),
        nodes: vec![SceneNodeRecord {
            id: SceneNodeId::new(1),
            parent: None,
            child_order: 0,
            transform: SceneTransform::IDENTITY,
            renderable_transform: SceneTransform::IDENTITY,
            kind: SceneNodeKind::Bootstrap(SceneBootstrapBindings {
                generator: Some(SceneGeneratorBinding {
                    provider_id: TUNNEL_GENERATOR_ID.to_string(),
                    preset_id: "tiny-enclosed".to_string(),
                    seed: 42,
                }),
                catalogs: Vec::new(),
            }),
            metadata: NodeMetadata::default(),
        }],
    };
    if with_parent {
        scene.nodes.push(SceneNodeRecord {
            id: SceneNodeId::new(2),
            parent: None,
            child_order: 1,
            transform: SceneTransform::at(Vec3::new(10.0, 0.0, 0.0)),
            renderable_transform: SceneTransform::IDENTITY,
            kind: SceneNodeKind::EmptyGroup,
            metadata: NodeMetadata::default(),
        });
    }
    scene
}

fn request(seed: u64, parent: Option<SceneNodeId>) -> EnvironmentMaterializationRequest {
    EnvironmentMaterializationRequest {
        expected_scene_revision: 0,
        config: TunnelGeneratorConfig::tiny_enclosed(seed),
        target: EnvironmentTarget {
            voxel_asset_id: "voxel-volume/generated-tunnel".to_string(),
            voxel_node_id: SceneNodeId::new(10),
            voxel_parent_id: parent,
            voxel_child_order: 2,
            voxel_label: Some("Generated tunnel".to_string()),
            voxel_transform: SceneTransform::at(Vec3::new(-3.5, -1.0, -5.5)),
            marker_targets: vec![
                EnvironmentMarkerTarget {
                    source_marker_id: "player_start".to_string(),
                    node_id: SceneNodeId::new(11),
                    marker_id: "spawn.player".to_string(),
                    child_order: 0,
                },
                EnvironmentMarkerTarget {
                    source_marker_id: "exit_hint".to_string(),
                    node_id: SceneNodeId::new(12),
                    marker_id: "navigation.exit".to_string(),
                    child_order: 1,
                },
            ],
        },
        material_palette: [
            (1, "material/tunnel-wall", "Wall"),
            (2, "material/tunnel-floor", "Floor"),
            (3, "material/tunnel-accent", "Accent"),
        ]
        .into_iter()
        .map(
            |(material_slot, material_asset_id, display_name)| VoxelAssetMaterialBinding {
                material_slot,
                material_asset_id: material_asset_id.to_string(),
                display_name: Some(display_name.to_string()),
            },
        )
        .collect(),
        limits: EnvironmentLimits::default(),
    }
}
