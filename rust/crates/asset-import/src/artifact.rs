use asset_catalog::encode_catalog;

use crate::ImportedAssets;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedArtifact {
    pub relative_path: String,
    pub bytes: Vec<u8>,
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
        },
        GeneratedArtifact {
            relative_path: format!("{name}.static-mesh.json"),
            bytes: mesh.into_bytes(),
        },
    ];
    artifacts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(artifacts)
}

pub(crate) fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.trim() == path
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
