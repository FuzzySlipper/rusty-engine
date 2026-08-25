use std::collections::BTreeMap;

use crate::{ProductDevHostError, MAX_BUNDLE_BYTES, MAX_BUNDLE_ENTRIES, MAX_BUNDLE_RESOURCE_BYTES};

/// The generated Product Bundle entry point served at the local origin root.
pub const PRODUCT_DEV_INDEX_PATH: &str = "index.html";

/// One pre-admitted immutable browser resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDevBundleEntry {
    path: String,
    content_type: &'static str,
    bytes: Vec<u8>,
}

impl ProductDevBundleEntry {
    pub fn new(
        path: impl Into<String>,
        content_type: &'static str,
        bytes: Vec<u8>,
    ) -> Result<Self, ProductDevHostError> {
        let path = normalize_path(&path.into())?;
        if !is_allowed_content_type(content_type) {
            return Err(ProductDevHostError::new(
                "DEV_HOST_BUNDLE_CONTENT_TYPE",
                "bundle resource content type is not admitted",
            ));
        }
        if bytes.len() > MAX_BUNDLE_RESOURCE_BYTES {
            return Err(ProductDevHostError::new(
                "DEV_HOST_BUNDLE_RESOURCE_BOUNDS",
                "bundle resource exceeds the maximum byte length",
            ));
        }
        Ok(Self {
            path,
            content_type,
            bytes,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn content_type(&self) -> &'static str {
        self.content_type
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Immutable exact bundle bytes admitted before the local server starts.
/// The server never reads product directories or generated artifacts after
/// construction; this prevents runtime source reach-through after relocation.
#[derive(Debug, Clone)]
pub struct ProductDevBundle {
    entries: BTreeMap<String, ProductDevBundleEntry>,
    total_bytes: usize,
}

impl ProductDevBundle {
    pub fn new(entries: Vec<ProductDevBundleEntry>) -> Result<Self, ProductDevHostError> {
        if entries.is_empty() || entries.len() > MAX_BUNDLE_ENTRIES {
            return Err(ProductDevHostError::new(
                "DEV_HOST_BUNDLE_ENTRY_BOUNDS",
                "bundle must contain between one and 4096 resources",
            ));
        }
        let mut map = BTreeMap::new();
        let mut total_bytes = 0_usize;
        for entry in entries {
            total_bytes = total_bytes.checked_add(entry.bytes.len()).ok_or_else(|| {
                ProductDevHostError::new("DEV_HOST_BUNDLE_BOUNDS", "bundle byte total overflowed")
            })?;
            if total_bytes > MAX_BUNDLE_BYTES {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_BUNDLE_BOUNDS",
                    "bundle exceeds the maximum aggregate byte length",
                ));
            }
            if map.insert(entry.path.clone(), entry).is_some() {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_BUNDLE_DUPLICATE",
                    "bundle contains duplicate normalized paths",
                ));
            }
        }
        if !map.contains_key(PRODUCT_DEV_INDEX_PATH) {
            return Err(ProductDevHostError::new(
                "DEV_HOST_BUNDLE_INDEX_REQUIRED",
                "bundle must contain index.html",
            ));
        }
        Ok(Self {
            entries: map,
            total_bytes,
        })
    }

    pub(crate) fn get(&self, request_path: &str) -> Option<&ProductDevBundleEntry> {
        let path = if request_path == "/" {
            PRODUCT_DEV_INDEX_PATH
        } else {
            request_path.strip_prefix('/')?
        };
        self.entries.get(path)
    }

    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn entries(&self) -> impl Iterator<Item = &ProductDevBundleEntry> {
        self.entries.values()
    }
}

fn normalize_path(value: &str) -> Result<String, ProductDevHostError> {
    if value.is_empty()
        || value.len() > 512
        || value.starts_with('/')
        || value.contains('\\')
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
    {
        return Err(ProductDevHostError::new(
            "DEV_HOST_BUNDLE_PATH",
            "bundle path must be a bounded normalized relative ASCII path",
        ));
    }
    Ok(value.to_owned())
}

fn is_allowed_content_type(value: &str) -> bool {
    matches!(
        value,
        "text/html; charset=utf-8"
            | "text/javascript; charset=utf-8"
            | "text/css; charset=utf-8"
            | "application/json; charset=utf-8"
            | "image/svg+xml"
            | "image/png"
            | "image/jpeg"
            | "audio/wav"
            | "application/wasm"
    )
}

#[cfg(test)]
mod tests {
    use super::ProductDevBundleEntry;

    #[test]
    fn admits_bounded_wav_bundle_bytes_without_opening_a_product_path() {
        let entry =
            ProductDevBundleEntry::new("content/renderer/theme.wav", "audio/wav", vec![0_u8; 44])
                .expect("WAV content type is an admitted immutable bundle resource");
        assert_eq!(entry.path(), "content/renderer/theme.wav");
        assert_eq!(entry.content_type(), "audio/wav");
    }

    #[test]
    fn rejects_media_types_outside_the_fixed_bundle_allowlist() {
        let error = ProductDevBundleEntry::new("content/renderer/theme.ogg", "audio/ogg", vec![1])
            .expect_err("unadmitted media type");
        assert!(error.to_string().contains("DEV_HOST_BUNDLE_CONTENT_TYPE"));
    }
}
