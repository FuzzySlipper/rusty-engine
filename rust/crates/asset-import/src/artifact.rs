use asset_catalog::encode_catalog;

use crate::{ImportedAnimatedGlb, ImportedAssets};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedArtifact {
    pub relative_path: String,
    pub bytes: Vec<u8>,
    /// The bytes' SHA-256 (lowercase hex) when the import already computed
    /// it: a runtime GLB is the hashed source. The manifest hashes the rest.
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRenderError(pub String);

impl std::fmt::Display for ArtifactRenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ArtifactRenderError {}

pub fn render_artifacts(
    name: &str,
    assets: &ImportedAssets,
) -> Result<Vec<GeneratedArtifact>, ArtifactRenderError> {
    let catalog =
        encode_catalog(&assets.catalog).map_err(|error| ArtifactRenderError(error.to_string()))?;
    let mut mesh = serde_json::to_string_pretty(&assets.static_mesh)
        .map_err(|error| ArtifactRenderError(error.to_string()))?;
    mesh.push('\n');
    let mut artifacts = vec![
        GeneratedArtifact {
            relative_path: format!("{name}.catalog.json"),
            bytes: catalog.into_bytes(),
            sha256: None,
        },
        GeneratedArtifact {
            relative_path: format!("{name}.static-mesh.json"),
            bytes: mesh.into_bytes(),
            sha256: None,
        },
    ];
    artifacts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(artifacts)
}

/// `source` is the imported GLB, published unchanged as the runtime resource.
pub fn render_animated_glb_artifacts(
    name: &str,
    assets: &ImportedAnimatedGlb,
    source: &[u8],
) -> Result<Vec<GeneratedArtifact>, ArtifactRenderError> {
    let catalog =
        encode_catalog(&assets.catalog).map_err(|error| ArtifactRenderError(error.to_string()))?;
    let mut mesh = serde_json::to_string_pretty(&assets.animated_mesh)
        .map_err(|error| ArtifactRenderError(error.to_string()))?;
    mesh.push('\n');
    let mut artifacts = vec![
        GeneratedArtifact {
            relative_path: format!("{name}.animated-mesh.json"),
            bytes: mesh.into_bytes(),
            sha256: None,
        },
        GeneratedArtifact {
            relative_path: assets.runtime_resource_path.clone(),
            bytes: source.to_vec(),
            sha256: assets
                .animated_mesh
                .content_hash
                .as_deref()
                .and_then(|hash| hash.strip_prefix("sha256:"))
                .map(str::to_owned),
        },
        GeneratedArtifact {
            relative_path: format!("{name}.catalog.json"),
            bytes: catalog.into_bytes(),
            sha256: None,
        },
    ];
    artifacts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(artifacts)
}
