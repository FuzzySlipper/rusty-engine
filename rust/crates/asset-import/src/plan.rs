use crate::{
    admit_gltf_source, build_manifest, build_manifest_with_source_hash, encode_import_manifest,
    import_animated_glb_asset, import_text, plan_reimport, render_animated_glb_artifacts,
    render_artifacts, GeneratedArtifact, GltfSourceClosure, ImportContext, ImportDiagnostic,
    ImportManifest, ReimportPlan, SidecarMetadata, SourceUri, SUPPORTED_ANIMATED_GLB_VERSION,
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

pub fn plan_animated_glb_import(
    source_uri: &SourceUri,
    source_bytes: &[u8],
    context: &ImportContext,
    mode: ImportMode,
    prior: Option<&ImportManifest>,
    sidecar: Option<&SidecarMetadata>,
) -> ImportPlan {
    let outcome = import_animated_glb_asset(source_uri, source_bytes, context);
    let mut report = format!(
        "rusty-asset-import: {}\nkind: animatedGlb\nmode: {}\ndiagnostics: {}\n",
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
        .animated_mesh
        .asset
        .strip_prefix("mesh-animation/")
        .unwrap_or(&assets.animated_mesh.asset);
    let Ok(mut files) = render_animated_glb_artifacts(name, &assets) else {
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
        source_bytes,
        SUPPORTED_ANIMATED_GLB_VERSION,
        &assets.animated_mesh.asset,
        sidecar.map(|metadata| metadata.guid.clone()),
        &files,
    );
    let manifest_json = encode_import_manifest(&manifest)
        .expect("newly built animated import manifest is valid and serializable");
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
    report.push_str(&format!("asset: {}\n", assets.animated_mesh.asset));
    report.push_str(&format!("sourceHash: {}\n", manifest.source_hash));
    report.push_str(&format!(
        "clips: {} channels: {} keyframes: {}\n",
        assets.receipt.clip_count, assets.receipt.channel_count, assets.receipt.keyframe_count
    ));
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

pub fn plan_animated_gltf_import(
    source_uri: &SourceUri,
    source: &GltfSourceClosure,
    context: &ImportContext,
    mode: ImportMode,
    prior: Option<&ImportManifest>,
    sidecar: Option<&SidecarMetadata>,
) -> ImportPlan {
    let packed = match admit_gltf_source(source) {
        Ok(packed) => packed,
        Err(diagnostic) => {
            let report = format!(
                "rusty-asset-import: {}\nkind: animatedGltf\nmode: {}\ndiagnostics: 1\n  {}\nresult: failed; no publication candidate produced\n",
                source_uri.value(),
                mode.label(),
                diagnostic.render(),
            );
            return ImportPlan {
                mode,
                source_uri: source_uri.clone(),
                diagnostics: vec![diagnostic],
                files: Vec::new(),
                manifest: None,
                reimport: None,
                sidecar_update: None,
                report,
                has_errors: true,
            };
        }
    };
    let glb_uri = glb_runtime_source_uri(source_uri);
    let outcome = import_animated_glb_asset(&glb_uri, &packed.glb_bytes, context);
    let mut report = format!(
        "rusty-asset-import: {}\nkind: animatedGltf\nmode: {}\ndiagnostics: {}\n",
        source_uri.value(),
        mode.label(),
        outcome.diagnostics.len()
    );
    for diagnostic in &outcome.diagnostics {
        report.push_str("  ");
        report.push_str(&diagnostic.render());
        report.push('\n');
    }
    let Some(mut assets) = outcome.assets else {
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
    assets.receipt.source_hash = format!("sha256:{}", packed.source_hash);
    assets.receipt.source_byte_count = packed.source_byte_count;
    let name = assets
        .animated_mesh
        .asset
        .strip_prefix("mesh-animation/")
        .unwrap_or(&assets.animated_mesh.asset);
    let Ok(mut files) = render_animated_glb_artifacts(name, &assets) else {
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
    let manifest = build_manifest_with_source_hash(
        source_uri.value(),
        packed.source_hash,
        SUPPORTED_ANIMATED_GLB_VERSION,
        &assets.animated_mesh.asset,
        sidecar.map(|metadata| metadata.guid.clone()),
        &files,
    );
    let manifest_json = encode_import_manifest(&manifest)
        .expect("newly built glTF import manifest is valid and serializable");
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
    report.push_str(&format!("asset: {}\n", assets.animated_mesh.asset));
    report.push_str(&format!("sourceHash: {}\n", manifest.source_hash));
    report.push_str(&format!(
        "sourceClosure: {} bytes, {} external resources\n",
        packed.source_byte_count,
        packed.external_resource_uris.len()
    ));
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

fn glb_runtime_source_uri(source_uri: &SourceUri) -> SourceUri {
    fn replace(value: &str) -> String {
        let path = std::path::Path::new(value);
        path.with_extension("glb").to_string_lossy().into_owned()
    }
    match source_uri {
        SourceUri::RelativePath(path) => SourceUri::RelativePath(replace(path)),
        SourceUri::AbsolutePath(path) => SourceUri::AbsolutePath(replace(path)),
        SourceUri::FileUrl(path) => SourceUri::FileUrl(replace(path)),
        SourceUri::ContentAddressed(path) => SourceUri::RelativePath(format!("{path}.glb")),
    }
}
