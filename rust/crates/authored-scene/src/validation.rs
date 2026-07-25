use std::collections::{BTreeMap, BTreeSet};

use core_assets::AssetKind;
use core_ids::SceneNodeId;
use core_math::Vec3;

use crate::{
    FlatSceneDocument, SceneLightInvalid, SceneNodeKind, SceneTransform,
    CURRENT_SCENE_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformInvalid {
    NonFiniteTranslation,
    TranslationOutOfRange,
    NonFiniteRotation,
    NonUnitRotation,
    NonFiniteScale,
    NonPositiveScale,
}

impl TransformInvalid {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NonFiniteTranslation => "non-finite-translation",
            Self::TranslationOutOfRange => "translation-out-of-range",
            Self::NonFiniteRotation => "non-finite-rotation",
            Self::NonUnitRotation => "non-unit-rotation",
            Self::NonFiniteScale => "non-finite-scale",
            Self::NonPositiveScale => "non-positive-scale",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneValidationError {
    UnsupportedSchemaVersion {
        found: u32,
        supported: u32,
    },
    AuthoringVersionAheadOfSchema {
        authored: u32,
        schema: u32,
    },
    DuplicateNodeId {
        id: SceneNodeId,
    },
    UnknownParent {
        node: SceneNodeId,
        parent: SceneNodeId,
    },
    Cycle {
        path: Vec<SceneNodeId>,
    },
    InvalidTransform {
        node: SceneNodeId,
        reason: TransformInvalid,
    },
    BlankLabel {
        node: SceneNodeId,
    },
    BlankTag {
        node: SceneNodeId,
    },
    DuplicateTag {
        node: SceneNodeId,
        tag: String,
    },
    DuplicateAssetDependency {
        asset: String,
    },
    MissingAssetDependency {
        node: SceneNodeId,
        asset: String,
    },
    AssetKindMismatch {
        node: SceneNodeId,
        expected: AssetKind,
        actual: AssetKind,
    },
    InvalidVoxelVolumeTransform {
        node: SceneNodeId,
        reason: &'static str,
    },
    InvalidLight {
        node: SceneNodeId,
        reason: SceneLightInvalid,
    },
    InvalidMarker {
        node: SceneNodeId,
        reason: &'static str,
    },
    DuplicateMarkerId {
        node: SceneNodeId,
        marker_id: String,
    },
    InvalidEntityInstance {
        node: SceneNodeId,
        reason: &'static str,
    },
    DuplicateEntityInstanceId {
        node: SceneNodeId,
        instance_id: String,
    },
    UnknownSpawnMarker {
        node: SceneNodeId,
        marker_id: String,
    },
    DuplicateBootstrapNode {
        node: SceneNodeId,
    },
    InvalidBootstrap {
        node: SceneNodeId,
        reason: &'static str,
    },
    DuplicateCatalogBinding {
        node: SceneNodeId,
        binding_id: String,
    },
}

impl SceneValidationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedSchemaVersion { .. } => "unsupported-schema-version",
            Self::AuthoringVersionAheadOfSchema { .. } => "authoring-version-ahead-of-schema",
            Self::DuplicateNodeId { .. } => "duplicate-node-id",
            Self::UnknownParent { .. } => "unknown-parent",
            Self::Cycle { .. } => "cycle",
            Self::InvalidTransform { .. } => "invalid-transform",
            Self::BlankLabel { .. } => "blank-label",
            Self::BlankTag { .. } => "blank-tag",
            Self::DuplicateTag { .. } => "duplicate-tag",
            Self::DuplicateAssetDependency { .. } => "duplicate-asset-dependency",
            Self::MissingAssetDependency { .. } => "missing-asset-dependency",
            Self::AssetKindMismatch { .. } => "asset-kind-mismatch",
            Self::InvalidVoxelVolumeTransform { .. } => "invalid-voxel-volume-transform",
            Self::InvalidLight { .. } => "invalid-light",
            Self::InvalidMarker { .. } => "invalid-marker",
            Self::DuplicateMarkerId { .. } => "duplicate-marker-id",
            Self::InvalidEntityInstance { .. } => "invalid-entity-instance",
            Self::DuplicateEntityInstanceId { .. } => "duplicate-entity-instance-id",
            Self::UnknownSpawnMarker { .. } => "unknown-spawn-marker",
            Self::DuplicateBootstrapNode { .. } => "duplicate-bootstrap-node",
            Self::InvalidBootstrap { .. } => "invalid-bootstrap",
            Self::DuplicateCatalogBinding { .. } => "duplicate-catalog-binding",
        }
    }

    pub fn diagnostic(&self) -> SceneDiagnostic {
        let (path, message) = match self {
            Self::UnsupportedSchemaVersion { found, supported } => (
                "schemaVersion".to_string(),
                format!("scene schema {found} is unsupported; latest supported is {supported}"),
            ),
            Self::AuthoringVersionAheadOfSchema { authored, schema } => (
                "metadata.authoringFormatVersion".to_string(),
                format!("authoring format {authored} is ahead of scene schema {schema}"),
            ),
            Self::DuplicateNodeId { id } => (
                format!("nodes[{}].id", id.raw()),
                format!("scene node id {} occurs more than once", id.raw()),
            ),
            Self::UnknownParent { node, parent } => (
                format!("nodes[{}].parent", node.raw()),
                format!("parent node {} is absent", parent.raw()),
            ),
            Self::Cycle { path } => (
                "nodes".to_string(),
                format!(
                    "parent cycle: {}",
                    path.iter()
                        .map(|node| node.raw().to_string())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                ),
            ),
            Self::InvalidTransform { node, reason } => (
                format!("nodes[{}].transform", node.raw()),
                format!("transform is invalid: {}", reason.code()),
            ),
            Self::BlankLabel { node } => (
                format!("nodes[{}].metadata.label", node.raw()),
                "label must not be blank".to_string(),
            ),
            Self::BlankTag { node } => (
                format!("nodes[{}].metadata.tags", node.raw()),
                "tag must not be blank".to_string(),
            ),
            Self::DuplicateTag { node, tag } => (
                format!("nodes[{}].metadata.tags", node.raw()),
                format!("tag `{tag}` occurs more than once"),
            ),
            Self::DuplicateAssetDependency { asset } => (
                "dependencies".to_string(),
                format!("asset dependency `{asset}` occurs more than once"),
            ),
            Self::MissingAssetDependency { node, asset } => (
                format!("nodes[{}].asset", node.raw()),
                format!("asset `{asset}` is not declared by scene dependencies"),
            ),
            Self::AssetKindMismatch {
                node,
                expected,
                actual,
            } => (
                format!("nodes[{}].asset", node.raw()),
                format!(
                    "asset is {}, expected {}",
                    actual.prefix(),
                    expected.prefix()
                ),
            ),
            Self::InvalidVoxelVolumeTransform { node, reason } => (
                format!("nodes[{}].transform", node.raw()),
                format!("voxel-volume world transform is invalid: {reason}"),
            ),
            Self::InvalidLight { node, reason } => (
                format!("nodes[{}].light", node.raw()),
                format!("light is invalid: {}", reason.code()),
            ),
            Self::InvalidMarker { node, reason } => (
                format!("nodes[{}].marker", node.raw()),
                format!("marker is invalid: {reason}"),
            ),
            Self::DuplicateMarkerId { node, marker_id } => (
                format!("nodes[{}].marker.markerId", node.raw()),
                format!("marker id `{marker_id}` occurs more than once"),
            ),
            Self::InvalidEntityInstance { node, reason } => (
                format!("nodes[{}].entityInstance", node.raw()),
                format!("entity instance is invalid: {reason}"),
            ),
            Self::DuplicateEntityInstanceId { node, instance_id } => (
                format!("nodes[{}].entityInstance.instanceId", node.raw()),
                format!("entity instance id `{instance_id}` occurs more than once"),
            ),
            Self::UnknownSpawnMarker { node, marker_id } => (
                format!("nodes[{}].entityInstance.spawnMarkerId", node.raw()),
                format!("spawn marker `{marker_id}` is absent"),
            ),
            Self::DuplicateBootstrapNode { node } => (
                format!("nodes[{}]", node.raw()),
                "only one scene bootstrap node is allowed".to_string(),
            ),
            Self::InvalidBootstrap { node, reason } => (
                format!("nodes[{}].bootstrap", node.raw()),
                format!("scene bootstrap is invalid: {reason}"),
            ),
            Self::DuplicateCatalogBinding { node, binding_id } => (
                format!("nodes[{}].bootstrap.catalogs", node.raw()),
                format!("catalog binding `{binding_id}` occurs more than once"),
            ),
        };
        SceneDiagnostic {
            code: self.code().to_string(),
            path,
            message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneValidationReport {
    pub errors: Vec<SceneValidationError>,
}

impl SceneValidationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn diagnostics(&self) -> Vec<SceneDiagnostic> {
        self.errors
            .iter()
            .map(SceneValidationError::diagnostic)
            .collect()
    }
}

pub fn validate_scene(document: &FlatSceneDocument) -> SceneValidationReport {
    let mut errors = Vec::new();
    if !(1..=CURRENT_SCENE_SCHEMA_VERSION).contains(&document.schema_version) {
        errors.push(SceneValidationError::UnsupportedSchemaVersion {
            found: document.schema_version,
            supported: CURRENT_SCENE_SCHEMA_VERSION,
        });
    }
    if document.metadata.authoring_format_version > document.schema_version {
        errors.push(SceneValidationError::AuthoringVersionAheadOfSchema {
            authored: document.metadata.authoring_format_version,
            schema: document.schema_version,
        });
    }

    let mut known = BTreeSet::new();
    let mut duplicate_ids = BTreeSet::new();
    for node in &document.nodes {
        if !known.insert(node.id) && duplicate_ids.insert(node.id) {
            errors.push(SceneValidationError::DuplicateNodeId { id: node.id });
        }
    }

    let mut dependency_ids = BTreeSet::new();
    for dependency in &document.dependencies {
        if !dependency_ids.insert(dependency.id().as_str()) {
            errors.push(SceneValidationError::DuplicateAssetDependency {
                asset: dependency.id().as_str().to_string(),
            });
        }
    }

    let marker_ids = document
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            SceneNodeKind::Marker(marker) if valid_stable_id(&marker.marker_id) => {
                Some(marker.marker_id.as_str())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut seen_markers = BTreeSet::new();
    let mut seen_instances = BTreeSet::new();
    let mut bootstrap_seen = false;

    for node in &document.nodes {
        if let Some(parent) = node.parent {
            if !known.contains(&parent) {
                errors.push(SceneValidationError::UnknownParent {
                    node: node.id,
                    parent,
                });
            }
        }
        if let Err(reason) = validate_transform(node.transform) {
            errors.push(SceneValidationError::InvalidTransform {
                node: node.id,
                reason,
            });
        }
        if node
            .metadata
            .label
            .as_deref()
            .is_some_and(|label| label.trim().is_empty())
        {
            errors.push(SceneValidationError::BlankLabel { node: node.id });
        }
        let mut tags = BTreeSet::new();
        for tag in &node.metadata.tags {
            if tag.trim().is_empty() {
                errors.push(SceneValidationError::BlankTag { node: node.id });
            } else if !tags.insert(tag) {
                errors.push(SceneValidationError::DuplicateTag {
                    node: node.id,
                    tag: tag.clone(),
                });
            }
        }
        if let (Some(expected), Some(asset)) = (node.kind.expected_asset_kind(), node.kind.asset())
        {
            if asset.kind() != expected {
                errors.push(SceneValidationError::AssetKindMismatch {
                    node: node.id,
                    expected,
                    actual: asset.kind(),
                });
            }
            if !dependency_ids.contains(asset.id().as_str()) {
                errors.push(SceneValidationError::MissingAssetDependency {
                    node: node.id,
                    asset: asset.id().as_str().to_string(),
                });
            }
        }
        match &node.kind {
            SceneNodeKind::Light(light) => {
                let result = if document.schema_version < 2
                    || document.metadata.authoring_format_version < 2
                {
                    Err(SceneLightInvalid::RequiresSchema2)
                } else if node.transform.scale != Vec3::ONE {
                    Err(SceneLightInvalid::NonUnitScale)
                } else {
                    light.validate()
                };
                if let Err(reason) = result {
                    errors.push(SceneValidationError::InvalidLight {
                        node: node.id,
                        reason,
                    });
                }
            }
            SceneNodeKind::Marker(marker) => {
                if document.schema_version < 4 || document.metadata.authoring_format_version < 4 {
                    errors.push(SceneValidationError::InvalidMarker {
                        node: node.id,
                        reason: "requires-schema-4",
                    });
                }
                if !valid_stable_id(&marker.marker_id) {
                    errors.push(SceneValidationError::InvalidMarker {
                        node: node.id,
                        reason: "invalid-marker-id",
                    });
                } else if !seen_markers.insert(marker.marker_id.as_str()) {
                    errors.push(SceneValidationError::DuplicateMarkerId {
                        node: node.id,
                        marker_id: marker.marker_id.clone(),
                    });
                }
            }
            SceneNodeKind::EntityInstance(instance) => {
                validate_instance(
                    document,
                    node.id,
                    instance,
                    &marker_ids,
                    &mut seen_instances,
                    &mut errors,
                );
            }
            SceneNodeKind::Bootstrap(bindings) => {
                if bootstrap_seen {
                    errors.push(SceneValidationError::DuplicateBootstrapNode { node: node.id });
                }
                bootstrap_seen = true;
                validate_bootstrap(document, node, bindings, &mut errors);
            }
            SceneNodeKind::EmptyGroup
            | SceneNodeKind::StaticMesh(_)
            | SceneNodeKind::Sprite(_)
            | SceneNodeKind::VoxelVolume(_) => {}
        }
    }

    detect_cycles(document, &known, &mut errors);
    if !errors.iter().any(|error| {
        matches!(
            error,
            SceneValidationError::DuplicateNodeId { .. }
                | SceneValidationError::UnknownParent { .. }
                | SceneValidationError::Cycle { .. }
                | SceneValidationError::InvalidTransform { .. }
        )
    }) {
        let world = composed_world_transforms(document);
        for node in &document.nodes {
            if !matches!(node.kind, SceneNodeKind::VoxelVolume(_)) {
                continue;
            }
            let transform = world[&node.id];
            if transform.rotation != crate::Quat::IDENTITY {
                errors.push(SceneValidationError::InvalidVoxelVolumeTransform {
                    node: node.id,
                    reason: "non-identity-rotation",
                });
            }
            if transform.scale != Vec3::ONE {
                errors.push(SceneValidationError::InvalidVoxelVolumeTransform {
                    node: node.id,
                    reason: "non-unit-scale",
                });
            }
        }
    }
    SceneValidationReport { errors }
}

fn validate_instance<'a>(
    document: &FlatSceneDocument,
    node: SceneNodeId,
    instance: &'a crate::SceneEntityInstance,
    marker_ids: &BTreeSet<&str>,
    seen_instances: &mut BTreeSet<&'a str>,
    errors: &mut Vec<SceneValidationError>,
) {
    if document.schema_version < 3 || document.metadata.authoring_format_version < 3 {
        errors.push(SceneValidationError::InvalidEntityInstance {
            node,
            reason: "requires-schema-3",
        });
    }
    if !valid_stable_id(&instance.instance_id) {
        errors.push(SceneValidationError::InvalidEntityInstance {
            node,
            reason: "invalid-instance-id",
        });
    } else if !seen_instances.insert(&instance.instance_id) {
        errors.push(SceneValidationError::DuplicateEntityInstanceId {
            node,
            instance_id: instance.instance_id.clone(),
        });
    }
    match &instance.reference {
        crate::SceneEntityReference::EntityDefinition { stable_id }
            if !valid_stable_id(stable_id) =>
        {
            errors.push(SceneValidationError::InvalidEntityInstance {
                node,
                reason: "invalid-entity-definition-id",
            });
        }
        crate::SceneEntityReference::Prefab {
            prefab_id,
            variant_id,
            ..
        } => {
            if document.schema_version < 4 || document.metadata.authoring_format_version < 4 {
                errors.push(SceneValidationError::InvalidEntityInstance {
                    node,
                    reason: "prefab-seed-requires-schema-4",
                });
            }
            if prefab_id.raw() == 0 {
                errors.push(SceneValidationError::InvalidEntityInstance {
                    node,
                    reason: "invalid-prefab-id",
                });
            }
            if variant_id
                .as_deref()
                .is_some_and(|variant| !valid_stable_id(variant))
            {
                errors.push(SceneValidationError::InvalidEntityInstance {
                    node,
                    reason: "invalid-prefab-variant-id",
                });
            }
        }
        crate::SceneEntityReference::EntityDefinition { .. } => {}
    }
    if let Some(marker_id) = instance.spawn_marker_id.as_deref() {
        if !valid_stable_id(marker_id) {
            errors.push(SceneValidationError::InvalidEntityInstance {
                node,
                reason: "invalid-spawn-marker-id",
            });
        } else if !marker_ids.contains(marker_id) {
            errors.push(SceneValidationError::UnknownSpawnMarker {
                node,
                marker_id: marker_id.to_string(),
            });
        }
    }
}

fn validate_bootstrap(
    document: &FlatSceneDocument,
    node: &crate::SceneNodeRecord,
    bindings: &crate::SceneBootstrapBindings,
    errors: &mut Vec<SceneValidationError>,
) {
    if document.schema_version < 3 || document.metadata.authoring_format_version < 3 {
        errors.push(SceneValidationError::InvalidBootstrap {
            node: node.id,
            reason: "requires-schema-3",
        });
    }
    if node.parent.is_some() {
        errors.push(SceneValidationError::InvalidBootstrap {
            node: node.id,
            reason: "bootstrap-must-be-root",
        });
    }
    if node.transform != SceneTransform::IDENTITY {
        errors.push(SceneValidationError::InvalidBootstrap {
            node: node.id,
            reason: "bootstrap-transform-must-be-identity",
        });
    }
    if let Some(generator) = &bindings.generator {
        if !valid_stable_id(&generator.provider_id) || !valid_stable_id(&generator.preset_id) {
            errors.push(SceneValidationError::InvalidBootstrap {
                node: node.id,
                reason: "invalid-generator-binding",
            });
        }
    }
    let mut binding_ids = BTreeSet::new();
    for catalog in &bindings.catalogs {
        if !valid_stable_id(&catalog.binding_id)
            || !valid_stable_id(&catalog.catalog_id)
            || !valid_project_relative_path(&catalog.source_path)
        {
            errors.push(SceneValidationError::InvalidBootstrap {
                node: node.id,
                reason: "invalid-catalog-binding",
            });
        } else if !binding_ids.insert(catalog.binding_id.as_str()) {
            errors.push(SceneValidationError::DuplicateCatalogBinding {
                node: node.id,
                binding_id: catalog.binding_id.clone(),
            });
        }
    }
}

fn validate_transform(transform: SceneTransform) -> Result<(), TransformInvalid> {
    if !vector_finite(transform.translation) {
        return Err(TransformInvalid::NonFiniteTranslation);
    }
    if [
        transform.translation.x,
        transform.translation.y,
        transform.translation.z,
    ]
    .into_iter()
    .any(|value| value.abs() > entity_state::MAX_ABS_TRANSLATION)
    {
        return Err(TransformInvalid::TranslationOutOfRange);
    }
    if ![
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
    ]
    .into_iter()
    .all(f32::is_finite)
    {
        return Err(TransformInvalid::NonFiniteRotation);
    }
    if (transform.rotation.norm_squared() - 1.0).abs() > 0.001 {
        return Err(TransformInvalid::NonUnitRotation);
    }
    if !vector_finite(transform.scale) {
        return Err(TransformInvalid::NonFiniteScale);
    }
    if transform.scale.x <= 0.0 || transform.scale.y <= 0.0 || transform.scale.z <= 0.0 {
        return Err(TransformInvalid::NonPositiveScale);
    }
    Ok(())
}

fn vector_finite(vector: Vec3) -> bool {
    vector.x.is_finite() && vector.y.is_finite() && vector.z.is_finite()
}

pub fn composed_world_transforms(
    document: &FlatSceneDocument,
) -> BTreeMap<SceneNodeId, SceneTransform> {
    let records = document
        .nodes
        .iter()
        .map(|record| (record.id, record))
        .collect::<BTreeMap<_, _>>();
    let mut resolved = BTreeMap::new();
    for record in &document.nodes {
        resolve_world_transform(record.id, &records, &mut resolved);
    }
    resolved
}

fn resolve_world_transform(
    node: SceneNodeId,
    records: &BTreeMap<SceneNodeId, &crate::SceneNodeRecord>,
    resolved: &mut BTreeMap<SceneNodeId, SceneTransform>,
) -> SceneTransform {
    if let Some(transform) = resolved.get(&node) {
        return *transform;
    }
    let record = records[&node];
    let world = record.parent.map_or(record.transform, |parent| {
        resolve_world_transform(parent, records, resolved).compose(record.transform)
    });
    resolved.insert(node, world);
    world
}

fn detect_cycles(
    document: &FlatSceneDocument,
    known: &BTreeSet<SceneNodeId>,
    errors: &mut Vec<SceneValidationError>,
) {
    let parents = document
        .nodes
        .iter()
        .map(|node| (node.id, node.parent))
        .collect::<BTreeMap<_, _>>();
    let mut completed = BTreeSet::new();
    let mut cyclic = BTreeSet::new();
    for start in parents.keys().copied() {
        if completed.contains(&start) || cyclic.contains(&start) {
            continue;
        }
        let mut order = Vec::new();
        let mut local = BTreeSet::new();
        let mut current = start;
        loop {
            if completed.contains(&current) || cyclic.contains(&current) {
                completed.extend(order);
                break;
            }
            if !local.insert(current) {
                let index = order
                    .iter()
                    .position(|candidate| *candidate == current)
                    .unwrap();
                let path = order[index..].to_vec();
                cyclic.extend(path.iter().copied());
                errors.push(SceneValidationError::Cycle { path });
                break;
            }
            order.push(current);
            match parents.get(&current).copied().flatten() {
                Some(parent) if known.contains(&parent) => current = parent,
                _ => {
                    completed.extend(order);
                    break;
                }
            }
        }
    }
}

fn valid_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/' | b':' | b'@')
        })
}

fn valid_project_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value.starts_with('/')
        && !value.contains('\\')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "..")
}
