use std::{collections::BTreeSet, path::Path};

use asset_catalog::{validate_catalog, AssetCatalog, CatalogEntry};
use core_assets::{AssetHash, AssetId, AssetKind};
use gltf::buffer::Source as BufferSource;
use gltf::image::Source as ImageSource;
use render_model::{
    AnimatedMeshAsset, AnimatedMeshEmbeddedMaterialSlot, AnimatedMeshRuntimeFormat,
    AnimationClipDescriptor, MeshBoundsDescriptor,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use voxel_convert::{
    import_animated_mesh_source, import_mesh_source, MeshSourceFormat, MeshSourceImportRequest,
    MAX_CONVERSION_SOURCE_BYTES,
};

use crate::{ImportCode, ImportContext, ImportDiagnostic, SourceUri};

pub const SUPPORTED_ANIMATED_GLB_VERSION: u32 = 2;
pub const MAX_ANIMATED_GLB_MATERIALS: usize = 256;
pub const MAX_ANIMATED_GLB_TEXTURES: usize = 256;
pub const MAX_ANIMATED_GLB_IMAGES: usize = 256;
pub const MAX_ANIMATED_GLB_JOINTS: usize = 4_096;
pub const MAX_ANIMATED_GLB_EMBEDDED_IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ANIMATED_GLB_EMBEDDED_IMAGE_TOTAL_BYTES: usize = 32 * 1024 * 1024;

/// The admitted GLB's embedded animation classification. Both variants retain
/// the existing GLB mesh resource and `AnimatedMeshAsset` wire lifecycle; this
/// readout avoids describing a zero-clip resource as animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlbAnimationKind {
    Static,
    Animated,
}

impl GlbAnimationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Static => "staticGlb",
            Self::Animated => "animatedGlb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedGlbImportReceipt {
    /// `Static` means the admitted GLB has zero embedded clips.
    pub animation_kind: GlbAnimationKind,
    pub source_hash: String,
    pub source_byte_count: u64,
    pub node_count: u32,
    pub mesh_count: u32,
    pub primitive_count: u32,
    pub material_count: u32,
    pub texture_count: u32,
    pub image_count: u32,
    pub skin_count: u32,
    pub joint_count: u32,
    pub clip_count: u32,
    pub channel_count: u32,
    pub keyframe_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAnimatedGlb {
    pub animated_mesh: AnimatedMeshAsset,
    pub catalog: AssetCatalog,
    pub runtime_resource_path: String,
    pub runtime_resource_bytes: Vec<u8>,
    pub receipt: AnimatedGlbImportReceipt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimatedGlbImportOutcome {
    pub assets: Option<ImportedAnimatedGlb>,
    pub diagnostics: Vec<ImportDiagnostic>,
}

impl AnimatedGlbImportOutcome {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(ImportDiagnostic::is_error)
    }
}

