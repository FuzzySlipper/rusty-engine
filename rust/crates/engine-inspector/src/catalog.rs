use std::collections::BTreeMap;
use std::fmt::Write;

use asset_catalog::{
    decode_catalog, decode_lock, validate_catalog, validate_lock, AssetCatalog, AssetLock,
    LockIssue,
};
use serde::Serialize;

use crate::{
    Diagnostic, DiagnosticDomain, DiagnosticLocation, DiagnosticSet, DiagnosticSeverity,
    RemedyAction,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedCount {
    pub name: String,
    pub count: usize,
}

impl NamedCount {
    pub(crate) fn from_map(counts: BTreeMap<String, usize>) -> Vec<Self> {
        counts
            .into_iter()
            .map(|(name, count)| Self { name, count })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogLockInspection {
    pub entry_count: usize,
    pub finding_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInspection {
    pub entry_count: usize,
    pub dependency_count: usize,
    pub kinds: Vec<NamedCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock: Option<CatalogLockInspection>,
    pub diagnostics: DiagnosticSet,
}

pub fn inspect_catalog(catalog: &AssetCatalog, lock: Option<&AssetLock>) -> CatalogInspection {
    let mut kinds = BTreeMap::new();
    let mut dependency_count = 0;
    for entry in &catalog.entries {
        *kinds.entry(entry.kind().prefix().to_string()).or_insert(0) += 1;
        dependency_count += entry.dependencies.len();
    }

    let mut diagnostics = DiagnosticSet::new();
    diagnostics.extend(
        validate_catalog(catalog)
            .diagnostics()
            .into_iter()
            .map(|source| {
                let action = match source.code.as_str() {
                    "unknown_dependency" => RemedyAction::ProvideAsset,
                    "wrong_kind_reference" => RemedyAction::FixReference,
                    "dependency_cycle" => RemedyAction::BreakCycle,
                    _ => RemedyAction::Inspect,
                };
                Diagnostic::new(
                    DiagnosticDomain::AssetCatalog,
                    DiagnosticSeverity::Error,
                    source.code,
                    DiagnosticLocation::path(source.path),
                    source.message,
                )
                .with_remedy(action, "correct the catalog authoring data")
            }),
    );

    let lock = lock.map(|lock| {
        let report = validate_lock(lock, catalog);
        diagnostics.extend(report.findings.iter().map(lock_diagnostic));
        CatalogLockInspection {
            entry_count: lock.entries.len(),
            finding_count: report.findings.len(),
        }
    });

    CatalogInspection {
        entry_count: catalog.entries.len(),
        dependency_count,
        kinds: NamedCount::from_map(kinds),
        lock,
        diagnostics,
    }
}

pub fn inspect_catalog_json(
    catalog_json: &str,
    lock_json: Option<&str>,
) -> Result<CatalogInspection, DiagnosticSet> {
    let catalog = decode_catalog(catalog_json).map_err(|error| {
        DiagnosticSet::one(
            Diagnostic::new(
                DiagnosticDomain::AssetCatalog,
                DiagnosticSeverity::Fatal,
                "catalog.decode",
                DiagnosticLocation::path(error.path),
                error.message,
            )
            .with_remedy(RemedyAction::FixReference, "fix the stored catalog shape"),
        )
    })?;
    let lock = lock_json.map(decode_lock).transpose().map_err(|error| {
        DiagnosticSet::one(
            Diagnostic::new(
                DiagnosticDomain::AssetCatalog,
                DiagnosticSeverity::Fatal,
                "assetLock.decode",
                DiagnosticLocation::path(error.path),
                error.message,
            )
            .with_remedy(
                RemedyAction::RefreshCache,
                "regenerate or fix the asset lock",
            ),
        )
    })?;
    Ok(inspect_catalog(&catalog, lock.as_ref()))
}

impl CatalogInspection {
    pub fn to_text(&self) -> String {
        let mut output = format!(
            "asset-catalog entries={} dependencies={}\n",
            self.entry_count, self.dependency_count
        );
        let kind_counts = self
            .kinds
            .iter()
            .map(|item| format!("{}={}", item.name, item.count))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(output, "kinds {kind_counts}");
        if let Some(lock) = &self.lock {
            let _ = writeln!(
                output,
                "asset-lock entries={} findings={}",
                lock.entry_count, lock.finding_count
            );
        }
        output.push_str(&self.diagnostics.to_text());
        output
    }
}

fn lock_diagnostic(finding: &asset_catalog::LockFinding) -> Diagnostic {
    let (severity, message, action) = match &finding.issue {
        LockIssue::Missing => (
            DiagnosticSeverity::Error,
            "locked asset is absent from the catalog".to_string(),
            RemedyAction::ProvideAsset,
        ),
        LockIssue::WrongKind { locked, current } => (
            DiagnosticSeverity::Error,
            format!(
                "locked kind is {}, current kind is {}",
                locked.prefix(),
                current.prefix()
            ),
            RemedyAction::FixReference,
        ),
        LockIssue::StaleVersion { locked, current } => (
            DiagnosticSeverity::Warning,
            format!("locked version is {locked}, current version is {current}"),
            RemedyAction::RefreshCache,
        ),
        LockIssue::StaleHash { locked, current } => (
            DiagnosticSeverity::Warning,
            format!("locked hash is {locked:?}, current hash is {current:?}"),
            RemedyAction::RefreshCache,
        ),
        LockIssue::DependencyDrift { added, removed } => (
            DiagnosticSeverity::Warning,
            format!(
                "dependencies changed; added=[{}] removed=[{}]",
                added
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                removed
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            RemedyAction::RefreshCache,
        ),
        LockIssue::NewInCatalog => (
            DiagnosticSeverity::Info,
            "catalog asset is not represented in the lock".to_string(),
            RemedyAction::RefreshCache,
        ),
    };
    Diagnostic::new(
        DiagnosticDomain::AssetCatalog,
        severity,
        format!("assetLock.{}", finding.issue.code()),
        DiagnosticLocation::path(format!("entries[{}]", finding.id.as_str()))
            .with_asset(finding.id.as_str()),
        message,
    )
    .with_remedy(action, "review and regenerate the lock intentionally")
}

#[cfg(test)]
mod tests {
    use asset_catalog::{generate_lock, AssetCatalog, CatalogEntry};
    use core_assets::{AssetId, AssetReference, AssetVersionReq};

    use super::*;

    #[test]
    fn report_counts_kinds_and_classifies_catalog_and_lock_findings() {
        let missing = AssetId::parse("texture/missing").unwrap();
        let mesh =
            CatalogEntry::new(AssetId::parse("mesh/wall").unwrap(), 2).with_dependencies(vec![
                AssetReference::new(missing, AssetVersionReq::Any, None),
            ]);
        let catalog = AssetCatalog::from_entries(vec![mesh]);
        let mut lock = generate_lock(&catalog);
        lock.entries[0].version = 1;

        let report = inspect_catalog(&catalog, Some(&lock));
        assert_eq!(report.entry_count, 1);
        assert_eq!(report.dependency_count, 1);
        assert_eq!(report.kinds[0].name, "mesh");
        assert_eq!(report.diagnostics.count_at(DiagnosticSeverity::Error), 1);
        assert_eq!(report.diagnostics.count_at(DiagnosticSeverity::Warning), 1);
        assert!(report.to_text().contains("unknown_dependency"));
    }
}
