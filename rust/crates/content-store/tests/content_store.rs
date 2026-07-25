use std::collections::BTreeSet;

use content_store::*;
use core_ids::{PrefabId, PrefabPartId};

fn manifest(path: &str, bytes: &[u8]) -> ContentManifest {
    ContentManifest::new(vec![ContentArtifact::durable(
        path,
        ArtifactRole::Resource("resource:test".to_owned()),
        bytes,
    )])
}

#[test]
fn canonical_manifest_and_bounded_batch_admit_exact_bytes() {
    let bytes = b"durable-content";
    let manifest = manifest("content/test.bin", bytes);
    let json = encode_manifest(&manifest).unwrap();
    let admitted = admit_source_batch(ContentSourceBatch {
        manifest_json: json.clone(),
        bodies: vec![ContentBody::new("content/test.bin", bytes)],
    })
    .unwrap();
    assert_eq!(admitted.body("content/test.bin"), Some(bytes.as_slice()));
    assert_eq!(
        encode_manifest(&decode_manifest(&json).unwrap()).unwrap(),
        json
    );

    let error = admit_source_batch(ContentSourceBatch {
        manifest_json: json,
        bodies: vec![ContentBody::new("content/test.bin", b"tampered")],
    })
    .unwrap_err();
    assert!(matches!(
        error.code,
        ContentSourceErrorCode::LengthMismatch | ContentSourceErrorCode::HashMismatch
    ));
}

#[test]
fn strict_manifest_rejects_unknown_fields_and_unsafe_paths() {
    let unknown = r#"{"schemaVersion":1,"artifacts":[],"runtimeSession":{}}"#;
    assert!(decode_manifest(unknown).is_err());
    let bad = ContentManifest::new(vec![ContentArtifact::durable(
        "../escape",
        ArtifactRole::ImportedAsset,
        b"x",
    )]);
    assert!(matches!(
        bad.validate(),
        Err(ManifestError::InvalidPath { .. })
    ));
}

#[test]
fn write_candidates_enforce_exact_transition_and_two_sided_cas() {
    let prior = manifest("old.bin", b"old");
    let next = ContentManifest::new(vec![
        ContentArtifact::durable(
            "moved.bin",
            ArtifactRole::Resource("resource:test".to_owned()),
            b"old",
        ),
        ContentArtifact::generated("new.bin", ArtifactRole::GeneratedMetadata, b"new"),
    ]);
    let candidate = ContentWriteCandidate::build(
        7,
        &prior,
        ContentWriteSetDraft {
            next_manifest: next.clone(),
            writes: vec![ContentWrite::new("new.bin", b"new")],
            moves: vec![ContentMove {
                from: "old.bin".to_owned(),
                to: "moved.bin".to_owned(),
                expected_content_hash: Some(ContentHash::of(b"old")),
            }],
            deletes: vec![],
        },
    )
    .unwrap();
    let save_plan = ContentSavePlan::from_candidate(&candidate);
    assert_eq!(save_plan.manifest_path, CONTENT_MANIFEST_PATH);
    assert_eq!(save_plan.writes.len(), 1);
    assert_eq!(save_plan.moves.len(), 1);
    let stale = ContentStoreIdentity::from_manifest(6, &prior).unwrap();
    assert_eq!(
        candidate.clone().authorize(&stale).unwrap_err(),
        ContentWriteSetError::StaleStore
    );
    let prior_identity = ContentStoreIdentity::from_manifest(7, &prior).unwrap();
    let authorized = candidate.authorize(&prior_identity).unwrap();
    let wrong_next = ContentStoreIdentity::from_manifest(8, &prior).unwrap();
    assert_eq!(
        authorized.clone().confirm(&wrong_next).unwrap_err(),
        ContentWriteSetError::PublicationMismatch
    );
    let receipt = authorized
        .confirm(&ContentStoreIdentity::from_manifest(8, &next).unwrap())
        .unwrap();
    assert_eq!(receipt.identity.revision, 8);
}

#[test]
fn load_plan_orders_owner_admission_before_resources() {
    let manifest = ContentManifest::new(vec![
        ContentArtifact::durable("z.resource", ArtifactRole::ImportedAsset, b"resource"),
        ContentArtifact::durable("scene.json", ArtifactRole::SceneDocument, b"scene"),
        ContentArtifact::durable("prefabs.json", ArtifactRole::PrefabRegistry, b"prefabs"),
        ContentArtifact::durable("catalog.json", ArtifactRole::AssetCatalog, b"catalog"),
    ]);
    let plan = ContentLoadPlan::build(&manifest).unwrap();
    assert!(plan.verify_order());
    assert_eq!(plan.steps[0].stage, ContentLoadStage::AssetAuthority);
    assert_eq!(plan.steps[1].stage, ContentLoadStage::Prefabs);
    assert_eq!(plan.steps[2].stage, ContentLoadStage::Scenes);
    assert_eq!(plan.steps[3].stage, ContentLoadStage::Resources);
}

