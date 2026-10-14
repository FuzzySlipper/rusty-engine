use std::collections::{BTreeMap, BTreeSet};

use authored_scene::{
    composed_world_transforms, decode_scene, encode_scene, validate_scene, AvailableSceneAsset,
    FlatSceneDocument, NodeMetadata, Quat, SceneAdmissionError, SceneAdmissionPlan,
    SceneBootstrapBindings, SceneCatalogBinding, SceneEditCommand, SceneEditError,
    SceneEditService, SceneEntityInstance, SceneEntityReference, SceneGeneratorBinding, SceneLight,
    SceneLightInvalid, SceneLightShadowIntent, SceneMarker, SceneMetadata, SceneNode,
    SceneNodeKind, SceneNodeRecord, SceneObjectSnapshot, SceneReferenceError,
    SceneResolutionContext, SceneTransform, SceneTree, SceneValidationError, TransformInvalid,
};
use core_assets::{AssetHash, AssetId, AssetReference, AssetVersionReq};
use core_ids::{EntityId, PrefabId, SceneId, SceneNodeId};
use core_math::Vec3;

#[test]
fn tree_flat_roundtrip_and_world_transforms_are_deterministic() {
    let child = SceneNode {
        id: SceneNodeId::new(2),
        transform: translated(2.0, 0.0, 0.0),
        renderable_transform: SceneTransform::IDENTITY,
        kind: SceneNodeKind::StaticMesh(mesh_reference()),
        metadata: NodeMetadata {
            label: Some("Child".into()),
            tags: vec!["visible".into()],
        },
        children: Vec::new(),
    };
    let root = SceneNode {
        id: SceneNodeId::new(1),
        transform: translated(10.0, 0.0, 0.0),
        renderable_transform: SceneTransform::IDENTITY,
        kind: SceneNodeKind::EmptyGroup,
        metadata: NodeMetadata::default(),
        children: vec![child],
    };
    let tree = SceneTree {
        id: SceneId::new(7),
        revision: 3,
        schema_version: 4,
        metadata: SceneMetadata {
            name: Some("Room".into()),
            authoring_format_version: 4,
        },
        dependencies: vec![mesh_reference()],
        roots: vec![root],
    };

    let flat = tree.to_flat();
    assert_eq!(
        flat.nodes
            .iter()
            .map(|node| node.id.raw())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(flat.to_tree().unwrap(), tree);
    assert_eq!(
        composed_world_transforms(&flat)[&SceneNodeId::new(2)].translation,
        Vec3::new(12.0, 0.0, 0.0)
    );
}

#[test]
fn strict_codec_is_canonical_and_rejects_nested_unknowns_and_trailing_input() {
    let document = complete_document();
    let encoded = encode_scene(&document).unwrap();
    assert!(encoded.ends_with('\n'));
    assert_eq!(
        encode_scene(&decode_scene(&encoded).unwrap()).unwrap(),
        encoded
    );

    let nested_unknown = encoded.replacen(
        "\"translation\": [",
        "\"unexpected\": true,\n        \"translation\": [",
        1,
    );
    let error = decode_scene(&nested_unknown).unwrap_err();
    assert!(error.path.contains("nodes[0].transform"), "{error:?}");

    let trailing = format!("{encoded}{{}}");
    assert!(decode_scene(&trailing)
        .unwrap_err()
        .message
        .contains("trailing"));

    let wrong_kind = encoded.replacen("mesh/room", "sprite/room", 2);
    assert!(decode_scene(&wrong_kind)
        .unwrap_err()
        .message
        .contains("asset-kind-mismatch"));
}

#[test]
fn renderable_transform_is_canonical_and_legacy_scenes_default_to_identity() {
    let mut document = simple_document();
    document.schema_version = 5;
    document.metadata.authoring_format_version = 5;
    document.nodes[1].renderable_transform = translated(0.0, -1.5, 0.0);

    let encoded = encode_scene(&document).unwrap();
    assert!(encoded.contains("renderableTransform"));
    let restored = decode_scene(&encoded).unwrap();
    assert_eq!(restored, document.canonical());

    let legacy = encode_scene(&simple_document()).unwrap();
    assert!(!legacy.contains("renderableTransform"));
    let restored_legacy = decode_scene(&legacy).unwrap();
    assert_eq!(
        restored_legacy.nodes[1].renderable_transform,
        SceneTransform::IDENTITY
    );
}

#[test]
fn renderable_transform_requires_an_asset_owner() {
    let mut non_asset = FlatSceneDocument::new(SceneId::new(5));
    non_asset.nodes.push(SceneNodeRecord {
        renderable_transform: translated(0.0, -1.0, 0.0),
        ..record(1, None, SceneNodeKind::EmptyGroup)
    });
    assert!(validate_scene(&non_asset).errors.contains(
        &SceneValidationError::RenderableTransformWithoutAsset {
            node: SceneNodeId::new(1),
        }
    ));
}

#[test]
fn validation_classifies_hierarchy_transform_light_and_voxel_failures() {
    let mut document = FlatSceneDocument::new(SceneId::new(1));
    document.nodes = vec![
        record(1, Some(2), SceneNodeKind::EmptyGroup),
        record(2, Some(1), SceneNodeKind::EmptyGroup),
        record(3, Some(99), SceneNodeKind::EmptyGroup),
        record(4, None, SceneNodeKind::EmptyGroup),
        record(4, None, SceneNodeKind::EmptyGroup),
    ];
    document.nodes[0].transform.rotation = Quat::new(0.0, 0.0, 0.0, 0.0);
    let report = validate_scene(&document);
    assert_has_error(&report.errors, "duplicate-node-id");
    assert_has_error(&report.errors, "unknown-parent");
    assert_has_error(&report.errors, "cycle");
    assert!(report
        .errors
        .contains(&SceneValidationError::InvalidTransform {
            node: SceneNodeId::new(1),
            reason: TransformInvalid::NonUnitRotation,
        }));

    let mut light_document = FlatSceneDocument::new(SceneId::new(2));
    light_document.nodes.push(record(
        1,
        None,
        SceneNodeKind::Light(SceneLight::Spot {
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            enabled: true,
            range: Some(-1.0),
            decay: 2.0,
            outer_angle_radians: 0.5,
            penumbra: 0.2,
            shadow_intent: SceneLightShadowIntent::Requested,
        }),
    ));
    assert!(validate_scene(&light_document)
        .errors
        .contains(&SceneValidationError::InvalidLight {
            node: SceneNodeId::new(1),
            reason: SceneLightInvalid::InvalidRange,
        }));
    // An HDR colour and a wide cone admit; the render model's light rules
    // apply where a product publishes the light.
    light_document.nodes[0].kind = SceneNodeKind::Light(SceneLight::Spot {
        color: [4.0, 2.0, 1.0],
        intensity: 1.0,
        enabled: true,
        range: Some(10.0),
        decay: 2.0,
        outer_angle_radians: 2.0,
        penumbra: 1.5,
        shadow_intent: SceneLightShadowIntent::Requested,
    });
    assert!(validate_scene(&light_document).errors.is_empty());

    let voxel = voxel_reference();
    let mut voxel_document = FlatSceneDocument::new(SceneId::new(3));
    voxel_document.dependencies.push(voxel.clone());
    voxel_document
        .nodes
        .push(record(1, None, SceneNodeKind::VoxelVolume(voxel)));
    voxel_document.nodes[0].transform.scale = Vec3::new(2.0, 2.0, 2.0);
    assert_has_error(
        &validate_scene(&voxel_document).errors,
        "invalid-voxel-volume-transform",
    );
}

#[test]
fn typed_edits_use_revisions_and_never_partially_mutate_on_failure() {
    let mut document = simple_document();
    let original = document.clone();
    assert!(matches!(
        SceneEditService.apply(
            &mut document,
            99,
            SceneEditCommand::Rename {
                id: SceneNodeId::new(1),
                label: Some("Changed".into()),
            }
        ),
        Err(SceneEditError::StaleRevision { .. })
    ));
    assert_eq!(document, original);

    let cycle_source = document.clone();
    assert!(matches!(
        SceneEditService.apply(
            &mut document,
            0,
            SceneEditCommand::Reparent {
                id: SceneNodeId::new(1),
                parent: Some(SceneNodeId::new(2)),
                child_order: 0,
            }
        ),
        Err(SceneEditError::InvalidAfter { .. })
    ));
    assert_eq!(document, cycle_source);

    let receipt = SceneEditService
        .apply(
            &mut document,
            0,
            SceneEditCommand::Rename {
                id: SceneNodeId::new(2),
                label: Some("Mesh".into()),
            },
        )
        .unwrap();
    assert_eq!((receipt.revision_before, receipt.revision_after), (0, 1));
    assert_eq!(document.nodes[1].metadata.label.as_deref(), Some("Mesh"));

    let selection = SceneEditService
        .apply(
            &mut document,
            1,
            SceneEditCommand::Select {
                id: Some(SceneNodeId::new(2)),
            },
        )
        .unwrap();
    assert_eq!(selection.revision_after, 1);

    let created = record(
        3,
        Some(1),
        SceneNodeKind::Light(SceneLight::Ambient {
            color: [0.2, 0.3, 0.4],
            intensity: 0.5,
            enabled: true,
            shadow_intent: SceneLightShadowIntent::Disabled,
        }),
    );
    SceneEditService
        .apply(
            &mut document,
            1,
            SceneEditCommand::Create { record: created },
        )
        .unwrap();
    assert!(document
        .nodes
        .iter()
        .any(|node| node.id == SceneNodeId::new(3)));

    SceneEditService
        .apply(
            &mut document,
            2,
            SceneEditCommand::Delete {
                id: SceneNodeId::new(1),
            },
        )
        .unwrap();
    assert!(document.nodes.is_empty());
    assert!(document.dependencies.is_empty());
}

#[test]
fn renderable_transform_edit_is_revision_guarded_and_asset_scoped() {
    let mut document = simple_document();
    let offset = translated(0.0, -2.0, 0.0);
    let receipt = SceneEditService
        .apply(
            &mut document,
            0,
            SceneEditCommand::SetRenderableTransform {
                id: SceneNodeId::new(2),
                transform: offset,
            },
        )
        .unwrap();
    assert_eq!((receipt.revision_before, receipt.revision_after), (0, 1));
    assert_eq!(document.schema_version, 4);
    assert_eq!(document.nodes[1].renderable_transform, offset);

    let stale_source = document.clone();
    assert!(matches!(
        SceneEditService.apply(
            &mut document,
            0,
            SceneEditCommand::SetRenderableTransform {
                id: SceneNodeId::new(2),
                transform: SceneTransform::IDENTITY,
            },
        ),
        Err(SceneEditError::StaleRevision { .. })
    ));
    assert_eq!(document, stale_source);

    assert!(matches!(
        SceneEditService.apply(
            &mut document,
            1,
            SceneEditCommand::SetRenderableTransform {
                id: SceneNodeId::new(1),
                transform: offset,
            },
        ),
        Err(SceneEditError::WrongObjectKind { .. })
    ));
    assert_eq!(document, stale_source);
}

#[test]
fn specialized_edits_validate_kinds_and_reconcile_voxel_dependencies() {
    let first_voxel = voxel_reference();
    let second_voxel = AssetReference::new(
        AssetId::parse("voxel-volume/atrium").unwrap(),
        AssetVersionReq::AtLeast(3),
        None,
    );
    let mut document = FlatSceneDocument::new(SceneId::new(8));
    document.dependencies.push(first_voxel.clone());
    document
        .nodes
        .push(record(1, None, SceneNodeKind::VoxelVolume(first_voxel)));

    SceneEditService
        .apply(
            &mut document,
            0,
            SceneEditCommand::SetTransform {
                id: SceneNodeId::new(1),
                transform: translated(3.0, 0.0, 0.0),
            },
        )
        .unwrap();
    SceneEditService
        .apply(
            &mut document,
            1,
            SceneEditCommand::RetargetVoxelAsset {
                id: SceneNodeId::new(1),
                asset: second_voxel.clone(),
                tags: vec!["generated".into()],
            },
        )
        .unwrap();
    assert_eq!(document.dependencies, [second_voxel]);
    assert_eq!(document.nodes[0].metadata.tags, ["generated"]);

    SceneEditService
        .apply(
            &mut document,
            2,
            SceneEditCommand::SetKind {
                id: SceneNodeId::new(1),
                kind: SceneNodeKind::EmptyGroup,
            },
        )
        .unwrap();
    assert!(document.dependencies.is_empty());
    assert!(matches!(document.nodes[0].kind, SceneNodeKind::EmptyGroup));

    let before_wrong_kind = document.clone();
    assert!(matches!(
        SceneEditService.apply(
            &mut document,
            3,
            SceneEditCommand::UpdateLight {
                id: SceneNodeId::new(1),
                light: SceneLight::Ambient {
                    color: [1.0, 1.0, 1.0],
                    intensity: 1.0,
                    enabled: true,
                    shadow_intent: SceneLightShadowIntent::Disabled,
                },
            }
        ),
        Err(SceneEditError::WrongObjectKind { .. })
    ));
    assert_eq!(document, before_wrong_kind);

    let mut light_document = FlatSceneDocument::new(SceneId::new(9));
    light_document.nodes.push(record(
        1,
        None,
        SceneNodeKind::Light(SceneLight::Ambient {
            color: [0.0, 0.0, 0.0],
            intensity: 0.0,
            enabled: false,
            shadow_intent: SceneLightShadowIntent::Disabled,
        }),
    ));
    SceneEditService
        .apply(
            &mut light_document,
            0,
            SceneEditCommand::UpdateLight {
                id: SceneNodeId::new(1),
                light: SceneLight::Point {
                    color: [0.4, 0.5, 0.6],
                    intensity: 4.0,
                    enabled: true,
                    range: Some(12.0),
                    decay: 2.0,
                    shadow_intent: SceneLightShadowIntent::Requested,
                },
            },
        )
        .unwrap();
    assert!(matches!(
        light_document.nodes[0].kind,
        SceneNodeKind::Light(SceneLight::Point { .. })
    ));
}

#[test]
fn selection_does_not_canonicalize_or_mutate_the_authored_document() {
    let mut document = simple_document();
    document.nodes.reverse();
    let before = document.clone();
    let receipt = SceneEditService
        .apply(
            &mut document,
            0,
            SceneEditCommand::Select {
                id: Some(SceneNodeId::new(2)),
            },
        )
        .unwrap();
    assert_eq!(document, before);
    assert_eq!(receipt.revision_after, 0);
    assert_eq!(receipt.selected, Some(SceneNodeId::new(2)));
    assert_eq!(
        SceneObjectSnapshot::from_document(&document).objects[0].id,
        SceneNodeId::new(1)
    );
}

#[test]
fn admission_resolves_references_and_retains_plan_facts() {
    let mut document = complete_document();
    document.schema_version = 5;
    document.metadata.authoring_format_version = 5;
    document.nodes[1].renderable_transform = translated(0.0, -1.5, 0.0);
    let empty_context = SceneResolutionContext::default();
    let error = SceneAdmissionPlan::prepare(&mut document, &empty_context).unwrap_err();
    let SceneAdmissionError::UnresolvedReferences { errors } = error else {
        panic!("expected reference rejection")
    };
    assert!(errors
        .iter()
        .any(|error| matches!(error, SceneReferenceError::UnknownAsset { .. })));
    assert!(errors
        .iter()
        .any(|error| matches!(error, SceneReferenceError::UnknownEntityDefinition { .. })));
    assert!(errors
        .iter()
        .any(|error| matches!(error, SceneReferenceError::UnknownGeneratorPreset { .. })));
    assert!(errors
        .iter()
        .any(|error| matches!(error, SceneReferenceError::UnknownCatalog { .. })));

    let context = complete_context();
    let plan =
        SceneAdmissionPlan::prepare_with_base(&mut document, EntityId::new(100), &context).unwrap();
    assert_eq!(
        plan.allocations()
            .iter()
            .map(|allocation| (allocation.node.raw(), allocation.entity.raw()))
            .collect::<Vec<_>>(),
        [
            (1, 100),
            (2, 101),
            (3, 102),
            (4, 103),
            (5, 104),
            (6, 105),
            (7, 106)
        ]
    );
    assert_eq!(plan.lights().len(), 1);
    assert_eq!(
        plan.resolved_instances()[0].world_transform.translation.x,
        14.0
    );
    assert!(plan.resolved_instances()[0].spawn_marker_id.is_some());

    assert_eq!(
        plan.renderables()[0].world_transform.translation,
        Vec3::new(12.0, 0.0, 0.0)
    );
    assert_eq!(
        plan.renderables()[0].renderable_local_transform,
        translated(0.0, -1.5, 0.0)
    );
}

#[test]
fn asset_version_and_hash_pins_are_checked_before_admission() {
    let mut document = simple_document();
    let mut context = complete_context();
    context.available_assets.insert(
        AssetId::parse("mesh/room").unwrap(),
        AvailableSceneAsset {
            version: 1,
            hash: Some(AssetHash::parse("bb22").unwrap()),
        },
    );
    let SceneAdmissionError::UnresolvedReferences { errors } =
        SceneAdmissionPlan::prepare(&mut document, &context).unwrap_err()
    else {
        panic!("expected reference rejection")
    };
    assert!(errors
        .iter()
        .any(|error| matches!(error, SceneReferenceError::AssetVersionMismatch { .. })));
    assert!(errors
        .iter()
        .any(|error| matches!(error, SceneReferenceError::AssetHashMismatch { .. })));
}

#[test]
fn node_asset_pins_must_match_the_declared_dependency_exactly() {
    let mut document = simple_document();
    let mismatched = AssetReference::new(
        AssetId::parse("mesh/room").unwrap(),
        AssetVersionReq::Exact(99),
        Some(AssetHash::parse("bb22").unwrap()),
    );
    document.nodes[1].kind = SceneNodeKind::StaticMesh(mismatched);

    let before = document.clone();
    let error = SceneAdmissionPlan::prepare(&mut document, &complete_context()).unwrap_err();
    assert!(matches!(error, SceneAdmissionError::InvalidScene(_)));
    assert!(validate_scene(&document)
        .errors
        .iter()
        .any(|error| matches!(error, SceneValidationError::MissingAssetDependency { .. })));
    assert_eq!(document, before);
}

#[test]
fn repeated_tags_and_dependencies_are_canonicalized_not_refused() {
    let mut document = simple_document();
    document.nodes[1].metadata.tags = vec!["b".into(), "a".into(), "b".into()];
    document.dependencies.push(mesh_reference());
    assert!(validate_scene(&document).is_valid());
    let decoded = decode_scene(&encode_scene(&document).unwrap()).unwrap();
    assert_eq!(decoded.nodes[1].metadata.tags, ["a", "b"]);
    assert_eq!(decoded.dependencies, [mesh_reference()]);

    SceneEditService
        .apply(
            &mut document,
            0,
            SceneEditCommand::Create {
                record: SceneNodeRecord {
                    metadata: NodeMetadata {
                        label: None,
                        tags: vec!["x".into(), "x".into()],
                    },
                    ..record(3, None, SceneNodeKind::EmptyGroup)
                },
            },
        )
        .unwrap();
    assert_eq!(document.nodes[2].metadata.tags, ["x"]);
}

#[test]
fn older_schema_documents_hold_every_node_kind() {
    let mut document = complete_document();
    document.schema_version = 1;
    document.metadata.authoring_format_version = 1;
    document.nodes[1].renderable_transform = translated(0.0, -1.5, 0.0);
    assert!(validate_scene(&document).is_valid());
    let decoded = decode_scene(&encode_scene(&document).unwrap()).unwrap();
    assert_eq!(decoded.schema_version, 1);
    assert_eq!(decoded, document.canonical());
}

#[test]
fn edits_refuse_what_they_would_break_elsewhere_and_leave_the_document_unchanged() {
    let turned = SceneTransform {
        rotation: Quat::new(0.0, 0.5_f32.sin(), 0.0, 0.5_f32.cos()),
        ..SceneTransform::IDENTITY
    };
    let mut document = complete_document();
    document.dependencies.push(voxel_reference());
    document.nodes.extend([
        record(8, Some(1), SceneNodeKind::EmptyGroup),
        record(9, Some(8), SceneNodeKind::VoxelVolume(voxel_reference())),
        SceneNodeRecord {
            transform: turned,
            ..record(10, None, SceneNodeKind::EmptyGroup)
        },
    ]);
    assert!(validate_scene(&document).is_valid());
    let refused = |document: &mut FlatSceneDocument, command, code: &str| {
        let before = document.clone();
        let Err(SceneEditError::InvalidAfter { errors }) =
            SceneEditService.apply(document, 0, command)
        else {
            panic!("expected {code} refusal");
        };
        assert_has_error(&errors, code);
        assert_eq!(*document, before);
    };
    // Removing or renaming the marker an instance spawns at.
    refused(
        &mut document,
        SceneEditCommand::Delete {
            id: SceneNodeId::new(3),
        },
        "unknown-spawn-marker",
    );
    refused(
        &mut document,
        SceneEditCommand::SetKind {
            id: SceneNodeId::new(3),
            kind: SceneNodeKind::Marker(SceneMarker {
                marker_id: "spawn.other".into(),
            }),
        },
        "unknown-spawn-marker",
    );
    // Identities other nodes already hold.
    refused(
        &mut document,
        SceneEditCommand::Create {
            record: record(
                11,
                None,
                SceneNodeKind::Marker(SceneMarker {
                    marker_id: "spawn.player".into(),
                }),
            ),
        },
        "duplicate-marker-id",
    );
    refused(
        &mut document,
        SceneEditCommand::SetKind {
            id: SceneNodeId::new(8),
            kind: SceneNodeKind::EntityInstance(SceneEntityInstance {
                instance_id: "enemy.one".into(),
                reference: SceneEntityReference::EntityDefinition {
                    stable_id: "player.definition".into(),
                },
                spawn_marker_id: None,
            }),
        },
        "duplicate-entity-instance-id",
    );
    refused(
        &mut document,
        SceneEditCommand::Create {
            record: record(
                11,
                None,
                SceneNodeKind::Bootstrap(SceneBootstrapBindings::default()),
            ),
        },
        "duplicate-bootstrap-node",
    );
    refused(
        &mut document,
        SceneEditCommand::Create {
            record: record(
                11,
                None,
                SceneNodeKind::EntityInstance(SceneEntityInstance {
                    instance_id: "late.one".into(),
                    reference: SceneEntityReference::EntityDefinition {
                        stable_id: "player.definition".into(),
                    },
                    spawn_marker_id: Some("spawn.absent".into()),
                }),
            ),
        },
        "unknown-spawn-marker",
    );
    // Hierarchy: a parent cycle, and a bootstrap moved off the root.
    refused(
        &mut document,
        SceneEditCommand::Reparent {
            id: SceneNodeId::new(1),
            parent: Some(SceneNodeId::new(9)),
            child_order: 0,
        },
        "cycle",
    );
    refused(
        &mut document,
        SceneEditCommand::Reparent {
            id: SceneNodeId::new(6),
            parent: Some(SceneNodeId::new(1)),
            child_order: 0,
        },
        "invalid-bootstrap",
    );
    // A voxel volume's composed placement, changed from above it.
    refused(
        &mut document,
        SceneEditCommand::SetTransform {
            id: SceneNodeId::new(1),
            transform: turned,
        },
        "invalid-voxel-volume-transform",
    );
    refused(
        &mut document,
        SceneEditCommand::Reparent {
            id: SceneNodeId::new(8),
            parent: Some(SceneNodeId::new(10)),
            child_order: 0,
        },
        "invalid-voxel-volume-transform",
    );
    // Node fields: a light's scale and a second pin of one asset.
    refused(
        &mut document,
        SceneEditCommand::SetTransform {
            id: SceneNodeId::new(5),
            transform: SceneTransform {
                scale: Vec3::new(2.0, 2.0, 2.0),
                ..SceneTransform::IDENTITY
            },
        },
        "invalid-light",
    );
    refused(
        &mut document,
        SceneEditCommand::Create {
            record: record(
                11,
                None,
                SceneNodeKind::StaticMesh(AssetReference::new(
                    AssetId::parse("mesh/room").unwrap(),
                    AssetVersionReq::Exact(3),
                    None,
                )),
            ),
        },
        "missing-asset-dependency",
    );
    assert!(validate_scene(&document).is_valid());
}

#[test]
fn edits_keep_a_larger_scene_valid_and_touch_one_node() {
    let mut document = FlatSceneDocument::new(SceneId::new(11));
    document.dependencies.push(mesh_reference());
    for id in 1..=2_000_u64 {
        let parent = (id > 1).then(|| (id - 1) / 4 + 1);
        let kind = if id % 3 == 0 {
            SceneNodeKind::StaticMesh(mesh_reference())
        } else {
            SceneNodeKind::EmptyGroup
        };
        document.nodes.push(record(id, parent, kind));
    }
    assert!(validate_scene(&document).is_valid());
    let untouched = document.nodes[..1_499].to_vec();

    let commands = [
        SceneEditCommand::Rename {
            id: SceneNodeId::new(1_500),
            label: Some("Lamp".into()),
        },
        SceneEditCommand::SetTransform {
            id: SceneNodeId::new(1_500),
            transform: translated(1.0, 2.0, 3.0),
        },
        SceneEditCommand::Reparent {
            id: SceneNodeId::new(1_500),
            parent: Some(SceneNodeId::new(2)),
            child_order: 9,
        },
        SceneEditCommand::SetKind {
            id: SceneNodeId::new(1_501),
            kind: SceneNodeKind::Marker(SceneMarker {
                marker_id: "spawn.large".into(),
            }),
        },
        SceneEditCommand::Create {
            record: record(
                2_001,
                Some(SceneNodeId::new(1_500).raw()),
                SceneNodeKind::EmptyGroup,
            ),
        },
        SceneEditCommand::Delete {
            id: SceneNodeId::new(1_502),
        },
    ];
    for (revision, command) in commands.into_iter().enumerate() {
        let receipt = SceneEditService
            .apply(&mut document, revision as u64, command)
            .unwrap();
        assert_eq!(receipt.revision_after, revision as u64 + 1);
        assert!(validate_scene(&document).is_valid());
    }
    assert_eq!(document.nodes[..1_499], untouched[..]);
    assert_eq!(document.dependencies, [mesh_reference()]);
    let node = &document.nodes[1_499];
    assert_eq!(
        (node.id.raw(), node.parent, node.metadata.label.as_deref()),
        (1_500, Some(SceneNodeId::new(2)), Some("Lamp"))
    );
    assert_eq!(document.nodes.last().unwrap().id.raw(), 2_001);
    assert!(!document
        .nodes
        .iter()
        .any(|node| node.id.raw() == 1_502 || node.parent == Some(SceneNodeId::new(1_502))));
}

fn simple_document() -> FlatSceneDocument {
    let mesh = mesh_reference();
    FlatSceneDocument {
        id: SceneId::new(42),
        revision: 0,
        schema_version: 4,
        metadata: SceneMetadata {
            name: Some("Test".into()),
            authoring_format_version: 4,
        },
        dependencies: vec![mesh.clone()],
        nodes: vec![
            SceneNodeRecord {
                transform: translated(10.0, 0.0, 0.0),
                ..record(1, None, SceneNodeKind::EmptyGroup)
            },
            SceneNodeRecord {
                transform: translated(2.0, 0.0, 0.0),
                ..record(2, Some(1), SceneNodeKind::StaticMesh(mesh))
            },
        ],
    }
}

fn complete_document() -> FlatSceneDocument {
    let mut document = simple_document();
    document.nodes.extend([
        SceneNodeRecord {
            transform: translated(3.0, 0.0, 0.0),
            ..record(
                3,
                Some(1),
                SceneNodeKind::Marker(SceneMarker {
                    marker_id: "spawn.player".into(),
                }),
            )
        },
        SceneNodeRecord {
            transform: translated(1.0, 0.0, 0.0),
            ..record(
                4,
                Some(1),
                SceneNodeKind::EntityInstance(SceneEntityInstance {
                    instance_id: "player.one".into(),
                    reference: SceneEntityReference::EntityDefinition {
                        stable_id: "player.definition".into(),
                    },
                    spawn_marker_id: Some("spawn.player".into()),
                }),
            )
        },
        record(
            5,
            None,
            SceneNodeKind::Light(SceneLight::Directional {
                color: [1.0, 0.9, 0.8],
                intensity: 2.0,
                enabled: true,
                shadow_intent: SceneLightShadowIntent::Requested,
            }),
        ),
        record(
            6,
            None,
            SceneNodeKind::Bootstrap(SceneBootstrapBindings {
                generator: Some(SceneGeneratorBinding {
                    provider_id: "terrain".into(),
                    preset_id: "caves".into(),
                    seed: 123,
                }),
                catalogs: vec![SceneCatalogBinding {
                    binding_id: "materials".into(),
                    catalog_id: "main.catalog".into(),
                    source_path: "catalogs/main.json".into(),
                }],
            }),
        ),
        SceneNodeRecord {
            transform: translated(20.0, 0.0, 0.0),
            ..record(
                7,
                None,
                SceneNodeKind::EntityInstance(SceneEntityInstance {
                    instance_id: "enemy.one".into(),
                    reference: SceneEntityReference::Prefab {
                        prefab_id: PrefabId::new(9),
                        variant_id: Some("elite".into()),
                        instantiation_seed: 456,
                    },
                    spawn_marker_id: None,
                }),
            )
        },
    ]);
    document
}

fn complete_context() -> SceneResolutionContext {
    SceneResolutionContext {
        available_assets: BTreeMap::from([(
            AssetId::parse("mesh/room").unwrap(),
            AvailableSceneAsset {
                version: 2,
                hash: Some(AssetHash::parse("aa11").unwrap()),
            },
        )]),
        entity_definition_ids: BTreeSet::from(["player.definition".into()]),
        prefab_ids: BTreeSet::from([PrefabId::new(9)]),
        generator_presets: BTreeSet::from([("terrain".into(), "caves".into())]),
        catalog_ids: BTreeSet::from(["main.catalog".into()]),
    }
}

fn record(id: u64, parent: Option<u64>, kind: SceneNodeKind) -> SceneNodeRecord {
    SceneNodeRecord {
        id: SceneNodeId::new(id),
        parent: parent.map(SceneNodeId::new),
        child_order: 0,
        transform: SceneTransform::IDENTITY,
        renderable_transform: SceneTransform::IDENTITY,
        kind,
        metadata: NodeMetadata::default(),
    }
}

fn translated(x: f32, y: f32, z: f32) -> SceneTransform {
    SceneTransform::at(Vec3::new(x, y, z))
}

fn mesh_reference() -> AssetReference {
    AssetReference::new(
        AssetId::parse("mesh/room").unwrap(),
        AssetVersionReq::Exact(2),
        Some(AssetHash::parse("aa11").unwrap()),
    )
}

fn voxel_reference() -> AssetReference {
    AssetReference::new(
        AssetId::parse("voxel-volume/chamber").unwrap(),
        AssetVersionReq::Any,
        None,
    )
}

fn assert_has_error(errors: &[SceneValidationError], code: &str) {
    assert!(
        errors.iter().any(|error| error.code() == code),
        "{errors:#?}"
    );
}
