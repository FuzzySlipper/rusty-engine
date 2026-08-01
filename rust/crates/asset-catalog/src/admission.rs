use crate::{
    decode_catalog, encode_catalog, validate_catalog, AssetCatalog, AssetCatalogCodecError,
    CatalogValidationReport,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedAssetCatalog {
    catalog: AssetCatalog,
    canonical_json: String,
    canonical_hash: String,
}

impl AdmittedAssetCatalog {
    pub fn admit(catalog: AssetCatalog) -> Result<Self, CatalogAdmissionError> {
        let catalog = catalog.canonical();
        let report = validate_catalog(&catalog);
        if !report.is_ok() {
            return Err(CatalogAdmissionError::Validation(report));
        }
        let canonical_json = encode_catalog(&catalog).map_err(CatalogAdmissionError::Codec)?;
        let canonical_hash = format!("sha256:{:x}", Sha256::digest(canonical_json.as_bytes()));
        Ok(Self {
            catalog,
            canonical_json,
            canonical_hash,
        })
    }

    pub fn reopen(input: &str) -> Result<Self, CatalogAdmissionError> {
        let catalog = decode_catalog(input).map_err(CatalogAdmissionError::Codec)?;
        Self::admit(catalog)
    }

    pub fn catalog(&self) -> &AssetCatalog {
        &self.catalog
    }

    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }

    pub fn canonical_hash(&self) -> &str {
        &self.canonical_hash
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssetCatalogStore {
    current: AdmittedAssetCatalog,
    revision: u64,
}

impl AssetCatalogStore {
    pub fn new(current: AdmittedAssetCatalog) -> Self {
        Self {
            current,
            revision: 1,
        }
    }

    pub fn current(&self) -> &AdmittedAssetCatalog {
        &self.current
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn replace(&mut self, candidate: AssetCatalog) -> Result<u64, CatalogAdmissionError> {
        let candidate = AdmittedAssetCatalog::admit(candidate)?;
        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or(CatalogAdmissionError::RevisionExhausted)?;
        self.current = candidate;
        self.revision = next_revision;
        Ok(next_revision)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogAdmissionError {
    Codec(AssetCatalogCodecError),
    Validation(CatalogValidationReport),
    RevisionExhausted,
}
