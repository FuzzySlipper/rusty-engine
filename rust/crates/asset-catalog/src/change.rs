use core_assets::AssetId;

use crate::{AssetCatalog, DependencyGraph, MaterialDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    VisualOnly,
    AuthorityImpacting,
    Structural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadSuggestion {
    Reproject,
    RevalidateDependents,
    RequiresFullReload,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangeImpactReport {
    pub asset: AssetId,
    pub change: ChangeKind,
    pub affected_dependents: Vec<AssetId>,
    pub safe: bool,
    pub requires_full_reload: bool,
    pub suggestion: ReloadSuggestion,
}

pub fn revalidate_asset(
    catalog: &AssetCatalog,
    asset: &AssetId,
    change: ChangeKind,
) -> Option<ChangeImpactReport> {
    if !catalog.contains(asset) {
        return None;
    }
    let (safe, requires_full_reload, suggestion) = match change {
        ChangeKind::VisualOnly => (true, false, ReloadSuggestion::Reproject),
        ChangeKind::AuthorityImpacting => (false, false, ReloadSuggestion::RevalidateDependents),
        ChangeKind::Structural => (false, true, ReloadSuggestion::RequiresFullReload),
    };
    Some(ChangeImpactReport {
        asset: asset.clone(),
        change,
        affected_dependents: DependencyGraph::build(catalog).dependents_of(asset),
        safe,
        requires_full_reload,
        suggestion,
    })
}

pub fn classify_material_change(
    before: &MaterialDefinition,
    after: &MaterialDefinition,
) -> ChangeKind {
    if before.authority == after.authority {
        ChangeKind::VisualOnly
    } else {
        ChangeKind::AuthorityImpacting
    }
}

pub fn material_change_impact(
    catalog: &AssetCatalog,
    asset: &AssetId,
    before: &MaterialDefinition,
    after: &MaterialDefinition,
) -> Option<ChangeImpactReport> {
    revalidate_asset(catalog, asset, classify_material_change(before, after))
}
