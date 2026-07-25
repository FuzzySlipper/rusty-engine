use std::collections::BTreeMap;
use std::fmt::Write;

use content_store::{
    decode_manifest_unvalidated, ContentLoadPlan, ContentLoadStage, ContentManifest, ManifestError,
};
use serde::Serialize;

use crate::{
    catalog::NamedCount, Diagnostic, DiagnosticDomain, DiagnosticLocation, DiagnosticSet,
    DiagnosticSeverity, RemedyAction,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentLoadStepInspection {
    pub stage: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceInspection {
    pub schema_version: u32,
    pub artifact_count: usize,
    pub required_artifact_count: usize,
    pub declared_byte_count: u64,
    pub classes: Vec<NamedCount>,
    pub roles: Vec<NamedCount>,
    pub load_steps: Vec<ContentLoadStepInspection>,
    pub diagnostics: DiagnosticSet,
}

pub fn inspect_content_manifest(manifest: &ContentManifest) -> PersistenceInspection {
    let mut classes = BTreeMap::new();
    let mut roles = BTreeMap::new();
    let mut declared_byte_count = 0_u64;
    for artifact in &manifest.artifacts {
        *classes.entry(artifact.class.tag().to_string()).or_insert(0) += 1;
        *roles.entry(artifact.role.tag().to_string()).or_insert(0) += 1;
        declared_byte_count =
            declared_byte_count.saturating_add(artifact.byte_len.unwrap_or_default());
    }

    let (load_steps, diagnostics) = match manifest.validate() {
        Ok(()) => {
            let plan = ContentLoadPlan::build(manifest)
                .expect("a validated content manifest always produces a load plan");
            let steps = plan
                .steps
                .into_iter()
                .map(|step| ContentLoadStepInspection {
                    stage: load_stage_label(step.stage).to_string(),
                    path: step.path,
                })
                .collect();
            (steps, DiagnosticSet::new())
        }
        Err(error) => (Vec::new(), DiagnosticSet::one(manifest_diagnostic(&error))),
    };

    PersistenceInspection {
        schema_version: manifest.schema_version,
        artifact_count: manifest.artifacts.len(),
        required_artifact_count: manifest.load_required().count(),
        declared_byte_count,
        classes: NamedCount::from_map(classes),
        roles: NamedCount::from_map(roles),
        load_steps,
        diagnostics,
    }
}

pub fn inspect_content_manifest_json(
    manifest_json: &str,
) -> Result<PersistenceInspection, DiagnosticSet> {
    let manifest = decode_manifest_unvalidated(manifest_json).map_err(|error| {
        DiagnosticSet::one(
            Diagnostic::new(
                DiagnosticDomain::Persistence,
                DiagnosticSeverity::Fatal,
                "contentManifest.decode",
                DiagnosticLocation::path(error.path),
                error.message,
            )
            .with_remedy(
                RemedyAction::RestoreArtifact,
                "fix the stored manifest shape",
            ),
        )
    })?;
    Ok(inspect_content_manifest(&manifest))
}

impl PersistenceInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "content-manifest schema={} artifacts={} required={} declaredBytes={}\n",
            self.schema_version,
            self.artifact_count,
            self.required_artifact_count,
            self.declared_byte_count
        );
        push_counts(&mut output, "classes", &self.classes);
        push_counts(&mut output, "roles", &self.roles);
        let _ = writeln!(output, "load-steps count={}", self.load_steps.len());
        for step in &self.load_steps {
            let _ = writeln!(output, "load {} path={}", step.stage, step.path);
        }
        output.push_str(&self.diagnostics.to_text());
        output
    }
}

fn manifest_diagnostic(error: &ManifestError) -> Diagnostic {
    let (code, path, message, remedy) = match error {
        ManifestError::UnsupportedSchema { found, supported } => (
            "contentManifest.unsupportedSchema",
            "$".to_string(),
            format!("manifest schema {found} is unsupported; expected {supported}"),
            RemedyAction::RestoreArtifact,
        ),
        ManifestError::InvalidPath { path } => (
            "contentManifest.invalidPath",
            path.clone(),
            "artifact path is not safe and project-relative".to_string(),
            RemedyAction::FixReference,
        ),
        ManifestError::DuplicatePath { path } => (
            "contentManifest.duplicatePath",
            path.clone(),
            "artifact path occurs more than once".to_string(),
            RemedyAction::FixReference,
        ),
        ManifestError::StoredArtifactMissingIdentity { path } => (
            "contentManifest.missingIdentity",
            path.clone(),
            "stored artifact is missing its content hash or byte length".to_string(),
            RemedyAction::Regenerate,
        ),
        ManifestError::InvalidCacheMetadata { path } => (
            "contentManifest.invalidCacheMetadata",
            path.clone(),
            "cache class, role, and optional identity metadata disagree".to_string(),
            RemedyAction::RefreshCache,
        ),
        ManifestError::InvalidRole { path } => (
            "contentManifest.invalidRole",
            path.clone(),
            "artifact role is invalid".to_string(),
            RemedyAction::FixReference,
        ),
    };
    Diagnostic::new(
        DiagnosticDomain::Persistence,
        DiagnosticSeverity::Fatal,
        code,
        DiagnosticLocation::path(path),
        message,
    )
    .with_remedy(remedy, "correct the manifest before content admission")
}

fn load_stage_label(stage: ContentLoadStage) -> &'static str {
    match stage {
        ContentLoadStage::AssetAuthority => "assetAuthority",
        ContentLoadStage::AssetData => "assetData",
        ContentLoadStage::Annotations => "annotations",
        ContentLoadStage::Prefabs => "prefabs",
        ContentLoadStage::Scenes => "scenes",
        ContentLoadStage::EntityState => "entityState",
        ContentLoadStage::Resources => "resources",
    }
}

fn push_counts(output: &mut String, label: &str, counts: &[NamedCount]) {
    let values = counts
        .iter()
        .map(|item| format!("{}={}", item.name, item.count))
        .collect::<Vec<_>>()
        .join(" ");
    let _ = writeln!(output, "{label} {values}");
}

#[cfg(test)]
mod tests {
    use content_store::{ArtifactRole, ContentArtifact, ContentManifest};

    use super::*;

    #[test]
    fn report_exposes_counts_and_dependency_ordered_load_steps() {
        let scene =
            ContentArtifact::durable("scenes/main.json", ArtifactRole::SceneDocument, b"scene");
        let catalog = ContentArtifact::durable(
            "catalog/assets.json",
            ArtifactRole::AssetCatalog,
            b"catalog",
        );
        let cache = ContentArtifact::cache("cache/preview.bin");
        let report = inspect_content_manifest(&ContentManifest::new(vec![scene, cache, catalog]));

        assert_eq!(report.artifact_count, 3);
        assert_eq!(report.required_artifact_count, 2);
        assert_eq!(report.load_steps[0].stage, "assetAuthority");
        assert_eq!(report.load_steps[1].stage, "scenes");
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn invalid_manifest_remains_inspectable_without_authorizing_a_load() {
        let artifact = ContentArtifact::durable("same.json", ArtifactRole::SceneDocument, b"scene");
        let manifest = ContentManifest::new(vec![artifact.clone(), artifact]);
        let report = inspect_content_manifest(&manifest);
        assert!(report.load_steps.is_empty());
        assert!(report.diagnostics.blocks_load());
        assert_eq!(
            report.diagnostics.diagnostics[0].code,
            "contentManifest.duplicatePath"
        );
    }
}
