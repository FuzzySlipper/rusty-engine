use std::collections::BTreeSet;

use core_assets::{AssetHash, AssetId, AssetKind};

use crate::{AssetCatalog, CatalogEntry};

#[derive(Debug, Clone, PartialEq)]
pub struct AssetLockEntry {
    pub id: AssetId,
    pub kind: AssetKind,
    pub version: u32,
    pub hash: Option<AssetHash>,
    pub dependencies: Vec<AssetId>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssetLock {
    pub entries: Vec<AssetLockEntry>,
}

pub fn generate_lock(catalog: &AssetCatalog) -> AssetLock {
    let mut entries: Vec<_> = catalog
        .entries
        .iter()
        .map(|entry| {
            let mut dependencies: Vec<_> = entry
                .dependencies
                .iter()
                .map(|dependency| dependency.id().clone())
                .collect();
            dependencies.sort_by(|left, right| left.as_str().cmp(right.as_str()));
            dependencies.dedup();
            AssetLockEntry {
                id: entry.id.clone(),
                kind: entry.kind(),
                version: entry.version,
                hash: entry.hash.clone(),
                dependencies,
            }
        })
        .collect();
    entries.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    AssetLock { entries }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LockIssue {
    Missing,
    WrongKind {
        locked: AssetKind,
        current: AssetKind,
    },
    StaleVersion {
        locked: u32,
        current: u32,
    },
    StaleHash {
        locked: Option<AssetHash>,
        current: Option<AssetHash>,
    },
    DependencyDrift {
        added: Vec<AssetId>,
        removed: Vec<AssetId>,
    },
    NewInCatalog,
}

impl LockIssue {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::WrongKind { .. } => "wrong_kind",
            Self::StaleVersion { .. } => "stale_version",
            Self::StaleHash { .. } => "stale_hash",
            Self::DependencyDrift { .. } => "dependency_drift",
            Self::NewInCatalog => "new_in_catalog",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LockFinding {
    pub id: AssetId,
    pub issue: LockIssue,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LockValidationReport {
    pub findings: Vec<LockFinding>,
}

impl LockValidationReport {
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

pub fn validate_lock(lock: &AssetLock, catalog: &AssetCatalog) -> LockValidationReport {
    let mut findings = Vec::new();
    let locked_ids: BTreeSet<&str> = lock.entries.iter().map(|entry| entry.id.as_str()).collect();
    for locked in &lock.entries {
        let Some(current) = catalog.get(&locked.id) else {
            findings.push(LockFinding {
                id: locked.id.clone(),
                issue: LockIssue::Missing,
            });
            continue;
        };
        if current.kind() != locked.kind {
            findings.push(LockFinding {
                id: locked.id.clone(),
                issue: LockIssue::WrongKind {
                    locked: locked.kind,
                    current: current.kind(),
                },
            });
            continue;
        }
        if current.version != locked.version {
            findings.push(LockFinding {
                id: locked.id.clone(),
                issue: LockIssue::StaleVersion {
                    locked: locked.version,
                    current: current.version,
                },
            });
        }
        if current.hash != locked.hash {
            findings.push(LockFinding {
                id: locked.id.clone(),
                issue: LockIssue::StaleHash {
                    locked: locked.hash.clone(),
                    current: current.hash.clone(),
                },
            });
        }
        let (added, removed) = dependency_drift(locked, current);
        if !added.is_empty() || !removed.is_empty() {
            findings.push(LockFinding {
                id: locked.id.clone(),
                issue: LockIssue::DependencyDrift { added, removed },
            });
        }
    }

    let mut new_entries: Vec<_> = catalog
        .entries
        .iter()
        .filter(|entry| !locked_ids.contains(entry.id.as_str()))
        .collect();
    new_entries.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    findings.extend(new_entries.into_iter().map(|entry| LockFinding {
        id: entry.id.clone(),
        issue: LockIssue::NewInCatalog,
    }));
    LockValidationReport { findings }
}

fn dependency_drift(
    locked: &AssetLockEntry,
    current: &CatalogEntry,
) -> (Vec<AssetId>, Vec<AssetId>) {
    let locked_ids: BTreeSet<&str> = locked.dependencies.iter().map(AssetId::as_str).collect();
    let current_ids: BTreeSet<&str> = current
        .dependencies
        .iter()
        .map(|dependency| dependency.id().as_str())
        .collect();

    let mut added: Vec<_> = current
        .dependencies
        .iter()
        .filter(|dependency| !locked_ids.contains(dependency.id().as_str()))
        .map(|dependency| dependency.id().clone())
        .collect();
    added.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    added.dedup();
    let removed = locked
        .dependencies
        .iter()
        .filter(|dependency| !current_ids.contains(dependency.as_str()))
        .cloned()
        .collect();
    (added, removed)
}
