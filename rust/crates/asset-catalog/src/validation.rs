use std::collections::BTreeSet;

use core_assets::{AssetId, AssetKind};
use serde::Serialize;

use crate::{AssetCatalog, DependencyGraph};

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogValidationError {
    DuplicateAssetId {
        id: AssetId,
    },
    MaterialPayloadMissing {
        id: AssetId,
    },
    MaterialPayloadOnNonMaterial {
        id: AssetId,
        kind: AssetKind,
    },
    WrongKindReference {
        from: AssetId,
        slot: &'static str,
        expected: AssetKind,
        actual: AssetKind,
        reference: AssetId,
    },
    UnknownDependency {
        from: AssetId,
        dependency: AssetId,
    },
    DependencyCycle {
        path: Vec<AssetId>,
    },
    EmptySourcePath {
        id: AssetId,
    },
}

impl CatalogValidationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::DuplicateAssetId { .. } => "duplicate_asset_id",
            Self::MaterialPayloadMissing { .. } => "material_payload_missing",
            Self::MaterialPayloadOnNonMaterial { .. } => "material_payload_on_non_material",
            Self::WrongKindReference { .. } => "wrong_kind_reference",
            Self::UnknownDependency { .. } => "unknown_dependency",
            Self::DependencyCycle { .. } => "dependency_cycle",
            Self::EmptySourcePath { .. } => "empty_source_path",
        }
    }

    pub fn diagnostic(&self) -> CatalogDiagnostic {
        let (path, message) = match self {
            Self::DuplicateAssetId { id } => (
                format!("entries[{}]", id.as_str()),
                format!("asset id `{}` occurs more than once", id.as_str()),
            ),
            Self::MaterialPayloadMissing { id } => (
                format!("entries[{}].material", id.as_str()),
                "material asset has no material definition".to_string(),
            ),
            Self::MaterialPayloadOnNonMaterial { id, kind } => (
                format!("entries[{}].material", id.as_str()),
                format!("{} assets cannot carry material definitions", kind.prefix()),
            ),
            Self::WrongKindReference {
                from,
                slot,
                expected,
                actual,
                reference,
            } => (
                format!("entries[{}].{slot}", from.as_str()),
                format!(
                    "reference `{}` is {}, expected {}",
                    reference.as_str(),
                    actual.prefix(),
                    expected.prefix()
                ),
            ),
            Self::UnknownDependency { from, dependency } => (
                format!("entries[{}].dependencies", from.as_str()),
                format!("dependency `{}` is absent", dependency.as_str()),
            ),
            Self::DependencyCycle { path } => (
                "entries".to_string(),
                format!(
                    "dependency cycle: {}",
                    path.iter()
                        .map(AssetId::as_str)
                        .collect::<Vec<_>>()
                        .join(" -> ")
                ),
            ),
            Self::EmptySourcePath { id } => (
                format!("entries[{}].sourcePath", id.as_str()),
                "source path is empty".to_string(),
            ),
        };
        CatalogDiagnostic {
            code: self.code().to_string(),
            path,
            message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogValidationReport {
    pub errors: Vec<CatalogValidationError>,
}

impl CatalogValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn diagnostics(&self) -> Vec<CatalogDiagnostic> {
        self.errors
            .iter()
            .map(CatalogValidationError::diagnostic)
            .collect()
    }
}

pub fn validate_catalog(catalog: &AssetCatalog) -> CatalogValidationReport {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    let mut reported = BTreeSet::new();
    for entry in &catalog.entries {
        if !seen.insert(entry.id.as_str()) && reported.insert(entry.id.as_str()) {
            errors.push(CatalogValidationError::DuplicateAssetId {
                id: entry.id.clone(),
            });
        }
    }

    for entry in &catalog.entries {
        match (entry.kind(), &entry.material) {
            (AssetKind::Material, None) => {
                errors.push(CatalogValidationError::MaterialPayloadMissing {
                    id: entry.id.clone(),
                });
            }
            (kind, Some(_)) if kind != AssetKind::Material => {
                errors.push(CatalogValidationError::MaterialPayloadOnNonMaterial {
                    id: entry.id.clone(),
                    kind,
                });
            }
            _ => {}
        }

        if let Some(texture) = entry
            .material
            .as_ref()
            .and_then(|material| material.style.texture.as_ref())
        {
            if texture.kind() != AssetKind::Texture {
                errors.push(CatalogValidationError::WrongKindReference {
                    from: entry.id.clone(),
                    slot: "material.style.texture",
                    expected: AssetKind::Texture,
                    actual: texture.kind(),
                    reference: texture.id().clone(),
                });
            }
        }

        if entry.source_path.as_deref() == Some("") {
            errors.push(CatalogValidationError::EmptySourcePath {
                id: entry.id.clone(),
            });
        }
        for dependency in &entry.dependencies {
            if !catalog.contains(dependency.id()) {
                errors.push(CatalogValidationError::UnknownDependency {
                    from: entry.id.clone(),
                    dependency: dependency.id().clone(),
                });
            }
        }
    }

    if let Some(path) = DependencyGraph::build(catalog).detect_cycle() {
        errors.push(CatalogValidationError::DependencyCycle { path });
    }
    CatalogValidationReport { errors }
}
