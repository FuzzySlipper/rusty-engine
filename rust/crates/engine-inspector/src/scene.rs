use std::collections::BTreeMap;
use std::fmt::Write;

use asset_catalog::{decode_catalog, AssetCatalog};
use authored_scene::{
    decode_scene_unvalidated, validate_scene, FlatSceneDocument, SceneValidationError,
};
use core_assets::{AssetReference, AssetVersionReq};
use serde::Serialize;

use crate::{
    catalog::{inspect_catalog, NamedCount},
    Diagnostic, DiagnosticDomain, DiagnosticLocation, DiagnosticSet, DiagnosticSeverity,
    RemedyAction,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneInspection {
    pub scene_id: u64,
    pub revision: u64,
    pub schema_version: u32,
    pub name: Option<String>,
    pub node_count: usize,
    pub root_count: usize,
    pub dependency_count: usize,
    pub node_kinds: Vec<NamedCount>,
    pub diagnostics: DiagnosticSet,
}

pub fn inspect_scene(
    document: &FlatSceneDocument,
    catalog: Option<&AssetCatalog>,
) -> SceneInspection {
    let mut kinds = BTreeMap::new();
    for node in &document.nodes {
        *kinds.entry(node.kind.tag().to_string()).or_insert(0) += 1;
    }

    let validation = validate_scene(document);
    let mut diagnostics = DiagnosticSet::new();
    for (error, source) in validation.errors.iter().zip(validation.diagnostics()) {
        let mut location = DiagnosticLocation::path(source.path);
        if let Some(node) = validation_node(error) {
            location = location.with_scene_node(node);
        }
        if let Some(asset) = validation_asset(error) {
            location = location.with_asset(asset);
        }
        diagnostics.push(
            Diagnostic::new(
                DiagnosticDomain::Scene,
                DiagnosticSeverity::Error,
                source.code,
                location,
                source.message,
            )
            .with_remedy(remedy_for(error), "correct the authored scene data"),
        );
    }

    if let Some(catalog) = catalog {
        diagnostics.extend(inspect_catalog(catalog, None).diagnostics.diagnostics);
        diagnostics.extend(catalog_reference_diagnostics(document, catalog));
    }

    SceneInspection {
        scene_id: document.id.raw(),
        revision: document.revision,
        schema_version: document.schema_version,
        name: document.metadata.name.clone(),
        node_count: document.nodes.len(),
        root_count: document
            .nodes
            .iter()
            .filter(|node| node.parent.is_none())
            .count(),
        dependency_count: document.dependencies.len(),
        node_kinds: NamedCount::from_map(kinds),
        diagnostics,
    }
}

pub fn inspect_scene_json(
    scene_json: &str,
    catalog_json: Option<&str>,
) -> Result<SceneInspection, DiagnosticSet> {
    let document = decode_scene_unvalidated(scene_json).map_err(|error| {
        DiagnosticSet::one(
            Diagnostic::new(
                DiagnosticDomain::Scene,
                DiagnosticSeverity::Fatal,
                "scene.decode",
                DiagnosticLocation::path(error.path),
                error.message,
            )
            .with_remedy(RemedyAction::RestoreArtifact, "fix the stored scene shape"),
        )
    })?;
    let catalog = catalog_json
        .map(decode_catalog)
        .transpose()
        .map_err(|error| {
            DiagnosticSet::one(
                Diagnostic::new(
                    DiagnosticDomain::AssetCatalog,
                    DiagnosticSeverity::Fatal,
                    "catalog.decode",
                    DiagnosticLocation::path(error.path),
                    error.message,
                )
                .with_remedy(
                    RemedyAction::RestoreArtifact,
                    "fix the stored catalog shape",
                ),
            )
        })?;
    Ok(inspect_scene(&document, catalog.as_ref()))
}

impl SceneInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "scene id={} revision={} schema={} name={:?}\n",
            self.scene_id, self.revision, self.schema_version, self.name
        );
        let _ = writeln!(
            output,
            "nodes total={} roots={} dependencies={}",
            self.node_count, self.root_count, self.dependency_count
        );
        let kinds = self
            .node_kinds
            .iter()
            .map(|kind| format!("{}={}", kind.name, kind.count))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(output, "node-kinds {kinds}");
        output.push_str(&self.diagnostics.to_text());
        output
    }
}

fn catalog_reference_diagnostics(
    document: &FlatSceneDocument,
    catalog: &AssetCatalog,
) -> Vec<Diagnostic> {
    document
        .dependencies
        .iter()
        .filter_map(|reference| {
            let node = document.nodes.iter().find(|node| {
                node.kind
                    .asset()
                    .is_some_and(|asset| asset.id() == reference.id())
            });
            let mut location = DiagnosticLocation::path(node.map_or_else(
                || format!("dependencies[{}]", reference.id().as_str()),
                |node| format!("nodes[{}].asset", node.id.raw()),
            ))
            .with_asset(reference.id().as_str());
            if let Some(node) = node {
                location = location.with_scene_node(node.id.raw());
            }

            let Some(entry) = catalog.get(reference.id()) else {
                return Some(
                    Diagnostic::new(
                        DiagnosticDomain::Scene,
                        DiagnosticSeverity::Error,
                        "scene.assetMissing",
                        location,
                        "scene dependency is absent from the asset catalog",
                    )
                    .with_remedy(
                        RemedyAction::ProvideAsset,
                        "add the asset or change the reference",
                    ),
                );
            };
            if !version_matches(reference, entry.version) {
                return Some(
                    Diagnostic::new(
                        DiagnosticDomain::Scene,
                        DiagnosticSeverity::Error,
                        "scene.assetVersionMismatch",
                        location,
                        format!(
                            "catalog version {} does not satisfy {:?}",
                            entry.version,
                            reference.version()
                        ),
                    )
                    .with_remedy(
                        RemedyAction::FixReference,
                        "update the reference or catalog asset",
                    ),
                );
            }
            if reference.hash().is_some() && reference.hash() != entry.hash.as_ref() {
                return Some(
                    Diagnostic::new(
                        DiagnosticDomain::Scene,
                        DiagnosticSeverity::Error,
                        "scene.assetHashMismatch",
                        location,
                        "catalog content hash does not match the scene pin",
                    )
                    .with_remedy(
                        RemedyAction::FixReference,
                        "review and repin the intended content",
                    ),
                );
            }
            None
        })
        .collect()
}

