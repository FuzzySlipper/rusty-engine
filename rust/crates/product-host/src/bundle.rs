use std::{collections::BTreeMap, sync::Arc};

use serde::Serialize;
use ts_rs::TS;

use crate::{ProductHostError, ProductHostRenderOutput};

/// The generated Product Bundle entry point served at the local origin root.
pub const PRODUCT_HOST_INDEX_PATH: &str = "index.html";
/// Where the runtime pack's page reads [`ProductHostBrowserBootstrap`].
pub const PRODUCT_HOST_BOOTSTRAP_PATH: &str = "product-bootstrap.json";

/// What the runtime pack's page needs to mount a product: the product host
/// writes it from the product's manifest.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostBrowserBootstrap {
    pub product: ProductHostBootstrapProduct,
    pub ui: ProductHostBootstrapUi,
    pub input: ProductHostBootstrapInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ui_projection: Option<ProductHostBootstrapUiProjection>,
    pub renderer: ProductHostBootstrapRenderer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostBootstrapProduct {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostBootstrapUi {
    /// The product UI module, relative to the page: `product-ui/...`.
    pub entry: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostBootstrapInput {
    pub cursor_mode: ProductHostCursorMode,
}

/// How gameplay holds the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostCursorMode {
    PointerLock,
    Unlocked,
    /// A visible cursor kept inside the game view.
    Confined,
}

/// The product UI projection stream and contract the page admits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostBootstrapUiProjection {
    pub expected_stream: String,
    pub expected_contract: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostBootstrapRenderer {
    /// The page shows the runtime's frames, or lets the desktop window show
    /// through.
    pub output: ProductHostRenderOutput,
    /// The width-to-height range the presentation keeps; the page
    /// letterboxes it beyond. Absent fills the page.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub presentation_aspect: Option<ProductHostPresentationAspect>,
}

/// An inclusive width-to-height range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostPresentationAspect {
    pub minimum: f64,
    pub maximum: f64,
}

/// One pre-admitted immutable browser resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductHostBundleEntry {
    path: String,
    content_type: String,
    bytes: Arc<[u8]>,
}

impl ProductHostBundleEntry {
    pub fn new(
        path: impl Into<String>,
        content_type: impl Into<String>,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Result<Self, ProductHostError> {
        let bytes = bytes.into();
        let content_type = content_type.into();
        let path = servable_path(&path.into())?;
        Ok(Self {
            path,
            content_type,
            bytes,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn shared_bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.bytes)
    }
}

/// Immutable exact bundle bytes admitted before the local server starts.
/// The server never reads product directories or generated artifacts after
/// construction; this prevents runtime source reach-through after relocation.
#[derive(Debug, Clone)]
pub struct ProductHostBundle {
    entries: BTreeMap<String, ProductHostBundleEntry>,
    total_bytes: usize,
}

impl ProductHostBundle {
    pub fn new(entries: Vec<ProductHostBundleEntry>) -> Result<Self, ProductHostError> {
        let total_bytes = entries.iter().map(|entry| entry.bytes.len()).sum();
        let map: BTreeMap<_, _> = entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect();
        if !map.contains_key(PRODUCT_HOST_INDEX_PATH) {
            return Err(ProductHostError::new(
                "PRODUCT_HOST_BUNDLE_INDEX_REQUIRED",
                "bundle must contain index.html",
            ));
        }
        Ok(Self {
            entries: map,
            total_bytes,
        })
    }

    pub(crate) fn get(&self, request_path: &str) -> Option<&ProductHostBundleEntry> {
        let path = if request_path == "/" {
            PRODUCT_HOST_INDEX_PATH
        } else {
            request_path.strip_prefix('/')?
        };
        self.entries.get(path)
    }

    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn entries(&self) -> impl Iterator<Item = &ProductHostBundleEntry> {
        self.entries.values()
    }
}

fn servable_path(value: &str) -> Result<String, ProductHostError> {
    // Bytes a browser sends unescaped in a path, since requests are matched
    // without decoding: any other byte could never be requested.
    if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/' | b'@' | b'+' | b'~')
    }) {
        return Err(ProductHostError::new(
            "PRODUCT_HOST_BUNDLE_PATH",
            format!("bundle path `{value}` has a byte a browser would escape"),
        ));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::ProductHostBundleEntry;

    #[test]
    fn bundle_accepts_large_shared_resources_and_many_entries() {
        let body: std::sync::Arc<[u8]> = vec![7; 64 * 1024 * 1024 + 1].into();
        let mut entries =
            vec![
                ProductHostBundleEntry::new("index.html", "text/html; charset=utf-8", vec![])
                    .unwrap(),
            ];
        for index in 0..4 {
            entries.push(
                ProductHostBundleEntry::new(
                    format!("content/large-{index}.bin"),
                    "application/octet-stream",
                    body.clone(),
                )
                .unwrap(),
            );
        }
        for index in 0..4096 {
            entries.push(
                ProductHostBundleEntry::new(
                    format!("content/small-{index}.bin"),
                    "application/octet-stream",
                    vec![],
                )
                .unwrap(),
            );
        }
        let bundle = super::ProductHostBundle::new(entries).unwrap();
        assert_eq!(bundle.total_bytes(), 4 * body.len());
        let served = bundle.get("/content/large-3.bin").unwrap();
        assert!(std::sync::Arc::ptr_eq(&body, &served.shared_bytes()));
    }
}
