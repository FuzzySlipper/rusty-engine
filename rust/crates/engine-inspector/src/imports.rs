use std::fmt::Write;

use asset_import::{
    decode_import_manifest, import_with_context, parse_source, validate_import_manifest,
    ImportContext, ImportDiagnostic, ImportManifest, ImportSeverity, SourceCollision,
};
use serde::Serialize;

use crate::{
    Diagnostic, DiagnosticDomain, DiagnosticLocation, DiagnosticSet, DiagnosticSeverity,
    RemedyAction,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSourceInspection {
    pub locus: String,
    pub name: Option<String>,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub material_count: usize,
    pub group_count: usize,
    pub collision: Option<String>,
    pub produced_asset_count: usize,
    pub diagnostics: DiagnosticSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportManifestInspection {
    pub schema_version: u32,
    pub source_uri: String,
    pub source_hash: String,
    pub source_schema_version: u32,
    pub importer_version: u32,
    pub mesh_asset_id: String,
    pub guid: Option<String>,
    pub artifact_count: usize,
    pub declared_byte_count: u64,
    pub artifact_paths: Vec<String>,
    pub diagnostics: DiagnosticSet,
}

pub fn inspect_import_source(
    source_text: &str,
    locus: &str,
    context: &ImportContext,
) -> ImportSourceInspection {
    let parsed = parse_source(source_text, locus);
    let mut diagnostics = DiagnosticSet::new();
    diagnostics.extend(parsed.diagnostics.iter().map(import_diagnostic));

    let Some(mesh) = parsed.mesh else {
        return ImportSourceInspection {
            locus: locus.to_string(),
            name: None,
            vertex_count: 0,
            triangle_count: 0,
            material_count: 0,
            group_count: 0,
            collision: None,
            produced_asset_count: 0,
            diagnostics,
        };
    };

    let outcome = import_with_context(&mesh, context);
    diagnostics.extend(outcome.diagnostics.iter().map(import_diagnostic));
    let produced_asset_count = outcome
        .assets
        .as_ref()
        .map_or(0, |assets| 1 + assets.catalog.entries.len());
    ImportSourceInspection {
        locus: locus.to_string(),
        name: Some(mesh.name),
        vertex_count: mesh.positions.len() / 3,
        triangle_count: mesh.indices.len() / 3,
        material_count: mesh.materials.len(),
        group_count: mesh.groups.len(),
        collision: Some(collision_label(&mesh.collision)),
        produced_asset_count,
        diagnostics,
    }
}

pub fn inspect_import_manifest(manifest: &ImportManifest) -> ImportManifestInspection {
    let mut artifact_paths = manifest
        .artifacts
        .iter()
        .map(|artifact| artifact.relative_path.clone())
        .collect::<Vec<_>>();
    artifact_paths.sort();
    let diagnostics =
        validate_import_manifest(manifest)
            .err()
            .map_or_else(DiagnosticSet::new, |error| {
                DiagnosticSet::one(
                    Diagnostic::new(
                        DiagnosticDomain::Import,
                        DiagnosticSeverity::Fatal,
                        "importManifest.invalid",
                        DiagnosticLocation::path(error.path),
                        error.message,
                    )
                    .with_remedy(
                        RemedyAction::RestoreArtifact,
                        "fix or regenerate the import manifest",
                    ),
                )
            });
    ImportManifestInspection {
        schema_version: manifest.schema_version,
        source_uri: manifest.source_uri.clone(),
        source_hash: manifest.source_hash.clone(),
        source_schema_version: manifest.source_schema_version,
        importer_version: manifest.importer_version,
        mesh_asset_id: manifest.mesh_asset_id.clone(),
        guid: manifest.guid.as_ref().map(|guid| guid.as_str().to_string()),
        artifact_count: manifest.artifacts.len(),
        declared_byte_count: manifest.artifacts.iter().fold(0_u64, |total, artifact| {
            total.saturating_add(artifact.byte_len)
        }),
        artifact_paths,
        diagnostics,
    }
}

pub fn inspect_import_manifest_json(
    manifest_json: &str,
) -> Result<ImportManifestInspection, DiagnosticSet> {
    let manifest = decode_import_manifest(manifest_json).map_err(|error| {
        DiagnosticSet::one(
            Diagnostic::new(
                DiagnosticDomain::Import,
                DiagnosticSeverity::Fatal,
                "importManifest.decode",
                DiagnosticLocation::path(error.path),
                error.message,
            )
            .with_remedy(
                RemedyAction::RestoreArtifact,
                "fix or regenerate the import manifest",
            ),
        )
    })?;
    Ok(inspect_import_manifest(&manifest))
}

impl ImportSourceInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "import-source locus={:?} name={:?} vertices={} triangles={} materials={} groups={} collision={:?} producedAssets={}\n",
            self.locus,
            self.name,
            self.vertex_count,
            self.triangle_count,
            self.material_count,
            self.group_count,
            self.collision,
            self.produced_asset_count
        );
        output.push_str(&self.diagnostics.to_text());
        output
    }
}