fn version_matches(reference: &AssetReference, version: u32) -> bool {
    match reference.version() {
        AssetVersionReq::Any => true,
        AssetVersionReq::Exact(required) => version == required,
        AssetVersionReq::AtLeast(required) => version >= required,
    }
}

fn validation_node(error: &SceneValidationError) -> Option<u64> {
    match error {
        SceneValidationError::DuplicateNodeId { id } => Some(id.raw()),
        SceneValidationError::UnknownParent { node, .. }
        | SceneValidationError::InvalidTransform { node, .. }
        | SceneValidationError::BlankLabel { node }
        | SceneValidationError::BlankTag { node }
        | SceneValidationError::DuplicateTag { node, .. }
        | SceneValidationError::MissingAssetDependency { node, .. }
        | SceneValidationError::AssetKindMismatch { node, .. }
        | SceneValidationError::InvalidVoxelVolumeTransform { node, .. }
        | SceneValidationError::InvalidLight { node, .. }
        | SceneValidationError::InvalidMarker { node, .. }
        | SceneValidationError::DuplicateMarkerId { node, .. }
        | SceneValidationError::InvalidEntityInstance { node, .. }
        | SceneValidationError::DuplicateEntityInstanceId { node, .. }
        | SceneValidationError::UnknownSpawnMarker { node, .. }
        | SceneValidationError::DuplicateBootstrapNode { node }
        | SceneValidationError::InvalidBootstrap { node, .. }
        | SceneValidationError::DuplicateCatalogBinding { node, .. } => Some(node.raw()),
        SceneValidationError::Cycle { path } => path.first().map(|node| node.raw()),
        SceneValidationError::UnsupportedSchemaVersion { .. }
        | SceneValidationError::AuthoringVersionAheadOfSchema { .. }
        | SceneValidationError::DuplicateAssetDependency { .. } => None,
    }
}

fn validation_asset(error: &SceneValidationError) -> Option<&str> {
    match error {
        SceneValidationError::DuplicateAssetDependency { asset }
        | SceneValidationError::MissingAssetDependency { asset, .. } => Some(asset),
        _ => None,
    }
}

fn remedy_for(error: &SceneValidationError) -> RemedyAction {
    match error {
        SceneValidationError::Cycle { .. } => RemedyAction::BreakCycle,
        SceneValidationError::MissingAssetDependency { .. } => RemedyAction::ProvideAsset,
        SceneValidationError::AssetKindMismatch { .. }
        | SceneValidationError::UnknownParent { .. }
        | SceneValidationError::UnknownSpawnMarker { .. } => RemedyAction::FixReference,
        _ => RemedyAction::Inspect,
    }
}

#[cfg(test)]
mod tests {
    use asset_catalog::AssetCatalog;
    use authored_scene::{
        FlatSceneDocument, NodeMetadata, SceneMetadata, SceneNodeKind, SceneNodeRecord,
        SceneTransform, CURRENT_SCENE_SCHEMA_VERSION,
    };
    use core_assets::{AssetId, AssetReference, AssetVersionReq};
    use core_ids::{SceneId, SceneNodeId};

    use super::*;

    #[test]
    fn scene_validation_and_catalog_cross_check_share_one_read_only_report() {
        let asset = AssetReference::new(
            AssetId::parse("mesh/missing").unwrap(),
            AssetVersionReq::Any,
            None,
        );
        let node = SceneNodeRecord {
            id: SceneNodeId::new(7),
            parent: None,
            child_order: 0,
            transform: SceneTransform::IDENTITY,
            kind: SceneNodeKind::StaticMesh(asset.clone()),
            metadata: NodeMetadata::default(),
        };
        let document = FlatSceneDocument {
            id: SceneId::new(9),
            revision: 3,
            schema_version: CURRENT_SCENE_SCHEMA_VERSION,
            metadata: SceneMetadata {
                name: Some("test".to_string()),
                authoring_format_version: CURRENT_SCENE_SCHEMA_VERSION,
            },
            dependencies: vec![asset],
            nodes: vec![node.clone(), node],
        };

        let report = inspect_scene(&document, Some(&AssetCatalog::new()));
        assert_eq!(report.node_count, 2);
        assert!(report
            .diagnostics
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "duplicate-node-id"));
        assert!(report
            .diagnostics
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "scene.assetMissing"));
        assert!(report.to_text().contains("sceneNode=7"));
    }
}
