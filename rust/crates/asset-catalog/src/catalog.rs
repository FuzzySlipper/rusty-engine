use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};

use crate::MaterialDefinition;

/// One authored asset definition.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub id: AssetId,
    pub version: u32,
    pub hash: Option<AssetHash>,
    /// Source location is metadata, never stable identity.
    pub source_path: Option<String>,
    pub label: Option<String>,
    pub dependencies: Vec<AssetReference>,
    /// Required for material IDs and rejected for every other asset kind.
    pub material: Option<MaterialDefinition>,
}

impl CatalogEntry {
    pub fn new(id: AssetId, version: u32) -> Self {
        Self {
            id,
            version,
            hash: None,
            source_path: None,
            label: None,
            dependencies: Vec::new(),
            material: None,
        }
    }

    pub fn kind(&self) -> AssetKind {
        self.id.kind()
    }

    pub fn with_hash(mut self, hash: AssetHash) -> Self {
        self.hash = Some(hash);
        self
    }

    pub fn with_source(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<AssetReference>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn with_material(mut self, material: MaterialDefinition) -> Self {
        self.material = Some(material);
        self
    }
}

/// Authored asset definitions. Construction is intentionally separate from
/// validation so decoders can return complete classified reports.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssetCatalog {
    pub entries: Vec<CatalogEntry>,
}

impl AssetCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_entries(entries: Vec<CatalogEntry>) -> Self {
        Self { entries }
    }

    pub fn get(&self, id: &AssetId) -> Option<&CatalogEntry> {
        self.entries.iter().find(|entry| &entry.id == id)
    }

    pub fn contains(&self, id: &AssetId) -> bool {
        self.get(id).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = &CatalogEntry> {
        self.entries.iter()
    }

    /// A deterministic copy. Entry identity controls order; dependency order is
    /// normalized without changing authored multiplicity so validation can still
    /// report the original semantic content.
    pub fn canonical(&self) -> Self {
        let mut catalog = self.clone();
        catalog
            .entries
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        for entry in &mut catalog.entries {
            entry.dependencies.sort_by(|left, right| {
                left.id()
                    .as_str()
                    .cmp(right.id().as_str())
                    .then_with(|| version_key(left.version()).cmp(&version_key(right.version())))
                    .then_with(|| {
                        left.hash()
                            .map(AssetHash::as_str)
                            .cmp(&right.hash().map(AssetHash::as_str))
                    })
            });
        }
        catalog
    }
}

fn version_key(requirement: AssetVersionReq) -> (u8, u32) {
    match requirement {
        AssetVersionReq::Any => (0, 0),
        AssetVersionReq::Exact(version) => (1, version),
        AssetVersionReq::AtLeast(version) => (2, version),
    }
}
