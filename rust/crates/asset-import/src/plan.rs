use crate::{
    build_manifest, encode_import_manifest, import_text, plan_reimport, render_artifacts,
    GeneratedArtifact, ImportContext, ImportDiagnostic, ImportManifest, ReimportPlan,
    SidecarMetadata, SourceUri,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    DryRun,
    Write,
}

impl ImportMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DryRun => "dry-run",
            Self::Write => "write",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPlan {
    pub mode: ImportMode,
    pub source_uri: SourceUri,
    pub diagnostics: Vec<ImportDiagnostic>,
    pub files: Vec<GeneratedArtifact>,
    pub manifest: Option<ImportManifest>,
    pub reimport: Option<ReimportPlan>,
    pub sidecar_update: Option<SidecarMetadata>,
    pub report: String,
    pub has_errors: bool,
}

pub fn plan_import(
    source_uri: &SourceUri,
    source_text: &str,
    context: &ImportContext,
    mode: ImportMode,
    prior: Option<&ImportManifest>,
    sidecar: Option<&SidecarMetadata>,
) -> ImportPlan {
    let outcome = import_text(source_text, source_uri.value(), context);
    let mut report = format!(
        "rusty-asset-import: {}\nmode: {}\ndiagnostics: {}\n",
        source_uri.value(),
        mode.label(),
        outcome.diagnostics.len()
    );
    for diagnostic in &outcome.diagnostics {
        report.push_str("  ");
        report.push_str(&diagnostic.render());
        report.push('\n');
    }
    let Some(assets) = outcome.assets else {
        report.push_str("result: failed; no publication candidate produced\n");
        return ImportPlan {
            mode,
            source_uri: source_uri.clone(),
            diagnostics: outcome.diagnostics,
            files: Vec::new(),
            manifest: None,
            reimport: None,
            sidecar_update: None,
            report,
            has_errors: true,
        };
    };
    let name = assets
        .static_mesh
        .asset
        .strip_prefix("mesh/")
        .unwrap_or(&assets.static_mesh.asset);
    let Ok(mut files) = render_artifacts(name, &assets) else {
        report.push_str("result: failed; generated artifacts could not be encoded\n");
        return ImportPlan {
            mode,
            source_uri: source_uri.clone(),
            diagnostics: outcome.diagnostics,
            files: Vec::new(),
            manifest: None,
            reimport: None,
            sidecar_update: None,
            report,
            has_errors: true,
        };
    };
    let manifest = build_manifest(
        source_uri.value(),
        source_text.as_bytes(),
        crate::SUPPORTED_SOURCE_SCHEMA,
        &assets.static_mesh.asset,
        sidecar.map(|metadata| metadata.guid.clone()),
        &files,
    );
    let manifest_json = encode_import_manifest(&manifest)
        .expect("newly built import manifest is valid and serializable");
    files.push(GeneratedArtifact {
        relative_path: format!("{name}.import.json"),
        bytes: manifest_json.into_bytes(),
    });
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let reimport = prior
        .map(|prior| plan_reimport(prior, &manifest))
        .unwrap_or_else(|| ReimportPlan::StructuralReload {
            reason: "first import".to_owned(),
            changed: manifest
                .artifacts
                .iter()
                .map(|artifact| artifact.relative_path.clone())
                .collect(),
        });
    let sidecar_update = sidecar.map(|prior| {
        let mut next = prior.clone();
        next.source_uri = source_uri.clone();
        next.source_hash = manifest.source_hash.clone();
        next.importer_version = manifest.importer_version;
        next.generated_artifacts = manifest.artifacts.clone();
        next
    });
    report.push_str(&format!("asset: {}\n", assets.static_mesh.asset));
    report.push_str(&format!("sourceHash: {}\n", manifest.source_hash));
    report.push_str(&format!("reimportPlan: {}\n", reimport.label()));
    report.push_str("files:\n");
    for file in &files {
        report.push_str(&format!(
            "  {} {} bytes\n",
            file.relative_path,
            file.bytes.len()
        ));
    }
    report.push_str(match mode {
        ImportMode::DryRun => "result: ok; dry-run leaves storage unchanged\n",
        ImportMode::Write => "result: ok; publication candidate ready\n",
    });
    ImportPlan {
        mode,
        source_uri: source_uri.clone(),
        diagnostics: outcome.diagnostics,
        files,
        manifest: Some(manifest),
        reimport: Some(reimport),
        sidecar_update,
        report,
        has_errors: false,
    }
}
