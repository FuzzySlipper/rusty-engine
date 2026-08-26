use std::collections::{BTreeMap, BTreeSet};

use render_model::{
    mesh_resource_content_hash, validate_mesh_resource_header, TextureDescriptor, TextureFilter,
    TexturePayloadSource, TextureWrap,
};
use serde::Serialize;

use crate::{ProductDevHostError, MAX_BUNDLE_BYTES, MAX_BUNDLE_ENTRIES, MAX_BUNDLE_RESOURCE_BYTES};

/// The generated Product Bundle entry point served at the local origin root.
pub const PRODUCT_DEV_INDEX_PATH: &str = "index.html";
/// The fixed renderer resource descriptor consumed by the Engine browser host.
pub const PRODUCT_DEV_RENDERER_PRELOAD_PATH: &str = "renderer-preload.json";

/// One immutable renderer resource admitted before the development host starts.
///
/// The resource keeps its exact bytes and content-addressed Engine identity. Products
/// do not construct renderer operations from it; presentation owners resolve the
/// identity through the ordinary renderer preload path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDevRendererResource {
    kind: ProductDevRendererResourceKind,
    identity: String,
    content_hash: String,
    path: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductDevRendererResourceKind {
    Texture,
    Mesh,
}

impl ProductDevRendererResource {
    pub fn admit_texture(
        path: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, ProductDevHostError> {
        let path = renderer_path(path.into(), ".png")?;
        let descriptor = TextureDescriptor::admit_png_rgba8_resource(
            "texture/product-dev-preload".to_owned(),
            &bytes,
            TextureFilter::Nearest,
            TextureWrap::Clamp,
            1,
        )
        .map_err(|error| {
            ProductDevHostError::new(
                "DEV_HOST_RENDERER_TEXTURE",
                format!("texture resource is not an admitted PNG: {error:?}"),
            )
        })?;
        let content_hash = descriptor
            .content_hash
            .expect("resource-backed texture has a content hash");
        let identity = match descriptor
            .payload
            .expect("resource-backed texture has a payload")
            .source
        {
            TexturePayloadSource::Resource { resource } => resource,
            TexturePayloadSource::Inline { .. } => {
                unreachable!("PNG admission constructs a resource-backed texture")
            }
        };
        ProductDevBundleEntry::new(path.clone(), "image/png", bytes.clone())?;
        Ok(Self {
            kind: ProductDevRendererResourceKind::Texture,
            identity,
            content_hash,
            path,
            bytes,
        })
    }

    pub fn admit_mesh(
        path: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, ProductDevHostError> {
        let path = renderer_path(path.into(), ".rmesh")?;
        validate_mesh_resource_header(&bytes).map_err(|error| {
            ProductDevHostError::new(
                "DEV_HOST_RENDERER_MESH",
                format!("mesh resource header is invalid: {error:?}"),
            )
        })?;
        let content_hash = mesh_resource_content_hash(&bytes);
        let identity = format!(
            "mesh-resource/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("Engine mesh hash uses SHA-256")
        );
        ProductDevBundleEntry::new(path.clone(), "application/octet-stream", bytes.clone())?;
        Ok(Self {
            kind: ProductDevRendererResourceKind::Mesh,
            identity,
            content_hash,
            path,
            bytes,
        })
    }

    pub const fn kind(&self) -> ProductDevRendererResourceKind {
        self.kind
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn media_type(&self) -> &'static str {
        match self.kind {
            ProductDevRendererResourceKind::Texture => "image/png",
            ProductDevRendererResourceKind::Mesh => "application/octet-stream",
        }
    }

    fn bundle_entry(&self) -> Result<ProductDevBundleEntry, ProductDevHostError> {
        ProductDevBundleEntry::new(self.path.clone(), self.media_type(), self.bytes.clone())
    }
}

/// Encode the fixed browser preload descriptor and its exact immutable resource bodies.
pub fn product_dev_renderer_preload_entries(
    resources: &[ProductDevRendererResource],
) -> Result<Vec<ProductDevBundleEntry>, ProductDevHostError> {
    let mut ordered = resources.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut identities = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for resource in &ordered {
        if !identities.insert(resource.identity.as_str()) || !paths.insert(resource.path.as_str()) {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RENDERER_RESOURCE_DUPLICATE",
                "renderer preload contains a duplicate identity or path",
            ));
        }
    }
    let descriptor = serde_json::to_vec(&RendererPreloadDescriptor {
        artifact: "rusty.product.renderer-preload.v1",
        resources: ordered
            .iter()
            .map(|resource| RendererPreloadResource {
                identity: &resource.identity,
                content_hash: &resource.content_hash,
                media_type: resource.media_type(),
                path: &resource.path,
                byte_length: resource.bytes.len(),
            })
            .collect(),
    })
    .map_err(|error| ProductDevHostError::new("DEV_HOST_RENDERER_DESCRIPTOR", error.to_string()))?;
    let mut entries = Vec::with_capacity(ordered.len() + 1);
    entries.push(ProductDevBundleEntry::new(
        PRODUCT_DEV_RENDERER_PRELOAD_PATH,
        "application/json; charset=utf-8",
        descriptor,
    )?);
    for resource in ordered {
        entries.push(resource.bundle_entry()?);
    }
    Ok(entries)
}

