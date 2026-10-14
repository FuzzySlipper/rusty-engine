//! Shared glTF decoding for conversion and exact-source renderer admission.

use std::borrow::Cow;

/// A decoded glTF document and, for a GLB, its BIN chunk borrowed from the
/// source bytes.
#[derive(Debug, Clone)]
pub struct ParsedGltf<'a> {
    pub document: gltf::Document,
    pub blob: Option<Cow<'a, [u8]>>,
}

/// Decode and validate without rewriting the authored document. gltf-json 1.4
/// implements these enabled material schemas but omits their names from its
/// ENABLED_EXTENSIONS table. Ignore only those two erroneous root diagnostics;
/// every schema/index error and every other unsupported requirement still fails.
pub fn parse_gltf_document(source: &[u8]) -> Result<ParsedGltf<'_>, gltf::Error> {
    use gltf::json::validation::{Error, Validate};
    let (json, blob) = if source.starts_with(b"glTF") {
        let glb = gltf::binary::Glb::from_slice(source)?;
        (gltf::json::Root::from_slice(&glb.json)?, glb.bin)
    } else {
        (gltf::json::Root::from_slice(source)?, None)
    };
    let parsed = ParsedGltf {
        document: gltf::Document::from_json_without_validation(json),
        blob,
    };
    let root = parsed.document.as_json();
    let missing_registry_entries = root
        .extensions_required
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            matches!(
                name.as_str(),
                "KHR_materials_specular" | "KHR_materials_volume"
            )
        })
        .map(|(index, name)| {
            gltf::json::Path::new()
                .field("extensionsRequired")
                .index(index)
                .value_str(name)
                .to_string()
        })
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    root.validate(root, gltf::json::Path::new, &mut |path, error| {
        let path = path();
        if error != Error::Unsupported || !missing_registry_entries.contains(&path.to_string()) {
            errors.push((path, error));
        }
    });
    if errors.is_empty() {
        Ok(parsed)
    } else {
        Err(gltf::Error::Validation(errors))
    }
}
