use std::collections::BTreeMap;

use render_model::{
    RenderAssetError, RenderAssetKind, RenderAssetRequirement, ResolvedRenderAsset,
};

/// Narrow, immutable asset information available to presentation validation.
/// It deliberately supports lookup only: it cannot enumerate, mutate, load, or
/// select project resources.
pub trait PresentationAssetLookup {
    fn get_presentation_asset(&self, id: &str) -> Option<&ResolvedRenderAsset>;
}

impl PresentationAssetLookup for BTreeMap<String, ResolvedRenderAsset> {
    fn get_presentation_asset(&self, id: &str) -> Option<&ResolvedRenderAsset> {
        self.get(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationAssetError {
    Missing(String),
    Invalid(RenderAssetError),
}

pub(crate) fn verify_asset(
    assets: &impl PresentationAssetLookup,
    id: &str,
    kind: RenderAssetKind,
    content_hash: Option<&str>,
) -> Result<(), PresentationAssetError> {
    let requirement = RenderAssetRequirement {
        id: id.to_string(),
        kind,
        content_hash: content_hash.map(str::to_string),
        minimum_version: 0,
    };
    let asset = assets
        .get_presentation_asset(id)
        .ok_or_else(|| PresentationAssetError::Missing(id.to_string()))?;
    asset
        .verify_requirement(&requirement)
        .map_err(PresentationAssetError::Invalid)
}