#[test]
fn undeclared_transition_never_produces_a_mutation_candidate() {
    let prior = manifest("same.bin", b"old");
    let next = manifest("same.bin", b"new");
    let error = ContentWriteCandidate::build(
        1,
        &prior,
        ContentWriteSetDraft {
            next_manifest: next,
            writes: vec![],
            moves: vec![],
            deletes: vec![],
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        ContentWriteSetError::UnaccountedNextChange("same.bin".to_owned())
    );
}

fn prefab_context() -> PrefabRegistryValidationContext {
    PrefabRegistryValidationContext {
        asset_ids: [
            "voxel-object/machine-body".to_owned(),
            "material/steel".to_owned(),
        ]
        .into_iter()
        .collect(),
        entity_definition_ids: ["machine.controller".to_owned()].into_iter().collect(),
    }
}

fn base_prefab() -> PrefabDefinition {
    PrefabDefinition {
        id: PrefabId::new(1),
        schema_version: PREFAB_DEFINITION_SCHEMA_VERSION,
        display_name: "Machine".to_owned(),
        parts: vec![
            PrefabPart {
                id: PrefabPartId::new(1),
                namespace: "body".to_owned(),
                display_name: "Body".to_owned(),
                parent: None,
                transform: PrefabTransform::IDENTITY,
                source: PrefabPartSource::VoxelObject {
                    asset: "voxel-object/machine-body".to_owned(),
                },
            },
            PrefabPart {
                id: PrefabPartId::new(2),
                namespace: "controller".to_owned(),
                display_name: "Controller".to_owned(),
                parent: Some(PrefabPartId::new(1)),
                transform: PrefabTransform::IDENTITY,
                source: PrefabPartSource::EntityDefinition {
                    stable_id: "machine.controller".to_owned(),
                },
            },
        ],
        part_roles: vec![
            PrefabPartRoleBinding {
                role: "visual".to_owned(),
                part: PrefabPartId::new(1),
            },
            PrefabPartRoleBinding {
                role: "gameplay".to_owned(),
                part: PrefabPartId::new(2),
            },
        ],
        variant: None,
    }
}

#[test]
fn prefab_variant_codec_validation_and_resolution_preserve_typed_behavior() {
    let variant = PrefabDefinition {
        id: PrefabId::new(2),
        schema_version: PREFAB_DEFINITION_SCHEMA_VERSION,
        display_name: "Dormant steel machine".to_owned(),
        parts: vec![],
        part_roles: vec![],
        variant: Some(PrefabVariantDelta {
            variant_id: "dormant".to_owned(),
            base: PrefabId::new(1),
            removed_roles: vec![],
            overrides: vec![
                PrefabOverride {
                    target_role: "visual".to_owned(),
                    value: PrefabOverrideValue::Material {
                        asset: "material/steel".to_owned(),
                    },
                },
                PrefabOverride {
                    target_role: "visual".to_owned(),
                    value: PrefabOverrideValue::Activation { active: false },
                },
            ],
        }),
    };
    let validated = ValidatedPrefabRegistry::new(
        PrefabRegistry {
            schema_version: PREFAB_REGISTRY_SCHEMA_VERSION,
            definitions: vec![variant, base_prefab()],
        },
        &prefab_context(),
    )
    .unwrap();
    let encoded = encode_prefab_registry(&validated).unwrap();
    let decoded = decode_prefab_registry(&encoded, &prefab_context()).unwrap();
    assert_eq!(encoded, encode_prefab_registry(&decoded).unwrap());
    let resolved = resolve_prefab(&decoded, PrefabId::new(2), &[]).unwrap();
    let visual = resolved
        .parts
        .iter()
        .find(|part| part.roles.contains(&"visual".to_owned()))
        .unwrap();
    assert_eq!(visual.material.as_deref(), Some("material/steel"));
    assert!(!visual.active);
}

#[test]
fn prefab_cycles_and_unsafe_removals_are_classified() {
    let mut base = base_prefab();
    base.part_roles.push(PrefabPartRoleBinding {
        role: "gameplay-alias".to_owned(),
        part: PrefabPartId::new(2),
    });
    let variant = PrefabDefinition {
        id: PrefabId::new(2),
        schema_version: 1,
        display_name: "Broken".to_owned(),
        parts: vec![],
        part_roles: vec![],
        variant: Some(PrefabVariantDelta {
            variant_id: "broken".to_owned(),
            base: PrefabId::new(1),
            removed_roles: vec!["gameplay".to_owned()],
            overrides: vec![PrefabOverride {
                target_role: "gameplay-alias".to_owned(),
                value: PrefabOverrideValue::Activation { active: false },
            }],
        }),
    };
    let report = validate_prefab_registry(
        &PrefabRegistry {
            schema_version: 1,
            definitions: vec![base, variant],
        },
        &prefab_context(),
    );
    let codes: BTreeSet<_> = report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert!(codes.contains(&PrefabDiagnosticCode::UnsafePartRemoval));
    assert!(codes.contains(&PrefabDiagnosticCode::DeletedRoleReferenced));
}

#[test]
fn instance_overrides_fail_closed_before_resolved_composition_changes() {
    let registry = ValidatedPrefabRegistry::new(
        PrefabRegistry {
            schema_version: 1,
            definitions: vec![base_prefab()],
        },
        &prefab_context(),
    )
    .unwrap();
    let wrong_kind = resolve_prefab(
        &registry,
        PrefabId::new(1),
        &[PrefabOverride {
            target_role: "visual".to_owned(),
            value: PrefabOverrideValue::Asset {
                asset: "material/steel".to_owned(),
            },
        }],
    )
    .unwrap_err();
    assert!(matches!(
        wrong_kind,
        PrefabResolutionError::InvalidOverrideValue(_)
    ));
    let duplicate = resolve_prefab(
        &registry,
        PrefabId::new(1),
        &[
            PrefabOverride {
                target_role: "visual".to_owned(),
                value: PrefabOverrideValue::Activation { active: false },
            },
            PrefabOverride {
                target_role: "visual".to_owned(),
                value: PrefabOverrideValue::Activation { active: true },
            },
        ],
    )
    .unwrap_err();
    assert!(matches!(
        duplicate,
        PrefabResolutionError::DuplicateOverride { .. }
    ));
}
