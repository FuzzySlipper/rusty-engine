use product_model::{validate_product_identity, ProductPath};
use serde::{Deserialize, Serialize};

use crate::error::ProductAssemblyError;

/// Stable marker for the current Product Assembly artifact family. There is
/// deliberately no numeric schema/version field; the admitted fields and
/// exact bytes define compatibility.
pub const PRODUCT_ASSEMBLY_ARTIFACT: &str = "rusty.product.assembly";

/// The closure roles retained in an assembly receipt. These roles are
/// intentionally semantic and versionless so stale generated browser/source
/// templates are visible as ordinary hash changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssemblyEntryKind {
    AuthoredSource,
    CompiledComposition,
    RuntimeContent,
    ExecutableWorkspace,
    BrowserBundle,
}

impl AssemblyEntryKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthoredSource => "authored-source",
            Self::CompiledComposition => "compiled-composition",
            Self::RuntimeContent => "runtime-content",
            Self::ExecutableWorkspace => "executable-workspace",
            Self::BrowserBundle => "browser-bundle",
        }
    }
}

/// One exact product-relative file in an assembly closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssemblyClosureEntry {
    kind: AssemblyEntryKind,
    path: String,
    bytes: usize,
    sha256: String,
}

impl AssemblyClosureEntry {
    pub(crate) fn new(kind: AssemblyEntryKind, path: impl Into<String>, bytes: &[u8]) -> Self {
        Self {
            kind,
            path: path.into(),
            bytes: bytes.len(),
            sha256: sha256_hex(bytes),
        }
    }