pub fn import_animated_glb_asset(
    source_uri: &SourceUri,
    source: &[u8],
    context: &ImportContext,
) -> AnimatedGlbImportOutcome {
    let mut diagnostics = Vec::new();
    let Some((asset_id, name, runtime_resource_path)) =
        source_identity(source_uri, &mut diagnostics)
    else {
        return failed(diagnostics);
    };
    if !context.settings.is_valid()
        || context.settings.scale != 1.0
        || context.settings.generate_collision
        || context.settings.material_namespace.is_some()
    {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::InvalidImportSettings,
            asset_id.as_str(),
            "animated GLB import retains exact source bytes and requires scale=1, collision disabled, and no material namespace",
            "apply scale on authored instances and keep GLB-owned materials/collision policy outside the import",
        ));
        return failed(diagnostics);
    }
    let parsed = match parse_and_preflight(source, source_uri.value()) {
        Ok(parsed) => parsed,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return failed(diagnostics);
        }
    };
    let source_hash_hex = sha256_hex(source);
    let source_hash = format!("sha256:{source_hash_hex}");
    let animated_request = MeshSourceImportRequest {
        source_asset_id: asset_id.as_str().to_owned(),
        asset_version: 1,
        source_path: source_uri.value().to_owned(),
        format: MeshSourceFormat::Glb,
        source_bytes: source.to_vec(),
        expected_source_sha256: Some(source_hash.clone()),
        mesh_primitive: None,
    };
    let (
        animation_kind,
        source_bounds,
        source_material_slots,
        clips,
        channel_count,
        keyframe_count,
    ) = if parsed.document.animations().next().is_some() {
        let imported = match import_animated_mesh_source(&animated_request) {
            Ok(imported) => imported,
            Err(error) => {
                diagnostics.extend(error.diagnostics().iter().map(|item| {
                    ImportDiagnostic::error(
                        map_conversion_code(item.code, &item.message),
                        item.path.clone(),
                        item.message.clone(),
                        conversion_remedy(item.code),
                    )
                }));
                return failed(diagnostics);
            }
        };
        let clips = imported
            .model
            .clips
            .iter()
            .map(|clip| AnimationClipDescriptor {
                id: clip.name.clone(),
                name: Some(clip.name.clone()),
                duration_seconds: Some(
                    clip.duration_microseconds as f32
                        / voxel_convert::ANIMATION_TIMESTAMP_TICKS_PER_SECOND as f32,
                ),
            })
            .collect::<Vec<_>>();
        let channel_count = imported
            .model
            .clips
            .iter()
            .map(|clip| clip.channels.len())
            .sum::<usize>();
        let keyframe_count = imported
            .model
            .clips
            .iter()
            .flat_map(|clip| &clip.channels)
            .map(|channel| channel.timestamps_microseconds.len() as u64)
            .sum::<u64>();
        (
            GlbAnimationKind::Animated,
            imported.source.receipt.metadata.source_bounds,
            imported
                .source
                .receipt
                .metadata
                .material_slots
                .iter()
                .map(|material| material.source_material_slot)
                .collect::<Vec<_>>(),
            clips,
            channel_count,
            keyframe_count,
        )
    } else {
        // `voxel-convert` retains separate static and animated source parsers.
        // This uses the existing bounded static scene parser only to validate
        // and measure a zero-clip GLB; publication remains the same GLB mesh
        // descriptor/resource lifecycle below.
        let static_request = MeshSourceImportRequest {
            source_asset_id: format!("mesh/{name}"),
            ..animated_request
        };
        let imported = match import_mesh_source(&static_request) {
            Ok(imported) => imported,
            Err(error) => {
                diagnostics.extend(error.diagnostics().iter().map(|item| {
                    ImportDiagnostic::error(
                        map_conversion_code(item.code, &item.message),
                        item.path.clone(),
                        item.message.clone(),
                        conversion_remedy(item.code),
                    )
                }));
                return failed(diagnostics);
            }
        };
        (
            GlbAnimationKind::Static,
            imported.receipt.metadata.source_bounds,
            imported
                .receipt
                .metadata
                .material_slots
                .iter()
                .map(|material| material.source_material_slot)
                .collect::<Vec<_>>(),
            Vec::new(),
            0,
            0,
        )
    };
    let bounds = match render_bounds(source_bounds) {
        Ok(bounds) => bounds,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return failed(diagnostics);
        }
    };
    let clip_count = clips.len();
    let embedded_material_slots =
        embedded_material_slots(source_material_slots, parsed.document.materials().count());
    let default_clip = clips
        .iter()
        .find(|clip| clip.id == "idle")
        .or_else(|| clips.first())
        .map(|clip| clip.id.clone());
    let animated_mesh = AnimatedMeshAsset {
        asset: asset_id.as_str().to_owned(),
        runtime_format: AnimatedMeshRuntimeFormat::Glb,
        content_hash: Some(source_hash.clone()),
        clips,
        clip_packs: Vec::new(),
        default_clip,
        embedded_material_slots,
        material_slots: Vec::new(),
        bounds,
    };
    if let Err(error) = animated_mesh.validate() {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::InvalidDescriptor,
            asset_id.as_str(),
            format!("generated animated mesh descriptor is invalid: {error:?}"),
            "repair source clip names, durations, transforms, or bounds",
        ));
        return failed(diagnostics);
    }
    let asset_hash = AssetHash::parse(&source_hash_hex)
        .expect("SHA-256 is a valid lowercase hexadecimal asset hash");
    let catalog = AssetCatalog::from_entries(vec![CatalogEntry::new(asset_id, 1)
        .with_hash(asset_hash)
        .with_source(runtime_resource_path.clone())
        .with_label(name)]);
    let validation = validate_catalog(&catalog);
    if !validation.is_ok() {
        diagnostics.extend(validation.diagnostics().into_iter().map(|item| {
            ImportDiagnostic::error(
                ImportCode::InvalidDescriptor,
                item.path,
                item.message,
                "repair generated animated asset identity or provenance",
            )
        }));
        return failed(diagnostics);
    }
    let primitive_count = parsed
        .document
        .meshes()
        .map(|mesh| mesh.primitives().count())
        .sum::<usize>();
    let joint_count = parsed
        .document
        .skins()
        .map(|skin| skin.joints().count())
        .sum::<usize>();
    AnimatedGlbImportOutcome {
        assets: Some(ImportedAnimatedGlb {
            animated_mesh,
            catalog: catalog.canonical(),
            runtime_resource_path,
            runtime_resource_bytes: source.to_vec(),
            receipt: AnimatedGlbImportReceipt {
                animation_kind,
                source_hash,
                source_byte_count: source.len() as u64,
                node_count: count_u32(parsed.document.nodes().count()),
                mesh_count: count_u32(parsed.document.meshes().count()),
                primitive_count: count_u32(primitive_count),
                material_count: count_u32(parsed.document.materials().count()),
                texture_count: count_u32(parsed.document.textures().count()),
                image_count: count_u32(parsed.document.images().count()),
                skin_count: count_u32(parsed.document.skins().count()),
                joint_count: count_u32(joint_count),
                clip_count: count_u32(clip_count),
                channel_count: count_u32(channel_count),
                keyframe_count,
            },
        }),
        diagnostics,
    }
}