#[derive(Serialize)]
struct RendererPreloadDescriptor<'a> {
    artifact: &'static str,
    resources: Vec<RendererPreloadResource<'a>>,
}

#[derive(Serialize)]
struct RendererPreloadResource<'a> {
    identity: &'a str,
    #[serde(rename = "contentHash")]
    content_hash: &'a str,
    #[serde(rename = "mediaType")]
    media_type: &'static str,
    path: &'a str,
    #[serde(rename = "byteLength")]
    byte_length: usize,
}

fn renderer_path(path: String, extension: &str) -> Result<String, ProductDevHostError> {
    if !path.starts_with("content/") || !path.ends_with(extension) {
        return Err(ProductDevHostError::new(
            "DEV_HOST_RENDERER_RESOURCE_PATH",
            "renderer resource must use its fixed content path and media extension",
        ));
    }
    Ok(path)
}

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
            | "application/octet-stream"
            | "application/wasm"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        product_dev_renderer_preload_entries, ProductDevBundleEntry, ProductDevRendererResource,
        PRODUCT_DEV_RENDERER_PRELOAD_PATH,
    };

    const CHECKER_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 244, 34, 127, 138, 0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207, 0, 68,
        255, 25, 26, 0, 16, 121, 3, 126, 153, 113, 48, 89, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
        130,
    ];

    #[test]
    fn admits_bounded_wav_bundle_bytes_without_opening_a_product_path() {
        let entry =
            ProductDevBundleEntry::new("content/renderer/theme.wav", "audio/wav", vec![0_u8; 44])
                .expect("WAV content type is an admitted immutable bundle resource");
        assert_eq!(entry.path(), "content/renderer/theme.wav");
        assert_eq!(entry.content_type(), "audio/wav");
    }

    #[test]
    fn admits_bounded_packed_mesh_bundle_bytes_with_the_renderer_media_type() {
        let entry = ProductDevBundleEntry::new(
            "content/renderer/packed.rmesh",
            "application/octet-stream",
            vec![0_u8; 16],
        )
        .expect("packed mesh content type is an admitted immutable bundle resource");
        assert_eq!(entry.path(), "content/renderer/packed.rmesh");
        assert_eq!(entry.content_type(), "application/octet-stream");
    }

    #[test]
    fn rejects_media_types_outside_the_fixed_bundle_allowlist() {
        let error = ProductDevBundleEntry::new("content/renderer/theme.ogg", "audio/ogg", vec![1])
            .expect_err("unadmitted media type");
        assert!(error.to_string().contains("DEV_HOST_BUNDLE_CONTENT_TYPE"));
    }

    #[test]
    fn publishes_typed_texture_and_mesh_resources_through_the_fixed_preload_bundle() {
        let texture = ProductDevRendererResource::admit_texture(
            "content/art/actors.png",
            CHECKER_PNG.to_vec(),
        )
        .expect("texture resource");
        let mut mesh_bytes = vec![0_u8; render_model::MESH_RESOURCE_HEADER_BYTES as usize];
        mesh_bytes[..8].copy_from_slice(&render_model::MESH_RESOURCE_MAGIC);
        let mesh_byte_length = mesh_bytes.len() as u32;
        mesh_bytes[8..12].copy_from_slice(&mesh_byte_length.to_le_bytes());
        mesh_bytes[12..16].copy_from_slice(&1_u32.to_le_bytes());
        let mesh = ProductDevRendererResource::admit_mesh(
            "content/meshes/world.rmesh",
            mesh_bytes.clone(),
        )
        .expect("mesh resource");

        let entries = product_dev_renderer_preload_entries(&[texture.clone(), mesh.clone()])
            .expect("renderer preload bundle entries");
        let descriptor = entries
            .iter()
            .find(|entry| entry.path() == PRODUCT_DEV_RENDERER_PRELOAD_PATH)
            .expect("preload descriptor");
        let value: serde_json::Value =
            serde_json::from_slice(descriptor.bytes()).expect("descriptor JSON");
        let resources = value["resources"].as_array().expect("resource array");
        assert_eq!(resources.len(), 2);
        assert!(resources.iter().any(|resource| {
            resource["identity"] == texture.identity()
                && resource["contentHash"] == texture.content_hash()
                && resource["path"] == texture.path()
        }));
        assert!(resources.iter().any(|resource| {
            resource["identity"] == mesh.identity()
                && resource["contentHash"] == mesh.content_hash()
                && resource["path"] == mesh.path()
        }));
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.path() == mesh.path())
                .expect("mesh body")
                .bytes(),
            mesh_bytes
        );
    }
}
