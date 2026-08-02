use serde::Deserialize;

use crate::{ImportCode, ImportDiagnostic};

pub const SUPPORTED_SOURCE_SCHEMA: u32 = 1;
pub const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_SOURCE_VERTICES: usize = 16_000_000;
pub const MAX_SOURCE_INDICES: usize = 48_000_000;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourceMaterial {
    pub slot: u16,
    pub name: String,
    pub color: [f32; 4],
    #[serde(default)]
    pub texture: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourceGroup {
    pub material_slot: u16,
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCollision {
    VisualOnly,
    AabbFallback,
    Proxy(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceMesh {
    pub schema_version: u32,
    pub name: String,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Option<Vec<f32>>,
    pub indices: Vec<u32>,
    pub materials: Vec<SourceMaterial>,
    pub groups: Vec<SourceGroup>,
    pub collision: SourceCollision,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceParse {
    pub mesh: Option<SourceMesh>,
    pub diagnostics: Vec<ImportDiagnostic>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredSource {
    schema_version: u32,
    name: String,
    positions: Vec<f32>,
    normals: Vec<f32>,
    #[serde(default)]
    uvs: Option<Vec<f32>>,
    indices: Vec<u32>,
    #[serde(default)]
    materials: Option<Vec<SourceMaterial>>,
    #[serde(default)]
    groups: Option<Vec<SourceGroup>>,
    #[serde(default)]
    collision: Option<StoredCollision>,
    #[serde(default)]
    #[serde(rename = "animations")]
    _animations: Option<serde_json::Value>,
    #[serde(default)]
    #[serde(rename = "skins")]
    _skins: Option<serde_json::Value>,
    #[serde(default)]
    #[serde(rename = "morphTargets")]
    _morph_targets: Option<serde_json::Value>,
    #[serde(default)]
    #[serde(rename = "cameras")]
    _cameras: Option<serde_json::Value>,
    #[serde(default)]
    #[serde(rename = "lights")]
    _lights: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StoredCollision {
    Label(String),
    Proxy(StoredProxy),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProxy {
    proxy: String,
}

pub fn parse_source(text: &str, locus: &str) -> SourceParse {
    if text.len() > MAX_SOURCE_BYTES {
        return failed(ImportDiagnostic::error(
            ImportCode::SourceTooLarge,
            locus,
            format!(
                "source is {} bytes; limit is {MAX_SOURCE_BYTES}",
                text.len()
            ),
            "split or simplify the offline source asset",
        ));
    }
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let stored: StoredSource = match serde_path_to_error::deserialize(&mut deserializer) {
        Ok(value) => value,
        Err(error) => {
            let path = error.path().to_string();
            return failed(ImportDiagnostic::error(
                ImportCode::MalformedSource,
                if path.is_empty() {
                    locus.to_owned()
                } else {
                    format!("{locus}#{path}")
                },
                error.inner().to_string(),
                "fix the source mesh JSON shape",
            ));
        }
    };
    if let Err(error) = deserializer.end() {
        return failed(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            locus,
            error.to_string(),
            "remove trailing input",
        ));
    }
    let root: serde_json::Value =
        serde_json::from_str(text).expect("the strict source decode already accepted this JSON");
    let mut diagnostics = Vec::new();
    for (name, present) in [
        ("animations", root.get("animations").is_some()),
        ("skins", root.get("skins").is_some()),
        ("morphTargets", root.get("morphTargets").is_some()),
        ("cameras", root.get("cameras").is_some()),
        ("lights", root.get("lights").is_some()),
    ] {
        if present {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                format!("{locus}#{name}"),
                format!("source feature `{name}` is not supported"),
                "remove the feature or add a dedicated offline front-end",
            ));
        }
    }
    if stored.schema_version != SUPPORTED_SOURCE_SCHEMA {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::UnsupportedSchema,
            locus,
            format!("source schema {} is unsupported", stored.schema_version),
            format!("author schema {SUPPORTED_SOURCE_SCHEMA}"),
        ));
    }
    if stored.name.is_empty() {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{locus}#name"),
            "name must not be empty",
            "supply a scoped-kebab-compatible asset name",
        ));
    }
    if stored.positions.len() / 3 > MAX_SOURCE_VERTICES
        || stored.normals.len() / 3 > MAX_SOURCE_VERTICES
        || stored
            .uvs
            .as_ref()
            .is_some_and(|uvs| uvs.len() / 2 > MAX_SOURCE_VERTICES)
    {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::SourceTooLarge,
            locus,
            "vertex stream exceeds the importer limit",
            "split the mesh into smaller offline assets",
        ));
    }
    if stored.indices.len() > MAX_SOURCE_INDICES {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::SourceTooLarge,
            locus,
            "index stream exceeds the importer limit",
            "split the mesh into smaller offline assets",
        ));
    }
    let collision = match stored.collision {
        None => SourceCollision::VisualOnly,
        Some(StoredCollision::Label(label)) if label == "visualOnly" => SourceCollision::VisualOnly,
        Some(StoredCollision::Label(label)) if label == "aabbFallback" => {
            SourceCollision::AabbFallback
        }
        Some(StoredCollision::Proxy(proxy)) if !proxy.proxy.is_empty() => {
            SourceCollision::Proxy(proxy.proxy)
        }
        Some(_) => {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::MalformedSource,
                format!("{locus}#collision"),
                "collision must be visualOnly, aabbFallback, or a proxy object",
                "choose a supported collision policy",
            ));
            SourceCollision::VisualOnly
        }
    };
    let materials = stored.materials.unwrap_or_else(|| {
        vec![SourceMaterial {
            slot: 0,
            name: "default".to_owned(),
            color: [1.0; 4],
            texture: None,
        }]
    });
    let groups = stored.groups.unwrap_or_else(|| {
        vec![SourceGroup {
            material_slot: 0,
            start: 0,
            count: stored.indices.len() as u32,
        }]
    });
    if diagnostics.iter().any(ImportDiagnostic::is_error) {
        SourceParse {
            mesh: None,
            diagnostics,
        }
    } else {
        SourceParse {
            mesh: Some(SourceMesh {
                schema_version: stored.schema_version,
                name: stored.name,
                positions: stored.positions,
                normals: stored.normals,
                uvs: stored.uvs,
                indices: stored.indices,
                materials,
                groups,
                collision,
            }),
            diagnostics,
        }
    }
}

fn failed(diagnostic: ImportDiagnostic) -> SourceParse {
    SourceParse {
        mesh: None,
        diagnostics: vec![diagnostic],
    }
}