/// Derive dense Engine-facing slots from the admitted source parser's used
/// material slots. Only explicit GLB material indices participate: primitives
/// without a `material` property use Three's default material and have no
/// source material index suitable for an Engine override slot.
fn embedded_material_slots(
    source_material_slots: Vec<u32>,
    glb_material_count: usize,
) -> Vec<AnimatedMeshEmbeddedMaterialSlot> {
    let glb_material_count =
        u32::try_from(glb_material_count).expect("animated GLB material admission bound fits u32");
    source_material_slots
        .into_iter()
        .filter(|source_material_slot| *source_material_slot < glb_material_count)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(
            |(slot, source_material_slot)| AnimatedMeshEmbeddedMaterialSlot {
                slot: u16::try_from(slot)
                    .expect("animated GLB material admission bound keeps dense slots in u16"),
                source_material_slot: u16::try_from(source_material_slot)
                    .expect("animated GLB material admission bound keeps source slots in u16"),
            },
        )
        .collect()
}

fn parse_and_preflight(source: &[u8], locus: &str) -> Result<gltf::Gltf, ImportDiagnostic> {
    if source.is_empty() || source.len() as u64 > MAX_CONVERSION_SOURCE_BYTES {
        return Err(ImportDiagnostic::error(
            ImportCode::SourceTooLarge,
            locus,
            format!(
                "animated GLB byte count {} is outside 1..={MAX_CONVERSION_SOURCE_BYTES}",
                source.len()
            ),
            "supply one bounded binary GLB source",
        ));
    }
    for extension in required_extensions(source, locus)? {
        if extension != "KHR_materials_unlit" {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                "source.extensionsRequired",
                format!("required GLB extension `{extension}` is not admitted"),
                "export core glTF 2.0 data or KHR_materials_unlit only",
            ));
        }
    }
    let parsed = gltf::Gltf::from_slice(source).map_err(|error| {
        ImportDiagnostic::error(
            ImportCode::InvalidContainer,
            locus,
            format!("invalid GLB 2.0 source: {error}"),
            "export a valid binary glTF 2.0 file",
        )
    })?;
    if parsed.blob.is_none() {
        return Err(ImportDiagnostic::error(
            ImportCode::InvalidContainer,
            locus,
            "animated import requires a GLB with one embedded BIN chunk",
            "embed all buffers and select binary glTF export",
        ));
    }
    let document = &parsed.document;
    for extension in document.extensions_used() {
        if extension != "KHR_materials_unlit" {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                "source.extensionsUsed",
                format!("GLB extension `{extension}` is not admitted"),
                "export core glTF 2.0 data or KHR_materials_unlit only",
            ));
        }
    }
    for extension in document.extensions_required() {
        if extension != "KHR_materials_unlit" {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                "source.extensionsRequired",
                format!("required GLB extension `{extension}` is not admitted"),
                "export core glTF 2.0 data or KHR_materials_unlit only",
            ));
        }
    }
    for buffer in document.buffers() {
        if let BufferSource::Uri(uri) = buffer.source() {
            return Err(ImportDiagnostic::error(
                ImportCode::ExternalResource,
                format!("source.buffers[{}]", buffer.index()),
                format!("animated GLB import never resolves external buffer URI `{uri}`"),
                "embed every buffer in the GLB BIN chunk",
            ));
        }
    }
    if document.cameras().next().is_some() {
        return Err(ImportDiagnostic::error(
            ImportCode::UnsupportedFeature,
            "source.cameras",
            "animated mesh resources do not import authored cameras",
            "remove cameras from the exported actor resource",
        ));
    }
    bounded_count(
        "source.materials",
        document.materials().count(),
        MAX_ANIMATED_GLB_MATERIALS,
    )?;
    bounded_count(
        "source.textures",
        document.textures().count(),
        MAX_ANIMATED_GLB_TEXTURES,
    )?;
    bounded_count(
        "source.images",
        document.images().count(),
        MAX_ANIMATED_GLB_IMAGES,
    )?;
    let joint_count = document
        .skins()
        .map(|skin| skin.joints().count())
        .try_fold(0usize, |total, count| total.checked_add(count))
        .ok_or_else(|| resource_limit("source.skins", "joint count overflowed"))?;
    bounded_count("source.skins.joints", joint_count, MAX_ANIMATED_GLB_JOINTS)?;
    let mut embedded_image_bytes = 0usize;
    for image in document.images() {
        match image.source() {
            ImageSource::Uri { .. } => {
                return Err(ImportDiagnostic::error(
                    ImportCode::ExternalResource,
                    format!("source.images[{}]", image.index()),
                    "animated GLB import never resolves external image URIs",
                    "embed PNG or JPEG images in the GLB",
                ));
            }
            ImageSource::View { view, mime_type } => {
                if !matches!(mime_type, "image/png" | "image/jpeg") {
                    return Err(ImportDiagnostic::error(
                        ImportCode::UnsupportedFeature,
                        format!("source.images[{}].mimeType", image.index()),
                        format!("embedded image MIME type `{mime_type}` is not admitted"),
                        "embed a PNG or JPEG image",
                    ));
                }
                if view.length() == 0 || view.length() > MAX_ANIMATED_GLB_EMBEDDED_IMAGE_BYTES {
                    return Err(resource_limit(
                        &format!("source.images[{}]", image.index()),
                        &format!(
                            "embedded image byte count {} is outside 1..={MAX_ANIMATED_GLB_EMBEDDED_IMAGE_BYTES}",
                            view.length()
                        ),
                    ));
                }
                embedded_image_bytes =
                    embedded_image_bytes
                        .checked_add(view.length())
                        .ok_or_else(|| {
                            resource_limit("source.images", "image byte count overflowed")
                        })?;
            }
        }
    }
    if embedded_image_bytes > MAX_ANIMATED_GLB_EMBEDDED_IMAGE_TOTAL_BYTES {
        return Err(resource_limit(
            "source.images",
            &format!(
                "embedded image bytes {embedded_image_bytes} exceed {MAX_ANIMATED_GLB_EMBEDDED_IMAGE_TOTAL_BYTES}"
            ),
        ));
    }
    Ok(parsed)
}