    pub const fn kind(&self) -> AssemblyEntryKind {
        self.kind
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn byte_length(&self) -> usize {
        self.bytes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Generated source/workspace file metadata retained for callers that want
/// to inspect only the executable plan.
pub type GeneratedAssemblyFile = AssemblyClosureEntry;

/// Deterministic, versionless Product Assembly receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssemblyReceipt {
    artifact: String,
    product: String,
    entries: Vec<AssemblyClosureEntry>,
}

impl AssemblyReceipt {
    pub(crate) fn new(
        product: impl Into<String>,
        mut entries: Vec<AssemblyClosureEntry>,
    ) -> Result<Self, ProductAssemblyError> {
        entries.sort_by(|left, right| {
            (left.kind, left.path.as_str()).cmp(&(right.kind, right.path.as_str()))
        });
        for pair in entries.windows(2) {
            if pair[0].kind == pair[1].kind && pair[0].path == pair[1].path {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_DUPLICATE_RECEIPT_ENTRY",
                    pair[0].path.clone(),
                    "one closure role may contain each product-relative path only once",
                ));
            }
        }
        let receipt = Self {
            artifact: PRODUCT_ASSEMBLY_ARTIFACT.to_owned(),
            product: product.into(),
            entries,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn artifact(&self) -> &str {
        &self.artifact
    }

    pub fn product(&self) -> &str {
        &self.product
    }

    pub fn entries(&self) -> &[AssemblyClosureEntry] {
        &self.entries
    }

    pub(crate) fn validate(&self) -> Result<(), ProductAssemblyError> {
        if self.artifact != PRODUCT_ASSEMBLY_ARTIFACT {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_RECEIPT_ARTIFACT",
                "artifact",
                "unsupported Product Assembly receipt artifact",
            ));
        }
        validate_product_identity(&self.product).map_err(|error| {
            ProductAssemblyError::new("ASSEMBLY_RECEIPT_PRODUCT", "product", error.to_string())
        })?;
        if self.entries.is_empty() || self.entries.len() > crate::MAX_GENERATED_FILES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_RECEIPT_ENTRY_COUNT",
                "entries",
                format!(
                    "receipt entries must contain 1..={} files",
                    crate::MAX_GENERATED_FILES
                ),
            ));
        }
        let mut total = 0usize;
        for (index, entry) in self.entries.iter().enumerate() {
            ProductPath::parse(entry.path.clone()).map_err(|error| {
                ProductAssemblyError::new(
                    "ASSEMBLY_RECEIPT_PATH",
                    format!("entries[{index}].path"),
                    error.to_string(),
                )
            })?;
            if entry.sha256.len() != 64
                || !entry
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_RECEIPT_HASH",
                    format!("entries[{index}].sha256"),
                    "receipt hashes must be lowercase 64-character SHA-256 hex",
                ));
            }
            total = total.checked_add(entry.bytes).ok_or_else(|| {
                ProductAssemblyError::new(
                    "ASSEMBLY_RECEIPT_BYTES",
                    "entries",
                    "receipt byte accounting overflowed",
                )
            })?;
            if entry.bytes > crate::MAX_ASSEMBLY_FILE_BYTES {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_RECEIPT_BYTES",
                    entry.path.clone(),
                    format!(
                        "receipt file bytes exceed {}",
                        crate::MAX_ASSEMBLY_FILE_BYTES
                    ),
                ));
            }
            if index > 0 {
                let previous = &self.entries[index - 1];
                if (previous.kind, previous.path.as_str()) >= (entry.kind, entry.path.as_str()) {
                    return Err(ProductAssemblyError::new(
                        "ASSEMBLY_RECEIPT_ORDER",
                        format!("entries[{index}].path"),
                        "receipt entries must be strictly sorted by role then path",
                    ));
                }
            }
            if !entry.path.starts_with("generated/")
                && entry.kind != AssemblyEntryKind::AuthoredSource
            {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_RECEIPT_PATH",
                    entry.path.clone(),
                    "generated closure roles must remain below generated/",
                ));
            }
            // assembly.json is copied after the receipt is encoded. It is
            // explicitly self-excluded to avoid recursive self-hashing.
            if matches!(
                entry.kind,
                AssemblyEntryKind::ExecutableWorkspace | AssemblyEntryKind::BrowserBundle
            ) && entry.path.ends_with("/assembly.json")
            {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_RECEIPT_SELF_REFERENCE",
                    entry.path.clone(),
                    "assembly.json is self-excluded from closure entries",
                ));
            }
        }
        if total > crate::MAX_ASSEMBLY_TOTAL_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_RECEIPT_BYTES",
                "entries",
                format!(
                    "receipt closure bytes exceed {}",
                    crate::MAX_ASSEMBLY_TOTAL_BYTES
                ),
            ));
        }
        Ok(())
    }

    /// Pretty JSON plus one trailing newline. Serde struct field order and
    /// already-sorted entries make this byte-stable across source reorder.
    pub fn json_bytes(&self) -> Result<Vec<u8>, ProductAssemblyError> {
        self.validate()?;
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_RECEIPT_SERIALIZE",
                "assembly.json",
                error.to_string(),
            )
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn json(&self) -> Result<String, ProductAssemblyError> {
        String::from_utf8(self.json_bytes()?).map_err(|error| {
            ProductAssemblyError::new("ASSEMBLY_RECEIPT_UTF8", "assembly.json", error.to_string())
        })
    }
}

/// Explicit alias for integrations that want the wire/serialization name.
pub type AssemblyReceiptJson = AssemblyReceipt;

/// Strictly decodes one receipt and rejects unknown fields, unsupported
/// artifact identity, invalid product-relative paths, malformed hashes,
/// duplicate entries, and nondeterministic ordering.
pub fn decode_assembly_receipt(bytes: &[u8]) -> Result<AssemblyReceipt, ProductAssemblyError> {
    if bytes.len() > crate::MAX_ASSEMBLY_FILE_BYTES {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RECEIPT_BYTES",
            "assembly.json",
            "receipt exceeds its bounded file size",
        ));
    }
    let receipt: AssemblyReceipt = serde_json::from_slice(bytes).map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_RECEIPT_DECODE",
            "assembly.json",
            error.to_string(),
        )
    })?;
    receipt.validate()?;
    Ok(receipt)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(bytes);
    let mut result = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(result, "{byte:02x}");
    }
    result
}
