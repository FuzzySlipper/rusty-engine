use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use gltf::{buffer::Source as BufferSource, image::Source as ImageSource};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{ImportCode, ImportDiagnostic, MAX_SOURCE_BYTES};

pub const MAX_GLTF_RESOURCE_COUNT: usize = 256;
pub const MAX_GLTF_RESOURCE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_GLTF_TOTAL_RESOURCE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfResource {
    /// Canonical project-relative URI, without a scheme, query, or fragment.
    pub uri: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfSourceClosure {
    pub root_json: Vec<u8>,
    pub resources: Vec<GltfResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbSourceClosure {
    pub root_glb: Vec<u8>,
    pub resources: Vec<GltfResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedGltfSource {
    pub glb_bytes: Vec<u8>,
    /// SHA-256 over the root bytes and sorted canonical external resources.
    pub source_hash: String,
    pub source_byte_count: u64,
    pub external_resource_uris: Vec<String>,
}

/// Returns the canonical relative resources a filesystem-owning adapter must load.
/// Data URIs stay inside the root document and are deliberately omitted.
pub fn gltf_relative_resource_uris(root_json: &[u8]) -> Result<Vec<String>, ImportDiagnostic> {
    let parsed = parse_root(root_json)?;
    relative_resource_uris(&parsed)
}

/// Returns the canonical relative resources referenced by a binary GLB root.
/// Its embedded BIN chunk remains part of the root and is not returned here.
pub fn glb_relative_resource_uris(root_glb: &[u8]) -> Result<Vec<String>, ImportDiagnostic> {
    let parsed = parse_glb_root(root_glb)?;
    relative_resource_uris(&parsed)
}

fn relative_resource_uris(parsed: &gltf::Gltf) -> Result<Vec<String>, ImportDiagnostic> {
    let mut authored_by_canonical = BTreeMap::<String, String>::new();
    for (path, uri) in document_resource_uris(parsed) {
        if uri.starts_with("data:") {
            continue;
        }
        let canonical = canonical_resource_uri(uri, &path)?;
        if let Some(previous) = authored_by_canonical.insert(canonical.clone(), uri.to_owned()) {
            if previous != uri {
                return Err(error(
                    ImportCode::MalformedSource,
                    path,
                    format!(
                        "resource URI `{uri}` collides canonically with `{previous}` as `{canonical}`"
                    ),
                    "use one unambiguous relative URI spelling for each resource",
                ));
            }
        }
    }
    Ok(authored_by_canonical.into_keys().collect())
}

/// Validates a complete immutable glTF source closure and deterministically packs
/// it into the Engine's existing self-contained GLB runtime contract.
pub fn admit_gltf_source(source: &GltfSourceClosure) -> Result<PackedGltfSource, ImportDiagnostic> {
    let parsed = parse_root(&source.root_json)?;
    let expected_uris = gltf_relative_resource_uris(&source.root_json)?;
    let resources = validate_resources(&source.resources)?;
    let supplied_uris = resources.keys().cloned().collect::<Vec<_>>();
    if expected_uris != supplied_uris {
        let missing = expected_uris
            .iter()
            .find(|uri| !resources.contains_key(*uri));
        let extra = supplied_uris
            .iter()
            .find(|uri| !expected_uris.contains(uri));
        let (path, message) = if let Some(uri) = missing {
            (
                "source.resources",
                format!("referenced resource `{uri}` is missing"),
            )
        } else {
            (
                "source.resources",
                format!(
                    "resource `{}` is not referenced by the glTF document",
                    extra.expect("unequal sets have a difference")
                ),
            )
        };
        return Err(error(
            ImportCode::ExternalResource,
            path,
            message,
            "provide exactly the bounded resource closure referenced by the root document",
        ));
    }

    let mut root: Value = serde_json::from_slice(&source.root_json).map_err(|failure| {
        error(
            ImportCode::InvalidContainer,
            "source",
            format!("invalid glTF JSON: {failure}"),
            "export a valid glTF 2.0 JSON document",
        )
    })?;
    let root_object = root.as_object_mut().ok_or_else(|| {
        error(
            ImportCode::InvalidContainer,
            "source",
            "glTF root must be a JSON object",
            "export a valid glTF 2.0 JSON document",
        )
    })?;

    let mut packed_bin = Vec::new();
    let mut buffer_offsets = Vec::new();
    for buffer in parsed.document.buffers() {
        align_four(&mut packed_bin);
        let bytes = match buffer.source() {
            BufferSource::Bin => {
                return Err(error(
                    ImportCode::InvalidContainer,
                    format!("source.buffers[{}]", buffer.index()),
                    "JSON glTF cannot refer to an embedded GLB BIN chunk",
                    "use a relative or bounded data URI for each JSON glTF buffer",
                ));
            }
            BufferSource::Uri(uri) => resolve_uri_bytes(
                uri,
                &format!("source.buffers[{}].uri", buffer.index()),
                &resources,
                DataKind::Buffer,
            )?,
        };
        if bytes.len() != buffer.length() {
            return Err(error(
                ImportCode::InvalidContainer,
                format!("source.buffers[{}].byteLength", buffer.index()),
                format!(
                    "declared byteLength {} does not match resolved byte count {}",
                    buffer.length(),
                    bytes.len()
                ),
                "repair the buffer byteLength or referenced resource bytes",
            ));
        }
        buffer_offsets.push(packed_bin.len());
        packed_bin.extend_from_slice(&bytes);
    }

    rewrite_buffer_views(root_object, &buffer_offsets)?;
    embed_uri_images(root_object, &parsed, &resources, &mut packed_bin)?;
    let packed_length = packed_bin.len();
    root_object.insert(
        "buffers".to_owned(),
        Value::Array(vec![Value::Object(Map::from_iter([(
            "byteLength".to_owned(),
            Value::from(packed_length as u64),
        )]))]),
    );

    let json = serde_json::to_vec(&root).map_err(|failure| {
        error(
            ImportCode::InvalidContainer,
            "source",
            format!("canonical glTF JSON could not be encoded: {failure}"),
            "repair the source document",
        )
    })?;
    let glb_bytes = encode_glb(json, packed_bin)?;
    let total = closure_byte_count(source)?;
    Ok(PackedGltfSource {
        glb_bytes,
        source_hash: closure_hash(&source.root_json, &resources),
        source_byte_count: total,
        external_resource_uris: expected_uris,
    })
}

/// Validates a complete immutable binary GLB closure and packs its embedded
/// BIN chunk plus any bounded relative buffers/images into one self-contained
/// GLB for the ordinary renderer resource path.
pub fn admit_glb_source(source: &GlbSourceClosure) -> Result<PackedGltfSource, ImportDiagnostic> {
    let parsed = parse_glb_root(&source.root_glb)?;
    let expected_uris = glb_relative_resource_uris(&source.root_glb)?;
    let resources = validate_resources(&source.resources)?;
    require_exact_resources(&expected_uris, &resources)?;

    let mut root = glb_json_document(&source.root_glb, "source")?;
    let root_object = root.as_object_mut().ok_or_else(|| {
        error(
            ImportCode::InvalidContainer,
            "source",
            "GLB JSON root must be an object",
            "export a valid binary glTF 2.0 document",
        )
    })?;

    let embedded = parsed.blob.clone().ok_or_else(|| {
        error(
            ImportCode::InvalidContainer,
            "source",
            "binary GLB closure requires one embedded BIN chunk",
            "embed the primary buffer in the binary glTF root",
        )
    })?;
    let mut embedded_seen = false;
    let mut packed_bin = Vec::new();
    let mut buffer_offsets = Vec::new();
    for buffer in parsed.document.buffers() {
        align_four(&mut packed_bin);
        let bytes = match buffer.source() {
            BufferSource::Bin if !embedded_seen => {
                embedded_seen = true;
                embedded.clone()
            }
            BufferSource::Bin => {
                return Err(error(
                    ImportCode::InvalidContainer,
                    format!("source.buffers[{}]", buffer.index()),
                    "binary GLB closure has more than one embedded buffer",
                    "retain one embedded BIN buffer and use relative resources for any others",
                ));
            }
            BufferSource::Uri(uri) => resolve_uri_bytes(
                uri,
                &format!("source.buffers[{}].uri", buffer.index()),
                &resources,
                DataKind::Buffer,
            )?,
        };
        if matches!(buffer.source(), BufferSource::Uri(_)) && bytes.len() != buffer.length() {
            return Err(error(
                ImportCode::InvalidContainer,
                format!("source.buffers[{}].byteLength", buffer.index()),
                format!(
                    "declared byteLength {} does not match resolved byte count {}",
                    buffer.length(),
                    bytes.len()
                ),
                "repair the buffer byteLength or referenced resource bytes",
            ));
        }
        buffer_offsets.push(packed_bin.len());
        packed_bin.extend_from_slice(&bytes);
    }
    if !embedded_seen {
        return Err(error(
            ImportCode::InvalidContainer,
            "source.buffers",
            "binary GLB closure did not reference its embedded BIN chunk",
            "retain the primary embedded GLB buffer",
        ));
    }

    rewrite_buffer_views(root_object, &buffer_offsets)?;
    embed_uri_images(root_object, &parsed, &resources, &mut packed_bin)?;
    root_object.insert(
        "buffers".to_owned(),
        Value::Array(vec![Value::Object(Map::from_iter([(
            "byteLength".to_owned(),
            Value::from(packed_bin.len() as u64),
        )]))]),
    );
    let json = serde_json::to_vec(&root).map_err(|failure| {
        error(
            ImportCode::InvalidContainer,
            "source",
            format!("canonical GLB JSON could not be encoded: {failure}"),
            "repair the source document",
        )
    })?;
    let glb_bytes = encode_glb(json, packed_bin)?;
    let total = closure_byte_count_parts(source.root_glb.len(), &source.resources)?;
    Ok(PackedGltfSource {
        glb_bytes,
        source_hash: closure_hash(&source.root_glb, &resources),
        source_byte_count: total,
        external_resource_uris: expected_uris,
    })
}

fn require_exact_resources(
    expected_uris: &[String],
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<(), ImportDiagnostic> {
    let supplied_uris = resources.keys().cloned().collect::<Vec<_>>();
    if expected_uris == supplied_uris {
        return Ok(());
    }
    let missing = expected_uris
        .iter()
        .find(|uri| !resources.contains_key(*uri));
    let extra = supplied_uris
        .iter()
        .find(|uri| !expected_uris.contains(uri));
    let message = if let Some(uri) = missing {
        format!("referenced resource `{uri}` is missing")
    } else {
        format!(
            "resource `{}` is not referenced by the glTF document",
            extra.expect("unequal sets have a difference")
        )
    };
    Err(error(
        ImportCode::ExternalResource,
        "source.resources",
        message,
        "provide exactly the bounded resource closure referenced by the root document",
    ))
}

fn parse_root(root_json: &[u8]) -> Result<gltf::Gltf, ImportDiagnostic> {
    if root_json.is_empty() || root_json.len() > MAX_SOURCE_BYTES {
        return Err(error(
            ImportCode::SourceTooLarge,
            "source",
            format!(
                "glTF root byte count {} is outside 1..={MAX_SOURCE_BYTES}",
                root_json.len()
            ),
            "supply one bounded glTF JSON root",
        ));
    }
    let parsed = gltf::Gltf::from_slice(root_json).map_err(|failure| {
        error(
            ImportCode::InvalidContainer,
            "source",
            format!("invalid glTF 2.0 JSON source: {failure}"),
            "export a valid glTF 2.0 JSON document",
        )
    })?;
    if parsed.blob.is_some() {
        return Err(error(
            ImportCode::InvalidContainer,
            "source",
            "glTF source closure root must be JSON rather than a GLB container",
            "send .glb sources through the existing binary import path",
        ));
    }
    validate_required_extensions(&parsed)?;
    Ok(parsed)
}

fn parse_glb_root(root_glb: &[u8]) -> Result<gltf::Gltf, ImportDiagnostic> {
    if root_glb.is_empty() || root_glb.len() > MAX_SOURCE_BYTES {
        return Err(error(
            ImportCode::SourceTooLarge,
            "source",
            format!(
                "GLB root byte count {} is outside 1..={MAX_SOURCE_BYTES}",
                root_glb.len()
            ),
            "supply one bounded binary GLB root",
        ));
    }
    let parsed = gltf::Gltf::from_slice(root_glb).map_err(|failure| {
        error(
            ImportCode::InvalidContainer,
            "source",
            format!("invalid binary glTF 2.0 source: {failure}"),
            "export a valid binary glTF 2.0 document",
        )
    })?;
    if parsed.blob.is_none() {
        return Err(error(
            ImportCode::InvalidContainer,
            "source",
            "binary GLB closure root must contain an embedded BIN chunk",
            "send JSON glTF sources through the existing JSON closure path",
        ));
    }
    validate_required_extensions(&parsed)?;
    Ok(parsed)
}

/// The shared parser must allow an omitted core texture source so an admitted
/// `EXT_texture_webp` asset can reach the animated importer. Do not let that
/// parser feature expand the package-level required-extension contract.
fn validate_required_extensions(parsed: &gltf::Gltf) -> Result<(), ImportDiagnostic> {
    for extension in parsed.document.extensions_required() {
        if !matches!(
            extension,
            "EXT_texture_webp" | "KHR_materials_unlit" | "KHR_texture_transform"
        ) {
            return Err(error(
                ImportCode::UnsupportedFeature,
                "source.extensionsRequired",
                format!("required glTF extension `{extension}` is not admitted"),
                "export core glTF data or an admitted required extension",
            ));
        }
    }
    Ok(())
}

fn document_resource_uris(parsed: &gltf::Gltf) -> Vec<(String, &str)> {
    let mut uris = Vec::new();
    for buffer in parsed.document.buffers() {
        if let BufferSource::Uri(uri) = buffer.source() {
            uris.push((format!("source.buffers[{}].uri", buffer.index()), uri));
        }
    }
    for image in parsed.document.images() {
        if let ImageSource::Uri { uri, .. } = image.source() {
            uris.push((format!("source.images[{}].uri", image.index()), uri));
        }
    }
    uris
}

fn validate_resources(
    resources: &[GltfResource],
) -> Result<BTreeMap<String, Vec<u8>>, ImportDiagnostic> {
    if resources.len() > MAX_GLTF_RESOURCE_COUNT {
        return Err(error(
            ImportCode::ResourceLimit,
            "source.resources",
            format!(
                "resource count {} exceeds {MAX_GLTF_RESOURCE_COUNT}",
                resources.len()
            ),
            "reduce the external glTF resource closure",
        ));
    }
    let mut result = BTreeMap::new();
    let mut total = 0usize;
    for (index, resource) in resources.iter().enumerate() {
        let canonical =
            canonical_resource_uri(&resource.uri, &format!("source.resources[{index}].uri"))?;
        if canonical != resource.uri {
            return Err(error(
                ImportCode::MalformedSource,
                format!("source.resources[{index}].uri"),
                format!("resource URI must be canonical `{canonical}`"),
                "pass canonical project-relative resource identities",
            ));
        }
        if resource.bytes.is_empty() || resource.bytes.len() > MAX_GLTF_RESOURCE_BYTES {
            return Err(error(
                ImportCode::ResourceLimit,
                format!("source.resources[{index}].bytes"),
                format!(
                    "resource byte count {} is outside 1..={MAX_GLTF_RESOURCE_BYTES}",
                    resource.bytes.len()
                ),
                "reduce the external resource below the per-resource limit",
            ));
        }
        total = total.checked_add(resource.bytes.len()).ok_or_else(|| {
            resource_limit("source.resources", "total resource byte count overflowed")
        })?;
        if total > MAX_GLTF_TOTAL_RESOURCE_BYTES {
            return Err(resource_limit(
                "source.resources",
                &format!("total resource bytes {total} exceed {MAX_GLTF_TOTAL_RESOURCE_BYTES}"),
            ));
        }
        if result
            .insert(canonical.clone(), resource.bytes.clone())
            .is_some()
        {
            return Err(error(
                ImportCode::MalformedSource,
                format!("source.resources[{index}].uri"),
                format!("duplicate resource URI `{canonical}`"),
                "provide each canonical resource exactly once",
            ));
        }
    }
    Ok(result)
}

fn canonical_resource_uri(uri: &str, path: &str) -> Result<String, ImportDiagnostic> {
    if uri.is_empty()
        || uri.trim() != uri
        || uri.starts_with('/')
        || uri.starts_with('\\')
        || uri.contains('\\')
        || uri.contains('?')
        || uri.contains('#')
        || uri.contains('\0')
    {
        return Err(external_uri(path, uri));
    }
    let decoded = percent_decode(uri).ok_or_else(|| {
        error(
            ImportCode::MalformedSource,
            path,
            format!("resource URI `{uri}` contains invalid percent encoding or UTF-8"),
            "use a valid UTF-8 project-relative URI",
        )
    })?;
    if decoded.starts_with('/')
        || decoded.starts_with('\\')
        || decoded.contains('\\')
        || decoded.contains('?')
        || decoded.contains('#')
        || decoded.contains(':')
        || decoded
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(external_uri(path, uri));
    }
    Ok(decoded)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex(bytes.get(index + 1).copied()?)?;
            let low = hex(bytes.get(index + 2).copied()?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum DataKind {
    Buffer,
    Image,
}

fn resolve_uri_bytes(
    uri: &str,
    path: &str,
    resources: &BTreeMap<String, Vec<u8>>,
    kind: DataKind,
) -> Result<Vec<u8>, ImportDiagnostic> {
    if uri.starts_with("data:") {
        return decode_data_uri(uri, path, kind).map(|(_, bytes)| bytes);
    }
    let canonical = canonical_resource_uri(uri, path)?;
    resources.get(&canonical).cloned().ok_or_else(|| {
        error(
            ImportCode::ExternalResource,
            path,
            format!("referenced resource `{canonical}` is missing"),
            "provide the complete immutable glTF resource closure",
        )
    })
}

fn decode_data_uri(
    uri: &str,
    path: &str,
    kind: DataKind,
) -> Result<(String, Vec<u8>), ImportDiagnostic> {
    let (metadata, encoded) = uri
        .strip_prefix("data:")
        .and_then(|value| value.split_once(','))
        .ok_or_else(|| malformed_data_uri(path))?;
    let mime = metadata
        .strip_suffix(";base64")
        .ok_or_else(|| malformed_data_uri(path))?;
    let allowed = match kind {
        DataKind::Buffer => matches!(mime, "application/octet-stream" | "application/gltf-buffer"),
        DataKind::Image => matches!(mime, "image/png" | "image/jpeg"),
    };
    if !allowed {
        return Err(error(
            ImportCode::UnsupportedFeature,
            path,
            format!("data URI MIME type `{mime}` is not admitted"),
            "use base64 application/octet-stream buffers or PNG/JPEG images",
        ));
    }
    let bytes = BASE64
        .decode(encoded)
        .map_err(|_| malformed_data_uri(path))?;
    if bytes.is_empty() || bytes.len() > MAX_GLTF_RESOURCE_BYTES {
        return Err(resource_limit(
            path,
            &format!(
                "decoded data URI byte count {} is outside 1..={MAX_GLTF_RESOURCE_BYTES}",
                bytes.len()
            ),
        ));
    }
    Ok((mime.to_owned(), bytes))
}

fn rewrite_buffer_views(
    root: &mut Map<String, Value>,
    buffer_offsets: &[usize],
) -> Result<(), ImportDiagnostic> {
    let Some(views) = root.get_mut("bufferViews").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for (index, view) in views.iter_mut().enumerate() {
        let object = view.as_object_mut().ok_or_else(|| invalid_view(index))?;
        let buffer = object
            .get("buffer")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value < buffer_offsets.len())
            .ok_or_else(|| invalid_view(index))?;
        let local = object
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let offset = u64::try_from(buffer_offsets[buffer])
            .ok()
            .and_then(|base| base.checked_add(local))
            .ok_or_else(|| resource_limit("source.bufferViews", "packed offset overflowed"))?;
        object.insert("buffer".to_owned(), Value::from(0));
        object.insert("byteOffset".to_owned(), Value::from(offset));
    }
    Ok(())
}

fn embed_uri_images(
    root: &mut Map<String, Value>,
    parsed: &gltf::Gltf,
    resources: &BTreeMap<String, Vec<u8>>,
    packed_bin: &mut Vec<u8>,
) -> Result<(), ImportDiagnostic> {
    for image in parsed.document.images() {
        let ImageSource::Uri { uri, mime_type } = image.source() else {
            continue;
        };
        let path = format!("source.images[{}].uri", image.index());
        let (resolved_mime, bytes) = if uri.starts_with("data:") {
            decode_data_uri(uri, &path, DataKind::Image)?
        } else {
            let canonical = canonical_resource_uri(uri, &path)?;
            let inferred = image_mime(&canonical).ok_or_else(|| {
                error(
                    ImportCode::UnsupportedFeature,
                    path.clone(),
                    format!("image resource `{canonical}` is not PNG or JPEG"),
                    "use a .png, .jpg, or .jpeg image resource",
                )
            })?;
            if mime_type.is_some_and(|declared| declared != inferred) {
                return Err(error(
                    ImportCode::InvalidContainer,
                    format!("source.images[{}].mimeType", image.index()),
                    format!("declared MIME type does not match `{canonical}`"),
                    "make the image MIME type and extension agree",
                ));
            }
            (
                inferred.to_owned(),
                resources.get(&canonical).cloned().ok_or_else(|| {
                    error(
                        ImportCode::ExternalResource,
                        path.clone(),
                        format!("referenced image `{canonical}` is missing"),
                        "provide the complete immutable glTF resource closure",
                    )
                })?,
            )
        };
        align_four(packed_bin);
        let offset = packed_bin.len();
        packed_bin.extend_from_slice(&bytes);
        let view_index = root
            .get("bufferViews")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let view = Value::Object(Map::from_iter([
            ("buffer".to_owned(), Value::from(0)),
            ("byteOffset".to_owned(), Value::from(offset as u64)),
            ("byteLength".to_owned(), Value::from(bytes.len() as u64)),
        ]));
        root.entry("bufferViews".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .expect("bufferViews created as array")
            .push(view);
        let object = root
            .get_mut("images")
            .and_then(Value::as_array_mut)
            .and_then(|images| images.get_mut(image.index()))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                error(
                    ImportCode::InvalidContainer,
                    format!("source.images[{}]", image.index()),
                    "image entry must be an object",
                    "repair the glTF image entry",
                )
            })?;
        object.remove("uri");
        object.insert("bufferView".to_owned(), Value::from(view_index as u64));
        object.insert("mimeType".to_owned(), Value::from(resolved_mime));
    }
    Ok(())
}

fn image_mime(uri: &str) -> Option<&'static str> {
    let extension = uri.rsplit_once('.')?.1;
    if extension.eq_ignore_ascii_case("png") {
        Some("image/png")
    } else if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
        Some("image/jpeg")
    } else {
        None
    }
}

fn encode_glb(mut json: Vec<u8>, mut bin: Vec<u8>) -> Result<Vec<u8>, ImportDiagnostic> {
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12usize
        .checked_add(8 + json.len())
        .and_then(|value| value.checked_add(8 + bin.len()))
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| resource_limit("source", "packed GLB length exceeds u32"))?;
    let mut output = Vec::with_capacity(total as usize);
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2u32.to_le_bytes());
    output.extend_from_slice(&total.to_le_bytes());
    output.extend_from_slice(&(json.len() as u32).to_le_bytes());
    output.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
    output.extend_from_slice(&json);
    output.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    output.extend_from_slice(&0x004e_4942u32.to_le_bytes());
    output.extend_from_slice(&bin);
    Ok(output)
}