fn required_extensions(source: &[u8], locus: &str) -> Result<Vec<String>, ImportDiagnostic> {
    const GLB_HEADER_BYTES: usize = 12;
    const CHUNK_HEADER_BYTES: usize = 8;
    const JSON_CHUNK_TYPE: u32 = 0x4e4f_534a;
    if source.len() < GLB_HEADER_BYTES + CHUNK_HEADER_BYTES || &source[..4] != b"glTF" {
        return Err(ImportDiagnostic::error(
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
    if version != SUPPORTED_ANIMATED_GLB_VERSION
        || declared_length != source.len()
        || chunk_type != JSON_CHUNK_TYPE
        || json_end.is_none()
    {
        return Err(ImportDiagnostic::error(
            ImportCode::InvalidContainer,
            locus,
            "GLB header, version, declared length, or JSON chunk is invalid",
            "export a valid binary glTF 2.0 file",
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(
        &source[20..json_end.expect("checked JSON end")],
    )
    .map_err(|error| {
        ImportDiagnostic::error(
            ImportCode::InvalidContainer,
            locus,
            format!("GLB JSON chunk is invalid: {error}"),
            "repair the embedded glTF JSON document",
        )
    })?;
    let Some(required) = value.get("extensionsRequired") else {
        return Ok(Vec::new());
    };
    let required = required.as_array().ok_or_else(|| {
        ImportDiagnostic::error(
            ImportCode::InvalidContainer,
            "source.extensionsRequired",
            "extensionsRequired must be an array",
            "repair the embedded glTF JSON document",
        )
    })?;
    required
        .iter()
        .map(|extension| {
            extension.as_str().map(str::to_owned).ok_or_else(|| {
                ImportDiagnostic::error(
                    ImportCode::InvalidContainer,
                    "source.extensionsRequired",
                    "required extension names must be strings",
                    "repair the embedded glTF JSON document",
                )
            })
        })
        .collect()
}

fn source_identity(
    source_uri: &SourceUri,
    diagnostics: &mut Vec<ImportDiagnostic>,
) -> Option<(AssetId, String, String)> {
    let value = source_uri.value();
    let path = value.strip_prefix("file://").unwrap_or(value);
    let Some(file_name) = Path::new(path).file_name().and_then(|value| value.to_str()) else {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            value,
            "animated GLB source URI has no UTF-8 filename",
            "select a named binary GLB source",
        ));
        return None;
    };
    let name = file_name
        .strip_suffix(".glb")
        .or_else(|| file_name.strip_suffix(".GLB"));
    let Some(name) = name.filter(|name| !name.is_empty()) else {
        diagnostics.push(ImportDiagnostic::error(
            ImportCode::UnsupportedFeature,
            value,
            "animated import accepts binary .glb source files only",
            "select a binary GLB file with a lowercase kebab-case filename",
        ));
        return None;
    };
    let asset_text = format!("mesh-animation/{name}");
    match AssetId::parse(&asset_text) {
        Ok(asset_id) if asset_id.kind() == AssetKind::AnimatedMesh => {
            Some((asset_id, name.to_owned(), format!("{name}.glb")))
        }
        Ok(_) => unreachable!("mesh-animation prefix always selects animated mesh kind"),
        Err(error) => {
            diagnostics.push(ImportDiagnostic::error(
                ImportCode::MalformedSource,
                value,
                format!("source filename does not form an animated mesh identity: {error}"),
                "rename the file with lowercase kebab-case path segments",
            ));
            None
        }
    }
}

fn render_bounds(
    bounds: voxel_convert::MeshSourceBounds,
) -> Result<MeshBoundsDescriptor, ImportDiagnostic> {
    let min = bounds.min.map(|value| value as f32);
    let max = bounds.max.map(|value| value as f32);
    if !min.iter().chain(max.iter()).all(|value| value.is_finite()) {
        return Err(ImportDiagnostic::error(
            ImportCode::NonFiniteValue,
            "source.bounds",
            "animated GLB bounds do not fit finite renderer coordinates",
            "normalize the source geometry into finite f32 coordinates",
        ));
    }
    Ok(MeshBoundsDescriptor { min, max })
}

fn bounded_count(path: &str, count: usize, limit: usize) -> Result<(), ImportDiagnostic> {
    if count > limit {
        return Err(resource_limit(
            path,
            &format!("count {count} exceeds {limit}"),
        ));
    }
    Ok(())
}

fn resource_limit(path: &str, message: &str) -> ImportDiagnostic {
    ImportDiagnostic::error(
        ImportCode::ResourceLimit,
        path,
        message,
        "reduce the authored resource below the documented animated GLB limit",
    )
}

fn map_conversion_code(code: &str, message: &str) -> ImportCode {
    match code {
        "conversion.resourceLimit" => ImportCode::ResourceLimit,
        "conversion.unsupportedFeature" | "conversion.unsupportedSource" => {
            ImportCode::UnsupportedFeature
        }
        "conversion.invalidAnimation"
        | "conversion.invalidSkin"
        | "conversion.invalidMorphTarget"
        | "conversion.invalidDeformation" => ImportCode::InvalidAnimation,
        "conversion.invalidGeometry" if message.contains("finite") => ImportCode::NonFiniteValue,
        "conversion.invalidSource" => ImportCode::InvalidContainer,
        _ => ImportCode::MalformedSource,
    }
}

fn conversion_remedy(code: &str) -> &'static str {
    match code {
        "conversion.resourceLimit" => "reduce the GLB below the documented import limits",
        "conversion.unsupportedFeature" | "conversion.unsupportedSource" => {
            "remove unsupported GLB features and embed every resource"
        }
        "conversion.invalidAnimation"
        | "conversion.invalidSkin"
        | "conversion.invalidMorphTarget"
        | "conversion.invalidDeformation" => {
            "repair animation channels, skin bindings, or morph targets"
        }
        _ => "repair the GLB container, hierarchy, geometry, or transforms",
    }
}

fn count_u32(value: usize) -> u32 {
    u32::try_from(value).expect("admission limits keep counts within u32")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn failed(diagnostics: Vec<ImportDiagnostic>) -> AnimatedGlbImportOutcome {
    AnimatedGlbImportOutcome {
        assets: None,
        diagnostics,
    }
}
