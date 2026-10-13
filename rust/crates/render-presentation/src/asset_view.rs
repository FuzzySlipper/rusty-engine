use std::collections::BTreeMap;

use render_model::RenderAssetKind;

/// Narrow, immutable asset information available to presentation validation.
/// It deliberately supports lookup only: it cannot enumerate, mutate, load, or
/// select project resources.
pub trait PresentationAssetLookup {
    /// The kind of the live asset `id`, if there is one.
    fn presentation_asset_kind(&self, id: &str) -> Option<RenderAssetKind>;
}

impl PresentationAssetLookup for BTreeMap<String, RenderAssetKind> {
    fn presentation_asset_kind(&self, id: &str) -> Option<RenderAssetKind> {
        self.get(id).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationAssetError {
    Missing,
    WrongKind,
}

/// The asset `id` is live and of `kind`. Its identity and content hash come
/// from the Engine that resolved it, so they are not compared again.
pub(crate) fn verify_asset(
    assets: &impl PresentationAssetLookup,
    id: &str,
    kind: RenderAssetKind,
) -> Result<(), PresentationAssetError> {
    match assets.presentation_asset_kind(id) {
        None => Err(PresentationAssetError::Missing),
        Some(actual) if actual != kind => Err(PresentationAssetError::WrongKind),
        Some(_) => Ok(()),
    }
}