fn closure_hash(root: &[u8], resources: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rusty-engine.gltf-source-closure.v1\0");
    hash_field(&mut hasher, b"root", root);
    for (uri, bytes) in resources {
        hash_field(&mut hasher, uri.as_bytes(), bytes);
    }
    format!("{:x}", hasher.finalize())
}

fn hash_field(hasher: &mut Sha256, name: &[u8], bytes: &[u8]) {
    hasher.update((name.len() as u64).to_le_bytes());
    hasher.update(name);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn closure_byte_count(source: &GltfSourceClosure) -> Result<u64, ImportDiagnostic> {
    closure_byte_count_parts(source.root_json.len(), &source.resources)
}

fn closure_byte_count_parts(
    root_bytes: usize,
    resources: &[GltfResource],
) -> Result<u64, ImportDiagnostic> {
    resources
        .iter()
        .try_fold(root_bytes as u64, |total, resource| {
            total
                .checked_add(resource.bytes.len() as u64)
                .ok_or_else(|| {
                    resource_limit("source.resources", "source closure byte count overflowed")
                })
        })
}

pub(crate) fn glb_json_document(source: &[u8], locus: &str) -> Result<Value, ImportDiagnostic> {
    const GLB_HEADER_BYTES: usize = 12;
    const CHUNK_HEADER_BYTES: usize = 8;
    const JSON_CHUNK_TYPE: u32 = 0x4e4f_534a;
    if source.len() < GLB_HEADER_BYTES + CHUNK_HEADER_BYTES || &source[..4] != b"glTF" {
        return Err(error(
            ImportCode::InvalidContainer,
            locus,
            "source does not contain a binary glTF header and JSON chunk",
            "export a valid binary glTF 2.0 file",
        ));
    }
    let version = u32::from_le_bytes(source[4..8].try_into().expect("fixed header slice"));
    let declared_length =
        u32::from_le_bytes(source[8..12].try_into().expect("fixed header slice")) as usize;
    let json_length =
        u32::from_le_bytes(source[12..16].try_into().expect("fixed chunk header slice")) as usize;
    let chunk_type =
        u32::from_le_bytes(source[16..20].try_into().expect("fixed chunk header slice"));
    let json_end = (GLB_HEADER_BYTES + CHUNK_HEADER_BYTES)
        .checked_add(json_length)
        .filter(|end| *end <= source.len());
    if version != 2
        || declared_length != source.len()
        || chunk_type != JSON_CHUNK_TYPE
        || json_end.is_none()
    {
        return Err(error(
            ImportCode::InvalidContainer,
            locus,
            "GLB header, version, declared length, or JSON chunk is invalid",
            "export a valid binary glTF 2.0 file",
        ));
    }
    serde_json::from_slice(&source[20..json_end.expect("checked JSON end")]).map_err(|failure| {
        error(
            ImportCode::InvalidContainer,
            locus,
            format!("GLB JSON chunk is invalid: {failure}"),
            "repair the embedded glTF JSON document",
        )
    })
}

fn align_four(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

fn invalid_view(index: usize) -> ImportDiagnostic {
    error(
        ImportCode::InvalidContainer,
        format!("source.bufferViews[{index}]"),
        "buffer view has an invalid buffer reference",
        "repair the glTF buffer view",
    )
}

fn malformed_data_uri(path: &str) -> ImportDiagnostic {
    error(
        ImportCode::MalformedSource,
        path,
        "data URI must contain a supported MIME type and valid base64 payload",
        "use a bounded base64 data URI",
    )
}

fn external_uri(path: &str, uri: &str) -> ImportDiagnostic {
    error(
        ImportCode::ExternalResource,
        path,
        format!("resource URI `{uri}` is not a safe project-relative path"),
        "remove network, absolute, traversal, query, fragment, and backslash paths",
    )
}

fn resource_limit(path: &str, message: &str) -> ImportDiagnostic {
    error(
        ImportCode::ResourceLimit,
        path,
        message,
        "reduce the glTF source package below the documented limits",
    )
}

fn error(
    code: ImportCode,
    path: impl Into<String>,
    message: impl Into<String>,
    remedy: impl Into<String>,
) -> ImportDiagnostic {
    ImportDiagnostic::error(code, path, message, remedy)
}
