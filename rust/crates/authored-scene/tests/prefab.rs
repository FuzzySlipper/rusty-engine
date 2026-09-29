use std::collections::BTreeSet;

use authored_scene::*;
use core_ids::{PrefabId, PrefabPartId};

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

    let unknown = resolve_prefab(
        &registry,
        PrefabId::new(1),
        &[PrefabOverride {
            target_role: "visual".to_owned(),
            value: PrefabOverrideValue::Material {
                asset: "material/not-in-catalog".to_owned(),
            },
        }],
    )
    .unwrap_err();
    assert_eq!(
        unknown,
        PrefabResolutionError::UnknownOverrideAsset("material/not-in-catalog".to_owned())
    );
}