impl ImportManifestInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "import-manifest schema={} source={:?} sourceSchema={} importer={} mesh={} guid={:?} artifacts={} declaredBytes={}\n",
            self.schema_version,
            self.source_uri,
            self.source_schema_version,
            self.importer_version,
            self.mesh_asset_id,
            self.guid,
            self.artifact_count,
            self.declared_byte_count
        );
        let _ = writeln!(output, "source-hash {}", self.source_hash);
        for path in &self.artifact_paths {
            let _ = writeln!(output, "artifact path={path}");
        }
        output.push_str(&self.diagnostics.to_text());
        output
    }
}

fn import_diagnostic(source: &ImportDiagnostic) -> Diagnostic {
    Diagnostic::new(
        DiagnosticDomain::Import,
        match source.severity {
            ImportSeverity::Warning => DiagnosticSeverity::Warning,
            ImportSeverity::Error => DiagnosticSeverity::Error,
        },
        source.code.label(),
        DiagnosticLocation::path(&source.locus),
        &source.message,
    )
    .with_remedy(RemedyAction::Inspect, &source.remedy)
}

fn collision_label(collision: &SourceCollision) -> String {
    match collision {
        SourceCollision::VisualOnly => "visualOnly".to_string(),
        SourceCollision::AabbFallback => "aabbFallback".to_string(),
        SourceCollision::Trimesh => "trimesh".to_string(),
        SourceCollision::Proxy(path) => format!("proxy:{path}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"{
      "schemaVersion": 1,
      "name": "wall",
      "positions": [0,0,0, 1,0,0, 0,1,0],
      "normals": [0,0,1, 0,0,1, 0,0,1],
      "indices": [0,1,2],
      "collision": "aabbFallback"
    }"#;

    #[test]
    fn source_inspection_runs_the_real_offline_import_without_mutation() {
        let report = inspect_import_source(SOURCE, "wall.source.json", &ImportContext::default());
        assert_eq!(report.name.as_deref(), Some("wall"));
        assert_eq!(report.vertex_count, 3);
        assert_eq!(report.triangle_count, 1);
        assert_eq!(report.collision.as_deref(), Some("aabbFallback"));
        assert!(report.produced_asset_count >= 2);
        assert!(!report.diagnostics.has_errors());
    }

    #[test]
    fn malformed_source_preserves_owner_codes_and_remedies() {
        let report = inspect_import_source("{ nope", "bad.json", &ImportContext::default());
        assert_eq!(report.produced_asset_count, 0);
        assert_eq!(report.diagnostics.diagnostics[0].code, "malformedSource");
        assert!(report.diagnostics.has_errors());
    }

    #[test]
    fn trimesh_collision_uses_the_canonical_source_label() {
        assert_eq!(collision_label(&SourceCollision::Trimesh), "trimesh");
    }

    #[test]
    fn typed_manifest_inspection_reuses_owner_validation() {
        let invalid = ImportManifest {
            schema_version: 99,
            source_uri: String::new(),
            source_hash: "not-a-hash".to_string(),
            source_schema_version: 0,
            importer_version: 0,
            mesh_asset_id: "not-an-asset".to_string(),
            guid: None,
            artifacts: Vec::new(),
        };

        let report = inspect_import_manifest(&invalid);

        assert_eq!(report.diagnostics.diagnostics.len(), 1);
        assert_eq!(
            report.diagnostics.diagnostics[0].code,
            "importManifest.invalid"
        );
        assert!(report.diagnostics.blocks_load());
    }
}
